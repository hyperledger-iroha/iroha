#[cfg(feature = "bls")]
use crate::{BlockMessage, state::State};

#[test]
fn production_lifecycle_owner_factory_opens_the_private_recovered_vote_branch() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary recovered-vote safety store");
    let storage = TempDir::new().expect("temporary recovered-vote lifecycle stores");
    let ledger_root = storage.path().join("ledger");
    let (startup, proposal, manifest, validated) =
        reopen_with_persisted_prepare_intent(&safety, &storage.path().join("body"), 0xC7);
    let commitment = validated.execution_commitment();
    {
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let joined =
            join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut holder);
        let (summary, durable) =
            joined
                .persist_repair_for_test(&ledger_root)
                .unwrap_or_else(|error| {
                    panic!(
                        "seed exact repaired recovered-vote ledger: {}",
                        error.reason()
                    )
                });
        assert!(summary.parent_advanced() && summary.child_live());
        drop(durable);
    }
    crate::sumeragi::status::clear_v2_status();
    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen the exact recovered-vote adapter startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate persisted recovered-vote startup: {error}")
        });
    assert!(authenticated.recovered_phase_vote_for_test().is_some());
    assert!(authenticated.effects.is_empty());
    let mut body_store = super::super::v2_body_store::V2BodyStore::open(
        storage.path().join("body"),
        authenticated.adapter.wire_context.clone(),
    )
    .expect("reopen exact recovered-vote body store");
    body_store
        .revalidate_recovered_markers(|_| Ok::<_, String>(commitment))
        .expect("semantically replay exact recovered-vote body marker");
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal exact recovered-vote body store");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic recovered-vote Serve retainer");
    let _owner = authenticated
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &storage.path().join("serve"),
            body_store,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("open complete recovered-vote lifecycle owner: {error}"));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "the recovered-vote owner remains unpublished until sealed launch activation"
    );
    crate::sumeragi::status::clear_v2_status();
}

#[test]
fn production_lifecycle_owner_factory_rejects_residual_effects_before_storage_open() {
    let safety = TempDir::new().expect("temporary residual-effect safety store");
    let storage = TempDir::new().expect("temporary residual-effect lifecycle stores");
    let mut authenticated = open_recovered_startup_test(&safety)
        .expect("open sealed residual-effect adapter startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate empty recovered WAL: {error}"));
    let tag = authenticated.adapter.current_tag();
    authenticated.effects.push(AdapterEffect::StoreBody {
        tag,
        round: wire::ConsensusRound {
            context_id: authenticated.adapter.wire_context.id(),
            height: authenticated.adapter.wire_context.height,
            view: tag.view(),
        },
        subject: subject(0xF4),
    });
    let body_root = storage.path().join("body-must-remain-absent");
    let ledger_root = storage.path().join("ledger-must-remain-absent");
    let serve_root = storage.path().join("serve-must-remain-absent");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic local Serve retainer");
    let error = match authenticated.open_production_lifecycle_owner_v1_from_roots_for_test(
        &lifecycle_owner_config(),
        4,
        &ledger_root,
        &serve_root,
        &body_root,
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
        &local_signer,
    ) {
        Ok(_owner) => panic!("unadmitted residual effects must fail startup"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "recovered adapter retained unadmitted startup effects"
    );
    assert!(!body_root.exists());
    assert!(!ledger_root.exists());
    assert!(!serve_root.exists());
}

