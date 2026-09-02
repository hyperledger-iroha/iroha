macro_rules! signed_lifecycle_attempt_fixture {
    (
        $context:literal;
        $network_id:ident, $epoch:ident, $height_context_id:ident,
        $payload_template:ident, $local_peer:ident, $signer:ident;
        $payload:ident, $reservation_group:ident, $binding:ident, $activated:ident,
        $activate:ident, $prepared_activate:ident, $live_activate:ident,
        $authentication_facts:ident, $sign_cursor:ident
    ) => {
        let (reservation_owner_hash, proposal_identity_hash) =
            autonomous_lane_reservation_identity_hashes_for_proposal(
                $network_id,
                $height_context_id,
                $epoch,
                &$payload_template.origin_proposal,
                &$local_peer,
            )
            .expect(concat!($context, ": derive reservation identities"));
        let mut reservation_keys = $payload_template.reservation_keys.clone();
        for reservation in &mut reservation_keys {
            reservation.reservation_owner_hash = reservation_owner_hash;
            reservation.proposal_identity_hash = proposal_identity_hash;
        }
        let $payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            $network_id,
            $epoch,
            $payload_template.origin_proposal.clone(),
            $payload_template.entrypoints.clone(),
            reservation_keys,
            $payload_template.routing_plans.clone(),
            $payload_template.native_amx_receipts.clone(),
            $local_peer.clone(),
            $signer.private_key(),
        )
        .expect(concat!($context, ": construct signed payload"));
        let $reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
            $payload.reservation_keys.iter(),
        )
        .expect(concat!($context, ": bind reservation group"));
        let $binding = AutonomousLifecycleAttemptBindingV1::from_payload(
            $height_context_id,
            1,
            &$payload,
            $reservation_group,
            &$local_peer,
        )
        .expect(concat!($context, ": bind lifecycle attempt"));
        let before_activate = ProductionInFlightFirstReleaseStateProjection {
            validator_count: 1,
            producer: 1,
            producer_selected_owner: 1,
            replicated_carrier_owners: 0,
            payload_binding_a: 1,
            binding_a: canonical_lane_queue_reservation_group_identity_projection(
                $reservation_group,
            ),
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count: $reservation_group.reservation_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection::default(),
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: 1,
                producer_alive: true,
                ..ProductionInFlightFirstReleaseSessionProjection::default()
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut $activated = before_activate;
        $activated.carrier.kura_active = 1;
        let $activate = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
            actor: 1,
            target: 0,
            before: before_activate,
            after: $activated,
        };
        let $sign_cursor =
            |sequence, previous_cursor_hash, phase: AutonomousLifecycleCursorPhaseV1| {
                let unsigned = AutonomousLifecycleCursorUnsignedV1::new(
                    sequence,
                    previous_cursor_hash,
                    $binding.clone(),
                    phase,
                    $local_peer.clone(),
                )
                .expect(concat!($context, ": construct lifecycle cursor"));
                let preimage = unsigned
                    .signing_preimage()
                    .expect(concat!($context, ": encode cursor signing preimage"));
                let signature = Signature::try_new($signer.private_key(), &preimage)
                    .expect(concat!($context, ": sign lifecycle cursor"));
                unsigned
                    .finalize(
                        <[u8; 96]>::try_from(signature.payload())
                            .expect(concat!($context, ": require 96-byte signature")),
                        &$payload.origin_proposal.descriptor.validator_set,
                    )
                    .expect(concat!($context, ": finalize lifecycle cursor"))
            };
        let $prepared_activate = $sign_cursor(
            1,
            None,
            AutonomousLifecycleCursorPhaseV1::prepared(1, $activate)
                .expect(concat!($context, ": construct Prepared ActivateKura")),
        );
        let $live_activate = $sign_cursor(
            2,
            Some($prepared_activate.cursor_hash()),
            AutonomousLifecycleCursorPhaseV1::live(1, $activated)
                .expect(concat!($context, ": construct Live ActivateKura")),
        );
        let $authentication_facts = ($height_context_id, 1, 1, $reservation_group);
    };
}

macro_rules! install_default_lane_markers_for_lifecycle_test {
    ($context:literal; $kura:ident, $lane_config:ident $(,)?) => {
        for entry in $lane_config.entries() {
            let incarnation = Hash::new(
                format!(
                    "kura-lane-incarnation:{}:{}",
                    entry.lane_id.as_u32(),
                    entry.dataspace_id.as_u64()
                )
                .as_bytes(),
            );
            $kura
                .install_lane_incarnation_marker_for_test(entry, incarnation, 0)
                .expect(concat!($context, ": install lifecycle lane marker"));
        }
    };
}

macro_rules! reopen_single_lifecycle_bootstrap {
    (
        $context:literal; $config:ident, $lane_config:ident, $network_id:ident,
        $local_peer:ident, $lane:ident, $payload:ident, $expected_stage:ident;
        $kura:ident, $generation:ident, $authority:ident
    ) => {
        let ($kura, _) = Kura::open_test_kura_with_configured_lane_config(&$config, &$lane_config)
            .expect(concat!($context, ": reopen Kura"));
        $kura
            .bind_local_peer_id($local_peer.clone())
            .expect(concat!($context, ": bind local peer"));
        let $generation = $kura
            .claim_autonomous_lifecycle_process_generation($network_id, &$local_peer)
            .expect(concat!($context, ": claim process generation"));
        let mut inventory = $kura
            .autonomous_lifecycle_bootstrap_recovery_inventory(
                &$generation,
                $lane.lane_id,
                $lane.dataspace_id,
                $payload.origin_proposal.descriptor.lane_incarnation,
            )
            .expect(concat!($context, ": inventory lifecycle bootstrap"));
        assert_eq!(
            inventory.len(),
            1,
            concat!($context, ": expected one lifecycle bootstrap")
        );
        let $authority = inventory
            .pop()
            .expect(concat!($context, ": missing lifecycle bootstrap authority"));
        assert_eq!(
            $authority.stage(),
            AutonomousLifecycleBootstrapRecoveryStage::$expected_stage,
            concat!($context, ": lifecycle bootstrap stage")
        );
    };
}

macro_rules! prepare_terminal_outcome {
    ($kura:ident, $lane:ident, $payload:ident, $source:ident) => {{
        let _prune_guard = $kura.prune_lock.lock();
        $kura
            .ensure_prune_recovery_not_required()
            .expect("terminal fixture has no prune recovery");
        let _canonical_chain_guard = $kura.canonical_chain_lock.lock();
        let _geometry_guard = $kura.lane_geometry_lock.lock();
        let entry = $kura
            .lane_storage_entry($lane.lane_id)
            .expect("terminal fixture lane entry");
        let _sidecar_guard = $kura.sidecar_lock.lock();
        $kura.prepare_autonomous_lifecycle_terminal_outcome_pending_locked(
            &entry, &$payload, $source,
        )
    }};
}

fn lifecycle_terminal_steady_capacity(
    kura: &Kura,
    physical_bytes: u64,
    global_terminal_reservations: u64,
    post_wsv_reservations: u64,
    context: &str,
) -> u64 {
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .unwrap_or_else(|error| panic!("{context}: measure durable frontier: {error}"));
    let pending_canonical_bytes = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .unwrap_or_else(|error| panic!("{context}: measure pending canonical bytes: {error}"));
    let certified_bundle_reservations = kura
        .certified_bundle_capacity_reserved_bytes()
        .unwrap_or_else(|error| {
            panic!("{context}: measure certified-bundle reservations: {error}")
        });
    physical_bytes
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(global_terminal_reservations))
        .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
        .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .unwrap_or_else(|| panic!("{context}: steady capacity fits u64"))
}

