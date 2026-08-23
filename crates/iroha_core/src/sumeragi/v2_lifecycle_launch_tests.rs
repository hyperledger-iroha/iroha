// Production lifecycle launch, activation, shutdown, and source-seal tests.
use iroha_crypto::{Hash, HashOf};
use tempfile::TempDir;

use super::*;
use crate::BlockMessage;
use crate::sumeragi::v2_lifecycle_coordinator::reviewed_lifecycle_ledger_source_for_test;

fn source_region<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let Some((_, after_start)) = source.split_once(start) else {
        panic!("missing source-region start token `{start}`");
    };
    let Some((region, _)) = after_start.split_once(end) else {
        panic!("missing source-region end token `{end}`");
    };
    region
}

fn source_token_position(source: &str, token: &str) -> usize {
    let Some(position) = source.find(token) else {
        panic!("missing source token `{token}`");
    };
    position
}

fn assert_required_source_tokens(source: &str, tokens: &[&str]) {
    for token in tokens {
        assert!(
            source.contains(*token),
            "missing required source token `{token}`"
        );
    }
}

fn assert_forbidden_source_tokens(source: &str, tokens: &[&str]) {
    for token in tokens {
        assert!(
            !source.contains(*token),
            "found forbidden source token `{token}`"
        );
    }
}

fn assert_source_tokens_in_order(source: &str, tokens: &[&str]) {
    let mut previous = None;
    for token in tokens {
        let position = source_token_position(source, token);
        if let Some((previous_position, previous_token)) = previous {
            assert!(
                previous_position < position,
                "source token `{previous_token}` must precede `{token}`"
            );
        }
        previous = Some((position, *token));
    }
}

fn assert_source_token_count(source: &str, token: &str, expected: usize) {
    assert_eq!(
        source.matches(token).count(),
        expected,
        "unexpected count for source token `{token}`"
    );
}

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
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;

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
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        context_id,
        HEIGHT,
    )
    .expect("bind the exact launch gate");
    assert!(
        ingress
            .state
            .lock()
            .leader_wire_lifecycle_gate
            .as_ref()
            .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &first_gate))
    );
    binding
        .retire()
        .expect("explicit retirement detaches the exact launch gate");
    binding
        .retire()
        .expect("explicit retirement remains idempotent");
    {
        let state = ingress.state.lock();
        assert!(state.leader_wire_lifecycle_gate.is_none());
    }

    let (drop_gate, drop_restore) = empty_leader_wire_gate_for_binding_test(
        &directory, "drop.wal", context_id, HEIGHT, &validator,
    );
    let binding = ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        Arc::clone(&drop_gate),
        drop_restore,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        context_id,
        HEIGHT,
    )
    .expect("rebind the exact launch gate");
    drop(binding);
    {
        let state = ingress.state.lock();
        assert!(
            state.leader_wire_lifecycle_gate.is_none(),
            "Drop must detach the exact launch gate"
        );
    }

    let (incumbent_gate, incumbent_restore) = empty_leader_wire_gate_for_binding_test(
        &directory,
        "incumbent.wal",
        context_id,
        HEIGHT,
        &validator,
    );
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&incumbent_gate),
            incumbent_restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            HEIGHT,
        )
        .expect("bind the incumbent gate");
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
        .retire_leader_wire_lifecycle_gate(&incumbent_gate)
        .expect("clean up the incumbent binding");
}

