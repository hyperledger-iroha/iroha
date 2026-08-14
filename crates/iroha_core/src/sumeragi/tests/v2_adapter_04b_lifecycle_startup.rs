use crate::sumeragi::message::BlockMessage;

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
            512 * 1024 * 1024,
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
    leader_wire_ingress.require_certified_serve_gate();
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

    let activated = launched
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
#[test]
fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies() {
    let handle = crate::sumeragi::sumeragi_thread_builder(
        "sumeragi-v2-production-lifecycle-marker-replay-test",
    )
    .spawn(production_lifecycle_factory_replays_markers_on_consensus_stack)
    .expect("spawn marker-replay test on the production consensus stack");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn production_lifecycle_factory_replays_markers_on_consensus_stack() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    for (marker, persist_matching_outcome) in [(0xB1_u8, true), (0xB2_u8, false)] {
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
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            format!("production recovered marker {marker:02x}"),
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
        if persist_matching_outcome {
            body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Ok::<_, String>(semantic_commitment)
                })
                .expect("persist matching semantic marker");
        } else {
            body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        "deliberately mismatched recovered rejection".to_owned(),
                    )
                })
                .expect("persist mismatched semantic marker");
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
                    512 * 1024 * 1024,
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
            leader_wire_ingress.require_certified_serve_gate();
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
            let mut launched = owner
                .launch(launch_inputs)
                .unwrap_or_else(|error| panic!("launch recovered-Apply lifecycle owner: {error}"));
            use super::super::{
                v2_lifecycle_coordinator::{
                    ProductionLifecycleCompletionSelectionV1, ProductionLifecycleCompletionTurnV1,
                    ProductionLifecycleIngressTurnV1,
                    ProductionPreparedCertifiedServeTestSettlementV1,
                },
                v2_runner::{
                    LifecycleRunnerRankTarget, ProductionLifecyclePreActivationRunnerBorrowV1,
                    with_lifecycle_current_runner_turn_for_test,
                },
            };

            let mut setup_runner = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
            let setup_tag = launched
                .with_runner_setup(&mut setup_runner, |executor, services| {
                    assert_eq!(executor.context(), &recovered_context);
                    assert!(services.matches_lifecycle_executor_output_guard(executor));
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
                |runner| match launched.drive_completion_turn(runner, &mut lane_work) {
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
                    |runner| match launched.drive_completion_turn(runner, &mut lane_work) {
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

            let (queued, after_apply_selection) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Completion,
                |runner| match launched.drive_completion_turn(runner, &mut lane_work) {
                    ProductionLifecycleCompletionTurnV1::Selected(
                        ProductionLifecycleCompletionSelectionV1::RecoveredIoDispatch(result),
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
                super::super::v2_lifecycle_coordinator::ProductionRecoveredCompletionDispatchV1::ApplyQueued { .. }
            ));
            let completion_deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let (applied, after_completion_turn) = with_lifecycle_current_runner_turn_for_test(
                    &recovered_context,
                    LifecycleRunnerRankTarget::Completion,
                    |runner| {
                        match launched.drive_completion_turn(runner, &mut lane_work) {
                            ProductionLifecycleCompletionTurnV1::Selected(
                                ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyApplied,
                            ) => true,
                            ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                                assert_eq!(runner.target(), LifecycleRunnerRankTarget::Completion);
                                drop(runner);
                                false
                            }
                            ProductionLifecycleCompletionTurnV1::Selected(
                                ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyCompletionDeferred,
                            ) => panic!("empty recovered block must not need a merge sidecar"),
                            ProductionLifecycleCompletionTurnV1::Selected(_) => {
                                panic!("recovered Apply completion selected an unexpected class")
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
                    |runner| match activated.drive_completion_turn(runner, &mut lane_work) {
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

            let ordinary_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
            ));
            assert!(matches!(
                leader_wire_ingress.try_push(crate::sumeragi::InboundBlockMessage::new(
                    ordinary_message,
                    Some(local_peer.clone()),
                )),
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
                leader_wire_ingress.try_push(crate::sumeragi::InboundBlockMessage::new(
                    BlockMessage::V2(invalid_response_message),
                    Some(local_peer.clone()),
                )),
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
                    )
                    .expect("consume the exact malformed-response ordinary handoff"),
                super::super::v2_runner::ordinary_ingress_consumer::ProductionPreparedOrdinaryIngressConsumptionV1::Continue,
            );
            assert_eq!(leader_wire_ingress.len(), 0);
            assert!(!output_guard.restart_required());

            let local_validator_index =
                usize::try_from(local_validator).expect("marker-replay validator index fits usize");
            let rejected_requester_index = (local_validator_index + 1) % keys.len();
            let admitted_requester_index = (local_validator_index + 2) % keys.len();
            let non_local_signers = (0..keys.len())
                .filter(|index| *index != local_validator_index)
                .collect::<Vec<_>>();
            let all_signers = (0..keys.len()).collect::<Vec<_>>();
            let (rejected_serve, _rejected_pops) =
                super::super::v2_worker::tests::production_authenticated_serve_request(
                    &recovered_context,
                    &keys,
                    &keys[rejected_requester_index],
                    round,
                    subject,
                    wire::GlobalPhase::Commit,
                    &non_local_signers,
                );
            let (admitted_serve, _admitted_pops) =
                super::super::v2_worker::tests::production_authenticated_serve_request(
                    &recovered_context,
                    &keys,
                    &keys[admitted_requester_index],
                    round,
                    subject,
                    wire::GlobalPhase::Commit,
                    &all_signers,
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
            assert!(matches!(
                rejected_settlement,
                ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)
                    if reason.contains("not a certified-body retention owner")
            ));
            assert_eq!(leader_wire_ingress.len(), 1);
            assert!(!output_guard.restart_required());

            let (retained, after_retained_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| {
                    match activated.drive_ingress_turn(runner) {
                        ProductionLifecycleIngressTurnV1::Selected(
                            super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::OrdinaryRetained,
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
            assert!(
                activated
                    .certified_serve_barrier_matches_for_test(admitted_serve.request_hash())
                    .expect("inspect the exact retained Serve barrier")
            );
            assert!(!output_guard.restart_required());

            drop(auxiliary_hold);
            let (admitted_turn, after_admitted_serve) = with_lifecycle_current_runner_turn_for_test(
                &recovered_context,
                LifecycleRunnerRankTarget::Ingress,
                |runner| match activated.drive_ingress_turn(runner) {
                    ProductionLifecycleIngressTurnV1::Ordinary(turn) => turn,
                    ProductionLifecycleIngressTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("released auxiliary capacity must admit exact Serve")
                    }
                    ProductionLifecycleIngressTurnV1::Selected(_) => {
                        panic!("released certified Serve selected the wrong outcome")
                    }
                },
            );
            assert_eq!(after_admitted_serve, LifecycleRunnerRankTarget::Completion);
            assert_eq!(
                admitted_turn.physical_ordinal_for_test(),
                admitted_serve_ordinal
            );
            assert!(admitted_turn.has_prepared_serve_for_test());
            assert_eq!(leader_wire_ingress.len(), 0);
            assert_eq!(
                activated
                    .settle_prepared_certified_serve_for_test(&mut serve_runner, admitted_turn,)
                    .expect("abort exact admitted Serve before worker commit"),
                ProductionPreparedCertifiedServeTestSettlementV1::AdmittedAborted
            );
            assert!(
                activated
                    .certified_serve_owners_are_clear_for_test()
                    .expect("inspect cleared Serve owners")
            );
            assert!(
                activated
                    .discharge_certified_serve_producer_episode_for_test()
                    .expect("finish the due local-producer episode")
            );
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
            let finalized = activated
                .into_finalized_rollover(&mut runner)
                .unwrap_or_else(|error| {
                    panic!("finalize recovered-Apply lifecycle owner: {error}")
                });
            let artifact = {
                let (receipt, artifact) = finalized.finality();
                assert_eq!(receipt.context_id(), recovered_context.id());
                assert_eq!(receipt.block_hash(), subject.block_hash);
                assert_eq!(artifact.subject, subject);
                artifact.clone()
            };
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
            let (post_output, retained_sidecars) = finalized
                .rollover_outputs(&mut runner, lane_work, &successor, 64)
                .unwrap_or_else(|error| panic!("roll over exact lifecycle output: {error}"));
            drop(retained_sidecars);
            let cleanup_ready = post_output
                .retire_lifecycle_stores()
                .unwrap_or_else(|error| panic!("retire recovered-Apply lifecycle stores: {error}"));
            let mut cleanup_supervisor = super::super::v2_worker::V2CleanupSupervisor::default();
            let outcome = cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor);
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
    let owner_seal = ProductionOpenedRecoveredWalSignLifecycleCut::from_opened_for_test(
        opened,
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
        adapter_startup: ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
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

#[test]
fn recovered_prepare_already_repaired_child_reopens_and_publishes() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("repaired-child safety directory");
    let ledger = TempDir::new().expect("repaired-child ledger");
    let payload = TempDir::new().expect("repaired-child payload store");
    let body = TempDir::new().expect("repaired-child body store");
    let (startup, _vote, proposal, manifest, validated) = reopen_with_prepare_intent(&safety, 0xDD);
    let replay_proposal = proposal.clone();
    let replay_manifest = manifest.clone();
    let replay_validated = validated.clone();

    let mut first_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let first =
        join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut first_holder);
    let (_summary, durable_before_crash) = first
        .persist_repair_for_test(ledger.path())
        .unwrap_or_else(|error| panic!("fsync the first repaired frame: {}", error.reason()));
    drop(durable_before_crash);

    let restarted = open_recovered_startup_test(&safety)
        .expect("fresh startup replays the unchanged repaired WAL frame");
    let mut restarted_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let restarted = join_recovered_prepare_startup(
        restarted,
        replay_proposal,
        replay_manifest,
        replay_validated,
        &mut restarted_holder,
    );
    let (changed, durable) = restarted
        .persist_reopened_repair_for_test(ledger.path())
        .unwrap_or_else(|error| {
            panic!(
                "stutter on the already repaired ledger frame: {}",
                error.reason()
            )
        });
    assert!(!changed);
    let installed = durable
        .install_recovered_sign_for_test(ledger.path())
        .unwrap_or_else(|error| {
            panic!("install the repaired-frame Sign child: {}", error.reason())
        });
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
            .seed_child_recovery_for_test(&mut recovery)
    );
    crate::sumeragi::status::clear_v2_status();
    let published = installed
        .open_coordinator_and_publish_for_test(ledger.path(), &mut payload_store, recovery)
        .unwrap_or_else(|error| {
            panic!(
                "already-repaired child must reopen idempotently: {}",
                error.reason()
            )
        });
    assert!(published.exact_published_join_for_test());
    assert!(crate::sumeragi::status::v2_status().is_some());
    drop(published);
    crate::sumeragi::status::clear_v2_status();
}

#[cfg(feature = "bls")]
#[test]
fn recovered_commit_vote_sign_retains_the_exact_authenticated_prepare_qc() {
    let directory = TempDir::new().expect("temporary Commit recovery directory");
    let (mut adapter, startup) = open_test(&directory).expect("open Commit replay fixture");
    assert!(startup.is_empty());
    let locked_subject = subject(0xD2);
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let (_, keys, _) = authenticated_context();
    let mut wire_prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xD2),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut wire_prepare, &keys);
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the authenticated PrepareQC");
    let core_context = adapter.reducer.context().id();
    let core_round = reducer::Round::new(round.height, round.view);
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("fixture local validator");
    let entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new_with_proposal_round(
                core_context,
                core_round,
                core_round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&entry, &TestAggregator)
        .expect("encode the exact LockAndCommit frame");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append lock frame")
            .sequence(),
        0
    );
    drop(adapter);

    let startup = open_recovered_startup_test(&directory)
        .expect("replay authenticated LockAndCommit behind the sealed startup cut");
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => {
            panic!("authenticate the current recovered LockAndCommit: {error}")
        }
    };
    let authority = authenticated
        .recovered_phase_vote_for_test()
        .expect("LockAndCommit carries one restart vote");
    assert!(authenticated.effects.is_empty());
    assert!(authority.wal_identity().is_exact());
    assert!(authority.replay_evidence_is_exact());
    assert!(
        authority.exactly_matches_wal_record(
            authenticated
                .adapter
                .wal
                .recovered_records()
                .last()
                .expect("LockAndCommit WAL frame remains retained")
        )
    );
    assert_eq!(authority.vote().phase, wire::GlobalPhase::Commit);
    assert_eq!(authority.vote().round, round);
    assert_eq!(authority.vote().proposal_round, round);
    assert_eq!(authority.vote().subject, locked_subject);
    let retained_prepare = authority
        .prepare_certificate()
        .expect("Commit recovery retains the exact PrepareQC");
    assert_eq!(retained_prepare, &wire_prepare);
    assert_eq!(
        retained_prepare.execution_commitment,
        authority.vote().execution_commitment
    );
    drop(authenticated);
}

#[test]
fn recovered_vote_sign_startup_cut_is_one_shot_and_drop_inert() {
    let directory = TempDir::new().expect("temporary recovery seal directory");
    let (startup, _expected_vote, _proposal, _manifest, _validated) =
        reopen_with_prepare_intent(&directory, 0xD3);
    let wal_path = directory.path().join("safety.wal");
    let durable_before = std::fs::read(&wal_path).expect("read sealed WAL before drop");
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => panic!("authenticate exact replay vote: {error}"),
    };
    assert!(authenticated.recovered_phase_vote_for_test().is_some());
    assert!(
        authenticated.effects.iter().all(|effect| !matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            }
        )),
        "the retained batch no longer contains the sealed vote"
    );
    assert!(
        authenticated.finish_without_wal_vote().is_err(),
        "a phase-vote startup cannot escape through the no-vote path"
    );
    assert_eq!(
        std::fs::read(&wal_path).expect("read sealed WAL after drop"),
        durable_before,
        "dropping the sealed startup cannot rewrite its WAL"
    );

    let repeated = open_recovered_startup_test(&directory)
        .expect("the unchanged WAL can be authenticated by a new sealed startup instance");
    let repeated = repeated
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("reauthenticate unchanged WAL: {error}"));
    assert!(repeated.recovered_phase_vote_for_test().is_some());
    assert!(repeated.effects.is_empty());
    drop(repeated);
}

#[test]
fn recovered_startup_seals_authenticated_control_wal_records() {
    let directory = TempDir::new().expect("temporary non-vote recovery directory");
    let (mut adapter, startup) = open_test(&directory).expect("open timeout replay fixture");
    assert!(startup.is_empty());
    let timeout = adapter
        .timeout_elapsed(adapter.current_tag())
        .expect("persist the exact TimeoutIntent")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    drop(adapter);

    let startup = open_recovered_startup_test(&directory)
        .expect("replay the durable TimeoutIntent behind the sealed startup cut");
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate TimeoutIntent: {error}"));
    assert!(authenticated.has_recovered_control_sign_for_test());
    assert!(authenticated.effects.is_empty());
    assert!(
        authenticated.finish_without_wal_vote().is_err(),
        "a control Sign cannot escape through the no-authority path"
    );
}
