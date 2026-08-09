#[test]
fn startup_reconciles_lifecycle_before_lane_work_activation() {
    let source = include_str!("../v2_runner.rs");
    let anchors = [
        "let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(",
        "LaneApplicationEvidenceRepairQueueFence::capture(queue.as_ref())?",
        "evidence_repair_queue_fence.revalidate(queue.as_ref())?",
        "let deferred_terminal_recovery =",
        "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
        "let planning = plan_lane_reservation_ownership(",
        "let lifecycle = reconcile_autonomous_lifecycle_startup(",
        "deferred_terminal_recovery,",
        "let replanned = plan_lane_reservation_ownership(",
        "let summary = apply_lane_reservation_reconciliation_plan(",
        "reservation_reconciliation_pending = false;",
        "let mut lane_work = construct_after_pending_tip_application_recovery(",
        "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;",
        "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
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
        "if recovery.chain_id_hash() != chain_id_hash",
        "let route_identities = recovery.route_identities();",
        ".any(|identity| !active_routes.contains(identity))",
        "pub(crate) fn reconcile_pending_autonomous_lifecycle_terminal_outcomes(",
        "let initial_queue_quarantine = queue.lane_reservation_startup_reconciliation_pending();",
        "let initial_snapshot = queue",
        "if !initial_snapshot.is_empty() && !initial_queue_quarantine",
        "let active_routes = active_lifecycle_routes(state, context)?",
        "let chain_id_hash = Hash::prehashed(*context.network_id.as_bytes());",
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

#[test]
fn deferred_terminal_completion_requires_two_exact_stage_proofs_and_ordered_pending_coverage() {
    let source = include_str!("../v2_lifecycle_recovery.rs");
    let start = source
        .find(
            "pub(crate) fn complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
        )
        .expect("deferred terminal completion remains source-bound");
    let end = source[start..]
        .find("/// Reconcile every local lifecycle bootstrap and cursor")
        .map(|offset| start + offset)
        .expect("deferred terminal completion remains independently bounded");
    let completion = &source[start..end];
    let anchors = [
        "let queue_snapshot = queue",
        "let mut expected_by_group = BTreeMap::new();",
        "for (group_position, observation) in unit.pending_groups.iter().enumerate()",
        "verify_expected_autonomous_lifecycle_terminal_outcome_stages(",
        "let mut expected_pending_groups = BTreeSet::new();",
        "pending_autonomous_lifecycle_terminal_outcome_inventory()",
        "let mut previous_group_position = None;",
        "previous_group_position.is_some_and(|previous| previous >= group_position)",
        "if observed_groups != expected_pending_groups",
        "for (recovery, pending_count) in preflighted",
        "recover_pending_autonomous_lifecycle_terminal_outcome(",
        "verify_expected_autonomous_lifecycle_terminal_outcome_stages(",
        "AutonomousLifecycleTerminalOutcomeDurableStage::Complete",
        "pending_autonomous_lifecycle_terminal_outcome_inventory()",
    ];
    let mut remainder = completion;
    for anchor in anchors {
        let offset = remainder
            .find(anchor)
            .unwrap_or_else(|| panic!("deferred completion lost safety anchor: {anchor}"));
        remainder = &remainder[offset + anchor.len()..];
    }
    assert_eq!(
        completion
            .match_indices("verify_expected_autonomous_lifecycle_terminal_outcome_stages(")
            .count(),
        2,
        "deferred completion must directly prove every handoff file before and after mutation",
    );
    assert_eq!(
        completion
            .match_indices("pending_autonomous_lifecycle_terminal_outcome_inventory()")
            .count(),
        2,
        "deferred completion must preflight and finally reject every remaining Pending source",
    );
}

#[test]
fn planner_covered_pending_attempts_are_exposed_for_pairing_but_skipped_for_recovery() {
    let kura_source = include_str!("../../kura/autonomous_lifecycle_terminal_outcomes.rs");
    let covered = kura_source
        .find("if planner_covered.get(&group.reservation_group_hash) == Some(&group)")
        .expect("Kura retains planner-covered Pending validation");
    let inventory_push = kura_source[covered..]
        .find("inventory.push(AutonomousLifecycleAttemptInventoryEntry")
        .map(|offset| covered + offset)
        .expect("covered Pending attempt remains available for signed identity pairing");
    assert!(
        !kura_source[covered..inventory_push].contains("continue;"),
        "exact covered Pending attempts must not be omitted from pairing inventory",
    );

    let lifecycle_source = include_str!("../v2_lifecycle_recovery.rs");
    let first_inventory = lifecycle_source
        .find(".active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(")
        .expect("claimed-generation covered inventory remains source-bound");
    let post_bootstrap = lifecycle_source[first_inventory..]
        .find("// Consume the checked action-25 stutters")
        .map(|offset| first_inventory + offset)
        .expect("claimed-generation projection pass remains independently bounded");
    let projection_pass = &lifecycle_source[first_inventory..post_bootstrap];
    let anchors = [
        "if !current_groups.contains(&identity) || projections.contains_key(&identity)",
        "let planner_paired = planner_paired_groups.contains(&identity);",
        "if planner_covered_groups.contains(&identity) && !planner_paired",
        "let cursor = attempt.cursor().ok_or_else",
        "let projection = if planner_paired",
        "lifecycle_identity_projection_for_cursor(",
        "require_local_producer_queue_owner(payload, cursor, &current_queue_groups)?;",
    ];
    let mut remainder = projection_pass;
    for anchor in anchors {
        let offset = remainder
            .find(anchor)
            .unwrap_or_else(|| panic!("paired projection pass lost safety anchor: {anchor}"));
        remainder = &remainder[offset + anchor.len()..];
    }

    let bootstrap_inventory = lifecycle_source
        .find("let mut bootstraps = Vec::new();")
        .expect("bootstrap inventory remains source-bound");
    let bootstrap_overlap = lifecycle_source[bootstrap_inventory..]
        .find("seen_pending_identities.contains(")
        .map(|offset| bootstrap_inventory + offset)
        .expect("deferred terminal identities preflight bootstrap overlap");
    let bootstrap_custody = lifecycle_source[bootstrap_inventory..]
        .find("authority.custody_source()")
        .map(|offset| bootstrap_inventory + offset)
        .expect("bootstrap custody validation remains source-bound");
    let bootstrap_completion = lifecycle_source[bootstrap_inventory..]
        .find("for authority in bootstraps {")
        .map(|offset| bootstrap_inventory + offset)
        .expect("bootstrap completion remains source-bound");
    assert!(bootstrap_overlap < bootstrap_custody);
    assert!(bootstrap_overlap < bootstrap_completion);

    let mutation_pass = lifecycle_source
        .rfind(
            ".active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(",
        )
        .expect("post-bootstrap mutation inventory remains source-bound");
    let mutation_pass = &lifecycle_source[mutation_pass..];
    let deferred_skip = mutation_pass
        .find("seen_pending_identities.contains(&identity)")
        .expect("post-bootstrap mutation skips every deferred handoff identity");
    let cursor = mutation_pass
        .find("let cursor = attempt.cursor().ok_or_else")
        .expect("post-bootstrap cursor extraction remains source-bound");
    let producer_custody = mutation_pass
        .find("require_local_producer_queue_owner(payload, cursor, &current_queue_groups)?;")
        .expect("post-bootstrap producer custody remains source-bound");
    let recovery = mutation_pass
        .find("if recover_one_attempt(")
        .expect("post-bootstrap Crash/Recover remains source-bound");
    assert!(deferred_skip < cursor);
    assert!(deferred_skip < producer_custody);
    assert!(deferred_skip < recovery);
}

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