#[test]
fn production_leader_wire_binding_parks_queued_carriers_atomically_before_unbind() {
    const HEIGHT: wire::Height = 9;
    let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"production-leader-wire-queued-retirement",
    )));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let directory = TempDir::new().expect("temporary queued-retirement directory");
    let wal_path = directory.path().join("queued-retirement.wal");
    let ingress = Arc::new(FairV2Ingress::new(16, 4 << 20, 1 << 20, 1 << 18, 1 << 18));
    ingress
        .configure_roster([validator.clone()])
        .expect("one-validator queued-retirement geometry");
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;

    let owner = [0xB7; 32];
    let capacity = LeaderWireLifecycleStoreGate::derived_capacity(1, 2)
        .expect("finite queued-retirement leader-wire capacity");
    let recovery_authority =
        crate::sumeragi::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context_id,
            HEIGHT,
            owner,
            0,
            false,
        );
    let (gate, restore) = LeaderWireLifecycleStoreGate::open(
        &wal_path,
        context_id,
        HEIGHT,
        owner,
        [validator.clone()].into_iter().collect(),
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open queued-retirement leader-wire gate");
    let mut binding = ProductionLeaderWireIngressBindingV1::bind(
        Arc::clone(&ingress),
        Arc::clone(&gate),
        restore,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        context_id,
        HEIGHT,
    )
    .expect("bind queued-retirement leader-wire gate");
    ingress.open().expect("open queued-retirement ingress");

    let timeout_vote = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(
        wire::TimeoutVote {
            round: wire::ConsensusRound {
                context_id,
                height: HEIGHT,
                view: 0,
            },
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0x5A],
        },
    ));
    ingress
        .try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(timeout_vote),
            validator.clone(),
        ))
        .expect("queue one durable productive carrier");
    let unbound_chunk = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"queued-retirement-unbound-manifest",
            )),
            index: 0,
            bytes: vec![0xA5],
            sender: 0,
            signature: vec![0xC3],
        }),
    );
    ingress
        .try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(unbound_chunk),
            validator.clone(),
        ))
        .expect("queue one proofless producer carrier");
    assert_eq!(ingress.snapshot_at(Instant::now()).depth, 2);

    binding
        .retire()
        .expect("retire every queued carrier before unbinding");
    assert_eq!(ingress.snapshot_at(Instant::now()).depth, 0);
    assert!(ingress.state.lock().leader_wire_lifecycle_gate.is_none());
    let retained = gate
        .restore()
        .expect("read queued-retirement durable projection");
    assert_eq!(retained.records().len(), 1);
    assert_eq!(
        retained.records()[0].status(),
        crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    assert!(retained.records()[0].runtime_owner().is_none());

    drop(binding);
    drop(gate);
    let recovery_authority =
        crate::sumeragi::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context_id,
            HEIGHT,
            owner,
            0,
            false,
        );
    let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
        &wal_path,
        context_id,
        HEIGHT,
        owner,
        [validator].into_iter().collect(),
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("reopen queued-retirement leader-wire gate");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(
        restore.records()[0].status(),
        crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant,
        "atomic ingress retirement must reopen under the exact durable retry owner"
    );
    assert!(restore.records()[0].runtime_owner().is_none());
    assert_eq!(
        reopened
            .earliest_ingress_scheduler_ordinal()
            .expect("read replay-dormant queued-retirement selector"),
        None
    );
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
    let worker_source = concat!(
        include_str!("v2_worker.rs"),
        include_str!("v2_worker_services_impl.rs"),
        include_str!("v2_worker/effect_services_impl.rs")
    );
    let effects_source = include_str!("v2_effects.rs");
    let runtime_source =
        crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_runtime_source_for_test();
    let runner_source = include_str!("v2_runner.rs");
    let lifecycle_run_inner_source = include_str!("v2_runner/lifecycle_run_inner.rs");
    let lifecycle_scheduler_source = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let pending_kura_lifecycle_source = include_str!("v2_runner/lifecycle_pending_kura.rs");
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
    let bound_launch = source_region(
        ledger_source,
        "// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_BEGIN",
        "// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_END",
    );

    assert!(bound_launch.contains(
            "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n    launched: Box<super::launch::LaunchedProductionLifecycleV1>,\n    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,\n}"
        ));
    assert_source_tokens_in_order(
        bound_launch,
        &[
            "impl BoundRecoveredCompleteTipSuccessorOwnerV1",
            "pub(in crate::sumeragi) fn launch(",
            "let Self {\n            owner,\n            mut retirement,\n        } = self;",
            "let mut launched = owner.launch(inputs)?;",
            "launched\n            .reauthenticate_recovered_complete_tip_successor(&mut retirement)",
            "LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n            launched,\n            retirement,",
            "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
            "impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
            "self.launched.with_runner_setup(runner, operation)",
            "let Self {\n            launched,\n            retirement,\n        } = self;",
            "launched.activate_recovered_complete_tip(now, runner, retirement, local_proposal)",
        ],
    );
    assert_forbidden_source_tokens(
        bound_launch,
        &[
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
        ],
    );
    assert_source_token_count(bound_launch, "owner.launch(inputs)?", 1);
    assert_source_token_count(
        bound_launch,
        "reauthenticate_recovered_complete_tip_successor(&mut retirement)",
        1,
    );

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
    let inputs = source_region(
        &source,
        "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
        "\n}",
    );
    assert_forbidden_source_tokens(
        inputs,
        &[
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
        ],
    );

    let launch = source_region(
        &source,
        "pub(in crate::sumeragi) fn launch(",
        "\n}\n\n#[cfg(test)]",
    );
    assert!(!source.contains("publish_status("));
    assert!(!launch.contains("set_v2_effect_completion_observer"));
    assert_source_tokens_in_order(
        launch,
        &[
            "begin_fail_stop_operation()",
            "if self.body_store.is_none()",
            "Self::launch_local_identity_matches(",
            "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
            "service.matches_lifecycle_launch(",
            "self.exact_lifecycle_output_ordinals_for_registry_census()",
            "exactly_covers_recovered_ready_work_with_owner_held_outputs(",
        ],
    );
    assert!(!launch.contains("reply_route_source_capacity()"));
    assert!(!launch.contains("RuntimeLifecycleOrdinalSource::after_high_watermark(0)"));
    assert_source_tokens_in_order(
        launch,
        &[
            "service.matches_lifecycle_launch(",
            "binding.storage_paths_for_launch(inputs.kura.as_ref())",
            ".prepare_leader_wire_launch(launch_storage.wal_path())",
            "super::authority::lifecycle_ordinal_authorities_after_high_watermark(",
            "self.coordinator.high_water()",
            "RuntimeLifecycleOrdinalSource::from_authority(runtime_ordinal_authority)",
            "leader_wire_launch.restored_producer_ordinal_high_watermark()",
            ".open_gate(",
            "self.body_store\n                        .as_ref()",
            "leader_wire_restore.scheduler_ordinal_high_watermark()",
            ".bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)",
            "ProductionLeaderWireIngressBindingV1::bind(",
            ".body_store\n            .take()",
            ".into_serialized_runtime(",
        ],
    );
    assert_source_tokens_in_order(
        launch,
        &[
            ".open_gate(",
            "leader_wire_restore.scheduler_ordinal_high_watermark()",
        ],
    );
    let take = source_token_position(launch, ".body_store\n            .take()");
    let take_apply = source_token_position(launch, ".apply_service\n            .take()");
    assert_eq!(
        launch.matches("inputs.auxiliary_io_capacity,").count(),
        1,
        "launch must pass the retained auxiliary capacity exactly once into service startup"
    );
    let identity = launch
        .rfind("self.body_store_identity = Some(body_store_identity)")
        .unwrap();
    let complete = launch.rfind("construction.complete()").unwrap();
    assert!(take <= take_apply);
    let gate_open = source_region(
        &adapter_source,
        "pub(in crate::sumeragi) fn open_gate(",
        "impl ProductionLifecycleAdapterStartupV1",
    );
    assert_required_source_tokens(
        gate_open,
        &[
            "body_store: &super::v2_body_store::V2BodyStore",
            "body_store.matches_context(context)",
            "body_store\n            .recovery_catalog()",
            ".map(|(_, receipt)| receipt)",
            "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(\n            self.storage,",
        ],
    );
    assert!(!gate_open.contains("durable_bodies: &[DurableBodyReceipt]"));
    let adapter_launch = source_region(
        &adapter_source,
        "pub(in crate::sumeragi) fn prepare_leader_wire_launch(",
        "/// Consume the sealed adapter startup directly",
    );
    assert_required_source_tokens(
        adapter_launch,
        &[
            "&mut self",
            "!*leader_wire_launch_prepared",
            "adapter.wal.matches_path(expected_wal_path)",
            "adapter\n                    .mint_leader_wire_store_authority(expected_wal_path)",
            "*leader_wire_launch_prepared = true",
        ],
    );
    let runtime_conversion = source_region(
        &adapter_source,
        "pub(in crate::sumeragi) fn into_serialized_runtime(",
        "#[cfg_attr(not(test), allow(dead_code))]\nimpl PreparedRecoveredPendingKuraApplyReplayV1",
    );
    assert!(runtime_conversion.contains("leader_wire_launch_prepared: true"));
    let adapter_open = source_region(
        &adapter_source,
        "fn open_with_aggregator_and_publication_with_capacity(",
        "/// Return the tag which must accompany a new asynchronous operation",
    );
    assert_source_tokens_in_order(
        adapter_open,
        &[
            "let (wal_path, wal) = match wal_target",
            "SafetyWal::open_with_kura_authority(",
            "SafetyWalOpenTarget::FixturePath(wal_path)",
            "wal.mint_serviced_candidate_store_authority(&wal_path)?",
            "ServicedCandidateStore::open_with_safety_wal_authority(",
            "let entries = wal\n            .recovered_records()",
        ],
    );

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
    assert_source_tokens_in_order(
        lifecycle_run_inner_source,
        &[
            ".mint_safety_wal_directory_authority()",
            "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
        ],
    );
    assert!(lifecycle_run_inner_source.contains("kura.as_ref(),\n            wal_authority,"));
    assert_source_tokens_in_order(
        launch,
        &[
            ".body_store\n            .take()",
            "V2EffectExecutor::open_with_body_store(",
            "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
            "executor\n                .install_authenticated_genesis_body(authenticated_genesis)",
            ".recovered_published_validate_retry_markers()",
            ".install_recovered_published_lifecycle_validate_retry_marker(",
            "ProductionV2Services::start_with_apply_service(",
        ],
    );
    let worker = source_token_position(launch, "ProductionV2Services::start_with_apply_service(");
    assert!(worker < identity && identity < complete);
    assert_source_tokens_in_order(
        launch,
        &[
            ".apply_service\n            .take()",
            "ProductionV2Services::start_with_apply_service(",
            "super::ProductionLifecycleApplyServiceLaunchPermitV1 {",
        ],
    );
    assert!(!launch.contains("inputs.block_cadence"));
    assert!(!launch.contains("genesis_account_for_launch"));
    assert!(launch.contains(
            "completion_observer_activation: Some(\n                ProductionV2CompletionObserverActivationPermitV1"
        ));
    assert!(launch.contains("leader_wire_ingress_binding,"));
    assert!(source.contains("impl Drop for ProductionLeaderWireIngressBindingV1"));
    let launched_fields = source_region(
        &source,
        "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
        "\n}",
    );
    assert_source_tokens_in_order(
        launched_fields,
        &[
            "services: ProductionV2Services",
            "pending_kura_apply_replay:",
            "recovered_local_proposal_attempt:",
            "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>",
            "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
        ],
    );
    assert!(!launched_fields.contains("recovered_lifecycle_sign_completion:"));
    let leader_wire_drop = source_region(
        &source,
        "impl ProductionLeaderWireIngressBindingV1 {",
        "impl Drop for ProductionLeaderWireIngressBindingV1",
    );
    assert_source_tokens_in_order(
        leader_wire_drop,
        &[
            "let Some(gate) = self.gate.as_ref().cloned()",
            "self.ingress.retire_leader_wire_lifecycle_gate(&gate)?",
            "self.gate = None",
        ],
    );
    assert!(!leader_wire_drop.contains("self.gate.take()"));
    let leader_wire_drop_impl = source_region(
        &source,
        "impl Drop for ProductionLeaderWireIngressBindingV1",
        "/// Opaque running stack produced by the sole consuming lifecycle launch.",
    );
    assert!(leader_wire_drop_impl.contains("self.retire()"));
    assert!(source.contains("impl Drop for ProductionV2CompletionObserverActivationPermitSealV1"));
    let worker_start = source_region(
        worker_source,
        "pub(crate) fn start(",
        "/// Sign and retain all canonical chunks",
    );
    let legacy_start = worker_start
        .split_once("pub(in crate::sumeragi) fn start_with_apply_service(")
        .expect("legacy construction ends before the sealed transfer seam")
        .0;
    assert!(legacy_start.contains("let apply_service = V2ApplyService::new("));
    assert!(legacy_start.contains("Self::start_inner("));
    assert!(!legacy_start.contains("Self::start_with_apply_service("));
    let transferred_start = source_region(
        worker_start,
        "pub(in crate::sumeragi) fn start_with_apply_service(",
        "fn start_inner(",
    );
    assert!(transferred_start.contains(
        "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1"
    ));
    assert_source_tokens_in_order(
        transferred_start,
        &[
            "apply_service.matches_lifecycle_launch(",
            "Self::start_inner(",
        ],
    );
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
    let status_publication = source_region(
        worker_source,
        "fn publish_effect_status(",
        "fn fail_closed(",
    );
    assert!(!worker_start.contains("set_v2_effect_completion_observer"));
    assert!(!worker_start.contains("activate_effect_completion_observer"));
    assert!(!worker_start.contains("publish_effect_status"));
    assert!(!status_publication.contains("set_v2_effect_completion_observer"));
    let observer_activation = source_region(
        worker_source,
        "fn activate_effect_completion_observer(",
        "pub(crate) fn capture_lifecycle_capacity_rank<'a>(",
    );
    assert!(observer_activation.contains("ProductionV2CompletionObserverActivationPermitV1"));
    assert_source_tokens_in_order(
        observer_activation,
        &[
            "begin_fail_stop_operation()",
            ".io\n            .as_ref()",
            "set_v2_effect_completion_observer",
            "activation.complete()",
        ],
    );
    assert_source_token_count(worker_source, "set_v2_effect_completion_observer(", 1);
    assert!(!worker_source.contains("ProductionV2CompletionObserverActivationPermitV1 {"));
    assert!(!launch.contains("activate_effect_completion_observer("));
    assert!(!runner_source.contains("activate_effect_completion_observer("));

    let preactivation_runner = source_region(
        runner_source,
        "pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1",
        "/// Exact reducer facts which own one local proposal-side work item",
    );
    assert_required_source_tokens(
        preactivation_runner,
        &[
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
        ],
    );
    assert_forbidden_source_tokens(
        preactivation_runner,
        &[
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
        ],
    );
    let local_proposal_owner = source_region(
        runner_source,
        "pub(in crate::sumeragi) struct ProductionLifecycleLocalProposalStateV1",
        "/// Run the v2-only worker until shutdown",
    );
    assert_required_source_tokens(
        local_proposal_owner,
        &["state: LocalProposalState", "fn fresh() -> Self"],
    );
    assert_forbidden_source_tokens(local_proposal_owner, &["pub state:", "fn into_parts("]);
    let prepared_local_proposal = source_region(
        &source,
        "struct ProductionLifecyclePreparedLocalProposalStateV1",
        "/// Opaque lifecycle stack after clocks",
    );
    assert_required_source_tokens(
        prepared_local_proposal,
        &[
            "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
            "context_id: wire::HeightContextId",
            "directive: super::super::v2::LocalProposalDirective",
            "fn exactly_matches(",
            "self.context_id == context_id",
            "self.directive == directive",
            "prepared_local_proposal_exactly_matches(directive)",
        ],
    );
    assert_forbidden_source_tokens(
        prepared_local_proposal,
        &[
            "derive(Clone)",
            "derive(Copy)",
            "pub runner:",
            "pub context_id:",
            "pub directive:",
            "fn into_parts(",
        ],
    );
    assert!(runtime_source.contains(
            "pub(in crate::sumeragi) const fn lifecycle_live_clocks_are_armed(&self) -> bool {\n        self.clocks_armed\n    }"
        ));
    assert!(effects_source.contains(
            "pub(in crate::sumeragi) fn lifecycle_live_clocks_are_unarmed(&self) -> bool {\n        !self.runtime.lifecycle_live_clocks_are_armed()\n    }"
        ));
    let preactivation_fail_stop = source_region(
        &source,
        "struct ProductionLifecyclePreActivationFailStopScopeV1",
        "impl LaunchedProductionLifecycleV1",
    );
    assert_required_source_tokens(
        preactivation_fail_stop,
        &[
            "output_guard: Arc<ConsensusOutputGuard>",
            "armed: bool",
            "impl Drop for ProductionLifecyclePreActivationFailStopScopeV1",
            "self.output_guard.close_admission_for_restart()",
        ],
    );
    assert!(!preactivation_fail_stop.contains("ConsensusFailStopOperation"));
    let preactivation_setup = source_region(
        &source,
        "fn with_runner_setup_transaction<R, E>(",
        "fn with_canonical_body_recovery_ingress_transaction<R, E, Activation>(",
    );
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
    assert_forbidden_source_tokens(
        preactivation_setup,
        &[
            "&mut self.owner",
            "ProductionLifecyclePreActivationRunnerBorrowV1",
            "bind_recovered_local_proposal",
            "arm_live_clocks(",
            "activate_effect_completion_observer(",
            "open_and_publish(",
            "into_parts(",
        ],
    );
    let public_setup = source_region(
        &source,
        "pub(in crate::sumeragi) fn with_runner_setup<R, E>(",
        "/// Join one recovered local Proposal attempt",
    );
    assert!(public_setup.contains("self.with_runner_setup_transaction(operation)"));
    assert!(!public_setup.contains("bind_recovered_local_proposal"));
    assert!(!public_setup.contains("operation(&mut self.executor"));

    let proposal_initialization = source_region(
        &source,
        "pub(in crate::sumeragi) fn initialize_recovered_local_proposal(",
        "/// Install one opaque recovered-attempt fixture",
    );
    assert_source_tokens_in_order(
        proposal_initialization,
        &[
            "self.recovered_local_proposal_attempt.take()",
            "self.with_runner_setup_transaction(",
            ".local_proposal_directive()",
            "recovered.exactly_matches_directive(directive)",
            "runner.bind_recovered_local_proposal(directive)",
            "ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine",
            "ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch",
            "ProductionLifecyclePreparedLocalProposalStateV1 {",
            "Ok((directive, prepared))",
        ],
    );
    assert_eq!(
        proposal_initialization
            .matches("runner.bind_recovered_local_proposal(directive)")
            .count(),
        1,
        "only the WAL-authenticated initializer may bind runner Proposal state"
    );
    assert_forbidden_source_tokens(
        proposal_initialization,
        &["fn into_parts(", "fn tag(", "fn round(", "fn subject("],
    );

    let activation_blocker = source_region(
        &source,
        "fn lifecycle_activation_recovery_blocker(",
        "/// Fail-stop failure while consuming an activated height",
    );
    assert_source_tokens_in_order(
        activation_blocker,
        &[
            "pending_kura_replay || pending_kura_evidence",
            "ProductionLifecycleActivationErrorV1::PendingKuraApply",
            "else if recovered_local_proposal",
            "ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized",
        ],
    );

    let lifecycle_activation = source_region(
        &source,
        "fn activate_with(",
        "impl ActivatedProductionLifecycleV1",
    );
    assert_source_tokens_in_order(
        lifecycle_activation,
        &[
            "lifecycle_activation_recovery_blocker(",
            "close_admission_for_restart()",
            "return Err(error)",
            "begin_fail_stop_operation()",
            ".local_proposal_directive()",
            "local_proposal.exactly_matches(self.executor.context().id(), current_directive)",
            "ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch",
            "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
            "arm_live_clocks(clock_activation, now)",
            "successor_activation_status_snapshot()",
            "completion_observer_activation.take()",
            "activate_effect_completion_observer(observer)",
            "publication.open_and_publish(",
            "activation.complete()",
            "ActivatedProductionLifecycleV1 {\n            runner_activation,\n            local_proposal,\n            launched: self,",
        ],
    );
    assert!(!lifecycle_activation.contains("set_v2_status"));
    assert!(!lifecycle_activation.contains("into_parts"));

    let activated_owner = source_region(
        &source,
        "struct ActivatedProductionLifecycleV1",
        "enum ProductionLifecycleActivationPublicationV1",
    );
    assert!(activated_owner.contains(
        "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1"
    ));
    assert!(
        activated_owner.contains("local_proposal: ProductionLifecyclePreparedLocalProposalStateV1")
    );
    assert!(activated_owner.contains("launched: LaunchedProductionLifecycleV1"));
    assert_source_tokens_in_order(
        activated_owner,
        &["runner_activation:", "local_proposal:", "launched:"],
    );
    assert_forbidden_source_tokens(
        activated_owner,
        &[
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
        ],
    );
    let activated_borrow = source_region(
        &source,
        "impl ActivatedProductionLifecycleV1",
        "impl ProductionLifecycleOwnerV1",
    );
    assert_required_source_tokens(
        activated_borrow,
        &[
            "fn with_runner_runtime<R>(",
            "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
            "&mut super::super::v2_runner::ProductionLifecycleLocalProposalStateV1",
            ".prepared_local_proposal_mut()",
            "&mut self.launched.owner",
            "&mut self.launched.executor",
            "&mut self.launched.services",
            "local_proposal",
            "fn close_runner_ingress_for_finalized_drain(",
            "receiver: &Arc<FairV2Ingress>",
            "self.runner_activation.close_ingress(receiver)?",
            "Arc::ptr_eq(",
        ],
    );
    assert_forbidden_source_tokens(
        activated_borrow,
        &[
            "into_parts",
            "fn into_owner(",
            "fn into_executor(",
            "fn into_services(",
            "pub launched:",
            "pub(crate) launched:",
        ],
    );

    let serve_retirement = source_region(
        &source,
        "fn refresh_live_serve_retirement_cut(",
        "/// Cross the ordinary/current/snapshot live-height boundary",
    );
    assert_source_tokens_in_order(
        serve_retirement,
        &[
            "exactly_covers_finalization_work(&self.coordinator)",
            "authenticate_current_lifecycle_serve_retirement(",
            "LifecycleLedgerV1::from_coordinator(&self.coordinator)",
            "authenticate_live_finalization_serve_census(",
            "self.serve_payloads = refreshed",
        ],
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
    let fixture_retirement = source_region(
        activated_borrow,
        "fn retire_lifecycle_stores_for_test(",
        "/// Borrow the live owner/runtime/service/local-Proposal owners only from the runner",
    );
    assert_source_tokens_in_order(
        fixture_retirement,
        &[
            "let Self {\n            mut launched,\n            local_proposal,\n            runner_activation,",
            "runner_activation\n            .retire(",
            "leader_wire_ingress_binding\n            .retire()",
            "seal_empty_exact_output_for_lifecycle_retirement_test()",
            "refresh_live_serve_retirement_cut(&launched.services, &retired_ingress)",
            ".retire_lifecycle_stores()",
        ],
    );

    let activated_finalization = source_region(
        activated_borrow,
        "fn into_finalized_rollover(",
        "/// Exercise the exact empty-output post-handoff retirement transaction",
    );
    let finalization_readiness = source_region(
        &source,
        "fn ready_for_finalized_rollover(&mut self) -> bool {",
        "impl ActivatedProductionLifecycleV1",
    );
    let owner_token = "let Self {\n            mut launched,\n            local_proposal,\n            runner_activation,";
    assert_required_source_tokens(
        finalization_readiness,
        &[
            "self.executor.ready_to_finish()",
            "!self.owner.has_recovered_lifecycle_outputs()",
            "self.pending_kura_apply_replay.is_none()",
            "self.recovered_local_proposal_attempt.is_none()",
            "self.pending_lifecycle_completion.is_none()",
            "self.pending_ingress_capacity.is_none()",
            "self.completion_observer_activation.is_none()",
            "exactly_covers_finalization_work(&self.owner.coordinator)",
        ],
    );
    assert_source_tokens_in_order(
        activated_finalization,
        &["!self.launched.ready_for_finalized_rollover()", owner_token],
    );
    assert_source_tokens_in_order(
        activated_finalization,
        &[
            owner_token,
            "runner_activation\n            .retire(",
            "leader_wire_ingress_binding\n            .retire()",
            "executor\n            .into_finalized_parts()",
            "begin_fail_stop_operation()",
            ".finish_height(&receipt, &artifact)",
            "operation.complete()",
        ],
    );
    assert_source_tokens_in_order(
        lifecycle_run_inner_source,
        &[
            "executor.ready_to_finish()",
            "if !apply_terminal_settled && (!ready_to_finish || producer_turn.is_some())",
            "schedule_local_proposal(",
            "let finalization_ready =",
            "activated.ready_for_finalized_rollover(&mut active_runner)",
            "let rollover_ready = if finalization_ready",
            "preflight_finalized_lane_rollover(",
            "if ready_to_finish && !rollover_ready",
            "if rollover_ready",
            "close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)",
            "let drained_terminal_ingress = activated.with_runner_runtime(\n                &mut active_runner,\n                |_owner, executor, services, _local_proposal| {\n                    let drained = drain_decided_lane_recovery_ingress(",
            "if drained_terminal_ingress",
            "continue;",
            "ensure_closed_drained_cut()",
            "finalize_lifecycle_height(",
        ],
    );

    let rollover = source_region(
        &source,
        "impl FinalizedProductionLifecycleRolloverV1",
        "impl ProductionLifecyclePostOutputHandoffV1",
    );
    assert_source_tokens_in_order(
        rollover,
        &[
            "rollover_finalized_height_outputs_for_lifecycle(",
            "ProductionLifecycleOutputRolloverPermitV1 {",
            "finalized_adapter.retire_after_output_handoff()",
            "refresh_live_serve_retirement_cut(&services, &retired_ingress)",
        ],
    );
    assert!(finalized_output_source.contains(
        "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1"
    ));

    let store_retirement = source_region(
        &source,
        "impl ProductionLifecyclePostOutputHandoffV1",
        "impl ProductionLifecycleCleanupReadyV1",
    );
    assert_source_tokens_in_order(
        store_retirement,
        &[
            "begin_fail_stop_operation()",
            ".retire_authenticated_cut(serve_payloads, &retained_serve_payloads)",
            ".stage_finalized_height_all_row_retirement(reconciliation)",
            ".persist_exact_finalization_successor(staged)",
            "publication.consume_owners(registry)",
            "kura_binding.bind_finalized_lifecycle_floor(published_floor)",
            "operation.complete()",
        ],
    );
    let cleanup = source_region(
        &source,
        "impl ProductionLifecycleCleanupReadyV1",
        "impl ProductionLifecycleOwnerV1",
    );
    assert_source_tokens_in_order(
        cleanup,
        &[
            "let output_guard = self.services.lifecycle_output_guard()",
            "self.services.allow_clean_shutdown()",
            ".finish_height(self.receipt, cleanup_timeout, supervisor)",
            "retained_floor: Some(self.retained_floor)",
            "output_guard,\n        }",
        ],
    );
    let floor_binding = source_region(
        &source,
        "fn bind_successor_storage(",
        "/// Move-only runner state joined to one exact launched reducer directive",
    );
    assert_source_tokens_in_order(
        floor_binding,
        &[
            "begin_fail_stop_operation()",
            "self.retained_floor.take()",
            ".bind_finalized_predecessor_floor(floor)",
            "operation.complete()",
        ],
    );
    for production_successor in [lifecycle_run_inner_source, pending_kura_lifecycle_source] {
        assert_source_tokens_in_order(
            production_successor,
            &[
                "cleanup.bind_successor_storage(lifecycle_storage_authority)?",
                "cleanup.wal_retirement_warning()",
                "cleanup.cleanup().warnings()",
            ],
        );
    }

    for state in [
        "FinalizedProductionLifecycleRolloverV1",
        "ProductionLifecyclePostOutputHandoffV1",
        "ProductionLifecycleCleanupReadyV1",
        "ProductionLifecycleFinalizationOutcomeV1",
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
        let declaration_start = prefix
            .rfind("\n}\n")
            .map_or(0, |offset| offset + 3)
            .max(prefix.rfind("\n\n").map_or(0, |offset| offset + 2));
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
    let published_retirement = source_region(
        ledger_source,
        "fn persist_exact_finalization_successor(",
        "#[cfg(test)]",
    );
    assert_source_tokens_in_order(
        published_retirement,
        &[
            "self,",
            "LifecycleLedgerV1::from_coordinator(&self)? != current",
            "store.persist_exact_successor(&current, &retired)?",
            "store.load()? != retired",
            "coordinator: self",
        ],
    );
    assert!(
        lifecycle_startup_test_source
            .contains("production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout")
    );
    let proposal_initialization_behavior = source_region(
        lifecycle_startup_test_source,
        "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
        "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network",
    );
    assert_source_tokens_in_order(
        proposal_initialization_behavior,
        &[
            "RecoveredLifecycleLocalProposalAttemptV1::for_test(",
            "retain_recovered_local_proposal_attempt_for_test(recovered_attempt)",
            "initialize_recovered_local_proposal(setup_runner)",
            "assert!(local_proposal_state.already_attempted(directive))",
            ".activate(Instant::now(), activation, local_proposal_state)",
        ],
    );
    assert!(
        lifecycle_startup_test_source
            .contains(".retire_lifecycle_stores_for_test(finality_receipt)")
    );
    assert!(
        lifecycle_startup_test_source
            .contains("cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)")
    );
    let finalization_behavior = source_region(
        lifecycle_startup_test_source,
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "fn expect_recovered_open_error",
    );
    assert_source_tokens_in_order(
        finalization_behavior,
        &[
            "let _status_guard = crate::sumeragi::status::rbc_status_test_guard()",
            "Algorithm::Ed25519",
            "TransactionBuilder::new_genesis(",
            "block_builder.set_da_proof_policies(Some(proof_policy_bundle))",
            ".try_build_with_signature(0, genesis_key.private_key())",
            "BlockSignaturePolicy::GenesisAuthority(",
            "WalRecordV2::Decision(decision.clone())",
            "let mut launched = owner",
            "ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(result)",
            "ProductionCompletionDispatchV1::ApplyQueued",
            "ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied",
            "let mut activated = launched",
            "lifecycle_run_inner::finalize_lifecycle_height(",
            ".retain_merge_sidecars_for_global_view(",
            "assert!(outcome.cleanup().warnings().is_empty())",
            "assert!(outcome.wal_retirement_warning().is_none())",
        ],
    );
    assert!(registry_validate_source.contains("broadcast.is_unpaired()"));
    assert!(
        registry_validate_source
            .contains("carrier.pairs_exact_next_sign(next_sign, next_sign_digest)")
    );

    let current_payload_census = source_region(
        payload_store_source,
        "fn authenticate_current_for_lifecycle_retirement(",
        "/// Compare this opened payload owner",
    );
    assert_required_source_tokens(
        current_payload_census,
        &[
            "self.reload_payload_census_strict()?",
            "payloads.keys().copied().collect::<BTreeSet<_>>() != self.indexed",
            ".authenticate_for_complete_tip_retirement(verified, local_signer)",
            "self.validate_authenticated_cut(&authenticated)?",
        ],
    );
    let live_serve_join = source_region(
        lifecycle_open_source,
        "fn authenticate_live_finalization_serve_census(",
        "/// Seal the final post-mutation Serve cut",
    );
    assert_required_source_tokens(
        live_serve_join,
        &[
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "authenticate_complete_tip_serve_census(ledger, recovered)?",
            "WaitSource::Capacity(class)",
            "receipt.exactly_matches_pending(payload.request())",
            "prepare_certified_serve_admission(",
            "candidate != waiting.candidate",
            "owned != recovered_ids",
        ],
    );
    let finalization_registry = source_region(
        registry_validate_source,
        "fn exactly_covers_finalization_work(",
        "fn exactly_covers_ready_work_with_extra(",
    );
    assert_required_source_tokens(
        finalization_registry,
        &[
            "DurableRecoveredLifecycleSignedBroadcast(_)",
            ".collect::<std::collections::BTreeSet<_>>()",
            "RecoveredWalRegistrySlotV1::None",
            "exactly_covers_ready_work_with_extra(",
            "&std::collections::BTreeSet::new()",
            "&refanned_broadcasts",
        ],
    );
    assert_required_source_tokens(
        registry_validate_source,
        &[
            "refanned_broadcasts.iter().all(|ordinal|",
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "!matches!(record.state, super::LifecycleState::Waiting(_))",
            "broadcast.validates_in_ledger(exact_ledger)",
            "broadcast.paired_next_sign_matches_terminal_record(",
            "broadcast.matches_current_finalization_record(",
            "!refanned_broadcasts.contains(&record.ordinal)",
        ],
    );
    assert!(lifecycle_run_inner_source.contains(
        "let finalization_ready =\n            ready_to_finish && activated.ready_for_finalized_rollover(&mut active_runner);"
    ));
    assert!(lifecycle_scheduler_source.contains(
        "finalization accepts the exact volatile refanout wait after its next Sign retires"
    ));
    assert!(
        lifecycle_scheduler_source.contains(
            "finalization rejects a corrupted retained digest after paired Sign retirement"
        )
    );
    assert!(
        lifecycle_scheduler_source.contains(
            "fn finalization_waits_for_every_authenticated_recovered_broadcast_refanout()"
        )
    );

    let runner_dependency_permit = source_region(
        runner_authority_source,
        "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
        "/// Process-local borrow key for preparing a launched lifecycle before activation",
    );
    assert_required_source_tokens(
        runner_dependency_permit,
        &[
            "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
            "local_signer: KeyPair",
            "block_cadence: Duration",
            "fn mint_for_recovered_runner(\n        local_signer: KeyPair,\n        block_cadence: Duration,\n    ) -> Self",
            "#[cfg(test)]",
            "fn for_test(",
            "fn into_factory_dependencies(self) -> (KeyPair, Duration)",
            "(self.local_signer, self.block_cadence)",
            "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
        ],
    );
    assert_forbidden_source_tokens(
        runner_dependency_permit,
        &[
            "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
            "pub(crate) fn mint_for_recovered_runner(",
            "pub fn mint_for_recovered_runner(",
            "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
            "fn into_parts(",
        ],
    );
    let ordinary_activation = source_region(
        runner_dependency_permit,
        "struct ProductionLifecycleRunnerActivationV1",
        "struct ProductionLifecycleCompleteTipRunnerActivationV1",
    );
    assert_required_source_tokens(
        ordinary_activation,
        &[
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
        ],
    );
    assert_source_tokens_in_order(
        ordinary_activation,
        &[
            "self.ingress_ready.store(false, Ordering::Release)",
            "Arc::ptr_eq",
            "self.block_ingress.close()",
            "self.block_ingress.open()",
            "let publication = match self.status",
        ],
    );
    let publish_status =
        source_token_position(ordinary_activation, "let publication = match self.status");
    let release_readiness = ordinary_activation
        .rfind("self.ingress_ready.store(true, Ordering::Release)")
        .unwrap();
    assert!(publish_status < release_readiness);
    assert_forbidden_source_tokens(
        ordinary_activation,
        &[
            "impl Clone for ProductionLifecycleRunnerActivationV1",
            "impl Copy for ProductionLifecycleRunnerActivationV1",
            "pub(in crate::sumeragi) fn current_height(",
            "pub(crate) fn current_height(",
            "pub fn current_height(",
            "pub(in crate::sumeragi) fn applied(",
            "pub(in crate::sumeragi) fn snapshot_bootstrap(",
            "fn into_parts(",
        ],
    );

    let complete_tip_activation = source_region(
        runner_dependency_permit,
        "struct ProductionLifecycleCompleteTipRunnerActivationV1",
        "struct ProductionLifecyclePendingKuraRunnerActivationV1",
    );
    assert_required_source_tokens(
        complete_tip_activation,
        &[
            "_seal: ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "struct ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "fn mint_for_recovered_runner(",
            "ProductionLifecycleActivatedRunnerAuthorityV1 {",
            "ingress_ready: self.ingress_ready",
            "block_ingress: self.block_ingress",
        ],
    );
    assert_source_tokens_in_order(
        complete_tip_activation,
        &[
            "self.ingress_ready.store(false, Ordering::Release)",
            "Arc::ptr_eq",
            "retirement.authorizes_successor_status(&successor)",
            "self.block_ingress.open()",
            "status::activate_recovered_complete_tip_v2_height(retirement, successor)",
            "self.ingress_ready.store(true, Ordering::Release)",
        ],
    );
    assert_source_token_count(complete_tip_activation, "self.block_ingress.close()", 3);
    assert_forbidden_source_tokens(
        complete_tip_activation,
        &[
            "impl Clone for ProductionLifecycleCompleteTipRunnerActivationV1",
            "impl Copy for ProductionLifecycleCompleteTipRunnerActivationV1",
            "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
            "pub(crate) fn mint_for_recovered_runner(",
            "pub fn mint_for_recovered_runner(",
            "fn into_parts(",
        ],
    );
    let activated_runner = source_region(
        runner_dependency_permit,
        "struct ProductionLifecycleActivatedRunnerAuthorityV1",
        "struct ProductionLifecycleActiveRunnerBorrowV1",
    );
    assert_required_source_tokens(
        activated_runner,
        &[
            "_seal: ProductionLifecycleActivatedRunnerAuthoritySealV1",
            "ingress_ready: Arc<AtomicBool>",
            "block_ingress: Arc<FairV2Ingress>",
            "impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1",
            "fn close_ingress(",
            "Finalized rollover uses this to establish a finite ingress cut",
            "fn retire(",
            "retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)",
            "fn retire_lifecycle_runner_ingress(",
            "ingress_ready.store(false, Ordering::Release)",
            "block_ingress.close()",
            "Arc::ptr_eq(block_ingress, launched_ingress)",
            "self.ingress_ready.store(false, Ordering::Release)",
            "self.block_ingress.close()",
            "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
        ],
    );
    assert_forbidden_source_tokens(
        activated_runner,
        &[
            "impl Clone for ProductionLifecycleActivatedRunnerAuthorityV1",
            "impl Copy for ProductionLifecycleActivatedRunnerAuthorityV1",
            "fn into_parts(",
            "pub ingress_ready:",
            "pub block_ingress:",
        ],
    );
    assert_source_token_count(
        activated_runner,
        "self.ingress_ready.store(false, Ordering::Release)",
        2,
    );
    assert_source_token_count(activated_runner, "self.block_ingress.close()", 2);
    let helper = &activated_runner
        [source_token_position(activated_runner, "fn retire_lifecycle_runner_ingress(")..];
    assert_source_tokens_in_order(
        helper,
        &[
            "ingress_ready.store(false, Ordering::Release)",
            "block_ingress.close()",
            "Arc::ptr_eq(block_ingress, launched_ingress)",
        ],
    );
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
    assert_required_source_tokens(
        runner_errors,
        &[
            "LifecycleOwnerStartup(#[from] super::v2::ProductionLifecycleOwnerStartupErrorV1)",
            "ProductionLifecycleLaunchErrorV1",
            "ProductionLifecycleActivationErrorV1",
            "ProductionLifecycleShutdownErrorV1",
            "ProductionLifecycleFinalizationErrorV1",
        ],
    );
    assert!(runner_tests_source.contains(
        "fn recovered_lifecycle_factory_dependency_permit_retains_exact_signer_and_cadence()"
    ));
    let factory_bind = source_region(
        &adapter_source,
        "fn bind_production_lifecycle_owner_factory_inputs_v1(",
        "/// Consume all recovered adapter and storage authority",
    );
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
    let worker_completion_source = include_str!("v2_worker_completion.rs");
    let worker_io_execution_source = include_str!("v2_worker_io_execution.rs");
    let worker_tests_source = include_str!("tests/v2_worker_recovered_lifecycle_output_cases.rs");
    let launch_source = include_str!("v2_lifecycle_launch.rs");
    let effects_tests_source = include_str!("tests/v2_effects_main_04.rs");

    let dispatch = source_region(
        scheduler_source,
        "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
        "pub(super) fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
    );
    assert_source_tokens_in_order(
        dispatch,
        &[
            "let Some(body_store_identity) = self.body_store_identity.as_ref()",
            "services.matches_lifecycle_body_store(body_store_identity)",
            "services.matches_lifecycle_executor_output_guard(executor)",
            "attest_ready_recovered_lifecycle_sign",
            "capture_recovered_lifecycle_sign_capacity(dispatch_key)",
            "self.coordinator.plan_turn(inputs)",
            "reservation.class() == CapacityClass::Consensus",
            "prepare_recovered_lifecycle_sign_dispatch",
            "reservation.preflight(&prepared)",
            "reservation.commit(prepared)",
        ],
    );
    assert_source_token_count(
        dispatch,
        "self.coordinator.rollback_unpublished_turn(&lease)",
        1,
    );
    assert_source_token_count(dispatch, "rollback_unpublished_reserved_turn(&lease", 3);
    assert_source_token_count(dispatch, "reservation.cancel_uncommitted()", 6);
    assert_forbidden_source_tokens(
        dispatch,
        &[
            "AdapterEffect",
            "PendingRuntimeEffectBinding",
            "RuntimeEffectOwnership",
            "EffectWorkId",
            "into_parts",
        ],
    );

    let phase_carrier = source_region(
        registry_source,
        "impl DurableRecoveredWalSignWork {",
        "/// Whether one concrete registry row is still an executable adapter effect",
    );
    assert_source_token_count(
        phase_carrier,
        "self.matches_current_terminal_parent(coordinator)",
        2,
    );
    assert_source_token_count(
        phase_carrier,
        "metadata.continuation == super::schema::DurableContinuation::None",
        2,
    );
    assert_required_source_tokens(
        phase_carrier,
        &[
            "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
            "metadata.matches_admission(parent)",
            "super::schema::DurableContinuation::successor(",
            "coordinator.key_index.get(&parent.key)",
            "coordinator.owner_index.get(&parent.causal_root)",
        ],
    );

    let identity = source_region(
        registry_source,
        "impl RecoveredLifecycleSignDispatchIdentityV1 {",
        "/// Read-only coordinates of one exact Waiting Fetch incumbent.",
    );
    assert_required_source_tokens(
        identity,
        &["&AdapterEffect::Sign {", "request: request.clone()"],
    );
    assert_forbidden_source_tokens(identity, &["tag.view() ==", "vote.round.view"]);

    let task = source_region(
        worker_source,
        "pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {",
        "enum V2IoCommand {",
    );
    assert_required_source_tokens(
        task,
        &[
            "identity: RecoveredLifecycleSignDispatchIdentityV1",
            "prepared_candidate: Option<PreparedCandidateBody>",
            "self.task.prepared_candidate == expected_prepared",
            "outbound_payload: Option<EncodedV2Payload>",
            "authorizes_request(self.task.tag, &self.task.request)",
        ],
    );
    assert_forbidden_source_tokens(
        task,
        &[
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
        ],
    );
    let parked_completion = source_region(
        worker_completion_source,
        "pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {",
        "/// Result of atomically returning one guarded missing-sidecar Apply",
    );
    assert_forbidden_source_tokens(
        parked_completion,
        &[
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
        ],
    );
    let signer = source_region(
        worker_io_execution_source,
        "fn sign_recovered_lifecycle_task(",
        "fn recover_outbound_proposal_payload(",
    );
    assert_forbidden_source_tokens(
        signer,
        &["prepared_candidates", "register_outbound_payload"],
    );
    let capacity = source_region(
        worker_source,
        "fn capture_recovered_lifecycle_sign_capacity<'a>(",
        "/// Project worker capacity for one recovered candidate without changing the queue cut.",
    );
    assert_source_token_count(capacity, "operation.complete()", 4);
    assert_forbidden_source_tokens(capacity, &["drop(operation)"]);

    let rollback = source_region(
        coordinator_source,
        "fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {",
        "/// Rebuild records after seeding the ordinal high-water mark.",
    );
    assert_required_source_tokens(
        rollback,
        &[
            "lease.output_reservation.is_some()",
            "assert!(\n            inserted,",
        ],
    );
    assert_forbidden_source_tokens(rollback, &["debug_assert!"]);

    for regression in [
        "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
        "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
        "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
        "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
    ] {
        assert!(
            worker_source.contains(regression)
                || worker_tests_source.contains(regression)
                || coordinator_source.contains(regression),
            "recovered Sign prerequisite omitted behavior regression {regression}"
        );
    }
    assert!(launch_source.contains(".dispatch_recovered_lifecycle_sign("));
    assert!(scheduler_source.contains(
        "Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)"
    ));
    assert!(
        effects_tests_source.contains(
            "a non-Completion runner cursor cannot claim or mutate a recovered Sign owner"
        )
    );

    let settlement = source_region(
        launch_source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
        "/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion(authority)",
            "prepare_recovered_lifecycle_sign_broadcast_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_transition(",
            "output_guard.begin_fail_stop_operation()",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "operation.complete();",
        ],
    );
    assert!(!settlement.contains("capture_recovered_lifecycle_signed_broadcast_refanout"));
    assert!(!settlement.contains("commit_after_publication();\n        output"));
    let coordinator_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let tail = &settlement[coordinator_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains(".is_err()"));

    let refanout = source_region(
        scheduler_source,
        "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        "pub(super) fn persist_recovered_decision_fetch_response_after_runner(",
    );
    assert_source_tokens_in_order(
        refanout,
        &[
            "if exact_ready != self.coordinator.ready_index",
            "record.work_class != LifecycleWorkClass::Broadcast",
            "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
            "attest_ready_recovered_lifecycle_signed_broadcast",
            "let factory = AuthenticatedSchedulerInputsFactory::new()",
            "attest_ready_recovered_lifecycle_sign(",
            "self.coordinator.plan_turn(inputs)",
            "project_claimed_recovered_lifecycle_signed_broadcast_output",
            "capture_recovered_lifecycle_signed_broadcast_refanout(authority)",
            "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
            "output.commit_after_publication()",
        ],
    );
    assert_required_source_tokens(
        refanout,
        &[
            "rollback_unpublished_turn(&lease)",
            "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(",
        ],
    );
    assert_forbidden_source_tokens(
        refanout,
        &[
            "exact_ready.len() == 2",
            "exact_ready.len() != 2",
            "persist_exact_successor",
            "TurnOutcome::Terminal",
        ],
    );

    let launched = source_region(
        launch_source,
        "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
        "\n}",
    );
    assert_source_tokens_in_order(
        launched,
        &[
            "services: ProductionV2Services",
            "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>",
            "pending_ingress_capacity: Option<PendingIngressCapacityV1>",
            "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
        ],
    );
    assert!(!launched.contains("recovered_lifecycle_sign_completion:"));
    assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed();
    assert_recovered_proposal_prepare_wal_settlement_is_restart_closed();
    assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed();
}

fn assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
        "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "preview.is_vote_broadcast_and_sign_shape()",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "output_guard.begin_fail_stop_operation()",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "operation.complete();",
        ],
    );
    assert_forbidden_source_tokens(
        settlement,
        &[
            "project_proposal_exact_output_authority",
            "capture_recovered_lifecycle_proposal_exact_output",
            "output.commit_after_publication()",
        ],
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}

