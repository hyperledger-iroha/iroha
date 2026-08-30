#[test]
fn startup_reconciles_lifecycle_before_lane_work_activation() {
    let parent = include_str!("../v2_runner.rs");
    for anchor in [
        "if pending_kura_apply.is_none()",
        "lifecycle_run_inner::run_non_pending_lifecycle_loop(",
    ] {
        assert!(
            parent.contains(anchor),
            "runner lost lifecycle handoff anchor: {anchor}"
        );
    }
    let source = include_str!("../v2_runner/lifecycle_run_inner.rs");
    let anchors = [
        "V2BodyStoreCapacity::new(",
        ".mint_v2_body_store_directory_authority()",
        "V2BodyStore::open_with_kura_authority_and_capacity(",
        ".into_quarantined_recovered_startup()",
        "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
        ".authenticate_final_wal_startup_authority()",
        "open_production_lifecycle_owner_v1(",
        "launch_non_pending_lifecycle_height(",
        "LaneApplicationEvidenceRepairQueueFence::capture(queue.as_ref())?",
        "evidence_repair_queue_fence.revalidate(queue.as_ref())?",
        "let deferred_terminal_recovery =",
        "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
        "let planning = plan_lane_reservation_ownership(",
        "let lifecycle = reconcile_autonomous_lifecycle_startup(",
        "deferred_terminal_recovery,",
        "plan_lane_reservation_ownership(",
        "let summary = apply_lane_reservation_reconciliation_plan(",
        "reservation_reconciliation_pending = false;",
        "construct_after_pending_tip_application_recovery(",
        "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;",
        "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
        "initialize_recovered_local_proposal(setup_runner)",
        "let height_started_at = Instant::now();",
        "preactivation.activate(height_started_at, local_proposal)",
        "run_lifecycle_active_height(",
    ];
    let mut remainder = source;
    for anchor in anchors {
        let offset = remainder
            .find(anchor)
            .unwrap_or_else(|| panic!("runner lost lifecycle startup anchor: {anchor}"));
        remainder = &remainder[offset + anchor.len()..];
    }
}

#[test]
fn fresh_proposal_refreshes_merge_certification_before_freezing_attachments() {
    let source = include_str!("../v2_runner.rs");
    let start = source
        .find("fn schedule_local_proposal(")
        .expect("fresh proposal scheduler remains source-bound");
    let end = source[start..]
        .find("fn canonical_height_one_proposal_wire(")
        .map(|offset| start + offset)
        .expect("fresh proposal scheduler remains independently bounded");
    let scheduler = &source[start..end];
    let refresh = scheduler
        .find("lane_work.refresh_merge_candidates(directive.tag().view())?")
        .expect("the current-round merge quorum is refreshed");
    let admissions = scheduler[refresh..]
        .find("lane_work.reconcile_pending_queue_plan_admissions(")
        .map(|offset| refresh + offset)
        .expect("QueuePlan controls are reconciled after merge refresh");
    let attachments = scheduler[admissions..]
        .find("let attachments = candidate_attachments(")
        .map(|offset| admissions + offset)
        .expect("candidate attachments are frozen after control reconciliation");
    let assembly = scheduler[attachments..]
        .find("let assembly = assembler.assemble(")
        .map(|offset| attachments + offset)
        .expect("candidate assembly consumes the frozen attachments");
    assert!(refresh < admissions && admissions < attachments && attachments < assembly);
}