#[test]
fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let kura = Kura::blank_kura_for_testing();
    let storage_root = kura.sumeragi_v2_storage_root();
    let canonical_wal_path = storage_root
        .join("wal")
        .join(format!("{:020}.wal", context().height));
    let authenticated = open_recovered_startup_at_test_path(&canonical_wal_path)
        .expect("open canonical-owner startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate canonical-owner startup: {error}")
        });
    let context = authenticated.adapter.wire_context.clone();
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic canonical-owner retainer");
    let signature_policy = super::super::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
        local_signer.public_key().clone(),
    );
    let body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        storage_root.join("bodies"),
        context.clone(),
        signature_policy.clone(),
    )
    .expect("open Kura-owned body store");
    let storage_authority = RecoveredLifecycleStorageAuthorityV1::for_test(
        kura.as_ref(),
        &verified_genesis(context.clone()),
        signature_policy.clone(),
        AccountId::new(local_signer.public_key().clone()),
    );
    let state = lifecycle_factory_state_for_test(
        Arc::clone(&kura),
        authenticated.adapter.wire_context.network_id,
    );
    let factory_inputs = try_lifecycle_factory_inputs_for_test(
        &authenticated,
        storage_authority,
        Arc::clone(&state),
        Arc::clone(&kura),
        &local_signer,
    )
    .unwrap_or_else(|error| panic!("bind launchable lifecycle factory inputs: {error}"));
    let body_store = quarantined_lifecycle_body_store_for_test(body_store);
    let owner = authenticated
        .open_production_lifecycle_owner_v1(
            &lifecycle_owner_config(),
            4,
            factory_inputs,
            body_store,
        )
        .unwrap_or_else(|error| panic!("open Kura-bound lifecycle owner: {error}"));
    let lifecycle_root = storage_root
        .join("lifecycle-v1")
        .join(hex::encode(context.id().0.as_ref()));
    assert!(lifecycle_root.join("lifecycle-ledger-v1.norito").exists());

    let local_peer = PeerId::new(local_signer.public_key().clone());
    let local_validator = context
        .roster
        .iter()
        .position(|entry| entry.validator == local_peer)
        .and_then(|position| u32::try_from(position).ok())
        .expect("canonical owner signer belongs to the verified roster");
    let leader_wire_ingress = Arc::new(
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
    leader_wire_ingress
        .configure_roster_for_context(
            context.roster.iter().map(|entry| entry.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .expect("configure the launchable owner's exact ingress roster");
    leader_wire_ingress.require_leader_wire_lifecycle_gate();
    let ingress_ready = Arc::new(AtomicBool::new(false));
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let launched_at = Instant::now();
    let kura_replica_advert_refresh = Arc::new(
        super::super::v2_worker::KuraReplicaAdvertRefreshOwner::from_kura(
            kura.as_ref(),
            launched_at,
        )
        .expect("bind the launchable owner's exact Kura advert source"),
    );
    let (exact_output_handoff_owner, _transport_owner) =
        super::super::v2_worker::durable_exact_output_handoff_owner_pair();
    let launch_inputs =
        super::super::v2_lifecycle_coordinator::ProductionLifecycleLaunchInputsV1::new(
            launched_at,
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::default(),
            super::super::v2_effects::EffectQueueConfig::default(),
            local_peer,
            Some(local_validator),
            local_signer.clone(),
            crate::IrohaNetwork::closed_for_tests(),
            state,
            Arc::clone(&kura),
            None,
            64,
            64,
            64,
            Arc::clone(&output_guard),
            Arc::clone(&leader_wire_ingress),
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
        );
    let mut launched = owner
        .launch(launch_inputs)
        .unwrap_or_else(|error| panic!("launch exact Kura-bound lifecycle owner: {error}"));
    assert!(crate::sumeragi::status::v2_status().is_none());
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!leader_wire_ingress.state.lock().open);

    let mut setup_runner =
        super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    let mut activation =
        super::super::v2_runner::ProductionLifecycleRunnerActivationV1::current_height_for_test(
            Arc::clone(&ingress_ready),
            Arc::clone(&leader_wire_ingress),
        );
    launched
        .with_canonical_body_recovery_ingress(
            &mut setup_runner,
            &mut activation,
            |aperture, executor, services| {
                assert!(ingress_ready.load(Ordering::Acquire));
                assert!(leader_wire_ingress.state.lock().open);
                assert!(std::ptr::eq(
                    aperture.ingress(),
                    leader_wire_ingress.as_ref()
                ));
                assert!(executor.lifecycle_live_clocks_are_unarmed());
                assert!(services.matches_lifecycle_executor_output_guard(executor));
                assert!(crate::sumeragi::status::v2_status().is_none());
                Ok::<
                    _,
                    super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1,
                >(())
            },
        )
        .expect("temporarily open the exact preactivation recovery ingress");
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!leader_wire_ingress.state.lock().open);
    assert!(!output_guard.restart_required());
    assert!(crate::sumeragi::status::v2_status().is_none());
    let directive = launched
        .with_runner_setup(&mut setup_runner, |executor, _services| {
            executor.local_proposal_directive().map_err(
                super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1::LocalProposalDirective,
            )
        })
        .expect("read the exact preactivation local-Proposal directive");
    assert!(directive.locked_body().is_none());
    assert!(directive.decided_subject().is_none());
    let recovered_attempt = super::super::v2::RecoveredLifecycleLocalProposalAttemptV1::for_test(
        directive.tag(),
        wire::ConsensusRound {
            context_id: context.id(),
            height: directive.tag().height(),
            view: directive.tag().view(),
        },
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"recovered lifecycle local Proposal attempt",
            )),
            payload_hash: Hash::new(b"recovered lifecycle local Proposal payload"),
        },
    );
    launched.retain_recovered_local_proposal_attempt_for_test(recovered_attempt);
    let (joined_directive, local_proposal_state) = launched
        .initialize_recovered_local_proposal(setup_runner)
        .expect("join the opaque recovered Proposal owner to runner-local state");
    assert_eq!(joined_directive, directive);
    assert!(local_proposal_state.already_attempted(directive));

    let mut activated = launched
        .activate(Instant::now(), activation, local_proposal_state)
        .unwrap_or_else(|error| panic!("activate exact Kura-bound lifecycle owner: {error}"));
    assert!(ingress_ready.load(Ordering::Acquire));
    assert!(leader_wire_ingress.state.lock().open);
    assert_eq!(
        crate::sumeragi::status::v2_status()
            .expect("sealed lifecycle activation publishes status")
            .height_context_id,
        context.id()
    );
    let mut active_runner =
        super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
    activated.with_runner_runtime(
        &mut active_runner,
        |_owner, executor, services, local_proposal| {
            assert_eq!(executor.context(), &context);
            assert!(services.matches_lifecycle_executor_output_guard(executor));
            assert!(local_proposal.already_attempted(directive));
        },
    );
    let finality_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"lifecycle all-row retirement block",
        )),
        payload_hash: iroha_crypto::Hash::new(b"lifecycle all-row retirement payload"),
    };
    let finality_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let finality_artifact = wire::finality::V2FinalityArtifact::new(
        context.clone(),
        finality_subject,
        wire::QuorumCertificate {
            round: finality_round,
            proposal_round: finality_round,
            phase: wire::GlobalPhase::Commit,
            subject: finality_subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                iroha_crypto::Hash::new(b"lifecycle retirement pre-state"),
                iroha_crypto::Hash::new(b"lifecycle retirement post-state"),
                iroha_crypto::Hash::new(b"lifecycle retirement writes"),
                0,
                iroha_crypto::Hash::new(b"lifecycle retirement block execution"),
            ),
            signers: Vec::new(),
            aggregate_signature: Vec::new(),
        },
        Vec::new(),
    );
    let finality_receipt = crate::kura::KuraV2CommitReceipt::for_test(&finality_artifact);
    let cleanup_ready = activated
        .retire_lifecycle_stores_for_test(finality_receipt)
        .unwrap_or_else(|error| panic!("retire exact live lifecycle stores: {error}"));
    let mut cleanup_supervisor = super::super::v2_worker::V2CleanupSupervisor::default();
    let outcome = cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor);
    assert!(outcome.cleanup().warnings().is_empty());
    assert!(outcome.wal_retirement_warning().is_none());
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!leader_wire_ingress.state.lock().open);
    crate::sumeragi::status::clear_v2_status();

    let mismatched_kura = Kura::blank_kura_for_testing();
    let mismatched_root = mismatched_kura.sumeragi_v2_storage_root();
    let mismatched_safety = TempDir::new().expect("temporary mismatched-owner WAL");
    let mismatched = open_recovered_startup_test(&mismatched_safety)
        .expect("open mismatched-WAL startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate mismatched-WAL startup: {error}"));
    let mismatched_context = mismatched.adapter.wire_context.clone();
    let mismatched_body = super::super::v2_body_store::V2BodyStore::open_with_policy(
        mismatched_root.join("bodies"),
        mismatched_context.clone(),
        signature_policy.clone(),
    )
    .expect("open canonical body store for mismatched-WAL case");
    let mismatched_storage = RecoveredLifecycleStorageAuthorityV1::for_test(
        mismatched_kura.as_ref(),
        &verified_genesis(mismatched_context),
        signature_policy.clone(),
        AccountId::new(local_signer.public_key().clone()),
    );
    let mismatched_inputs = lifecycle_factory_inputs_for_test(
        &mismatched,
        mismatched_storage,
        Arc::clone(&mismatched_kura),
        &local_signer,
    );
    let mismatched_body = quarantined_lifecycle_body_store_for_test(mismatched_body);
    let error = match mismatched.open_production_lifecycle_owner_v1(
        &lifecycle_owner_config(),
        4,
        mismatched_inputs,
        mismatched_body,
    ) {
        Ok(_owner) => panic!("an adapter opened on a foreign WAL must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "recovered adapter safety WAL changed its Kura-derived storage path"
    );
    assert!(!mismatched_root.join("lifecycle-v1").exists());

    let foreign_kura = Kura::blank_kura_for_testing();
    let foreign_storage_root = foreign_kura.sumeragi_v2_storage_root();
    let foreign = open_recovered_startup_at_test_path(
        foreign_storage_root
            .join("wal")
            .join(format!("{:020}.wal", context.height)),
    )
    .expect("open foreign-owner startup")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _startup)| panic!("authenticate foreign-owner startup: {error}"));
    let foreign_context = foreign.adapter.wire_context.clone();
    let foreign_body_root = TempDir::new().expect("foreign body-store root");
    let foreign_body = super::super::v2_body_store::V2BodyStore::open_with_policy(
        foreign_body_root.path(),
        foreign_context.clone(),
        signature_policy.clone(),
    )
    .expect("open foreign body store");
    let foreign_storage_authority = RecoveredLifecycleStorageAuthorityV1::for_test(
        foreign_kura.as_ref(),
        &verified_genesis(foreign_context),
        signature_policy.clone(),
        AccountId::new(local_signer.public_key().clone()),
    );
    let foreign_inputs = lifecycle_factory_inputs_for_test(
        &foreign,
        foreign_storage_authority,
        Arc::clone(&foreign_kura),
        &local_signer,
    );
    let foreign_body = quarantined_lifecycle_body_store_for_test(foreign_body);
    let foreign_lifecycle_parent = foreign_kura.sumeragi_v2_storage_root().join("lifecycle-v1");
    let error = match foreign.open_production_lifecycle_owner_v1(
        &lifecycle_owner_config(),
        4,
        foreign_inputs,
        foreign_body,
    ) {
        Ok(_owner) => panic!("a body store outside the Kura layout must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "recovered body-store handoff failed: Sumeragi v2 body-store publication target mismatch"
    );
    assert!(!foreign_lifecycle_parent.exists());

    let wrong_kura = Kura::blank_kura_for_testing();
    let wrong_storage_root = wrong_kura.sumeragi_v2_storage_root();
    let wrong_policy = open_recovered_startup_at_test_path(
        wrong_storage_root
            .join("wal")
            .join(format!("{:020}.wal", context.height)),
    )
    .expect("open wrong-policy startup")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _startup)| panic!("authenticate wrong-policy startup: {error}"));
    let wrong_context = wrong_policy.adapter.wire_context.clone();
    let wrong_body = super::super::v2_body_store::V2BodyStore::open_with_policy(
        wrong_storage_root.join("bodies"),
        wrong_context,
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open canonical-root body store with the wrong policy");
    let wrong_storage_authority = RecoveredLifecycleStorageAuthorityV1::for_test(
        wrong_kura.as_ref(),
        &verified_genesis(wrong_policy.adapter.wire_context.clone()),
        signature_policy,
        AccountId::new(local_signer.public_key().clone()),
    );
    let wrong_inputs = lifecycle_factory_inputs_for_test(
        &wrong_policy,
        wrong_storage_authority,
        Arc::clone(&wrong_kura),
        &local_signer,
    );
    let wrong_body = quarantined_lifecycle_body_store_for_test(wrong_body);
    let error = match wrong_policy.open_production_lifecycle_owner_v1(
        &lifecycle_owner_config(),
        4,
        wrong_inputs,
        wrong_body,
    ) {
        Ok(_owner) => panic!("a wrong body signature policy must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "recovered body-store handoff failed: Sumeragi v2 body-store publication target mismatch"
    );
    assert!(!wrong_storage_root.join("lifecycle-v1").exists());
    crate::sumeragi::status::clear_v2_status();
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn production_empty_genesis_complete_tip_adopts_control_repair_and_launches() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let (kura, state, verified, storage_authority, local_signer, retirement) =
        super::super::v2_recovery::production_empty_genesis_complete_tip_fixture_for_test();
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
    let timeout = adapter
        .timeout_elapsed(adapter.current_tag())
        .expect("persist the exact H+1 TimeoutIntent")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
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
        panic!("authenticate CompleteTip successor TimeoutIntent: {error}")
    });
    assert!(authenticated.has_recovered_control_sign_for_test());
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
        .expect("read the owner-open recovered-control successor frame");
    assert_ne!(
        repaired_successor, empty_successor,
        "owner startup must reproduce the production empty-to-control-repair publication"
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
    let mut lane_work =
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
    let (mut activated, setup_context) =
        super::super::v2_runner::lifecycle_run_inner::launch_non_pending_lifecycle_height_and_activate_for_test(
            owner,
            launch_inputs,
            Some(retirement),
            &ingress_ready,
            &ingress,
    )
    .unwrap_or_else(|error| panic!("launch sealed CompleteTip H+1 owner: {error}"));
    assert_eq!(setup_context, context.id());

    let mut active_runner =
        super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
    let mut block_sync_server =
        super::super::v2_block_sync::V2BlockSyncServer::new(context.network_id.clone(), 64)
            .expect("open CompleteTip block-sync server");
    let mut block_sync =
        super::super::v2_block_sync::V2BlockSyncDiscovery::new(context.clone(), local_peer, 64)
            .expect("open CompleteTip block-sync discovery");
    let mut block_sync_request = None;
    let mut npos_vrf = super::super::v2_npos::V2NposVrfLifecycle::open(
        &context,
        state.as_ref(),
        Some(local_validator),
        &local_signer,
    )
    .expect("open CompleteTip NPoS lifecycle");
    let mut npos_beacon = super::super::v2_beacon::V2GlobalBeaconLifecycle::open(
        &context,
        state.as_ref(),
        Some(local_validator),
        None,
    )
    .expect("open CompleteTip global beacon lifecycle");
    let first = super::super::v2_runner::drain_lifecycle_v2_ingress(
        &mut activated,
        &mut active_runner,
        &ingress,
        &mut lane_work,
        kura.as_ref(),
        &local_signer,
        &mut block_sync_server,
        &mut block_sync,
        &mut block_sync_request,
        &mut npos_vrf,
        &mut npos_beacon,
        1,
        super::super::v2_runner::LifecycleProducerClaimDispositionV1::initial(),
    )
    .expect("dispatch the first active CompleteTip recovered Sign");
    assert_eq!(
        first.producer_claim(),
        super::super::v2_runner::LifecycleProducerClaimDispositionV1::AwaitingCompletion,
        "the recovered Sign worker owns Completion before ProducerTurn may claim"
    );
    assert!(first.requires_yield());
    assert!(
        !output_guard.restart_required(),
        "queueing the recovered Sign must keep consensus output open"
    );

    let completion_deadline = Instant::now() + Duration::from_secs(5);
    let mut producer_claim = first.producer_claim();
    loop {
        let next = super::super::v2_runner::drain_lifecycle_v2_ingress(
            &mut activated,
            &mut active_runner,
            &ingress,
            &mut lane_work,
            kura.as_ref(),
            &local_signer,
            &mut block_sync_server,
            &mut block_sync,
            &mut block_sync_request,
            &mut npos_vrf,
            &mut npos_beacon,
            1,
            producer_claim,
        )
        .expect("settle the active CompleteTip recovered Sign");
        producer_claim = next.producer_claim();
        if producer_claim == super::super::v2_runner::LifecycleProducerClaimDispositionV1::Eligible
        {
            assert!(!next.requires_yield());
            break;
        }
        assert!(next.requires_yield());
        assert!(!output_guard.restart_required());
        if Instant::now() >= completion_deadline {
            panic!("timed out waiting for the CompleteTip recovered Sign completion");
        }
        std::thread::yield_now();
    }
    assert!(
        !output_guard.restart_required(),
        "the recovered Sign Broadcast settlement must keep output open"
    );
    let broadcast_successor = std::fs::read(&successor_ledger_path)
        .expect("read the durable CompleteTip Broadcast successor frame");
    assert_ne!(
        broadcast_successor, repaired_successor,
        "the worker completion must durably replace the recovered Sign with its Broadcast child"
    );
    let producer = activated
        .claim_producer_turn_for_local_proposal(&mut active_runner)
        .unwrap_or_else(|error| {
            panic!("post-Sign ProducerTurn claim must not see an unsettled lease: {error:?}")
        });
    assert!(
        producer.is_none(),
        "the repaired CompleteTip fixture has no ready ProducerTurn"
    );
    activated
        .into_clean_shutdown(&mut active_runner)
        .unwrap_or_else(|error| panic!("stop active CompleteTip H+1 owner: {error}"));

    assert!(!ingress_ready.load(Ordering::Acquire));
    let ingress_state = ingress.state.lock();
    assert!(!ingress_state.open);
    assert!(ingress_state.leader_wire_lifecycle_gate.is_none());
    drop(ingress_state);
    assert!(!output_guard.restart_required());
    assert!(crate::sumeragi::status::v2_status().is_some());
    crate::sumeragi::status::clear_v2_status();
    assert!(crate::sumeragi::status::v2_status().is_none());
}

#[test]
fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network() {
    let kura = Kura::blank_kura_for_testing();
    let storage_root = kura.sumeragi_v2_storage_root();
    let authenticated = open_recovered_startup_at_test_path(
        storage_root
            .join("wal")
            .join(format!("{:020}.wal", context().height)),
    )
    .expect("open exact factory-input startup")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _startup)| panic!("authenticate factory-input startup: {error}"));
    let recovered_context = authenticated.adapter.wire_context.clone();
    let signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic factory-input signer");
    let policy = super::super::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
        signer.public_key().clone(),
    );
    let account = AccountId::new(signer.public_key().clone());
    let storage = || {
        RecoveredLifecycleStorageAuthorityV1::for_test(
            kura.as_ref(),
            &verified_genesis(recovered_context.clone()),
            policy.clone(),
            account.clone(),
        )
    };
    let exact_state =
        lifecycle_factory_state_for_test(Arc::clone(&kura), recovered_context.network_id);
    assert!(
        try_lifecycle_factory_inputs_for_test(
            &authenticated,
            storage(),
            exact_state,
            Arc::clone(&kura),
            &signer,
        )
        .is_ok(),
        "the exact State/Kura/network tuple must mint the move-only factory input"
    );

    let foreign_kura = Kura::blank_kura_for_testing();
    let foreign_state =
        lifecycle_factory_state_for_test(Arc::clone(&foreign_kura), recovered_context.network_id);
    let foreign_kura_error = match try_lifecycle_factory_inputs_for_test(
        &authenticated,
        storage(),
        Arc::clone(&foreign_state),
        Arc::clone(&foreign_kura),
        &signer,
    ) {
        Ok(_inputs) => panic!("a foreign Kura cannot consume the storage seal"),
        Err(error) => error,
    };
    assert_eq!(
        foreign_kura_error.to_string(),
        "recovered lifecycle execution dependencies changed identity"
    );
    let foreign_state_error = match try_lifecycle_factory_inputs_for_test(
        &authenticated,
        storage(),
        foreign_state,
        Arc::clone(&kura),
        &signer,
    ) {
        Ok(_inputs) => panic!("a State backed by another Kura cannot enter the seal"),
        Err(error) => error,
    };
    assert_eq!(
        foreign_state_error.to_string(),
        "recovered lifecycle execution dependencies changed identity"
    );
    let wrong_network_state =
        lifecycle_factory_state_for_test(Arc::clone(&kura), test_network_id(0xFE));
    let wrong_network_error = match try_lifecycle_factory_inputs_for_test(
        &authenticated,
        storage(),
        wrong_network_state,
        Arc::clone(&kura),
        &signer,
    ) {
        Ok(_inputs) => panic!("a foreign State network cannot enter the seal"),
        Err(error) => error,
    };
    assert_eq!(
        wrong_network_error.to_string(),
        "recovered lifecycle execution dependencies changed identity"
    );
    assert!(
        !storage_root.join("lifecycle-v1").exists(),
        "input binding must not open lifecycle storage"
    );

    let exact_state =
        lifecycle_factory_state_for_test(Arc::clone(&kura), recovered_context.network_id);
    let placeholder_cadence = exact_state.sumeragi_block_cadence();
    let authenticated_cadence = placeholder_cadence
        .checked_add(Duration::from_millis(1))
        .expect("fixture authenticated cadence remains representable");
    assert_ne!(
        placeholder_cadence, authenticated_cadence,
        "fixture must distinguish recovered cadence from placeholder State"
    );
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(crate::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender.clone(),
    ));
    let cadence_inputs = authenticated
        .bind_production_lifecycle_owner_factory_inputs_v1(
            super::super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1::for_test(
                signer.clone(),
                authenticated_cadence,
            ),
            storage(),
            exact_state,
            queue,
            Arc::clone(&kura),
            None,
            None,
            events_sender,
        )
        .expect("authenticated cadence must cross the runner-only factory seal");
    assert_eq!(cadence_inputs.block_cadence, authenticated_cadence);
}

