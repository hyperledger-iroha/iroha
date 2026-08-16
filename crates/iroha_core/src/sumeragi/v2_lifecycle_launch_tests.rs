// Production lifecycle launch, activation, shutdown, and source-seal tests.
use iroha_crypto::{Hash, HashOf};
use tempfile::TempDir;

use super::*;
use crate::sumeragi::v2_lifecycle_coordinator::reviewed_lifecycle_ledger_source_for_test;

#[test]
fn preactivation_fail_stop_scope_closes_on_drop_and_disarms_on_complete() {
    let dropped_guard = ConsensusOutputGuard::isolated();
    drop(ProductionLifecyclePreActivationFailStopScopeV1::new(
        Arc::clone(&dropped_guard),
    ));
    assert!(dropped_guard.restart_required());

    let completed_guard = ConsensusOutputGuard::isolated();
    ProductionLifecyclePreActivationFailStopScopeV1::new(Arc::clone(&completed_guard)).complete();
    assert!(!completed_guard.restart_required());
}

#[test]
fn activation_recovery_blocker_requires_pending_and_proposal_preactivation() {
    assert!(matches!(
        lifecycle_activation_recovery_blocker(true, false, false),
        Some(ProductionLifecycleActivationErrorV1::PendingKuraApply)
    ));
    assert!(matches!(
        lifecycle_activation_recovery_blocker(false, true, true),
        Some(ProductionLifecycleActivationErrorV1::PendingKuraApply)
    ));
    assert!(matches!(
        lifecycle_activation_recovery_blocker(false, false, true),
        Some(ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized)
    ));
    assert!(lifecycle_activation_recovery_blocker(false, false, false).is_none());
}

#[test]
fn prepared_local_proposal_state_is_affine_and_context_directive_bound() {
    let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"prepared local Proposal lifecycle context",
    )));
    let tag =
        crate::sumeragi::v2_core::EventTag::new(1, 2, crate::sumeragi::v2_core::Generation::new(3));
    let directive = super::super::v2::LocalProposalDirective::for_test(tag, 0, None, None, None);
    let runner =
        super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    let prepared = ProductionLifecyclePreparedLocalProposalStateV1 {
        runner,
        context_id,
        directive,
    };
    assert!(prepared.exactly_matches(context_id, directive));

    let foreign_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign prepared local Proposal lifecycle context",
    )));
    assert!(!prepared.exactly_matches(foreign_context, directive));
    let foreign_directive = super::super::v2::LocalProposalDirective::for_test(
        crate::sumeragi::v2_core::EventTag::new(1, 4, crate::sumeragi::v2_core::Generation::new(5)),
        0,
        None,
        None,
        None,
    );
    assert!(!prepared.exactly_matches(context_id, foreign_directive));
}

#[test]
fn launch_local_identity_requires_the_bound_key_and_exact_roster_position() {
    let key_pair = KeyPair::random();
    let local_peer = PeerId::new(key_pair.public_key().clone());
    let roster = vec![wire::ValidatorPower {
        validator: local_peer.clone(),
        power: 1,
    }];
    assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
        &roster,
        &local_peer,
        Some(0),
        &key_pair,
    ));
    assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
        &roster,
        &local_peer,
        None,
        &key_pair,
    ));
    assert!(!ProductionLifecycleOwnerV1::launch_local_identity_matches(
        &roster,
        &local_peer,
        Some(1),
        &key_pair,
    ));
    let foreign_key = KeyPair::random();
    assert!(!ProductionLifecycleOwnerV1::launch_local_identity_matches(
        &roster,
        &local_peer,
        Some(0),
        &foreign_key,
    ));
    let observer_key = KeyPair::random();
    let observer_peer = PeerId::new(observer_key.public_key().clone());
    assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
        &roster,
        &observer_peer,
        None,
        &observer_key,
    ));
}

fn empty_leader_wire_gate_for_binding_test(
    directory: &TempDir,
    filename: &str,
    context_id: wire::HeightContextId,
    height: wire::Height,
    validator: &PeerId,
) -> (
    Arc<LeaderWireLifecycleStoreGate>,
    LeaderWireLifecycleRestore,
) {
    let owner = [0xA7; 32];
    let max_chunk_count = 2;
    let capacity = LeaderWireLifecycleStoreGate::derived_capacity(1, max_chunk_count)
        .expect("finite leader-wire binding fixture capacity");
    let recovery_authority =
            crate::sumeragi::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context_id,
                height,
                owner,
                0,
                false,
            );
    LeaderWireLifecycleStoreGate::open(
        &directory.path().join(filename),
        context_id,
        height,
        owner,
        [validator.clone()].into_iter().collect(),
        capacity,
        max_chunk_count,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open empty leader-wire binding fixture")
}

#[test]
fn production_leader_wire_binding_retires_explicitly_on_drop_and_closes_on_failure() {
    const HEIGHT: wire::Height = 7;
    let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"production-leader-wire-launch-binding",
    )));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let directory = TempDir::new().expect("temporary launch binding directory");
    let ingress = Arc::new(FairV2Ingress::new(16, 1 << 20, 1 << 18, 0, 0));
    ingress
        .configure_roster([validator.clone()])
        .expect("one-validator launch binding geometry");
    ingress.require_certified_serve_gate();
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;

    let (first_serve_gate, first_ordinals) =
        crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
    let (first_gate, first_restore) = empty_leader_wire_gate_for_binding_test(
        &directory,
        "explicit.wal",
        context_id,
        HEIGHT,
        &validator,
    );
    let mut binding = ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        Arc::clone(&first_gate),
        first_restore,
        first_ordinals,
        context_id,
        HEIGHT,
    )
    .expect("bind the exact launch gate")
    .bind_certified_serve(first_serve_gate.clone())
    .expect("join the exact certified Serve gate");
    assert!(
        ingress
            .state
            .lock()
            .leader_wire_lifecycle_gate
            .as_ref()
            .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &first_gate))
    );
    assert!(
        ingress
            .state
            .lock()
            .certified_serve_gate
            .as_ref()
            .is_some_and(|bound| bound.ptr_eq(&first_serve_gate))
    );
    binding
        .retire()
        .expect("explicit retirement detaches both exact launch gates");
    binding
        .retire()
        .expect("explicit retirement remains idempotent");
    {
        let state = ingress.state.lock();
        assert!(state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none());
    }

    let (drop_serve_gate, drop_ordinals) =
        crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
    let (drop_gate, drop_restore) = empty_leader_wire_gate_for_binding_test(
        &directory, "drop.wal", context_id, HEIGHT, &validator,
    );
    let binding = ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        Arc::clone(&drop_gate),
        drop_restore,
        drop_ordinals,
        context_id,
        HEIGHT,
    )
    .expect("rebind the exact launch gate")
    .bind_certified_serve(drop_serve_gate)
    .expect("rejoin the certified Serve gate");
    drop(binding);
    {
        let state = ingress.state.lock();
        assert!(
            state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none(),
            "Drop must detach both exact launch gates"
        );
    }

    let (mismatched_serve_gate, _) =
        crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
    let (mismatch_gate, mismatch_restore) = empty_leader_wire_gate_for_binding_test(
        &directory,
        "mismatch.wal",
        context_id,
        HEIGHT,
        &validator,
    );
    let mismatch = match ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        mismatch_gate,
        mismatch_restore,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        context_id,
        HEIGHT,
    )
    .expect("bind the leader gate before the mismatched Serve join")
    .bind_certified_serve(mismatched_serve_gate)
    {
        Ok(_) => panic!("a foreign lifecycle ordinal source passed the joint join"),
        Err(error) => error,
    };
    assert!(mismatch.contains("actor-global lifecycle ordinal source"));
    {
        let state = ingress.state.lock();
        assert!(
            state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none(),
            "a failed joint join must drop the retained leader binding"
        );
    }

    let (incumbent_gate, incumbent_restore) = empty_leader_wire_gate_for_binding_test(
        &directory,
        "incumbent.wal",
        context_id,
        HEIGHT,
        &validator,
    );
    let (incumbent_serve_gate, incumbent_ordinals) =
        crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&incumbent_gate),
            incumbent_restore,
            incumbent_ordinals,
            context_id,
            HEIGHT,
        )
        .expect("bind the incumbent gate");
    ingress
        .bind_certified_serve_gate(incumbent_serve_gate.clone())
        .expect("bind the incumbent certified Serve gate");
    ingress.open().expect("open the incumbent ingress");
    let (foreign_gate, foreign_restore) = empty_leader_wire_gate_for_binding_test(
        &directory,
        "foreign.wal",
        context_id,
        HEIGHT,
        &validator,
    );
    let error = match ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        foreign_gate,
        foreign_restore,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        context_id,
        HEIGHT,
    ) {
        Ok(_) => panic!("an open, already-bound ingress accepted a foreign launch gate"),
        Err(error) => error,
    };
    assert!(error.contains("empty closed ingress"));
    assert!(
        !ingress.state.lock().open,
        "failed binding must close ingress"
    );
    ingress
        .unbind_height_ingress_gates(&incumbent_serve_gate, &incumbent_gate)
        .expect("clean up both incumbent bindings");
}