fn assert_recovered_proposal_prepare_wal_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(",
        "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
            "preview.project_proposal_exact_output_authority()",
            "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
            "output.prepare_wal_append_permit()",
            "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "output.commit_after_publication();",
        ],
    );
    assert_required_source_tokens(
        settlement,
        &[
            "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
            "Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
        ],
    );
    assert_forbidden_source_tokens(settlement, &["output.abort_before_publication()"]);
    let wal = source_token_position(
        settlement,
        "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let post_wal = &settlement[wal..transition_commit];
    assert!(post_wal.matches("drop(output);").count() >= 3);
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}

fn assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
        "/// Drive and retry one exact missing-sidecar lifecycle Decision Apply owner.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "preview.project_proposal_exact_output_authority()",
            "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "output.commit_after_publication();",
        ],
    );
    assert_source_token_count(settlement, "output.abort_before_publication()", 2);
    assert_required_source_tokens(
        settlement,
        &[
            "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
            "Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
            "drop(output);",
        ],
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}

#[test]
fn recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let dispatch = scheduler
        .split_once("fn dispatch_completion_with_runner_debt(")
        .expect("lifecycle Completion has one composite dispatch transaction")
        .1
        .split_once(
            "/// Reserve, claim, and dispatch the sole Ready lifecycle-owned recovered Sign.",
        )
        .expect("composite dispatch stays a bounded source region")
        .0;
    let census = dispatch
        .find("capture_lifecycle_completion_capacity_census(probes)")
        .expect("the joint physical census is captured");
    let claim = dispatch
        .find("self.coordinator.plan_turn(inputs)")
        .expect("coordinator claim exists");
    let output = dispatch
        .find(".select_fetch(ordinal)")
        .expect("the selected Fetch owns exact output");
    let executor = dispatch
        .find("prepare_recovered_decision_fetch_request_registration(owner)")
        .expect("executor vacancy is reserved");
    let staged_wait = dispatch
        .find("let mut next = self.coordinator.stage_durable_transaction();")
        .expect("the exact external wait is staged before owner mutation");
    let registry = dispatch
        .find("prepare_recovered_decision_fetch_dispatch(")
        .expect("the claimed row projects its exact task");
    let commit = dispatch
        .find("registration.commit(prepared, wait_source)")
        .expect("request owner has one commit tail");
    let waiting = dispatch
        .find("self.coordinator = next;")
        .expect("the claimed Fetch is parked before external publication");
    let publication = dispatch
        .find("output.commit();")
        .expect("exact output publishes after request installation");
    assert!(
        census < claim
            && claim < output
            && output < executor
            && executor < staged_wait
            && staged_wait < registry
            && registry < commit
            && commit < waiting
            && waiting < publication
    );
}