#[test]
fn autonomous_lifecycle_bootstrap_recovers_every_signed_crash_boundary() {
    let temp_dir = TempDir::new().expect("bootstrap temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = autonomous_temp_recovery_catalog();
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let lane = lane_config.primary();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload_template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-autonomous-bootstrap-height-context"),
    ));
    let local_peer = PeerId::new(signer.public_key().clone());
    signed_lifecycle_attempt_fixture!(
        "bootstrap lifecycle fixture";
        network_id, epoch, height_context_id, payload_template, local_peer, signer;
        payload, reservation_group, binding, activated, activate, prepared_activate,
        live_activate, authentication_facts, sign_cursor
    );
    assert!(check_production_in_flight_first_release_transition(activate).is_some());
    let (kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("authenticated bootstrap Kura");
    install_default_lane_markers_for_lifecycle_test!(
        "authenticated bootstrap";
        kura,
        lane_config
    );
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    publish_temp_recovery_catalog_baseline(&kura, &catalog);
    drop(kura);
    let (kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("reopen authenticated bootstrap Kura after catalog publication");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind bootstrap local peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim bootstrap signing generation");
    let sign_bootstrap = |kura: &Kura, generation: &AutonomousLifecycleProcessGenerationClaim| {
        let preimage = kura
            .autonomous_lifecycle_bootstrap_signing_preimage_for_tests(
                generation,
                &payload,
                binding.clone(),
                prepared_activate.clone(),
                live_activate.clone(),
                authentication_facts,
            )
            .expect("build lifecycle bootstrap signature preimage");
        let signature = <[u8; 96]>::try_from(
            Signature::try_new(signer.private_key(), &preimage)
                .expect("sign lifecycle bootstrap")
                .payload(),
        )
        .expect("lifecycle bootstrap signature is exactly 96 bytes");
        (preimage, signature)
    };
    let (bootstrap_preimage, bootstrap_signature) = sign_bootstrap(&kura, &generation_one);
    let persist_bootstrap = |kura: &Kura,
                             generation: &AutonomousLifecycleProcessGenerationClaim,
                             signature: [u8; 96]| {
        kura.persist_autonomous_lifecycle_bootstrap_for_tests(
            generation,
            &payload,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            signature,
            authentication_facts,
        )
    };
    let bootstrap_path = Kura::autonomous_lifecycle_bootstrap_path_for_entry(
        lane,
        temp_dir.path(),
        1,
        payload.origin_proposal.descriptor.proposal_height,
    );
    kura.fail_next_atomic_write_after_temporary_sync_for_test();
    let crash_error = persist_bootstrap(&kura, &generation_one, bootstrap_signature)
        .err()
        .expect("inject bootstrap failure after temp fsync and before rename");
    let bootstrap_atomic_temp = match crash_error {
        Error::IO(_, path)
            if path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX)
                }) =>
        {
            path
        }
        other => panic!("unexpected injected bootstrap publication error: {other}"),
    };
    assert!(bootstrap_atomic_temp.is_file());
    let bootstrap_atomic_bytes =
        fs::read(&bootstrap_atomic_temp).expect("read crash-resident bootstrap temporary");
    let bootstrap_quarantine = bootstrap_atomic_temp.parent().unwrap().join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&bootstrap_atomic_bytes)
    ));
    assert!(!bootstrap_path.exists());
    drop(kura);
    assert!(Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err());
    assert!(bootstrap_atomic_temp.exists());
    let (kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("startup quarantines the real pre-rename bootstrap temporary");
    assert!(!bootstrap_atomic_temp.exists());
    assert_retained_publication_quarantine(&bootstrap_quarantine, &bootstrap_atomic_bytes);
    assert!(!bootstrap_path.exists());
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind bootstrap local peer after crash recovery");
    let authority = persist_bootstrap(&kura, &generation_one, bootstrap_signature)
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
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .is_none(),
        "bootstrap durability must precede every payload mutation",
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "bootstrap deletion must fail before exact Live durability",
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
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
        "startup must reject a multiply linked lifecycle bootstrap",
    );
    fs::remove_file(&hardlink_alias).expect("remove bootstrap hardlink alias");
    let mut tampered_bootstrap = bootstrap_bytes.clone();
    let tampered_byte = tampered_bootstrap
        .last_mut()
        .expect("bootstrap encoding is non-empty");
    *tampered_byte ^= 0x80;
    let canonical_bootstrap =
        Kura::decode_autonomous_lifecycle_bootstrap(&bootstrap_path, &bootstrap_bytes)
            .expect("decode canonical signed bootstrap");
    let mut corrupted_signature = canonical_bootstrap.clone();
    corrupted_signature.signature[0] ^= 0x40;
    let mut substituted_source = canonical_bootstrap.clone();
    substituted_source.body.custody.source =
        AutonomousLifecyclePayloadCustodySourceV1::ProtectedCarrierReceive;
    substituted_source.bootstrap_hash = substituted_source
        .body
        .canonical_hash()
        .expect("hash source-substituted bootstrap body");
    let mut corrupted_evidence = canonical_bootstrap.clone();
    corrupted_evidence.body.custody.evidence_hash =
        Hash::new(b"corrupted bootstrap custody evidence hash");
    corrupted_evidence.bootstrap_hash = corrupted_evidence
        .body
        .canonical_hash()
        .expect("hash evidence-corrupted bootstrap body");
    let wrong_local_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let mut wrong_signer_bootstrap = canonical_bootstrap.clone();
    wrong_signer_bootstrap.signature = <[u8; 96]>::try_from(
        Signature::try_new(wrong_local_signer.private_key(), &bootstrap_preimage)
            .expect("sign bootstrap with wrong local key")
            .payload(),
    )
    .expect("wrong BLS-normal bootstrap signature is exactly 96 bytes");
    let rejected_bootstraps = [
        (
            tampered_bootstrap,
            "startup must reject bootstrap signature, canonical, or self-hash drift",
        ),
        (
            corrupted_signature
                .encode_framed()
                .expect("encode signature-corrupted bootstrap"),
            "startup must reject an exact-size corrupted bootstrap signature",
        ),
        (
            substituted_source
                .encode_framed()
                .expect("encode source-substituted bootstrap"),
            "a ProducerQueue signature must not authorize another custody source",
        ),
        (
            corrupted_evidence
                .encode_framed()
                .expect("encode evidence-corrupted bootstrap"),
            "the full-body signature must reject custody evidence-hash substitution",
        ),
        (
            wrong_signer_bootstrap
                .encode_framed()
                .expect("encode wrong-signer bootstrap"),
            "bootstrap signatures from another local key must fail closed",
        ),
    ];
    for (rejected_bytes, rejection_diagnostic) in rejected_bootstraps {
        fs::write(&bootstrap_path, rejected_bytes).unwrap_or_else(|error| {
            panic!("{rejection_diagnostic}: write rejected bootstrap fixture: {error}")
        });
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "{rejection_diagnostic}",
        );
        fs::write(&bootstrap_path, &bootstrap_bytes).unwrap_or_else(|error| {
            panic!("{rejection_diagnostic}: restore canonical bootstrap: {error}")
        });
    }
    let process_generation_path =
        Kura::autonomous_lifecycle_process_generation_path_for(temp_dir.path());
    let generation_one_bytes =
        fs::read(&process_generation_path).expect("read generation-one record");
    let generation_two_record =
        AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer.clone(), 2)
            .expect("construct generation-two replay record");
    let generation_two_bytes = generation_two_record
        .encode_framed()
        .expect("encode generation-two replay record");
    let prepared_activate_two = sign_cursor(
        1,
        None,
        AutonomousLifecycleCursorPhaseV1::prepared(2, activate)
            .expect("construct generation-two Prepared ActivateKura"),
    );
    let live_activate_two = sign_cursor(
        2,
        Some(prepared_activate_two.cursor_hash()),
        AutonomousLifecycleCursorPhaseV1::live(2, activated)
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
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
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
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
        "startup must reject an oversized bootstrap before decoding",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore bounded bootstrap");
    assert_bootstrap_atomic_temp_recovery_controls(
        temp_dir.path(),
        &config,
        &lane_config,
        &catalog,
        &bootstrap_path,
        &bootstrap_bytes,
    );
    let bootstrap_parent = bootstrap_path.parent().expect("bootstrap path has parent");
    let legacy_bootstrap = bootstrap_parent.join(format!(
        "autonomous_lifecycle_bootstrap_v0_{:020}_{:020}.norito",
        1, payload.origin_proposal.descriptor.proposal_height,
    ));
    fs::write(&legacy_bootstrap, &bootstrap_bytes).expect("write legacy bootstrap path");
    assert!(
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
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
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "startup must reject a symlinked lifecycle bootstrap",
        );
        fs::remove_file(&bootstrap_path).expect("remove bootstrap symlink");
        fs::remove_file(&symlink_target).expect("remove bootstrap symlink target");
        fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore bootstrap after symlink test");
    }
    reopen_single_lifecycle_bootstrap!(
        "bootstrap-only crash boundary";
        config, lane_config, network_id, local_peer, lane, payload, BootstrapOnly;
        kura, _generation_two, authority
    );
    let wrong_height_context = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"wrong-bootstrap-height-context"),
    ));
    assert!(
        kura.authenticate_autonomous_lifecycle_bootstrap_recovery_for_tests(
            authority,
            (wrong_height_context, 1, 1, reservation_group),
        )
        .is_err(),
        "recovery must never advance without fresh exact Queue and height authentication",
    );
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist exact payload after signed bootstrap");
    drop(kura);
    reopen_single_lifecycle_bootstrap!(
        "payload-durable crash boundary";
        config, lane_config, network_id, local_peer, lane, payload, PayloadDurable;
        kura, _generation_three, authority
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "payload durability alone must not authorize bootstrap deletion",
    );
    assert_eq!(
        kura.publish_autonomous_lifecycle_bootstrap_cursor_stage(
            &authority,
            AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
        )
        .expect("publish exact signed Prepared ActivateKura cursor"),
        LaneBlockAuxiliaryPersistenceOutcome::Persisted,
    );
    drop(kura);
    reopen_single_lifecycle_bootstrap!(
        "Prepared-durable crash boundary";
        config, lane_config, network_id, local_peer, lane, payload, PreparedDurable;
        kura, _generation_four, authority
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "Prepared durability alone must not authorize bootstrap deletion",
    );
    assert_eq!(
        kura.publish_autonomous_lifecycle_bootstrap_cursor_stage(
            &authority,
            AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
        )
        .expect("publish exact signed Live successor"),
        LaneBlockAuxiliaryPersistenceOutcome::Persisted,
    );
    drop(kura);
    reopen_single_lifecycle_bootstrap!(
        "Live-durable crash boundary";
        config, lane_config, network_id, local_peer, lane, payload, LiveDurable;
        kura, generation_five, authority
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
    let AutonomousLifecycleBootstrapCompletionOutcome::Completed(completion) = completion else {
        panic!("non-terminal bootstrap completion must return its exact Live cursor");
    };
    assert!(completion.takeover_required());
    assert_eq!(completion.cursor(), &live_activate);
    assert!(!bootstrap_path.exists());
    assert!(
        persist_bootstrap(&kura, &generation_five, bootstrap_signature).is_err(),
        "completed or historical bootstrap bytes must never be replayed around durable state",
    );
    let (current, takeover_lease) = completion.into_cursor_read().into_parts();
    let current = current.expect("completion returns exact historical Live cursor");
    let direct_current_live = sign_cursor(
        3,
        Some(current.cursor_hash()),
        AutonomousLifecycleCursorPhaseV1::live(5, activated)
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
        AutonomousLifecycleCursorPhaseV1::crashed(1, 5, activated, crashed)
            .expect("construct required old-generation crash takeover"),
    );
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(takeover_lease, crashed_cursor.clone(),)
            .expect("publish required old-generation crash takeover")
            .cursor(),
        Some(&crashed_cursor),
        "successful old-generation takeover must return its exact durable cursor",
    );
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
        AutonomousLifecycleCursorPhaseV1::prepared(5, recover)
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
        AutonomousLifecycleCursorPhaseV1::live(5, recovered)
            .expect("construct current-generation recovered Live"),
    );
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(prepared_lease, live_recovered.clone())
            .expect("publish current-generation Live only after Crash and Recover")
            .cursor(),
        Some(&live_recovered),
        "successful current-generation recovery must return its exact durable cursor",
    );
    // Build the canonical merge source in a separate Kura so none of the
    // target crash-stage fixtures gain payload, READY, or certified-session
    // durability before their signed bootstrap reaches that boundary.
    let terminal_source_temp_dir = TempDir::new().expect("terminal source temp dir");
    let terminal_source_config = kura_config_for_dir(&terminal_source_temp_dir, BLOCKS_IN_MEMORY);
    let (terminal_source_kura, _) =
        test_kura_with_default_lane_markers(&terminal_source_config, &lane_config);
    install_autonomous_lane_marker_for_kura(&terminal_source_kura, &lane_config, &payload);
    let terminal_execution =
        canonical_terminal_merge_execution_for_test(&terminal_source_kura, &payload, &signer);
    let (terminal_parent, terminal_carrier, terminal_merge_entry) =
        canonical_terminal_merge_carrier_for_test(terminal_execution, 1);
    let terminal_carrier_height = terminal_carrier.header().height().get();
    let terminal_carrier_hash = terminal_carrier.hash();
    drop(terminal_source_kura);
    for receipt_stage in [
        AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly,
        AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable,
        AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
        AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
    ] {
        let terminal_temp_dir = TempDir::new().expect("terminal bootstrap temp dir");
        let terminal_config = kura_config_for_dir(&terminal_temp_dir, BLOCKS_IN_MEMORY);
        let (terminal_kura, _) =
            test_kura_with_default_lane_markers(&terminal_config, &lane_config);
        terminal_kura
            .bind_local_peer_id(local_peer.clone())
            .expect("bind terminal-bootstrap local peer");
        let terminal_generation = terminal_kura
            .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
            .expect("claim terminal-bootstrap generation");
        install_autonomous_lane_marker_for_kura(&terminal_kura, &lane_config, &payload);
        let (_, terminal_bootstrap_signature) =
            sign_bootstrap(&terminal_kura, &terminal_generation);
        let mut terminal_authority = persist_bootstrap(
            &terminal_kura,
            &terminal_generation,
            terminal_bootstrap_signature,
        )
        .expect("persist terminal lifecycle bootstrap");
        if receipt_stage != AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly {
            assert_eq!(
                terminal_kura
                    .persist_lane_executable_payload(&payload, network_id, epoch)
                    .expect("persist terminal-bootstrap payload"),
                LaneBlockAuxiliaryPersistenceOutcome::Persisted,
            );
            terminal_authority = terminal_kura
                .refresh_autonomous_lifecycle_bootstrap_authority(terminal_authority)
                .expect("refresh payload-durable terminal bootstrap");
        }
        if matches!(
            receipt_stage,
            AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable
                | AutonomousLifecycleBootstrapRecoveryStage::LiveDurable
        ) {
            assert_eq!(
                terminal_kura
                    .publish_autonomous_lifecycle_bootstrap_cursor_stage(
                        &terminal_authority,
                        AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
                    )
                    .expect("publish independently durable Prepared cursor"),
                LaneBlockAuxiliaryPersistenceOutcome::Persisted,
            );
            terminal_authority = terminal_kura
                .refresh_autonomous_lifecycle_bootstrap_authority(terminal_authority)
                .expect("refresh Prepared-durable terminal bootstrap");
        }
        if receipt_stage == AutonomousLifecycleBootstrapRecoveryStage::LiveDurable {
            assert_eq!(
                terminal_kura
                    .publish_autonomous_lifecycle_bootstrap_cursor_stage(
                        &terminal_authority,
                        AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
                    )
                    .expect("publish independently durable Live cursor"),
                LaneBlockAuxiliaryPersistenceOutcome::Persisted,
            );
            terminal_authority = terminal_kura
                .refresh_autonomous_lifecycle_bootstrap_authority(terminal_authority)
                .expect("refresh Live-durable terminal bootstrap");
        }
        assert_eq!(terminal_authority.stage(), receipt_stage);
        let terminal_bootstrap_path = terminal_authority.path.clone();
        terminal_kura
            .store_block(Arc::clone(&terminal_parent))
            .expect("store receipt-terminal carrier parent");
        terminal_kura
            .store_block_with_merge_entry(Arc::clone(&terminal_carrier), &terminal_merge_entry)
            .expect("store receipt-terminal canonical merge carrier");
        let _ = persist_v2_finality_chain_through(
            &terminal_kura,
            NonZeroUsize::new(
                usize::try_from(terminal_carrier_height).expect("carrier height fits usize"),
            )
            .expect("carrier height is non-zero"),
        );
        terminal_kura
            .persist_merge_lane_block_application_receipts(
                &terminal_merge_entry,
                terminal_carrier_height,
                terminal_carrier_hash,
            )
            .expect("persist receipt-terminal merge receipt and frontier");
        let receipt = terminal_kura
            .read_lane_block_application_receipt(
                payload.origin_proposal.descriptor.lane_id,
                payload.origin_proposal.descriptor.lane_block_height,
            )
            .expect("read receipt-terminal merge receipt");
        assert_eq!(receipt.proposal, payload.origin_proposal);
        let terminal_source =
            Kura::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
                .expect("derive exact receipt terminal source");
        let blocked = prepare_terminal_outcome!(terminal_kura, lane, payload, terminal_source);
        let blocked = match blocked {
            Ok(_) => panic!("terminal outcome must wait at {receipt_stage:?}"),
            Err(error) => error,
        };
        assert!(
            blocked
                .to_string()
                .contains("waits for signed lifecycle bootstrap completion"),
            "unexpected terminal-outcome blocker at {receipt_stage:?}: {blocked}",
        );
        let terminal_permit = terminal_kura
            .authenticate_autonomous_lifecycle_bootstrap_recovery_for_tests(
                terminal_authority,
                authentication_facts,
            )
            .expect("authenticate receipt-terminal bootstrap");
        assert!(matches!(
            terminal_kura
                .complete_autonomous_lifecycle_bootstrap(terminal_permit)
                .expect("roll receipt-terminal bootstrap forward to exact Live"),
            AutonomousLifecycleBootstrapCompletionOutcome::AlreadyTerminal,
        ));
        assert!(
            !terminal_bootstrap_path.exists(),
            "receipt-terminal {receipt_stage:?} bootstrap must be deleted only after exact Live",
        );
        let terminal_cursor = terminal_kura
            .read_autonomous_lifecycle_cursor(&payload, &binding, &terminal_generation)
            .expect("read receipt-terminal Live cursor");
        assert_eq!(terminal_cursor.cursor(), Some(&live_activate));
        let terminal_inventory = terminal_kura
            .active_autonomous_lifecycle_attempt_inventory(
                &terminal_generation,
                lane.lane_id,
                lane.dataspace_id,
                payload.origin_proposal.descriptor.lane_incarnation,
            )
            .expect("receipt-terminal completion keeps active inventory valid");
        assert_eq!(terminal_inventory.len(), 1);
        assert_eq!(terminal_inventory[0].executable_payload(), &payload);
        assert_eq!(terminal_inventory[0].cursor(), Some(&live_activate));
        let publishable = prepare_terminal_outcome!(terminal_kura, lane, payload, terminal_source);
        if let Err(error) = publishable {
            panic!(
                "receipt-terminal {receipt_stage:?} completion must leave a Pending-publishable Live unit: {error}"
            );
        }
        drop(terminal_kura);
        let (restarted_terminal_kura, _) =
            Kura::open_test_kura_with_configured_lane_config(&terminal_config, &lane_config)
                .expect("receipt-terminal Live lifecycle unit is restart-valid");
        restarted_terminal_kura
            .bind_local_peer_id(local_peer.clone())
            .expect("rebind restarted terminal-bootstrap local peer");
        let restarted_terminal_generation = restarted_terminal_kura
            .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
            .expect("claim restarted terminal-bootstrap generation");
        let restarted_bootstraps = restarted_terminal_kura
            .autonomous_lifecycle_bootstrap_recovery_inventory(
                &restarted_terminal_generation,
                lane.lane_id,
                lane.dataspace_id,
                payload.origin_proposal.descriptor.lane_incarnation,
            )
            .expect("inventory receipt-terminal lifecycle unit after restart");
        assert!(
            restarted_bootstraps.is_empty(),
            "receipt-terminal {receipt_stage:?} completion must not retain a bootstrap",
        );
    }
}
struct CanonicalTerminalCapacityFixture {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    merge_entry: MergeLedgerEntry,
    carrier: Arc<SignedBlock>,
    outcome_paths: Vec<PathBuf>,
    route_identities: Vec<(LaneId, (u64, u64))>,
    reservation_groups: Vec<LaneQueueReservationGroupBindingV1>,
    pending_lengths: Vec<u64>,
    initial_disk_usage: u64,
    global_terminal_reservations: u64,
    reserved_post_wsv_bytes: u64,
    admitted_limit: u64,
}
fn assert_exact_canonical_terminal_publication(
    publication: AutonomousLifecycleCanonicalCarrierSourceOutcomePublication,
    entry: &MergeLedgerEntry,
    expected_groups: &[LaneQueueReservationGroupBindingV1],
) {
    let queue_authorizations = publication
        .consume_for_v2_apply(entry)
        .expect("capacity publication authenticates its exact committed carrier");
    assert_eq!(queue_authorizations.len(), expected_groups.len());
    for ((group, authorization), expected_group) in
        queue_authorizations.into_iter().zip(expected_groups)
    {
        assert_eq!(group, *expected_group);
        let (authorized_group, ordered_keys, source_outcome_hash) = authorization
            .consume_for_queue()
            .expect("capacity publication contains an exact Queue source authorization");
        assert_eq!(authorized_group, group);
        assert_eq!(
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                .expect("capacity publication preserves FIFO reservation-key order"),
            group,
        );
        assert!(
            source_outcome_hash.as_ref().iter().any(|byte| *byte != 0),
            "capacity publication must bind a non-zero durable source-outcome hash",
        );
    }
}