#[test]
fn launch_source_keeps_status_sealed_and_orders_store_transfer() {
    let source = concat!(
        include_str!("v2_lifecycle_launch.rs"),
        include_str!("v2_lifecycle_preactivation.rs")
    );
    let adapter_source = [
        crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_adapter_source_for_test(),
        include_str!("v2_pending_kura_recovery.rs"),
    ]
    .concat();
    let safety_wal_source = include_str!("safety_wal.rs");
    let kura_source = concat!(
        include_str!("../kura.rs"),
        include_str!("../kura/bound_progress_and_retained_support.rs")
    );
    let adjacent_store_source = include_str!("serviced_candidate_store.rs");
    let worker_source = include_str!("v2_worker.rs");
    let effects_source = include_str!("v2_effects.rs");
    let runtime_source =
        crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_runtime_source_for_test();
    let runner_source = include_str!("v2_runner.rs");
    let lifecycle_run_inner_source = include_str!("v2_runner/lifecycle_run_inner.rs");
    let runner_authority_source = concat!(
        include_str!("v2_runner/lifecycle_runner_authority.rs"),
        include_str!("v2_runner.rs")
    );
    let finalized_output_source = include_str!("v2_runner/finalized_output_rollover.rs");
    let runner_tests_source = include_str!("v2_runner_tests.rs");
    let coordinator_source = include_str!("v2_lifecycle_coordinator.rs");
    let ledger_source = reviewed_lifecycle_ledger_source_for_test();
    let payload_store_source = include_str!("v2_certified_serve_payload_store.rs");
    let lifecycle_open_source = include_str!("v2_lifecycle_open.rs");
    let registry_validate_source = concat!(
        include_str!("v2_lifecycle_work_registry_validate_recovery.rs"),
        include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs")
    );
    let lifecycle_startup_test_source = include_str!("tests/v2_adapter_04b_lifecycle_startup.rs");
    let bound_launch = ledger_source
        .split_once("// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_BEGIN")
        .expect("the bound CompleteTip launch has one sealed source region")
        .1
        .split_once("// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_END")
        .expect("the bound CompleteTip launch region has one end")
        .0;

    let bind = bound_launch
        .find("impl BoundRecoveredCompleteTipSuccessorOwnerV1")
        .expect("the bound H+1 owner has one launch implementation");
    let launch = bound_launch
        .find("pub(in crate::sumeragi) fn launch(")
        .expect("the bound H+1 owner exposes one consuming launch");
    let consume = bound_launch
        .find("let Self { owner, retirement } = self;")
        .expect("launch consumes both halves of the exact join");
    let generic_launch = bound_launch
        .find("let launched = owner.launch(inputs)?;")
        .expect("the bound owner enters the sole generic launch transaction");
    let retained = bound_launch
            .find("LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n            launched,\n            retirement,")
            .expect("the successful launch retains its retirement authority");
    let wrapper = bound_launch
        .find("struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1")
        .expect("the typed post-launch wrapper stays opaque");
    assert!(bound_launch.contains(
            "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n    launched: Box<super::launch::LaunchedProductionLifecycleV1>,\n    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,\n}"
        ));
    let activation_impl = bound_launch
        .find("impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1")
        .expect("the launched H+1 join has one consuming activation implementation");
    let closed_setup = bound_launch
        .find("self.launched.with_runner_setup(runner, operation)")
        .expect("the sealed H+1 join lends only closed-ingress runner setup");
    let activation_consume = bound_launch
        .find("let Self {\n            launched,\n            retirement,\n        } = self;")
        .expect("CompleteTip activation consumes the still-joined launched owner and retirement");
    let typed_activation = bound_launch
        .find("launched.activate_recovered_complete_tip(now, runner, retirement, local_proposal)")
        .expect("CompleteTip activation enters only the typed publication boundary");
    assert!(
        bind < launch
            && launch < consume
            && consume < generic_launch
            && generic_launch < retained
            && retained < wrapper
            && wrapper < activation_impl
            && activation_impl < closed_setup
            && closed_setup < activation_consume
            && activation_consume < typed_activation
    );
    for forbidden in [
        "set_v2_status",
        "publish_status(",
        "successor_activation_status",
        "activate_effect_completion_observer",
        "into_owner",
        "into_parts",
        "fn owner(",
        "fn retirement(",
        "fn launched(",
        "fn into_launched(",
        "fn into_retirement(",
        "-> ProductionLifecycleOwnerV1",
        "-> super::launch::LaunchedProductionLifecycleV1",
        "-> RetiredRecoveredCompleteTipActivationAuthorityV1",
        "pub launched:",
        "pub retirement:",
        "pub(crate) launched:",
        "pub(crate) retirement:",
        "pub(in crate::sumeragi) launched:",
        "pub(in crate::sumeragi) retirement:",
    ] {
        assert!(
            !bound_launch.contains(forbidden),
            "bound CompleteTip launch exposes forbidden surface {forbidden}"
        );
    }
    assert_eq!(bound_launch.matches("owner.launch(inputs)?").count(), 1);

    assert!(source.contains("authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,"));
    assert_eq!(
        source
            .matches("authenticated_genesis: Option<AuthenticatedGenesisBodyV1>")
            .count(),
        2,
        "the move-only genesis seal must occur only in the launch input field and constructor"
    );
    assert!(!source.contains("authenticated_genesis: Option<SignedBlock>"));
    let raw_genesis_account_input = ["genesis_account", ": AccountId"].concat();
    assert!(
        !source.contains(&raw_genesis_account_input),
        "launch inputs must not accept a caller-selected genesis validation authority"
    );
    let inputs = source
        .split_once("pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {")
        .expect("launch inputs have one declaration")
        .1
        .split_once("\n}")
        .expect("launch input declaration is closed")
        .0;
    for forbidden in [
        "chunk_root: PathBuf",
        "wal_path: PathBuf",
        "lifecycle_ordinals: RuntimeLifecycleOrdinalSource",
        "durable_bodies:",
        "recovered_body_receipts:",
        "queue: Arc<Queue>",
        "provider_ingest_finalized_archive:",
        "reputation_finalized_archive:",
        "block_cadence: Duration",
        "events_sender: EventsSender",
    ] {
        assert!(
            !inputs.contains(forbidden),
            "launch inputs expose caller-selected durable authority {forbidden}"
        );
    }

    let launch = source
        .split_once("pub(in crate::sumeragi) fn launch(")
        .expect("the owner has one consuming launch")
        .1
        .split_once("\n}\n\n#[cfg(test)]")
        .expect("the consuming launch ends before its source guards")
        .0;
    assert!(!source.contains("publish_status("));
    assert!(!launch.contains("set_v2_effect_completion_observer"));
    let arm = launch.find("begin_fail_stop_operation()").unwrap();
    let owner_check = launch.find("if self.body_store.is_none()").unwrap();
    let local_identity = launch
        .find("Self::launch_local_identity_matches(")
        .expect("launch checks local peer, validator index, and bound signer before I/O");
    let kura_check = launch
        .find("binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)")
        .expect("launch rejoins the owner with the exact recovery Kura and local signer");
    let apply_identity = launch
        .find("service.matches_lifecycle_launch(")
        .expect("launch verifies the retained replay service before taking owner cuts");
    let registry_check = launch
        .find("exactly_covers_recovered_ready_work(&self.coordinator)")
        .unwrap();
    let storage_paths = launch
        .find("binding.storage_paths_for_launch(inputs.kura.as_ref())")
        .expect("launch derives paths from the exact recovery-owned Kura seal");
    let body_receipts = launch
        .find("self.body_store\n                        .as_ref()")
        .expect("launch derives exact receipts from its owner-held body store");
    let adapter_wal = launch
        .find(".prepare_leader_wire_launch(launch_storage.wal_path())")
        .expect("launch rejoins adapter authority to the recovery-sealed WAL");
    let restore_ordinals = launch
        .find("ProductionV2Services::restore_lifecycle_ordinal_source(")
        .expect("launch restores its sole lifecycle ordinal source internally");
    assert!(launch.contains(
            "inputs.network.reply_route_source_capacity().max(1),\n            inputs.auxiliary_io_capacity,"
        ));
    let producer_high_water = launch
        .find("leader_wire_launch.restored_producer_ordinal_high_watermark()")
        .expect("launch folds the adapter producer high-watermark");
    let open_gate = launch
        .find(".open_gate(")
        .expect("launch opens the gate with exact owner-store receipts");
    let gate_high_water = launch
        .find("leader_wire_restore.scheduler_ordinal_high_watermark()")
        .expect("launch folds the restored leader-wire high-watermark");
    let bind_gate = launch
        .find("ProductionLeaderWireIngressBindingV1::bind(")
        .expect("launch binds the exact gate before runtime construction");
    let take = launch.find(".body_store\n            .take()").unwrap();
    let take_apply = launch
        .find(".apply_service\n            .take()")
        .expect("launch consumes the exact marker-replay service once");
    let runtime = launch
        .find(".into_serialized_runtime(")
        .expect("launch consumes the adapter into the serialized runtime");
    let executor = launch
        .find("V2EffectExecutor::open_with_body_store(")
        .unwrap();
    let genesis_gate = launch
        .find("if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()")
        .expect("fresh-genesis installation stays behind the owned optional seal");
    let genesis = launch
            .find(
                "executor\n                .install_authenticated_genesis_body(authenticated_genesis.signed_block())",
            )
            .expect("authenticated genesis enters the executor before worker start");
    let worker = launch
        .find("ProductionV2Services::start_with_apply_service(")
        .expect("launch transfers the exact marker-replay service to the worker");
    let certified_serve_gate = launch
        .find("services\n            .certified_serve_ingress_gate()")
        .expect("launch obtains the exact service-owned Serve gate");
    let joint_ingress_bind = launch
        .find(".bind_certified_serve(certified_serve_gate)")
        .expect("launch joins both durable ingress gates before success");
    let worker_permit = launch
        .find("super::ProductionLifecycleApplyServiceLaunchPermitV1 {")
        .expect("launch mints the sole private Apply-service transfer permit");
    assert_eq!(
        launch.matches("inputs.auxiliary_io_capacity,").count(),
        2,
        "Serve restore and service startup must share the exact certified-request capacity"
    );
    let identity = launch
        .rfind("self.body_store_identity = Some(body_store_identity)")
        .unwrap();
    let complete = launch.rfind("construction.complete()").unwrap();
    assert!(
        arm < owner_check
            && owner_check < local_identity
            && local_identity < kura_check
            && kura_check < apply_identity
            && apply_identity < registry_check
    );
    assert!(
        apply_identity < storage_paths
            && storage_paths < adapter_wal
            && adapter_wal < restore_ordinals
            && restore_ordinals < producer_high_water
            && producer_high_water < open_gate
            && open_gate < body_receipts
            && body_receipts < gate_high_water
            && open_gate < gate_high_water
            && gate_high_water < bind_gate
            && bind_gate < take
            && take <= take_apply
            && take < runtime
    );
    let gate_open = adapter_source
        .split_once("pub(in crate::sumeragi) fn open_gate(")
        .expect("adapter projection has one consuming gate open")
        .1
        .split_once("impl ProductionLifecycleAdapterStartupV1")
        .expect("gate open ends before adapter startup methods")
        .0;
    assert!(gate_open.contains("body_store: &super::v2_body_store::V2BodyStore"));
    assert!(gate_open.contains("body_store.matches_context(context)"));
    assert!(gate_open.contains("body_store\n            .recovery_catalog()"));
    assert!(gate_open.contains(".map(|(_, receipt)| receipt)"));
    assert!(!gate_open.contains("durable_bodies: &[DurableBodyReceipt]"));
    assert!(gate_open.contains(
        "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(\n            self.storage,"
    ));
    let adapter_launch = adapter_source
        .split_once("pub(in crate::sumeragi) fn prepare_leader_wire_launch(")
        .expect("adapter startup has one leader-wire projection")
        .1
        .split_once("/// Consume the sealed adapter startup directly")
        .expect("leader-wire projection ends before runtime consumption")
        .0;
    for required in [
        "&mut self",
        "!*leader_wire_launch_prepared",
        "adapter.wal.matches_path(expected_wal_path)",
        "adapter\n                    .mint_leader_wire_store_authority(expected_wal_path)",
        "*leader_wire_launch_prepared = true",
    ] {
        assert!(
            adapter_launch.contains(required),
            "one-shot adapter leader-wire projection omitted {required}"
        );
    }
    let runtime_conversion = adapter_source
        .split_once("pub(in crate::sumeragi) fn into_serialized_runtime(")
        .expect("adapter startup has one runtime conversion")
        .1
        .split_once(
            "#[cfg_attr(not(test), allow(dead_code))]\nimpl PreparedRecoveredPendingKuraApplyReplayV1",
        )
        .expect("runtime conversion ends before pending-Kura replay installation")
        .0;
    assert!(runtime_conversion.contains("leader_wire_launch_prepared: true"));
    let adapter_open = adapter_source
        .split_once("fn open_with_aggregator_and_publication_with_capacity(")
        .expect("adapter has one production recovery open")
        .1
        .split_once("/// Return the tag which must accompany a new asynchronous operation")
        .expect("adapter recovery open ends before projections")
        .0;
    let safety_open = adapter_open
        .find("let (wal_path, wal) = match wal_target")
        .expect("adapter selects one sealed WAL open target first");
    let kura_open = adapter_open
        .find("SafetyWal::open_with_kura_authority(")
        .expect("production adapter consumes the Kura-root authority");
    let fixture_open = adapter_open
        .find("SafetyWalOpenTarget::FixturePath(wal_path)")
        .expect("legacy pathname opening is explicitly test-only");
    let serviced_mint = adapter_open
        .find("wal.mint_serviced_candidate_store_authority(&wal_path)?")
        .expect("adapter mints the fixed serviced-candidate authority");
    let serviced_open = adapter_open
        .find("ServicedCandidateStore::open_with_safety_wal_authority(")
        .expect("adapter consumes the serviced-candidate authority");
    let wal_replay = adapter_open
        .find("let entries = wal\n            .recovered_records()")
        .expect("adapter replays the bound WAL after adjacent recovery");
    assert!(safety_open < kura_open && kura_open < fixture_open);
    assert!(fixture_open < serviced_mint && serviced_mint < serviced_open);
    assert!(serviced_open < wal_replay);

    for capability in [
        "SafetyWalServicedCandidateStoreAuthority",
        "SafetyWalLeaderWireStoreAuthority",
    ] {
        let declaration = safety_wal_source
            .split_once(&format!("pub(crate) struct {capability} {{"))
            .unwrap_or_else(|| panic!("missing {capability}"))
            .1
            .split_once("\n}")
            .expect("capability declaration is closed")
            .0;
        assert!(declaration.contains("entry: BoundSafetyWalAdjacentEntry"));
        assert!(!safety_wal_source.contains(&format!("impl Clone for {capability}")));
        assert!(!safety_wal_source.contains(&format!("impl Copy for {capability}")));
    }
    for required in [
        "#[cfg(any(test, not(all(unix, not(target_os = \"espidf\")))))]\nuse std::fs::OpenOptions;",
        "direct_lexical_directory_metadata(expected_path)?",
        "open_canonical_directory_nofollow(&canonical_path)?",
        "let metadata = fs::symlink_metadata(expected_path)?;",
        "fs::symlink_metadata(&self.expected_path)",
        "let linked = fs::symlink_metadata(self.expected_path.join(name))?;",
        "rustix::fs::OFlags::CREATE\n                        | rustix::fs::OFlags::EXCL",
        "unix_file_identity(&opened) != expected_identity",
        "fn write_all(&mut self, bytes: &[u8])",
        "fn sync_data(&mut self)",
        "self.directory.verify_leaf(self.file, self.wal_name)",
        "let durable = rustix::fs::statat(",
        "promoted adjacent snapshot changed across directory sync",
        "BoundSafetyWalDirectory::from_kura_authority(kura, authority)",
        "safety-WAL authority belongs to a different Kura instance",
        "#[cfg(test)]\n    fn bind(expected_path: &Path)",
        "#[cfg(test)]\n    pub(crate) fn open(",
    ] {
        assert!(
            safety_wal_source.contains(required),
            "opened WAL-directory authority omitted {required}"
        );
    }
    for required in [
        "store_root_directory: BoundProgressDirectory",
        "Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?",
        "KuraSafetyWalDirectoryAuthority",
        "#[derive(Debug)]\n#[must_use = \"the Kura-bound safety-WAL directory authority must open one WAL\"]",
    ] {
        assert!(
            kura_source.contains(required),
            "Kura storage owner omitted {required}"
        );
    }
    assert!(!kura_source.contains("impl Clone for KuraSafetyWalDirectoryAuthority"));
    assert!(!kura_source.contains("impl Copy for KuraSafetyWalDirectoryAuthority"));
    for required in [
        "pub(crate) fn mint_safety_wal_directory_authority(",
        "rustix::fs::openat(\n                &root.file,\n                STORE_ROOT_LOCK_FILE_NAME,",
        "Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)",
        "rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU)",
        "Self::open_bound_progress_child_directory(",
        "kura_identity: self.instance_identity()",
    ] {
        assert!(
            kura_source.contains(required),
            "Kura-root WAL authority omitted {required}"
        );
    }
    assert_eq!(
        safety_wal_source
            .matches("Err(SafetyWalError::UnsupportedStorageBinding {")
            .count(),
        3,
        "the production Kura-root open and both adjacent authority mints must reject on non-Unix"
    );
    assert_eq!(
        safety_wal_source
            .matches("snapshot storage is unsupported on this platform")
            .count(),
        3,
        "non-Unix adjacent read, publication, and retirement must have no path fallback"
    );
    assert!(adjacent_store_source.contains("storage: SafetyWalServicedCandidateStoreAuthority"));
    assert!(adjacent_store_source.contains("storage: SafetyWalLeaderWireStoreAuthority"));
    assert!(adjacent_store_source.contains("pub(crate) fn open_with_safety_wal_authority("));
    assert!(adjacent_store_source.contains(
        "#[cfg(test)]\n    #[allow(clippy::too_many_arguments)]\n    pub(crate) fn open("
    ));
    let runner_wal_mint = lifecycle_run_inner_source
        .find(".mint_safety_wal_directory_authority()")
        .expect("lifecycle runner mints the Kura-owned WAL directory authority");
    let runner_adapter_open = lifecycle_run_inner_source
        .find("SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(")
        .expect("lifecycle runner opens only through the typed WAL authority");
    assert!(runner_wal_mint < runner_adapter_open);
    assert!(lifecycle_run_inner_source.contains("kura.as_ref(),\n            wal_authority,"));
    assert!(
        take < executor
            && executor < genesis_gate
            && genesis_gate < genesis
            && genesis < worker
            && worker < certified_serve_gate
            && certified_serve_gate < joint_ingress_bind
            && joint_ingress_bind < identity
            && worker < identity
            && identity < complete
    );
    assert!(take_apply < worker && worker < worker_permit);
    assert!(!launch.contains("inputs.block_cadence"));
    assert!(!launch.contains("genesis_account_for_launch"));
    assert!(launch.contains(
            "completion_observer_activation: Some(\n                ProductionV2CompletionObserverActivationPermitV1"
        ));
    assert!(launch.contains("leader_wire_ingress_binding,"));
    assert!(source.contains("impl Drop for ProductionLeaderWireIngressBindingV1"));
    assert!(source.contains("certified_serve_gate: Option<CertifiedServeIngressGate>"));
    let launched_fields = source
        .split_once("pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {")
        .expect("launched wrapper has one declaration")
        .1
        .split_once("\n}")
        .expect("launched wrapper declaration is closed")
        .0;
    let services_field = launched_fields
        .find("services: ProductionV2Services")
        .expect("launched wrapper retains the service worker");
    let pending_kura_field = launched_fields
        .find("pending_kura_apply_replay:")
        .expect("launched wrapper retains pending-Kura replay ownership");
    let proposal_attempt_field = launched_fields
        .find("recovered_local_proposal_attempt:")
        .expect("launched wrapper retains recovered local-Proposal ownership");
    let sign_completion_field = launched_fields
            .find(
                "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
            )
            .expect("launched wrapper retains the guarded recovered Sign completion");
    let binding_field = launched_fields
        .find("leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1")
        .expect("launched wrapper retains leader-wire binding ownership");
    assert!(
        services_field < pending_kura_field
            && pending_kura_field < proposal_attempt_field
            && proposal_attempt_field < sign_completion_field
            && sign_completion_field < binding_field,
        "Rust field drop order must stop services before dropping the Sign guard and unbinding leader-wire ingress"
    );
    let leader_wire_drop = source
        .split_once("impl ProductionLeaderWireIngressBindingV1 {")
        .expect("leader-wire launch binding has one implementation")
        .1
        .split_once("impl Drop for ProductionLeaderWireIngressBindingV1")
        .expect("leader-wire binding Drop follows its implementation")
        .0;
    let close = leader_wire_drop
        .find("self.ingress.close()")
        .expect("leader-wire retirement closes ingress first");
    let unbind = leader_wire_drop
        .find("self.ingress.unbind_leader_wire_lifecycle_gate(gate)?")
        .expect("leader-wire retirement unbinds the exact retained gate");
    assert!(close < unbind);
    let joint_unbind = leader_wire_drop
        .find(".unbind_height_ingress_gates(certified_serve_gate, leader_wire_gate)")
        .expect("completed launch retirement detaches both exact gates atomically");
    assert!(close < joint_unbind);
    assert!(source.contains("impl Drop for ProductionV2CompletionObserverActivationPermitSealV1"));
    let worker_start = worker_source
        .split_once("pub(crate) fn start(")
        .expect("production services have one constructor")
        .1
        .split_once("/// Sign and retain all canonical chunks")
        .expect("service construction ends before outbound registration")
        .0;
    let legacy_start = worker_start
        .split_once(
            "/// Start with the exact application service used for recovered marker replay.",
        )
        .expect("legacy construction ends before the sealed transfer seam")
        .0;
    assert!(legacy_start.contains("let apply_service = V2ApplyService::new("));
    assert!(legacy_start.contains("Self::start_inner("));
    assert!(!legacy_start.contains("Self::start_with_apply_service("));
    let transferred_start = worker_start
        .split_once("pub(in crate::sumeragi) fn start_with_apply_service(")
        .expect("worker has one sealed recovered-service transfer seam")
        .1
        .split_once("fn start_inner(")
        .expect("sealed transfer validation precedes the shared constructor")
        .0;
    assert!(transferred_start.contains(
        "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1"
    ));
    let service_identity = transferred_start
        .find("apply_service.matches_lifecycle_launch(")
        .expect("worker rechecks exact recovered service identity");
    let enter_inner = transferred_start
        .find("Self::start_inner(")
        .expect("worker transfers only the checked service");
    assert!(service_identity < enter_inner);
    assert!(!transferred_start.contains("create_dir_all"));
    assert_eq!(
        worker_source
            .matches("ProductionLifecycleApplyServiceLaunchPermitV1")
            .count(),
        1,
        "only the sealed worker parameter may name the launch permit"
    );
    assert_eq!(
        source
            .matches("ProductionLifecycleApplyServiceLaunchPermitV1 {")
            .count(),
        1,
        "only lifecycle launch may construct the private permit"
    );
    assert!(coordinator_source.contains(
            "pub(in crate::sumeragi) struct ProductionLifecycleApplyServiceLaunchPermitV1 {\n    _seal: ProductionLifecycleApplyServiceLaunchPermitSealV1,\n}"
        ));
    assert!(
        coordinator_source
            .contains("impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1")
    );
    let status_publication = worker_source
        .split_once("fn publish_effect_status(")
        .expect("production services have one effect-status publisher")
        .1
        .split_once("fn fail_closed(")
        .expect("effect-status publication ends before fail-stop handling")
        .0;
    assert!(!worker_start.contains("set_v2_effect_completion_observer"));
    assert!(!worker_start.contains("activate_effect_completion_observer"));
    assert!(!worker_start.contains("publish_effect_status"));
    assert!(!status_publication.contains("set_v2_effect_completion_observer"));
    let observer_activation = worker_source
        .split_once("fn activate_effect_completion_observer(")
        .expect("the completion observer has one sealed activation seam")
        .1
        .split_once("/// Atomically reserve the selected lifecycle carrier")
        .expect("the sealed activation seam stays narrow")
        .0;
    assert!(observer_activation.contains("ProductionV2CompletionObserverActivationPermitV1"));
    let activation_arm = observer_activation
        .find("begin_fail_stop_operation()")
        .unwrap();
    let live_worker = observer_activation
        .find(".io\n            .as_ref()")
        .unwrap();
    let register = observer_activation
        .find("set_v2_effect_completion_observer")
        .unwrap();
    let activation_complete = observer_activation.find("activation.complete()").unwrap();
    assert!(
        activation_arm < live_worker && live_worker < register && register < activation_complete
    );
    assert_eq!(
        worker_source
            .matches("set_v2_effect_completion_observer(")
            .count(),
        1
    );
    assert!(!worker_source.contains("ProductionV2CompletionObserverActivationPermitV1 {"));
    assert!(!launch.contains("activate_effect_completion_observer("));
    assert!(!runner_source.contains("activate_effect_completion_observer("));

    let preactivation_runner = runner_source
        .split_once("pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1")
        .expect("runner has one sealed lifecycle preactivation borrow")
        .1
        .split_once("/// Cadence-derived process-local deadline")
        .expect("preactivation borrow ends before interrupted-tip recovery")
        .0;
    for required in [
        "_seal: ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "local_proposal: Option<ProductionLifecycleLocalProposalStateV1>",
        "struct ProductionLifecyclePreActivationRunnerBorrowSealV1;",
        "impl Drop for ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "fn mint_for_recovered_runner() -> Self",
        "local_proposal: Some(ProductionLifecycleLocalProposalStateV1::fresh())",
        "#[cfg(test)]",
        "pub(in crate::sumeragi) fn for_test() -> Self",
        "fn bind_recovered_local_proposal(",
        "let Some(local_proposal) = self.local_proposal.as_mut()",
        "if !local_proposal.state.is_pristine()",
        "LocalProposalState::from_recovered_lifecycle_attempt(true, directive)",
        "fn local_proposal_state_is_pristine(",
        "fn prepared_local_proposal_exactly_matches(",
    ] {
        assert!(preactivation_runner.contains(required));
    }
    for forbidden in [
        "derive(Clone)",
        "derive(Copy)",
        "pub _seal:",
        "pub(crate) _seal:",
        "pub(in crate::sumeragi) _seal:",
        "pub local_proposal:",
        "pub(crate) local_proposal:",
        "pub(in crate::sumeragi) local_proposal:",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner",
        "fn into_parts(",
    ] {
        assert!(!preactivation_runner.contains(forbidden));
    }
    let local_proposal_owner = runner_source
        .split_once("pub(in crate::sumeragi) struct ProductionLifecycleLocalProposalStateV1")
        .expect("runner retains one opaque lifecycle local-Proposal state owner")
        .1
        .split_once("/// Run the v2-only worker until shutdown")
        .expect("opaque lifecycle local-Proposal state ends before the runner")
        .0;
    assert!(local_proposal_owner.contains("state: LocalProposalState"));
    assert!(local_proposal_owner.contains("fn fresh() -> Self"));
    assert!(!local_proposal_owner.contains("pub state:"));
    assert!(!local_proposal_owner.contains("fn into_parts("));
    let prepared_local_proposal = source
        .split_once("struct ProductionLifecyclePreparedLocalProposalStateV1")
        .expect("launch owns one affine prepared local-Proposal state")
        .1
        .split_once("/// Opaque lifecycle stack after clocks")
        .expect("prepared local-Proposal state ends before activated ownership")
        .0;
    for required in [
        "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
        "context_id: wire::HeightContextId",
        "directive: super::super::v2::LocalProposalDirective",
        "fn exactly_matches(",
        "self.context_id == context_id",
        "self.directive == directive",
        "prepared_local_proposal_exactly_matches(directive)",
    ] {
        assert!(prepared_local_proposal.contains(required));
    }
    for forbidden in [
        "derive(Clone)",
        "derive(Copy)",
        "pub runner:",
        "pub context_id:",
        "pub directive:",
        "fn into_parts(",
    ] {
        assert!(!prepared_local_proposal.contains(forbidden));
    }
    assert!(runtime_source.contains(
            "pub(in crate::sumeragi) const fn lifecycle_live_clocks_are_armed(&self) -> bool {\n        self.clocks_armed\n    }"
        ));
    assert!(effects_source.contains(
            "pub(in crate::sumeragi) fn lifecycle_live_clocks_are_unarmed(&self) -> bool {\n        !self.runtime.lifecycle_live_clocks_are_armed()\n    }"
        ));
    let preactivation_fail_stop = source
        .split_once("struct ProductionLifecyclePreActivationFailStopScopeV1")
        .expect("preactivation setup has one non-permit fail-stop scope")
        .1
        .split_once("impl LaunchedProductionLifecycleV1")
        .expect("preactivation fail-stop scope ends before launched setup")
        .0;
    for required in [
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for ProductionLifecyclePreActivationFailStopScopeV1",
        "self.output_guard.close_admission_for_restart()",
    ] {
        assert!(preactivation_fail_stop.contains(required));
    }
    assert!(!preactivation_fail_stop.contains("ConsensusFailStopOperation"));
    let preactivation_setup = source
        .split_once("fn with_runner_setup_transaction<R, E>(")
        .expect("launched lifecycle has one sealed preactivation setup transaction")
        .1
        .split_once("/// Borrow executor and services for closed-ingress runner setup")
        .expect("private setup transaction ends before its runner aperture")
        .0;
    let setup_guard = preactivation_setup
        .find("let output_guard = self.services.lifecycle_output_guard()")
        .expect("preactivation setup binds the exact output guard first");
    let setup_initial_admission = preactivation_setup
        .find("let initial_admission = output_guard")
        .expect("preactivation setup witnesses initially open output");
    let setup_release_initial = preactivation_setup
        .find("drop(initial_admission)")
        .expect("preactivation setup releases its permit before the callback");
    let setup_arm = preactivation_setup
        .find("ProductionLifecyclePreActivationFailStopScopeV1::new")
        .expect("preactivation setup arms a non-permit fail-stop scope");
    let setup_owner = preactivation_setup
        .find("matches_lifecycle_executor_output_guard(&self.executor)")
        .expect("preactivation setup authenticates executor/service ownership");
    let setup_ingress = preactivation_setup
        .find("self.leader_wire_ingress_binding.ingress.state.lock().open")
        .expect("preactivation setup keeps exact ingress closed");
    let setup_observer = preactivation_setup
        .find("self.completion_observer_activation.is_none()")
        .expect("preactivation setup retains the observer authority");
    let setup_clocks = preactivation_setup
        .find("self.executor.lifecycle_live_clocks_are_unarmed()")
        .expect("preactivation setup keeps live clocks unarmed");
    let setup_callback = preactivation_setup
        .find("operation(&mut self.executor, &mut self.services)?")
        .expect("preactivation transaction exposes only executor and services");
    let setup_post_owner = preactivation_setup[setup_callback..]
        .find("matches_lifecycle_executor_output_guard(&self.executor)")
        .map(|offset| setup_callback + offset)
        .expect("preactivation setup reauthenticates ownership after the callback");
    let setup_post_ingress = preactivation_setup[setup_post_owner..]
        .find("self.leader_wire_ingress_binding.ingress.state.lock().open")
        .map(|offset| setup_post_owner + offset)
        .expect("preactivation setup rechecks closed ingress after the callback");
    let setup_post_observer = preactivation_setup[setup_post_ingress..]
        .find("self.completion_observer_activation.is_none()")
        .map(|offset| setup_post_ingress + offset)
        .expect("preactivation setup rechecks the observer after the callback");
    let setup_post_clocks = preactivation_setup[setup_post_observer..]
        .find("self.executor.lifecycle_live_clocks_are_unarmed()")
        .map(|offset| setup_post_observer + offset)
        .expect("preactivation setup rechecks live clocks after the callback");
    let setup_complete = preactivation_setup
        .find("setup.complete()")
        .expect("preactivation setup opens output only after postflight");
    let setup_final_admission = preactivation_setup
        .find("let final_admission = output_guard")
        .expect("preactivation setup re-witnesses open output before success");
    let setup_release_final = preactivation_setup
        .find("drop(final_admission)")
        .expect("preactivation setup releases its final witness after disarming");
    assert!(
        setup_guard < setup_initial_admission
            && setup_initial_admission < setup_arm
            && setup_arm < setup_release_initial
            && setup_arm < setup_owner
            && setup_owner < setup_ingress
            && setup_ingress < setup_observer
            && setup_observer < setup_clocks
            && setup_clocks < setup_callback
            && setup_callback < setup_post_owner
            && setup_post_owner < setup_post_ingress
            && setup_post_ingress < setup_post_observer
            && setup_post_observer < setup_post_clocks
            && setup_post_clocks < setup_final_admission
            && setup_final_admission < setup_complete
            && setup_complete < setup_release_final
    );
    assert!(!preactivation_setup.contains("begin_fail_stop_operation()"));
    for forbidden in [
        "&mut self.owner",
        "ProductionLifecyclePreActivationRunnerBorrowV1",
        "bind_recovered_local_proposal",
        "arm_live_clocks(",
        "activate_effect_completion_observer(",
        "open_and_publish(",
        "into_parts(",
    ] {
        assert!(!preactivation_setup.contains(forbidden));
    }
    let public_setup = source
        .split_once("pub(in crate::sumeragi) fn with_runner_setup<R, E>(")
        .expect("launched lifecycle exposes one sealed runner setup aperture")
        .1
        .split_once("/// Join one recovered local Proposal attempt")
        .expect("public setup aperture ends before Proposal initialization")
        .0;
    assert!(public_setup.contains("self.with_runner_setup_transaction(operation)"));
    assert!(!public_setup.contains("bind_recovered_local_proposal"));
    assert!(!public_setup.contains("operation(&mut self.executor"));

    let proposal_initialization = source
        .split_once("pub(in crate::sumeragi) fn initialize_recovered_local_proposal(")
        .expect("preactivation has one recovered local-Proposal join")
        .1
        .split_once("/// Install one opaque recovered-attempt fixture")
        .expect("local-Proposal join stays bounded before its test seam")
        .0;
    let proposal_take = proposal_initialization
        .find("self.recovered_local_proposal_attempt.take()")
        .expect("local-Proposal join consumes its opaque replay owner once");
    let proposal_setup = proposal_initialization
        .find("self.with_runner_setup_transaction(")
        .expect("local-Proposal join remains inside closed-ingress setup");
    let proposal_directive = proposal_initialization
        .find(".local_proposal_directive()")
        .expect("local-Proposal join reads only the reducer directive");
    let proposal_compare = proposal_initialization
        .find("recovered.exactly_matches_directive(directive)")
        .expect("local-Proposal join compares through the opaque oracle");
    let proposal_bind = proposal_initialization
        .find("runner.bind_recovered_local_proposal(directive)")
        .expect("local-Proposal join mutates the real runner-owned state");
    let proposal_non_pristine = proposal_initialization
        .find("ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine")
        .expect("non-pristine runner-local Proposal state fails closed");
    let proposal_mismatch = proposal_initialization
        .find("ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch")
        .expect("local-Proposal drift fails closed");
    let proposal_result = proposal_initialization
        .find("ProductionLifecyclePreparedLocalProposalStateV1 {")
        .expect("local-Proposal join privately mints its affine state owner");
    let proposal_return = proposal_initialization
        .find("Ok((directive, prepared))")
        .expect("local-Proposal join returns the directive with its affine state owner");
    assert!(
        proposal_take < proposal_setup
            && proposal_setup < proposal_directive
            && proposal_directive < proposal_compare
            && proposal_compare < proposal_bind
            && proposal_bind < proposal_non_pristine
            && proposal_non_pristine < proposal_mismatch
            && proposal_mismatch < proposal_result
            && proposal_result < proposal_return
    );
    assert_eq!(
        proposal_initialization
            .matches("runner.bind_recovered_local_proposal(directive)")
            .count(),
        1,
        "only the WAL-authenticated initializer may bind runner Proposal state"
    );
    for forbidden in ["fn into_parts(", "fn tag(", "fn round(", "fn subject("] {
        assert!(!proposal_initialization.contains(forbidden));
    }

    let activation_blocker = source
        .split_once("fn lifecycle_activation_recovery_blocker(")
        .expect("activation has one recovery preflight classifier")
        .1
        .split_once("/// Fail-stop failure while consuming an activated height")
        .expect("activation recovery classifier stays bounded")
        .0;
    let pending_blocker = activation_blocker
        .find("pending_kura_replay || pending_kura_evidence")
        .expect("pending-Kura recovery blocks ordinary clocks");
    let pending_error = activation_blocker
        .find("ProductionLifecycleActivationErrorV1::PendingKuraApply")
        .expect("pending-Kura recovery maps to its exact error");
    let proposal_blocker = activation_blocker
        .find("else if recovered_local_proposal")
        .expect("uninitialized recovered Proposal blocks ordinary clocks");
    let proposal_error = activation_blocker
        .find("ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized")
        .expect("uninitialized recovered Proposal maps to its exact error");
    assert!(
        pending_blocker < pending_error
            && pending_error < proposal_blocker
            && proposal_blocker < proposal_error
    );

    let lifecycle_activation = source
        .split_once("fn activate_with(")
        .expect("the launched lifecycle has one consuming activation transaction")
        .1
        .split_once("impl ActivatedProductionLifecycleV1")
        .expect("activation ends before the runner-borrowed live type state")
        .0;
    let recovery_blocker = lifecycle_activation
        .find("lifecycle_activation_recovery_blocker(")
        .expect("activation checks every recovery-only precondition first");
    let recovery_close = lifecycle_activation
        .find("close_admission_for_restart()")
        .expect("activation closes output when recovery setup is incomplete");
    let recovery_error = lifecycle_activation
        .find("return Err(error)")
        .expect("activation returns only after fail-stop closure");
    let activation_guard = lifecycle_activation
        .find("begin_fail_stop_operation()")
        .expect("activation arms the process-wide fail-stop boundary");
    let proposal_reproject = lifecycle_activation
        .find(".local_proposal_directive()")
        .expect("activation reprojects the reducer Proposal directive");
    let proposal_exact = lifecycle_activation
        .find("local_proposal.exactly_matches(self.executor.context().id(), current_directive)")
        .expect("activation rejoins prepared state to this exact lifecycle");
    let proposal_mismatch = lifecycle_activation
        .find("ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch")
        .expect("foreign or stale prepared Proposal state fails closed");
    let clock_activation = lifecycle_activation
        .find("let clock_activation = ProductionLifecycleLiveClockActivationPermitV1")
        .expect("activation mints the one ordinary live-clock permit");
    let clocks = lifecycle_activation
        .find("arm_live_clocks(clock_activation, now)")
        .expect("activation consumes the ordinary permit while arming live clocks");
    let status = lifecycle_activation
        .find("successor_activation_status_snapshot()")
        .expect("activation projects status only after clocks arm");
    let observer = lifecycle_activation
        .find("completion_observer_activation.take()")
        .expect("activation consumes the sole observer permit");
    let register_observer = lifecycle_activation
        .find("activate_effect_completion_observer(observer)")
        .expect("activation installs the completion observer");
    let publish = lifecycle_activation
        .find("publication.open_and_publish(")
        .expect("activation delegates ingress and status to runner authority");
    let complete = lifecycle_activation
        .find("activation.complete()")
        .expect("activation releases output only after publication");
    let activated = lifecycle_activation
            .find("ActivatedProductionLifecycleV1 {\n            runner_activation,\n            local_proposal,\n            launched: self,")
            .expect("activation returns the sole opaque live owner");
    assert!(
        recovery_blocker < recovery_close
            && recovery_close < recovery_error
            && recovery_error < activation_guard
            && activation_guard < proposal_reproject
            && proposal_reproject < proposal_exact
            && proposal_exact < proposal_mismatch
            && proposal_mismatch < clock_activation
            && clock_activation < clocks
            && clocks < status
            && status < observer
            && observer < register_observer
            && register_observer < publish
            && publish < complete
            && complete < activated
    );
    assert!(!lifecycle_activation.contains("set_v2_status"));
    assert!(!lifecycle_activation.contains("into_parts"));

    let activated_owner = source
        .split_once("struct ActivatedProductionLifecycleV1")
        .expect("activation returns one opaque owner type state")
        .1
        .split_once("enum ProductionLifecycleActivationPublicationV1")
        .expect("the activated owner declaration ends before publication authority")
        .0;
    assert!(activated_owner.contains(
        "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1"
    ));
    assert!(
        activated_owner.contains("local_proposal: ProductionLifecyclePreparedLocalProposalStateV1")
    );
    assert!(activated_owner.contains("launched: LaunchedProductionLifecycleV1"));
    assert!(
        activated_owner.find("runner_activation:").unwrap()
            < activated_owner.find("local_proposal:").unwrap()
            && activated_owner.find("local_proposal:").unwrap()
                < activated_owner.find("launched:").unwrap(),
        "readiness and local Proposal state must drop before durable gates"
    );
    for forbidden in [
        "pub launched:",
        "pub(crate) launched:",
        "pub(in crate::sumeragi) launched:",
        "pub runner_activation:",
        "pub(crate) runner_activation:",
        "pub(in crate::sumeragi) runner_activation:",
        "pub local_proposal:",
        "pub(crate) local_proposal:",
        "pub(in crate::sumeragi) local_proposal:",
        "impl Clone for ActivatedProductionLifecycleV1",
        "impl Copy for ActivatedProductionLifecycleV1",
    ] {
        assert!(!activated_owner.contains(forbidden));
    }
    let activated_borrow = source
        .split_once("impl ActivatedProductionLifecycleV1")
        .expect("the activated owner has one runner-borrow surface")
        .1
        .split_once("impl ProductionLifecycleOwnerV1")
        .expect("the activated owner surface ends before launch helpers")
        .0;
    for required in [
        "fn with_runner_runtime<R>(",
        "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
        "&mut super::super::v2_runner::ProductionLifecycleLocalProposalStateV1",
        ".prepared_local_proposal_mut()",
        "&mut self.launched.owner",
        "&mut self.launched.executor",
        "&mut self.launched.services",
        "local_proposal",
    ] {
        assert!(activated_borrow.contains(required));
    }
    for forbidden in [
        "into_parts",
        "fn into_owner(",
        "fn into_executor(",
        "fn into_services(",
        "pub launched:",
        "pub(crate) launched:",
    ] {
        assert!(!activated_borrow.contains(forbidden));
    }

    let serve_retirement = source
        .split_once("fn refresh_live_serve_retirement_cut(")
        .expect("live Serve retirement has one launch-private join")
        .1
        .split_once("/// Cross the ordinary/current/snapshot live-height boundary")
        .expect("live Serve retirement stays bounded before activation")
        .0;
    let registry_census = serve_retirement
        .find("exactly_covers_finalization_work(&self.coordinator)")
        .expect("retirement rejoins the exact live concrete registry");
    let service_census = serve_retirement
        .find("authenticate_current_lifecycle_serve_retirement(")
        .expect("retirement authenticates through the exact launched service");
    let ledger = serve_retirement
        .find("LifecycleLedgerV1::from_coordinator(&self.coordinator)")
        .expect("retirement derives the current ledger from the same owner");
    let payload_census = serve_retirement
        .find("authenticate_live_finalization_serve_census(")
        .expect("retirement joins ledger rows and admission-wait payloads");
    let install = serve_retirement
        .find("self.serve_payloads = refreshed")
        .expect("retirement replaces the stale startup cut only after authentication");
    assert!(
        registry_census < service_census
            && service_census < ledger
            && ledger < payload_census
            && payload_census < install
    );
    assert!(
        serve_retirement.contains("_retired_ingress: &ProductionLifecycleRetiredIngressPermitV1")
    );
    assert!(!serve_retirement.contains("CertifiedServePayloadStoreV1::open("));
    for authority in [
        "ProductionLifecycleServeRetirementAuthenticationPermitV1",
        "ProductionLifecycleRetiredIngressPermitV1",
    ] {
        assert!(!source.contains(&format!("impl Clone for {authority}")));
        assert!(!source.contains(&format!("impl Copy for {authority}")));
    }
    let fixture_retirement = activated_borrow
        .split_once("fn retire_lifecycle_stores_for_test(")
        .expect("activation behavior has one consuming retirement fixture")
        .1
        .split_once("/// Borrow the live owner/runtime/service triple")
        .expect("retirement fixture ends before the ordinary runner borrow")
        .0;
    let fixture_owner_order = fixture_retirement
            .find("let Self {\n            mut launched,\n            local_proposal,\n            runner_activation,")
            .expect("fixture retains drop-safe launched/local/runner binding order");
    let readiness_retire = fixture_retirement
        .find("runner_activation\n            .retire(")
        .expect("retirement clears runner readiness first");
    let gates_retire = fixture_retirement
        .find("leader_wire_ingress_binding\n            .retire()")
        .expect("retirement detaches both ingress gates second");
    let output_handoff = fixture_retirement
        .find("seal_empty_exact_output_for_lifecycle_retirement_test()")
        .expect("fixture seals its exact empty output handoff");
    let refresh = fixture_retirement
        .find("refresh_live_serve_retirement_cut(&launched.services, &retired_ingress)")
        .expect("fixture refreshes Serve only after output handoff");
    let retirement = fixture_retirement
        .find(".retire_lifecycle_stores()")
        .expect("fixture exercises the post-handoff durable retirement tail");
    assert!(
        fixture_owner_order < readiness_retire
            && readiness_retire < gates_retire
            && gates_retire < output_handoff
            && output_handoff < refresh
            && refresh < retirement
    );

    let activated_finalization = activated_borrow
        .split_once("fn into_finalized_rollover(")
        .expect("activated owner has one consuming finalization")
        .1
        .split_once("/// Exercise the exact empty-output post-handoff retirement transaction")
        .expect("production finalization ends before its behavior fixture")
        .0;
    let executor_ready = activated_finalization
        .find("executor.ready_to_finish()")
        .expect("finalization first proves exact executor quiescence");
    let registry_ready = activated_finalization
        .find("exactly_covers_finalization_work")
        .expect("finalization first proves exact lifecycle-owner quiescence");
    let owner_order = activated_finalization
            .find("let Self {\n            mut launched,\n            local_proposal,\n            runner_activation,")
            .expect("finalization retains drop-safe launched/local/runner binding order");
    let runner_retire = activated_finalization
        .find("runner_activation\n            .retire(")
        .expect("finalization clears runner readiness and ingress");
    let gate_retire = activated_finalization
        .find("leader_wire_ingress_binding\n            .retire()")
        .expect("finalization jointly retires both durable ingress gates");
    let executor_consume = activated_finalization
        .find("executor\n            .into_finalized_parts()")
        .expect("finalization consumes the exact executor after gate retirement");
    let operation = activated_finalization
        .find("begin_fail_stop_operation()")
        .expect("adapter finalization is fail-stop guarded");
    let adapter_finish = activated_finalization
        .find(".finish_height(&receipt, &artifact)")
        .expect("the serialized adapter consumes exact Kura finality");
    let operation_complete = activated_finalization
        .find("operation.complete()")
        .expect("adapter finalization completes the fail-stop operation last");
    assert!(executor_ready < owner_order && registry_ready < owner_order);
    assert!(
        owner_order < runner_retire
            && runner_retire < gate_retire
            && gate_retire < executor_consume
            && executor_consume < operation
            && operation < adapter_finish
            && adapter_finish < operation_complete
    );

    let rollover = source
        .split_once("impl FinalizedProductionLifecycleRolloverV1")
        .expect("finalized owner has one output-rollover implementation")
        .1
        .split_once("impl ProductionLifecyclePostOutputHandoffV1")
        .expect("output rollover ends before lifecycle-store retirement")
        .0;
    let sealed_output = rollover
        .find("rollover_finalized_height_outputs_for_lifecycle(")
        .expect("finalized owner invokes the existing exact output handoff");
    let output_permit = rollover
        .find("ProductionLifecycleOutputRolloverPermitV1 {")
        .expect("only the finalized owner mints the sibling-call permit");
    let serve_refresh = rollover
        .find("refresh_live_serve_retirement_cut(&services, &retired_ingress)")
        .expect("Serve census refresh follows durable output handoff");
    assert!(sealed_output < output_permit && output_permit < serve_refresh);
    assert!(finalized_output_source.contains(
        "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1"
    ));

    let store_retirement = source
        .split_once("impl ProductionLifecyclePostOutputHandoffV1")
        .expect("post-output owner has one lifecycle-store implementation")
        .1
        .split_once("impl ProductionLifecycleCleanupReadyV1")
        .expect("store retirement ends before clean worker teardown")
        .0;
    let retirement_operation = store_retirement
        .find("begin_fail_stop_operation()")
        .expect("store retirement arms process-wide fail-stop ownership");
    let payload_retire = store_retirement
        .find(".retire_authenticated_cut(serve_payloads, &retained_serve_payloads)")
        .expect("the exact live payload cut retires before LedgerV1");
    let ledger_stage = store_retirement
        .find(".stage_finalized_height_all_row_retirement(reconciliation)")
        .expect("all rows stage from the refreshed Serve cut");
    let ledger_publish = store_retirement
        .find(".persist_exact_finalization_successor(staged)")
        .expect("the opaque staged successor fsyncs exactly once");
    let owner_consume = store_retirement
        .find("publication.consume_owners(registry)")
        .expect("only the published token consumes logical and concrete owners");
    let retirement_complete = store_retirement
        .find("operation.complete()")
        .expect("store retirement releases fail-stop ownership last");
    assert!(
        retirement_operation < payload_retire
            && payload_retire < ledger_stage
            && ledger_stage < ledger_publish
            && ledger_publish < owner_consume
            && owner_consume < retirement_complete
    );
    let cleanup = source
        .split_once("impl ProductionLifecycleCleanupReadyV1")
        .expect("cleanup-ready owner has one consuming cleanup")
        .1
        .split_once("impl ProductionLifecycleOwnerV1")
        .expect("cleanup-ready surface ends before launch construction")
        .0;
    assert!(
        cleanup
            .find("self.services.allow_clean_shutdown()")
            .expect("only cleanup-ready state permits normal service Drop")
            < cleanup
                .find(".finish_height(self.receipt, cleanup_timeout, supervisor)")
                .expect("clean service teardown follows explicit permission")
    );

    for state in [
        "FinalizedProductionLifecycleRolloverV1",
        "ProductionLifecyclePostOutputHandoffV1",
        "ProductionLifecycleCleanupReadyV1",
        "StagedFinalizationRetirementV1",
        "PublishedFinalizationRetirementV1",
        "ProductionLifecycleOutputRolloverPermitV1",
    ] {
        assert!(!source.contains(&format!("impl Clone for {state}")));
        assert!(!source.contains(&format!("impl Copy for {state}")));
        assert!(!ledger_source.contains(&format!("impl Clone for {state}")));
        assert!(!ledger_source.contains(&format!("impl Copy for {state}")));
        let declaration_source = if source.contains(&format!("struct {state}")) {
            source
        } else {
            ledger_source
        };
        let start = declaration_source
            .find(&format!("struct {state}"))
            .unwrap_or_else(|| panic!("missing opaque finalization state {state}"));
        let prefix = &declaration_source[..start];
        let declaration_start = prefix.rfind("\n\n").unwrap_or(0);
        let declaration_end = declaration_source[start..]
            .find("\n}")
            .map(|offset| start + offset)
            .expect("opaque finalization declaration is closed");
        let declaration = &declaration_source[declaration_start..declaration_end];
        assert!(!declaration.contains("Clone"));
        assert!(!declaration.contains("Copy"));
        assert!(!declaration.contains("pub owner:"));
        assert!(!declaration.contains("pub coordinator:"));
        assert!(!declaration.contains("pub services:"));
        assert!(!declaration.contains("pub current:"));
        assert!(!declaration.contains("pub retired:"));
    }
    let published_retirement = ledger_source
        .split_once("fn persist_exact_finalization_successor(")
        .expect("coordinator has one consuming finalization publication")
        .1
        .split_once("#[cfg(test)]")
        .expect("finalization publication ends before test helpers")
        .0;
    let consume_coordinator = published_retirement
        .find("self,")
        .expect("publication consumes the exact coordinator instance");
    let exact_source = published_retirement
        .find("LifecycleLedgerV1::from_coordinator(&self)? != current")
        .expect("publication rejoins the staged source to that coordinator");
    let persist = published_retirement
        .find("store.persist_exact_successor(&current, &retired)?")
        .expect("publication fsyncs the exact staged successor");
    let reload = published_retirement
        .find("store.load()? != retired")
        .expect("publication revalidates the linked store after fsync");
    let sealed = published_retirement
        .find("coordinator: self")
        .expect("published token retains the exact consumed coordinator");
    assert!(
        consume_coordinator < exact_source
            && exact_source < persist
            && persist < reload
            && reload < sealed
    );
    assert!(
        lifecycle_startup_test_source
            .contains("production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout")
    );
    let proposal_initialization_behavior = lifecycle_startup_test_source
        .split_once("fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()")
        .expect("Kura-bound owner has one preactivation Proposal behavior fixture")
        .1
        .split_once("fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network")
        .expect("preactivation Proposal fixture ends before factory-input tests")
        .0;
    let proposal_fixture = proposal_initialization_behavior
        .find("RecoveredLifecycleLocalProposalAttemptV1::for_test(")
        .expect("fixture retains one opaque recovered Proposal attempt");
    let proposal_retain = proposal_initialization_behavior
        .find("retain_recovered_local_proposal_attempt_for_test(recovered_attempt)")
        .expect("fixture installs the opaque attempt without exposing its parts");
    let proposal_initialize = proposal_initialization_behavior
        .find("initialize_recovered_local_proposal(setup_runner)")
        .expect("fixture executes the production preactivation join");
    let proposal_attempted = proposal_initialization_behavior
        .find("assert!(local_proposal_state.already_attempted(directive))")
        .expect("fixture proves the real runner-local state owns the Proposal");
    let proposal_activate = proposal_initialization_behavior
        .find(".activate(Instant::now(), activation, local_proposal_state)")
        .expect("fixture activates only after Proposal initialization");
    assert!(
        proposal_fixture < proposal_retain
            && proposal_retain < proposal_initialize
            && proposal_initialize < proposal_attempted
            && proposal_attempted < proposal_activate
    );
    assert!(
        lifecycle_startup_test_source
            .contains(".retire_lifecycle_stores_for_test(finality_receipt)")
    );
    assert!(
        lifecycle_startup_test_source
            .contains("cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)")
    );
    let finalization_behavior = lifecycle_startup_test_source
            .split_once(
                "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
            )
            .expect("marker replay has one production finalization behavior fixture")
            .1
            .split_once("fn expect_recovered_open_error")
            .expect("production finalization behavior ends before recovery helpers")
            .0;
    let status_guard = finalization_behavior
        .find("let _status_guard = crate::sumeragi::status::rbc_status_test_guard()")
        .expect("the production finalization fixture serializes global status mutation");
    let genesis_transaction = finalization_behavior
        .find("TransactionBuilder::new_genesis(")
        .expect("the production finalization fixture uses a genesis-domain transaction");
    let genesis_key = finalization_behavior
        .find("Algorithm::Ed25519")
        .expect("the genesis transaction uses an allowed non-consensus signing key");
    let genesis_da = finalization_behavior
        .find("block_builder.set_da_proof_policies(Some(proof_policy_bundle))")
        .expect("the production finalization genesis seals its active DA policy");
    let genesis_signature = finalization_behavior
        .find(".try_build_with_signature(0, genesis_key.private_key())")
        .expect("the configured genesis authority signs at index zero");
    let genesis_policy = finalization_behavior
        .find("BlockSignaturePolicy::GenesisAuthority(")
        .expect("the recovered body store retains the genesis signature policy");
    let decision = finalization_behavior
        .find("WalRecordV2::Decision(decision)")
        .expect("the finalization fixture starts from a durable Decision");
    let launch = finalization_behavior
        .find("let mut launched = owner")
        .expect("the recovered Decision owner launches through production");
    let dispatch = finalization_behavior
        .find(".dispatch_recovered_decision_apply(")
        .expect("the recovered Apply uses the lifecycle scheduler");
    let settle = finalization_behavior
        .find("settle_recovered_decision_apply_completion(&mut lane_work)")
        .expect("the recovered Apply publishes exact finality");
    let activation = finalization_behavior
        .find("let activated = launched")
        .expect("the completed recovered height activates through the runner seal");
    let finalize = finalization_behavior
        .find(".into_finalized_rollover(&mut runner)")
        .expect("the activated owner runs the production finalization transition");
    let retain_decision = finalization_behavior
        .find(".retain_merge_sidecars_for_global_view(")
        .expect("the lane owner retains the ordinary exact Decision carrier");
    let output = finalization_behavior
        .find(".rollover_outputs(&mut runner, lane_work, &successor, 64)")
        .expect("the exact service and lane owners seal output together");
    let stores = finalization_behavior
        .find(".retire_lifecycle_stores()")
        .expect("lifecycle stores retire only after output handoff");
    let workers = finalization_behavior
        .find("cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)")
        .expect("clean worker teardown is the final behavior step");
    assert!(
        status_guard < genesis_key
            && genesis_key < genesis_transaction
            && genesis_transaction < genesis_da
            && genesis_da < genesis_signature
            && genesis_signature < genesis_policy
            && genesis_policy < decision
            && decision < launch
            && launch < dispatch
            && dispatch < settle
            && settle < activation
            && activation < finalize
            && finalize < retain_decision
            && retain_decision < output
            && output < stores
            && stores < workers
    );
    assert!(registry_validate_source.contains("broadcast.is_unpaired()"));
    assert!(
        registry_validate_source
            .contains("carrier.pairs_exact_next_sign(next_sign, next_sign_digest)")
    );

    let current_payload_census = payload_store_source
        .split_once("fn authenticate_current_for_lifecycle_retirement(")
        .expect("Serve store has one current retirement census")
        .1
        .split_once("/// Compare this opened payload owner")
        .expect("current retirement census stays bounded")
        .0;
    for required in [
        "self.reload_payload_census_strict()?",
        "payloads.keys().copied().collect::<BTreeSet<_>>() != self.indexed",
        ".authenticate_for_complete_tip_retirement(verified, local_signer)",
        "self.validate_authenticated_cut(&authenticated)?",
    ] {
        assert!(current_payload_census.contains(required));
    }
    let live_serve_join = lifecycle_open_source
        .split_once("fn authenticate_live_finalization_serve_census(")
        .expect("Serve retirement has one ledger/wait join")
        .1
        .split_once("/// Seal the final post-mutation Serve cut")
        .expect("live Serve join stays bounded")
        .0;
    for required in [
        "LifecycleLedgerV1::from_coordinator(coordinator)",
        "authenticate_complete_tip_serve_census(ledger, recovered)?",
        "WaitSource::Capacity(class)",
        "receipt.exactly_matches_pending(payload.request())",
        "prepare_certified_serve_admission(",
        "candidate != waiting.candidate",
        "owned != recovered_ids",
    ] {
        assert!(live_serve_join.contains(required));
    }
    let finalization_registry = registry_validate_source
        .split_once("fn exactly_covers_finalization_work(")
        .expect("registry has one finalization-only census")
        .1
        .split_once("fn exactly_covers_ready_work_with_extra(")
        .expect("finalization census delegates to the shared exact coverage")
        .0;
    assert!(
        finalization_registry
            .contains("exactly_covers_ready_work_with_extra(coordinator, extra, None, true)")
    );
    assert!(registry_validate_source.contains("broadcast.matches_current_finalization_record("));

    let runner_dependency_permit = runner_authority_source
        .split_once(
            "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
        )
        .expect("runner owns the recovered lifecycle dependency permit")
        .1
        .split_once("/// Cadence-derived process-local deadline")
        .expect("runner dependency permit stays a bounded source region")
        .0;
    for required in [
        "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
        "local_signer: KeyPair",
        "block_cadence: Duration",
        "fn mint_for_recovered_runner(\n        local_signer: KeyPair,\n        block_cadence: Duration,\n    ) -> Self",
        "#[cfg(test)]",
        "fn for_test(",
        "fn into_factory_dependencies(self) -> (KeyPair, Duration)",
        "(self.local_signer, self.block_cadence)",
        "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
    ] {
        assert!(runner_dependency_permit.contains(required));
    }
    for forbidden in [
        "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
        "pub(crate) fn mint_for_recovered_runner(",
        "pub fn mint_for_recovered_runner(",
        "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
        "fn into_parts(",
    ] {
        assert!(!runner_dependency_permit.contains(forbidden));
    }
    let ordinary_activation = runner_dependency_permit
        .split_once("struct ProductionLifecycleRunnerActivationV1")
        .expect("runner retains one ordinary activation authority")
        .1
        .split_once("struct ProductionLifecycleCompleteTipRunnerActivationV1")
        .expect("ordinary activation ends before the CompleteTip authority")
        .0;
    for required in [
        "_seal: ProductionLifecycleRunnerActivationSealV1",
        "ingress_ready: Arc<AtomicBool>",
        "block_ingress: Arc<FairV2Ingress>",
        "status: ProductionLifecycleRunnerStatusAuthorityV1",
        "struct ProductionLifecycleRunnerActivationSealV1",
        "impl Drop for ProductionLifecycleRunnerActivationSealV1",
        "fn current_height(",
        "fn applied(",
        "fn snapshot_bootstrap(",
        "CurrentHeight",
        "Applied",
        "SnapshotBootstrap",
        "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
        "self.ingress_ready.store(false, Ordering::Release)",
        "self.block_ingress.open()",
        "status::set_v2_status(successor)",
        "status::activate_v2_successor_height(",
        "status::activate_snapshot_bootstrap_v2_height(",
        "self.block_ingress.close()",
        "self.ingress_ready.store(true, Ordering::Release)",
        "ProductionLifecycleActivatedRunnerAuthorityV1 {",
        "ingress_ready: self.ingress_ready",
        "block_ingress: self.block_ingress",
    ] {
        assert!(ordinary_activation.contains(required));
    }
    let close_readiness = ordinary_activation
        .find("self.ingress_ready.store(false, Ordering::Release)")
        .unwrap();
    let exact_ingress = ordinary_activation.find("Arc::ptr_eq").unwrap();
    let reject_close = ordinary_activation
        .find("self.block_ingress.close()")
        .unwrap();
    let open_ingress = ordinary_activation
        .find("self.block_ingress.open()")
        .unwrap();
    let publish_status = ordinary_activation
        .find("let publication = match self.status")
        .unwrap();
    let release_readiness = ordinary_activation
        .rfind("self.ingress_ready.store(true, Ordering::Release)")
        .unwrap();
    assert!(
        close_readiness < exact_ingress
            && exact_ingress < reject_close
            && reject_close < open_ingress
            && open_ingress < publish_status
            && publish_status < release_readiness
    );
    for forbidden in [
        "impl Clone for ProductionLifecycleRunnerActivationV1",
        "impl Copy for ProductionLifecycleRunnerActivationV1",
        "pub(in crate::sumeragi) fn current_height(",
        "pub(crate) fn current_height(",
        "pub fn current_height(",
        "pub(in crate::sumeragi) fn applied(",
        "pub(in crate::sumeragi) fn snapshot_bootstrap(",
        "fn into_parts(",
    ] {
        assert!(!ordinary_activation.contains(forbidden));
    }

    let complete_tip_activation = runner_dependency_permit
        .split_once("struct ProductionLifecycleCompleteTipRunnerActivationV1")
        .expect("runner retains one CompleteTip activation authority")
        .1
        .split_once("struct ProductionLifecycleActivatedRunnerAuthorityV1")
        .expect("CompleteTip activation ends before the live runner borrow key")
        .0;
    for required in [
        "_seal: ProductionLifecycleCompleteTipRunnerActivationSealV1",
        "struct ProductionLifecycleCompleteTipRunnerActivationSealV1",
        "impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1",
        "fn mint_for_recovered_runner(",
        "ProductionLifecycleActivatedRunnerAuthorityV1 {",
        "ingress_ready: self.ingress_ready",
        "block_ingress: self.block_ingress",
    ] {
        assert!(complete_tip_activation.contains(required));
    }
    let close_readiness = complete_tip_activation
        .find("self.ingress_ready.store(false, Ordering::Release)")
        .unwrap();
    let exact_ingress = complete_tip_activation.find("Arc::ptr_eq").unwrap();
    let retirement_join = complete_tip_activation
        .find("retirement.authorizes_successor_status(&successor)")
        .unwrap();
    let open_ingress = complete_tip_activation
        .find("self.block_ingress.open()")
        .unwrap();
    let publish_status = complete_tip_activation
        .find("status::activate_recovered_complete_tip_v2_height(retirement, successor)")
        .unwrap();
    let release_readiness = complete_tip_activation
        .find("self.ingress_ready.store(true, Ordering::Release)")
        .unwrap();
    assert!(
        close_readiness < exact_ingress
            && exact_ingress < retirement_join
            && retirement_join < open_ingress
            && open_ingress < publish_status
            && publish_status < release_readiness
    );
    assert_eq!(
        complete_tip_activation
            .matches("self.block_ingress.close()")
            .count(),
        3,
        "mismatch, invalid retirement, and publication failure each close exact ingress"
    );
    for forbidden in [
        "impl Clone for ProductionLifecycleCompleteTipRunnerActivationV1",
        "impl Copy for ProductionLifecycleCompleteTipRunnerActivationV1",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
        "pub(crate) fn mint_for_recovered_runner(",
        "pub fn mint_for_recovered_runner(",
        "fn into_parts(",
    ] {
        assert!(!complete_tip_activation.contains(forbidden));
    }
    let activated_runner = runner_dependency_permit
        .split_once("struct ProductionLifecycleActivatedRunnerAuthorityV1")
        .expect("activation retains one exact readiness/ingress owner")
        .1
        .split_once("struct ProductionLifecycleActiveRunnerBorrowV1")
        .expect("activated runner ownership ends before the live borrow key")
        .0;
    for required in [
        "_seal: ProductionLifecycleActivatedRunnerAuthoritySealV1",
        "ingress_ready: Arc<AtomicBool>",
        "block_ingress: Arc<FairV2Ingress>",
        "impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1",
        "fn retire(",
        "retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)",
        "fn retire_lifecycle_runner_ingress(",
        "ingress_ready.store(false, Ordering::Release)",
        "block_ingress.close()",
        "Arc::ptr_eq(block_ingress, launched_ingress)",
        "self.ingress_ready.store(false, Ordering::Release)",
        "self.block_ingress.close()",
        "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
    ] {
        assert!(activated_runner.contains(required));
    }
    for forbidden in [
        "impl Clone for ProductionLifecycleActivatedRunnerAuthorityV1",
        "impl Copy for ProductionLifecycleActivatedRunnerAuthorityV1",
        "fn into_parts(",
        "pub ingress_ready:",
        "pub block_ingress:",
    ] {
        assert!(!activated_runner.contains(forbidden));
    }
    assert_eq!(
        activated_runner
            .matches("self.ingress_ready.store(false, Ordering::Release)")
            .count(),
        1
    );
    assert_eq!(
        activated_runner
            .matches("self.block_ingress.close()")
            .count(),
        1
    );
    let helper_start = activated_runner
        .find("fn retire_lifecycle_runner_ingress(")
        .expect("activated runner keeps one shared ingress-retirement helper");
    let helper = &activated_runner[helper_start..];
    let readiness_close = helper
        .find("ingress_ready.store(false, Ordering::Release)")
        .unwrap();
    let ingress_close = helper.find("block_ingress.close()").unwrap();
    let exact_ingress = helper
        .find("Arc::ptr_eq(block_ingress, launched_ingress)")
        .unwrap();
    assert!(readiness_close < ingress_close && ingress_close < exact_ingress);
    let runner_borrow = runner_dependency_permit
        .split_once("struct ProductionLifecycleActiveRunnerBorrowV1")
        .expect("runner owns one live borrow key")
        .1;
    assert!(runner_borrow.contains("fn mint_for_recovered_runner() -> Self"));
    assert!(!runner_borrow.contains("pub(in crate::sumeragi) fn mint_for_recovered_runner"));
    assert!(!runner_borrow.contains("fn into_parts("));
    let runner_errors = runner_source
        .split_once("pub(super) enum V2RunnerError")
        .expect("runner retains one fail-closed error surface")
        .1;
    for required in [
        "LifecycleOwnerStartup(#[from] super::v2::ProductionLifecycleOwnerStartupErrorV1)",
        "ProductionLifecycleLaunchErrorV1",
        "ProductionLifecycleActivationErrorV1",
        "ProductionLifecycleShutdownErrorV1",
        "ProductionLifecycleFinalizationErrorV1",
    ] {
        assert!(runner_errors.contains(required));
    }
    assert!(runner_tests_source.contains(
        "fn recovered_lifecycle_factory_dependency_permit_retains_exact_signer_and_cadence()"
    ));
    let factory_bind = adapter_source
        .split_once("fn bind_production_lifecycle_owner_factory_inputs_v1(")
        .expect("adapter has one sealed lifecycle factory-input bind")
        .1
        .split_once("/// Consume all recovered adapter and storage authority")
        .expect("factory-input bind remains a bounded source region")
        .0;
    assert!(
        factory_bind
            .contains("permit: super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1")
    );
    assert!(
        factory_bind
            .contains("let (local_signer, block_cadence) = permit.into_factory_dependencies();")
    );
    assert!(!factory_bind.contains("state.sumeragi_block_cadence()"));
    assert!(!source.contains("fn body_store("));
    assert!(!source.contains("fn adapter("));
    assert!(!source.contains("debug_assert!(startup_effects.is_empty())"));
}