#[test]
fn recovered_decision_fetch_queue_parks_generic_drain_and_uses_unified_completion_classifier() {
    let worker = [
        include_str!("v2_worker.rs"),
        include_str!("v2_worker_services_impl.rs"),
    ]
    .concat();
    let generic = worker
        .split_once("fn take_io_completion(")
        .expect("generic completion selector exists")
        .1
        .split_once("fn take_recovered_lifecycle_sign_completion(")
        .expect("generic selector stays bounded")
        .0;
    assert!(generic.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)"));
    assert!(generic.contains("self.held_io_completion = Some(completion);"));
    let classifier = worker
        .split_once("fn take_next_lifecycle_completion(")
        .expect("unified recovered lifecycle classifier exists")
        .1
        .split_once("pub(in crate::sumeragi) fn drain_recovered_lifecycle_sign_completion(")
        .expect("unified classifier stays bounded")
        .0;
    assert!(classifier.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded)"));
    assert!(classifier.contains("LifecycleCompletionTakeV1::DecisionFetch("));
    assert!(worker.contains("tracked.state = V2IoWorkState::Active;"));
    assert!(worker.contains("tracked.state = V2IoWorkState::CompletionPending;"));
    assert!(!worker.contains("drain_recovered_decision_fetch_body_completion"));
}