#[test]
fn recovered_lifecycle_factory_inputs_reject_a_same_context_foreign_startup() {
    let kura = Kura::blank_kura_for_testing();
    let storage_root = kura.sumeragi_v2_storage_root();
    let wal_path = storage_root
        .join("wal")
        .join(format!("{:020}.wal", context().height));
    let first = open_recovered_startup_at_test_path(&wal_path)
        .expect("open first exact startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate first startup: {error}"));
    let second = open_recovered_startup_at_test_path(&wal_path)
        .expect("open second same-context startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate second startup: {error}"));
    let recovered_context = first.adapter.wire_context.clone();
    let signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic exact-startup splice signer");
    let policy = super::super::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
        signer.public_key().clone(),
    );
    let storage = RecoveredLifecycleStorageAuthorityV1::for_test(
        kura.as_ref(),
        &verified_genesis(recovered_context.clone()),
        policy.clone(),
        AccountId::new(signer.public_key().clone()),
    );
    let factory_inputs =
        lifecycle_factory_inputs_for_test(&first, storage, Arc::clone(&kura), &signer);
    let body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        storage_root.join("bodies"),
        recovered_context,
        policy,
    )
    .expect("open exact-startup splice body store");
    let body_store = quarantined_lifecycle_body_store_for_test(body_store);
    let error = match second.open_production_lifecycle_owner_v1(
        &lifecycle_owner_config(),
        4,
        factory_inputs,
        body_store,
    ) {
        Ok(_owner) => panic!("a same-context foreign startup cannot consume the factory seal"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "recovered lifecycle execution dependencies changed identity"
    );
    assert!(
        !storage_root.join("lifecycle-v1").exists(),
        "exact-startup rejection must precede lifecycle store creation"
    );
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assert_safety_wal_retention(path: &std::path::Path, retained: bool) {
    assert_eq!(path.exists(), retained, "unexpected safety WAL retention");
}
fn exercise_pending_kura_production_lifecycle(
    owner: super::super::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    verified: VerifiedHeightContext,
    context: wire::HeightContext,
    state: Arc<State>,
    queue: Arc<crate::queue::Queue>,
    kura: Arc<Kura>,
    local_signer: KeyPair,
    expected: super::super::v2_recovery::PendingKuraApply,
    safety_wal_path: std::path::PathBuf,
    finalize: bool,
) {
    let local_peer = PeerId::new(local_signer.public_key().clone());
    let local_validator = context
        .roster
        .iter()
        .position(|entry| entry.validator == local_peer)
        .and_then(|position| u32::try_from(position).ok())
        .expect("pending Kura lifecycle signer belongs to the verified roster");
    let leader_wire_ingress = Arc::new(
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
    leader_wire_ingress
        .configure_roster_for_context(
            context.roster.iter().map(|entry| entry.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .expect("configure pending Kura lifecycle ingress");
    leader_wire_ingress.require_leader_wire_lifecycle_gate();
    let ingress_ready = Arc::new(AtomicBool::new(false));
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let launched_at = Instant::now();
    let kura_replica_advert_refresh = Arc::new(
        super::super::v2_worker::KuraReplicaAdvertRefreshOwner::from_kura(
            kura.as_ref(),
            launched_at,
        )
        .expect("bind pending Kura advert source"),
    );
    let (exact_output_handoff_owner, transport_owner) =
        super::super::v2_worker::durable_exact_output_handoff_owner_pair();
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
            8,
            64,
            Arc::clone(&output_guard),
            Arc::clone(&leader_wire_ingress),
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
        );
    let launched = owner
        .launch(launch_inputs)
        .unwrap_or_else(|error| panic!("launch pending Kura lifecycle owner: {error}"));
    let mut setup_runner =
        super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    let activation =
        super::super::v2_runner::ProductionLifecyclePendingKuraRunnerActivationV1::for_test(
            Arc::clone(&ingress_ready),
            Arc::clone(&leader_wire_ingress),
        );
    let mut pending = launched
        .install_pending_kura_apply(&mut setup_runner)
        .unwrap_or_else(|error| panic!("install exact pending Kura replay: {error}"));
    pending
        .with_runner_setup(&mut setup_runner, |executor, services| {
            super::super::v2_runner::reconcile_executor_locked_body_for_pending_kura_test(
                executor, services,
            )
            .unwrap_or_else(|error| {
                panic!("reconcile pending Kura locked body before Apply recovery: {error}")
            });
            Ok::<
                _,
                super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1,
            >(())
        })
        .expect("mirror production pending Kura pre-activation reconciliation");
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!leader_wire_ingress.state.lock().open);
    assert!(crate::sumeragi::status::v2_status().is_none());

    use super::super::v2_effects::PendingKuraApplyRecoveryStage as Stage;
    let completion_deadline = Instant::now() + Duration::from_secs(5);
    let mut recovery_stages = Vec::new();
    loop {
        let progress = pending
            .drive_apply_recovery_turn(&mut setup_runner, 64)
            .unwrap_or_else(|error| panic!("drive pending Kura Apply recovery: {error}"));
        let (completed, observed_stage) = match progress {
            super::super::v2_lifecycle_coordinator::ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                stage,
                ..
            }
            | super::super::v2_lifecycle_coordinator::ProductionPendingKuraApplyRecoveryProgressV1::Waiting {
                stage,
                ..
            } => (false, stage),
            super::super::v2_lifecycle_coordinator::ProductionPendingKuraApplyRecoveryProgressV1::Completed {
                ..
            } => (true, Stage::Completed),
        };
        if recovery_stages.last() != Some(&observed_stage) {
            recovery_stages.push(observed_stage);
        }
        pending
            .with_runner_setup(&mut setup_runner, |executor, services| {
                assert!(executor.lifecycle_live_clocks_are_unarmed());
                if completed {
                    assert!(services.matches_installed_pending_kura_tip(expected));
                }
                Ok::<
                    _,
                    super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1,
                >(())
            })
            .expect("inspect closed pending Kura lifecycle");
        assert!(!ingress_ready.load(Ordering::Acquire));
        assert!(!leader_wire_ingress.state.lock().open);
        assert!(crate::sumeragi::status::v2_status().is_none());
        if completed {
            break;
        }
        if Instant::now() >= completion_deadline {
            let (status, owner_flags, recovered_retry_keys, io) = pending
                .with_runner_setup(&mut setup_runner, |executor, services| {
                    Ok::<
                        _,
                        super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1,
                    >((
                        executor.status(),
                        executor.pending_kura_apply_owner_flags_for_test(),
                        executor.recovered_durable_validate_retry_keys_for_test(),
                        services.pending_kura_apply_io_snapshot_for_test(),
                    ))
                })
                .expect("inspect stalled pending Kura Apply ownership");
            panic!(
                "timed out waiting for pending Kura Apply completion: progress={progress:?}, status={status:?}, owner_flags={owner_flags:?}, recovered_retry_keys={recovered_retry_keys:?}, io={io:?}"
            );
        }
        std::thread::yield_now();
    }
    assert!(
        recovery_stages.ends_with(&[Stage::ApplicationDispatched, Stage::Completed]),
        "pending Kura typed Apply crossed an unexpected stage sequence: {recovery_stages:?}"
    );
    assert_eq!(
        u64::try_from(state.committed_height()).expect("pending Kura State height fits u64"),
        expected.height()
    );
    assert_eq!(
        kura.get_block(std::num::NonZeroUsize::new(1).expect("pending Kura height"))
            .expect("read applied pending Kura block")
            .hash(),
        expected.block_hash()
    );

    let prepared = pending
        .prepare_lane_recovery(
            &mut setup_runner,
            &queue,
            |installed, _executor, _services| {
                assert_eq!(installed, expected);
                super::super::v2_lane_work::V2LaneWorkAdapter::pending_kura_lifecycle_fixture_for_test(
                    verified.context().clone(),
                    local_peer,
                    local_signer,
                    Arc::clone(&state),
                    Arc::clone(&kura),
                    installed,
                    Arc::clone(&output_guard),
                    transport_owner,
                )
                .map_err(super::super::v2_runner::V2RunnerError::from)
            },
        )
        .unwrap_or_else(|error| panic!("prepare affine pending Kura lane recovery: {error}"));
    let mut activated = prepared
        .activate_no_clock(activation)
        .unwrap_or_else(|error| panic!("activate exact pending Kura no-clock lifecycle: {error}"));
    assert!(ingress_ready.load(Ordering::Acquire));
    assert!(leader_wire_ingress.state.lock().open);
    assert!(crate::sumeragi::status::v2_status().is_some());
    let mut active_runner =
        super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
    activated.with_runner_runtime(&mut active_runner, |executor, services, lane_work| {
        assert!(executor.lifecycle_live_clocks_are_unarmed());
        assert!(executor.ready_to_finish());
        assert!(services.matches_installed_pending_kura_tip(expected));
        assert!(services.matches_lifecycle_lane_work(lane_work));
    });

    if !finalize {
        activated
            .into_clean_shutdown(&mut active_runner)
            .unwrap_or_else(|error| panic!("cleanly stop active pending Kura lifecycle: {error}"));
        assert!(!ingress_ready.load(Ordering::Acquire));
        assert!(!leader_wire_ingress.state.lock().open);
        assert!(!output_guard.restart_required());
        crate::sumeragi::status::clear_v2_status();
        return;
    }

    let (finalized, mut lane_work) = activated
        .into_finalized_rollover(&mut active_runner)
        .unwrap_or_else(|error| panic!("finalize pending Kura lifecycle owner: {error}"));
    assert_safety_wal_retention(&safety_wal_path, true);
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!leader_wire_ingress.state.lock().open);
    let (_, artifact) = finalized.finality();
    lane_work
        .retain_merge_sidecars_for_global_view(
            artifact.commit_qc.round.view,
            None,
            Some(artifact.subject),
        )
        .unwrap_or_else(|error| panic!("retain pending Kura Decision sidecar: {error}"));
    let mut successor = context;
    successor.height = successor
        .height
        .checked_add(1)
        .expect("pending Kura successor height remains in range");
    successor.parent_commit_qc = Some(artifact.commit_qc.clone());
    successor
        .validate()
        .expect("pending Kura successor context is valid");
    let (post_output, retained_sidecars) = finalized
        .rollover_outputs(&mut active_runner, lane_work, &successor, 64)
        .unwrap_or_else(|error| panic!("roll over pending Kura lifecycle outputs: {error}"));
    assert_safety_wal_retention(&safety_wal_path, false);
    let cleanup_ready = post_output
        .retire_lifecycle_stores()
        .unwrap_or_else(|error| panic!("retire pending Kura lifecycle stores: {error}"));
    let mut cleanup_supervisor = super::super::v2_worker::V2CleanupSupervisor::default();
    let outcome = cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor);
    drop(retained_sidecars);
    assert!(outcome.cleanup().warnings().is_empty());
    assert!(outcome.wal_retirement_warning().is_none());
    assert!(!output_guard.restart_required());
    crate::sumeragi::status::clear_v2_status();
}