#[test]
fn recovered_lifecycle_sign_dispatch_source_is_sealed_and_restart_closed() {
    let scheduler_source = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let registry_source = include_str!("v2_lifecycle_work_registry.rs");
    let coordinator_source = include_str!("v2_lifecycle_coordinator.rs");
    let worker_source = include_str!("v2_worker.rs");
    let launch_source = include_str!("v2_lifecycle_launch.rs");
    let effects_source = include_str!("v2_effects.rs");

    let dispatch = scheduler_source
        .split_once("fn dispatch_recovered_lifecycle_sign_with_runner_debt(")
        .expect("production owner has one recovered Sign dispatch transaction")
        .1
        .split_once(
            "/// Refanout one durable recovered signed Broadcast at the live Completion cursor.",
        )
        .expect("recovered Sign dispatch stays a bounded source region")
        .0;
    let body_owner = dispatch
        .find("let Some(body_store_identity) = self.body_store_identity.as_ref()")
        .expect("dispatch requires its launched body-store identity");
    let service_owner = dispatch
        .find("services.matches_lifecycle_body_store(body_store_identity)")
        .expect("dispatch rejoins the exact launched body store");
    let output_owner = dispatch
        .find("services.matches_lifecycle_executor_output_guard(executor)")
        .expect("dispatch rejoins service and executor output ownership");
    let attest = dispatch
        .find("attest_ready_recovered_lifecycle_sign")
        .expect("dispatch authenticates one current Ready carrier");
    let reserve = dispatch
        .find("capture_recovered_lifecycle_sign_capacity(dispatch_key)")
        .expect("dispatch reserves dedicated capacity before claiming");
    let claim = dispatch
        .find("self.coordinator.plan_turn(inputs)")
        .expect("dispatch claims only after capacity is held");
    let broadcast_reservation = dispatch
        .find("reservation.class() == CapacityClass::Consensus")
        .expect("the claimed Sign retains its mandatory Broadcast reservation");
    let projection = dispatch
        .find("prepare_recovered_lifecycle_sign_dispatch")
        .expect("the claimed carrier projects directly into its opaque task");
    let preflight = dispatch
        .find("reservation.preflight(&prepared)")
        .expect("queue identity is rechecked before publication");
    let publish = dispatch
        .find("reservation.commit(prepared)")
        .expect("the reserved queue cut performs the sole publication");
    assert!(
        body_owner < service_owner
            && service_owner < output_owner
            && output_owner < attest
            && attest < reserve
            && reserve < claim
            && claim < broadcast_reservation
            && broadcast_reservation < projection
            && projection < preflight
            && preflight < publish
    );
    assert_eq!(
        dispatch
            .matches("self.coordinator.rollback_unpublished_turn(&lease)")
            .count(),
        1,
        "the polymorphic unexpected-plan branch retains the unreserved rollback"
    );
    assert_eq!(
        dispatch
            .matches("rollback_unpublished_reserved_turn(&lease")
            .count(),
        3,
        "every reserved post-claim failure must release the coordinator overlay"
    );
    assert_eq!(
        dispatch.matches("reservation.cancel_uncommitted()").count(),
        6,
        "every reserved prepublication failure must release its capacity owner"
    );
    for forbidden in [
        "AdapterEffect",
        "PendingRuntimeEffectBinding",
        "RuntimeEffectOwnership",
        "EffectWorkId",
        "into_parts",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "recovered Sign dispatch exposes forbidden raw authority {forbidden}"
        );
    }

    let phase_carrier = registry_source
        .split_once("impl DurableRecoveredWalSignWork {")
        .expect("PhaseVote carrier has one exactness implementation")
        .1
        .split_once("/// Whether one concrete registry row is still an executable adapter effect")
        .expect("PhaseVote carrier exactness stays a bounded source region")
        .0;
    assert_eq!(
        phase_carrier
            .matches("self.matches_current_terminal_parent(coordinator)")
            .count(),
        2,
        "Ready and Claimed PhaseVote checks must rejoin the current terminal Validate parent"
    );
    assert_eq!(
        phase_carrier
            .matches("metadata.continuation == super::schema::DurableContinuation::None")
            .count(),
        2,
        "Ready and Claimed Sign children must remain standalone durable carriers"
    );
    for required in [
        "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
        "metadata.matches_admission(parent)",
        "super::schema::DurableContinuation::successor(",
        "coordinator.key_index.get(&parent.key)",
        "coordinator.owner_index.get(&parent.causal_root)",
    ] {
        assert!(
            phase_carrier.contains(required),
            "PhaseVote parent rejoin omitted {required}"
        );
    }

    let identity = registry_source
        .split_once("impl RecoveredLifecycleSignDispatchIdentityV1 {")
        .expect("recovered Sign identity has one sealed implementation")
        .1
        .split_once("/// Read-only coordinates of one exact Waiting Fetch incumbent.")
        .expect("recovered Sign identity stays a bounded source region")
        .0;
    assert!(identity.contains("&AdapterEffect::Sign {"));
    assert!(identity.contains("request: request.clone()"));
    assert!(!identity.contains("tag.view() =="));
    assert!(!identity.contains("vote.round.view"));

    let task = worker_source
        .split_once("pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {")
        .expect("worker has one opaque recovered Sign task")
        .1
        .split_once("enum V2IoCommand {")
        .expect("recovered Sign task/result stay a bounded source region")
        .0;
    for required in [
        "identity: RecoveredLifecycleSignDispatchIdentityV1",
        "prepared_candidate: Option<PreparedCandidateBody>",
        "self.task.prepared_candidate == expected_prepared",
        "outbound_payload: Option<EncodedV2Payload>",
        "authorizes_request(self.task.tag, &self.task.request)",
    ] {
        assert!(
            task.contains(required),
            "opaque Sign task omitted {required}"
        );
    }
    for forbidden in [
        "pub tag:",
        "pub request:",
        "pub signature:",
        "pub outbound_payload:",
        "fn into_parts(",
        "fn into_result(",
        "fn into_task(",
        "fn request(",
        "fn prepared_candidate(",
        "fn result(",
        "fn acknowledgement(",
        "fn acknowledge(",
        "fn signature(",
        "fn outbound_payload(",
    ] {
        assert!(
            !task.contains(forbidden),
            "opaque Sign task/result expose forbidden surface {forbidden}"
        );
    }
    let parked_completion = worker_source
        .split_once("pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {")
        .expect("worker has one opaque parked recovered Sign completion")
        .1
        .split_once("/// Result of atomically returning one guarded missing-sidecar Apply")
        .expect("parked recovered Sign completion stays a bounded source region")
        .0;
    for forbidden in [
        "fn into_parts(",
        "fn into_result(",
        "fn into_task(",
        "fn request(",
        "fn prepared_candidate(",
        "fn result(",
        "fn acknowledgement(",
        "fn acknowledge(",
        "fn signature(",
        "fn outbound_payload(",
        "fn settle(",
    ] {
        assert!(
            !parked_completion.contains(forbidden),
            "parked recovered Sign completion exposes forbidden surface {forbidden}"
        );
    }
    let signer = worker_source
        .split_once("fn sign_recovered_lifecycle_task(")
        .expect("worker has one fixed recovered Sign implementation")
        .1
        .split_once("fn recover_outbound_proposal_payload(")
        .expect("fixed recovered Sign stays a bounded source region")
        .0;
    assert!(!signer.contains("prepared_candidates"));
    assert!(!signer.contains("register_outbound_payload"));
    let capacity = worker_source
        .split_once("fn capture_recovered_lifecycle_sign_capacity<'a>(")
        .expect("worker has one dedicated recovered Sign capacity capture")
        .1
        .split_once("fn begin_decision_serve_reconciliation(")
        .expect("recovered Sign capacity capture stays a bounded source region")
        .0;
    assert_eq!(capacity.matches("operation.complete()").count(), 5);
    assert!(!capacity.contains("drop(operation)"));

    let rollback = coordinator_source
        .split_once("fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {")
        .expect("coordinator has one unpublished-claim rollback")
        .1
        .split_once("/// Rebuild records after seeding the ordinal high-water mark.")
        .expect("unpublished rollback stays a bounded source region")
        .0;
    assert!(rollback.contains("lease.output_reservation.is_some()"));
    assert!(rollback.contains("assert!(\n            inserted,"));
    assert!(!rollback.contains("debug_assert!"));

    for regression in [
        "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
        "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
        "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
        "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
    ] {
        assert!(
            worker_source.contains(regression) || coordinator_source.contains(regression),
            "recovered Sign prerequisite omitted behavior regression {regression}"
        );
    }
    assert!(effects_source.contains("owner.dispatch_recovered_lifecycle_sign("));
    assert!(effects_source.contains(
        "Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)"
    ));
    assert!(
        effects_source.contains(
            "a non-Completion runner cursor cannot claim or mutate a recovered Sign owner"
        )
    );

    let settlement = launch_source
        .split_once("pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(")
        .expect("recovered Sign has one durable Broadcast settlement")
        .1
        .split_once("/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.")
        .expect("recovered Sign settlement stays a bounded source region")
        .0;
    let completion = settlement
        .find("recovered_lifecycle_sign_completion.take()")
        .expect("settlement takes the guarded completion once");
    let preview = settlement
        .find("prepare_recovered_lifecycle_sign_completion(authority)")
        .expect("settlement previews the exact signed reducer successor");
    let registry = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_successor(")
        .expect("settlement seals the exact registry child");
    let transition = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_transition(")
        .expect("settlement stages one exact LedgerV1 successor");
    let operation = settlement
        .find("output_guard.begin_fail_stop_operation()")
        .expect("settlement arms the shared output guard before fsync");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("settlement fsyncs the exact successor");
    let coordinator_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("coordinator, registry, and adapter commit after fsync");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("the worker owner retires last");
    let operation_commit = settlement
        .find("operation.complete();")
        .expect("the fail-stop operation completes after every owner commit");
    assert!(
        completion < preview
            && preview < registry
            && registry < transition
            && transition < operation
            && operation < fsync
            && fsync < coordinator_commit
            && coordinator_commit < worker_commit
            && worker_commit < operation_commit
    );
    assert!(!settlement.contains("capture_recovered_lifecycle_signed_broadcast_refanout"));
    assert!(!settlement.contains("commit_after_publication();\n        output"));
    let tail = &settlement[coordinator_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains(".is_err()"));

    let refanout = scheduler_source
        .split_once("fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(")
        .expect("durable Broadcast has one typed refanout transaction")
        .1
        .split_once("/// Sign, reserve, claim, and publish the sole recovered Decision Fetch")
        .expect("durable Broadcast refanout stays a bounded source region")
        .0;
    let census = refanout
        .find("if exact_ready != self.coordinator.ready_index")
        .expect("refanout authenticates the complete Ready census");
    let target = refanout
        .find("work_class == LifecycleWorkClass::Broadcast")
        .expect("refanout selects one Broadcast without requiring a two-row census");
    let retained_pair = refanout
        .find("recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal")
        .expect("pair recognition comes from the Broadcast carrier's retained child seal");
    let attest = refanout
        .find("attest_ready_recovered_lifecycle_signed_broadcast")
        .expect("refanout authenticates the durable Ready carrier");
    let full_rows = refanout
        .find("for ready_ordinal in &exact_ready")
        .expect("all unrelated Ready work remains in scheduler ranking");
    let ordinary_sign = refanout
        .find("attest_ready_recovered_lifecycle_sign(")
        .expect("an unrelated adjacent Sign uses its ordinary carrier attestation");
    let claim = refanout
        .find("self.coordinator.plan_turn(inputs)")
        .expect("refanout claims through the lifecycle scheduler");
    let projection = refanout
        .find("project_claimed_recovered_lifecycle_signed_broadcast_output")
        .expect("refanout rechecks the claimed durable carrier");
    let capture = refanout
        .find("capture_recovered_lifecycle_signed_broadcast_refanout(authority)")
        .expect("refanout reserves the exact network corridor");
    let wait = refanout
        .find("settle_turn(lease, super::TurnOutcome::Blocked(wait))")
        .expect("successful refanout parks only volatile scheduler state");
    let commit = refanout
        .find("output.commit_after_publication()")
        .expect("fanout commits only after the durable row is parked");
    assert!(
        census < target
            && target < retained_pair
            && retained_pair < attest
            && attest < full_rows
            && full_rows < ordinary_sign
            && ordinary_sign < claim
            && claim < projection
            && projection < capture
            && capture < wait
    );
    assert!(wait < commit);
    assert!(refanout.contains("rollback_unpublished_turn(&lease)"));
    assert!(refanout.contains("attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote("));
    assert!(!refanout.contains("exact_ready.len() == 2"));
    assert!(!refanout.contains("exact_ready.len() != 2"));
    assert!(!refanout.contains("persist_exact_successor"));
    assert!(!refanout.contains("TurnOutcome::Terminal"));

    let launched = launch_source
        .split_once("pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {")
        .expect("launched stack has one retained-owner declaration")
        .1
        .split_once("\n}")
        .expect("launched stack declaration is closed")
        .0;
    let services = launched.find("services: ProductionV2Services").unwrap();
    let completion = launched
            .find(
                "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
            )
            .unwrap();
    let ingress = launched
        .find("leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1")
        .unwrap();
    assert!(services < completion && completion < ingress);
    assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed();
    assert_recovered_proposal_prepare_wal_settlement_is_restart_closed();
    assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed();
}

fn assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source
        .split_once(
            "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
        )
        .expect("recovered Prepare Vote has one combined settlement")
        .1
        .split_once("/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.")
        .expect("combined Vote settlement stays bounded")
        .0;
    let completion = settlement
        .find("recovered_lifecycle_sign_completion.take()")
        .expect("take the guarded worker completion once");
    let body = settlement
        .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
        .expect("join the exact launched body owner");
    let mode = settlement
        .find("preview.is_vote_broadcast_and_sign_shape()")
        .expect("accept only Prepare-Broadcast then Commit-Sign");
    let registry = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
        .expect("seal the exact two-child registry successor");
    let transition = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
        .expect("stage the exact two-child Ledger successor");
    let operation = settlement
        .find("output_guard.begin_fail_stop_operation()")
        .expect("arm fail-stop output before fsync");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("fsync the two-child successor once");
    let transition_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("publish coordinator, registry, and adapter after fsync");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("retire the guarded worker after publication");
    let operation_commit = settlement
        .find("operation.complete();")
        .expect("complete fail-stop ownership last");
    assert!(
        completion < body
            && body < mode
            && mode < registry
            && registry < transition
            && transition < operation
            && operation < fsync
            && fsync < transition_commit
            && transition_commit < worker_commit
            && worker_commit < operation_commit
    );
    assert!(!settlement.contains("project_proposal_exact_output_authority"));
    assert!(!settlement.contains("capture_recovered_lifecycle_proposal_exact_output"));
    assert!(!settlement.contains("output.commit_after_publication()"));
    let tail = &settlement[transition_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains(".is_err()"));
    assert!(!tail.contains('?'));
}