#[test]
fn ordinary_certified_body_pipeline_has_no_retained_compatibility_carrier() {
    let effects = include_str!("v2_effects.rs");
    let runtime = include_str!("v2_runtime.rs");
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let ordinary_consumer = include_str!("v2_runner/ordinary_ingress_consumer.rs");
    let turn_driver = include_str!("v2_lifecycle_turn_driver.rs");

    for (source, forbidden) in [
        (effects, concat!("RetainedCertifiedBody", "Response")),
        (effects, concat!("retained_certified_body_", "response")),
        (
            effects,
            concat!("accept_certified_body_", "response_with_ingress_ownership"),
        ),
        (runtime, "retained_response_predecessor_target_ordinal"),
        (runtime, "retained_response_predecessor_retry_attempted"),
        (
            run_inner,
            concat!("service_retained_certified_", "response"),
        ),
        (
            run_inner,
            concat!("retry_retained_certified_body_", "response"),
        ),
    ] {
        assert!(
            !source.contains(forbidden),
            "retired ordinary response compatibility surface returned: {forbidden}",
        );
    }
    assert!(
        ordinary_consumer.contains("retired certified body response outside lifecycle selection")
    );
    assert!(
        ordinary_consumer
            .contains("a selected fetch response must instead complete through lifecycle")
    );
    assert!(!ordinary_consumer.contains(concat!("accept_certified_body_", "response(")));
    assert!(turn_driver.contains("drive_certified_fetch_ingress_selector(selector, runner)"));
    assert!(turn_driver.contains("complete_certified_fetch_body_persistence("));
}