#[cfg(feature = "bls")]
#[test]
fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies() {
    if std::thread::current().name() != Some("production-lifecycle-marker-replay") {
        return run_marker_replay_test_on_stack();
    }
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    for (
        marker,
        persist_matching_outcome,
        shutdown_before_activation,
        shutdown_after_activation,
        pending_kura_finalize,
    ) in [
        (0xB1_u8, true, false, false, None),
        (0xB2_u8, false, false, false, None),
        (0xB3_u8, true, true, false, None),
        (0xB4_u8, true, false, true, None),
        (0xB5_u8, true, false, false, Some(false)),
        (0xB6_u8, true, false, false, Some(true)),
    ] {
        let kura = Kura::blank_kura_for_testing();
        let storage_root = kura.sumeragi_v2_storage_root();
        let (mut recovered_context, keys, proofs) = authenticated_context();
        let genesis_key = KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::Ed25519)
            .expect("deterministic production marker-replay genesis key");
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let state = Arc::new(
            crate::state::State::new_with_chain_and_network_id_for_testing(
                crate::state::World::with(
                    [],
                    [iroha_data_model::Registrable::build(
                        iroha_data_model::account::Account::new(genesis_account.clone()),
                        &genesis_account,
                    )],
                    [],
                ),
                Arc::clone(&kura),
                crate::query::store::LiveQueryStore::start_test(),
                "sumeragi-v2-lifecycle-test"
                    .parse()
                    .expect("lifecycle fixture chain id"),
                recovered_context.network_id,
            ),
        );
        recovered_context.nexus_amx_context_hash =
            super::super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref());
        recovered_context.execution_policy_hash =
            super::super::v2_recovery::committed_execution_policy_hash(state.as_ref())
                .expect("derive marker-replay execution policy");
        recovered_context
            .validate()
            .expect("marker-replay context remains valid");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(crate::queue::Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events_sender.clone(),
        ));
        let semantic_probe = super::super::v2_apply::V2ApplyService::new(
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            None,
            None,
            state.sumeragi_block_cadence(),
            genesis_account.clone(),
            events_sender.clone(),
            proofs.clone(),
        );
        let round = wire::ConsensusRound {
            context_id: recovered_context.id(),
            height: recovered_context.height,
            view: 0,
        };
        let transaction = iroha_data_model::transaction::TransactionBuilder::new_genesis(
            genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([iroha_data_model::isi::SetParameter::new(
            iroha_data_model::parameter::Parameter::Sumeragi(
                iroha_data_model::parameter::SumeragiParameter::MaxClockDriftMs(u64::from(marker)),
            ),
        )])
        .sign(genesis_key.private_key());
        let creation_time_ms = (transaction.creation_time() + Duration::from_millis(1))
            .as_millis()
            .try_into()
            .expect("marker-replay genesis creation time fits u64");
        let mut header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("marker-replay height is non-zero"),
            None,
            None,
            None,
            creation_time_ms,
            round.view,
        );
        let confidential_features = {
            let state_view = state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                state_view.world(),
                &state_view.zk,
                state_view.sccp_registry.as_ref(),
                recovered_context.height,
            );
            (!digest.is_empty()).then_some(digest)
        };
        header.set_confidential_features(confidential_features);
        let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
            &state.nexus_snapshot(),
            recovered_context.height,
        );
        let mut block_builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        block_builder.push_transaction(transaction);
        block_builder.set_da_proof_policies(Some(proof_policy_bundle));
        let block = block_builder
            .try_build_with_signature(0, genesis_key.private_key())
            .expect("sign canonical production marker-replay genesis")
            .canonical_resultless_proposal();
        let canonical_wire = block
            .encode_wire()
            .expect("encode production marker-replay body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let chunks = wire::encode_payload_chunks(recovered_context.da_layout, &canonical_wire)
            .expect("encode production marker-replay chunks");
        let manifest = wire::PayloadManifest::derive(
            &recovered_context,
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("marker-replay body length fits u64"),
            &chunks,
        )
        .expect("derive production marker-replay manifest");
        let semantic_commitment = semantic_probe
            .revalidate_recovered_candidate(&recovered_context, &block)
            .expect("derive exact production marker-replay outcome");
        let signature_policy = super::super::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
            genesis_key.public_key().clone(),
        );
        let mut body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
            storage_root.join("bodies"),
            recovered_context.clone(),
            signature_policy.clone(),
        )
        .expect("open production marker-replay body store");
        let durable = body_store
            .store(manifest.clone(), canonical_wire.clone())
            .expect("persist production marker-replay body");
        let validated_receipt = if persist_matching_outcome {
            let outcome = body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Ok::<_, String>(semantic_commitment)
                })
                .expect("persist matching semantic marker");
            Some(
                outcome
                    .validated_receipt()
                    .expect("matching semantic outcome is validated")
                    .clone(),
            )
        } else {
            let _ = body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        "deliberately mismatched recovered rejection".to_owned(),
                    )
                })
                .expect("persist mismatched semantic marker");
            None
        };
        let mut decision = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: semantic_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut decision, &keys);

        if let Some(finalize) = pending_kura_finalize {
            let task = super::super::v2_effects::ApplyTask::for_test(
                u64::from(marker),
                super::super::v2_core::EventTag::new(
                    recovered_context.height,
                    round.view,
                    super::super::v2_core::Generation::INITIAL,
                ),
                subject,
                decision.clone(),
                validated_receipt.expect("pending Kura fixture has a validated body"),
            );
            semantic_probe.fail_after_kura_store_for_test();
            assert!(matches!(
                semantic_probe.execute(&recovered_context, &mut body_store, &task),
                Err(super::super::v2_apply::V2ApplyError::InjectedCrashAfterKuraStore)
            ));
            drop(body_store);
            assert_eq!(state.committed_height(), 0);
            assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
            let expected = super::super::v2_recovery::PendingKuraApply::for_test(
                recovered_context.id(),
                recovered_context.height,
                subject.block_hash,
            );
            let recovered_body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
                storage_root.join("bodies"),
                recovered_context.clone(),
                signature_policy.clone(),
            )
            .expect("reopen pending Kura body store")
            .into_quarantined_recovered_startup()
            .expect("quarantine pending Kura validation markers");
            let wal_path = storage_root
                .join("wal")
                .join(format!("{:020}.wal", recovered_context.height));
            let authenticated = write_and_reopen_authenticated_wal_startup_at_path(
                wal_path.clone(),
                &recovered_context,
                &proofs,
                0,
                [marker; 32],
                vec![WalRecordV2::Decision(decision.clone())],
            )
            .bind_pending_kura_apply(expected)
            .unwrap_or_else(|(error, _startup)| panic!("bind exact pending Kura startup: {error}"))
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|error| panic!("authenticate pending Kura WAL replay: {error}"));
            let verified =
                VerifiedHeightContext::genesis(recovered_context.clone(), proofs.clone())
                    .expect("verify pending Kura lifecycle context");
            let storage = RecoveredLifecycleStorageAuthorityV1::for_test(
                kura.as_ref(),
                &verified,
                signature_policy.clone(),
                genesis_account.clone(),
            );
            let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
                .expect("deterministic pending Kura Serve signer");
            let factory_inputs = authenticated
                .bind_production_lifecycle_owner_factory_inputs_v1(
                    super::super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1::for_test(
                        local_signer.clone(),
                        state.sumeragi_block_cadence(),
                    ),
                    storage,
                    Arc::clone(&state),
                    Arc::clone(&queue),
                    Arc::clone(&kura),
                    None,
                    None,
                    events_sender.clone(),
                )
                .unwrap_or_else(|error| panic!("bind pending Kura lifecycle inputs: {error}"));
            let owner = authenticated
                .open_production_lifecycle_owner_v1(
                    &lifecycle_owner_config(),
                    4,
                    factory_inputs,
                    recovered_body_store,
                )
                .unwrap_or_else(|error| panic!("open pending Kura lifecycle owner: {error}"));
            exercise_pending_kura_production_lifecycle(
                owner,
                verified,
                recovered_context,
                state,
                queue,
                kura,
                local_signer,
                expected,
                wal_path,
                finalize,
            );
            continue;
        }
        let prepromoted_error = match body_store.into_quarantined_recovered_startup() {
            Ok(_body_store) => {
                panic!("a caller-promoted marker cannot enter production quarantine")
            }
            Err(error) => error,
        };
        assert_eq!(
            prepromoted_error.to_string(),
            "recovered Sumeragi v2 validation markers were already promoted before startup"
        );
        assert!(
            !storage_root.join("lifecycle-v1").exists(),
            "pre-promoted marker rejection must precede lifecycle-store creation"
        );

        let wal_path = storage_root
            .join("wal")
            .join(format!("{:020}.wal", recovered_context.height));
        let authenticated = write_and_reopen_authenticated_wal_startup_at_path(
            wal_path,
            &recovered_context,
            &proofs,
            0,
            [marker; 32],
            vec![WalRecordV2::Decision(decision.clone())],
        )
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate production marker-replay startup: {error}")
        });
        let verified = VerifiedHeightContext::genesis(recovered_context.clone(), proofs.clone())
            .expect("verify production marker-replay context");
        let storage = RecoveredLifecycleStorageAuthorityV1::for_test(
            kura.as_ref(),
            &verified,
            signature_policy.clone(),
            genesis_account,
        );
        let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
            .expect("deterministic production marker-replay Serve signer");
        let factory_inputs = authenticated
            .bind_production_lifecycle_owner_factory_inputs_v1(
                super::super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1::for_test(
                    local_signer.clone(),
                    state.sumeragi_block_cadence(),
                ),
                storage,
                Arc::clone(&state),
                Arc::clone(&queue),
                Arc::clone(&kura),
                None,
                None,
                events_sender.clone(),
            )
            .unwrap_or_else(|error| panic!("bind production marker-replay inputs: {error}"));
        let recovered_body_store = quarantined_lifecycle_body_store_for_test(
            super::super::v2_body_store::V2BodyStore::open_with_policy(
                storage_root.join("bodies"),
                recovered_context.clone(),
                signature_policy,
            )
            .expect("reopen quarantined production marker-replay store"),
        );
        let lifecycle_root = storage_root
            .join("lifecycle-v1")
            .join(hex::encode(recovered_context.id().0.as_ref()));
        let result = authenticated.open_production_lifecycle_owner_v1(
            &lifecycle_owner_config(),
            4,
            factory_inputs,
            recovered_body_store,
        );
        if persist_matching_outcome {
            let owner = result.unwrap_or_else(|error| {
                panic!("matching recovered marker must enter production owner: {error}")
            });
            assert!(lifecycle_root.join("lifecycle-ledger-v1.norito").exists());

            let local_peer = PeerId::new(local_signer.public_key().clone());
            let local_validator = recovered_context
                .roster
                .iter()
                .position(|entry| entry.validator == local_peer)
                .and_then(|position| u32::try_from(position).ok())
                .expect("marker-replay signer belongs to the verified roster");
            let leader_wire_ingress = Arc::new(
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
            leader_wire_ingress
                .configure_roster_for_context(
                    recovered_context
                        .roster
                        .iter()
                        .map(|entry| entry.validator.clone()),
                    &recovered_context.network_id,
                    recovered_context.da_layout,
                )
                .expect("configure recovered-Apply lifecycle ingress");
            leader_wire_ingress.require_leader_wire_lifecycle_gate();
            let ingress_ready = Arc::new(AtomicBool::new(false));
            let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
            let launched_at = Instant::now();
            let kura_replica_advert_refresh = Arc::new(
                super::super::v2_worker::KuraReplicaAdvertRefreshOwner::from_kura(
                    kura.as_ref(),
                    launched_at,
                )
                .expect("bind recovered-Apply Kura advert source"),
            );
            let (exact_output_handoff_owner, transport_owner) =
                super::super::v2_worker::durable_exact_output_handoff_owner_pair();
            let mut lane_work =
                super::super::v2_lane_work::V2LaneWorkAdapter::lifecycle_finalization_fixture_for_test(
                    recovered_context.clone(),
                    local_peer.clone(),
                    local_signer.clone(),
                    Arc::clone(&state),
                    Arc::clone(&kura),
                    Arc::clone(&output_guard),
                    transport_owner,
                )
                .expect("open exact lifecycle lane/output owner");
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
                    1,
                    64,
                    Arc::clone(&output_guard),
                    Arc::clone(&leader_wire_ingress),
                    kura_replica_advert_refresh,
                    exact_output_handoff_owner,
                );
            use super::super::{
                v2_lifecycle_coordinator::{
                    ProductionLifecycleCompletionSelectionV1, ProductionLifecycleCompletionTurnV1,
                    ProductionLifecycleIngressTurnV1,
                    ProductionPreparedCertifiedServeTestSettlementV1,
                },
                v2_runner::{
                    LifecycleRunnerRankTarget, ProductionLifecyclePreActivationRunnerBorrowV1,
                    producer_turn_attempt_permit_for_test,
                    with_lifecycle_current_runner_turn_for_test,
                },
            };
            if shutdown_before_activation {
                let setup_context = super::super::v2_runner::lifecycle_run_inner::
                    launch_non_pending_lifecycle_height_and_shutdown_for_test(
                        owner,
                        launch_inputs,
                        None,
                        &ingress_ready,
                        &leader_wire_ingress,
                    )
                    .unwrap_or_else(|error| {
                        panic!("launch and stop unpublished lifecycle owner: {error}")
                    });
                assert_eq!(setup_context, recovered_context.id());
                assert!(!ingress_ready.load(Ordering::Acquire));
                assert!(!leader_wire_ingress.state.lock().open);
                assert!(!output_guard.restart_required());
                assert!(crate::sumeragi::status::v2_status().is_none());
                continue;
            }
            let mut launched = owner
                .launch(launch_inputs)
                .unwrap_or_else(|error| panic!("launch recovered-Apply lifecycle owner: {error}"));
            let mut setup_runner = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
            let setup_tag = launched
                .with_runner_setup(&mut setup_runner, |executor, services| {
                    assert_eq!(executor.context(), &recovered_context);
                    assert!(services.matches_lifecycle_executor_output_guard(executor));
                    services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
                    Ok::<_, super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1>(
                        executor.current_tag(),
                    )
                })
                .expect("closed-ingress runner setup borrows the launched stack");
            assert_eq!(setup_tag.height(), recovered_context.height);
            assert!(
                !leader_wire_ingress.state.lock().open,
                "runner setup must leave the jointly bound ingress closed"
            );
            let ((), after_ingress_pass_through) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| match launched.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        assert_eq!(runner.target(), LifecycleRunnerRankTarget::Ingress);
                        drop(runner);
                    }
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                        drop(turn);
                        panic!("empty lifecycle ingress cannot mint an ordinary handoff")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("empty lifecycle ingress must pass through")
                    }
                },
            );
            assert_eq!(
                after_ingress_pass_through,
                LifecycleRunnerRankTarget::Completion
            );
            let ((), after_wrong_class_pass_through) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Runtime,
                |runner| match launched.drive_completion_turn_for_test(runner, &mut lane_work) {
                    ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                        assert_eq!(runner.target(), LifecycleRunnerRankTarget::Runtime);
                        drop(runner);
                    }
                    ProductionLifecycleCompletionTurnV1::Selected(_) => {
                        panic!("wrong runner class must not select lifecycle work")
                    }
                },
            );
            assert_eq!(
                after_wrong_class_pass_through,
                LifecycleRunnerRankTarget::Ingress
            );
            assert!(!output_guard.restart_required());
            let mut foreign_context = recovered_context.clone();
            foreign_context.height = foreign_context
                .height
                .checked_add(1)
                .expect("fixture foreign height remains in range");
            let ((), after_foreign_context_pass_through) =
                with_lifecycle_current_runner_turn_for_test(
                    &foreign_context,
                    LifecycleRunnerRankTarget::Completion,
                    |runner| match launched.drive_completion_turn_for_test(runner, &mut lane_work) {
                        ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                            assert_eq!(runner.target(), LifecycleRunnerRankTarget::Completion);
                            drop(runner);
                        }
                        ProductionLifecycleCompletionTurnV1::Selected(_) => {
                            panic!("foreign runner context must not select lifecycle work")
                        }
                    },
                );
            assert_eq!(
                after_foreign_context_pass_through,
                LifecycleRunnerRankTarget::Runtime
            );
            assert!(!output_guard.restart_required());
            launched
                .with_runner_setup(&mut setup_runner, |executor, _services| {
                    let directive = executor.local_proposal_directive().map_err(
                        super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1::LocalProposalDirective,
                    )?;
                    executor
                        .acknowledge_runner_decision_cleanup(
                            directive.tag(),
                            directive.decided_subject(),
                        )
                        .map_err(
                            super::super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1::LocalProposalDirective,
                        )
                })
                .expect("mirror the runner's exact Decision cleanup before recovered Apply");
            let (queued, after_apply_selection) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Completion,
                |runner| match launched.drive_completion_turn_for_test(runner, &mut lane_work) {
                    ProductionLifecycleCompletionTurnV1::Selected(
                        ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(result),
                    ) => {
                        result.unwrap_or_else(|error| panic!("dispatch recovered Apply: {error:?}"))
                    }
                    ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("ready recovered Apply must select lifecycle work")
                    }
                    ProductionLifecycleCompletionTurnV1::Selected(_) => {
                        panic!("ready recovered Apply selected the wrong lifecycle class")
                    }
                },
            );
            assert_eq!(after_apply_selection, LifecycleRunnerRankTarget::Runtime);
            assert!(matches!(
                queued,
                super::super::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::ApplyQueued { .. }
            ));
            let completion_deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let (applied, after_completion_turn) = with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Completion,
                    |runner| {
                        match launched.drive_completion_turn_for_test(runner, &mut lane_work) {
                            ProductionLifecycleCompletionTurnV1::Selected(
                                ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied,
                            ) => true,
                            ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                                assert_eq!(runner.target(), LifecycleRunnerRankTarget::Completion);
                                drop(runner);
                                false
                            }
                            ProductionLifecycleCompletionTurnV1::Selected(
                                ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyCompletionDeferred,
                            ) => panic!("empty recovered block must not need a merge sidecar"),
                            ProductionLifecycleCompletionTurnV1::Selected(_) => {
                                panic!("recovered Apply completion selected the wrong lifecycle class")
                            }
                        }
                    },
                );
                assert_eq!(after_completion_turn, LifecycleRunnerRankTarget::Runtime);
                if applied {
                    break;
                }
                if Instant::now() >= completion_deadline {
                    panic!("timed out waiting for recovered Apply completion");
                }
                std::thread::yield_now();
            }
            let (prepared_directive, local_proposal_state) = launched
                .initialize_recovered_local_proposal(setup_runner)
                .expect("prepare the fresh runner local-Proposal state for activation");
            assert_eq!(prepared_directive.tag(), setup_tag);
            let activation = super::super::v2_runner::ProductionLifecycleRunnerActivationV1::current_height_for_test(
                Arc::clone(&ingress_ready),
                Arc::clone(&leader_wire_ingress),
            );
            let mut activated = launched
                .activate(Instant::now(), activation, local_proposal_state)
                .unwrap_or_else(|error| {
                    panic!("activate recovered-Apply lifecycle owner: {error}")
                });
            let ((), after_activated_completion_pass_through) =
                with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Completion,
                    |runner| match activated.drive_completion_turn_for_test(runner, &mut lane_work)
                    {
                        ProductionLifecycleCompletionTurnV1::PassThrough(runner) => drop(runner),
                        ProductionLifecycleCompletionTurnV1::Selected(_) => {
                            panic!("quiescent activated lifecycle Completion must pass through")
                        }
                    },
                );
            assert_eq!(
                after_activated_completion_pass_through,
                LifecycleRunnerRankTarget::Runtime
            );
            let mut terminal_serve_runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            assert!(
                activated
                    .ready_for_finalized_rollover(&mut terminal_serve_runner)
                    .expect("authenticate the terminal finalization census"),
                "the terminal fixture must be quiescent before capacity is withheld"
            );
            let terminal_ledger_before =
                std::fs::read(lifecycle_root.join("lifecycle-ledger-v1.norito"))
                    .expect("read terminal ledger before stale-claim CurrentServe repair");
            let (_unused_rejection, terminal_serve) =
                production_serve_requests_for_execution_commitment(
                    &recovered_context,
                    &keys,
                    local_validator,
                    round,
                    subject,
                    semantic_commitment,
                );
            assert!(matches!(
                leader_wire_ingress.try_push(
                    super::super::v2_worker::tests::certified_serve_inbound(
                        terminal_serve.request(),
                        local_peer.clone(),
                    ),
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let terminal_serve_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            let terminal_auxiliary_hold = activated
                .hold_auxiliary_io_admission_for_test()
                .expect("hold auxiliary capacity across the Apply barrier");
            // Fixture-only construction of the capacity wait reached when a terminal-ready
            // executor's process-local claim has returned to Eligible. From the sealed
            // authoritative repair permit onward this fixture exercises only the production
            // terminal handoff and direct CurrentServe path.
            let (terminal_capacity_pending, after_terminal_capacity) =
                with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Ingress,
                    |runner| {
                        matches!(
                            activated.drive_ingress_turn(runner),
                            ProductionLifecycleIngressTurnV1::Selected(
                                super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending,
                            )
                        )
                    },
                );
            assert!(terminal_capacity_pending);
            assert_eq!(
                after_terminal_capacity,
                LifecycleRunnerRankTarget::Completion
            );
            assert_eq!(leader_wire_ingress.len(), 1);
            assert_eq!(
                leader_wire_ingress.state.lock().last_admission_ordinal,
                terminal_serve_ordinal,
                "capacity handoff cannot dequeue or renumber the terminal Serve"
            );
            assert!(
                !activated
                    .ready_for_finalized_rollover(&mut terminal_serve_runner)
                    .expect("authenticate the capacity-blocked finalization census"),
                "the parked Serve capacity owner must block finalization"
            );
            let permit = activated.with_runner_runtime(
                &mut terminal_serve_runner,
                |_owner, executor, _services, _local_proposal| {
                    let directive = executor
                        .local_proposal_directive()
                        .expect("inspect terminal executor Decision");
                    super::super::v2_runner::LifecycleProducerClaimDispositionV1::initial()
                        .terminal_ready_decided_lane_recovery_permit(
                            executor.ready_to_finish(),
                            directive.decided_subject().is_some(),
                        )
                        .expect("terminal-ready Eligible executor authorizes direct recovery")
                },
            );
            assert!(
                activated
                    .reconcile_decided_lane_certified_serve(&mut terminal_serve_runner, permit)
                    .expect("handoff the same-service capacity wait to direct CurrentServe"),
                "the parked Serve capacity owner must make progress"
            );
            assert_eq!(leader_wire_ingress.len(), 1);
            assert_eq!(
                leader_wire_ingress.state.lock().last_admission_ordinal,
                terminal_serve_ordinal,
                "terminal handoff must retain the exact fair-ingress occurrence"
            );
            let mut terminal_block_sync_server =
                super::super::v2_block_sync::V2BlockSyncServer::new(
                    recovered_context.network_id.clone(),
                    4,
                )
                .expect("open terminal CurrentServe block-sync server");
            let terminal_serve_drained = activated.with_runner_runtime(
                &mut terminal_serve_runner,
                |_owner, executor, services, _local_proposal| {
                    super::super::v2_runner::lifecycle_run_inner::drain_decided_lane_recovery_ingress_for_test(
                        &leader_wire_ingress,
                        executor,
                        services,
                        &mut lane_work,
                        output_guard.as_ref(),
                        kura.as_ref(),
                        &local_signer,
                        &mut terminal_block_sync_server,
                    )
                },
            )
            .expect("direct CurrentServe consumes the handed-off request");
            assert!(terminal_serve_drained);
            assert_eq!(leader_wire_ingress.len(), 0);
            assert_eq!(
                std::fs::read(lifecycle_root.join("lifecycle-ledger-v1.norito"))
                    .expect("read terminal ledger after stale-claim CurrentServe repair"),
                terminal_ledger_before,
                "terminal-ready direct recovery cannot append fresh Serve or Producer rows"
            );
            drop(terminal_auxiliary_hold);
            let permit =
                super::super::v2_runner::LifecycleProducerClaimDispositionV1::ApplyTerminalSettled
                    .decided_lane_recovery_permit()
                    .expect("settled Apply authorizes the quiescent Serve census");
            assert!(
                !activated
                    .reconcile_decided_lane_certified_serve(&mut terminal_serve_runner, permit,)
                    .expect("the direct Serve leaves no hidden lifecycle owner"),
                "the quiescent Apply barrier cannot retain another Serve owner"
            );
            assert!(
                activated
                    .ready_for_finalized_rollover(&mut terminal_serve_runner)
                    .expect("authenticate the released finalization census"),
                "direct CurrentServe must release the last hidden finalization owner"
            );
            assert!(!output_guard.restart_required());

            let ordinary_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadManifest(manifest.clone()),
            ));
            assert!(matches!(
                leader_wire_ingress.try_push(
                    crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                        ordinary_message,
                        local_peer.clone(),
                    )
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let ordinary_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            let invalid_response = wire::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"unrelated malformed response family",
                )),
                manifest: manifest.clone(),
                body: canonical_wire.clone(),
                responder: 0,
                signature: Vec::new(),
            };
            let invalid_response_message = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(invalid_response),
            );
            assert!(matches!(
                leader_wire_ingress.try_push(
                    crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                        crate::sumeragi::message::BlockMessage::V2(invalid_response_message),
                        local_peer.clone(),
                    )
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let invalid_response_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            assert!(ordinary_ordinal < invalid_response_ordinal);
            let (ordinary_turn, after_ordinary_ingress) =
                with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Ingress,
                    |runner| match activated.drive_ingress_turn(runner) {
                        ProductionLifecycleIngressTurnV1::Ordinary(turn) => turn,
                        ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                            drop(runner);
                            panic!("an exact ordinary winner cannot return the unchanged cursor")
                        }
                        ProductionLifecycleIngressTurnV1::Selected(_) => {
                            panic!("an ordinary head cannot be poisoned by a later response family")
                        }
                    },
                );
            assert_eq!(
                after_ordinary_ingress,
                LifecycleRunnerRankTarget::Completion
            );
            assert_eq!(ordinary_turn.physical_ordinal_for_test(), ordinary_ordinal);
            assert!(!ordinary_turn.has_prepared_serve_for_test());
            assert_eq!(
                leader_wire_ingress.len(),
                1,
                "the unrelated malformed response remains behind the exact drained head"
            );
            let mut ordinary_runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            let mut block_sync_server = super::super::v2_block_sync::V2BlockSyncServer::new(
                recovered_context.network_id.clone(),
                4,
            )
            .expect("open ordinary-tail block-sync server");
            let mut block_sync = super::super::v2_block_sync::V2BlockSyncDiscovery::new(
                recovered_context.clone(),
                local_peer.clone(),
                4,
            )
            .expect("open ordinary-tail block-sync discovery");
            let mut block_sync_request = None;
            let mut npos_vrf = super::super::v2_npos::V2NposVrfLifecycle::open(
                &recovered_context,
                state.as_ref(),
                Some(local_validator),
                &local_signer,
            )
            .expect("open ordinary-tail NPoS lifecycle");
            let mut npos_beacon = super::super::v2_beacon::V2GlobalBeaconLifecycle::open(
                &recovered_context,
                state.as_ref(),
                Some(local_validator),
                None,
            )
            .expect("open ordinary-tail global beacon lifecycle");
            assert_eq!(
                activated
                    .consume_prepared_ordinary_ingress_turn(
                        &mut ordinary_runner,
                        ordinary_turn,
                        &mut lane_work,
                        kura.as_ref(),
                        &local_signer,
                        &mut block_sync_server,
                        &mut block_sync,
                        &mut block_sync_request,
                        &mut npos_vrf,
                        &mut npos_beacon,
                    )
                    .expect("consume the exact ordinary runner handoff"),
                super::super::v2_runner::ordinary_ingress_consumer::ProductionPreparedOrdinaryIngressConsumptionV1::Continue,
            );
            assert!(!output_guard.restart_required());
            let (invalid_turn, after_invalid_ingress) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => turn,
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("invalid-signature response is a drainable ordinary winner")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("invalid unrelated response cannot claim recovered Phase A")
                    }
                },
            );
            assert_eq!(after_invalid_ingress, LifecycleRunnerRankTarget::Completion);
            assert_eq!(
                invalid_turn.physical_ordinal_for_test(),
                invalid_response_ordinal
            );
            let mut invalid_runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            assert_eq!(
                activated
                    .consume_prepared_ordinary_ingress_turn(
                        &mut invalid_runner,
                        invalid_turn,
                        &mut lane_work,
                        kura.as_ref(),
                        &local_signer,
                        &mut block_sync_server,
                        &mut block_sync,
                        &mut block_sync_request,
                        &mut npos_vrf,
                        &mut npos_beacon,
                    )
                    .expect("consume the exact malformed-response ordinary handoff"),
                super::super::v2_runner::ordinary_ingress_consumer::ProductionPreparedOrdinaryIngressConsumptionV1::Continue,
            );
            assert_eq!(leader_wire_ingress.len(), 0);
            assert!(!output_guard.restart_required());
            let batch_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadManifest(manifest.clone()),
            ));
            assert!(matches!(
                leader_wire_ingress.try_push(
                    crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                        batch_message,
                        local_peer.clone(),
                    )
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let mut batch_runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            let producer_claim = super::super::v2_runner::drain_lifecycle_v2_ingress(
                &mut activated,
                &mut batch_runner,
                &leader_wire_ingress,
                &mut lane_work,
                kura.as_ref(),
                &local_signer,
                &mut block_sync_server,
                &mut block_sync,
                &mut block_sync_request,
                &mut npos_vrf,
                &mut npos_beacon,
                1,
                super::super::v2_runner::LifecycleProducerClaimDispositionV1::initial(),
            )
            .expect("drain one exact lifecycle-owned ordinary batch");
            assert!(!producer_claim.requires_yield());
            assert_eq!(leader_wire_ingress.len(), 0);
            assert!(!output_guard.restart_required());
            let (rejected_serve, admitted_serve) =
                production_serve_requests_for_execution_commitment(
                    &recovered_context,
                    &keys,
                    local_validator,
                    round,
                    subject,
                    semantic_commitment,
                );
            assert!(matches!(
                leader_wire_ingress.try_push(
                    super::super::v2_worker::tests::certified_serve_inbound(
                        rejected_serve.request(),
                        local_peer.clone(),
                    ),
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let rejected_serve_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            assert!(matches!(
                leader_wire_ingress.try_push(
                    super::super::v2_worker::tests::certified_serve_inbound(
                        admitted_serve.request(),
                        local_peer.clone(),
                    ),
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let admitted_serve_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            assert!(rejected_serve_ordinal < admitted_serve_ordinal);
            let auxiliary_hold = activated
                .hold_auxiliary_io_admission_for_test()
                .expect("hold the sole auxiliary I/O admission unit");
            let (rejected_turn, after_rejected_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => turn,
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("current certified Serve rejection must own ingress")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("retention-owner rejection must drain as ordinary")
                    }
                },
            );
            assert_eq!(after_rejected_serve, LifecycleRunnerRankTarget::Completion);
            assert_eq!(
                rejected_turn.physical_ordinal_for_test(),
                rejected_serve_ordinal
            );
            let mut serve_runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            let rejected_settlement = activated
                .settle_prepared_certified_serve_for_test(&mut serve_runner, rejected_turn)
                .expect("retire exact deterministic Serve negative");
            assert!(
                matches!(
                    &rejected_settlement,
                    ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)
                        if reason.contains("no certified retention authority")
                ),
                "unexpected rejected-Serve settlement: {rejected_settlement:?}"
            );
            assert_eq!(leader_wire_ingress.len(), 1);
            assert!(!output_guard.restart_required());

            let (retained, after_retained_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| {
                    match activated.drive_ingress_turn(runner) {
                        ProductionLifecycleIngressTurnV1::Selected(
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending,
                        ) => true,
                        ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                            drop(runner);
                            panic!("backpressured certified Serve remains lifecycle-owned")
                        }
                        ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                            drop(turn);
                            panic!("full auxiliary prefix cannot drain the prepared Serve")
                        }
                        ProductionLifecycleIngressTurnV1::Selected(_) => {
                            panic!("backpressured certified Serve selected the wrong outcome")
                        }
                    }
                },
            );
            assert!(retained);
            assert_eq!(after_retained_serve, LifecycleRunnerRankTarget::Completion);
            assert_eq!(leader_wire_ingress.len(), 1);
            assert_eq!(
                leader_wire_ingress.state.lock().last_admission_ordinal,
                admitted_serve_ordinal
            );
            assert!(!output_guard.restart_required());

            drop(auxiliary_hold);
            let ((), after_admitted_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| {
                    match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Selected(
                        super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeQueued,
                    ) => {}
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("released auxiliary capacity must admit exact Serve")
                    }
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                        drop(turn);
                        panic!("released certified Serve must enter lifecycle dispatch directly")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(selected) => {
                        let selected = match selected {
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending => "CertifiedServeCapacityPending",
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady => "CertifiedServeCompetingReady",
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeRetry => "CertifiedServeRetry",
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal => "CertifiedServeTerminal",
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued => "CertifiedServeReplayQueued",
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::RestartRequired => "RestartRequired",
                            _ => "non-Serve selection",
                        };
                        panic!("released certified Serve selected the wrong outcome: {selected}")
                    }
                }
                },
            );
            assert_eq!(after_admitted_serve, LifecycleRunnerRankTarget::Completion);
            assert_eq!(leader_wire_ingress.len(), 0);
            assert!(!output_guard.restart_required());
            let completion_deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let permit = super::super::v2_runner::LifecycleProducerClaimDispositionV1::ApplyTerminalSettled
                    .decided_lane_recovery_permit()
                    .expect("settled Apply authorizes class-specific Serve completion");
                let completed = activated
                    .reconcile_decided_lane_certified_serve(&mut serve_runner, permit)
                    .expect("settle only the in-flight Serve completion at the Apply barrier");
                if completed {
                    break;
                }
                assert!(
                    Instant::now() < completion_deadline,
                    "timed out waiting for current Serve completion"
                );
                std::thread::yield_now();
            }
            assert!(matches!(
                leader_wire_ingress.try_push(
                    super::super::v2_worker::tests::certified_serve_inbound(
                        admitted_serve.request(),
                        local_peer.clone(),
                    ),
                ),
                Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
            ));
            let competing_serve_ordinal = leader_wire_ingress.state.lock().last_admission_ordinal;
            let ((), after_competing_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| {
                    match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Selected(
                        super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady,
                    ) => {}
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("Ready Producer must retain the current certified Serve")
                    }
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                        drop(turn);
                        panic!("Ready Producer cannot let the current certified Serve drain")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("Ready Producer selected the wrong Serve outcome")
                    }
                }
                },
            );
            assert_eq!(after_competing_serve, LifecycleRunnerRankTarget::Completion);
            assert_eq!(leader_wire_ingress.len(), 1);
            assert_eq!(
                leader_wire_ingress.state.lock().last_admission_ordinal,
                competing_serve_ordinal,
                "the Ready-Producer guard cannot dequeue or renumber the retained Serve"
            );
            assert!(!output_guard.restart_required());
            let claimed_producer = activated
                .claim_producer_turn_for_local_proposal(&mut serve_runner)
                .expect("authenticate the complete Ready Producer census")
                .expect("completed Serve must release one adjacent ProducerTurn");
            let attempted_producer = claimed_producer
                .into_attempted(producer_turn_attempt_permit_for_test(&mut serve_runner));
            activated
                .settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)
                .expect("durably settle the attempted ProducerTurn");
            assert!(!output_guard.restart_required());

            let ((), after_terminal_replay_queue) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| {
                    match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Selected(
                        super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued,
                    ) => {}
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("released Producer must expose the retained Serve terminal replay")
                    }
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                        drop(turn);
                        panic!("authenticated terminal Serve replay remains lifecycle-owned")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("retained Serve replay selected the wrong outcome")
                    }
                }
                },
            );
            assert_eq!(
                after_terminal_replay_queue,
                LifecycleRunnerRankTarget::Completion
            );
            assert_eq!(leader_wire_ingress.len(), 0);
            let replay_deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let (completed, _) = with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Completion,
                    |runner| match activated.drive_completion_turn_for_test(runner, &mut lane_work)
                    {
                        ProductionLifecycleCompletionTurnV1::Selected(
                            ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted,
                        ) => true,
                        ProductionLifecycleCompletionTurnV1::PassThrough(_) => false,
                        ProductionLifecycleCompletionTurnV1::Selected(selected) => {
                            assert!(
                                !selected.restart_required(),
                                "terminal Serve replay completion requires lifecycle restart"
                            );
                            false
                        }
                    },
                );
                if completed {
                    break;
                }
                assert!(
                    Instant::now() < replay_deadline,
                    "timed out waiting for retained Serve terminal replay completion"
                );
                std::thread::yield_now();
            }
            assert!(!output_guard.restart_required());

            let ((), after_activated_ingress_pass_through) =
                with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Ingress,
                    |runner| match activated.drive_ingress_turn(runner) {
                        ProductionLifecycleIngressTurnV1::PassThrough(runner) => drop(runner),
                        ProductionLifecycleIngressTurnV1::Ordinary(turn) => {
                            drop(turn);
                            panic!("empty activated lifecycle cannot mint an ordinary handoff")
                        }
                        ProductionLifecycleIngressTurnV1::Selected(_) => {
                            panic!("empty activated lifecycle Ingress must pass through")
                        }
                    },
                );
            assert_eq!(
                after_activated_ingress_pass_through,
                LifecycleRunnerRankTarget::Completion
            );
            let mut runner =
                super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test();
            if shutdown_after_activation {
                activated
                    .into_clean_shutdown(&mut runner)
                    .unwrap_or_else(|error| panic!("cleanly stop active lifecycle owner: {error}"));
                assert!(!ingress_ready.load(Ordering::Acquire));
                assert!(!leader_wire_ingress.state.lock().open);
                assert!(!output_guard.restart_required());
                assert!(crate::sumeragi::status::v2_status().is_some());
                crate::sumeragi::status::clear_v2_status();
                continue;
            }
            let mut cleanup_supervisor = super::super::v2_worker::V2CleanupSupervisor::default();
            let ((), retained_sidecars, outcome) =
                super::super::v2_runner::lifecycle_run_inner::finalize_lifecycle_height(
                    activated,
                    &mut runner,
                    lane_work,
                    64,
                    &mut cleanup_supervisor,
                    |receipt, artifact, lane_work| {
                        assert_eq!(receipt.context_id(), recovered_context.id());
                        assert_eq!(receipt.block_hash(), subject.block_hash);
                        assert_eq!(artifact.subject, subject);
                        lane_work
                            .retain_merge_sidecars_for_global_view(
                                artifact.commit_qc.round.view,
                                None,
                                Some(artifact.subject),
                            )
                            .unwrap_or_else(|error| {
                                panic!("retain exact recovered-Apply Decision carrier: {error}")
                            });
                        let mut successor = recovered_context.clone();
                        successor.height = successor
                            .height
                            .checked_add(1)
                            .expect("fixture successor height remains in range");
                        successor.parent_commit_qc = Some(artifact.commit_qc.clone());
                        successor
                            .validate()
                            .expect("immediate recovered-Apply successor context is valid");
                        Ok::<_, super::super::v2_runner::V2RunnerError>((successor, ()))
                    },
                )
                .unwrap_or_else(|error| {
                    panic!("finalize recovered-Apply lifecycle owner: {error}")
                });
            drop(retained_sidecars);
            assert!(outcome.cleanup().warnings().is_empty());
            assert!(outcome.wal_retirement_warning().is_none());
            assert!(!ingress_ready.load(Ordering::Acquire));
            assert!(!leader_wire_ingress.state.lock().open);
            crate::sumeragi::status::clear_v2_status();
        } else {
            let error = match result {
                Ok(_owner) => panic!("a changed semantic marker outcome must fail closed"),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("validation outcome differs from semantic replay")
            );
            assert!(
                !lifecycle_root.exists(),
                "semantic marker mismatch must precede lifecycle-store creation"
            );
        }
    }
}