fn assert_recovered_proposal_prepare_wal_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source
        .split_once("pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(")
        .expect("initial recovered Proposal has one WAL-first transaction")
        .1
        .split_once("/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.")
        .expect("initial Proposal WAL transaction stays bounded")
        .0;
    let completion = settlement
        .find("recovered_lifecycle_sign_completion.take()")
        .expect("take the guarded Proposal completion once");
    let body = settlement
        .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
        .expect("preflight the exact future Prepare Sign and body");
    let shape = settlement
        .find("RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal")
        .expect("accept only the initial Proposal persistence shape");
    let output_projection = settlement
        .find("preview.project_proposal_exact_output_authority()")
        .expect("seal output from the same pre-WAL preview");
    let output_capture = settlement
        .find("capture_recovered_lifecycle_proposal_exact_output(output_authority)")
        .expect("reserve Proposal control and chunks before WAL I/O");
    let wal_permit = settlement
        .find("output.prepare_wal_append_permit()")
        .expect("borrow the WAL authority from the still-armed output reservation");
    let wal = settlement
        .find("append_recovered_lifecycle_proposal_prepare_wal(wal_permit)")
        .expect("append and fsync the exact PrepareIntent");
    let registry = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
        .expect("seal the post-WAL two-child registry successor");
    let transition = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
        .expect("stage the exact two-child Ledger successor");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("fsync the two-child Ledger successor");
    let transition_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("publish coordinator, registry, and adapter after Ledger fsync");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("retire the guarded worker after durable publication");
    let output_commit = settlement
        .find("output.commit_after_publication();")
        .expect("enqueue the pre-WAL output reservation last");
    assert!(
        completion < body
            && body < shape
            && shape < output_projection
            && output_projection < output_capture
            && output_capture < wal_permit
            && wal_permit < wal
            && wal < registry
            && registry < transition
            && transition < fsync
            && fsync < transition_commit
            && transition_commit < worker_commit
            && worker_commit < output_commit
    );
    assert!(
        settlement
            .contains("RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)")
    );
    assert!(settlement.contains("*recovered_lifecycle_sign_completion = Some(completion)"));
    assert!(!settlement.contains("output.abort_before_publication()"));
    let post_wal = &settlement[wal..transition_commit];
    assert!(post_wal.matches("drop(output);").count() >= 3);
    let tail = &settlement[transition_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains(".is_err()"));
    assert!(!tail.contains('?'));
}