macro_rules! persist_exact_canonical_terminal_pending {
    ($fixture:ident, $context:literal) => {{
        let publication = $fixture
            .kura
            .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&$fixture.merge_entry)
            .expect(concat!($context, ": persist Pending outcomes"))
            .expect(concat!(
                $context,
                ": execution carrier has Pending outcomes"
            ));
        assert_exact_canonical_terminal_publication(
            publication,
            &$fixture.merge_entry,
            &$fixture.reservation_groups,
        );
    }};
}

fn canonical_terminal_capacity_fixture() -> CanonicalTerminalCapacityFixture {
    let temp_dir = TempDir::new().expect("canonical terminal capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-canonical-terminal-capacity-context"),
    ));
    let lanes = [
        lane_config.primary(),
        lane_config.entry(LaneId::new(1)).expect("lane one"),
    ];
    let payloads = lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            canonical_terminal_payload_for_test(
                lane,
                height_context_id,
                &signer,
                u8::try_from(index + 11).expect("capacity fixture salt fits u8"),
            )
        })
        .collect::<Vec<_>>();
    let network_id = payloads[0].network_id;
    let epoch = payloads[0].epoch;
    let (mut kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("canonical terminal capacity Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind canonical terminal capacity peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim canonical terminal capacity generation");
    let mut executions = Vec::new();
    let mut bindings = Vec::new();
    let mut reservation_groups = Vec::new();
    let mut outcome_paths = Vec::new();
    let mut route_identities = Vec::new();
    for (lane, payload) in lanes.iter().zip(&payloads) {
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, payload);
        executions.push(canonical_terminal_merge_execution_for_test(
            &kura, payload, &signer,
        ));
        let (binding, reservation_group) = install_live_lifecycle_cursor_for_terminal_test(
            &kura,
            &generation,
            payload,
            height_context_id,
            &signer,
        );
        bindings.push(binding);
        reservation_groups.push(reservation_group);
        let descriptor = &payload.origin_proposal.descriptor;
        route_identities.push((
            descriptor.lane_id,
            (descriptor.lane_block_height, descriptor.proposal_height),
        ));
        outcome_paths.push(Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane,
            temp_dir.path(),
            descriptor.lane_block_height,
            descriptor.proposal_height,
        ));
    }
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let entrypoint_count = executions
        .iter()
        .map(|execution| u64::try_from(execution.entrypoints.len()).expect("entrypoint count fits"))
        .sum();
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"terminal capacity base state"));
    let write_set_root = Hash::new(b"terminal capacity write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: crate::merge::merge_application_header_from_carrier(
            &raw_carrier.header(),
        ),
        execution_root: crate::merge::merge_execution_root(&executions),
        entrypoint_count,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&executions)
            .expect("capacity carrier has entrypoints"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("capacity carrier has results"),
        lanes: executions,
        application_write_set_root: Hash::new(b"terminal capacity application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut merge_entry = sample_merge_entry(epoch);
    merge_entry.epoch_id = epoch;
    merge_entry.execution_batch = Some(batch);
    let bound_carrier = bind_merge_entry_to_carrier(raw_carrier, &mut merge_entry);
    let mut executed_carrier = bound_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = Arc::new(executed_carrier);
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent).expect("store capacity parent");
    let baseline_disk_usage = kura
        .refresh_disk_usage_bytes()
        .expect("measure pre-carrier capacity baseline");
    let global_terminal_reservations = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("account globally reserved terminal slots");
    let post_wsv_reservations = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure pre-carrier post-WSV reservations");
    let steady_capacity = lifecycle_terminal_steady_capacity(
        &kura,
        baseline_disk_usage,
        global_terminal_reservations,
        post_wsv_reservations,
        "pre-carrier",
    );
    let carrier_required = kura
        .block_required_bytes_for_budget(carrier.as_ref(), Some(&merge_entry), u64::MAX)
        .expect("account exact carrier and post-WSV components");
    let merge_commit_required = kura
        .merge_commit_required_bytes(carrier.as_ref(), &merge_entry)
        .expect("account exact merge association");
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(carrier.as_ref(), Some(&merge_entry))
        .expect("account exact canonical association stage");
    let admitted_limit = steady_capacity
        .checked_add(carrier_required)
        .and_then(|bytes| bytes.checked_add(merge_commit_required))
        .and_then(|bytes| bytes.checked_add(association_stage_required))
        .expect("original carrier capacity limit fits u64");
    Arc::get_mut(&mut kura)
        .expect("capacity Kura remains exclusive before carrier admission")
        .max_disk_usage_bytes = admitted_limit;
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store capacity carrier at its original exact limit");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist capacity carrier receipts");
    let batch = merge_entry
        .execution_batch
        .as_ref()
        .expect("capacity fixture execution batch");
    let pending_lengths = bindings
        .iter()
        .zip(&batch.lanes)
        .map(|(binding, execution)| {
            let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
                &merge_entry,
                batch,
                execution,
                Kura::merge_lane_block_execution_source(execution),
                carrier_height,
                carrier_hash,
            );
            let source = Kura::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
                .expect("derive capacity Pending source");
            let pending = AutonomousLifecycleTerminalOutcomeV1::pending(binding.clone(), source)
                .expect("construct capacity Pending outcome");
            u64::try_from(
                pending
                    .encode_framed()
                    .expect("encode capacity Pending outcome")
                    .len(),
            )
            .expect("capacity Pending length fits")
        })
        .collect::<Vec<_>>();
    let initial_disk_usage = kura
        .kura_disk_usage_bytes()
        .expect("read capacity fixture disk usage");
    let reserved_post_wsv_bytes = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("read capacity fixture post-WSV envelope");
    assert_eq!(
        reserved_post_wsv_bytes,
        u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)
            .expect("shared carrier transient fits u64"),
        "durable receipt/frontier components leave only the carrier shared transient",
    );
    CanonicalTerminalCapacityFixture {
        _temp_dir: temp_dir,
        kura,
        merge_entry,
        carrier,
        outcome_paths,
        route_identities,
        reservation_groups,
        pending_lengths,
        initial_disk_usage,
        global_terminal_reservations,
        reserved_post_wsv_bytes,
        admitted_limit,
    }
}
#[test]
fn canonical_pending_capacity_preflights_full_set_and_consumes_reserved_slots_idempotently() {
    let mut fixture = canonical_terminal_capacity_fixture();
    let budgets_before = fixture
        .route_identities
        .iter()
        .map(|(lane_id, identity)| {
            fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read pre-Pending reservation")
        })
        .collect::<Vec<_>>();
    assert!(budgets_before.iter().all(|budget| budget.0));
    assert!(fixture.outcome_paths.iter().all(|path| !path.exists()));
    let exact_steady_required = lifecycle_terminal_steady_capacity(
        &fixture.kura,
        fixture.initial_disk_usage,
        fixture.global_terminal_reservations,
        fixture.reserved_post_wsv_bytes,
        "pre-Pending",
    );
    assert!(
        exact_steady_required <= fixture.admitted_limit,
        "materialized receipt/frontier components must fit the original carrier admission limit",
    );
    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura is exclusive")
        .max_disk_usage_bytes = exact_steady_required - 1;
    let first_error = match fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&fixture.merge_entry)
    {
        Ok(_) => panic!("one byte below steady reserved capacity must reject the full set"),
        Err(error) => error,
    };
    assert!(
        first_error
            .to_string()
            .contains("reserved terminal or carrier capacity")
    );
    assert!(fixture.outcome_paths.iter().all(|path| !path.exists()));
    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura remains exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    persist_exact_canonical_terminal_pending!(
        fixture,
        "the original carrier admission limit admits the complete Pending set"
    );
    assert!(fixture.outcome_paths.iter().all(|path| path.is_file()));
    let budgets_after = fixture
        .route_identities
        .iter()
        .zip(&fixture.pending_lengths)
        .zip(&budgets_before)
        .map(|(((lane_id, identity), pending_len), before)| {
            let after = fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read consumed Pending reservation");
            assert!(!after.0, "durable Pending consumes its conceptual slot");
            assert_eq!(after.1, before.1, "reservation consumption is file-neutral");
            assert_eq!(
                after.2,
                before.2 - AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64 + pending_len,
                "exact Pending bytes replace the maximum conceptual reservation",
            );
            after
        })
        .collect::<Vec<_>>();
    let outcome_bytes = fixture
        .outcome_paths
        .iter()
        .map(|path| fs::read(path).expect("read durable capacity Pending"))
        .collect::<Vec<_>>();
    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura remains exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    persist_exact_canonical_terminal_pending!(fixture, "exact Pending retry remains admitted");
    for ((path, expected_bytes), ((lane_id, identity), expected_budget)) in fixture
        .outcome_paths
        .iter()
        .zip(&outcome_bytes)
        .zip(fixture.route_identities.iter().zip(&budgets_after))
    {
        assert_eq!(
            fs::read(path).expect("read retried Pending").as_slice(),
            expected_bytes.as_slice(),
        );
        assert_eq!(
            fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read idempotent reservation budget"),
            *expected_budget,
        );
    }
}
#[test]
fn canonical_complete_releases_shared_transient_and_store_retry_stutters() {
    let mut fixture = canonical_terminal_capacity_fixture();
    Arc::get_mut(&mut fixture.kura)
        .expect("complete fixture Kura is exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    persist_exact_canonical_terminal_pending!(fixture, "publish exact canonical Pending set");
    for (path, group) in fixture
        .outcome_paths
        .iter()
        .zip(&fixture.reservation_groups)
    {
        let bytes = fs::read(path).expect("read exact canonical Pending");
        let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(path, &bytes)
            .expect("decode exact canonical Pending");
        fixture
            .kura
            .complete_autonomous_lifecycle_terminal_outcome(
                *group,
                canonical_terminal_projection_for_test(*group),
                true,
                pending.outcome_hash,
            )
            .expect("Complete must fit the original carrier admission limit");
    }
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read pre-release carrier transient"),
        fixture.reserved_post_wsv_bytes,
        "the shared carrier transient remains until explicit all-Complete release",
    );
    fixture
        .kura
        .release_post_wsv_lane_artifact_budget_reservation(
            &fixture.merge_entry,
            fixture.carrier.header().height().get(),
            fixture.carrier.hash(),
        )
        .expect("all exact Complete members release the carrier envelope");
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read released carrier envelope"),
        0,
    );
    fixture
        .kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clear();
    fixture
        .kura
        .store_block_with_merge_entry(Arc::clone(&fixture.carrier), &fixture.merge_entry)
        .expect("exact store retry authenticates all-Complete tombstones");
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read store-retry reservation state"),
        0,
        "all-Complete store retry must not reinstall a full or shared envelope",
    );
    assert!(
        fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("all-Complete inventory readback")
            .is_empty(),
    );
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read post-inventory reservation state"),
        0,
        "all-Complete recovery inventory must stutter without a reservation map",
    );
}
#[test]
fn retired_release_pending_and_complete_progress_at_the_original_exact_limit() {
    let temp_dir = TempDir::new().expect("release capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.primary();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"release-terminal-capacity-context"),
    ));
    let payload = canonical_terminal_payload_for_test(&lane, height_context_id, &signer, 0x51);
    let network_id = payload.network_id;
    let epoch = payload.epoch;
    let (mut kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("release capacity Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind release capacity peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim release capacity generation");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist release capacity payload");
    let (_, group) = install_live_lifecycle_cursor_for_terminal_test(
        &kura,
        &generation,
        &payload,
        height_context_id,
        &signer,
    );
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist release capacity retirement");
    let barrier = retirement
        .queue_release_barrier()
        .expect("derive release capacity barrier");
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("durably release exact Queue claims");
    let physical = kura
        .kura_disk_usage_bytes()
        .expect("measure pre-Pending release bytes");
    let global_reservations = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure release terminal slot and shared transient");
    let post_wsv_reservations = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure release post-WSV reservations");
    let exact_limit = lifecycle_terminal_steady_capacity(
        &kura,
        physical,
        global_reservations,
        post_wsv_reservations,
        "release",
    );
    Arc::get_mut(&mut kura)
        .expect("release Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    assert!(
        kura.persist_autonomous_lifecycle_release_terminal_outcome_pending(
            &retirement,
            network_id,
            epoch,
        )
        .is_err(),
        "one byte below the admitted global slot must reject before Pending",
    );
    let path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        &lane,
        temp_dir.path(),
        payload.origin_proposal.descriptor.lane_block_height,
        payload.origin_proposal.descriptor.proposal_height,
    );
    assert!(!path.exists());
    Arc::get_mut(&mut kura)
        .expect("release Kura remains exclusive after rejection")
        .max_disk_usage_bytes = exact_limit;
    let source_authorization = kura
        .persist_autonomous_lifecycle_release_terminal_outcome_pending(
            &retirement,
            network_id,
            epoch,
        )
        .expect("release Pending fits its original exact reservation");
    let pending_bytes = fs::read(&path).expect("read exact release Pending");
    let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(&path, &pending_bytes)
        .expect("decode release Pending");
    assert_eq!(
        source_authorization.consume_for_queue(&barrier),
        Some(pending.outcome_hash),
        "the move-only Queue authorization must name the exact durable Pending outcome",
    );
    kura.complete_autonomous_lifecycle_terminal_outcome(
        group,
        release_terminal_projection_for_test(&kura, &payload, &retirement, &barrier),
        false,
        pending.outcome_hash,
    )
    .expect("release Complete fits the same original exact limit");
    let complete_bytes = fs::read(&path).expect("read exact release Complete");
    assert_eq!(complete_bytes.len(), pending_bytes.len());
    assert_eq!(
        kura.autonomous_global_terminal_outcome_reserved_bytes()
            .expect("read post-Complete global reservation"),
        0,
        "Complete consumes the final global stable slot and shared CAS transient",
    );
}
pub(crate) fn persist_merge_application_receipt_for_autonomous_payload_for_test(
    kura: &Kura,
    payload: &LaneExecutablePayloadV1,
) -> LaneBlockApplicationReceiptArtifact {
    let source = kura
        .durable_autonomous_lane_merge_source(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            payload.network_id,
            payload.epoch,
        )
        .expect("read fully authenticated autonomous merge source");
    let execution =
        canonical_terminal_merge_execution_from_durable_source_for_test(payload, source);
    let (parent, carrier, merge_entry) = canonical_terminal_merge_carrier_for_test(execution, 1);
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent)
        .expect("store merge carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store committed merge carrier");
    let _ = persist_v2_finality_chain_through(
        kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits usize"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist terminal merge receipt and frontier");
    kura.read_lane_block_application_receipt(
        payload.origin_proposal.descriptor.lane_id,
        payload.origin_proposal.descriptor.lane_block_height,
    )
    .expect("read terminal merge receipt")
}
#[test]
fn merge_application_receipt_makes_autonomous_auxiliary_persistence_terminal() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let lane_entry = lane_config.primary();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Kura");
    let entrypoint = indexed_log_entrypoint([0xD2; 32], [0xD3; 32]);

    let proposal = merge_entry_with_indexed_entrypoint(entrypoint.clone())
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("merge execution fixture")
        .proposal
        .clone();
    let producer = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::BlsNormal)
        .expect("derive merge execution producer");
    let routing_plan = RoutingPlan::single(crate::queue::RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let reservation = LaneQueueReservationKeyV1 {
        version: LaneQueueReservationKeyV1::VERSION,
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"merge-terminal-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"merge-terminal-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let network_id = test_network_id(b"merge-terminal-autonomous-chain");
    let epoch = 0;
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal.clone(),
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        PeerId::new(producer.public_key().clone()),
        producer.private_key(),
    )
    .expect("construct merge-terminal autonomous payload");
    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        proposal.descriptor.lane_incarnation,
        0,
    )
    .expect("install merge-terminal lane marker");
    let _execution = canonical_terminal_merge_execution_for_test(&kura, &payload, &producer);
    let recovered = kura
        .recover_autonomous_lane_block_payload(&proposal, network_id, epoch)
        .expect("recover autonomous merge execution input");
    let retained_payload = kura
        .read_autonomous_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            network_id,
            epoch,
        )
        .expect("read retained autonomous payload before terminal receipt");
    let retained_input = kura
        .read_lane_block_execution_input(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("read retained execution input before terminal receipt");
    let receipt =
        persist_merge_application_receipt_for_autonomous_payload_for_test(&kura, &payload);
    assert_eq!(
        receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution
    );
    assert!(kura.lane_block_application_receipt_available(&proposal));
    assert_eq!(
        kura.read_autonomous_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            network_id,
            epoch,
        ),
        Some(retained_payload.clone()),
        "terminal authority must retain the crash-sensitive autonomous lifecycle unit"
    );
    assert_eq!(
        kura.read_lane_block_execution_input(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        ),
        Some(retained_input.clone()),
        "terminal authority must retain execution input until bounded indexed-history compaction"
    );
    let terminal_files_before_retries = snapshot_regular_files_recursively(temp_dir.path());
    let terminal_new_view = next_durable_lane_view_certificate_for_kura(
        &proposal, &payload, &producer, network_id, epoch,
    );
    let mut forged_terminal_new_view = terminal_new_view.clone();
    forged_terminal_new_view.certificate.body.target_view = forged_terminal_new_view
        .certificate
        .body
        .target_view
        .saturating_add(1);
    assert!(
        kura.persist_lane_new_view_certificate(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            forged_terminal_new_view,
            network_id,
            epoch,
        )
        .is_err(),
        "terminal state must not turn an invalid NewView certificate into a duplicate",
    );
    assert!(
        matches!(
            kura.persist_lane_new_view_certificate(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                terminal_new_view,
                network_id,
                epoch,
            ),
            Ok(LaneBlockNewViewPersistenceOutcome::AlreadyTerminal)
        ),
        "a terminal receipt must prevent later NewView evidence from mutating the retained attempt",
    );
    assert!(
        matches!(
            kura.persist_lane_executable_payload(&payload, network_id, epoch),
            Ok(LaneBlockAuxiliaryPersistenceOutcome::AlreadyTerminal)
        ),
        "a merge receipt serialized before payload persistence must be terminal"
    );
    assert!(
        matches!(
            kura.persist_lane_block_execution_input(&recovered),
            Ok(LaneBlockAuxiliaryPersistenceOutcome::AlreadyTerminal)
        ),
        "a merge receipt serialized before execution-input persistence must be terminal"
    );
    assert_eq!(
        kura.read_autonomous_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            network_id,
            epoch,
        ),
        Some(retained_payload),
        "terminal payload persistence must not rewrite retained lifecycle evidence"
    );
    assert_eq!(
        kura.read_lane_block_execution_input(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        ),
        Some(retained_input),
        "terminal input persistence must not rewrite retained lifecycle evidence"
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        terminal_files_before_retries,
        "terminal auxiliary retries must not mutate the retained file inventory"
    );
}