#[test]
fn emergency_fast_idles_before_any_active_height_recovery() {
    let parent = include_str!("../v2_runner.rs");
    let fast_binding = parent
        .find("if kura.emergency_fast_startup_enabled()")
        .expect("runner must check the Kura Fast policy before recovery");
    let inventory_release = parent[fast_binding..]
        .find("startup_replay_inventory_guard.finish();")
        .map(|offset| fast_binding + offset)
        .expect("Fast runner must release the startup inventory");
    let ingress_close = parent[inventory_release..]
        .find("block_rx.close();")
        .map(|offset| inventory_release + offset)
        .expect("Fast runner must close consensus ingress directly");
    let passive_return = parent[ingress_close..]
        .find("return Ok(());")
        .map(|offset| ingress_close + offset)
        .expect("Fast runner must remain passive until shutdown");
    let platform_check = parent[passive_return..]
        .find("require_validator_storage_platform(")
        .map(|offset| passive_return + offset)
        .expect("Strict platform checks must remain available");
    let recovery = parent[platform_check..]
        .find("let recovered = recover_active_height_with_plan")
        .map(|offset| platform_check + offset)
        .expect("Strict active-height recovery must remain available");
    assert!(fast_binding < inventory_release);
    assert!(inventory_release < ingress_close && ingress_close < passive_return);
    assert!(passive_return < platform_check && platform_check < recovery);
}
#[test]
fn lane_evidence_repair_fence_accepts_an_empty_unquarantined_replay() {
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender,
    );
    let journal_dir = tempfile::tempdir().expect("empty runner Queue journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install empty runner QueuePlan journal");
    let replay = queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install empty runner reservation journal");
    assert_eq!(replay, Default::default());
    assert!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture empty runner Queue replay")
            .is_empty()
    );
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    let fence = LaneApplicationEvidenceRepairQueueFence::capture(&queue)
        .expect("empty unquarantined Queue replay is a valid startup cut");
    fence
        .revalidate(&queue)
        .expect("unchanged empty Queue replay remains valid through evidence repair");
}
#[test]
fn terminal_sweep_source_partitions_whole_units_before_any_mutation() {
    let source = include_str!("../v2_lifecycle_recovery.rs");
    let start = source
        .find("fn pending_terminal_recovery_observations(")
        .expect("terminal partition preflight remains source-bound");
    let end = source[start..]
        .find("/// Close any planner-covered Pending sources left after the normal Queue")
        .map(|offset| start + offset)
        .expect("terminal sweep remains independently bounded");
    let sweep = &source[start..end];
    let anchors = [
        "if recovery.network_id() != network_id",
        "let route_identities = recovery.route_identities();",
        ".any(|identity| !active_routes.contains(identity))",
        "pub(crate) fn reconcile_pending_autonomous_lifecycle_terminal_outcomes(",
        "let initial_queue_quarantine = queue.lane_reservation_startup_reconciliation_pending();",
        "let initial_snapshot = queue",
        "if !initial_snapshot.is_empty() && !initial_queue_quarantine",
        "let active_routes = active_lifecycle_routes(state, context)?",
        "let network_id = context.network_id;",
        "pending_autonomous_lifecycle_terminal_outcome_inventory()",
        "let mut seen_entrypoint_hashes = BTreeSet::new();",
        "!seen_entrypoint_hashes.insert(key.entrypoint_hash.clone())",
        "pending_terminal_group_has_exact_queue_owner(&initial_snapshot, observation)?",
        "let deferred = !owned_group_hashes.is_empty();",
        "if preflight.deferred",
        "recover_pending_autonomous_lifecycle_terminal_outcome(",
        "!= initial_snapshot",
        "!= initial_queue_quarantine",
        "pending_autonomous_lifecycle_terminal_outcome_inventory()",
        "observed_deferred_units != expected_deferred_units",
    ];
    let mut remainder = sweep;
    for anchor in anchors {
        let offset = remainder
            .find(anchor)
            .unwrap_or_else(|| panic!("terminal sweep lost safety anchor: {anchor}"));
        remainder = &remainder[offset + anchor.len()..];
    }
    assert_eq!(
        sweep
            .match_indices("pending_autonomous_lifecycle_terminal_outcome_inventory()")
            .count(),
        2,
        "terminal sweep must use one bounded input inventory and one exact deferred-set readback",
    );
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    deferred_terminal_completion_requires_two_exact_stage_proofs_and_ordered_pending_coverage
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    planner_covered_pending_attempts_are_exposed_for_pairing_but_skipped_for_recovery
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    nonqueue_replica_release_is_fifo_proved_move_only_and_restart_closed
);
#[test]
fn local_producer_queue_custody_is_preflighted_before_cursor_mutation() {
    let source = include_str!("../v2_lifecycle_recovery.rs");
    let helper = source
        .find("fn require_local_producer_queue_owner(")
        .expect("local-producer custody helper remains source-bound");
    let startup = source
        .find("pub(crate) fn reconcile_autonomous_lifecycle_startup(")
        .expect("lifecycle startup remains source-bound");
    let first_full_inventory = source[startup..]
        .find(".active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(")
        .map(|offset| startup + offset)
        .expect("startup retains a read-only all-attempt preflight");
    let producer_preflight = source[first_full_inventory..]
        .find("require_local_producer_queue_owner(payload, cursor, &current_queue_groups)?;")
        .map(|offset| first_full_inventory + offset)
        .expect("startup preflights exact local-producer Queue custody");
    let bootstrap_mutation = source[startup..]
        .find("for authority in bootstraps {")
        .map(|offset| startup + offset)
        .expect("startup retains bounded bootstrap completion");
    let cursor_mutation = source[startup..]
        .find("if recover_one_attempt(")
        .map(|offset| startup + offset)
        .expect("startup retains bounded cursor recovery");
    assert!(helper < startup);
    assert!(first_full_inventory < producer_preflight);
    assert!(producer_preflight < bootstrap_mutation);
    assert!(producer_preflight < cursor_mutation);
    assert!(
        source[helper..startup].contains("local_actor != binding.producer_actor_projection()",),
        "observer Kura custody must remain independent of producer Queue ownership",
    );
    assert!(
        source[helper..startup].contains("current_keys.as_slice() == ordered_keys"),
        "producer recovery must preserve the exact ordered reservation keys",
    );
    assert!(
        source[helper..startup].contains("lane_queue_reservation_group_binding_from_ordered_keys"),
        "producer recovery must recompute the exact ordered reservation binding",
    );
}