fn assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source
        .split_once(
            "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
        )
        .expect("recovered Proposal has one combined settlement")
        .1
        .split_once("/// Refanout one durable recovered signed Broadcast")
        .expect("combined Proposal settlement stays bounded")
        .0;
    let completion = settlement
        .find("recovered_lifecycle_sign_completion.take()")
        .expect("take the guarded worker completion once");
    let body = settlement
        .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
        .expect("join the exact launched body owner");
    let output_projection = settlement
        .find("preview.project_proposal_exact_output_authority()")
        .expect("project output only from the same adapter preview");
    let output_capture = settlement
        .find("capture_recovered_lifecycle_proposal_exact_output(output_authority)")
        .expect("reserve Proposal control and chunks atomically");
    let registry = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
        .expect("seal the exact two-child registry successor");
    let transition = settlement
        .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
        .expect("stage the exact two-child Ledger successor");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("fsync the two-child successor once");
    let transition_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("publish coordinator, registry, and adapter after fsync");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("retire the guarded worker after publication");
    let output_commit = settlement
        .find("output.commit_after_publication();")
        .expect("enqueue the reserved atomic batch last");
    assert!(
        completion < body
            && body < output_projection
            && output_projection < output_capture
            && output_capture < registry
            && registry < transition
            && transition < fsync
            && fsync < transition_commit
            && transition_commit < worker_commit
            && worker_commit < output_commit
    );
    assert_eq!(
        settlement
            .matches("output.abort_before_publication()")
            .count(),
        2,
        "every fallible post-reservation pre-fsync branch must release the batch"
    );
    assert!(
        settlement
            .contains("RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)")
    );
    assert!(settlement.contains("*recovered_lifecycle_sign_completion = Some(completion)"));
    assert!(settlement.contains("drop(output);"));
    let tail = &settlement[transition_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains(".is_err()"));
    assert!(!tail.contains("?"));
}