fn expect_recovered_open_error<'registry>(
    result: Result<
        PublishedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleOpenPublicationError<'registry>,
    >,
    message: &str,
) -> RecoveredWalLifecycleOpenPublicationError<'registry> {
    match result {
        Ok(_published) => panic!("{message}"),
        Err(error) => error,
    }
}

#[test]
fn recovered_prepare_wal_vote_fsyncs_repair_and_installs_exact_sign() {
    let directory = TempDir::new().expect("temporary Prepare recovery directory");
    let (startup, expected_vote, proposal, manifest, validated) =
        reopen_with_prepare_intent(&directory, 0xD1);
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => {
            panic!("authenticate the current recovered PrepareIntent: {error}")
        }
    };
    let authority = authenticated
        .recovered_phase_vote_for_test()
        .expect("PrepareIntent carries one restart vote");
    assert!(
        authenticated.effects.is_empty(),
        "the raw vote-sign effect is consumed"
    );
    assert!(authority.wal_identity().is_exact());
    assert!(authority.replay_evidence_is_exact());
    let wal_frame = authenticated
        .adapter
        .wal
        .recovered_records()
        .last()
        .expect("PrepareIntent WAL frame remains retained");
    assert!(authority.exactly_matches_wal_record(wal_frame));
    let mut foreign_hash = wal_frame.frame_hash();
    foreign_hash[0] ^= 1;
    assert!(
        !authority
            .wal_identity()
            .exactly_matches(RecoveredWalFrameIdentity::for_test(
                wal_frame.sequence(),
                wal_frame
                    .sequence()
                    .checked_add(1)
                    .expect("fixture sequence"),
                foreign_hash,
            ))
    );
    assert!(
        RecoveredWalFrameIdentity::for_test(0, 1, [0; 32]).is_exact(),
        "cryptographic hash bytes have no reserved sentinel value"
    );
    assert_eq!(authority.tag(), authenticated.adapter.current_tag());
    assert_eq!(authority.vote(), &expected_vote);
    assert_eq!(authority.vote().round, authority.vote().proposal_round);
    assert_eq!(authority.vote().phase, wire::GlobalPhase::Prepare);
    assert_eq!(
        authority.vote().execution_commitment,
        expected_vote.execution_commitment
    );
    assert!(authority.prepare_certificate().is_none());
    let verified = VerifiedHeightContext {
        context: authenticated.adapter.wire_context.clone(),
        proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
        parent_verification: authenticated.adapter.parent_verification.clone(),
    };
    let mut holder = super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let validate = holder.recovered_wal_validate_registry_cut_for_test(
        &verified, authority, proposal, manifest, validated,
    );
    let joined = authenticated
        .authenticate_recovered_validate(validate)
        .unwrap_or_else(|error| panic!("join recovered Prepare WAL vote: {}", error.reason()));
    assert!(joined.repair.concrete_pair_and_validation_are_exact());
    assert!(
        joined
            .repair
            .rejects_wrong_ledger_parent_bindings_for_test()
    );
    assert!(
        joined.repair.rejects_foreign_replay_authorities_for_test(),
        "structurally valid foreign replay origins must fail for both repaired rows"
    );

    let ledger_directory = TempDir::new().expect("temporary recovered Prepare ledger");
    let (summary, durable_startup) = joined
        .persist_repair_for_test(ledger_directory.path())
        .unwrap_or_else(|error| {
            panic!(
                "fsync recovered Prepare lifecycle repair: {}",
                error.reason()
            )
        });
    assert!(summary.first_changed());
    assert!(!summary.repeat_changed());
    assert!(summary.parent_advanced());
    assert!(summary.child_live());
    assert_eq!(summary.child_ordinal(), 2);
    assert!(summary.is_prepare_edge());
    assert!(!summary.is_commit_edge());
    assert_eq!(summary.high_water(), 2);
    assert!(summary.durable_frame_bound());
    assert!(summary.reopened_exact());
    assert!(
        durable_startup.remains_sealed_and_exact_for_test(ledger_directory.path()),
        "post-fsync startup must retain the adapter, empty unpublished batch, and vacant registry pair"
    );
    let installed = durable_startup
        .install_recovered_sign_for_test(ledger_directory.path())
        .unwrap_or_else(|error| {
            panic!(
                "install exact recovered Prepare Sign child: {}",
                error.reason()
            )
        });
    assert!(
        installed.exact_installed_shape_for_test(ledger_directory.path()),
        "the parent must stay absent while one same-owner child occupies the sole Effect slot at the durable ordinal and digest"
    );
    drop(installed);
    assert_eq!(
        holder.recovered_wal_sign_entry_count_for_test(),
        1,
        "dropping the exclusive installed cut releases only its borrow"
    );
}