#[test]
fn registered_validate_sidecar_barrier_services_only_lane_transport_before_yield() {
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let barrier = source_region(
        run_inner,
        "let lane_only_completion_barrier = producer_claim.blocks_runtime();",
        "let discovery_was_outstanding = if lane_only_completion_barrier",
    );
    assert_source_tokens_in_order(
        barrier,
        &[
            "if lane_only_completion_barrier",
            "drain_lane_relay_ingress(",
            "lane_work.schedule_retransmission()?",
            "dispatch_lane_work_effects(&mut lane_work, services, control_queue_capacity)",
        ],
    );
    assert!(!barrier.contains("advance_executor("));

    let post_drain = source_region(
        run_inner,
        "producer_claim = drain_disposition.producer_claim();",
        "let (ready_to_finish, lifecycle_yield)",
    );
    assert_source_tokens_in_order(
        post_drain,
        &[
            "if drain_disposition.requires_yield()",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "continue;",
        ],
    );
}

#[test]
fn recovered_decision_fetch_phase_a_is_reachable_only_after_runner_validation() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let ingress_turn = driver
        .split_once("pub(in crate::sumeragi) fn drive_ingress_turn")
        .expect("unified ingress driver exists")
        .1
        .split_once("fn drive_recovered_ingress_selector")
        .expect("runner validation precedes the recovered Phase-A helper")
        .0;
    let cursor = ingress_turn
        .find("if !self.runner_turn_matches(")
        .expect("the driver validates the borrow-bound runner");
    let handoff = ingress_turn
        .find("self.drive_recovered_ingress_selector(selector, runner)")
        .expect("the validated runner enters recovered Phase A");
    assert!(cursor < handoff);
    assert!(driver.contains("persist_recovered_decision_fetch_response_after_runner("));
    assert!(!scheduler.contains("fn persist_recovered_decision_fetch_response("));
}