#[test]
fn recovered_decision_fetch_dispatch_reserves_capacity_before_claim_and_failures_leave_no_mutation()
{
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let dispatch = scheduler
        .split_once("fn dispatch_recovered_decision_fetch_with_runner_debt(")
        .expect("recovered Fetch has one request-dispatch transaction")
        .1
        .split_once("/// Persist one selected recovered Decision Fetch response")
        .expect("request dispatch stays a bounded source region")
        .0;
    let output = dispatch
        .find("capture_recovered_decision_fetch_exact_output(&owner)")
        .expect("exact output is captured");
    let executor = dispatch
        .find("prepare_recovered_decision_fetch_request_registration(owner)")
        .expect("executor vacancy is reserved");
    let claim = dispatch
        .find("self.coordinator.plan_turn(inputs)")
        .expect("coordinator claim exists");
    let commit = dispatch
        .find("registration.commit(prepared)")
        .expect("request owner has one commit tail");
    assert!(output < executor && executor < claim && claim < commit);
    assert!(dispatch.contains("output.abort_before_claim();"));
    assert!(dispatch.contains("rollback_unpublished_turn(&lease)"));
}

#[test]
fn recovered_decision_fetch_queue_parks_generic_drain_and_extracts_only_dedicated_completion() {
    let worker = include_str!("v2_worker.rs");
    let generic = worker
        .split_once("fn take_io_completion(")
        .expect("generic completion selector exists")
        .1
        .split_once("fn take_recovered_decision_apply_completion(")
        .expect("generic selector stays bounded")
        .0;
    assert!(generic.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)"));
    assert!(generic.contains("self.held_io_completion = Some(completion);"));
    let dedicated = worker
        .split_once("fn take_recovered_decision_fetch_body_completion(")
        .expect("dedicated recovered Fetch extractor exists")
        .1
        .split_once("fn take_next_completion(")
        .expect("dedicated extractor stays bounded")
        .0;
    assert!(dedicated.contains("RecoveredDecisionFetchBodyPersisted"));
    assert!(worker.contains("tracked.state = V2IoWorkState::Active;"));
    assert!(worker.contains("tracked.state = V2IoWorkState::CompletionPending;"));
    assert!(worker.contains("drain_recovered_decision_fetch_body_completion"));
}