#[test]
fn recovered_prepare_outer_fsync_rejects_a_stale_opened_ledger_snapshot() {
    let directory = TempDir::new().expect("temporary stale Prepare recovery directory");
    let (startup, _expected_vote, proposal, manifest, validated) =
        reopen_with_prepare_intent(&directory, 0xD3);
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => {
            panic!("authenticate stale recovered PrepareIntent: {error}")
        }
    };
    let authority = authenticated
        .recovered_phase_vote_for_test()
        .expect("PrepareIntent carries one stale-snapshot restart vote");
    let verified = VerifiedHeightContext {
        context: authenticated.adapter.wire_context.clone(),
        proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
        parent_verification: authenticated.adapter.parent_verification.clone(),
    };
    let mut holder = super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let validate = holder.recovered_wal_validate_registry_cut_for_test(
        &verified, authority, proposal, manifest, validated,
    );
    let joined = authenticated
        .authenticate_recovered_validate(validate)
        .unwrap_or_else(|error| {
            panic!("join stale recovered Prepare WAL vote: {}", error.reason())
        });
    let ledger_directory = TempDir::new().expect("temporary stale Prepare ledger");
    let error = match joined.persist_stale_snapshot_for_test(ledger_directory.path()) {
        Ok(_durable) => panic!("a stale opened ledger snapshot must not fsync"),
        Err(error) => error,
    };
    assert_eq!(
        error.reason(),
        "recovered WAL ledger fsync did not complete authoritatively"
    );
    drop(error);
}