#[test]
fn autonomous_lifecycle_live_carrier_hint_promotion_survives_restart() {
    let temp_dir = TempDir::new().expect("carrier-hint promotion temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = autonomous_temp_recovery_catalog();
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let lane = lane_config.primary();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, mut payload_template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let carrier_hint = payload_template
        .origin_proposal
        .payload_block_hint
        .take()
        .expect("fixture global carrier hint");
    let local_peer = PeerId::new(signer.public_key().clone());
    let mut roster = vec![ValidatorPower {
        validator: local_peer.clone(),
        power: 1,
    }];
    while roster.len() < 4 {
        let validator = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let validator = PeerId::new(validator.public_key().clone());
        if roster.iter().all(|entry| entry.validator != validator) {
            roster.push(ValidatorPower {
                validator,
                power: 1,
            });
        }
    }
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    let (offline_cash_mint_finality_epoch_id, offline_cash_mint_finality_epoch_roster) =
        crate::offline_cash_v1_test_fixtures::mint_finality_roster_and_id(
            network_id, epoch, &roster,
        );
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: carrier_hint.proposal_height,
        epoch,
        epoch_end_height: carrier_hint.proposal_height.saturating_add(100),
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: Some(
            iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                snapshot_height: carrier_hint.proposal_height.saturating_sub(1),
                snapshot_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"kura-carrier-hint-promotion-snapshot-block",
                )),
                snapshot_block_creation_time_ms: carrier_hint.proposal_height,
                snapshot_state_hash: Hash::new(b"kura-carrier-hint-promotion-snapshot-state"),
            },
        ),
        quorum: DualQuorum::from_roster(&roster).expect("carrier-hint promotion quorum"),
        roster,
        offline_cash_mint_finality_epoch_id,
        offline_cash_mint_finality_epoch_roster,
        nexus_amx_context_hash: Hash::new(b"kura-carrier-hint-promotion-nexus"),
        execution_policy_hash: Hash::new(b"kura-carrier-hint-promotion-policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xC7; 32],
    };
    context
        .validate()
        .expect("valid carrier-hint promotion height context");
    let height_context_id = context.id();
    signed_lifecycle_attempt_fixture!(
        "carrier-hint promotion lifecycle fixture";
        network_id, epoch, height_context_id, payload_template, local_peer, signer;
        hint_free, reservation_group, binding, after_activate, activate, prepared_activate,
        live_activate, authentication_facts, _sign_cursor
    );
    assert!(hint_free.origin_proposal.payload_block_hint.is_none());
    let hinted = hint_free
        .attach_global_hint_exact(carrier_hint, network_id, epoch)
        .expect("attach the exact protected carrier hint");
    let checked_activate = || {
        check_production_in_flight_first_release_transition(activate)
            .expect("carrier-hint promotion uses the production ActivateKura transition")
    };
    let bootstrap_path = Kura::autonomous_lifecycle_bootstrap_path_for_entry(
        lane,
        temp_dir.path(),
        1,
        hint_free.origin_proposal.descriptor.proposal_height,
    );

    let (kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("authenticated carrier-hint promotion Kura");
    install_default_lane_markers_for_lifecycle_test!(
        "carrier-hint promotion";
        kura,
        lane_config
    );
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &hint_free);
    publish_temp_recovery_catalog_baseline(&kura, &catalog);
    drop(kura);
    let (kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("reopen authenticated carrier-hint promotion Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind carrier-hint promotion local peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim carrier-hint promotion generation");
    let queue_preimage = kura
        .autonomous_lifecycle_bootstrap_signing_preimage_for_tests(
            &generation_one,
            &hint_free,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            authentication_facts,
        )
        .expect("build hint-free Queue bootstrap preimage");
    let queue_signature = <[u8; 96]>::try_from(
        Signature::try_new(signer.private_key(), &queue_preimage)
            .expect("sign hint-free Queue bootstrap")
            .payload(),
    )
    .expect("BLS-normal Queue bootstrap signature is exactly 96 bytes");
    let queue_authority = kura
        .persist_autonomous_lifecycle_bootstrap_for_tests(
            &generation_one,
            &hint_free,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            queue_signature,
            authentication_facts,
        )
        .expect("persist hint-free Queue bootstrap");
    let queue_permit = kura
        .authenticate_autonomous_lifecycle_bootstrap_recovery_for_tests(
            queue_authority,
            authentication_facts,
        )
        .expect("authenticate hint-free Queue bootstrap");
    let AutonomousLifecycleBootstrapCompletionOutcome::Completed(queue_completion) = kura
        .complete_autonomous_lifecycle_bootstrap(queue_permit)
        .expect("complete hint-free Queue bootstrap")
    else {
        panic!("hint-free Queue bootstrap must complete before any terminal receipt");
    };
    assert!(!queue_completion.takeover_required());
    assert_eq!(queue_completion.cursor(), &live_activate);
    assert!(!bootstrap_path.exists());
    assert_eq!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("read durable hint-free Queue payload")
            .0,
        hint_free,
    );

    let locked_round = ConsensusRound {
        context_id: height_context_id,
        height: context.height,
        view: carrier_hint.proposal_view,
    };
    let locked_subject = BlockSubject {
        parent_block_hash: None,
        block_hash: carrier_hint.proposal_block_hash,
        payload_hash: hinted.payload_hash,
    };
    let wrong_locked_subject = BlockSubject {
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"wrong-carrier-hint-promotion-block",
        )),
        ..locked_subject
    };
    assert!(
        kura.authorize_protected_carrier_receive_payload_custody(
            &hinted,
            binding.clone(),
            &context,
            locked_round,
            wrong_locked_subject,
            &local_peer,
            checked_activate(),
        )
        .is_err(),
        "a protected lock for another block must not authorize carrier-hint promotion",
    );
    let promotion_authorization = kura
        .authorize_protected_carrier_receive_payload_custody(
            &hinted,
            binding.clone(),
            &context,
            locked_round,
            locked_subject,
            &local_peer,
            checked_activate(),
        )
        .expect("authorize exact protected-carrier custody");
    let promotion_authorization = kura
        .classify_autonomous_payload_custody_for_persistence(&hinted, promotion_authorization)
        .expect("classify exact carrier-hint promotion")
        .expect("hint-free Live state requires a promotion bootstrap");
    let promotion_preimage = kura
        .autonomous_lifecycle_bootstrap_signing_preimage_with_payload_custody(
            &generation_one,
            &hinted,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            &promotion_authorization,
        )
        .expect("build protected-carrier promotion bootstrap preimage");
    let promotion_signature = <[u8; 96]>::try_from(
        Signature::try_new(signer.private_key(), &promotion_preimage)
            .expect("sign protected-carrier promotion bootstrap")
            .payload(),
    )
    .expect("BLS-normal promotion bootstrap signature is exactly 96 bytes");
    let promotion_authority = kura
        .persist_autonomous_lifecycle_bootstrap_with_payload_custody(
            &generation_one,
            &hinted,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            promotion_signature,
            promotion_authorization,
        )
        .expect("persist protected-carrier promotion bootstrap around exact Live state");
    assert_eq!(
        promotion_authority.stage(),
        AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
    );
    assert_eq!(
        promotion_authority.custody_source(),
        AutonomousLifecyclePayloadCustodySourceV1::ProtectedCarrierReceive,
    );
    assert_eq!(promotion_authority.executable_payload(), &hinted);
    let promotion_path = promotion_authority.path.clone();
    assert_eq!(
        fs::canonicalize(&promotion_path).expect("canonical promotion bootstrap path"),
        fs::canonicalize(&bootstrap_path).expect("canonical expected bootstrap path"),
    );
    assert!(promotion_path.is_file());
    assert_eq!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("read payload before promotion-bootstrap completion")
            .0,
        hint_free,
        "bootstrap publication alone must not mutate the hint-free payload",
    );
    drop(kura);

    reopen_single_lifecycle_bootstrap!(
        "Live-durable carrier-hint promotion boundary";
        config, lane_config, network_id, local_peer, lane, hinted, LiveDurable;
        kura, _generation_two, promotion_authority
    );
    assert_eq!(promotion_authority.executable_payload(), &hinted);
    let promotion_permit = kura
        .authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody(
            promotion_authority,
        )
        .expect("authenticate promotion from durable protected-carrier custody");
    let AutonomousLifecycleBootstrapCompletionOutcome::Completed(promotion_completion) = kura
        .complete_autonomous_lifecycle_bootstrap(promotion_permit)
        .expect("complete restarted carrier-hint promotion bootstrap")
    else {
        panic!("carrier-hint promotion must complete before any terminal receipt");
    };
    assert!(promotion_completion.takeover_required());
    assert_eq!(
        promotion_completion.cursor(),
        &live_activate,
        "carrier-hint promotion must retain the exact pre-crash Live cursor",
    );
    assert!(!promotion_path.exists());
    assert_eq!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("read completed carrier-hinted payload")
            .0,
        hinted,
    );
    let recovered = kura
        .recover_autonomous_lane_block_payload(&hinted.origin_proposal, network_id, epoch)
        .expect("recover exact carrier-hinted payload after promotion restart");
    assert_eq!(recovered.proposal, hinted.origin_proposal);
}
