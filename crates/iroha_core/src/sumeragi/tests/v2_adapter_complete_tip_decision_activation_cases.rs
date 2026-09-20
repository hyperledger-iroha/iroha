// Actual CompleteTip activation with a durable, unapplied successor Decision.

#[cfg(feature = "bls")]
#[test]
fn production_complete_tip_activates_recovered_unapplied_decision() {
    run_lifecycle_fixture_on_large_stack(
        "production_complete_tip_activates_recovered_unapplied_decision",
        production_complete_tip_activates_recovered_unapplied_decision_body,
    );
}

#[cfg(feature = "bls")]
fn production_complete_tip_activates_recovered_unapplied_decision_body() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let (kura, state, verified, storage_authority, local_signer, retirement) =
        super::super::v2_recovery::production_genesis_complete_tip_fixture_for_test();
    let context = verified.context().clone();
    let local_peer = PeerId::new(local_signer.public_key().clone());
    let local_validator = context
        .roster
        .iter()
        .position(|entry| entry.validator == local_peer)
        .and_then(|position| u32::try_from(position).ok())
        .expect("CompleteTip shutdown signer belongs to the H+1 roster");
    let storage_root = kura.sumeragi_v2_storage_root();
    let wal_path = storage_root
        .join("wal")
        .join(format!("{:020}.wal", context.height));
    let successor_ledger_path = storage_root
        .join("lifecycle-v1")
        .join(hex::encode(context.id().0.as_ref()))
        .join("lifecycle-ledger-v1.norito");
    let empty_successor = std::fs::read(&successor_ledger_path)
        .expect("read the retirement-time empty successor frame");
    let (mut adapter, effects) = SumeragiV2Adapter::open_with_aggregator(
        wal_path.clone(),
        verified.clone(),
        Some(local_validator),
        reducer::Generation::new(1),
        [0x4D; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open production-shaped H+1 adapter");
    assert!(effects.is_empty());
    let (_, keys, _) = authenticated_context();
    let parent = context
        .parent_commit_qc
        .as_ref()
        .expect("canonical parent QC");
    let subject = wire::BlockSubject {
        parent_block_hash: Some(parent.subject.block_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"unapplied complete-tip successor")),
        payload_hash: Hash::new(b"missing complete-tip successor payload"),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: execution_commitment(0x4D),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let envelope = WalEnvelopeV2 {
        protocol_version: wire::PROTOCOL_VERSION,
        persistence_id: 1,
        record: WalRecordV2::Decision(decision.clone()),
    };
    let payload = envelope.encode();
    let receipt = adapter
        .wal
        .append(&payload)
        .expect("fsync the sole successor Decision before restart");
    assert_eq!(
        receipt.sequence().checked_add(1),
        Some(envelope.persistence_id)
    );
    let records = adapter.wal.recovered_records();
    assert_eq!(records.len(), 1);
    assert!(records[0].exactly_matches_receipt(receipt));
    assert_eq!(records[0].payload(), payload.as_slice());
    drop(adapter);
    crate::sumeragi::status::clear_v2_status();
    let authenticated = SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        wal_path,
        verified.clone(),
        Some(local_validator),
        reducer::Generation::new(1),
        [0x4D; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open sealed empty H+1 adapter startup")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _startup)| {
        panic!("authenticate CompleteTip successor Decision: {error}")
    });
    assert!(matches!(
        &authenticated.authority,
        RecoveredWalStartupAuthorityV1::DecisionFetch(_)
    ));
    let signature_policy = super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader;
    let body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        storage_root.join("bodies"),
        context.clone(),
        signature_policy.clone(),
    )
    .expect("open canonical H+1 body store");
    let body_store = quarantined_lifecycle_body_store_for_test(body_store);
    let factory_inputs = try_lifecycle_factory_inputs_for_test(
        &authenticated,
        storage_authority,
        Arc::clone(&state),
        Arc::clone(&kura),
        &local_signer,
    )
    .unwrap_or_else(|error| panic!("bind CompleteTip H+1 lifecycle inputs: {error}"));
    let owner = authenticated
        .open_production_lifecycle_owner_v1(
            &lifecycle_owner_config(),
            4,
            factory_inputs,
            body_store,
        )
        .unwrap_or_else(|error| panic!("open CompleteTip H+1 lifecycle owner: {error}"));
    let repaired_successor = std::fs::read(&successor_ledger_path)
        .expect("read the owner-open recovered-Decision successor frame");
    assert_ne!(
        repaired_successor, empty_successor,
        "owner startup must reproduce the production empty-to-Decision-Fetch publication"
    );
    let ingress = Arc::new(
        crate::sumeragi::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            640 * 1024 * 1024,
            128 * 1024 * 1024,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    ingress
        .configure_roster_for_context(
            context.roster.iter().map(|entry| entry.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .expect("configure CompleteTip H+1 lifecycle ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let ingress_ready = Arc::new(AtomicBool::new(false));
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let launched_at = Instant::now();
    let kura_replica_advert_refresh = Arc::new(
        super::super::v2_worker::KuraReplicaAdvertRefreshOwner::from_kura(
            kura.as_ref(),
            launched_at,
        )
        .expect("bind CompleteTip H+1 Kura advert source"),
    );
    let (exact_output_handoff_owner, transport_owner) =
        super::super::v2_worker::durable_exact_output_handoff_owner_pair();
    let _lane_work =
        super::super::v2_lane_work::V2LaneWorkAdapter::lifecycle_finalization_fixture_for_test(
            context.clone(),
            local_peer.clone(),
            local_signer.clone(),
            Arc::clone(&state),
            Arc::clone(&kura),
            Arc::clone(&output_guard),
            transport_owner,
        )
        .expect("open exact CompleteTip lifecycle lane/output owner");
    let launch_inputs =
        super::super::v2_lifecycle_coordinator::ProductionLifecycleLaunchInputsV1::new(
            launched_at,
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::default(),
            super::super::v2_effects::EffectQueueConfig::default(),
            local_peer.clone(),
            Some(local_validator),
            local_signer.clone(),
            crate::IrohaNetwork::closed_for_tests(),
            Arc::clone(&state),
            Arc::clone(&kura),
            None,
            64,
            64,
            64,
            Arc::clone(&output_guard),
            Arc::clone(&ingress),
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
        );
    let (activated, setup_context) =
        super::super::v2_runner::lifecycle_run_inner::launch_non_pending_lifecycle_height_and_activate_for_test(
            owner,
            launch_inputs,
            Some(retirement),
            &ingress_ready,
            &ingress,
    )
    .unwrap_or_else(|error| panic!("launch sealed CompleteTip H+1 owner: {error}"));
    assert_eq!(setup_context, context.id());

    let published = crate::sumeragi::status::v2_status()
        .expect("CompleteTip Decision publishes actual successor status");
    published
        .validate()
        .expect("pending Decision status preserves the public invariants");
    assert_eq!(published.height, context.height);
    assert_eq!(published.last_committed_height, context.height);
    assert_eq!(published.last_committed_subject, Some(subject));
    assert_eq!(
        published.last_commit_qc.as_ref().map(|qc| qc.certificate),
        Some(decision.as_ref())
    );
    assert_eq!(published.phase, wire::SumeragiV2StatusPhase::PendingApply);
    assert_eq!(
        published.body_state,
        wire::SumeragiV2BodyState::PendingApply
    );
    assert_eq!(
        state.committed_height(),
        usize::try_from(context.height - 1).expect("small predecessor")
    );
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("canonical Kura count"),
        usize::try_from(context.height - 1).expect("small predecessor")
    );
    assert!(ingress_ready.load(Ordering::Acquire));
    assert!(ingress.state.lock().open);
    assert!(!output_guard.restart_required());
    let mut active_runner =
        super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
    activated
        .into_clean_shutdown(&mut active_runner)
        .expect("stop activated pending Decision without applying it");
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    assert!(!output_guard.restart_required());
    assert_eq!(
        state.committed_height(),
        usize::try_from(context.height - 1).expect("small predecessor")
    );
}