#[test]
fn recovered_decision_fetch_phase_a_rejects_foreign_ingress_cursor_before_mutation() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let wrapper = scheduler
        .split_once("pub(crate) fn persist_recovered_decision_fetch_response(")
        .expect("production Phase-A wrapper exists")
        .1
        .split_once("/// Exercise Phase A with a fixture-owned current Ingress snapshot.")
        .expect("production cursor check stays isolated")
        .0;
    let cursor = wrapper
        .find("runner.target() != LifecycleRunnerRankTarget::Ingress")
        .expect("Phase A requires the Ingress cursor");
    let reject = wrapper
        .find("ForeignRunnerObservation")
        .expect("foreign cursor rejects explicitly");
    let handoff = wrapper
        .find("persist_recovered_decision_fetch_response_after_runner")
        .expect("mutation lives behind cursor validation");
    assert!(cursor < reject && reject < handoff);
    assert!(!wrapper[..handoff].contains("capture_lifecycle_capacity_rank"));
    assert!(!wrapper[..handoff].contains("prepare_recovered_decision_fetch_response_claim"));
}

#[test]
fn recovered_decision_fetch_response_claim_precedes_assertion_only_queue_publication() {
    let effects = include_str!("v2_effects.rs");
    let commit = effects
        .split_once("pub(in crate::sumeragi) fn commit_with_queue(")
        .expect("recovered response has one composite commit")
        .1
        .split_once("impl RecoveredDecisionFetchResponseCandidateV1")
        .expect("composite commit stays bounded")
        .0;
    let claim = commit
        .find("owner.commit_exact_response_claim(response_hash)")
        .expect("exact response claim is installed");
    let queue = commit
        .find("queue.commit_recovered_decision_fetch_body_persistence(task)")
        .expect("dedicated persistence is published");
    assert!(claim < queue);
    assert!(commit.contains("assert!(owner.matches_response_claim_preflight"));
    let worker = include_str!("v2_worker.rs");
    let queue_commit = worker
        .split_once("fn commit_recovered_decision_fetch_body_persistence(")
        .expect("dedicated queue commit exists")
        .1
        .split_once("#[cfg(test)]")
        .expect("queue commit stays bounded")
        .0;
    assert!(queue_commit.contains("assert!("));
    assert!(!queue_commit.contains("return Err"));
}