#[test]
fn authenticated_current_serve_context_drift_fails_closed_instead_of_retrying() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let narrowing = source_region(
        driver,
        "let expected_context = lifecycle_context_for_ingress(executor.context());",
        "let selector = match executor.capture_lifecycle_ingress_selector(lifecycle_cut)",
    );
    assert_source_tokens_in_order(
        narrowing,
        &[
            "Ok(FairIngressTurnContextCut::Ordinary(cut))",
            "authenticated current Certified-Serve lost its active lifecycle context",
            "close_admission_for_restart()",
            "drop(cut);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );
    assert!(!driver.contains("OrdinaryRetained"));
}

#[test]
fn authenticated_current_serve_queue_refresh_retries_without_closing_output() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let narrowing = source_region(
        driver,
        "let expected_context = lifecycle_context_for_ingress(executor.context());",
        "let selector = match executor.capture_lifecycle_ingress_selector(lifecycle_cut)",
    );
    let retry = source_region(
        narrowing,
        "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
        "Err((error, retained))",
    );
    assert_source_tokens_in_order(
        retry,
        &[
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!retry.contains("close_admission_for_restart()"));

    let structural_failure = source_region(
        driver,
        "Err((error, retained))",
        "let selector = match executor.capture_lifecycle_ingress_selector(lifecycle_cut)",
    );
    assert_source_tokens_in_order(
        structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );

    let selector_capture = source_region(
        driver,
        "let selector = match executor.capture_lifecycle_ingress_selector(lifecycle_cut)",
        "let (dequeue, target)",
    );
    let selector_retry = source_region(
        selector_capture,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        selector_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!selector_retry.contains("close_admission_for_restart()"));

    let selector_structural_failure = source_region(
        driver,
        "authenticated current Certified-Serve selector capture failed closed",
        "let (dequeue, target)",
    );
    assert_source_tokens_in_order(
        selector_structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );

    let exact_dequeue = source_region(driver, "let (dequeue, target)", "let ready_ledger =");
    let exact_dequeue_retry = source_region(
        exact_dequeue,
        "Err(CertifiedServeExactDequeueErrorV1::Queue(",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        exact_dequeue_retry,
        &[
            "FairIngressQueueCutError::QueueCutChanged",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!exact_dequeue_retry.contains("close_admission_for_restart()"));

    let exact_dequeue_structural_failure = source_region(
        driver,
        "Certified-Serve exact dequeue failed closed",
        "let ready_ledger =",
    );
    assert_source_tokens_in_order(
        exact_dequeue_structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );
}

#[test]
fn certified_response_queue_refresh_retries_without_closing_output() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let response_path = source_region(
        driver,
        "if !selected_ingress_is_certified_body_response",
        "fn drive_recovered_ingress_selector",
    );

    let narrowing = source_region(
        response_path,
        "let expected_context = lifecycle_context_for_ingress(self.executor.context());",
        "match contextual",
    );
    let narrowing_retry = source_region(
        narrowing,
        "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
        "Err((error, retained))",
    );
    assert_source_tokens_in_order(
        narrowing_retry,
        &[
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!narrowing_retry.contains("close_output_for_restart()"));

    let ownership = source_region(
        response_path,
        "let response_owner = match self",
        "match response_owner",
    );
    let ownership_retry = source_region(
        ownership,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        ownership_retry,
        &[
            "drop(cut);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!ownership_retry.contains("close_output_for_restart()"));

    let ordinary = source_region(
        response_path,
        "SelectedCertifiedBodyResponseOwnerV1::OrdinaryWinner",
        "SelectedCertifiedBodyResponseOwnerV1::RecoveredWinner",
    );
    let ordinary_retry = source_region(
        ordinary,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        ordinary_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!ordinary_retry.contains("close_output_for_restart()"));

    let recovered = source_region(
        response_path,
        "SelectedCertifiedBodyResponseOwnerV1::RecoveredWinner",
        "self.drive_recovered_ingress_selector(selector, runner)",
    );
    let recovered_retry = source_region(
        recovered,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        recovered_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!recovered_retry.contains("close_output_for_restart()"));

    for structural_failure in [
        source_region(narrowing, "Err((error, retained))", "};"),
        source_region(ownership, "Err(error)", "};"),
        source_region(ordinary, "Err(error)", "};"),
        source_region(recovered, "Err(error)", "};"),
    ] {
        assert_source_tokens_in_order(
            structural_failure,
            &[
                "close_output_for_restart();",
                "drop(runner);",
                "ProductionLifecycleIngressSelectionV1::RestartRequired",
            ],
        );
    }
}

#[test]
fn recovered_decision_fetch_phase_a_wakes_waiting_owner_before_queue_publication() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let registry = include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs");
    let phase_a = source_region(
        scheduler,
        "pub(super) fn persist_recovered_decision_fetch_response_after_runner(",
        "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
    );
    assert_source_tokens_in_order(
        phase_a,
        &[
            "self.coordinator.active_lease.is_some()",
            "attest_scheduler_recovered_fetch_carrier(",
            "capture_lifecycle_capacity_rank(selector)",
            "authenticated_waiting_fetch_ready_row(",
            "prepare_recovered_decision_fetch_response_claim(&task)",
            "let mut next = self.coordinator.stage_durable_transaction();",
            "let lease = match next.plan_turn(inputs)",
            "matches_claimed_dispatched_recovered_decision_fetch(",
            "self.coordinator = next;",
            "claim.commit_with_queue(reservation, task);",
        ],
    );
    let swap = source_token_position(phase_a, "self.coordinator = next;");
    let tail = &phase_a[swap..];
    assert!(!tail.contains("return Err"));
    assert!(!tail.contains("settle_turn("));
    assert!(tail.contains("assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))"));
    let waiting_carrier = source_region(
        registry,
        "pub(super) fn matches_waiting_dispatched_recovered_decision_fetch(",
        "/// Join one exact claimed recovered Decision Fetch back to its closed carrier.",
    );
    assert_required_source_tokens(
        waiting_carrier,
        &[
            "coordinator.records.iter().any",
            "*candidate != ordinal",
            "wait.source() == wait_source",
        ],
    );
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

    let worker = include_str!("v2_worker_completion.rs");
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

    let ledger = [
        include_str!("v2_lifecycle_ledger.rs"),
        include_str!("v2_lifecycle_ledger_operations.rs"),
    ]
    .concat();
    let open = include_str!("v2_lifecycle_open.rs");
    let registry_source = [
        include_str!("v2_lifecycle_work_registry_validate_recovery.rs"),
        include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"),
    ]
    .concat();
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