#[test]
fn recovered_prepare_sign_install_rejects_wrong_store_before_registry_mutation() {
    let directory = TempDir::new().expect("temporary wrong-store Prepare recovery directory");
    let (startup, _expected_vote, proposal, manifest, validated) =
        reopen_with_prepare_intent(&directory, 0xD4);
    let mut holder = super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let joined =
        join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut holder);
    let ledger_directory = TempDir::new().expect("exact recovered Prepare ledger");
    let (_summary, durable) = joined
        .persist_repair_for_test(ledger_directory.path())
        .unwrap_or_else(|error| panic!("fsync recovered Prepare repair: {}", error.reason()));
    let wrong_ledger_directory = TempDir::new().expect("foreign recovered Prepare ledger root");
    let error = match durable.install_recovered_sign_for_test(wrong_ledger_directory.path()) {
        Ok(_installed) => panic!("a foreign store frame must not install recovered Sign work"),
        Err(error) => error,
    };
    assert_eq!(
        error.reason(),
        "fsynced recovered WAL Sign child failed exact registry preflight"
    );
    assert!(
        error.remains_sealed_with_exact_vacancies_for_test(ledger_directory.path()),
        "the opaque error must retain the adapter, empty batch, exact receipt, and both vacant registry addresses"
    );
    drop(error);
    assert_eq!(
        holder.recovered_wal_sign_entry_count_for_test(),
        0,
        "preflight failure must not insert a recovered Sign row"
    );
}