#[test]
fn recovered_decision_fetch_store_settlement_is_restart_closed_and_tail_infallible() {
    let launch = include_str!("v2_lifecycle_launch.rs");
    let settlement = launch
        .split_once("pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(")
        .expect("recovered Fetch has one Store settlement transaction")
        .1
        .split_once("/// Reserve, claim, and queue one recovered Sign")
        .expect("recovered Fetch Store settlement stays bounded")
        .0;
    let selector = settlement
        .find("prepare_lifecycle_ingress_selector(")
        .expect("fresh selector preflight exists");
    let request = settlement
        .find("prepare_recovered_decision_fetch_owner_retirement(")
        .expect("request/response retirement preflight exists");
    let ingress = settlement
        .find("into_locked_recovered_decision_fetch_dequeue(")
        .expect("exact ingress occurrence is locked");
    let carrier = settlement
        .find("prepare_recovered_decision_fetch_store_adapter_authority(")
        .expect("claimed recovered carrier preflight exists");
    let adapter = settlement
        .find("prepare_recovered_decision_fetch_store_adapter(")
        .expect("fixed reducer preview exists");
    let registry = settlement
        .find("prepare_recovered_decision_fetch_store_successor(")
        .expect("dedicated Store carrier preflight exists");
    let transition = settlement
        .find("prepare_recovered_decision_fetch_store_transition(")
        .expect("Fetch-to-Store coordinator successor is staged");
    let output = settlement
        .find("begin_fail_stop_operation()")
        .expect("output fail-stop cut precedes publication");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("exact LedgerV1 successor is fsynced once");
    let coordinator_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("coordinator/registry/adapter tail exists");
    let request_commit = settlement
        .find("commit_recovered_decision_fetch_owner_retirement(retirement);")
        .expect("dedicated request owner retires after publication");
    let ingress_commit = settlement
        .find("locked_dequeue.commit();")
        .expect("locked ingress occurrence retires after publication");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("worker owner retires and disarms after publication");
    let output_commit = settlement
        .find("operation.complete();")
        .expect("output fail-stop cut closes last");
    assert!(
        selector < request
            && request < ingress
            && ingress < carrier
            && carrier < adapter
            && adapter < registry
            && registry < transition
            && transition < output
            && output < fsync
            && fsync < coordinator_commit
            && coordinator_commit < request_commit
            && request_commit < ingress_commit
            && ingress_commit < worker_commit
            && worker_commit < output_commit
    );
    let tail = &settlement[coordinator_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains("Result<"));
    assert!(!tail.contains(".is_err()"));

    let worker = include_str!("v2_worker.rs");
    let guarded = worker
        .split_once("impl GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {")
        .expect("recovered Fetch completion has one armed guard")
        .1
        .split_once("impl GuardedCertifiedFetchBodyPersistenceCompletion")
        .expect("recovered Fetch guard stays bounded")
        .0;
    assert!(guarded.contains("let _completion = self"));
    assert!(guarded.contains(".take()"));
    assert!(guarded.contains("self.drop_guard.disarm();"));
    let prepared = worker
        .split_once("impl PreparedRecoveredDecisionFetchBodyCompletionV1 {")
        .expect("parked recovered Fetch completion has one consuming acknowledgement")
        .1
        .split_once("impl PreparedRecoveredLifecycleSignCompletionV1")
        .expect("parked recovered Fetch acknowledgement stays bounded")
        .0;
    let index = prepared
        .find("acknowledge_recovered_decision_fetch_body(key, id, response_hash);")
        .expect("exact worker index is removed");
    let disarm = prepared
        .find("self.guarded.acknowledge_after_publication();")
        .expect("restart guard is disarmed after index removal");
    assert!(index < disarm);

    let ledger = include_str!("v2_lifecycle_ledger.rs");
    let open = include_str!("v2_lifecycle_open.rs");
    let registry_source = include_str!("v2_lifecycle_work_registry_validate_recovery.rs");
    for required in [
        "authenticate_recovered_decision_fetch_store",
        "open_recovered_decision_store_startup",
        "stage_recovered_decision_apply_projection",
        "successor_records_after_live_store",
    ] {
        assert!(ledger.contains(required), "cold restart omitted {required}");
    }
    assert!(open.contains("RecoveredWalStartupProjectionV1::DecisionStore"));
    assert!(registry_source.contains("install_recovered_wal_decision_store"));
}