#[test]
fn recovered_prepare_restart_reenters_repaired_frame_and_installs_sign() {
    let directory = TempDir::new().expect("temporary re-entry Prepare recovery directory");
    let (startup, _expected_vote, proposal, manifest, validated) =
        reopen_with_prepare_intent(&directory, 0xD5);
    let replay_proposal = proposal.clone();
    let replay_manifest = manifest.clone();
    let replay_validated = validated.clone();
    let ledger_directory = TempDir::new().expect("re-entry recovered Prepare ledger");

    let mut first_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let first_joined =
        join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut first_holder);
    let (first_summary, durable_before_crash) = first_joined
        .persist_repair_for_test(ledger_directory.path())
        .unwrap_or_else(|error| panic!("fsync first recovered Prepare repair: {}", error.reason()));
    assert!(first_summary.first_changed());
    drop(durable_before_crash);
    assert_eq!(first_holder.recovered_wal_sign_entry_count_for_test(), 0);

    let restarted = open_recovered_startup_test(&directory)
        .expect("fresh startup replays the unchanged Prepare WAL frame");
    let mut restarted_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let restarted_joined = join_recovered_prepare_startup(
        restarted,
        replay_proposal,
        replay_manifest,
        replay_validated,
        &mut restarted_holder,
    );
    let (changed, durable) = restarted_joined
        .persist_reopened_repair_for_test(ledger_directory.path())
        .unwrap_or_else(|error| {
            panic!(
                "idempotently fsync reopened Prepare repair: {}",
                error.reason()
            )
        });
    assert!(
        !changed,
        "the exact Advanced-parent/live-child pair must stutter on fresh startup"
    );
    let installed = durable
        .install_recovered_sign_for_test(ledger_directory.path())
        .unwrap_or_else(|error| {
            panic!(
                "install recovered Prepare Sign after re-entry: {}",
                error.reason()
            )
        });
    assert!(installed.exact_installed_shape_for_test(ledger_directory.path()));
    drop(installed);
    assert_eq!(
        restarted_holder.recovered_wal_sign_entry_count_for_test(),
        1,
        "fresh startup leaves one exact closed Sign child after releasing the borrow"
    );
}

#[test]
fn recovered_owner_seal_cannot_relabel_the_authenticated_payload_store() {
    let safety = TempDir::new().expect("temporary owner-seal safety store");
    let ledger = TempDir::new().expect("temporary owner-seal ledger");
    let payload = TempDir::new().expect("temporary owner-seal payload store");
    let body = TempDir::new().expect("temporary owner-seal body store");
    let mut holder = super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let installed = install_recovered_prepare_startup(&safety, ledger.path(), 0xD0, &mut holder);
    let verified = verified_from_installed_startup(&installed);
    let body_store =
        super::super::v2_body_store::V2BodyStore::open(body.path(), verified.context().clone())
            .expect("open exact owner-seal body store");
    let (mut payload_store, recovered_payloads) =
        super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
            payload.path(),
            verified.context(),
        )
        .expect("open exact owner-seal payload store");
    let signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic empty-payload signer");
    let payloads = recovered_payloads
        .authenticate(&verified, &signer, &body_store)
        .expect("authenticate empty owner-seal payload recovery");
    let (_ledger_store, opened_ledger) =
        super::super::v2_lifecycle_coordinator::LifecycleLedgerStoreV1::open(
            ledger.path(),
            super::super::v2_lifecycle_coordinator::lifecycle_context(verified.context()),
        )
        .expect("open exact repaired owner-seal ledger");
    let mut recovery = AuthenticatedLifecycleRecoveryCut::empty_for_recovered_wal_test(
        &verified,
        opened_ledger,
        payloads,
    )
    .expect("assemble owner-seal recovery cut");
    assert!(
        installed
            .installed
            .seed_parent_recovery_for_test(&mut recovery)
    );
    let verified_context = verified.context().clone();
    let InstalledRecoveredWalLifecycleStartup {
        adapter,
        effects,
        installed,
    } = installed;
    let opened = installed
        .open_coordinator_for_test(&verified, ledger.path(), &mut payload_store, recovery)
        .unwrap_or_else(|error| panic!("open exact owner-seal coordinator: {}", error.reason()));
    let (adapter_startup, owner_seal) =
        ProductionOpenedRecoveredWalSignLifecycleCut::from_opened_for_test(
            opened,
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
            verified,
            &body_store,
            &payload_store,
        )
        .into_production_owner_open()
        .unwrap_or_else(|_opened| panic!("convert exact open into owner seal"));
    let (foreign_payload_store, _foreign_recovery) =
        super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
            payload.path(),
            &verified_context,
        )
        .expect("reopen the same payload path as a distinct instance");
    let paired = ProductionRecoveredLifecycleOwnerStartupV1 {
        adapter_startup,
        opened: owner_seal,
    };
    let error = match paired.into_owner(holder, foreign_payload_store, body_store) {
        Ok(_owner) => panic!("same-path foreign payload-store instance must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "authenticated lifecycle storage instances changed before startup"
    );
}

#[test]
fn recovered_prepare_opens_exact_coordinator_before_status_publication() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary published Prepare recovery directory");
    let ledger = TempDir::new().expect("temporary published Prepare ledger");
    let payload = TempDir::new().expect("temporary published payload store");
    let body = TempDir::new().expect("temporary published body store");
    let mut holder = super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let installed = install_recovered_prepare_startup(&safety, ledger.path(), 0xD6, &mut holder);
    let verified = verified_from_installed_startup(&installed);
    let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
        &verified,
        ledger.path(),
        payload.path(),
        body.path(),
    );
    assert!(
        installed
            .installed
            .seed_parent_recovery_for_test(&mut recovery)
    );
    crate::sumeragi::status::clear_v2_status();
    assert!(crate::sumeragi::status::v2_status().is_none());

    let published = installed
        .open_coordinator_and_publish_for_test(ledger.path(), &mut payload_store, recovery)
        .unwrap_or_else(|error| {
            panic!(
                "open exact recovered coordinator before status: {}",
                error.reason()
            )
        });
    assert!(published.exact_published_join_for_test());
    assert_eq!(
        crate::sumeragi::status::v2_status()
            .expect("status is published only after the exact join")
            .height,
        verified.context().height
    );
    drop(published);
    crate::sumeragi::status::clear_v2_status();
}

#[test]
fn recovered_prepare_open_failures_retain_authority_and_publish_no_status() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();

    // A same-context cut with no exact parent or child is rejected before
    // coordinator preparation.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("missing-recovery safety directory");
        let ledger = TempDir::new().expect("missing-recovery ledger");
        let payload = TempDir::new().expect("missing-recovery payload store");
        let body = TempDir::new().expect("missing-recovery body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xD7, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "missing recovered parent/child must fail closed",
        );
        assert_eq!(
            error.reason(),
            "authenticated recovery lacks the exact recovered WAL handoff"
        );
        assert!(error.retains_exact_installed_for_test(ledger.path()));
        assert!(crate::sumeragi::status::v2_status().is_none());
    }

    // A cut from another authenticated height context cannot be spliced.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("foreign-recovery safety directory");
        let ledger = TempDir::new().expect("foreign-recovery ledger");
        let payload = TempDir::new().expect("foreign-recovery payload store");
        let body = TempDir::new().expect("foreign-recovery body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xD8, &mut holder);
        let mut foreign_context = installed.adapter.wire_context.clone();
        foreign_context.leader_seed[0] ^= 0x5A;
        let foreign_verified = verified_genesis(foreign_context);
        let foreign_ledger = TempDir::new().expect("foreign-recovery authenticated ledger");
        let (mut payload_store, recovery) = empty_authenticated_lifecycle_recovery(
            &foreign_verified,
            foreign_ledger.path(),
            payload.path(),
            body.path(),
        );
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "foreign recovery context must fail closed",
        );
        assert_eq!(
            error.reason(),
            "authenticated recovery lacks the exact recovered WAL handoff"
        );
        assert!(error.retains_exact_installed_for_test(ledger.path()));
        assert!(crate::sumeragi::status::v2_status().is_none());
    }

    // Both exact sides are an ambiguous recovery shape and must be
    // preserved rather than normalized by overwriting either key.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("wrong-recovery safety directory");
        let ledger = TempDir::new().expect("wrong-recovery ledger");
        let payload = TempDir::new().expect("wrong-recovery payload store");
        let body = TempDir::new().expect("wrong-recovery body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xD9, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_both_recovery_for_test(&mut recovery)
        );
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "ambiguous exact parent/child recovery must fail closed",
        );
        assert_eq!(
            error.reason(),
            "authenticated recovery lacks the exact recovered WAL handoff"
        );
        assert!(error.retains_exact_installed_for_test(ledger.path()));
        assert!(crate::sumeragi::status::v2_status().is_none());
    }

    // A foreign ledger root fails during non-publishing preparation while
    // the exact receipt-bound installed row remains sealed.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("wrong-ledger safety directory");
        let ledger = TempDir::new().expect("wrong-ledger exact ledger");
        let wrong_ledger = TempDir::new().expect("wrong-ledger foreign root");
        let payload = TempDir::new().expect("wrong-ledger payload store");
        let body = TempDir::new().expect("wrong-ledger body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xDA, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_parent_recovery_for_test(&mut recovery)
        );
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                wrong_ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "foreign lifecycle ledger must fail before publication",
        );
        assert_eq!(
            error.reason(),
            "repaired lifecycle ledger could not prepare an exact coordinator open"
        );
        assert!(error.retains_exact_installed_for_test(ledger.path()));
        assert!(crate::sumeragi::status::v2_status().is_none());
    }

    // A corrupt opaque registry seal cannot mint the logical projection;
    // its closed row remains owned by the fail-stop error.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("wrong-registry safety directory");
        let ledger = TempDir::new().expect("wrong-registry ledger");
        let payload = TempDir::new().expect("wrong-registry payload store");
        let body = TempDir::new().expect("wrong-registry body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let mut installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xDB, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_parent_recovery_for_test(&mut recovery)
        );
        installed.installed.corrupt_registry_seal_for_test();
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "corrupt installed registry seal must fail closed",
        );
        assert_eq!(
            error.reason(),
            "installed recovered Sign registry seal is inconsistent"
        );
        assert!(error.retains_closed_registry_row_for_test());
        assert!(crate::sumeragi::status::v2_status().is_none());
    }

    // Even after the exact coordinator and both stores are committed, a
    // status construction error retains that whole opened authority.
    crate::sumeragi::status::clear_v2_status();
    {
        let safety = TempDir::new().expect("status-failure safety directory");
        let ledger = TempDir::new().expect("status-failure ledger");
        let payload = TempDir::new().expect("status-failure payload store");
        let body = TempDir::new().expect("status-failure body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let mut installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xDC, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_parent_recovery_for_test(&mut recovery)
        );
        installed.adapter.registry.validators.clear();
        crate::sumeragi::status::clear_v2_status();
        let error = expect_recovered_open_error(
            installed.open_coordinator_and_publish_for_test(
                ledger.path(),
                &mut payload_store,
                recovery,
            ),
            "invalid adapter status must fail after exact open",
        );
        assert_eq!(
            error.reason(),
            "adapter status publication failed after exact lifecycle open"
        );
        assert!(error.retains_exact_installed_for_test(ledger.path()));
        assert!(crate::sumeragi::status::v2_status().is_none());
    }
    crate::sumeragi::status::clear_v2_status();
}

include!("v2_adapter_04b_lifecycle_startup_tail.rs");
