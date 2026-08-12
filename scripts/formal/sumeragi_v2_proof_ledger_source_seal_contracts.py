# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

import importlib.util
import subprocess
import sys


def _load_recursive_reviewed_rust_source_module() -> Any:
    """Load the shared authenticated recursive Rust include resolver."""

    module_name = "_sumeragi_v2_proof_ledger_reviewed_rust_source"
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        return loaded
    path = Path(__file__).with_name(
        "sumeragi_v2_multilane_reviewed_rust_source.py"
    )
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(
            f"cannot load authenticated reviewed Rust source resolver: {path}"
        )
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


_RECURSIVE_REVIEWED_RUST_SOURCE = _load_recursive_reviewed_rust_source_module()

def _retired_sidecar_gate_ttl_source_errors(
    path: Path,
    source: str,
    role: str,
) -> list[str]:
    """Reject any server-request gate TTL identifier in executable Rust."""

    retired_ttl_tokens = sorted(
        {
            token
            for token in rust_code_tokens(source)
            if "ttl" in token.lower()
            and all(
                fragment in token.lower()
                for fragment in ("server", "request", "gate")
            )
        }
    )
    if not retired_ttl_tokens:
        return []
    return [
        f"{path}: retired wall-clock sidecar gate TTL must remain absent "
        f"from production; found identifiers {retired_ttl_tokens} in the "
        f"{role} seam"
    ]


def _require_exact_output_startup_and_successor_rollover_seams(
    lane_path: Path,
    lane_ack_items: dict[str, RustItem | None],
    runner_path: Path,
    runner_ack_items: dict[str, RustItem | None],
    errors: list[str],
) -> None:
    """Check exact startup activation and successor rollover ownership seams."""

    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner"
        ),
        "adapter.hydrate_canonical_lane_artifacts()?;",
        "the production constructor must remain carrier-silent before exact Queue installation",
        errors,
        count=0,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner"
        ),
        "adapter.drive_lane_sessions();",
        "the production constructor must not drive a lane session before exact Queue installation",
        errors,
        count=0,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::activate_after_lane_drain_queue_install"
        ),
        """
if !Arc::ptr_eq(installed_queue, queue) {
    return Err(V2LaneWorkError::InvalidContext(
        "lane-work startup activation names a different queue source".to_owned(),
    ));
}
let installed_queue = Arc::clone(installed_queue);
let output_guard = Arc::clone(&self.output_guard);
let activation = output_guard
    .begin_fail_stop_operation()
    .ok_or(V2LaneWorkError::RestartRequired)?;
self.hydrate_canonical_lane_artifacts()?;
self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;
self.startup_activation_complete = true;
self.drive_lane_sessions();
activation.complete();
Ok(())
""",
        "one-shot startup activation must authenticate the installed Queue, fail-stop hydration, revalidate every local reservation owner, and only then drive lane sessions",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("run_inner"),
        """
let _initial_local_validator =
    local_validator_index(verified_context.context(), &local_peer, config.role)?;
let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(
    config.role,
    kura.as_ref(),
    verified_context.context(),
    &local_peer,
)?;
""",
        "runner startup must classify height-local duty before acquiring one configured-role process generation",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("claim_runner_lifecycle_process_generation"),
        """
match role {
    NodeRole::Observer => Ok(None),
    NodeRole::Validator => kura
        .claim_autonomous_lifecycle_process_generation(
            context.network_id,
            local_peer
        )
        .map(Some)
        .map_err(|error| {
            V2RunnerError::Service(format!(
                "failed to claim the durable autonomous lifecycle process generation: {error}"
            ))
        }),
}
""",
        "the configured-role process generation helper must durably claim validator ownership by passing the authenticated context NetworkId directly with the local identity",
        errors,
    )
    for forbidden_identity_bypass_token in (
        "context.roster",
        "local_validator_index",
        "_initial_local_validator",
        "context.chain_id",
        "lifecycle_chain_id",
        "Hash::new",
        "NetworkId::default",
        "Default::default",
        "synthetic_network_id",
    ):
        _require_rust_token_sequence(
            runner_path,
            runner_ack_items.get("claim_runner_lifecycle_process_generation"),
            forbidden_identity_bypass_token,
            "the configured-role process generation helper must not consult height-local roster membership or derive/substitute a legacy/default/foreign NetworkId",
            errors,
            count=0,
        )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("run_inner"),
        """
reconcile_autonomous_lifecycle_startup(
    state.as_ref(),
    queue.as_ref(),
    kura.as_ref(),
    &context,
    planner_evidence,
    deferred_terminal_recovery,
    _lifecycle_process_generation.as_ref(),
""",
        "startup reconciliation must borrow the exact process-lifetime generation claim",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("run_inner"),
        """
V2LaneWorkAdapter::new_with_output_guard_and_transport(
    &verified_context,
    local_peer.clone(),
    common_config.key_pair.clone(),
    config.role == NodeRole::Validator,
""",
        "the lane adapter must be constructed from the same configured role after startup reconciliation",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("run_inner"),
        "_lifecycle_process_generation.clone(),",
        "the lane adapter must receive the same process-lifetime generation claim",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("run_inner"),
        """
lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
lane_work.activate_after_lane_drain_queue_install(&queue)?;
""",
        "runner startup must install the exact Queue before the one-shot carrier activation seam",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("RetainedMergeSidecars::rehydrate_for_successor"),
        """
if self.successor_context_id != successor.id()
    || self.successor_context_hash != HashOf::new(successor)
{
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff names another successor context".to_owned(),
    ));
}
let authority = DurableMergeSidecarRolloverAuthority {
    _exact_output_handoff: self.exact_output_handoff,
};
self.transport
    .rehydrate_with_exact_geometry_after_durable_handoff(
        reply_source_capacity,
        limits,
        server_stream_capacity,
        server_roster_digest,
        now,
        authority,
    )
""",
        "retained sidecar ownership must bind the exact successor context and consume its durable output handoff before roster-aware rehydration",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
if self.has_pending_committed_output_handoff() {
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff still owns committed lane output".to_owned(),
    ));
}
if self.effect_count() != 0 {
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff still owns undispatched lane output".to_owned(),
    ));
}
successor
    .validate()
    .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
""",
        "lane rollover must prove all committed and queued output empty before validating the immediate successor",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
if !exact_output_handoff
    .is_bound_to_transport_owner(&self.exact_output_handoff_owner)
{
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff belongs to another service/transport owner".to_owned(),
    ));
}
if !exact_output_handoff.matches_predecessor_context(&self.context)
    || !exact_output_handoff.matches_finality_artifact(artifact)
{
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff belongs to another predecessor artifact".to_owned(),
    ));
}
if !exact_output_handoff.authorizes_immediate_successor(successor) {
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff does not authorize the immediate successor".to_owned(),
    ));
}
""",
        "lane rollover must consume only its paired service receipt for the exact predecessor artifact and immediate successor",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
let retained = RetainedMergeSidecars {
    transport: self.merge_sidecars,
    exact_output_handoff,
    successor_context_id: successor.id(),
    successor_context_hash: HashOf::new(successor),
};
handoff.complete();
Ok(retained)
""",
        "lane rollover must bind retained transport ownership to the canonical successor before completing the fail-stop operation",
        errors,
    )


_KURA_PRODUCTION_COMPONENT_FILES = (
    "kura/startup_finality_support.rs",
    "kura/bound_progress_and_retained_support.rs",
    "kura/autonomous_reservation_bounds.rs",
    "kura/certified_bundle_capacity_reservation_types.rs",
    "kura/prune_commit_merge_support.rs",
    "kura/merge_ledger_latest_execution_index.rs",
    "kura/replica_advert_and_body_status.rs",
    "kura/retained_finality_replica_authority.rs",
    "kura/durable_block_and_atomic_sidecar_io.rs",
    "kura/prune_intent_publication.rs",
    "kura/prune_recovery_capacity.rs",
    "kura/block_store_definition_and_test_controls.rs",
    "kura/pipeline_and_lane_artifacts.rs",
    "kura/autonomous_terminal_capacity.rs",
    "kura/autonomous_publication_temp_recovery.rs",
    "kura/historical_autonomous_recovery_temp_reconciliation.rs",
    "kura/hot_path_capacity_preflight.rs",
    "kura/autonomous_execution_view_capacity.rs",
    "kura/certified_bundle_capacity.rs",
    "kura/lane_artifact_budget.rs",
    "kura/autonomous_lifecycle_terminal_outcomes.rs",
    "kura/autonomous_release_authority.rs",
    "kura/autonomous_application_evidence.rs",
    "kura/indexed_sidecar_io.rs",
    "kura/indexed_sidecar_rewrite.rs",
    "kura/lane_history_compaction.rs",
    "kura/test_fault_injection_state.rs",
    "kura/test_fault_injection_controls.rs",
    "kura/file_error_support.rs",
)

_REVIEWED_RUST_INCLUDE_MANIFESTS = {
    "crates/iroha_core/src/commit_roster_journal.rs": (
        "commit_roster_journal/tests.rs",
    ),
    "crates/iroha_config/src/parameters/actual.rs": (
        "actual/runtime_tail_tests.rs",
        "actual/tests.rs",
    ),
    "crates/iroha_config/src/parameters/user.rs": (
        "user/kura.rs",
        "user/governance_dag_head_mode_tests.rs",
        "user/kura_and_snapshot_tests.rs",
        "user/runtime_tail_tests.rs",
    ),
    "crates/iroha_data_model/src/block/consensus_v2.rs": (
        "consensus_v2_tests.rs",
    ),
    "crates/iroha_core/src/kura.rs": (
        *_KURA_PRODUCTION_COMPONENT_FILES,
        "kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs",
        "kura/tests/01_prune_capacity_support.rs",
        "kura/tests/01a_retained_eviction_and_rewrite_tail.rs",
        "kura/tests/02_replacement_and_preflight.rs",
        "kura/tests/02a_unauthenticated_preflight.rs",
        "kura/tests/03_preflight_and_merge_entry.rs",
        "kura/tests/03a_preflight_and_merge_entry_tail.rs",
        "kura/tests/04_merge_log_and_associations.rs",
        "kura/tests/04b_merge_artifact_budget.rs",
        "kura/tests/04c_canonical_association_capacity.rs",
        "kura/tests/04d_prune_intent_capacity.rs",
        "kura/tests/05_merge_resolution_and_eviction.rs",
        "kura/tests/05a_replica_advert_and_body_eviction.rs",
        "kura/tests/06_eviction_and_autonomous_lanes.rs",
        "kura/tests/07a_autonomous_reservation_reconciliation_support.rs",
        "kura/tests/07_autonomous_lanes_and_sidecars.rs",
        "kura/tests/07b_autonomous_reservation_reconciliation_tests.rs",
        "kura/tests/07c_lane_execution_sidecar_tests.rs",
        "kura/tests/07d_strict_lane_ownership_barrier_tests.rs",
        "kura/tests/07e_autonomous_lifecycle_and_canonical_artifact_tests.rs",
        "kura/tests/07e_autonomous_publication_temp_recovery_tests.rs",
        "kura/tests/07e_terminal_capacity_hardening_tests.rs",
        "kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs",
        "kura/tests/07g_claim_capacity_preflight_tests.rs",
        "kura/tests/07h_autonomous_execution_view_capacity_tests.rs",
        "kura/tests/07i_historical_autonomous_batch_capacity_tests.rs",
        "kura/tests/07j_certified_bundle_capacity_tests.rs",
        "kura/tests/07k_historical_atomic_temp_recovery_tests.rs",
        "kura/tests/07l_pending_canonical_capacity_tests.rs",
        "kura/tests/08_lane_receipts_and_artifacts.rs",
        "kura/tests/08a_certified_lane_block_read_tests.rs",
        "kura/tests/08b_lane_history_compaction_capacity_tests.rs",
        "kura/tests/09_lane_artifacts_and_fastpq.rs",
        "kura/tests/10_native_amx_and_roster.rs",
        "kura/tests/10b_native_amx_prepublication_transition.rs",
        "kura/tests/11_roster_and_progress_sidecars.rs",
        "kura/tests/12_sidecar_index_and_pruning.rs",
        "kura/tests/13_manifests_and_fsync.rs",
    ),
    "crates/iroha_core/src/kura/tests/10_native_amx_and_roster.rs": (
        "10c_native_amx_latest_index_support_and_bounds.rs",
    ),
    "crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs": (
        "autonomous_merge_bundle_support.rs",
        "autonomous_reservation_types.rs",
        "autonomous_reservation_inventory.rs",
        "autonomous_reservation_classifier.rs",
        "historical_autonomous_recovery.rs",
        "native_amx_participant_application_artifacts.rs",
    ),
    "crates/iroha_core/src/kura/lane_geometry.rs": (
        "lane_geometry/bootstrap_path_safety.rs",
        "lane_geometry/bootstrap_relabel.rs",
        "lane_geometry/catalog_validation.rs",
        "lane_geometry/retirement_bounds.rs",
        "lane_geometry_tests/00_support.rs",
        "lane_geometry/native_amx_retained_window_tests.rs",
        "lane_geometry_tests/00_retirement.rs",
        "lane_geometry_tests/01_retirement_and_recovery.rs",
        "lane_geometry_tests/02_geometry_moves_and_journal.rs",
        "lane_geometry_tests/03_gc_and_startup.rs",
    ),
    "crates/iroha_core/src/merge_sidecar.rs": (
        "merge_sidecar_signing_guard_tests.rs",
    ),
    "crates/iroha_core/src/queue.rs": (
        "queue/canonical_terminal_cleanup.rs",
        "queue/plan_journal_startup_atomicity_tests.rs",
        "queue/global_guard_claim_conflict_tests.rs",
        "queue/queue_metadata_and_admission_tests.rs",
        "queue/instruction_and_state_routing_tests.rs",
        "queue/routing_batch_admission_tests.rs",
        "queue/teu_limit_and_backlog_tests.rs",
        "queue/routing_projection_resilience_tests.rs",
        "queue/capacity_and_concurrency_tests.rs",
        "queue/pressure_resync_tests.rs",
        "queue/expiry_tracking_tests.rs",
        "queue/inflight_tracking_tests.rs",
        "queue/lane_reservation_tests.rs",
        "queue/lane_reservation_terminal_fault_tests.rs",
        "queue/reservation_recovery_tests.rs",
    ),
    "crates/iroha_core/src/queue/journal.rs": (
        "journal_reservation_commit_preflight.rs",
        "journal_direct_file_io.rs",
        "plan_journal_bounds_tests.rs",
        "plan_journal_replay_tests.rs",
    ),
    "crates/iroha_core/src/state.rs": (
        "state/vpn_lease_validation.rs",
        "state/zk_asset_state.rs",
        "state/passive_lane_diagnostic_methods.rs",
        "state/diagnostic_state_generation.rs",
        "state/autonomous_predecessor_application.rs",
        "state/state_commit_lock_order_tests.rs",
        "state/transfer_transcript_tests.rs",
        "state/block_proof_tests.rs",
        "state/range_bounds.rs",
        "state/deserialize_core.rs",
        "state/deserialize_world.rs",
        "state/default_oracle.rs",
    ),
    "crates/iroha_core/src/snapshot.rs": (
        "snapshot/support_policy_tests.rs",
        "snapshot/write_roundtrip_tests.rs",
        "snapshot/reconciliation_generation_tests.rs",
    ),
    "crates/iroha_core/src/sumeragi/evidence.rs": (
        "evidence/missing_signer_pop_test.rs",
        "evidence/signature_missing_test.rs",
        "evidence/roundtrip_matrix_test.rs",
    ),
    "crates/iroha_p2p/src/network.rs": (
        "network/handle_update_tests.rs",
        "network/queue_depth_tests.rs",
    ),
    "crates/iroha_p2p/src/peer.rs": (
        "peer_handshake_config_tests.rs",
        "peer_state_tests.rs",
        "peer_consensus_mode_test.rs",
        "peer_tests.rs",
    ),
    "crates/irohad/src/main.rs": (
        "main/runtime_deps.rs",
        "main_tests/governance_dag_publisher_binding_signer.rs",
        "main/governance_dag_launcher_tests.rs",
        "main/runtime_budget_and_config_tests.rs",
        "main/startup_tail_tests.rs",
    ),
    "integration_tests/tests/taira_public_localnet.rs": (
        "taira_public_localnet_config_digest_test.rs",
    ),
    "crates/iroha_core/src/sumeragi/mod.rs": (
        "tests/mod_authoritative_runtime_gate_01_support.rs",
        "tests/mod_authoritative_runtime_gate_02_carrierless_replay.rs",
        "tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs",
        "tests/mod_authoritative_runtime_gate_04_routes_and_dequeue.rs",
        "tests/mod_authoritative_runtime_gate_05_ownership_maintenance.rs",
        "tests/mod_authoritative_runtime_gate_06_source_isolation.rs",
        "tests/mod_authoritative_runtime_gate_07_wire_bounds.rs",
        "tests/mod_authoritative_runtime_gate_08_capacity_and_control.rs",
        "tests/mod_authoritative_runtime_gate_09_snapshot_and_source_lanes.rs",
    ),
    "crates/iroha_core/src/sumeragi/status.rs": (
        "status/test_guards.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2.rs": (
        "tests/v2_adapter_activation_context.rs",
        "tests/v2_adapter_04_wal_recovery.rs",
        "tests/v2_adapter_05_direct_lifecycle.rs",
        "tests/v2_adapter_01_replay_and_registry.rs",
        "tests/v2_adapter_02_view_and_lock_progress.rs",
        "tests/v2_adapter_03_tc_and_terminal_ingress.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs": (
        "tests/v2_lifecycle_coordinator_explorer_cases.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs": (
        "v2_lifecycle_replay_authority_payload_projection.rs",
        "tests/v2_lifecycle_replay_authority_fixtures.rs",
        "tests/v2_lifecycle_replay_authority_cases.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs": (
        "v2_lifecycle_work_registry_validate_recovery.rs",
        "tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs",
        "tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs",
        "tests/v2_lifecycle_work_registry_exact_registry_cases.rs",
        "tests/v2_lifecycle_work_registry_replay_evidence_cases.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_runtime.rs": (
        "tests/v2_runtime_unsealed_01b_lifecycle_bounds.rs",
        "tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_worker.rs": (
        "v2_worker/exact_output_rollover_claim.rs",
        "v2_worker/kura_replica_advert_refresh.rs",
        "v2_worker/current_lane_output_rollover_claim.rs",
        "tests/v2_worker_equivocation_and_selected_serve_fixture.rs",
        "tests/v2_worker_reply_route_cases.rs",
        "tests/v2_worker_backpressure_cases.rs",
        "v2_worker/applied_height_handoff_tests.rs",
        "v2_worker/upstream_reply_route_test.rs",
        "tests/v2_worker_nonzero_view_restart.rs",
        "tests/v2_worker_serve_unsealed_cases.rs",
        "tests/v2_worker_serve_decision_restart_cases.rs",
        "tests/v2_worker_certified_serve_budget_cases.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_runner.rs": (
        "v2_runner/height_ingress_bindings.rs",
        "v2_runner/lifecycle_terminal_recovery.rs",
        "v2_runner/finalized_output_rollover.rs",
        "v2_runner/canonical_recovery_ingress.rs",
        "v2_runner/reply_route_retention.rs",
        "v2_runner/merge_sidecar_recovery.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_runner_tests.rs": (
        "tests/v2_runner_unsealed_00.rs",
        "tests/v2_runner_unsealed_01.rs",
        "tests/v2_runner_unsealed_02.rs",
        "tests/v2_runner_upstream_recovery.rs",
        "tests/v2_runner_lifecycle_startup_order.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_apply.rs": (
        "v2_apply/autonomous_recovery_types.rs",
        "v2_apply/historical_autonomous_recovery.rs",
        "v2_apply/reconciliation_authority.rs",
        "v2_apply/committed_carrier_cleanup.rs",
        "v2_apply/error_recovery.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs": (
        "tests/reducer_timeout_and_projection.rs",
        "tests/v2_core_reducer_primitive_projection.rs",
        "reducer/counterfeit_boundary_capability_test.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs": (
        "refinement_constructor_test_helpers.rs",
        "refinement/transition_gate_tail.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs": (
        "refinement_cases/effect_candidate.rs",
        "refinement_cases/terminal_body_pipeline.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/tests.rs": (
        "tests/committee_fallback_and_retransmit.rs",
        "tests/v2_core_view_zero_parent_binding.rs",
        "tests/empty_replay_resume_test.rs",
        "tests/v2_core_terminal_transactionality.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_effects.rs": (
        "tests/v2_effects_03_locked_body_and_sidecar.rs",
        "tests/v2_effects_kura_tip_replay.rs",
        "tests/v2_effects_01_view_churn_and_runtime_steps.rs",
        "tests/v2_effects_02_admission_handoffs.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs": (
        "v2_lane_work/canonical_executed_block_application_repair.rs",
        "v2_lane_work/native_amx_signing_guard_capacity_boundary_test.rs",
        "v2_lane_work/typed_finality_handoff_tests.rs",
        "tests/v2_lane_work_native_signing_guard.rs",
        "v2_lane_work/native_amx_route_and_receipt_tests.rs",
        "tests/v2_lane_work_observer_role.rs",
        "tests/v2_lane_work_native_body_recovery.rs",
        "tests/v2_lane_work_effect_queue.rs",
        "v2_lane_work/historical_recovery_and_carrier_tests.rs",
    ),
    "integration_tests/tests/sumeragi_v2_runner.rs": (
        "sumeragi_v2_runner/restart_timing_test.rs",
        "sumeragi_v2_runner/status_validation_helpers.rs",
    ),
    "crates/iroha_sumeragi_core/src/verus_proofs.rs": (
        "verus_proofs/production_transition_contracts.rs",
        "verus_proofs/in_flight_first_release_proofs.rs",
        "verus_proofs/production_kernel_tail.rs",
    ),
}


def _read_reviewed_rust_source_fixture(
    repo_root: Path,
    relative: str,
    errors: list[str],
    description: str,
    expanded_components: tuple[str, ...] | None = None,
) -> tuple[Path, str]:
    """Expand a non-Git mutation fixture using the reviewed direct manifest."""

    path = repo_root / relative
    if not path.is_file() or path.is_symlink():
        errors.append(f"{path}: {description} must be a regular non-symlink file")
        return path, ""
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{path}: cannot read {description}: {error}")
        return path, ""

    manifest = _REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative)
    if manifest is None:
        return path, source
    if expanded_components is not None:
        unknown_components = tuple(
            component
            for component in expanded_components
            if component not in manifest
        )
        if unknown_components:
            errors.append(
                f"{path}: requested unknown reviewed Rust include components "
                f"{unknown_components!r}"
            )

    masked_source = mask_rust_comments(source)
    include_invocations = tuple(
        re.finditer(r"(?m)^[ \t]*include\s*!", masked_source)
    )
    include_pattern = re.compile(
        r'(?m)^[ \t]*include\s*!\s*\(\s*"'
        r'(?P<relative>[^"\n]+\.rs)"\s*\)\s*;[ \t]*(?:\n|$)'
    )
    observed = tuple(
        match.group("relative") for match in include_pattern.finditer(masked_source)
    )
    if observed != manifest or len(include_invocations) != len(manifest):
        errors.append(
            f"{path}: reviewed Rust include inventory must equal {manifest!r}; "
            f"found {observed!r} across {len(include_invocations)} include "
            "invocation(s)"
        )

    component_sources: dict[str, str] = {}
    for component_relative in manifest:
        component_path = path.parent / component_relative
        if not component_path.is_file() or component_path.is_symlink():
            errors.append(
                f"{component_path}: reviewed Rust include component for {path} "
                "must be a regular non-symlink file"
            )
            component_source = ""
        else:
            try:
                component_source = component_path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as error:
                errors.append(
                    f"{component_path}: cannot read reviewed Rust include "
                    f"component for {path}: {error}"
                )
                component_source = ""
        component_sources[component_relative] = component_source

    def expand(match: re.Match[str]) -> str:
        component_relative = match.group("relative")
        component_source = component_sources.get(component_relative)
        if component_source is None or (
            expanded_components is not None
            and component_relative not in expanded_components
        ):
            return match.group(0)
        return component_source

    return path, include_pattern.sub(expand, source)


def _read_reviewed_rust_source(
    repo_root: Path,
    relative: str,
    errors: list[str],
    description: str,
    expanded_components: tuple[str, ...] | None = None,
) -> tuple[Path, str]:
    """Read one Rust source through the authenticated recursive resolver.

    Production validation always uses the shared stage-aware recursive closure.
    Synthetic mutation fixtures are intentionally not Git worktrees, so their
    already-reviewed direct components retain the narrow fixture reader.
    """

    if expanded_components is not None or repo_root.resolve() != ROOT_DIR.resolve():
        return _read_reviewed_rust_source_fixture(
            repo_root,
            relative,
            errors,
            description,
            expanded_components,
        )
    closure = _RECURSIVE_REVIEWED_RUST_SOURCE._resolve_reviewed_rust_source(
        repo_root,
        relative,
        description,
        errors,
    )
    path = repo_root / relative
    if closure is None:
        return path, ""
    return closure.path, closure.source

@_RECURSIVE_REVIEWED_RUST_SOURCE._reviewed_rust_source_cache()
def _reviewed_rust_include_manifest_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Fail closed unless every reviewed split Rust source has its exact closure."""

    errors: list[str] = []
    reviewed_paths = tuple(
        dict.fromkeys(
            path
            for parent_relative, component_relatives in _REVIEWED_RUST_INCLUDE_MANIFESTS.items()
            for path in (
                parent_relative,
                *(
                    (Path(parent_relative).parent / component_relative).as_posix()
                    for component_relative in component_relatives
                ),
            )
        )
    )
    try:
        tracked_result = subprocess.run(
            ["git", "-C", str(repo_root), "ls-files", "--stage", "-z", "--", *reviewed_paths],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        errors.append(
            f"{repo_root}: cannot authenticate reviewed Rust include Git tracking: {error}"
        )
        tracked_paths: set[str] = set()
    else:
        if tracked_result.returncode != 0:
            detail = tracked_result.stderr.decode("utf-8", errors="replace").strip()
            errors.append(
                f"{repo_root}: cannot authenticate reviewed Rust include Git tracking: "
                f"{detail or f'git ls-files exited {tracked_result.returncode}'}"
            )
            tracked_paths = set()
        else:
            tracked_paths = {
                record.split(b"\t", 1)[1].decode("utf-8", errors="surrogateescape")
                for record in tracked_result.stdout.split(b"\0")
                if b"\t" in record
            }
    for relative in reviewed_paths:
        if relative not in tracked_paths:
            errors.append(
                f"{repo_root / relative}: reviewed Rust source must be Git-tracked"
            )
    for relative in _REVIEWED_RUST_INCLUDE_MANIFESTS:
        _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "reviewed split Rust source",
        )
    return errors


def _kura_production_source_inventory(
    repo_root: Path = ROOT_DIR,
) -> tuple[
    Path,
    str,
    tuple[tuple[str, Path, str], ...],
    list[str],
]:
    """Load the exact direct production include closure of ``kura.rs``."""

    source_root = repo_root / "crates" / "iroha_core" / "src"
    kura_path = source_root / "kura.rs"
    errors: list[str] = []
    if not kura_path.is_file() or kura_path.is_symlink():
        errors.append(
            f"{kura_path}: Kura production source inventory root must be a "
            "regular file"
        )
        kura_source = ""
    else:
        try:
            kura_source = kura_path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(
                f"{kura_path}: cannot read Kura production source inventory "
                f"root: {error}"
            )
            kura_source = ""

    structural_source = mask_rust_comments_and_literals(kura_source)
    exact_test_module_markers = tuple(
        re.finditer(
            r"(?m)^#\[cfg\(test\)\]\n"
            r"pub\(crate\) mod tests (?P<open>\{)[ \t]*$",
            structural_source,
        )
    )
    test_module_candidates = tuple(
        re.finditer(
            r"(?m)^#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*"
            r"(?:(?:pub(?:\s*\([^\r\n)]*\))?)\s+)?"
            r"mod\s+tests\s*\{",
            structural_source,
        )
    )
    if len(exact_test_module_markers) != 1 or len(test_module_candidates) != 1:
        errors.append(
            f"{kura_path}: Kura production source inventory must retain "
            "exactly one terminal cfg(test) module boundary in the exact "
            "`#[cfg(test)]\\npub(crate) mod tests {` form; found "
            f"{len(exact_test_module_markers)} exact boundary marker(s) "
            f"across {len(test_module_candidates)} cfg(test) tests-module "
            "candidate(s)"
        )
        production_source = kura_source
    else:
        test_module_marker = exact_test_module_markers[0]
        production_source = kura_source[: test_module_marker.start()]
        depth = 0
        test_module_close = None
        for offset in range(
            test_module_marker.start("open"), len(structural_source)
        ):
            token = structural_source[offset]
            if token == "{":
                depth += 1
            elif token == "}":
                depth -= 1
                if depth == 0:
                    test_module_close = offset
                    break
        if (
            test_module_close is None
            or structural_source[test_module_close + 1 :].strip()
        ):
            errors.append(
                f"{kura_path}: Kura cfg(test) tests module boundary must own "
                "the terminal source suffix"
            )

    include_parse_errors: list[str] = []
    include_invocations = _RECURSIVE_REVIEWED_RUST_SOURCE._rust_include_invocations(
        production_source,
        kura_path,
        include_parse_errors,
    )
    errors.extend(include_parse_errors)
    observed_components = tuple(
        invocation.relative for invocation in include_invocations
    )
    production_test_components = tuple(
        component
        for component in observed_components
        if component.startswith("kura/tests/")
    )
    if production_test_components:
        errors.append(
            f"{kura_path}: Kura production source must end before all test "
            f"includes; found {production_test_components!r} before the exact "
            "terminal cfg(test) module boundary"
        )
    if (
        observed_components != _KURA_PRODUCTION_COMPONENT_FILES
        or len(include_invocations) != len(_KURA_PRODUCTION_COMPONENT_FILES)
    ):
        errors.append(
            f"{kura_path}: Kura direct production include inventory must equal "
            f"{_KURA_PRODUCTION_COMPONENT_FILES!r}; found "
            f"{observed_components!r} across {len(include_invocations)} "
            "include invocation(s)"
        )

    components: list[tuple[str, Path, str]] = []
    for relative in _KURA_PRODUCTION_COMPONENT_FILES:
        path = source_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: Kura production source inventory component must be "
                "a regular non-symlink file"
            )
            source = ""
        else:
            try:
                source = path.read_text(encoding="utf-8")
            except OSError as error:
                errors.append(
                    f"{path}: cannot read Kura production source inventory "
                    f"component: {error}"
                )
                source = ""
        components.append((relative, path, source))
    return kura_path, kura_source, tuple(components), errors


# Exact token-stream digests for the nontrivial Verus projection theorems and
# their concrete mutation witnesses. Unlike raw substrings, these bind the
# declaration, contract, and proof body of one real, context-checked item while
# remaining insensitive to comments, literals, and formatting.
_PRODUCTION_CAUSAL_FIFO_VERUS_ITEM_SHA256 = {
    "production_stable_subsequence": (
        "820ed299168153274ca3457e3446bd8bd2c433b49286bd9f5e4c57975af252ba"
    ),
    "production_fresh_causal_successors_excludes_prior_owners": (
        "b6236603215e29cabfc1a44d7495ad101d0390f23863127714354bb2d89b0c56"
    ),
    "production_fresh_causal_successors_keeps_every_fresh_value": (
        "0ff2376b08c3a96cb0c82f3d87404796a4d863f919c9cf58ddee5858c3ebf898"
    ),
    "production_fresh_causal_successors_has_unique_values": (
        "030db5f75b6cbddc32ba57aa8eac66169b993b3b067105232005d0985ee83daa"
    ),
    "production_fresh_causal_successors_preserves_first_owner_order": (
        "14ef7781949552882dda3fb35f066ea89e0a943bb40dcca2b7464b0a05d81109"
    ),
    "production_async_causal_fifo_after_batch_preserves_fresh_tail": (
        "ef11350ee44c0551a2a34f091f3197559a684075b3c86a28d0d4a9528acc6a6e"
    ),
    "production_inverted_owner_filter_mutant": (
        "550fa68ae21e42cf9bd1c9756e229ed40b23e28ebc417cd7907d9b805e634bec"
    ),
    "production_inverted_owner_filter_mutant_is_rejected": (
        "898288a0767a678bd7080abe09995de2ec2e7c406c796ab79202b28e200d0a41"
    ),
    "production_reversed_fresh_order_mutant": (
        "e14bf30fb8489b4922e8f81c79e305a0f2c07d57f2abd3101bff581c915760a7"
    ),
    "production_reversed_fresh_order_mutant_is_rejected": (
        "a0b995d9442eb0fef330eb7383c258f16dab35b61b84e02716437f758214a6f5"
    ),
    "production_completion_capacity_product_rank": (
        _CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_SHA256
    ),
    "production_completion_capacity_product_rank_descends": (
        _CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_DESCENT_SHA256
    ),
}

_PRODUCTION_EFFECT_CANDIDATE_TLA_OPERATOR_SHA256 = {
    "AsyncCausalCandidateLifecycleCapacity": (
        "02882bb4b0f0b52c5e9d0994bdb8f459b768e7612851b68956932bbaecf93591"
    ),
    "AsyncCandidateCausalOrigin": (
        "00a780f6b919c66fc03507311341ff7590f4d37cba9bc4711ee73ec29e23eeef"
    ),
    "ExactAsyncCandidateIdentity": (
        "ceeaebbac450ef309c949f0fc399393ea441763fbb91948bdd5bb8c89f9a165f"
    ),
    "FreshCommandSuccessors": (
        "387d64f773b0d6df95e41ce65e108bd37e23c6b3574ecdd0f049b242b2235997"
    ),
    "AppendCausalSuccessors": (
        "f4c500c716af8d23357691a07a603700fa1d762b2192a2f310845ea021bc5781"
    ),
}

_PRODUCTION_EFFECT_CANDIDATE_TLA_THEOREM_SHA256 = {
    "CommandSuccessorsRetainCausalOrigin": (
        "cf52b0688a75691dc706a787f2f776f71d929013d82566ddaead6cfb36602247"
    ),
}

_PRODUCTION_COMPLETION_CAPACITY_TLA_OPERATOR_SHA256 = {
    "Stage6CompletionCapacityGoal": (
        "c983618885f38d74a140a2e1d03f0e9e2e1721553db0e0e457efbf67dbc41dde"
    ),
}

_PRODUCTION_COMPLETION_CAPACITY_TLA_THEOREM_SHA256 = {
    "FairStage6CompletionCapacityOpens": (
        "d6395e9f9616aca03edbd2dffde7e3aa340e80e3be9fe333f8c4a362ef2e21c1"
    ),
}

# Exact comment/literal-free token digests for the production Rust items whose
# control flow establishes the bounded persisted-continuation, one-transition
# deferred scheduling, and TC recovery seam.
# These whole-item bindings prevent an attacker from preserving the reviewed
# snippets in a dead branch while changing the executable path around them.
_PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256 = {
    "from_record": (
        "399605c9add8c3bf579fc03c6b46b52533c1ca726cc175fcd80cd33536882ce7"
    ),
    "budget": (
        "db8200b278278f301d4a254f04ca49d42353b486baac55c104cb4461ba5d125f"
    ),
    "deferred_work_is_serviceable": (
        "1036f93b127acad65512fcb095d7f19bdc547f93d22ebebbd227277a0a30405c"
    ),
    "completion_unblocks_deferred_fence": (
        "0964e3e5d76ef3f72a240c03c46c3c15a71c52572acaa34784276c97858d3ab7"
    ),
    "command_is_blocked_by_deferred_fence": (
        "069222d7ba0857ef873a6784ccd4cbc8ecd38882a7914a1cb7d87d27ec5ec7d6"
    ),
    "authenticated_command_reaches_fenced_reducer": (
        "75b1f243e311b75994e8caf13f64192a4672359b9a7daff0c7d48f674e7f66b9"
    ),
    "ready_to_finish": (
        "039df6143d8b4505489db3c75ab2d3d24c01a30a4da0f0d277d865f44cfa0ffa"
    ),
    "drain_deferred_with_evidence": (
        _SERVICED_CANDIDATE_PRODUCTION_ITEM_SHA256[
            "drain_deferred_with_evidence"
        ]
    ),
    "fail_deferred_service_contract": (
        "b3ca797528869db777b292d74ccba9425cced620e3d56f429ffdeb624b285799"
    ),
    "step_reducer": (
        "c9c5192fe1e4b7042a76aced8da8fcbafe51d4ed4a0f5ad6608c0b81492864a6"
    ),
    "drive_effects": (
        "7493eed9736bb149f208b8990ba5f1b69d1b9aa6645dc8c2ded7c91a87a20255"
    ),
    "pop_fence_dependency_with_ownership": (
        "cf5a9312b16db72176857a1dfad4e7b64636f7bebd7edc7142f052ec3a2e567b"
    ),
    "fence_blocked_occurrence_owners": (
        "45b2891248d62b2a998ebcaf2e29d35bfeb1687a8869a0b27776904ee6b3f778"
    ),
    "freeze_due_clock_owners": (
        "d28538a60f9391277b1db6c60b71ed694c1d776de425305c52f8703450ebae85"
    ),
    "validate_clock_owner_physical_cuts": (
        "0c32bff217ddf2b631eba89320a3b6fad6dda20830ed9e0ed7778ab47d2185d6"
    ),
    "clock_owner_reservation_blockers_occurrence": (
        "940ef5e22bf2e2a1d9ef78fca48d2517d6294259b1b8108892420d0de78ba34e"
    ),
    "clock_owner_reservation_blocks_occurrence": (
        "9fa4051663d34eab5a8ecbfc1192ca92789ca710085e95726a796a29b20450da"
    ),
    "clock_owner_reservation_blocks": (
        "f6db1825402a97f52f35c0ef7cabda380be9055ca7507dfae0476d339057304c"
    ),
    "enqueue_after_clock_reservation": (
        "d71a57ec0eb41724bb65040a35e9d0ac308a2ef387868c207070add6e02be541"
    ),
    "scheduler_arbitration_inputs": (
        "926499ad5bca5e49d512774f85206ab108748834ee27f4864cb88b081d7fdc36"
    ),
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "61dfb5c5bed49fe8191be06bd09a2e929f70da89b1ff829add86f8966fd90f01"
    ),
    "minimum_active_lifecycle_ordinal_for_deferred": (
        "1d27b616e3804e98006a4353d9c7d1e03005ce9299c2e0b0ccbb7782209210df"
    ),
    "minimum_active_lifecycle_ordinal_for_deferred_excluding": (
        "eca0b361eb0d72cf5e92e3d38563e04efe062ccde0ffd0db0d75942a40e12505"
    ),
    "eligible_deferred_admission_ordinals": (
        "ca2db5b8e601e556e10001d3c0a6be5b18dd1cd1f92e5eecf0273fe158fb232d"
    ),
    "runtime_step": "bafd283fd50fe929e000481a8314f98cd0ad3aef30c8e8677a93b0784045136c",
    "runtime_step_recovery": (
        "818947b3b1356bfe825b34f2b4ee35f8293b24d9a12ef21fdd0d4f5d97c4ef0e"
    ),
    "dispatch_one_adapter_deferred": (
        "a4c901cdd676731f6cfd3c4dcb52718df366f65bc7ca5e8d1a54a841ec30cdab"
    ),
    "dispatch_one_fence_dependency": (
        "539239fa96fca8ea08dc56ac89041b7be0bb6f5f3d33f65259ad8e7833173b69"
    ),
    "real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo": (
        "682c243806ba2c813238722e16e9506ad1534fac572a18c29a3d270a9157ec51"
    ),
    "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers": (
        "93f8290d3a87f463bc2b99d13a9585edc988794ce18f5d2a2e3d80282a79e2ca"
    ),
    "tc_promoted_lock_requires_same_subject_reproposal_before_commit": (
        "1534b682d7dc004f6df81426ce28004c74401da7ecd0559ec2a7cf38958e07b1"
    ),
}

# Structural inventory for the process-local capability chain which binds an
# exceptional fence/pacemaker selection to one immutable physical queue
# occurrence and one exact signer incarnation. Exact token digests are kept in
# the reviewed SHA-256 tables once refreshed; this inventory independently
# prevents a helper from disappearing from the proof-ledger fidelity pass.
_PRODUCTION_CAUSAL_FIFO_NONFORGEABLE_HELPER_CHAIN = (
    "cached_queue_occurrence_owner",
    "ownership_snapshot",
    "mint_selection_seal",
    "contains_queue_occurrence_owner",
    "restore_selected_command",
    "oldest_active_lifecycle_ordinal_before_physical_cut_excluding",
    "physically_eligible_deferred_admission_ordinals",
    "pop_pacemaker_progress_with_ownership",
    "runtime_queue_occurrence_owner_projection_hash",
    "runtime_queue_ownership_snapshot_projection_hash",
    "runtime_queue_occurrence_set_matches_snapshot",
    "runtime_queue_selection_seal_projection_hash",
    "matches_scheduler_occurrence",
    "runtime_scheduler_projection_hash",
    "current_signature_fence_identity",
    "clear_fence_retry_blocked_fifo_owners",
    "reconcile_fence_retry_blocked_fifo_owners",
    "retain_fence_retry_blocked_fifo_owner",
    "retain_scheduler_ownership",
    "try_step_pacemaker_escape",
    "dispatch_one_pacemaker_progress",
)

_PRODUCTION_CAUSAL_FIFO_NONFORGEABLE_ITEM_SHA256 = {
    "cached_queue_occurrence_owner": (
        "a113678ffffef7c7bfae9ba827bfddff80e069dd1c1e84d9dbb5525e99138103"
    ),
    "ownership_snapshot": (
        "b46fe089ce520f215926646c49fb991755ae3e2ba6b4dda5242808df6f8d0645"
    ),
    "mint_selection_seal": (
        "981e90ec1cfb8388e1e6058874ecab69d8e415b7319890c5587c683e4dec7568"
    ),
    "contains_queue_occurrence_owner": (
        "c116a2a5423be69a43d6dbf13c00525a416f974020cd96ad7f55282a27293a98"
    ),
    "restore_selected_command": (
        "af1497f566c0c9547983f23469490478ecb322286899a44d7f2761dd668af3a4"
    ),
    "oldest_active_lifecycle_ordinal_before_physical_cut_excluding": (
        "469e352d63c31ef4776fa31bfce7484c16a33f00982058005317494fd87f9e17"
    ),
    "physically_eligible_deferred_admission_ordinals": (
        "0b51819b618a5d50f632ffb772a94be1560e960b57746c10ba3e077aa97b60d3"
    ),
    "pop_pacemaker_progress_with_ownership": (
        "10beefbc980f21e6eb3e07de7153be4e72568a323908211afe1042335125eca9"
    ),
    "runtime_queue_occurrence_owner_projection_hash": (
        "c220a852adcdb9e7a2a20cec297ee79a87cf5b88acac55410eac3c824614d1a6"
    ),
    "runtime_queue_ownership_snapshot_projection_hash": (
        "bb6d0062be867b73c5269979c4f850bc9614e8f15c76f65c60dbc9e44d497eb9"
    ),
    "runtime_queue_occurrence_set_matches_snapshot": (
        "c89fd5e9e3df83fb51e201555e53799c625c9978932fb61be844476ad2c977be"
    ),
    "runtime_queue_selection_seal_projection_hash": (
        "b9036732219388125a51d4652ce4143aaefe00574d7f9f9f359cc90b76b56368"
    ),
    "matches_scheduler_occurrence": (
        "355052d58f6acbf00d4c7164d0e54a3e13044a087150cd4d56643110f2d51ed6"
    ),
    "runtime_scheduler_projection_hash": (
        "bcee96f000cf6652240fe9dbfa5411f667e3979dc156710e3b1803bc88c63e08"
    ),
    "current_signature_fence_identity": (
        "d3e7b714af5442d66ec2e8f4e1c57bfe649c5b8f41b310d71929fa6586baf0ed"
    ),
    "clear_fence_retry_blocked_fifo_owners": (
        "2ce5c0a06b37a5342eaa297e3fef4ae70ea0856344f03c8eaaf08fe5488097ae"
    ),
    "reconcile_fence_retry_blocked_fifo_owners": (
        "d603d692ea6053b116efce0eb77fb0872e2f0277480239a5a4183583015de370"
    ),
    "retain_fence_retry_blocked_fifo_owner": (
        "f6c5e5ac82dfda26d972715a073a2ddb7a41135ef9be0ee02ed77944f9e186f8"
    ),
    "retain_scheduler_ownership": (
        "4addd609c67402a1a5de9704e093e23bb8a7ea6cef2c2477a9adb6e2aedc5cb6"
    ),
    "try_step_pacemaker_escape": (
        "aa0a41501d13d502e119566bb4fc55e202f820ece97cf66e486ac851b458c64d"
    ),
    "dispatch_one_pacemaker_progress": (
        "fc7d5f12b41f703242826e8926dd91f4267b78d775b1d8c2453d85006a3ac965"
    ),
    "occurrence_owner_validate_exact": (
        "0f018cbe8ab36b2a5e1f3aa128d7c22c367083b399184aedc8c829f9228db8e3"
    ),
    "occurrence_owner_matches_queued": (
        "e8544136325028c614b22287649c3e7af5779669328b0b64918093edabb8302a"
    ),
    "queue_snapshot_validate_identity": (
        "9ee5bf23ffa531c168a3722a216ef32a9c2cf83f4265e25dde0fe750b4fa890f"
    ),
    "queue_selection_validate_identity": (
        "18019651786d6151ee975cc68de9f43f2e9ece72e979d3957fb67acbb7c09774"
    ),
    "queue_selection_matches_scheduler_occurrence": (
        "355052d58f6acbf00d4c7164d0e54a3e13044a087150cd4d56643110f2d51ed6"
    ),
    "scheduler_evidence_validate_exact": (
        "04da1f906e4e197bad4e49e962a92ae7d99c2749f6aecfdfe28f256c4426a188"
    ),
    "adapter_pacemaker_escape_is_parked": (
        "5513bb3477396c268d3dc7ed75ded1d6c027bc5780c5f129b53920720c498d86"
    ),
    "adapter_signature_fence_is_active": (
        "2012f7befb770adf8ab34a94452c592836dc3ae6c8e4e8ca4c3148f745e4db38"
    ),
    "adapter_signature_fence_identity": (
        "3bfa48370472e85c7727695165fae3c6758a4a9ff228111c806e558929f7faad"
    ),
    "runtime_driver_certified_progress_bypasses_signature_fence": (
        "6c6761d3f63a292cf04fd123fc055891200081b7dda8f211482f58cfef7b385d"
    ),
    "runtime_driver_completion_unblocks_deferred_fence": (
        "1379d6ae2714aa4bfc2b02cec74611a0f65799319ab94b8fc37b04ef0fec62a5"
    ),
    "runtime_driver_command_is_blocked_by_deferred_fence": (
        "63b773eb42e48fc13be96b5aecd992d9e23a264ba28d0720e450eace4578dc5d"
    ),
    "runtime_driver_default_command_matches_deferred_authenticated_owner": (
        "f7890a72aa8fe05e4f16321614d6fc96b31de12353b923fe34724a1e46109b61"
    ),
    "runtime_driver_pacemaker_escape_is_parked": (
        "4ed5db3024338e14145b7186c4392feb1c14348e887f3695d813f980ad6680e7"
    ),
    "runtime_driver_signature_fence_is_active": (
        "34aa7d6b66cf8ad39f348f2b0ebecd773f4c771867951769222215f5ab877294"
    ),
    "runtime_driver_signature_fence_identity": (
        "135fcb20dbb91a2adfa807031ee70a6b11e396b8369959dbeca22a8aba2718f7"
    ),
}

# Exact comment/literal-free token digests for the production ownership bridge
# which retains the complete canonical envelope and fair-ingress carrier from
# authenticated admission through Busy-deferred service.
_RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256 = (
    "422f7ae170b202c5023c98f94182a4526eb883c39e7576d3c1e0ba69f0723958"
)
_RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256 = (
    "b80fda2727730a33aa51875e3681e9cf355fc0394943b170a4793c2a78f46e23"
)
_RUNNER_DRAIN_V2_INGRESS_ITEM_SHA256 = (
    "9a218b6c25e62bb63fe0ced59d5a0a4ab65576d8dd692485068da8e02541e704"
)
_RUNNER_ROLLOVER_FINALIZED_HEIGHT_OUTPUTS_ITEM_SHA256 = (
    "7049c460f181dbf4b32b3ad153387c0ebd79cf271347b4de39a55502883c686d"
)


_AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256 = {
    "matches_authenticated_runtime_bytes": (
        "1d5bd7b516504865f7f8f8db0416c7c1d337ba83342cbd66b564b24e46df3870"
    ),
    "deferred_authenticated_message_owner": (
        "b8279dae9cd51a72cc4a84cd80063fc5a90b2ca72066b48f59571166b560cc8b"
    ),
    "authenticated_deferred_admission_ordinals": (
        "56e2d8e09f770616ace0c2421e6105c7ee2cf8d7428b4016003b5aa5b71bf271"
    ),
    "deferred_authenticated_event_matches_wire": (
        "71cff12249ba75d45cc55f3be85c966fa2f317a3638ce36fa250d399c0f88fd5"
    ),
    "wire_ingress_missing_execution_commitment": (
        "ab4345a4067a48f67735cb5867d717cf8f32aa809f7218aeb04c8eaaf3775678"
    ),
    "runtime_ingress_from_fair_ingress": (
        "638d44eae201d3477a987e857d3c9318c7a525347cddb9d5bbce704d9bbc7985"
    ),
    "runtime_ingress_validate_exact": (
        "87a8e78b3de06372da6678ca45737fa8874d5b9c3aac537d0db55b7848715a41"
    ),
    "runtime_ingress_leader_wire_physical_carrier": (
        "964b47b38e48c18e93b0a4e9d63ac711f5ef14ed89d933ffbe1e93233d59d7c5"
    ),
    "runtime_ingress_is_physical_leader_wire_replay": (
        "7af77803b6cb064c9f611601e930c2ee13db7e961a5c85fad10a80900299792a"
    ),
    "runtime_ingress_earliest_physical_carrier": (
        "aab95bf9cc5928cda4550fa375c56f8958b473940ab865b7527946eb5f235325"
    ),
    "runtime_ingress_contains_physical_carrier": (
        "6554701b2dcc0d85a2006eabf89f0c538526fd99cd9a78456c031d79c5aec1bf"
    ),
    "runtime_ingress_validate_frozen_physical": (
        "51990098b63ed265a79a1617ff4e22c54a1a9eb740efea948df9950ba8f4a1b5"
    ),
    "runtime_ingress_exactly_matches_authenticated": (
        "bd5cfafeb6e9ea0c7a2bd11e92a3cf9e49e8b952bd33586389ff01d751cf8d57"
    ),
    "runtime_ingress_can_merge_downstream": (
        "0a9b6424c76b8d53a6e630431e3e9153d20e25d755abbe38384c5f59e9064553"
    ),
    "runtime_ingress_merge_downstream": (
        "55f59286a356c0cd1c47b572db72660dcdedcc40f3946c2e3b62c7eb5c805e92"
    ),
    "runtime_driver_dispatch": (
        "cc2e5b6036d7e6107d052e4e4bf3b87355fff389b3749b69a7fbf643e63629ad"
    ),
    "runtime_driver_dispatch_deferred": (
        "f80c271a6766106b620e6b0dcbd7fb3a37db63fece634c09818ddb74498202d9"
    ),
    "runtime_driver_deferred_occurrence_ownership": (
        "be42df5e4cfeb06f2887a6d713b06c4a0edb4acc15e6f1bafe8e1454bf58ea91"
    ),
    "runtime_driver_seal_deferred_runtime_ownership": (
        "2948fe28ee3912010c5f0628e6688a329de631c8c5c3f9456178c17e7f58358a"
    ),
    "runtime_driver_command_matches_deferred_authenticated_owner": (
        "8b4d64222b3f18e244c28e62aeb60075cb7995d920d55e08c67540db0a663fa8"
    ),
    "runtime_ingress_physical_ownership_validate_exact": (
        "488ba0d5d7896c09a4966d90d16c831090f516c42ab213c70c62aead6b49f0b5"
    ),
    "runtime_deferred_lifecycle_validate_exact": (
        "63e7de646d2eb0dcbc7c9e8aaa87562e44ee3bb007b3185e51237b0b554e4cb3"
    ),
    "runtime_deferred_lifecycle_validate_against_ingress": (
        "5584747a03531f1709f299d16b7cd1884becd7cbd5a37c7619777277112b4e30"
    ),
    "runtime_deferred_lifecycle_validate_active_against_ingress": (
        "0ae0c8bde63fd29b4732f301498515f9444a9991f8008aaf3fc23a9a27583918"
    ),
    "runtime_deferred_lifecycle_rebase_deferred_ingress": (
        "981bd3b563516a04eb013b82fbacc7017b4d3a8df16756bcf5c5d7cd72808de4"
    ),
    "reconcile_deferred_ingress_ownership": (
        "37ebb9c8e38cf6d1eda06a31a83570c40f45771f0847adea36d3305d7a6d38bc"
    ),
    "reconcile_deferred_runtime_ownership_after_retirement": (
        "dd51598b026d3c5bc97abfcf934b04438ac3f57f1f7577fa6faac4c084953603"
    ),
    "active_leader_wire_runtime_ordinals": (
        "60659f35ec4d592990db68069fb1373bc2af662711fb11a14db1078d8b395dc6"
    ),
    "retire_orphaned_leader_wire_runtime_receipts": (
        "9c4e92b725c4da1cf83dcb82ae6f76b8d9906ec4fbcdca1117dfadef6024bd7d"
    ),
    "register_leader_wire_runtime_receipt": (
        "58838980b5f6eac5ad28b4e40af9bbfaab41e4d2a32e5215fbe918a890e6ec11"
    ),
    "complete_leader_wire_runtime_owner": (
        "e17e62beccb6e2e219f3aac01c126456531ffb0448930b83026d9cf02da6695c"
    ),
    "complete_driver_dispatch_leader_wire_owners": (
        "6fccf9497abffa385d06a6e4f4fd9a0f641745ca51b17f8b6e7dcede26c449eb"
    ),
    "accept_driver_dispatch": (
        "33e443dbffe23a764940bf525a0a65a2539a96af0616dd954ec0c9ce0906207b"
    ),
    # The public enqueue alias is shared with the timeout-episode inventory;
    # the two admission predicates retain independently reviewed seals.
    "enqueue_network_with_ingress_ownership": (
        _RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256
    ),
    "can_admit_pre_runtime_leader_wire": (
        "c87bffb9117487d2697e1cb242859927e0083d8c965674acc9b475a56b50d360"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "338ddd65e84755b37a84a618375f27631ddabc72e478b2405ee7f9904b5ecad3"
    ),
    "take_last_scheduler_ownership": (
        "b781f7ace9823e4ba2b395230912a703a78c2b6ae8fb48e96a0f0f120c9fa7c8"
    ),
    "network_admission_uses_exact_normal_and_progress_reservations": (
        "1655bf5a50867d34cd23e13fb0ac771ae471f4ccf4c8a59383702f307bfe2834"
    ),
}

_PRODUCTION_CAUSAL_FIFO_RUNTIME_REGRESSION_SHA256 = {
    "deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences": (
        "bdd92f55342073ef1a709fbf8400708056ea2042c3a0904fd1f2993993b3298d"
    ),
    "later_same_semantic_fair_retry_retains_runtime_lifecycle_root": (
        "12e41e90bcd59ef22389131ad4b5cdeb56dce7fa04cd15357d18ce9f62cb1b6a"
    ),
    "ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it": (
        "b69ec152b545fb4e705335eeaecaa5785b1736349fa1494b383f18c257aa8d18"
    ),
    "older_frozen_aggregate_carrier_rebases_queued_runtime_minimum": (
        "77dbbfac307f2bd98b86d1d9d111579c40b30fed6254da221544dacf1df8a7ba"
    ),
    "network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals": (
        "878269e9bdf567147360a1f3c2bf5a7def18b7abd1b38aed3978dc7923624291"
    ),
    "distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner": (
        "9fd964c7159754e3a01d6d905f40423fb1fb4d573f033f031b3497b02969c1d4"
    ),
    "pacemaker_escape_coalesces_prequeued_distinct_origin_prepare_qc_into_live_busy_producer": (
        "5ae5c533c0498b9d067634d88722b736f52ff78c6a63983111a390c36c3603fe"
    ),
    # This alias is shared with the timeout-episode regression inventory.
    "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner": (
        _RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256
    ),
}

# Exact comment/literal-free token seals for the finite current-view
# TimeoutVote recovery episode. Each entry is approved only after its focused
# positive baseline and negative mutations pass together; no digest in this
# inventory may be bulk-refreshed.
_TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256 = {
    "ingress::fair_v2_ingress_is_direct_validator_timeout_vote_owner": (
        "be2e39948e3696646e15c3331e020351b8f5065ecb25d9aef466674afc18b648"
    ),
    "ingress::try_recv_if_checked": (
        "091f57ccb6adaafd50864565891f636b364658cb8ed70cc5254d521901779a82"
    ),
    "ingress::try_recv_if_checked_retiring_obsolete": (
        "06c5d041e60dd73dbddb63a9e4600772d7ff1e80d250255612a18a3ecb7b7733"
    ),
    "ingress::try_recv_if_checked_retiring_obsolete_with_barrier_bypass": (
        "36e0d2de5bd03d5f3547ab4e7b9be43a74d51b7abfb2f4ca72753171f8605bf6"
    ),
    "ingress::try_recv_if_at_checked": (
        "ab9e90e132fbd369c255a7982b5ceab98e74ed3b535cc494b0b12a57a53bf81f"
    ),
    "ingress::try_recv_if_at_checked_classified": (
        "234399223cc9b36bd4c6f3be6dd29040fd0761b67b4a1c38f4fcdd40bf79ac19"
    ),
    "ingress::fair_v2_ingress_queue_gate_verdict": (
        "c867fbfccf0d45fff2757bfbec97655de382b225ce9adada9f451e01c3e38e8d"
    ),
    "runner::run_inner": (
        "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe"
    ),
    "runner::drain_v2_ingress": (
        _RUNNER_DRAIN_V2_INGRESS_ITEM_SHA256
    ),
    "runtime::RuntimeTimeoutVoteEpisodeOwner::validate_against": (
        "fb2a98e36b7014a4199ae78eda34119f69cec26fc25fff2d4ce5758f41d74777"
    ),
    "runtime::RuntimeTimeoutVoteEpisodeOwner::same_lifecycle_owner_as": (
        "4c7e987d86308f3f71c36a9c91f9f4dfdda750f411382dd5934fbb61554668bb"
    ),
    "runtime::RuntimeTimeoutVoteEpisodeAdmissionPlan::count_transition": (
        "63923a863bfa409746d6f5efe7a5609913a9ebd19ffe98d65281a2e9a94f310f"
    ),
    "runtime::emitted_timeout_recovery_owner": (
        "62364351b1c0cbc70ec583b61313e53a374b60fc83e4b4257beefcd35c953fa5"
    ),
    "runtime::timeout_recovery_episode_allows_clock_blockers": (
        "4c52b5a78486a535d247919079bb60090ace9ba4b77e1759763aec0f48627f55"
    ),
    "runtime::timeout_vote_recovery_candidate_from_fair": (
        "8a3399864c9016a838a2c64df3ac10e1642e9acdf9275e7b054c0160e7905072"
    ),
    "runtime::timeout_vote_recovery_candidate_from_runtime": (
        "edbe98f16b7a80cb790a6eee1c0a3026c7e6030e005aeaf0d4ef81de45403594"
    ),
    "runtime::timeout_vote_recovery_candidate": (
        "46f46e7ce5eae9a06ff23fbf54c582addeccc5bfe5cef9372e54c1a6053ceda0"
    ),
    "runtime::timeout_vote_episode_admission_plan": (
        "a4e4b2455b2c355c57bdeab05470e4a700afa498980adb02ed31504f75ed01ec"
    ),
    "runtime::enqueue_network_with_ingress_ownership": (
        _RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256
    ),
    "runtime::can_admit_timeout_vote_recovery_episode": (
        "9b99f56691a9752e95501583a1e187d8727498035d8d899c55277ccd3ffde47b"
    ),
}

_TIMEOUT_VOTE_EPISODE_RUNTIME_REGRESSION_SHA256 = {
    "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner": (
        _RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256
    ),
    "restored_pre_runtime_timeout_vote_releases_only_an_absolute_timeout_cut": (
        "00eb6a4425997fe9ef985adb09e792913c5df8805e58c4de9473e7fd720724d2"
    ),
    "pre_timeout_scheduler_owner_may_publish_across_the_physical_snapshot": (
        "47d9e77bbf51c1564dbdb4bb7ab2f5d3dafecc279a11fb59f91dcdeed2254d7f"
    ),
    "two_fresh_timeout_vote_slots_replenish_once_and_close_a_four_validator_view": (
        "7b7545acd3419b5e23f92402787307896e18465e6ad8a922b94a1ec0bddb6581"
    ),
    "restored_timeout_vote_reactivation_binds_fresh_carrier_before_runtime_admission": (
        "89eb5d842b627d1c1382143a53b73600936443130b086e5a9e0be5e5180dc6d8"
    ),
}

_TIMEOUT_VOTE_EPISODE_INGRESS_REGRESSION_SHA256 = {
    "timeout_vote_episode_crosses_only_the_bounded_certified_response_barrier": (
        "394810ab62d382e3016e5a6c88660778c34beca86d0804b567a74be47f8694d5"
    ),
}

_TIMEOUT_VOTE_EPISODE_WORKER_REGRESSION_SHA256 = {
    "timeout_vote_episode_reaches_its_predicate_across_a_selected_serve_barrier": (
        "6b66126a0ae12666093b2126bfadf1149738c9e19c3cd3ff9990fe0be46587bf"
    ),
}

# Comment-normalized formal operators for the same finite producer episode.
# These remain fail-closed placeholders until every listed semantic mutation
# is observed to fail independently.
_TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncLeaderWireCanonicalLifecyclePayload": (
            "b921a227ed962459b4973297dd0d9be90c81401af62ba7f385e398de31a8b453"
        ),
        "AsyncLeaderWireLifecycleSubject": (
            "77c927fa76dda21438c34668681704a702894bde0e6497f57c3518c9f4acb1cb"
        ),
        "AsyncLeaderWireLifecycleIdentityAt": (
            "3144055cc1d2ba15799bc2f716423a113d1998fcdfe2e1e5410e437aafac0045"
        ),
        "AsyncLeaderWireLifecycleRecord": (
            "854dd230bb474215e7b417f3d80d3327811b5aaf3a1eb5cb2ca4c3c3bdb6bc51"
        ),
        "AsyncLeaderWireLifecycleTyped": (
            "dc59143994e349417932182b8042555bd572c60cfa7a6ccf37dd3db9d2252053"
        ),
        "AsyncTimeoutRecoveryVoteOwnerDispositions": (
            "f6d0a37b228934a0f88c3d9ae8cc46aec9ed1331ae9172b5784ef78ec6092090"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionDispositions": (
            "2c2ef87daca3055b2b0747d1664ad617a826c85370434509f8ea58b6050c4c23"
        ),
        "AsyncTimeoutRecoveryEpisodeKey": (
            "61dfcd9bf069add7d70901f79ac047bf57fad89e302b3855c9dddc3ea390da59"
        ),
        "AsyncTimeoutRecoveryEpisodeKeySet": (
            "35f54e3863b1adea21d2f768a5be5862d13d9c4aaff3e41c9eb54fbb3db3a37a"
        ),
        "AsyncTimeoutRecoveryEpisodeParameterSet": (
            "97e276e0aa217f74f2670a2bd2102ff27100a460d14fcd07af09e96d201de2e6"
        ),
        "AsyncTimeoutRecoveryEpisodeFromParameters": (
            "e545c3e4954c93475837024ba05441adbd7e847c72af1ef7293099a2c40603f4"
        ),
        "AsyncTimeoutRecoveryEpisodeSet": (
            "25949f35225bb00891c0ce09e0f52a9b73677ef92d0336b5337c9d1d11966fbd"
        ),
        "AsyncTimeoutRecoveryVoteOwnerSlot": (
            "681ac1b76eedfd4cf39b4407c5f75a199fc3088fcb89ff2704ef0450e32e757f"
        ),
        "AsyncTimeoutRecoveryVoteOwnerUniverse": (
            "eb5c4dc611f078a882b491ffba0e33446987966a124e23630b9835d214e5cdb5"
        ),
        "AsyncTimeoutRecoveryVoteOwnerValidForEpisode": (
            "5a7bdb2dff26729c0a73dd25a76f084af512fd78152d84511d77f0cf48b9faa0"
        ),
        "AsyncTimeoutRecoveryEpisodeValidIn": (
            "ebc240a8012b97d2d93de0c8e89b8dc59d16555b4b6246bc5f80640e974aa53d"
        ),
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant": (
            "67dda9f1b2cf4edf41d7ef7a7e7279615e9472dc97895e9e677208b882560879"
        ),
        "AsyncTimeoutRecoveryVoteBasicBinding": (
            "7bb504333f577e6112b7d4e700c5bd06273c6a991f37f19b5a2ad148bd3fe286"
        ),
        "AsyncTimeoutRecoveryVoteIngressRecordsFor": (
            "bfa3c52d46c7f667b1c4692190c73202864dde163cb07948424677b7336c0ebf"
        ),
        "AsyncTimeoutRecoveryVotePreCutDescent": (
            "e281c02fe826ce29730d14d8d1660a1fe850ef6e37ba9957d6239e494d8278fb"
        ),
        "AsyncTimeoutRecoveryVoteRestoredDescent": (
            "b62e3824511a155faa4a16e5b264675d112b41c47705d634dc0da866880e1db1"
        ),
        "AsyncTimeoutRecoveryVoteFreshReplenishment": (
            "866f9a2e43a1e7434faa9ed0aabfa15e5a880e8d34b50812c39331763df2bed5"
        ),
        "AsyncTimeoutRecoveryVoteDispositionMatches": (
            "7d7b112535e7390fcc77ea79d994ece09822223bc2f58c255dd57732dd7437f9"
        ),
        "AsyncTimeoutRecoveryVoteCandidateOwners": (
            "30123fde668ec8ff7f8142ba8eb692926e42c9a4bdbb17a568ac3cbd940d1173"
        ),
        "AsyncTimeoutRecoveryVoteCandidateDefined": (
            "0050d45aafb4106ebc997e8b044c310806bbfdaf5233b712374a0a6d3e9889a6"
        ),
        "AsyncTimeoutRecoveryVoteCandidateOwner": (
            "f7fdd337e582dc62d429eab16fd16674b17e07890d2753301f66d9ae0c91bc66"
        ),
        "AsyncTimeoutRecoveryVoteIncumbents": (
            "5de335606d8bf7816ddfea5b85dd1716545d0ca267bd22e89e9888bb732350a1"
        ),
        "AsyncTimeoutRecoverySameVoteLifecycleOwner": (
            "7c638dd0530b7bd889e0755a45eda303f98359575397a219e86593067a78caff"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionPlan": (
            "d0da8ba07c67af4fd18a668523e3748f3972a66f85b05c2b12795ece44068b55"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionRequired": (
            "2f5ee1a824beb7736d0bbb7f0c051b29fee24fb4a782c56a3a7b1ab863afc273"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionAllowed": (
            "8dd0ff9ee7a45c7bd12d8cee1c0ba3c155115b1ec6f8cc83bbdfc237fe14b7d9"
        ),
        "AsyncTimeoutRecoveryVoteBarrierException": (
            "d8522b9b23b8314bdbbe6b6a81d601dfab6d3fbaa389e51e76029b33db0f5b5e"
        ),
        "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier": (
            "085073b59232d8c3e930b7fead0151ac8d6a37602452c556ad4174a98b734988"
        ),
        "AsyncServeIngressIndexMayPrecedeAdmittedTarget": (
            "d2f99c02442f82c8ae6041155b28d431bc38821ca3bb7b5420e032930baa6145"
        ),
        "AsyncLeaderWireIngressIndexMayPrecedeAdmittedTarget": (
            "86ad64b20c4ec9626b9fc1221b15c3d51a67187d0664a0c41bcd22b48c66a94b"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep": (
            "28f5c1e16034f5dd5c3cae9ccf22c81cb02e1d9ffc1eb20aa04f04e6b915b385"
        ),
        "AsyncTimeoutRecoveryVoteRuntimeRecordsAfter": (
            "48aa5a6e4b6422d73214ea872f4fa817071bd17b2a6fa4c55b59aa100f0ac5b2"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep": (
            "cc519f7ac25e4fed87ac00a68d907e97b72b1385eb6d527d4ed94facde6456a8"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission": (
            "f49556faa311370cee18c10d8af8489054c87bfceca95df7e10ca49f4114e0b2"
        ),
        "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission": (
            "37d807e2f76d7170ca7145829e8ebe8b2fafd5f2b24ea0130e3beff0fa5d6021"
        ),
        "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition": (
            "acf7fbae28f75e45ecd317451bc2e38ab865fab16acdc736d0bd8024d952d200"
        ),
        "AsyncTimeoutRecoveryAdmittedVoteSlots": (
            "b09ad8beca8d2938814d9d19fd3446c596f01131e885bbb4f8875f1749800183"
        ),
        "AsyncTimeoutRecoveryRemainingProducerSlots": (
            "a3c7c870d7110bfcf64c6be0e5889f13fb836379b55954775349f11ab762a75c"
        ),
        "AsyncTimeoutRecoveryProducerEpisodeMeasure": (
            "316131577d432419223b8e039fc85c66144da1996701736cf6a04882105747b7"
        ),
        "AsyncTimeoutRecoveryEpisodeCreationReadyIn": (
            "463fcd6b8a67c383cd62b258d220831cd60d0854df625b77b359ff3859827283"
        ),
        "AsyncTimeoutRecoveryTransitionGateIn": (
            "02e689cac6c1aba321ea319591aff3feb1957c946608e9cc86f3826c854445ed"
        ),
        "AsyncTimeoutRecoveryEpisodeRetiresThisStep": (
            "ec514a038cee6fb9f31e63409040bee22defb732edc7208e3a683779a24ad686"
        ),
        "AsyncTimeoutRecoveryExistingCaptureClearsThisStep": (
            "92a31d4bd2a3c6b1a57c6bbbcf5d67d33da9b575922159aa12074e373226eb1d"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterTransition": (
            "043fa928e89e959c987aa19209e2c3ef68541a551017062b9f38ae1a0d267fbc"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition": (
            "15b01a6485f2446a013ff833d7ed3831dca42ba6933642afa74bb123622d7f76"
        ),
        "AsyncTimeoutRecoveryNewEpisodeIn": (
            "dfb0f545e1804325d054d841731c71e8da09565a9ae99177980e66b42f828b54"
        ),
        "AsyncTimeoutRecoveryNewEpisodesAfterTransition": (
            "0bb56383a4306a31b7df629c7908fd37f772a5c31305ee641b887c287a46b3f2"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochProofs.tla": {
        "AsyncTimeoutRecoveryEpisodeBoundaryIn": (
            "a45ca5da201b0d81c538304cfbd4740029c9b8aaa17190f2c0f835a99ec5bc8a"
        ),
        "AsyncTimeoutRecoveryBoundaryFrameShape": (
            "7b851fbaf17ebee465208fbde0c47b447c6cf9e278cc54b369ce00da2146eecf"
        ),
        "AsyncTimeoutRecoveryMutationFrameShape": (
            "f85b7549b33097ba1cf31adbb63d48a85fdf0b61f409b60e128451acbf37656f"
        ),
        "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor": (
            "5bb5d4962bbc9db7fcbe00c53e70d5532ed39550ee5da04acdac6048dc0fc5c3"
        ),
        "AsyncTimeoutRecoveryNewBaseEpisodeIn": (
            "aebba5c89eb28cc959098eef0a148c0a3defe3818d71f7aacc07ab8f2fd73136"
        ),
        "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn": (
            "e90869e129c22b6a56cdac3759bcf47be53b7c89276675019eeb1b7e39891d2b"
        ),
    },
}

# Exact theorem statements and proofs which close the finite producer episode.
# These seals stay deliberately unapproved until every independent theorem and
# downstream-provider mutation fails after its local digest is refreshed.
_TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite": (
            "1df06952a3664f7770385d6d4175e73bfbeddc53200715e3ccd627ac925a39f9"
        ),
        "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot": (
            "88bd8b5a5262c9e4220a14c1041492fea1435795035c0df5833da6b01ae7b8f6"
        ),
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission": (
            "09724aa3803ff9a87bbc61b24f101afa4d2ead895f167a3c05beb3bd59403698"
        ),
        "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot": (
            "44874f718618e4409812b99ff62e28cefa9dc904a4938889e04104a5a4f1485b"
        ),
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode": (
            "4c25562e7e3e5ba4b301a083caf80fc507ca23793fdff5f0d83a61d98ab598db"
        ),
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot": (
            "402d555a2123de59812a33b3beb9efd8018fa08f70d293b34284ea83d12ebd12"
        ),
        "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState": (
            "81b6e5c8ab71af82a8099a563a96db2e5ee106fc31c9d773b86db2c7652c064e"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent": (
            "f23a0f081c1b20572022d0756f8daa9ee1dbc06da96b1e4ed933fe2406b53686"
        ),
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation": (
            "4ff696a18c898ff16ba41a705e464668de9083c018efdafe1baccbeceeae3cb7"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode": (
            "21fad6c95f6d72febeaea68a67b42c917c44c5d3958203eb2fc2e442227c1529"
        ),
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState": (
            "579691707af9891234bdee0b535a018dd864baac48548c17ddac66f428dec26e"
        ),
        "AsyncPostGstHasNoControlServiceReset": (
            "d738c3b56078f2a44f76f15c319d310288f9fad7a2ec011a7cdc04af5d06e36d"
        ),
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode": (
            "d3cd30a1100860fd8c718fce20f20379aba20ed13974fa7eed912503f9138ce8"
        ),
        "AsyncUnchangedCoreStatePreservesTimeoutBoundary": (
            "47d944b51bd1a575c066d1d3218e62de883b07984b04725ae9355234f73a20cc"
        ),
        "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary": (
            "2ea11aa8dc93fa7684d2ecb2a96cb4af7dbbbc8da63d30a008048a2a1fd045aa"
        ),
        "AsyncUnchangedCoreStateExcludesPersistInstall": (
            "095b9ab62786836bcfbb5a4363477378905311a89aebc4d1fcf4feb4bd7ba97f"
        ),
        "AsyncFairIngressDrainPreservesRetransmitTimerState": (
            "b473620a788d1b0f680dc34677d31471a8f218415ab2572c2d18835c1e64b2d9"
        ),
        "AsyncFairIngressDrainExcludesDirectRetransmit": (
            "ad31fdd4ae63ba80ec7d6ca8e9944f5dcc0f8c237ceb6596b35d23d12f7ea674"
        ),
        "AsyncTypedOutstandingTagRemovalChangesFunction": (
            "8ea219351421de1566369e590290bebafc6fd19d80ef1f3173b81d9cafdc11de"
        ),
        "AsyncDeferredRetransmitRemovesOutstandingTag": (
            "4b85c87bc746d4fee0e6844c5f1f3cb72c53a9acaf3cdb8b0f15a2fd69744bb0"
        ),
        "AsyncFairIngressDrainExcludesDeferredRetransmit": (
            "bc19fc379f8da8f848667b77cb0c859605fae3153e2ba6d2a2be334f0b40c17b"
        ),
        "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle": (
            "21b37b8848a7bd3922bf934c2ae2533af315f563465cd07866562762a82dd959"
        ),
        "AsyncIngressDrainFramesDeferredAndCausalQueues": (
            "ed85c48c72dd007237193ba7a5087ba3f635b4fb2c5d9e3c05f5ac3fd78ceb59"
        ),
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork": (
            "9ec8a9f5753e50c531e67e7ff18140a6a442f82afee336a0da542aa24f370c9c"
        ),
        "AsyncSequenceSetAfterAppendAddsOnlyValue": (
            "54303c304b4d75584ef078930e256dbc611662ba19d72a80907c555c4d328ee2"
        ),
        "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue": (
            "41fef605e80b132b9fa9dc1e8be04bbcdc05b95cd3ecb7816db3866d2bcdf01e"
        ),
        "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers": (
            "333bf376ecd4a6de1510f3ef6be559e847804cfc5b5c99abe06bff76a27e241b"
        ),
        "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin": (
            "ec3bf0910e32f4ff43e9c2de072985cfce9ee193894b49857cba30fb2a95222c"
        ),
        "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase": (
            "9f877ab73f3e7dd856d0e815adcd53f49537531203273d688470bee4af9cf8bf"
        ),
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase": (
            "9e91b44806f1f022f7b864cf76c6195a7e4a7d4e47e47685f5cb8db75e426053"
        ),
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin": (
            "7895a9a9334907f80dafa4f489440db25dd1834c76d1aec5aad4fbf6b3343ae2"
        ),
        "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase": (
            "24f1cc906957182fcb9cc2070f04a36d3d412c649f7eb3266efaac4af6fc3774"
        ),
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind": (
            "6738b6c3ff4b2f59e8a76c0f3d48a45a1596f33c1034e63bbccee12f687aef9b"
        ),
        "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase": (
            "8575237ea078b3f9aa6ae52221c7005926d933edd1f64e6f76d105ee10b1bde1"
        ),
        "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle": (
            "ef4c3d237655ff50311beb25ccd505c64a08ec4d7101dc8cfb8561eb27a442ad"
        ),
        "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame": (
            "f042e10c01d4109284f2557ce3da9806f771009526a3ea528b8d899ad794cb64"
        ),
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode": (
            "43b470ee0007250242614c689a697ece3ba3c5b97a508ae5dd184f12d9431c28"
        ),
        "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState": (
            "47d20f52eb8495d81169989c68ba754f7f36bfa6622b02a56433240f41367c54"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition": (
            "9b7852d4c48d9c782e1e06ad51abdf998b9fe770e65a4c656a13231ce080f93f"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochProofs.tla": {
        "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileEpisodeDue": (
            "87a2bae16488fb91aa75fe1a768cf789af85395501336a69ad8049ea81dc41f3"
        ),
        "AsyncNextPreservesEmptyServeIngressOwnersWhileEpisodeDue": (
            "ef5e61577913d1fac48f01db1ba8cfefb0ee72f722bf9a2bda30da5db92384a2"
        ),
        "AsyncNextPreservesServeProducerEpisodeTypeInvariant": (
            "341a2e27665c84e9a483ddb897b03535d3b81406d299f17a81db1ec920e9b90a"
        ),
        "AsyncNextPreservesServeProducerEpisodeInvariants": (
            "09da7c9d7814236562f6a87fb5f88d48777db0dd5f739ae99a5e77f65f51739f"
        ),
        "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame": (
            "1b4c3dfb92c959259a888b735f68844adab19402ea9fb2898dbddea784965c8a"
        ),
        "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape": (
            "4d2c2d79ef909af71925238d588ed7c64d76a589f2792b699ea1adc63ede7a1f"
        ),
        "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape": (
            "919a0a8655b7d59f9ff637e654c32dbacaa5e3111f06495a757fd9b4a5926050"
        ),
        "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary": (
            "d83c752c87732813a8c874cbc6767d8e094b8babe60b5fb9d2c7a03055b12ed6"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary": (
            "8dcef1fda3370f5227fd39b43c1361a7607e16f41e7ccce7d8c2fae1220a96a7"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary": (
            "165cbd893a1cea61c0c91cfd04ce9d7f955bd66ff0ec6c342fcd1668a4b04429"
        ),
        "AsyncTimeoutRecoveryNewEpisodeDecomposition": (
            "b5ee625013c0ceb306bc595c023bd508605aa53015dc870417f70c046f8532ff"
        ),
        "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary": (
            "7904b774302b1d694602a1998453a7eda81b24476c907a6f0ad9db82a4e062a4"
        ),
        "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary": (
            "e2805c35555463b3deb60c7406ca6857bbf8be56c176e865e64417085519a43d"
        ),
        "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary": (
            "ff78a382b38ce79b326523adc660a8b44be48edfdccb4cce8125f9cea9319cd8"
        ),
        "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary": (
            "9729dc41b99b9e312a7720b1d13eba71eae422822ff341a3d22b3dddf9c8146f"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary": (
            "5c0bf320cc8859bf01595208361885d31718a1b6cf99fcad84a0104de0927cbf"
        ),
        "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary": (
            "2373ca20ddef74981b6efe3ca4d917b3f305aed0a6eda616559c7ab770954b9f"
        ),
        "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet": (
            "63b50294ef186208a2a221563512cf3954df54e9325cec390b70c782af1b189c"
        ),
        "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet": (
            "3ae8217b29fadcf0f9bd24bf30e60d03c4951f3c51bc89a536455201430adfa2"
        ),
        "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary": (
            "cd37abed710d6c5fda8727bc5131ffd5127e4ce6e8ff057d590e58973a978b41"
        ),
        "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant": (
            "e7333ca33c4b997ae139c209748ae603fac1b24002c319dab295bcb02325ae4a"
        ),
    },
    "SumeragiV2AdequateLeaderServiceClosureProofs.tla": {
        "AdequateLeaderFreshTimeoutVoteReplenishmentConsumesProducerSlotAndOpensNonDescentEpisode": (
            "3cb5653b8cceb88b3a193570b5b344cde20f58e5759bfb770fa343f9e392126c"
        ),
    },
}

# Exact production effect-executor entry points which retain the authenticated
# ingress carrier and latch a fail-stop restart on malformed ownership.
_EFFECT_CAPACITY_PRODUCTION_RUST_ITEM_SHA256 = {
    "enqueue_network_with_ingress_ownership": (
        "9bf3e8ec45247e920681de7f13941d8544df863b73c5a3ac243adcacae0b2587"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "b2d35847177381a3f4ab5fd6fb06ce4b15e7449220db5daac00699cc5825677d"
    ),
}

# The retained exact Fetch lifecycle must not duplicate service work while its
# immutable owner is already installed. Completion must publish the reserved
# runtime successor before retiring local ownership; protected-view transfer
# must move an unpublished token with its Fetch consumer; every terminal Fetch
# path must retire either that token or its exact restart-restored stage-7
# parent before releasing P/Q ownership. The checker separately binds the
# persist-first deferred batch, restart-only producer frontier, persistent-root
# rebind, and live leader-wire recovery cut by ordered source contracts. Bind
# the complete production methods and adversarial regressions in addition to
# focused order and coordinate checks.
_EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256 = {
    "begin_fetch": (
        "9d50ba3ff1b4227c8e0ce7a8444796eb1b50c6c1a23b8c5f5669991652f7f4e0"
    ),
    "reserve_body_available_with_owner": (
        "a29bf418476f2b07e4821d63c6c1415ac611fc52a59868ea5fb7dc077d17dfa2"
    ),
    "rebind_unpublished_body_available": (
        "00bbe44f9d02199bee50f805425df0760caffc257598003f55aaa35ddb47d46c"
    ),
    "retire_unpublished_body_available": (
        "c692816b54ef0184f6453614a2c8272168ac445e5e1f397ab27a6b508354c6fd"
    ),
    "effect_runtime_rebind_unpublished_body_available": (
        "e126b97f8f5a75b0868f6334ada164ec16e2d35b924e6940daa91f5b5fe3550e"
    ),
    "effect_runtime_retire_unpublished_body_available": (
        "3ed15174c27d497028b91f264a25ec2c1294c35e228298ebc5caa51f74332df5"
    ),
    "commit_pending_fetch_retirement": (
        "2a48078e9a5ffd2d2fc9d57011089be27f15e847b346ba2ff1c3b0a4343d4815"
    ),
    "install_view": (
        "aead33e5d7df955164b9ff8764dc4aefbe46648b96041d5c056fb4f366d3697d"
    ),
    "commit_fetch_completion": (
        "ee63eedba56be13b1af788f81f9f69dc380a834bc0a8d0b36e1de3910f041f92"
    ),
    "retain_retransmit_effect_ownership_for_test": (
        "7501414a5b20cfeacf429a9d2b43cf5dd2a797e2a579559c05d987437352ed8d"
    ),
    "production_capacity_saturation_admits_response_and_reconstructible_fetch": (
        "a97f5cab5c218297c5df0dbc3c9aa803c9a0284b50fc7c8ad960c2314e56b861"
    ),
    "ready_body_backpressure_retains_exact_ingress_until_capacity_retry": (
        "ecadc5af1fc748393bfeb99ac01513e84b3eb51bd59453524257441ee65de126"
    ),
    "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner": (
        "920f70f6d4ebf90ce3f9a4365e99bc88da19671d80e4f716aa57bc08caece7fd"
    ),
    "tc_retires_unprotected_retryable_body_token_before_the_next_fetch": (
        "49cbdd3ee3ef185f6e259b986b6fe1d2eea11f83b97eebab02aa8cae48903a44"
    ),
    "durable_sign_preemption_retires_a_retryable_body_token": (
        "4eff93945c51ac3473a2817a29dfe493a6606b3b2919ba33ff957ee121b325c1"
    ),
}

# Exact adversarial integration witnesses for the durable locked-Commit owner
# alternatives.  These tests are release inventory, not deductive evidence;
# whole-item seals prevent a renamed or weakened lookalike from satisfying the
# progress-witness regression contract.
_LOCKED_COMMIT_PROGRESS_WITNESS_TEST_SHA256 = {
    "locked_commit_progress_witness_rejects_inexact_or_empty_ownership": (
        "a40965bfa911b0f8b2cf118644aaf07d4cd6898246b1d5b72b9ea4e15649a9d6"
    ),
    "locked_commit_progress_witness_accepts_each_exact_owner": (
        "07823a34cefc84c62880027df0898fd5524c5e1e7f9ec71e0ebc84d7998e45b7"
    ),
}
_LOCKED_COMMIT_PROGRESS_WITNESS_HELPER_SHA256 = {
    "locked_commit_has_exact_progress_witness": (
        "e63115adfa82478faa975238d72973dbe84791f45fb8c968732d62974d2c44f4"
    ),
    "validate_locked_commit_progress_witness": (
        "fe6f766850236b7f363fb828e4acaa810eb676876e800dcb3fcd9b88711084ae"
    ),
}

_PRODUCTION_LIVENESS_RELEASE_COUNT = 854
_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT = 88
_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256 = (
    "df90ef7d94284bc805ff55ead6c6d938ba0f681a1d39e7b929fc275e8019aefa"
)
_PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256 = (
    "d581ede7aed449c27f24b27be9ef88cc7ef640e3d5149ba9070fcc99d5cf6fed"
)
_SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256 = (
    "e99da2c824b86930b76c741d2f7aa47ab16092c2f84e43550fb6362a36133268"
)
_SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256 = (
    "42fc1fb789e115df9f54c230ee6bfc1e1c20504a904aa20f945b6369df6d7679"
)
_CLOSED_SIDECAR_PREFIX_HANDOFF_TEST_SHA256 = (
    "75019365bd62839da229b51671071af1b9165f4c08fc06d36be6bc2e4e14b893"
)
_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT = 525
_PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT = 526
_PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256 = (
    "dc428b5bb9054495ef88aacd5b07a0f932ba2ada9da0c015dc45f36edbdf1352"
)
_PRODUCTION_MULTILANE_FOCUS_CONTRACTS = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
    ),
    (
        "required_multilane_config_runtime_focus_tests",
        "g-unit-iroha-config-runtime",
        "iroha_config",
    ),
    (
        "required_multilane_config_fixtures_focus_tests",
        "g-unit-iroha-config-fixtures",
        "iroha_config",
    ),
    (
        "required_multilane_data_model_focus_tests",
        "g-unit-iroha-data-model",
        "iroha_data_model",
    ),
    (
        "required_multilane_torii_focus_tests",
        "g-unit-iroha-torii",
        "iroha_torii",
    ),
    (
        "required_multilane_torii_shared_focus_tests",
        "g-unit-iroha-torii-shared",
        "iroha_torii_shared",
    ),
    (
        "required_multilane_integration_lib_focus_tests",
        "g-unit-integration-tests",
        "integration_tests",
    ),
)
_GENESIS_HEADER_BINDING_TEST_SHA256 = (
    "8d847d27cdea09a87f5ee4ec940f60f9fa73fb85ca9a965d2a3fcac19eb3b41e"
)
_RESTART_VIEW_ZERO_DEADLINE_TEST_SHA256 = (
    "13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f"
)
_SUCCESSOR_PARENT_BINDING_TEST_SHA256 = {
    "successor_core_context_preserves_the_parent_certificate_binding": (
        "79c2caea8dfd6f17885ff3d72253a41cb34db7a99d7976b52d5fdab45c0e9a89"
    ),
    "successor_context_requires_the_durable_cryptographic_parent": (
        "c00cd9b450f0b1b92e5697eb6a27dcdbc283b20cb4cf597f73a3e1bca5b8fae4"
    ),
    "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter": (
        "423991082e2c3a6151fdd80b69b643d56b56d2cb7963331024453b2fab7c037c"
    ),
}
_LATE_LANE_RECOVERY_TEST_SHA256 = (
    "b0457ed9453abd1999e7246c3cd2d96aa1dc6e4763466a5f7c76635699997344"
)
_PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS = (
    ("production-kura-progress-durability", "kura::tests", 17),
    ("production-kura-lane-geometry", "kura::lane_geometry::tests", 8),
    ("production-lane-relay-exact-ownership", "nexus::lane_relay::tests", 4),
    (
        "production-authoritative-ingress",
        "sumeragi::authoritative_runtime_gate_tests",
        43,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 118),
    ("production-state-governance-unlock-audit", "state::tests", 1),
    ("production-v2-core", "sumeragi::v2_core::tests", 38),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 17),
    (
        "production-v2-core-wal",
        "sumeragi::v2_core::wal::byte_lifecycle_tests",
        1,
    ),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        8,
    ),
    (
        "production-v2-equivocation-evidence",
        "sumeragi::evidence::tests",
        1,
    ),
    (
        "production-v2-leader-wire-lifecycle-store",
        "sumeragi::serviced_candidate_store::tests",
        1,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 46),
    ("production-v2-body-store", "sumeragi::v2_body_store::tests", 2),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 3),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 72),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 61),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 68),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    (
        "production-v2-lifecycle-recovery",
        "sumeragi::v2_lifecycle_recovery::tests",
        5,
    ),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 37),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 133),
    (
        "production-v2-watchdog",
        "sumeragi::status::v2_liveness_watchdog_tests",
        19,
    ),
    (
        "production-kagemusha-finality",
        "zk::kagemusha_finality::tests",
        1,
    ),
    (
        "production-data-model-v2-finality",
        "block::consensus_v2::finality::tests",
        1,
    ),
    (
        "production-data-model-offline-compact-qc",
        "offline::kagemusha_v4_topup_provenance_tests",
        1,
    ),
    (
        "production-data-model-v2-context-identity",
        "block::consensus_v2::tests",
        2,
    ),
    ("production-v2-integration-runner", "sumeragi_v2_runner", 4),
    ("production-p2p-peer-reliable-flush", "peer::run::tests", 11),
    (
        "production-p2p-shared-source-byte-geometry",
        "peer::shared_byte_budget_tests",
        8,
    ),
    ("production-p2p-network-reliable-actor", "network::tests", 84),
    (
        "production-p2p-source-memory-geometry",
        "network::inbound_source_memory_bound_tests",
        2,
    ),
    (
        "production-p2p-waiter-rank-geometry",
        "network::handle_update_tests",
        4,
    ),
    (
        "production-irohad-consensus-message-control",
        "consensus_message_control::tests",
        8,
    ),
    ("production-irohad-network-relay", "network_relay_tests", 4),
    ("production-irohad-authenticated-via", "tests::relay_fairness", 7),
    (
        "production-config-v2-exact-output-geometry",
        "parameters::actual::tests",
        2,
    ),
    (
        "production-config-v2-exact-output-root-parse",
        "parameters::user::duration_clamp_tests",
        5,
    ),
)
_PRODUCTION_LIVENESS_RELEASE_MODULES = tuple(
    module for _, module, _ in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = (
    "kura::tests::lifecycle_release_terminal_outcomes_are_exact_idempotent_and_ordered",
    "kura::tests::autonomous_view_state_latest_read_only_selects_crash_temp_without_mutation",
    "kura::tests::unfinalized_merge_carrier_tip_rebuilds_post_wsv_reservation_on_restart",
    "kura::tests::merge_application_receipt_makes_autonomous_auxiliary_persistence_terminal",
    "sumeragi::v2_lifecycle_recovery::tests::generation_takeover_runs_crash_recover_and_rehydrate_then_stutters",
    "sumeragi::v2_lifecycle_recovery::tests::every_lifecycle_recovery_cursor_cas_boundary_survives_restart",
    "sumeragi::v2_lifecycle_recovery::tests::prepared_bootstrap_and_crash_boundaries_resolve_only_their_durable_side",
    "sumeragi::v2_lifecycle_recovery::tests::empty_queue_reconciliation_returns_the_same_checked_receipt",
    "sumeragi::v2_lifecycle_recovery::tests::local_producer_recovery_requires_the_exact_current_queue_owner",
    "sumeragi::v2_lane_work::tests::production_adapter_stays_carrier_silent_until_exact_queue_activation",
    "sumeragi::v2_apply::tests::deferred_canonical_carrier_owned_and_absent_groups_complete_before_gate_publication",
    "sumeragi::v2_apply::tests::deferred_canonical_carrier_missing_after_queue_cleanup_keeps_startup_gate_closed",
    "sumeragi::v2_runner::tests::startup_reconciles_lifecycle_before_lane_work_activation",
    "sumeragi::v2_runner::tests::terminal_sweep_source_partitions_whole_units_before_any_mutation",
    "sumeragi::v2_runner::tests::local_producer_queue_custody_is_preflighted_before_cursor_mutation",
    "kura::tests::certified_lane_block_encoding_enforces_source_envelope",
    "nexus::lane_relay::tests::actor_backpressure_retains_exact_relay_and_fifo_ticket",
    "nexus::lane_relay::tests::blocked_relay_does_not_starve_a_responsive_relay",
    "nexus::lane_relay::tests::terminal_actor_failures_return_exact_relay_ownership",
    "nexus::lane_relay::tests::saturated_relay_owner_returns_sixty_fifth_without_actor_ticket",
    "sumeragi::authoritative_runtime_gate_tests::direct_and_synthetic_envelopes_keep_identity_roles_consistent",
    "sumeragi::authoritative_runtime_gate_tests::atomic_lane_certificate_uses_the_shared_progress_owner",
    "sumeragi::authoritative_runtime_gate_tests::oversized_atomic_lane_certificate_is_returned_exactly",
    "sumeragi::authoritative_runtime_gate_tests::relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin",
    "sumeragi::authoritative_runtime_gate_tests::authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains",
    "sumeragi::authoritative_runtime_gate_tests::roster_origin_relay_completion_has_authenticated_source_count_and_byte_owner",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_wire_index_keeps_authenticated_origins_distinct",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_escape_survives_exact_same_source_saturation",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_serializes_distinct_timeout_certificates_per_source",
    "sumeragi::serviced_candidate_store::tests::leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots",
    "sumeragi::v2_effects::tests::effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains",
    "sumeragi::v2_effects::tests::retained_live_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_effects::tests::retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_effects::tests::exact_candidate_retry_coalesces_under_the_incumbent_owner",
    "sumeragi::v2_effects::tests::fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work",
    "sumeragi::v2_effects::tests::adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes",
    "sumeragi::v2_runtime::tests::adapter_effect_binding_is_exact_route_neutral_and_three_bounded",
    "sumeragi::v2_runtime::tests::certified_body_pipeline_retains_statement_and_owner_across_stage_kinds",
    "sumeragi::v2_runtime::tests::body_pipeline_acquires_commit_authority_monotonically_under_one_owner",
    "sumeragi::v2_runtime::tests::applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome",
    "sumeragi::v2_runtime::tests::applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation",
    "sumeragi::v2_runtime::tests::drained_internal_ignore_uses_exact_durable_tombstone_before_readmission",
    "sumeragi::v2_runtime::tests::queued_body_completion_coalesces_only_its_incumbent_owner",
    "sumeragi::v2_runtime::tests::stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal",
    "sumeragi::v2_runtime::tests::restored_serve_high_watermark_precedes_startup_runtime_owner",
    "sumeragi::v2_runtime::tests::full_runtime_churn_cannot_cross_an_exact_serve_ordinal",
    "sumeragi::v2_runtime::tests::decision_retirement_releases_queued_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::lock_retirement_releases_busy_deferred_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::production_authenticated_preflight_is_never_semantic_only_coalesce",
    "sumeragi::v2_runtime::tests::semantic_only_authenticated_coalesce_fails_before_receipt_registration",
    "sumeragi::v2_runner::tests::fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes",
    "sumeragi::authoritative_runtime_gate_tests::alternate_reply_route_attaches_before_authenticated_source_lane_cap",
    "sumeragi::authoritative_runtime_gate_tests::transport_reply_route_construction_is_fallible_and_target_bound",
    "consensus_message_control::tests::stale_duplicate_reordered_and_unknown_releases_are_atomic",
    "consensus_message_control::tests::hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic",
    "consensus_message_control::tests::drain_fence_holds_racing_chunks_fifo_until_atomic_cutover",
    "tests::relay_fairness::hold_release_preserves_exact_layered_ownership_until_recorded_terminal",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_must_fit_network_geometry",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry",
    "parameters::actual::tests::sumeragi_v2_config_format_changes_the_handshake_fingerprint",
    "sumeragi::v2_core::refinement::tests::historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution",
    "sumeragi::v2_core::refinement::tests::historical_certificate_kernel_rejects_foreign_admission_and_unretired_request",
    "peer::run::tests::consensus_lane_and_v2_topics_share_authenticated_high_source_credit",
    "merge_sidecar::tests::exact_active_delivery_retry_preserves_decreasing_chunk_rank",
    "merge_sidecar::tests::alternate_source_progress_and_reconnect_preserve_independent_cursors",
    "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
    "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
    "merge_sidecar::tests::later_delivery_preserves_the_current_source_cursor",
    "merge_sidecar::tests::later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit",
    "merge_sidecar::tests::late_old_exact_item_receipt_completes_reconnected_attempt_once",
    "merge_sidecar::tests::later_delivery_during_materialization_keeps_exact_authorized_route",
    "merge_sidecar::tests::writable_reconnect_during_materialization_keeps_exact_authorized_tenure",
    "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
    "merge_sidecar::tests::equal_sequence_with_different_semantic_identity_is_rejected_before_materialization",
    "merge_sidecar::tests::transient_materialization_release_keeps_exact_retry",
    "merge_sidecar::tests::transient_response_capacity_defers_materialization_on_the_same_delivery",
    "merge_sidecar::tests::response_materialization_requires_and_consumes_its_exact_admission_gate",
    "merge_sidecar::tests::sidecar_admission_matches_the_cached_arc_without_changing_ownership",
    "merge_sidecar::tests::inactive_reply_route_is_rejected_before_server_gate_admission",
    "merge_sidecar::tests::completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses",
    "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
    "merge_sidecar::tests::completed_source_does_not_block_a_new_alternate_source",
    "merge_sidecar::tests::configured_route_source_capacity_bounds_semantic_attempts",
    "merge_sidecar::tests::configured_source_geometry_reserves_more_than_eight_independent_attempts",
    "merge_sidecar::tests::third_session_from_one_hub_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::source_byte_overflow_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::completed_short_session_replacement_cannot_starve_an_older_long_session",
    "merge_sidecar::tests::route_retirement_between_admission_and_enqueue_releases_all_response_reservations",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_session",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_bytes",
    "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
    "merge_sidecar::tests::durable_response_drain_persists_pending_identity_before_handoff",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_is_one_atomic_kura_backed_response",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_serves_rotated_validator_after_pressure",
    "sumeragi::v2_lane_work::tests::historical_certificate_survives_successor_lock_decision_persistence_and_restart",
    "sumeragi::v2_lane_work::tests::carrier_replacement_filters_persistence_and_output_sources_together",
    "sumeragi::v2_lane_work::tests::applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts",
    "sumeragi::v2_lane_work::tests::native_amx_request_rejects_inactive_reply_route_before_signing",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_preserves_exact_source_delivery",
    "sumeragi::v2_lane_work::tests::reply_effect_rejects_missing_or_retargeted_route_set",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_updates_only_later_delivery_from_same_source",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_retains_alternate_sources_across_source_update",
    "sumeragi::v2_lane_work::tests::temporarily_unserviceable_effect_requeues_behind_later_reserved_work",
    "sumeragi::v2_lane_work::tests::retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling",
    "sumeragi::v2_runtime::tests::certified_tc_crosses_full_fence_blocked_prepare_prefix",
    "peer::run::tests::authenticated_source_credit_precedes_network_and_subscriber_backlogs",
    "peer::run::tests::recoverable_post_acknowledges_only_after_full_write_and_flush",
    "peer::run::tests::partial_write_error_closes_ack_without_false_completion",
    "peer::run::tests::coalesced_batch_acknowledges_every_item_only_after_flush",
    "peer::run::tests::maximum_frame_uses_a_bounded_number_of_source_reservations",
    "peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting",
    "peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift",
    "network::inbound_source_memory_bound_tests::authenticated_source_count_share_is_checked_and_never_zero",
    "network::tests::actor_progress_bypasses_full_deferred_owner_and_waits_for_writer_flush",
    "network::tests::actor_progress_lease_survives_topology_transition",
    "network::tests::actor_progress_retries_exactly_once_on_peer_writer_replacement",
    "network::tests::actor_progress_retry_round_robin_bypasses_partitioned_target",
    "network::tests::cap_one_blocked_source_cannot_prevent_live_source_service",
    "network::tests::actor_progress_lease_survives_debug_packet_loss_until_delivery_retries",
    "network::tests::actor_broadcast_retry_targets_only_failed_peers",
    "network::tests::reliable_subscriber_is_single_consumer_under_clone_budget_pressure",
    "network::tests::reconnecting_peer_cannot_multiply_retained_source_credits",
    "sumeragi::v2_core::refinement::tests::two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations",
    "tests::relay_fairness::daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner",
    "tests::relay_fairness::saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits",
    "tests::relay_fairness::real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_resets_on_successor_owner_and_status_clear",
    "sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff",
    "sumeragi::v2_runner::tests::reserved_lane_output_bypasses_unserviceable_head_without_losing_owner",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_durable_lane_certificate_reply_routes",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_certified_sidecar_chunk_reply_routes",
    "sumeragi::v2_runner::tests::bounded_sidecar_admission_turn_applies_only_its_budget",
    "sumeragi::v2_runner::tests::runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling",
    "sumeragi::v2_runner::tests::runner_dispatch_advances_certified_sidecar_only_after_writer_flush",
    "sumeragi::v2_runner::tests::runner_dispatch_retired_admission_race_emits_no_sidecar_receipt",
    "sumeragi::v2_runner::tests::runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
    "sumeragi::v2_runner::tests::closed_sidecar_prefix_handoff_requeues_only_failed_suffix",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_durable_response_without_reply_routes",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_exact_final_lane_commit_qc_post",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_complete_merge_share_fanout",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_pending_capacity_replacement",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_materialized_unclaimed_replacement",
    "sumeragi::v2_worker::tests::certified_serve_shutdown_rolls_back_materialized_unclaimed_replacement",
    "sumeragi::v2_worker::tests::certified_serve_terminal_replay_waits_for_barrier_then_bypasses_full_serve_fifo",
    "sumeragi::v2_worker::tests::certified_serve_terminal_replay_source_retains_retired_route_and_reconnects",
    "sumeragi::v2_worker::tests::same_tenure_updates_and_reconnect_preserve_current_item",
    "sumeragi::v2_worker::tests::closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures",
    "sumeragi::v2_worker::tests::completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge",
    "sumeragi::v2_worker::tests::later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
    "sumeragi::v2_worker::tests::inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision",
    "sumeragi::v2_worker::tests::owned_reply_history_merge_retries_candidate_retirement_after_prune",
    "sumeragi::v2_worker::tests::newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source",
    "sumeragi::v2_worker::tests::a_b_a_hub_reconnect_preserves_each_source_cursor",
    "sumeragi::v2_worker::tests::owned_reply_transfer_retirement_after_validation_is_atomic",
    "sumeragi::v2_worker::tests::bulk_backpressure_does_not_block_reserved_lane_or_safety_output",
    "sumeragi::v2_worker::tests::non_roster_targets_cannot_consume_frozen_validator_reservations",
    "sumeragi::v2_worker::tests::partial_fanout_progress_releases_only_the_completed_target_unit",
    "sumeragi::v2_worker::tests::ownership_units_reject_reservation_spill_and_release_exact_target",
    "sumeragi::v2_worker::tests::backpressured_source_does_not_block_other_sources_or_consume_their_reserve",
    "sumeragi::v2_worker::tests::production_output_path_serves_later_fanout_while_target_stays_backpressured",
    "sumeragi::v2_worker::tests::response_outputs_without_exact_routes_fail_stop",
    "sumeragi::v2_worker::tests::sidecar_receipts_use_a_separate_bounded_control_queue",
    "sumeragi::v2_worker::tests::actor_backpressure_cannot_change_returned_payload_identity",
    "sumeragi::v2_worker::tests::exact_output_retry_rejects_a_different_message_identity",
    "sumeragi::v2_worker::tests::full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
    "sumeragi::v2_worker::tests::applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_output_without_reconstruction",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_unbound_lane_output_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_wrong_height_global_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_historical_kura_global_responses_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
    "peer::run::tests::full_write_without_flush_ack_closes_actor_witness_and_retries_on_replacement",
    "network::handle_update_tests::progress_budget_preserves_fifo_for_three_registered_producers",
    "network::tests::reliable_progress_class_matches_actor_reservations_exactly",
    "network::tests::reply_route_survives_peer_message_clone_mapping_and_split",
    "network::tests::peer_message_rehydration_rejects_second_reply_route_without_retargeting",
    "network::tests::reply_source_key_groups_relay_origins_and_orders_actor_instances",
    "network::tests::reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
    "network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
    "network::tests::cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
    "network::tests::dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
    "network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
    "network::tests::reply_route_binding_rejects_evicted_tombstone_collision",
    "network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
    "network::tests::route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
    "network::tests::reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
    "network::tests::reply_actor_admission_does_not_complete_writer_flush_ack",
    "network::tests::reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none",
    "network::tests::retired_reply_tenure_closes_flush_ack_without_false_completion",
    "network::tests::reply_flush_test_fixture_distinguishes_success_timeout_and_close",
    "network::tests::reply_flush_ack_completes_only_after_peer_writer_flush",
    "network::handle_update_tests::progress_ticket_rejects_a_different_same_length_payload",
    "network::tests::configured_assist_hub_connection_cannot_overflow_reliable_geometry",
    "network::tests::topology_larger_than_reliable_target_geometry_is_rejected_atomically",
    "network::tests::assist_hub_refresh_above_reliable_geometry_is_rejected_atomically",
    "network::tests::topology_removal_cancels_every_deferred_owner_for_removed_peer",
    "network::tests::deferred_progress_survives_ttl_but_explicit_peer_removal_cancels_it",
    "network::tests::outside_topology_retransmit_is_not_misreported_as_delivered",
    "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
    "network::handle_update_tests::targetized_broadcast_coalesces_only_the_same_digest_and_membership",
    "network::tests::distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases",
    "network::tests::exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not",
    "network::tests::removed_membership_cancels_only_old_broadcast_debt_across_readd",
    "network::tests::cancelled_target_child_with_pending_flush_ack_releases_exactly_once",
    "network::tests::requested_topology_is_not_authority_and_closed_fanout_returns_all_targets",
    "network::tests::reliable_delivery_waits_for_its_route_subscriber",
    "network::tests::closed_reliable_subscriber_transfers_actor_pending_backlog_to_replacement",
    "network::tests::network_actor_drop_retires_routes_and_only_its_waiters",
    "consensus_message_control::tests::controlled_v2_admission_preserves_distinct_relay_identity",
    "network_relay_tests::test_control_hold_release_preserves_live_route_and_retires_canceled_reentry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
    "merge_sidecar::tests::sidecar_flush_refinement_advances_only_exact_source_chunk",
    "sumeragi::v2::tests::deferred_actor_source_never_aliases_across_adapter_instances",
    "sumeragi::v2::tests::deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step",
    "sumeragi::v2::tests::deferred_authenticated_retry_retains_exact_original_and_effective_tags",
    "sumeragi::v2::tests::deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap",
    "sumeragi::v2::tests::deferred_service_debt_overflow_is_typed_and_fail_closed",
    "sumeragi::v2::tests::deferred_service_evidence_rejects_every_owner_and_rank_mutation",
    "sumeragi::v2::tests::deferred_zero_ordinal_is_exact_single_use_and_never_reminted",
    "sumeragi::v2_effects::tests::live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks",
    "sumeragi::v2_effects::tests::recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_coalescing_preserves_alternate_ingress_owners",
    "sumeragi::v2_runtime::tests::adapter_command_identity_is_derived_from_exact_immutable_payload",
    "sumeragi::v2_runtime::tests::admission_ordinal_exhaustion_fails_runtime_closed",
    "sumeragi::v2_runtime::tests::runtime_rejects_replayed_foreign_and_mutated_deferred_tokens",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields",
    "sumeragi::v2_runtime::tests::scheduler_owner_must_be_taken_before_a_later_step_can_enter",
    "sumeragi::v2_runtime::tests::selected_owner_without_a_runtime_minted_ordinal_fails_closed",
    "sumeragi::v2_worker::tests::exact_output_coalescing_preserves_distinct_fair_ingress_admissions",
    "sumeragi::v2_worker::tests::orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes",
    "sumeragi::v2_worker::tests::sidecar_flush_ack_identity_mismatch_fails_closed",
    "network::tests::reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence",
    "network::tests::reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor",
    "consensus_message_control::tests::failed_release_clears_in_flight_ownership_and_latches_fatal",
    "consensus_message_control::tests::fatal_controller_rejects_an_unchanged_command_poll",
    "consensus_message_control::tests::retired_release_finishes_drain_without_claiming_delivery",
    "network_relay_tests::obsolete_sumeragi_relay_message_completes_as_delivered",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
    "merge_sidecar::tests::authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes",
    "sumeragi::v2::tests::authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer",
    "sumeragi::v2_effects::tests::owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal",
    "sumeragi::v2_effects::tests::certified_body_response_carrier_swap_fails_closed_before_fetch_mutation",
    "sumeragi::v2_runtime::tests::runtime_merges_alternate_sources_for_one_semantic_request",
    "sumeragi::v2_runtime::tests::runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent",
    "sumeragi::v2_runtime::tests::busy_deferred_request_merges_alternate_source_and_services_exact_carrier",
    "sumeragi::v2_worker::tests::owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors",
    "network::tests::peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures",
    "parameters::actual::tests::sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
    "sumeragi::v2_core::tests::future_view_commit_qc_uses_current_owner_through_application",
    "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
    "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
    "sumeragi::v2_core::tests::stale_generation_completion_is_rejected_after_view_change",
    "sumeragi::v2_core::tests::stale_persistence_completions_stutter_while_current_append_is_pending",
    "sumeragi::v2_core::tests::strictly_ahead_install_timeout_advances_owner_and_protects_highest_prepare",
    "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
    "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
    "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
    "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
    "sumeragi::v2_core::refinement::tests::durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations",
    "sumeragi::v2_core::refinement::tests::locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations",
    "sumeragi::v2_core::reducer::source_link_tests::certified_fetch_capability_requires_the_exact_proposal_origin",
    "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
    "sumeragi::v2_core::wal::byte_lifecycle_tests::same_round_timeout_replay_accepts_only_a_strict_prepare_origin_upgrade",
    "sumeragi::evidence::tests::sumeragi_v2_equivocation_authenticates_vote_origin_and_execution",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_ownership_projection_ignores_route_liveness_until_maintenance",
    "sumeragi::v2::tests::deferred_projection_distinguishes_authenticated_proposal_origins",
    "sumeragi::v2::tests::vote_body_ownership_uses_the_authenticated_proposal_origin",
    "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
    "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
    "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
    "sumeragi::v2_lane_work::tests::prior_height_hydration_stays_local_under_successor_backpressure",
    "sumeragi::v2_runner::tests::first_same_subject_lock_from_prior_view_retires_unlocked_work",
    "sumeragi::v2_runtime::tests::exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot",
    "sumeragi::v2_runtime::tests::same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically",
    "sumeragi::v2_runtime::tests::runtime_ingress_carrier_capacity_returns_backpressure_atomically",
    "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
    "zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin",
    "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
    "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
    "offline::kagemusha_v4_topup_provenance_tests::compact_qc_rejects_foreign_or_future_proposal_origin",
    "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
    "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
    "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
    "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
    "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
    "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
)
_PRODUCTION_LIVENESS_RETIRED_REGRESSIONS = frozenset(
    (
        "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
        "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
        "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
        "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
        "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
        "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
        "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
        "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
        "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
        "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
        "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
        "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
        "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
        "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
        "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
        "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
        "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
        "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
        "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
        "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
        "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
        "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
        "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
        "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
        "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
        "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
    )
)
_PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS = (
    "merge_sidecar::tests::reused_actor_ordinals_under_different_tenures_are_rejected_atomically",
    "merge_sidecar::tests::reply_unwritable_route_parks_inflight_materialization_without_bytes",
    "merge_sidecar::tests::exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon",
    "merge_sidecar::tests::unsent_request_restores_holder_and_backoff_state",
    "merge_sidecar::tests::idle_request_retry_starts_strictly_after_the_fairness_cursor",
    "merge_sidecar::tests::request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix",
    "merge_sidecar::tests::authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression",
    "merge_sidecar::tests::rejected_request_does_not_consume_server_stream_state",
    "merge_sidecar::tests::height_rollover_retries_only_each_sources_current_in_flight_chunk",
    "merge_sidecar::tests::durable_requester_restart_advances_sequence_and_carries_close_floor",
    "merge_sidecar::tests::durable_requester_crash_before_send_closes_unobserved_sequence",
    "merge_sidecar::tests::durable_stream_epochs_and_service_generations_bound_peer_churn",
    "merge_sidecar::tests::durable_lifecycle_rejects_canonical_payload_with_stale_digest",
    "merge_sidecar::tests::durable_responder_restart_preserves_same_hub_gate_budget",
    "merge_sidecar::tests::durable_responder_restart_allows_new_source_while_recovered_source_is_offline",
    "merge_sidecar::tests::durable_responder_restart_preserves_terminal_source_cursor_and_rebinds_capability",
    "sumeragi::v2_lane_work::tests::sidecar_lifecycle_journal_failure_latches_restart_before_request_dispatch",
    "sumeragi::v2_lane_work::tests::sidecar_close_journal_failure_latches_restart_and_blocks_queued_chunk",
    "sumeragi::v2_lane_work::tests::sidecar_close_ack_journal_failure_latches_restart_before_completion",
    "sumeragi::v2_lane_work::tests::sidecar_timeout_journal_failure_latches_restart_before_retry_dispatch",
    "sumeragi::v2_worker::tests::delayed_old_tenure_delivery_cannot_replace_newer_worker_reply_route",
    "sumeragi::v2_worker::tests::ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses",
    "sumeragi::v2_worker::tests::ordinary_reply_late_old_flush_after_reconnect_advances_exactly_once",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_replaced_connection_to_actor",
    "network::tests::delayed_superseded_tenure_cannot_replace_or_tombstone_newer_same_source_writer",
    "network::tests::reply_wrapper_exposes_delivery_active_unwritable_no_ownership",
    "network::tests::accepted_draining_connection_delivers_reliable_progress_after_replacement",
    "network::tests::reply_route_tenure_retires_only_after_final_receiver_guard_drops",
    "consensus_message_control::tests::private_reader_treats_safe_atomic_replacement_as_retryable_identity_churn",
    "network_relay_tests::certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical",
    "sumeragi::v2::tests::strict_same_round_tc_preserves_and_retags_timeout_vote_owners",
    "sumeragi::v2_core::tests::later_reproposal_commit_qc_replays_and_applies_its_exact_certified_round",
    "sumeragi::v2_core::tests::valid_commit_qc_supersedes_different_subject_prepare_lock_live_and_replay",
    "sumeragi::v2_effects::tests::different_subject_decision_supersedes_protected_lock_and_frees_losing_capacity",
    "sumeragi::v2_effects::tests::apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work",
    "sumeragi::v2_core::tests::height_context_requires_one_same_round_parent_commit_geometry",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_rebinds_lock_and_retains_current_timeout_vote",
    "sumeragi::v2_core::tests::later_reproposal_commit_ack_retires_durable_old_round_commit_pool",
    "sumeragi::v2_core::tests::tc_lock_survives_closed_view_and_commits_after_later_same_subject_reproposal",
    "sumeragi::v2_core::tests::replay_resigns_same_subject_reproposal_fifo_without_relabelling_old_commit",
    "sumeragi::v2_core::refinement::tests::strict_same_round_refinement_kernels_reject_split_round_mutations",
    "sumeragi::v2_core::refinement::tests::wal_retirement_authorization_rejects_split_round_decision_and_receipt",
    "sumeragi::v2_core::refinement::tests::semantic_commit_decision_identity_ignores_only_qc_rounds",
    "sumeragi::v2_core::reducer::source_link_tests::closed_proposal_round_cannot_create_a_new_commit_intent",
    "sumeragi::v2::tests::locked_subject_reproposal_and_strict_higher_prepare_are_safe",
    "sumeragi::v2::tests::successor_context_requires_the_durable_cryptographic_parent",
    "sumeragi::v2::tests::authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
    "sumeragi::v2_effects::tests::authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally",
    "sumeragi::v2_effects::tests::reproposal_commit_qc_applies_the_exact_unchanged_body",
    "sumeragi::v2_effects::tests::reproposal_commit_signing_uses_its_same_round_validation_marker",
    "sumeragi::v2_runtime::tests::exact_authenticated_timeout_certificate_coalesces_then_applies_through_signer",
    "sumeragi::v2_runtime::tests::body_available_rebind_accepts_same_view_higher_generation",
    "sumeragi::v2_transport::tests::reproposal_commit_qc_authenticates_its_exact_same_round_body",
    "block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round",
    "block::consensus_v2::tests::height_context_identity_ignores_reproposal_round_and_rejects_split_rounds",
    "sumeragi::v2_core::tests::vote_statement_identity_excludes_only_the_authenticated_signer",
    "sumeragi::v2_core::tests::certificate_height_subject_identity_ignores_round_and_phase_only",
    "sumeragi::v2_core::tests::view_zero_binds_semantic_parent_decision_across_reproposal_rounds",
    "sumeragi::v2_core::tests::earlier_same_body_commit_qc_supersedes_a_later_reproposal_lock",
    "sumeragi::v2::tests::registry_rejects_split_round_vote_and_qc_reference",
    "sumeragi::v2_body_store::tests::rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round",
    "sumeragi::v2_body_store::tests::rotating_leader_reproposal_authenticates_the_immutable_header_leader",
    "sumeragi::v2_effects::tests::deferred_merge_sidecar_accepts_earlier_carrier_and_rejects_future_or_foreign",
    "sumeragi::v2_effects::tests::split_round_commit_signing_is_rejected_before_service_dispatch",
    "sumeragi::v2_runner::tests::exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift",
    "sumeragi::v2_runner::tests::replayed_proposal_sign_reserves_only_the_exact_current_lock_owner",
    "sumeragi::v2_worker::tests::closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance",
    "sumeragi::v2_worker::tests::closed_flush_racing_final_receiver_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::unavailable_admission_racing_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::entered_view_accepts_same_view_higher_generation_supersession",
    "peer::run::tests::peer_task_abort_drains_queued_worker_then_notifies_exact_connection_once",
    "peer::run::tests::peer_task_panic_closes_delivery_producer_and_notifies_exact_connection_once",
    "peer::run::tests::dispatch_worker_join_error_is_returned_after_fail_closed_teardown",
    "network::tests::duplicate_configured_termination_does_not_advance_backoff_or_metrics",
    "sumeragi::v2_lane_work::tests::late_old_sidecar_flush_removes_only_reconnected_source_retry",
    "tests::relay_fairness::hold_release_same_source_reconnect_retires_old_delivery_without_rebinding_new_route",
    "network::tests::deferred_queue_preserves_order_and_connection_bindings",
    "network::tests::flush_deferred_frames_closed_session_restores_remaining_unbound",
    "network::tests::flush_deferred_frames_drops_stale_connection_binding_without_posting",
    "network::tests::flush_deferred_frames_rebinds_reliable_stale_connection",
    "network::tests::flush_deferred_frames_sends_unbound_entries_to_current_session",
    "network::tests::live_session_backpressure_defers_retry_with_current_connection",
    "network::tests::live_session_closed_defers_retry_unbound_and_removes_peer",
    "network::tests::live_session_post_overflow_disconnect_policy_defers_unbound",
    "network::tests::missing_session_retains_unbound_consensus_frame_and_schedules_reconnect",
    "network::tests::rejected_authenticated_connection_is_cancelled_and_remains_cap_accounted",
    "sumeragi::v2_core::tests::recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade",
    "sumeragi::v2_core::tests::recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification",
    "sumeragi::v2_core::tests::replay_accepts_strictly_higher_matching_prepare_qc_proposal",
    "sumeragi::v2_core::tests::replay_resigns_proposal_with_equivalent_parent_reproposal_round",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_is_exact_local_proposal_justification",
    "sumeragi::v2_core::refinement::tests::decision_ack_retires_competing_owners_and_keeps_one_body_pipeline",
    "sumeragi::v2_core::refinement::tests::lock_and_commit_requires_one_current_vote_and_proposal_round",
    "block::consensus_v2::tests::timeout_proposal_accepts_only_the_selected_prepare_subject",
    "sumeragi::v2_core::tests::future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership",
    "sumeragi::v2_core::tests::tc_omitting_the_local_high_keeps_its_exact_prepare_qc_retransmittable",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_projection_selects_and_fetches_the_exact_post_install_lock",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_without_a_lock_carries_and_fetches_nothing",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_effect_cannot_substitute_an_equal_reference_certificate",
    "merge_sidecar::tests::full_server_table_never_advances_generation_without_a_changed_roster",
    "sumeragi::v2_lane_work::tests::duplicate_generation_hint_coalesces_alternate_reply_sources",
    "sumeragi::v2_runner::tests::relayed_generation_hint_preserves_reply_route_from_lane_through_worker",
    "sumeragi::v2_worker::tests::generation_hint_requires_exact_reply_route_ownership",
    "network_relay_tests::certified_merge_sidecar_messages_preserve_ingress_reply_route",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_required_serve_gate_precedes_open",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_churn",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes",
    "sumeragi::authoritative_runtime_gate_tests::restored_productive_retry_stays_behind_an_earlier_certified_request_carrier",
    "sumeragi::v2_worker::tests::exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
    "sumeragi::v2_worker::tests::completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
    "sumeragi::v2_worker::tests::repeated_exact_serve_claims_close_all_older_sources_before_later_io",
    "sumeragi::v2_worker::tests::exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission",
    "sumeragi::v2_worker::tests::fair_ingress_exact_ticket_coalesces_and_commits_before_later_io_producers",
    "sumeragi::v2_worker::tests::drained_exact_retransmission_gets_fresh_scheduler_ordinal",
    "sumeragi::v2_worker::tests::fair_ingress_gate_overflow_closes_without_partial_admission",
    "sumeragi::v2_worker::tests::fair_ingress_classifies_current_historical_future_and_unauthenticated_requests",
    "sumeragi::v2_worker::tests::fair_ingress_rollover_retires_ticket_before_old_service_teardown",
    "sumeragi::v2_worker::tests::fair_ingress_producer_episode_wins_or_yields_without_partial_exact_admission",
    "sumeragi::v2_worker::tests::fair_ingress_full_prefix_materializes_exact_serve_before_later_churn",
    "sumeragi::v2_worker::tests::fair_ingress_serve_only_prefix_materializes_after_frozen_completion_ack",
    "sumeragi::v2_worker::tests::fair_ingress_terminal_retry_replays_without_lifecycle_resurrection",
    "sumeragi::v2_worker::tests::fair_ingress_higher_view_waits_out_active_family_before_admission",
    "sumeragi::v2_worker::tests::durable_serve_restart_before_terminal_seal_locally_completes_without_retry",
    "sumeragi::v2_worker::tests::durable_coalesced_retransmission_restart_locally_completes_without_retry",
    "sumeragi::v2_worker::tests::restored_serve_waiter_advances_shared_runtime_source",
    "sumeragi::v2_worker::tests::durable_serve_abort_before_commit_restarts_into_local_completion",
    "sumeragi::v2_worker::tests::durable_serve_seal_before_completion_post_restores_terminal_replay",
    "sumeragi::v2_worker::tests::durable_serve_seal_survives_post_before_physical_ack",
    "sumeragi::v2_worker::tests::durable_serve_corruption_fails_closed_without_highwater_reset",
    "sumeragi::v2_worker::tests::durable_serve_frame_bound_covers_max_layout_manifest_hashes",
    "sumeragi::v2_worker::tests::durable_higher_view_abort_republishes_displaced_terminal_before_restart",
    "sumeragi::v2_worker::tests::durable_higher_view_admission_crash_locally_completes_successor_union",
    "sumeragi::v2_worker::tests::durable_serve_restore_rejects_capacity_owner_swap_across_replacement",
    "sumeragi::v2_worker::tests::durable_serve_state_is_pruned_only_with_successor_rollover_root",
    "sumeragi::v2_worker::tests::certified_serve_future_slot_blocks_control_and_consensus_replenishment",
    "sumeragi::v2_worker::tests::certified_serve_cross_relay_retry_replays_one_terminal_tombstone",
    "sumeragi::v2_worker::tests::certified_serve_terminal_rejects_mismatched_response_hash_without_releasing_owner",
    "sumeragi::v2_worker::tests::certified_serve_observer_owner_contains_prepare_and_commit_subfamilies",
    "sumeragi::v2_worker::tests::certified_serve_higher_view_abort_restores_terminal_high_watermark",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_aborts_reserved_replacement_without_orphan",
    "sumeragi::v2_worker::tests::certified_serve_delayed_lower_view_cross_relay_cannot_resurrect",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence",
    "sumeragi::v2::tests::deferred_occurrence_capability_binds_direct_authenticated_provenance",
    "sumeragi::v2_runtime::tests::runtime_rejects_driver_selection_outside_eligible_deferred_owner_set",
    "sumeragi::v2_runtime::tests::runtime_physical_cut_is_monotone_and_regression_fails_closed",
    "sumeragi::v2_runtime::tests::deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences",
    "sumeragi::v2_runtime::tests::post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target",
    "sumeragi::v2_runtime::tests::pre_dequeue_probe_validates_unfrozen_leader_wire_identity",
    "sumeragi::v2_runtime::tests::busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation",
    "sumeragi::v2_worker::tests::invalid_requester_signed_qc_quarantines_one_family_without_consuming_honest_capacity",
    "sumeragi::v2_effects::tests::fetch_retransmissions_reuse_one_work_slot_and_one_signed_request",
    "sumeragi::v2_effects::tests::apply_retransmissions_reuse_one_work_slot",
    "sumeragi::v2_runtime::tests::distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner",
    "sumeragi::v2_worker::tests::durable_reconstructed_body_terminalizes_late_chunk_across_arrival_order",
    "sumeragi::v2_worker::tests::productive_retry_after_proofless_reconstruction_does_not_become_orphan",
    "sumeragi::v2_effects::tests::late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
    "sumeragi::v2_lane_work::tests::native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_excludes_coordinator_only_receipts",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_route_identity_conflict",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_duplicate_group_source",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_matches_decoded_replay_entry",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_height_participant_identity_conflict",
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = tuple(
    test_name
    for test_name in _PRODUCTION_LIVENESS_NEW_REGRESSIONS
    if test_name not in _PRODUCTION_LIVENESS_RETIRED_REGRESSIONS
) + _PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS

# The retained executor queue is the concrete FIFO consumer for reducer
# batches. Bind the complete reviewed methods, then additionally check the
# semantic ordering fragments below so failures identify the violated seam.
_PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256 = {
    "consume_effects": (
        "59deecb4349fb6061e438508940ffb96dd4dcc87169db3c23727f18267a3e6ce"
    ),
    "retain_effect_batch": (
        "3976c357aa3b66c71eac8bf8003bab79e024f76e9e469468bb8fb86c4c58dcfe"
    ),
    "retain_effect_batch_at_frontier": (
        "1e31788cecc9c7b9240ec50b1943f8db28ae206e2b5df3aca9aeefd054052695"
    ),
    "preflight_effect_batch_frontier": (
        "d2debd37b0ef330c5dba99fd01e483e44edc4be3df5ccee02513ef7203f3f96d"
    ),
    "prepare_parked_effects_for_frontier": (
        "0c71370501ce7e04b6ab7abbad10ef79b471d74c9f20f8ada3e512d4514671a9"
    ),
    "commit_reconciliation_frontier": (
        "1deed24b38bdba50796b402def3fc3ef9e9eaad44c93449e8e96f1bdb38c430c"
    ),
    "drain_retained_effect_batch": (
        "b796ca52147def7d7642b5f8c352f55e6ab2214b3c8d5c1a631ac5aa9c64bba5"
    ),
    "consume_pacemaker_effects": (
        "36d01c500f897902a5680bf290b58076d1ca7b959fc6d20973f6df85900f92b2"
    ),
    "consume_pending_tip_recovery_effects": (
        "275bb4749f89c76810b7154c1eca151f2c85e970c162fb3f737034870520578b"
    ),
    "step_pacemaker_once": (
        "09140d1b32a493a591303e376d5ed642ea1479cb2fddae358d8d4d9b5b2a7df5"
    ),
    "step": "c56df5c78384e0685390837f6442bf17f81f37134a840445cb2522326a226bcb",
    "step_pending_tip_recovery": (
        "2fea51da5e34af31e90c8a42ed6915cb8d6e64dc18a7df89ef0a0545b81b6f94"
    ),
}

_PRODUCTION_EFFECT_SCHEDULER_HANDOFF_ITEM_SHA256 = {
    "take_scheduler_ownership": (
        "bccdd7c65959f3921d46373c86d1d6545568a326ddea42379362c7b1970397cf"
    ),
}

# Exact comment/literal-free token digests for the production exact-output
# scheduler and applied-height retirement seam.  The scheduler digests bind
# target-local/global FIFO ownership, round-robin service, and exact returned
# post retention.  The retirement digests bind the narrow Retransmit whitelist
# and the Kura receipt/finality-artifact authority checked before output is
# handed to durable reconstruction.
_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256 = {
    "take_attempt": "acc18d3997a0cc6fcca4926b72a63fedf5d0987ecb33c1114e93e0da3b2254d7",
    "mark_admitted": "c6e502433ef5249540446d75e0f88f665a7ffc456bf4014216f808fd123f072c",
    "retain_returned": "8f1436db10edaa22360b416024817e8da58752a2fa7b604d97b0f6819976b0cd",
    "owns_source": "6a89444dd116f019fabd7ef2465b5ccc9d0b404adbf783dc47d47ad150ebe1b2",
    "target_is_local_head": "a66a280165f7945efa150447183cb391824878b3d002c7cd7d964f60d8a44096",
    "advance_target_cursor": "b16e6854e6b213b04e89a740f8b20f1c00f13ae88e8da38a39faac55ec6b0c26",
    "handoff_applied_height_to_durable_reconstruction": (
        "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80"
    ),
    "target_is_global_head": (
        "f71465dbc379cc235d6669a17c7cc6eca1b456acbf5b8b996ab39ebebbac05ee"
    ),
    "next_schedulable_target": (
        "46a5fc46be8b95eb70516a03c7fc52537238a7fe26545540a1a85413e3465475"
    ),
    "advance_after_attempt": (
        "e678eb75bf1e124b8c3ac7b196bc9abee8d5429a6f064a1aaf64851291bc0e07"
    ),
    "drive_with_budget_ack": (
        "675d0e99a923f9b8cc96d725458584f4ea762cd4fd69fd9f8e7e76b158c82d52"
    ),
    "drive_bounded_with_ack": (
        "7e630c6be2466fe0980df81ea194e101b70e4abac9546a379ab9f24c8eb23861"
    ),
    "applied_height_reconstruction_covers": (
        "2dde9c6c1a14c499bca48506c2b99f20f0b07709ef34eb7e0c92657e4449e9e6"
    ),
    "durable_exact_output_handoff_owner_pair": (
        "f848a53734e173e64122e3cf198d6e0adfea99909594e667c56a7e11ea271a84"
    ),
    "certified_sidecar_prefix_covers_occurrence": (
        "019ae94e6c1ebdea5e6948361cf117d49003d4433cd9daa7c3a28f2e1e391741"
    ),
    "handoff_applied_height_output_to_durable_reconstruction": (
        "7444ffcb40c52af925865f03520de3eac64f59d1c31ae6e549c5bf92e17aaf84"
    ),
}

# Complete token seals for the source-isolated exact-output corridor.  These
# complement the projection checks below: a mutation cannot preserve one
# positive subsequence while inserting an early return, dropping a sibling
# source, or disconnecting a writer-flush receipt elsewhere in the same item.
_PRODUCTION_MERGE_SIDECAR_SEAM_ITEM_SHA256 = {
    "CertifiedMergeSidecarChunkAdmission::from_admitted_reply": "a2b593a56511c97eae5bfaca2235490ace31a128884729704bb6dc907a98da44",
    "apply_reliable_flush_application": "93912d6c690daa60be7fda65daa0a997a0e55839d3661560ada00aaca8dd8a52",
    "MergeSidecarTransport::with_limits_and_server_stream_capacity": "acdb21ade067f5460c4019cd4fb0f65741d2f2707ccd1ac447d260641dc3b7f1",
    "MergeSidecarTransport::derive_server_request_capacities": "02926b83f40ccd433a5d18f79b4cf725d81a81d198d0b5e4ac043f09354f52a2",
    "MergeSidecarTransport::begin_request_or_close": "8fae96ccb47e45e090ba0ddad6359153951f47583e23be35c6eccfa6ce61e3ca",
    "MergeSidecarTransport::release_unsent_request": "dab1c81134717c57640cd64da49a68ab99cf61d474e02d7b4f1c0ee0b398793a",
    "MergeSidecarTransport::release_authorized_server_request_attempts": "713f97a0a252983fffcd258d631f1052cd3801f634d8554d1febcdc65cf689d9",
    "MergeSidecarTransport::park_authorized_server_request_attempts": "a7f6ad8f3a595fe655f0535b11d2564a2a783d60df1cc6bebeadb704606e96f6",
    "MergeSidecarTransport::reclaim_inactive_outbound_attempts": "9600b575eb21edcb26955867d706880f5040c062ee84b45c266efeb5d484a1a7",
    "MergeSidecarTransport::prune_server_gates": "13f9e547c581b576aed5a552fcd2ecd2f0ea7260fe5a72c7bd7a78832ceb61c2",
    "MergeSidecarTransport::server_request_source": "c1c1f62439ac7e654a67db7cd7de13504315c3da463da0ac7d639e8c3722bffd",
    "MergeSidecarTransport::source_gate_count": "61c262742b3758298046503a720335804b1981748c8bc156527dce9705c80bc8",
    "MergeSidecarTransport::server_gate_attempt_count": "e9c80512a68ae8368554aa4e6990a625ca57a5b6c5fb47ecb5b7bf1598a38acb",
    "MergeSidecarTransport::outbound_attempt_count": "d0b5ac92c19eab61d75f4b461c6ecb4a64d6861fd5baf194a52737bf4843879f",
    "MergeSidecarTransport::source_outbound_count": "3b17f1f9955d281381cdf58b38f910687a62423dc48f8ddfa96fc150d401f451",
    "MergeSidecarTransport::global_outbound_bytes": "1ec3ffa56334677b60c9c920e8e15a562ed04653674849e665b2d44c623c73a6",
    "MergeSidecarTransport::source_outbound_bytes": "2c4678e27b465dd69d9b36e07d9f8b1e85b56f2568282a90f9bbef47a919c3c7",
    "MergeSidecarTransport::route_update": "2f8d4bf918efcb9070aedbb7e4d57baa89f8127ebcf4375c53af35afa852856a",
    "MergeSidecarTransport::alternate_source_is_authorized": "8ccfab565eb86d3527950b783f0ef326303e0a386dc10f268e25f72fcee25421",
    "MergeSidecarTransport::route_source_capacity": "162b152d56446bffa077c6005f4457ce2757a26a73306018a80cc53f68a9ba25",
    "MergeSidecarTransport::can_add_outbound_attempt": "7ec9222597fcb0ada3b06a589d775066cd94335c8d6bb2cfcb2f685fb922c43d",
    "MergeSidecarTransport::next_server_request_materialization": "445d03272e85e2bf3f7e3f054c2acfa2d8ff6434b86c62defca965bc42bc3002",
    "MergeSidecarTransport::admission_after_fair_materialization_selection": "26e50ac363ea2e8c75497ad2df045ab1dcd4ed2315db16147cd9488be04d82e9",
    "MergeSidecarTransport::admit_server_request": "78fcf00b793e0ef8b871657c83c3b05b8b81149e789470faca831fd8c3b50fc8",
    "MergeSidecarTransport::cancel_unmaterialized_server_request": "40f0831462c1e44e4e55919d9ba9dfc443b6901dd963c47ad79106ce274586fd",
    "MergeSidecarTransport::enqueue_response": "2e11a16de83ba8372b3ad92e58e6b222f9268d70611be2bb1c56f765f4aaa06b",
    "MergeSidecarTransport::drain_outbound_chunks_durable": "b15f79eea3fa0a065bed3026d821e05a7562e379e24dff34db19c8b32b601ecd",
    "MergeSidecarTransport::acknowledge_outbound_chunk": "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484",
    "MergeSidecarTransport::tick_bounded": "8a01af0d92b9474c1b9c4ff0c861d2b29315b4681062e1db668a4b1db543c541",
}

# Complete seals for the crash-safe semantic request lifecycle. Process-local
# reply capabilities are deliberately absent: only stable peer ownership,
# sequence floors, terminal/pending cursors, and immutable pending-chunk
# identities may survive a restart.
_PRODUCTION_MERGE_SIDECAR_LIFECYCLE_ITEM_SHA256 = {
    "RequestStreamState::allocate": "035e187a842019c9adf402f2984a0e69ea7de331056e6038d0f3974311e566ee",
    "RequestStreamState::close": "ea56cf4ad45b41875017f40148321ce2189dc77919fd6dbd60ba83b45107166f",
    "RequestStreamState::emit_close": "662b1d15ddcaa2ba9b0ef4c776e7d30647f5aab383ffddeb5bf6dd567178a552",
    "RequestStreamState::acknowledge_close": "4a2e627dd28de3c332a45c68a351d86bb392ebf07a9c4a4fa8021399bfb78dd3",
    "ServerRequestSource::budget_source": "ff85c7db75fcbff14abacdbd2d23351e5c244b8046989f7ed6d8feb2bfe59663",
    "ServerRequestSource::shares_budget_with": "d2fccc4fab299dac39f9a6bac5639887a4dd6e115eae29ec90579da93c75051e",
    "CertifiedMergeSidecarGenerationHintV1::canonical_hint_id": "2731d88621ae21e7119162b150578b5419f3b1a7e04676f9565e6d2f78a0ebe7",
    "MergeSidecarLifecycleSnapshotV3::new": "b76fc1b3f9d52cd47e221b55ca35cd814bf86290cac86b7de8c8150dff0d8754",
    "MergeSidecarLifecycleSnapshotV3::integrity_is_valid": "413dd7181f9898e41bb9b9f2bb826ceaa793c44f9a4a12943d36fe58ae32cf0c",
    "MergeSidecarLifecycleRootHighWaterV3::bootstrap": "09c144912ad447177997f12fc31de749830f39e500a726e7ca7d0490ccf0de51",
    "MergeSidecarLifecycleRootHighWaterV3::new": "6fb62886f1fc7515ccb93708a257239f55e2fcb252543fc170427cd2ed4944fe",
    "MergeSidecarLifecycleRootHighWaterV3::is_bootstrap": "af4f8d7064a8e7d4334a2fd28b6979ce8f97040f9a8fcdf79840e823b6e41621",
    "MergeSidecarLifecycleRootHighWaterV3::matches": "3138b2e1831a9710b7dcd84570fdeceaef945398cb0bb1e16ba7ff585a431d1f",
    "MergeSidecarLifecycleJournal::open": "81f2b35d6df08c5cac28ebb95fb240a7e5a949e52283c7f4ea5ccd8a357fdd33",
    "MergeSidecarLifecycleJournal::state_path": "e1a8db899e4e4ae845a2a5d5c7731c3ec7a4d04a57494a90876c761063b14789",
    "MergeSidecarLifecycleJournal::state_path_for_generation": "958b5b38f871dc10d788964dd806276f7112376c426412c402ff66d2086acf48",
    "MergeSidecarLifecycleJournal::temp_path": "4023b2cff04791329db79483fe6ac8dd2af85f90ded291d23c7cbbb33c17bf7a",
    "MergeSidecarLifecycleJournal::root_high_water_path": "fc93b03a37534d588c93fa488d50dad2909c133463a2d11cf0176bf42c8e05ad",
    "MergeSidecarLifecycleJournal::root_high_water_temp_path": "775a520e25b9c83c46aa08944341590719b4d59d0d2d12af95fb9d2c968099d8",
    "MergeSidecarLifecycleJournal::publish_bootstrap_marker": "40c9b879e62f60d03b3a81db96b1c67a9f97fefb36fe5453e7cc44ecdf836c92",
    "MergeSidecarLifecycleJournal::bootstrap_candidate": "5b0488a94395189e31943072b55da8dc71da1dda18106afeced87f73ed7c1048",
    "MergeSidecarLifecycleJournal::finalize_validated_open": "86894b3b15406d99916d483471c8ebb16ca3234745e93335768d100fd0bd1b0c",
    "MergeSidecarLifecycleJournal::sync_directory": "6bebdd14196431dd0c8c9160bca5965b8c902ae8fd9addd79596e6ff65ca9d8b",
    "MergeSidecarLifecycleJournal::artifact_exists": "f975c5a57d4c64323fa39d6fc2439658d13d3db6b944ad62805feadd841f7edb",
    "MergeSidecarLifecycleJournal::reject_artifact_if_present": "0d3908089c208e1abb49f232c1191b354960c100f1b3ce7dafceda699adfd7f6",
    "MergeSidecarLifecycleJournal::reject_known_temps": "3e359c0b5dc40cbb9e51b962ba8c840c9dd9837bca38378b58d38c95346ee06b",
    "MergeSidecarLifecycleJournal::remove_regular_artifact": "91167e9513b22b8b66d14f6fa3891510e4dcf88964f1ddaad777e6c69d76b911",
    "MergeSidecarLifecycleJournal::validate_regular_artifact_if_present": "4ab43f67d2bc6776e74b901e5cae02dd27c4259e4b5e525d86f71832893d2528",
    "MergeSidecarLifecycleJournal::validate_known_temps": "79f498aac06957b42a448cb12169f0e88612792854cf870e9065fc1501b14044",
    "MergeSidecarLifecycleJournal::discard_uncommitted_temps": "5075863b4359aff4aca8f37895fa497cfbcf82298ddef44e5d439a92e8a337f9",
    "MergeSidecarLifecycleJournal::discard_inactive_slot": "675c63c3195613a66fd9551590ebedd83e2f58527f13a0ceb04aab1f757926e9",
    "MergeSidecarLifecycleJournal::validate_directory_entries": "2c13ac3a3cec9f55d88eecf2f7781635f70fd70c824f71750e35332f6b9d2cec",
    "MergeSidecarLifecycleJournal::read_bounded_regular": "38f7bc44bed7b7c1f1d9847e75f65430dc6bd15532827ce888c0c8ba824e0563",
    "MergeSidecarLifecycleJournal::decode_snapshot": "2473d8b7f9040a2a041bbc46f9c23a8f8d1940ebc779a455edf63590f5a1c402",
    "MergeSidecarLifecycleJournal::decode_root_high_water": "76f8a0caa1f65018f82353e36d2bb07846cb8a49583aac60483dd7eab0c818c7",
    "MergeSidecarLifecycleJournal::load_pair_strict": "c3e273a90e4d180f08476d9bf7dd21b2ce91f0032b227857c09fbbc9b119cc5d",
    "MergeSidecarLifecycleJournal::load": "d7c2d8180e6579b1b8213fddf5ffcb984ef7025a52e66fa5ed4bc2d26c6bf6c7",
    "MergeSidecarLifecycleJournal::write_new_synced": "ab267b2611abb24926fad0ad306da0cb44a49eec565cd7450c76f762473fb2fa",
    "MergeSidecarLifecycleJournal::persist_atomic_replacement": "e8a844216e2110bf463f9884e7bff9d06b8dd0a9d6baff8cf1835bbb29798064",
    "MergeSidecarLifecycleJournal::live_generation": "8d2d97c739d2a32c7e899751791a8ef9528bcd6a5a1900a0f0f7e017a27a2fde",
    "MergeSidecarLifecycleJournal::preflight_next_commit": "5c58aa339bb87024e46da01db648c75a7da2e3e78f12fa7a285414ba70e103f0",
    "MergeSidecarLifecycleJournal::persist_next": "e4b9c82c810f07d1158140e054ffc8e3f07ed143b4e0d68f17e0f3d136c541e6",
    "MergeSidecarTransport::lifecycle_runtime_geometry_v3": "34f7fde0aec9a4d933b9bfd71afa80c9391911797b2db30074f01db8110d119f",
    "MergeSidecarTransport::lifecycle_geometry": "eae65fb2d75f710c039fec57b4b4cfa0801b6620393a1ce90958c6656cd6496b",
    "MergeSidecarTransport::lifecycle_geometry_for_server_roster": "4b6d81251a26903563d0d5a7b309e2b91ebc16fde3e9d78cbae72953d9694d45",
    "MergeSidecarTransport::lifecycle_max_snapshot_bytes_for_attempt_capacity": "6a8066cd1ab881b346c2f410cdcb3fef9001b787b2016de257df86457d9cd3f9",
    "MergeSidecarTransport::lifecycle_protocol_max_snapshot_bytes": "2ab3af73ff8454c4a11044f0b87d81e3484433de497d2c2f14392bd3c63b3a52",
    "MergeSidecarTransport::lifecycle_snapshot": "e51da54b26a259562e37b630f1a3c5e6f6ee6c3dffff2824557fc16801f4e6b5",
    "MergeSidecarTransport::restore_lifecycle_snapshot": "8627186b8d588686e3cb570af7f57cc72ee6781910adfb59c0c5de37b41d47b2",
    "MergeSidecarTransport::configure_prior_lifecycle_server_geometry": "ac39616a1ae23408941413d2472553d6234589168219cf31c0a47b7aed7dcfcd",
    "MergeSidecarTransport::open_durable_with_server_stream_capacity": "3e2ed1cb57775d36918a553dc3e17e065a4e994c87d1e67ddfef2ab3b8344a0a",
    "MergeSidecarTransport::persist_lifecycle_projection": "721b44da8a5911f197ad7326d8cfcb7919c8df7bbd1ee4413af1817f5b03eb11",
    "MergeSidecarTransport::preflight_lifecycle_mutation": "e792b37472d616f160d5dd5d2c28f869d35670df261e31ede7e56d8b93b55030",
    "MergeSidecarTransport::persist_lifecycle_state": "4a8658892bd135d9065f5f8aee9ad6beb73f7f3993b54dc3cb1e7c236bd5fd2b",
    "MergeSidecarTransport::rehydrate_with_exact_geometry": "95238c6a27265eaf9030443a5c92afdbbbe071ebf1337684f3b8f4b88b25aea2",
    "MergeSidecarTransport::rehydrate_with_exact_geometry_after_durable_handoff": "d5c8d8cc61fbe16f2ad8b91595ed63f6ca477e0c8b5269be6417ad486472e821",
    "MergeSidecarTransport::rehydrate_after_lifecycle_restore": "221f0ed6a140103b37b857a210da74887d77aab9a19400ef96b4f4993c5a420c",
    "MergeSidecarTransport::validate_retained_height_geometry": "a52a9db9ba2745b1555ff5422edc3c64fcbb3b910ae5f62216009866b706b923",
    "MergeSidecarTransport::requeue_retained_outbound_after_height_rollover": "a09ba68802f834de1a13219085ee6f24e7d1bca666a4c951b80501489eed0be8",
    "MergeSidecarTransport::allocate_request_sequence": "4a00c1f454545b38973384a4a4e968d8eb725f80b9d5c9b54b03afed0327c010",
    "MergeSidecarTransport::close_request_sequence": "d13ef957882ac705a2fdb715f72faf705c2d6c33248b354b61b834f82ce5ffb5",
    "MergeSidecarTransport::begin_close": "023c75991bc11e8528e4692fb11101e1e565f18f496be829eb20a4935f13a9f0",
    "MergeSidecarTransport::begin_request": "1e337d236787c31def6238bef35a2e461abcc78e70b47a54347cb7948e29195b",
    "CertifiedMergeSidecarClosedPrefix::covers": "21db3c4644755e3724626fc5c862b41346facb61576c135749144bd17ba803a6",
    "MergeSidecarTransport::acknowledge_close": "a6dc61f720126bf504dba5bc556385739819b18d6d9af845034b7e175d1ad0c5",
    "MergeSidecarTransport::acknowledge_generation_hint": "86b52bcc4c6063ea5ddf2ce983bd204784d5c129843e5af9f9db50c1d3cd4efd",
    "MergeSidecarTransport::generation_hint_post": "8f16175266e8ef580a910ddb39208d1963c471d058f164222428a90bf78eb922",
    "MergeSidecarTransport::preflight_server_request_stream": "039e4f6bf2d7ed58244185b7a5811f40b19737e953a603eed417f960546008c0",
    "MergeSidecarTransport::record_server_closure": "a8027bf18c82e939aed4072172cfe52b3c3116d5325a3f08c0b611ef95ffe6d9",
    "MergeSidecarTransport::server_generation_is_terminal": "91daaff340270d5a630e818719ce26a4b02d6264e80c4d4f2114bb2eea378f38",
    "MergeSidecarTransport::transition_server_service_generation": "442b14db6826182fa67c8563a3dd7412e7fd2bcc8824b8575640512547665675",
    "MergeSidecarTransport::transition_server_service_generation_after_durable_handoff": "7bf098b2a24c176a5bdfeb473f860986a3219675d10428be7550af1fa249a797",
    "MergeSidecarTransport::transition_server_service_generation_after_exact_output_fence": "68251fab01ae053907984e39eeac0779f1018d50cc069e791a8a85fe010e369a",
    "MergeSidecarTransport::prepare_server_service_generation_transition": "ae7d51586773145d9933369af877e09257c66def9ba4300d4c251109cdc0dabb",
    "MergeSidecarTransport::commit_server_service_generation_transition": "bafcbce053766a2f838c437d8c4f55cbbae22fe22f91e0e5a94944a707146684",
    "MergeSidecarTransport::ensure_server_stream_slot": "54412c913251408bb8da2595fb3fc36ef3b7df4486f09e8c85f8ab821f887597",
    "MergeSidecarTransport::supersede_server_stream": "4bfce430840bb6fd125779e211580bf98b60358736e735d2d124e8726d8a8bc1",
    "MergeSidecarTransport::advance_server_close_floor": "a8b0fe4fd6c15a17c344f1a318e39421efffaf3c24693e1965444d7c4ccb78b9",
    "MergeSidecarTransport::admit_server_close": "b4f0563eda7a12f58f3715b84f68ded706d03274c1744756ce8ba56a3cb2f073",
    "MergeSidecarTransport::drain_closed_server_prefixes": "517b76f57cca2c96ed8d1fbd53db8318b4003e09c7401dba9c8478ca7217f1c0",
    "MergeSidecarTransport::confirm_closed_server_prefix_handoff": "76c4d49654aebfb34456efc2fe5f0ac1737226a954bf3916bffe2ca5319ad7b7",
    "MergeSidecarTransport::server_gate_attempt_count_after_close": "d10ba051868323f576c5030b9d9c73c4c8debe4ae7bb0da21a04db9864e449f1",
    "MergeSidecarTransport::server_gate_count_after_close": "7b3476b7ec3e31013a84f9c331795d6c1218d03ad614734c2e17003e554baa0a",
    "MergeSidecarTransport::source_gate_count_after_close": "a065e18f460113a9ca617675037064f607d592e4edd456744a5ea8a2bcc97a24",
    "MergeSidecarTransport::drain_outbound_chunks_inner": "999e7704f60196377c7e7ca0c1f380b0e774d889ddf4ae34b2bfa084b96fcfee",
    "MergeSidecarTransport::finish_completed": "f3bfa218ed218197e1450d68ca7538c3dbc45c1ea5e2819035c49baaff3b3d62",
    "MergeSidecarTransport::discard_invalid": "d63d81f340e4309f0b795a5901f66e71ee0cb315910980012db0ec0f91b4e254",
    "MergeSidecarTransport::retain_pending_blocks": "498f9767b397cd0466e98d3fe8801d6a8de1d319169c19620a023a9e785e1117",
}

# Attribute-inclusive seals for every platform branch which establishes the
# directory/file identity used by the crash-safe lifecycle journal.  These
# helpers are part of the trust boundary: validating a path and then reopening
# it without `NOFOLLOW` or without a stable handle/path identity comparison
# would reintroduce a TOCTOU decode path.
_PRODUCTION_MERGE_SIDECAR_ARTIFACT_HELPER_SHA256 = {
    "lifecycle_artifact_identity": (
        "301e701d0753691df10a350b69a5eea47cdde38195e156c8b6dbaa5e72aa0d61",
        "9165ef4e4c97ef04d2ae55b5b00605853a2db11688432cbed96a61f567f3a86a",
        "ee13fb0e994aedad25ddab6ead76f0cf992219ab68beffb348f51b1c532bddaa",
    ),
    "lifecycle_artifact_revision": (
        "119f0656e2127e4c66bc6915ad8df49d2710006aad9744122cb17ead34b51e42",
        "9bea5709b4c9f9fb0f06d2003e3a26f696af5bf8cfc51b590a842e113cd0956c",
        "f3123c09a7b193028dbafb465fe169b2bf55d1e00fdba6f185a28d5c33b3b097",
    ),
    "lifecycle_artifact_identity_available": (
        "6b621ac96e7d735fdf061ca29f91228fa68e94eeeeb07f5a8205f56e7f48b225",
        "c21870b387d5cd786c93f51f3873466a002cd6dc9d532f3c950e7b53e44d4bdc",
        "f8d258b48fdda51575891c262d03635f0d8026f500740e5c03a24e67012e35d8",
    ),
    "lifecycle_artifact_is_single_link": (
        "91a822912011db14fdf25c7d86a33d69e72e2e25daf250d3513a3bc1252a0b82",
    ),
    "lifecycle_artifact_is_reparse_point": (
        "e0f9e851429da4dfe4f5b2691083c78fbf53272129be634fe06ddd64cbaf9514",
        "f3613fe601a454d7c1a960b49b86565671cec04b4c6dff410272ed37b722c2c5",
    ),
    "lifecycle_artifact_metadata_unchanged": (
        "2ea5f87b8ceb44ae3ef92320ec86c3c6b88b66d49bbb1dffc9574077189641db",
    ),
    "verify_open_lifecycle_directory": (
        "98f56261beb3cd37739864c2f0223e479177000fa6581bfde5cc85e2418e9da1",
    ),
    "open_lifecycle_directory": (
        "1c480a6f24d1d39a6fe9c7c37bb9cfc738cad69609082b66c60531f141f3fdec",
        "cfc8db5f4cd9eaee8a95a476440bb53c2f39423b12860d444ffd713c051f1ceb",
    ),
    "verify_open_lifecycle_regular": (
        "b3354225cd5d701857ffc3ddd06b958ceffeb4b3f611d5bb7af9980b7f52d12f",
    ),
    "open_lifecycle_regular": (
        "1762c07b05b671e00fa3b476b61267510a4313aac498d6f440f4b29651e3836e",
        "536e5ed345f17fe72ce9856541070e24c9cfad92ae8da06e8ef18c4024c54c2e",
    ),
}

_PRODUCTION_MERGE_SIDECAR_BOUNDARY_TEST_SHA256 = {
    "authenticated_source_quota_rejects_origin_churn_and_preserves_other_source": (
        "177b9dcad1c91ebd413bc7824e2a2340c20287fa89d1899f83df154146394233"
    ),
    "legacy_lifecycle_v1_snapshot_is_rejected_without_migration": (
        "c100cca083e1def146023e86038c60b04ae9a641a2e29dad0a7f2926e7232e83"
    ),
    "durable_responder_restart_preserves_same_hub_gate_budget": (
        "2c98602d0876f99438260e9accd99459d09334d014e6707e497cbdf7bdbe3ca6"
    ),
    "fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses": (
        "72591d58cb56aa08d3ea0bce920f7cf36c9a00f6b14593bc5dbf17f3834da444"
    ),
    "quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps": (
        "e5e5d27f264d8c30229eb80df95e7c95329c5fb42eb3370f0b3a4ddb1bd37342"
    ),
    "durable_lifecycle_v3_root_high_water_is_exact_monotonic_and_noop_stable": (
        "34edbc3a0e29cb88e9c672095506bd8e40aaeedeaf0faa63991baee4aa33c4d1"
    ),
    "durable_lifecycle_v3_bootstrap_recovers_first_commit_and_rejects_missing_roots": (
        "e01fd4052a5a78e492624de151d297dfaf91f44f47894a7645f355e253bf3f48"
    ),
    "durable_lifecycle_v3_rejects_crossed_bootstrap_and_committed_root_shapes": (
        "6288640952d9ea0e72797895aa2de7dd62186daf81da33497bd4bd5a89518c83"
    ),
    "durable_lifecycle_v3_recovers_regular_temps_and_rejects_unsafe_artifacts": (
        "cd06d9140fd9f052787048d9719feba1255e670d4eb0fa619aee24ced110be00"
    ),
    "durable_lifecycle_v3_validates_semantics_before_retiring_crash_artifacts": (
        "b9802b3eb8ebb243b566f6b8fad320369912c600111d3c045f36b9f1674cfd49"
    ),
    "durable_lifecycle_v3_rejects_split_generations_and_rehashed_state": (
        "bee7172a2a75e10ae2d34e2da9ebc8f688bf152cb9c74de324cd38bf9cf0ef10"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_close_mutation": (
        "529af4e365c92c84a976a06f6a1271bb41afc5878a4206bab39bfef06726b908"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_writer_flush_cas": (
        "22bd6d8a616406275daf917c063d1af71d5196ee9953320f4c6615873f8e85d8"
    ),
    "durable_lifecycle_v3_recovers_predecessor_before_state_directory_sync": (
        "6c51610572f862a27b7c4cdb704fd85b9cada24ea849313a61078091782fb518"
    ),
    "durable_lifecycle_v3_recovers_predecessor_between_state_and_root_publication": (
        "37fdf12044f2511e5125de87e8d468d61831be1328ca1778b4737e0518b38920"
    ),
    "durable_lifecycle_v3_resyncs_replaced_root_before_predecessor_cleanup": (
        "d8ac3e60c1d433064f25b8d75226080e3cfe98b0f955de14d29ec769505a631b"
    ),
    "durable_lifecycle_v3_recovers_successor_after_root_publication": (
        "7664893351c81f1a06df874c5a133539e6aab8f8d5ea95158419e2f8c8caa7c1"
    ),
    "durable_lifecycle_v3_rejects_missing_state_with_surviving_root_high_water": (
        "7da31a3cbc8c7d50c7cbb5015a1691b73827385de9598e4fa8cc5db8a53c7295"
    ),
}

_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256 = {
    "V2LaneWorkLimits::new": "1cbcc8e9fc532ca4c879c3339fda0a846bf71e9961e0e9bdd802c78880cf1090",
    "RetainedMergeSidecars::rehydrate_for_successor": "709b44e4cf845ffe76903ad4d7f61b9fa174ccc1f0a1a793d6733a37a23cd0a6",
    "V2LaneWorkAdapter::new_with_output_guard_and_transport": "017c1afe0515ff169d85bd9fd1a9ba86689ea35405952fe7647c66f42dabc8dd",
    "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner": "9d9d208f6f09526aed4f49f6f4ed9dc5a71808bdab5bd7576fd8c9284bbe5bb6",
    "V2LaneWorkAdapter::activate_after_lane_drain_queue_install": "638503cfcc9963213cb6146d16d82ab270016e6f5da7bbbc8b2918aea9120cb2",
    "V2LaneWorkAdapter::into_retained_merge_sidecars": "96d1e194eda3660ccccf9b4e1860b26a4db8d9ee9fb2bad4f988e37c85627218",
    "V2LaneWorkAdapter::accept_relay_message": "8c307167fe5ea486fc509817d12c352fa8a831751e0e7fc1342b6d43b6ef00bf",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar": "8e348b518da26f91fff8840d1e5bf4016d783fef4720bc275baba90ebe14a0bd",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_request": "7906403df13232aa6cbe853cfec670bea780f091d6887503c4a7420f2195ba92",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_close": "801722bdc8c28483601a0e35e98ede130f2f767af8cda9c7d423522c8e3a2c01",
    "V2LaneWorkAdapter::apply_closed_server_prefixes": "ca526c1cdcf5ade6263ec12641207c4edf83b5ca71c48a5ed39c8bcdc972a2db",
    "V2LaneWorkAdapter::coalesce_closed_sidecar_prefix": "70aeb1f315515d6c69f913afed4e4874b834d74972c68d1058a8b8916b03e771",
    "V2LaneWorkAdapter::drain_closed_sidecar_prefixes": "2d0a07530ab64b95f687a48fa09cd029a40bad9c18ef29bf82e8b624274bf2e3",
    "V2LaneWorkAdapter::requeue_closed_sidecar_prefixes": "97bdd3f66d265ba00e0cc088af59f005e7cfdc97c138cad9faa64138f944df2d",
    "V2LaneWorkAdapter::confirm_closed_sidecar_prefix_handoff": "7aaca416f7ad687d7ee0c1b99da038d11dbdc710d81229c340a63bda10476c28",
    "V2LaneWorkAdapter::stranded_retryable_sidecar_control_index": "09ed26efa19aefc39d448b1bee81d5b070c272557137e89a51c4f1dc6334419b",
    "V2LaneWorkAdapter::replace_stranded_retryable_sidecar_control": "13094aa7fc37a32648ebf74c461802e857173858f09d7c61ef0054530e340e4c",
    "V2LaneWorkAdapter::service_next_certified_merge_sidecar_materialization": "ac05dcfd3b1457f5632cbd3eb9fc337ee2a1200b6ce02e2cd154c392a3f5cc32",
    "V2LaneWorkAdapter::persist_anchored_sessions": "a8ea64ed2657d6fa38b586cdbc6e8804ae7a7ac381d2ee4d7aa133257342ddbd",
    "V2LaneWorkAdapter::hydrate_canonical_lane_artifacts": "15bb7473773ab07cf8f68557c3ce443defa60fac45c9697aaef3945a3b5a24cf",
    "V2LaneWorkAdapter::next_effect": "62af9ea4c3707845b5b097a27f5cc9281b8ade4bc60db49cdbc9f1c3e2b3496a",
    "V2LaneWorkAdapter::effect_count": "3be06e0c96fdc63e06952ec83b5aa900daf39912955249ca6aad64ec50e1354a",
    "V2LaneWorkAdapter::requeue_effect": "5259377bba158615135666cb3cddf88e0fbfbdb63e55a7691ba397e34195d856",
    "V2LaneWorkAdapter::drain_effects": "478982ec7c7cec9990a70993011e34e0cf79f57fb903b3c7cbabc040052b1aba",
    "V2LaneWorkAdapter::preflight_effect_insertion": "568289d7497ed37ca03cd91e1fc222bfa010109de640b5d3244672d0a9cf41b3",
    "V2LaneWorkAdapter::push_effect": "8974bca860609c853efe07e78397cb4be80e8bf1a688831bf1c28b1807293441",
    "V2LaneWorkAdapter::schedule_retransmission": "7468d25a90d61258242527880622e74ff38143c0f75c2e7bf572c9792c9f6232",
    "V2LaneWorkAdapter::schedule_retransmission_at": "5f5ccf9a1f78a69d064ed99331a452571d0379497e28cd67daac836effbecd51",
    "V2LaneWorkAdapter::prune_finalized_merge_sidecars": "b8400dca9234242c7f6b8583ffabed34eb17fecc23cf5fd81799bec5cd692af7",
    "V2LaneWorkAdapter::sidecar_effect_slots": "13a99e0350b8d59489cc591573adcc2b1f3f775308e9b225f1672649235699e6",
    "V2LaneWorkAdapter::next_sidecar_effect_selection": "614006f927ec384764396c8742a3b05b5a70907dddd8bcf39ed2db0aba4d8975",
    "V2LaneWorkAdapter::push_merge_sidecar_post": "89e2c9f8586ac17ba40acbcf14b133193ff0aade68ce717ec24c2820dce07301",
    "V2LaneWorkAdapter::push_merge_sidecar_post_or_restart": "85ca2f652d8409223ea2fd331d3c9010710301b5aafc0e2082e2b63557eed2e9",
    "V2LaneWorkAdapter::remove_acknowledged_sidecar_retry_effect": "14002f78eb6eee073c72b1c5fa547c69187392e1a2085d8bd5c1e385fbbf2efb",
    "V2LaneWorkAdapter::acknowledge_certified_merge_sidecar_chunk_admission": "60809702ee6e6fca12c654f9e93a453e9a4098be03608ccb11a6513cc5a6b5c5",
    "V2LaneWorkAdapter::push_merge_sidecar_effect": "08a2b77d14faf80060a2ca76a491df474dbbf548b3f0f4a5f95c13d14102c750",
    "retryable_sidecar_server_control_has_writable_route": "4f7ac1895057d195e13c094178010703e09198a4b6824a96b75739072c1362eb",
}

_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256 = {
    "run_inner": "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe",
    "claim_runner_lifecycle_process_generation": "b4bb67413cbfce25355d34b5342d81a96ad7674614e4134b48f7c74cd49af315",
    # Approved together with the exact-output alias after the focused
    # non-descent and handoff mutations passed against the reconciled component.
    "rollover_finalized_height_outputs": (
        "7049c460f181dbf4b32b3ad153387c0ebd79cf271347b4de39a55502883c686d"
    ),
    "require_peeked_lane_work_effect": "bb5763cb4c16586460c17c92f9578a5431c976fb83bc512e94e84646d6e5c1da",
    "lane_work_limits": "71eda678492f71e0e64577ca9829cd3bdd3bce4d6b3b67ba610dde863af249ce",
    "apply_bounded_sidecar_admissions": "27eb4ede4dd038babb38255b89f6a25259b79f55c6dcee33779efbc5d91e04ad",
    "apply_certified_merge_sidecar_chunk_admissions": "0243d1f22247947cc44ac474293a9c852c63509fd46f9357e4ce56b3fd0be518",
    "apply_certified_merge_sidecar_closed_prefixes": "4d27f99c125389f9ffd2cb85b752445882043824f7d5427edc00838ce00417f9",
    "apply_certified_merge_sidecar_closed_prefixes_with": "8a4969958909c1f7b00e17c71597c4f731dbd3eb0803dfc1a60af0d5250a158b",
    "retry_exact_output_and_apply_sidecar_admissions": "3f05df2b0b705f2adb01ccb3b21de1c2422d947b6f05fb591e316a4a27895422",
    "dispatch_lane_work_effects": "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c",
    "retain_active_owned_reply_routes": "bafe4c316b7d50e5b89bb9468dcf47271985b5f17f8277cb7c70bac5df74be87",
    "retain_active_owned_reply_routes_with_snapshot_hook": "c52a63001d4b73ccd7f06bb0527b7eb4481e29a8ea00be8beed24d841093d212",
    "dispatch_lane_work_effect": "4f26246db63b064c5b6f6389e9960f36968df9861ae9520d911eedbe4c5b317c",
}

# `asyncNodeServiceDeadlines` is a proof-only projection of this one explicit
# trusted runtime contract. These complete-item seals bind the structural
# production half of that boundary: one local serialized height loop, finite
# service batches, and finite idle waits. They intentionally do not claim that
# Rust proves host scheduling or I/O latency; those remain the trusted half.
_RUNTIME_AFTER_GST_REQUIREMENT = (
    "After GST, each non-crashing responsive validator participating in the "
    "active-height or exact historical-recovery corridor has an advancing "
    "local monotonic clock, its serialized height runner is invoked within "
    "the declared service bound after every finite wait, and its admitted "
    "local fsync, signature, reconstruction, validation, and application "
    "work terminates within declared service bounds"
)

_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256 = {
    "V2IoCertifiedServeIngressReservation": (
        "8a169187bf7b80abe0350fd1cdbffc5baebb1f476a0f6813968f712202e1d565"
    ),
    # Bound after completion-owner field/provenance mutations were rejected.
    "V2IoCompletionOwnership": (
        "7e76f629cf23bbf9e665d1a8fef17d9fbae053f41edb43f06c0fb085a034d69f"
    ),
    "CertifiedServeProducerEpisode": (
        "e1e0bdfc4854c5553d5fbf70a67a153998be353b7e7016e890af3bbe9a76a67b"
    ),
    "V2IoCommandQueueState": (
        "95fe80ab3bb8703ea55dfcac5a8b107f6dd7f35a663e4100ed5923cf2f8fa5db"
    ),
}

_EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256 = {
    # Each carrier is bound after its construction, validation, mint, and
    # consume mutations were rejected independently.
    "ExactServePredecessorCompletionEvidence": (
        "7082b85e0e2a57faf487af708330fa5a1b465ac7ca99688d0b33ffc1b643f017"
    ),
    "ExactServePredecessorEpisodeWitness": (
        "b89048c6a5cd56665d845cf7c652a8f6acbb82d17197bc2694544ec8bcd45bf3"
    ),
}

_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256 = {
    # Bound after invalid-construction and exact-validation mutations were
    # rejected independently.
    "ExactServePredecessorCompletionEvidence::try_new": (
        "828911f00faca005db61b893bec82ef145090eb7d1bd33173b053e8be04e4598"
    ),
    "ExactServePredecessorCompletionEvidence::lifecycle_ordinal": (
        "3ba70b16c7a9ecf65562435fe08dcfb3733263d1380bffe7e0b12b5302df667c"
    ),
    "ExactServePredecessorCompletionEvidence::validate_exact": (
        "b0633ba009dd400f635563661d1dc9ae31a7a6b9a708abdaaeca8742cd042c8e"
    ),
    "ExactServePredecessorEpisodeWitness::try_new": (
        "0a16fe612079276be78abafa73c67546047a1aefe40e9b1e0b203a9280f86f77"
    ),
    "ExactServePredecessorEpisodeWitness::validate_exact": (
        "6b86676a3dbfc611646eaf2dd9eaf1f7148df6f2a5cc03e16e1162e6ee6bdba7"
    ),
}

_EXACT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256 = {
    # Each provider is bound after retain/peek/abandon and exact
    # lifecycle-ordinal mutations were independently rejected.
    "V2IoCommand::runtime_lifecycle_ordinal": (
        "85b4ed4f21d28ef3ceeb92e55fc249bc7096e021856abaa3da55a7c7b11e5777"
    ),
    "V2IoAdmission::retain_completion": (
        "5531929f63a9b9abc1c8d66beac2f81324b5e53bb5fba7e58c367e414cf4fdf8"
    ),
    "V2IoAdmission::abandon_latest_completion": (
        "8b15ac13c7155d468869bc06451eda891a0e83306291e8979645a40b7b1403a4"
    ),
    "V2IoAdmission::completion_ownership_at": (
        "d5344d0c356eefd40ac512da5052531d72b1b421293656086dea462874c8732e"
    ),
    "V2IoHandle::spawn": (
        "4481d874df3ab188bcdf483a307af0e819b5a174e66ec233de57bc8e7f5039d0"
    ),
    "V2IoHandle::completion_ownership_at": (
        "3b8de8e5d3c0702bee6510648bf0bd4f36f64fc46c79fcd04528d5cd8f892035"
    ),
    "LocalCompletion::runtime_lifecycle_ordinal": (
        "f71dece7980910cbc53626ff041fd945f7f163082b943e1a429bd55e95559417"
    ),
    "send_completion_with_lifecycle_ordinal": (
        "5d91be7698dbaf645fbe0d1d83cdfe346f97259ad437e8e6e02c36fe7444378e"
    ),
    "send_tracked_completion_with_lifecycle_ordinal": (
        "77f39f7b7469a5e9fe6fc163bd4650e80d9af9b93c854db222c88c195f9dd8ea"
    ),
    "try_send_tracked_completion_with_lifecycle_ordinal": (
        "02b7d43468ac9fedc802f264c1c828a7aa4e27ec1527d8fdaee7ea2bbf042784"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256 = {
    "barrier": (
        "bce35afb422de5e3b0b7a2667bea940f9170f0ba95dfae03bbea01c36de5a9ad"
    ),
    "matches_barrier": (
        "d4ba831e00f4262e51db685213bd83bbdac6edb12ebc691b9aaa62774880a3b8"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256 = {
    "build_v2_io_command_channel": (
        "2623e948dc4a847491de8d93a1930a4657868f40dfb7ade6932007e2e6970a06"
    ),
    "CertifiedServeProducerEpisode::drop": (
        "926626c686c14d453e18bf20dd7232e33383b26b814178e2d2b7e2c2beb161e3"
    ),
    "V2IoCommandQueue::close_receiver": (
        "fefb2552e1b12444fab81480a8d9b1ba0dff351a60113207ac02ac6336020bf7"
    ),
    "V2IoCommandQueue::reserve_serve_ingress": (
        "2077c9245b85b0447c9a9a29fa8211e706a4fb1f787378c21fd2a3104ed6a11e"
    ),
    "V2IoCommandQueue::retire_selected_serve_ingress_occurrence": (
        "c0821d882ae2997fa2632ab0855afde09bf051d353a4a1bd76293b9491d0dce1"
    ),
    "V2IoCommandQueue::try_begin_producer_episode": (
        "31abb399d6e4ee100a51caa5c7213977aa99835a586d1e129057d4a9f9f2c8d0"
    ),
    "V2IoCommandQueue::suspend_materialized_serve_barrier_for_runtime_predecessor": (
        "af55e56ebf4bffef6b765e06aec105ede58a1be56f9612c310799fe67aef2ec1"
    ),
    "V2IoCommandQueue::serve_barrier": (
        "1104236dd8cfd0ccec5ea0f3171b9d23c6dc1b0f257f0ec13ff6fe8685ad7d4e"
    ),
    "V2IoCommandQueue::claim_serve_runtime_episode": (
        "776649d85722700536fff55e1af781a66aa8694492c550ba3361255d516d0c2e"
    ),
    # Bound after exact +1, same-witness stutter/conflict, and Complete-to-Ready
    # mutations were all rejected.
    "V2IoCommandQueue::observe_serve_predecessor_episode_witness": (
        "5cd4dc9cd17670e62e308cd39ee85b85029b9d3332737505678aa848ef9de8ac"
    ),
    "V2IoCommandQueue::serve_runtime_predecessor_capacity_available": (
        "203786f0b005ec7cf105e7189d4f05a23efe71d241c1cb16b3a700c2d4036733"
    ),
    "V2IoCommandQueue::finish_serve_runtime_episode_turn": (
        "02d2bb266994d99ca4fdef5ec8d90fef193a969fe2136eb6e0f0a7f0ae47f0df"
    ),
    "V2IoCommandQueue::try_send_as": (
        "6c7a0afdfa074b745803704e6910671ab28099c9bc294c21c5fe67a55fd3905d"
    ),
    "ProductionV2Services::certified_serve_barrier": (
        "2daa1fafb0049a95cb78d01630aa55932539b3de9a0db99eb169c0e5e031f56d"
    ),
    "ProductionV2Services::claim_certified_serve_runtime_episode": (
        "3b2fce586ae8d9b59bba1bd4f196ae8e9f5b6fcf3d65ae21d03c541480aef5db"
    ),
    # Bound after strict-cut, capacity, least-owner, and non-consuming
    # projection mutations were all rejected.
    "ProductionV2Services::certified_serve_predecessor_completion_evidence": (
        "d52c64cf0e0cfd12bce5b23e0b70242dad19f209fc5f62dff9df85691d178951"
    ),
    # Bound after the production forwarding mutation was rejected.
    "ProductionV2Services::observe_certified_serve_predecessor_episode_witness": (
        "77d06ede9f9ee5427ae010eb5256830de9aaa1acfc899cfe4409fa9bd9dca9be"
    ),
    "ProductionV2Services::certified_serve_runtime_predecessor_capacity_available": (
        "b995b96761597dfa1f580742c4898a1ee4a365a627cbaf98203435c33f252ed2"
    ),
    "ProductionV2Services::finish_certified_serve_runtime_episode_turn": (
        "2467e4a47772c21034f08851f53fcf55cd34bc642bfb8f4f92b519cafe2524bd"
    ),
    "ProductionV2Services::try_begin_certified_serve_producer_episode": (
        "b86f953644efbd6a73d81bba2a3cbc23195cc9dcb68e44bc90e6b9a4a8cfd15f"
    ),
    "ProductionV2Services::take_exact_serve_predecessor_completion": (
        "c033089df5ad252d912aec754b9535d16f5a96d99c718a47239116c5916dad88"
    ),
    "ProductionV2Services::take_lifecycle_prefix_completion": (
        "4e8e73973d093f5b642996a26b0995247a38b2c13726567d2b719fbae232d1b6"
    ),
    "ProductionV2Services::drain_exact_serve_runtime_predecessor": (
        "aa2d628cbb2b5f54d0901f9949d3581d6188efe72a3656958e95e9e9dacbae92"
    ),
    "ProductionV2Services::drain_completions_inner": (
        "64eff7f2bf4a68f5167819b7e0ef79a95f2c0d0aa374762ed41df27d95a04f45"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256 = {
    "advance_executor_once_before_exact_serve": (
        "065cad68c50e4271298c681772d42e7d823bdd253ee2dc848597194f8490ca26"
    ),
    # Bound after the observe-before-claim ordering mutation was rejected.
    "run_inner": "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe",
}

_EXACT_SERVE_RUNTIME_EPISODE_RESTORE_ITEM_SHA256 = {
    # Bound after restart replay could not synthesize a consumed witness.
    "restore_certified_serve_tombstones": (
        "4c818b6e4e825ae655981aa1b04b886cfe27dfaf799422a42567c8420c3377d9"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256 = {
    "publish_external_lifecycle_owners": (
        "0b872fdf4ad76a4b092cadc6cd33d041463153adbe0d67724b7919767e7f46ab"
    ),
    # Bound after alternating retained-response/selected-Serve target-state
    # mutations were rejected.
    "older_runtime_lifecycle_predates_retained_response": (
        "596d7ac14ed46a77c83a8d1697ba45c4c6424563bdbe037de96c63eed99f5ec9"
    ),
    # Bound after publication/delegation mutations were rejected.
    "exact_serve_predecessor_episode_witness": (
        "42439ba19db76d9d46589b9a51ad27a5fb31689aa3b29e830fdd39f96e6f381d"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256 = {
    # Bound after every isolated target-state initialization mutation was
    # rejected through all changed-item aliases.
    "with_driver_and_lifecycle_ordinals": (
        "ba31ffeba2ffc96269ae8d531d5d5a6a01fc80a5af50346e672c54e9fea9bc53"
    ),
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "61dfb5c5bed49fe8191be06bd09a2e929f70da89b1ff829add86f8966fd90f01"
    ),
    # Bound after completion-evidence integrity, minted-source, and
    # least-runnable-owner mutations were rejected independently.
    "minimum_runnable_lifecycle_ordinal": (
        "18d848ad2bd2263010994b2649313c9e3d9d1db9b2117a9c772120478bb65fd3"
    ),
    "active_lifecycle_uses_ordinal": (
        "ae450e3b7d749e88ebb10ea93d575ffd88257f1c691bc58d9478adde058cd869"
    ),
    "older_lifecycle_predates_exact_serve": (
        "960b126dc222020336f085ec86a1518295ee2c20f0953c69098c68f325555791"
    ),
    # Bound after alternating target-state mutations were rejected.
    "older_lifecycle_predates_retained_response": (
        "2a93432530d127c39a34cbf2abc8349809173a5af546f8aa23cad0da081efed1"
    ),
    # Bound after absence-to-presence mint and retry-latch mutations were rejected.
    "exact_serve_predecessor_episode_witness": (
        "4e1cf313800bf991188bbdc6097e775e9eafb4c9395d124b640642f674657a8d"
    ),
    # Bound after both isolated target retry latches were mutated.
    "step": "bafd283fd50fe929e000481a8314f98cd0ad3aef30c8e8677a93b0784045136c",
}

_EXACT_SERVE_RUNTIME_EPISODE_INGRESS_ITEM_SHA256 = {
    "oldest_active_lifecycle_ordinal": (
        "a9dcc40ab11d2af33c91c5449a24bd524289d8a00e89ea2cdfafe99b27ed2a86"
    ),
    "uses_lifecycle_ordinal": (
        "248f70bfe769734211acc566c8b91b7e0faf037990be47f95b231d5f102a557a"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256 = {
    "final_serve_retirement_yields_one_producer_episode_before_replenishment": (
        "059218b1bec2859b84f8fe440c785360072fca157e1ea7a4579bff252cd6dde0"
    ),
    # Bound after non-consuming completion-evidence projection mutations were rejected.
    "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io": (
        "59a1d839e4970b6257be61f5e6c91c7964920697aa1f9cef132e5e81663030c1"
    ),
    "repeated_exact_serve_claims_close_all_older_sources_before_later_io": (
        "e97bed021406e0a72e692d2ac6ed58e491a4826b5a9054f17ca8a64d95ef0968"
    ),
    "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission": (
        "ebeeb9606074ce7e7d51aea4d55d54dabacd23435d639d81ac6f0a9849616e21"
    ),
    "worker_completion_is_retained_behind_a_full_runtime_fifo": (
        "4e807f3dd45e855a96923c803bd970dbc2a6684583b8dfa84aa6db788c5206c1"
    ),
    "production_drain_publishes_worker_completion_behind_full_runtime_fifo": (
        "913796745fa6c589f20dd309da632c1dfb05f84aa963377b04ffa9a0556e21eb"
    ),
    "drained_exact_retransmission_gets_fresh_scheduler_ordinal": (
        "8c4b30a4d0c7d2730d6d2e4c702efb2cc48e514a64513d6b84751f8c91654811"
    ),
    "certified_serve_future_slot_blocks_control_and_consensus_replenishment": (
        "4eb2da42d968642c6eaf184f0ddda6f726e4a7fe49d6b6c2499092ee3c97d075"
    ),
    # Bound after exact-witness consumer mutations were rejected.
    "completed_exact_serve_episode_reopens_once_for_new_runtime_witness": (
        "3136dd2fbf27c196a4e7d279cdd36fbe12e376f71a8da12640f8985c70a6a3ec"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256 = {
    # Bound after the passive-Fetch late-runnable mutation set was rejected.
    "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps": (
        "fec30fc71608a1656b86529ea2fb19fa81478ff23a67d5e44dcc33ae02d70f0f"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256 = {
    "restart_dormant_local_fifo_reservation_survives_full_class_churn": (
        "4173b41c9622f676b9c9d412a267cb0b3b2aca91ed9725e2a5c450485d7442d9"
    ),
    # Bound after alternating selected-Serve/retained-response target mutations
    # proved that the two process-local target states remain isolated.
    "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt": (
        "a5cb9f948cfc8411c6b8b38a87c9e6889f627f62b31f4a514e0540d3a8863cd0"
    ),
}

_LEADER_WIRE_PHYSICAL_INGRESS_REGRESSION_TEST_SHA256 = {
    "restored_productive_retry_freezes_the_current_physical_source_prefix": (
        "0717cee735d0b0a435bdcd502ca6e29c87a554a9f017cb7edbf1efb46ef33d20"
    ),
    "restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire": (
        "e994c3391ce2626972c26c57a37039f71573adb9d82bbcfa64619edb70640255"
    ),
}

_LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256 = {
    "fair_v2_ingress_admit_leader_wire": (
        "bda06e6c542397e50d18cde8fd85fb568e619de471719967a594cfbdfc850350"
    ),
    "try_recv_if_at_checked": (
        "ab9e90e132fbd369c255a7982b5ceab98e74ed3b535cc494b0b12a57a53bf81f"
    ),
    "try_recv_if_at_checked_classified": (
        "234399223cc9b36bd4c6f3be6dd29040fd0761b67b4a1c38f4fcdd40bf79ac19"
    ),
    "fair_v2_ingress_leader_wire_selector_projection": (
        "34eebea6c6b1a3aefeb68010a2ea367970cc3f16a7d5adf592e184bcc799201e"
    ),
    "fair_v2_ingress_queue_gate_verdict": (
        "c867fbfccf0d45fff2757bfbec97655de382b225ce9adada9f451e01c3e38e8d"
    ),
    "ingress_scheduler_ordinals": (
        "994beede48b0f3f8b0418f2eac37029ca5f65fc934aa4206e9dfc69d1a2acefe"
    ),
    "bind_leader_wire_lifecycle_gate": (
        "d6d8898d1a10684cc1f1aea7c91aa84dfb2f90862378d41d0f0908d57e2dfcc2"
    ),
}

_PRODUCTION_LOCAL_RUNNER_SERVICE_ITEM_SHA256 = {
    "run_inner": "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe",
    "service_certified_serve_barrier_liveness_turn": (
        "d6afb60b54ec1b0a482ac76cdcc4f14469dbd39930a21ae26904c1a2d4cacb88"
    ),
    "advance_executor": "ce2f1975fd47aac0b33326547595f05779ab0a920e5996499bf450330c457f93",
    "advance_pending_tip_recovery_executor": (
        "a85c018053d4b47dd1c36194a66318422f72eb80e3cca3ac2ba9db5f44eeb9dd"
    ),
    "outer_ingress_turns": (
        "7b77924cf0d587238d52bb45398db1f4b39806e71606f1014be4b225bda7e377"
    ),
    "apply_bounded_sidecar_admissions": (
        "27eb4ede4dd038babb38255b89f6a25259b79f55c6dcee33779efbc5d91e04ad"
    ),
    "dispatch_lane_work_effects": (
        "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c"
    ),
    "drain_lane_relay_ingress": (
        "665e0ea1c01501d80a547ec3d4ddd72117d32f7ea748de4ee2d0803519afbfb6"
    ),
    "ProductionV2Services::drain_completions": (
        "dbdb63d50e19b3dfe3617aaedf53e7d7f13c105c4b844c5367d31487fad10ea3"
    ),
}

# Exact comment/literal-free source seals for the production-shaped selected
# Serve timeout-recovery regression. Each seal was approved independently after
# its corresponding digest-refreshed semantic mutation was rejected.
_PRODUCTION_SELECTED_SERVE_LIVENESS_REGRESSION_ITEM_SHA256 = {
    "runner::CertifiedServeBarrierLivenessAction": (
        "4a520a5e830e2eb78e6384de5e353c50d488698ab1f3ee622bcb97a7b8dc2d5b"
    ),
    # Bound after the composed late-passive-Fetch invocation was removed under
    # a digest refresh and rejected semantically.
    "runner::complete_certified_serve_episode_cannot_veto_pacemaker": (
        "0b221d9b897271b92cfb7c37cdf9f7cfe14699c4b372da7f122360de6c406e68"
    ),
    # Each fixture carrier is bound after exact-inventory and field-removal
    # mutations were rejected.
    "worker::SelectedServeTimeoutRecoveryMode": (
        "4d7d121b2669db449618073d79f8c0d43a8195a584d958e2e847bba657c76c9c"
    ),
    "worker::SelectedServeLatePassiveFetch": (
        "291c5357cc35ec7571fe2d166a9b6b1b4ea5f27fd509ebc1f1c5ac10417804b5"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture": (
        "6bda84eb41716036a6938b9f727bf54bcba2f5ca3918146eadbb0cee9f859fc5"
    ),
    # Bound after both exact constructor delegates and the shared mode
    # constructor survived their individual semantic mutations.
    "worker::SelectedServeTimeoutRecoveryFixture::new": (
        "793a3708eb7fdce3d109a60528acf0be0bc5771bc52027c2dbfcfe44414081d8"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::new_late_passive_fetch": (
        "d23e6ab382f4c478ed392c4cdf9dafff92db47f6817fd0713075671442d2fca0"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode": (
        "0ff17b16d8a026e7547b0d21fb4180f7716ef60067b80534e1f54d7be66e5058"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix": (
        "9c456e9353ff8ef97a6ae0d329fb77fa90bdb6980dbda330984f61323c44365b"
    ),
    # Bound after the complete BodyAvailable -> Store -> Validate rejection ->
    # Serve -> producer handoff mutation set was rejected.
    "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve": (
        "b52fdd8a6acfd5951e9aa514dce783984a2edf6a752f638c91c31d0ce05b0b40"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::service_timeout_vote_episode": (
        "aa266e22920cc2b8b5ce8ea0095b519da73c93827d9d916e075df8002ddeca50"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::service_timeout_recovery_prefix": (
        "26f8db46c991874a993888431415631adee676513c41ba1671176f8e7b91bd9a"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::service_pacemaker": (
        "38dbb93aa1cb6f4326767382a354244d21e07a97c78119472da994558d96f65c"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::entered_view_one": (
        "dff5113863756090795cfd2ff950a3bfd390e5120260d9ef7986b8a95772d93c"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::assert_complete": (
        "6f322ae1a6658bfa8b5275ed3c7220a8e9a0dbffcd86851340e00358005fdda9"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::assert_missing_proposal_serve_selected": (
        "72b5987aae636d8c8be84bcc96617451c63fc3d0b2765a189737a7bb04365c7a"
    ),
    "worker::SelectedServeTimeoutRecoveryFixture::drop": (
        "ce14c1b4e75b8ba5c1b225653eac74cfa39a0432dfd7a84592791342f6cb9881"
    ),
}

_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256 = {
    "PendingExactFanout::classified_with_route_history": "a1f9ad9102ffddade0c83bac4895a781a72ba7bec980d99e914eb0641dafc9e7",
    "PendingExactFanout::classified_with_reply_routes": "84c22ccbeecfb531f69c7a03b6f659281fb69fde25f013e0aa394afba347914a",
    "PendingExactFanout::retain_active_unowned_reply_targets": "d34c8c2d4f0f4e402bae740f6463445a30453b88761fcb125cc5ac506479819e",
    "PendingExactFanout::reply_target_merge_plan": "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653",
    "PendingExactFanout::reply_target_merge_plan_with_hooks": "7fd055d3df0b09abc838ebcef9d21f9dc98020bcdd162f1ffc4a7f4fda697ef0",
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::retryable_certified_sidecar_responder_control_target": "6ec5aac36548c23c407f8212aa93e9794a1383f4ce20c517ac6c158f24193cab",
    "DurableExactOutputServiceOwner::is_sealed": "e739a883ce93dc761dcb9ce0673f69e2e280562ab67f3ca94d1a454efa0860b6",
    "DurableExactOutputServiceOwner::seal": "abe0ec8f7915faab4687fa7be6606ab69e8b58f650cf4ba62bf8610eb586e652",
    "DurableExactOutputHandoffReceipt::is_bound_to_transport_owner": "3a991fb6c233282aab23f9e1cf30857457983736bbd15c7da0f25423eb76a2b6",
    "DurableExactOutputHandoffReceipt::matches_predecessor_context": "7e6c407fea5b7393d085f3ae32a99832b5b9cf8906e64c5a168bacdd46ca1dcb",
    "DurableExactOutputHandoffReceipt::matches_finality_artifact": "2e0593d1f00ebd5f7fd0717ad6123cbb4c37a4c27e45b2b514e5dfbb5d37d4cc",
    "DurableExactOutputHandoffReceipt::authorizes_immediate_successor": "0d432476a9154613c9bcb9285a7d69a0dd8264e818dc993a12e77903d8d3e069",
    "PendingExactOutput::new": "4d6f8872fd25a2d6041dfaab7d2182d3b43d13fcae408b478fab17f34afc738b",
    "PendingExactOutput::is_pending": "6cd7ecb71f163b7b59e59abcce7f413a4eef60bfbd160950c4afd03a0ff73588",
    "PendingExactOutput::close_certified_sidecar_prefix": "d7e50f6894a043a303ce239e0f37dfbd7a12a7c9a621f0e48f6754a72739c3af",
    "PendingExactOutput::pending_sidecar_flushes": "14c74fcdfe37c137fe20897ab19acbac746b9d3a4f764a939abbbe1bc43b2048",
    "PendingExactOutput::sidecar_control_units": "e5eeb08ba9065c86d9b13abbf45b991864d538339f09b2dfe42e5afa0b84ef68",
    "PendingExactOutput::restore_pending_flush": "ff20f9c93c8cbab55fdeef391e73223cba5bfbc0bf4fac2b9c8130b984ac0d7e",
    "PendingExactOutput::poll_reply_flushes": "eae8ee4dc4996b077b9d0e3315e96e8c35a18b0189f2add40e898e60a4167749",
    "PendingExactOutput::validate_owned_reply_transfer": "c39a07ac424ad25dc1d2d1d5cffec3daacbbd7a83c029ff289739745acb6f591",
    "PendingExactOutput::can_enqueue_owned_reply_transfer": "8fcf9c1dc5edb24b0001104718bf80ff142716e45e36b179f74a430e2b214c38",
    "PendingExactOutput::enqueue": "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6",
    "PendingExactOutput::enqueue_owned_reply_transfer": "285ad0dcddadc2a95c155a520f636d14932f6c9c2913e7eea7e43cf443e6642b",
    "PendingExactOutput::project_sidecar_receipt_completions": "e506155b90096fb5980cb878611a5c42190591efebcd7d10d3961061cf925158",
    "PendingExactOutput::retains_retryable_sidecar_responder_control_for": "8f2960dccb011be19e3cb0135ba26138f4020dbc91333bbcd1a73cdcae8d17c1",
    "PendingExactOutput::enqueue_validated": "035f6caf6f60eb2fa8c6a2da471fd60d6f4692061815906378aa115ad29cb92b",
    "PendingExactOutput::handoff_applied_height_to_durable_reconstruction": "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80",
    "PendingExactOutput::drive_with_budget_ack": "675d0e99a923f9b8cc96d725458584f4ea762cd4fd69fd9f8e7e76b158c82d52",
    "PendingExactOutput::drive_bounded_with_ack": "7e630c6be2466fe0980df81ea194e101b70e4abac9546a379ab9f24c8eb23861",
    "PendingExactOutput::park_unwritable_reply_target": "bae33ea7bcf13a905da400e913bed5bf2347f103d52be229e0d57fc6f24376d2",
    "ProductionV2Services::admit_network_exact_output": "8652c4a435eeba2522055d198d1bc997befa05615097688986be0d4cb0d1f460",
    "ProductionV2Services::drive_pending_exact_output": "ff02d49d849445526f4f4571a0544f96ad113b257a49ba682a39c6f03e88b950",
    "ProductionV2Services::enqueue_owned_exact_reply_routes_while_guarded": "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff",
    "ProductionV2Services::retry_pending_exact_output": "a826dc12f81db5e245b5a3eb1d94d89f43ecdf61b2e49f594d4f0c696594b31a",
    "ProductionV2Services::has_pending_exact_output": "107becdd3b504739250f12867eecdb459362f907957797ea3a0e31a71e360768",
    "ProductionV2Services::drain_certified_merge_sidecar_chunk_admissions": "d065d7b2b625852ed7ecd8458997c763b8c6d4d1ef445275150c16a62f1badf6",
    "ProductionV2Services::close_certified_merge_sidecar_prefix": "fb6879c8c325aedc7c19f093a9e5b2cb1b8c25c7bd89c7da1f2694e923cde3a2",
    "ProductionV2Services::can_retain_lane_work_effect": "a934ecbe56eb0118a5cabbce17c1815fc7346f611e4c83e36c85c918840fde4e",
    "ProductionV2Services::handoff_applied_height_output_to_durable_reconstruction": "7444ffcb40c52af925865f03520de3eac64f59d1c31ae6e549c5bf92e17aaf84",
    "ProductionV2Services::seal_applied_height_output_handoff": "293b5cf2fc90356761639f3be19586dc6e0ded95b1f6ab69a1c34dd89fea77ac",
    "ProductionV2Services::validate_applied_height_output_handoff_authority": "7e122f2997f6fa75a67033addc960c72e2dad0dd766588c2d667e056b9363bc3",
    "ProductionV2Services::finish_height": "e59001dccea8575629f971e2f0fa6a7af0f2f4f13e97c7260a52a4d4de5a60bb",
}

# Exact token seals for the frozen validator ownership-unit geometry and
# independent authenticated-source attempts. Qualified keys are used where
# common method names occur in several worker impls. The semantic checks below
# additionally bind atomic per-source route merging, non-regressing cursors,
# roster x class reserves, and shared-unit accounting.
_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256 = {
    "PendingExactTarget::apply_reply_route_update": (
        "e75cca425a7afcbd98a0eedf225f6586dd333a06afbb38366e0660b591a84fee"
    ),
    "PendingExactFanout::classified_with_routes": (
        "c848ccf3f2e54014325dc8b27794a5f9771a7df45f6ed9aa16ccc03451145d18"
    ),
    "PendingExactFanout::target_source_at": (
        "1214b0b8088d93df32646db104442dd46d56dafd53b4cceb14afedff54629cd9"
    ),
    "PendingExactFanout::outstanding_sources": (
        "0acccc51a9bedd5fec75171eada06cfa27036daa8b29d74dfebe9ee8a65f0078"
    ),
    "PendingExactFanout::outstanding_sources_excluding": (
        "5ed556d54c561a57c50d7636f84fdefaf29e14d1179de7edc245447ca861f1be"
    ),
    "PendingExactFanout::outstanding_reservation_counts": (
        "02972f0bfa1e31b0dca617382be3254a8da07177396c69101e068cb02075b3fc"
    ),
    "PendingExactFanout::target_reservation": (
        "94273ddf470c326dea9ab618150c7611ebf353a5f2358339bd52b3e382335bcb"
    ),
    "PendingExactFanout::certified_sidecar_topology_progress_target": (
        "fcb909658b3a0e546a8e6e5379ca4437ce4e55c2262f88e7ebb59ab7d8ff428b"
    ),
    "PendingExactFanout::admission_reservation_counts": (
        "3b5e7800f133b7673650fee75acefd31e56316c2d33ea5405c2b898fd11a8659"
    ),
    "PendingExactFanout::reply_target_merge_plan": (
        "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653"
    ),
    "PendingExactFanout::reply_target_merge_plan_with_hooks": (
        "7fd055d3df0b09abc838ebcef9d21f9dc98020bcdd162f1ffc4a7f4fda697ef0"
    ),
    "PendingExactFanout::coalesce_reservation_additions_for_plan": (
        "081e95192953390b7b1b17794f956c9e089d7986bda1c55ce49ddd83ad4f0979"
    ),
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::can_coalesce_retry": (
        "b5f48359f0342b142f04b6fee8c6e74e7cbeaf5068f1dc628fffe7ad5a971de5"
    ),
    "PendingExactFanout::has_writable_reply_target": (
        "a717b59e74c10dda4248c99527c781463eabb0c8ac355f2e718f5b3f3e0f0b9a"
    ),
    "PendingExactFanout::is_stranded_retryable_certified_sidecar_responder_control": (
        "ce27592a9a7e240a207c1b0ea26340edb7e7f25d11f15bdb7ef3d0698446ef49"
    ),
    "PendingExactOutput::new": (
        "4d6f8872fd25a2d6041dfaab7d2182d3b43d13fcae408b478fab17f34afc738b"
    ),
    "PendingExactOutput::ownership_addition_load": (
        "c566f3dc97560d01457335a290f876f96f5128236bf8cdbbeda6c0c6d14e50ef"
    ),
    "PendingExactOutput::ownership_capacity_available": (
        "078ce17f80c831c8018269a75c2955852a9088e4a150eac969bbab05fc0e2119"
    ),
    "PendingExactOutput::source_fifo_owners_after_fanout_replacement": (
        "2826119bad851abb20d3e7f838ba7df428b9015ec26a448ab0c5536fd3c0fc3a"
    ),
    "PendingExactOutput::ownership_state_after_additions": (
        "a22e2ff976a86ec6218c8a417b2f9b3fd066d16bb9b8edd561fcc00512008ad2"
    ),
    "PendingExactOutput::ownership_state_after_replacement": (
        "80d59ecc4e9300743cf22d82a07712f9c585cbda0cbaf49197ff89bfefc66932"
    ),
    "PendingExactOutput::coalesced_target_geometry_available": (
        "16637dff951694ae9999357a0121a18e520dda07935fb88839625d080e47f403"
    ),
    "PendingExactOutput::remove_ownership_units": (
        "028bce9d7aa7c84ad347212ecfdb1b76066ac80fa9e713219a89e590ef0715bc"
    ),
    "PendingExactOutput::validate_fanout_bounds": (
        "1b40b3085feb03a957e28a2bb7776ce874cd571f0206148aa00126a7dd57bea6"
    ),
    "PendingExactOutput::capacity_available_for": (
        "de94c2648c59bc1e9a4b0ec3c5f4824ad7bdedf153012603c6ddf407c42326e0"
    ),
    "PendingExactOutput::can_enqueue": (
        "21e501bc34a7c1b41ca787e5a8c4a48d9e853a8cf7b3960c685ef58e7d33cc0a"
    ),
    "PendingExactOutput::stranded_responder_control_replacement_index": (
        "5a93e8d4bcf188928987ad45e193a7569f25c68f05ca9d7f25a93ba3e96974df"
    ),
    "PendingExactOutput::responder_control_replacement_ownership": (
        "3b28dfe491d895c31600f4c6f871fb04015f7a9214dbb6768431c8484bf6dac6"
    ),
    "PendingExactOutput::responder_control_replacement_available": (
        "1680843d65a96ef27f9c5d68c968dbbea10998b7641194647adede693c3fb4c7"
    ),
    "PendingExactOutput::responder_control_replacement_plan": (
        "1a914ee0f02c0e45e7297282a6a9d415761d610ef303665bd98951ef6136fb79"
    ),
    "PendingExactOutput::replace_stranded_responder_control": (
        "5b87149fac9b75e1690787b5167ed04681676a186f0bd2bfb834ecc5a3c748ff"
    ),
    "PendingExactOutput::enqueue": (
        "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6"
    ),
    "ProductionV2Services::start": (
        "2de2f39b1e46685d96f308d7d5e1a23972c251a67cfdd52b0b1a5dcc792f7c64"
    ),
    "ProductionV2Services::exact_target_geometry": (
        "978520459f9dd3c5459478e222418ffed2924445c40a79722c307f97e6d28871"
    ),
    "ProductionV2Services::can_retain_lane_work_effect": (
        "a934ecbe56eb0118a5cabbce17c1815fc7346f611e4c83e36c85c918840fde4e"
    ),
}

_PRODUCTION_DURABLE_HISTORY_WORKER_ITEM_SHA256 = {
    "durable_history_source_covers": (
        "c903d837cc5eec02804ef8913532775b2c4d996c6299cf58cbdef29c4dd4fe16"
    ),
}

# Typed semantic claims are the production authority which lets exact output
# cross the applied-height boundary.  Seal both the claim validators and every
# production constructor so an untyped producer cannot hide behind the
# scheduler/retirement digests above.
_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256 = {
    "accepts_superseded_reply_delivery": "b96c62bc732dbe826dcbdd990524bcb43c25e197162e126efeeab235179f476f",
    "covers": "48529c793eedab83283aa2d471486e8bbd77ce399c0be52576bc23ee3b8b540c",
    "from_request": (
        "790314790a852bac49e79b6c71bbe07931ade269d273c536978b45f4822a4b68"
    ),
    "from_chunk": (
        "73871d46bfc5ff28b4db03bef64811ee26e995795dca98c552f5dc8b1e4067f9"
    ),
    "native_amx_message_body": (
        "90737e4116833fb086b7c1f3a7a04dbb34fc9157385103f3dc1be6e74c127eae"
    ),
    "scope": "0dcaff94bd70e6828d83d7fdf5839ee64649022195fd032bde24f67c514b5942",
    "validate_fanout": (
        "e0260ce34cc0e15e9bcff1c3ef89b8cab30cb70449b0574caaf3e85893dab9c6"
    ),
    "validate_non_retireable_lane_transport_fanout": (
        "6f57bc6fc5d7b127d67745f5d042e780aac18e9abce9f4d53c90a1c4ebe934d0"
    ),
    "claimed": "75dcecc8adae80ad5980fb812e4d13af3e7f432605c685985a6c2d0e27a67a10",
    "claimed_with_routes": (
        "b4e6d00e981ae28935489541d49c181941a4a38c1384a823ffc7d8ace27e9418"
    ),
    "enqueue_exact_fanout_while_guarded": (
        "8f222f8dc1b6421990600f5a59d489fef0452793c2c9e4468296c092629d1d2d"
    ),
    "enqueue_owned_exact_reply_routes_while_guarded": (
        "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff"
    ),
    "drive_pending_exact_output": (
        "ff02d49d849445526f4f4571a0544f96ad113b257a49ba682a39c6f03e88b950"
    ),
    "exact_output_scope": (
        "2c322931cf99b7f7e6484c11c48b4bb570b48bc6a95f240e09fab88eea599be0"
    ),
    "post_to_peer_on_reply_routes": (
        "326e01fb46a4f99e7bd8c3b09f3216252b74ee7448003a2640dec578e4a8c08f"
    ),
    "post_durable_history_response_on_reply_routes_with_permit": (
        "c2f9ac9b07a52814d6ba43a0701ecd6a04cb6ca7cd890efb100330b958c0d651"
    ),
    "post_durable_history_response_with_routes": (
        "c6b6ad7759ad801016087dcd0d172c40685e6ee382d3de51efc5e0557b93f638"
    ),
    "post_lane_block": (
        "1a4be6518deb1d756893951563f23f8017bd840779366054ce9b5a3329531389"
    ),
    "post_durable_lane_certificate_on_reply_routes": (
        "84b9fa1c26b45f636ad7fd2b605b3cb4398a74abb3f515ea9e05034ca7a9e429"
    ),
    "post_durable_lane_certificate_with_routes": (
        "d4463b2b414c7e4c4febf871d63ccabca15b060f3345b820b6f7da9c2f3deb79"
    ),
    "post_certified_merge_sidecar_with_reply_routes": (
        "333334838aaf7761315f2eabb1f34e6cead127ddfd4f1e6282ae116e5c9846bc"
    ),
    "post_native_amx_with_reply_routes": (
        "17d66596b19902ce6ac3da41c7c05a000b42a54a87320f92cb588500f151c396"
    ),
    "broadcast_merge_to_voters": (
        "99b0c80af2876f9b92cd9789605a7040d3e6dec0b4ab11f47edf292aeadf5f59"
    ),
    "post_block_message_while_guarded": (
        "5556a35937aa384b399482ceb30f45cfdbe212c59a10bf10aa9fc6f46e17f670"
    ),
    "post_block_message_on_reply_routes_while_guarded": (
        "c28e740e75f29dc743dc81eda0b34b70e46040ca6a44355b8092cb15a98ba9b2"
    ),
    "broadcast_preencoded_to_voters_while_guarded": (
        "1c22c254ba300a86887affe725dd01e126563ac584f791e9da329fe67296bf4f"
    ),
    "broadcast_to_voters_while_guarded": (
        "bbc43f18d7ee7cc95ecfabc70894ae1b82780edab21ebecb8a60445e68113a4d"
    ),
}

_EXACT_OUTPUT_ROLLOVER_CLAIM_IMPL_ITEMS = frozenset(
    (
        "accepts_superseded_reply_delivery", "scope",
        "validate_fanout",
        "validate_non_retireable_lane_transport_fanout",
    )
)

# The lane authority is the only production witness allowed to supersede or
# reconstruct lane-local output at global-height retirement.  Bind its exact
# Kura/application source commitment and its winning/non-winning validators.
_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256 = {
    "durable_historical_lane_output_source_hash": "9d2cee85c5c027238fcc905c74831ec27aecf59b431f3559b0ebe49b6e21a280",
    "durable_historical_lane_verification_pops": "090336f96d80a1ea90e51a2ea2cf4161aca46057c6e11c87cefeeb24c18b36ee",
    "covered_source_hash": (
        "0958a0a7b042dbbd5a86f53caf8aee9aa4e780ee063e22b46b6f31c73a02823a"
    ),
    "persistent": (
        "3e865b8a1b9b7136ab2cf81dd6d33fe84ae9e9b14eab2006156ae9cbc29520ff"
    ),
    "lane_output_identity": (
        "bf17d20ee94a5023ce4623b31eb333e8d7cb12c1a155d058a2a9f22840430b58"
    ),
    "validate_winning_lane_output": (
        "d6a5cc077d2e4bcec65b52451771fcfac4f29d8ac82a8b1af0fcfb17daae3114"
    ),
    "validate_winning_lane_qc": (
        "345e7f2f2faadd6dc57195390ce8e711549608ba56f1cb01f962250414f4c011"
    ),
    "validate_superseded_lane_output": (
        "ec480ca71d859f5b27fcc31731a1bc2ebb02ee8d5002475d53d3ed14109c94c2"
    ),
    "durable_lane_rollover_authority": (
        "9de896c617ad4809199d5b8fc8215ed860a35a96ab028cb2a95eae7f71aa0e8a"
    ),
    "serve_durable_lane_certificate": (
        "bcab2428b4a2d43dd23989bebe917077e84b069c4e120808f6b25ce4503ce52f"
    ),
    "reconstruct_durable_lane_certificate": (
        "adf988938c94e9869ec4f26c9942cea89fb0cc0669677688640f9c7567dc2a59"
    ),
    "reply_routes_are_live_for_peer": (
        "cdca18bef9df99c77e3698622c9cf6941dd249967bb587f60e9fd381a4f8b235"
    ),
    "lane_work_effect_reply_routes_have_valid_shape": (
        "5140d1973a6992f34c59d93739d2fe62cd3e6115998963a6fdd3a46d142ee83f"
    ),
    "lane_work_effect_reply_routes_are_valid": (
        "893db592b49209bff9a7ea5ee430acc76390ade30a1a765ca7ae19b1b5806a31"
    ),
    "merge_optional_reply_routes": (
        "39e76e7cfe0d234d3508852537e442bb6511a51f372cbfded1bf6bd0ff853f2d"
    ),
    "optional_reply_routes_retain_candidate": (
        "5c43c5b723d77fdd98b194a0e86ab37c25c3a165c4aa96c032654238863bca17"
    ),
    "merge_lane_work_effect_reply_routes": (
        "0d32e263ed0d2659f4274b03a1f8bb87f8de386beb9afb8103eaf3342c6b4459"
    ),
    "merge_lane_work_effect_reply_routes_after_route_merge": (
        "0d0e5c73d9a47d0141fd1038cd81805a4d6e0aba1655e0caf620afc0db2fa616"
    ),
    "lane_work_effect_key": (
        "953b1b7b5464d9a574c4ea9d3b15cde884320bde2f49efe57f8fc4fba63438de"
    ),
}

_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256 = {
    # drain_v2_ingress was approved separately after its three-mode drain
    # mutations passed.
    "run_inner": "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe",
    "drain_v2_ingress": (
        "9a218b6c25e62bb63fe0ced59d5a0a4ab65576d8dd692485068da8e02541e704"
    ),
    # Bound after the exact-output handoff mutation survived refreshing this
    # helper's own token digest.
    "rollover_finalized_height_outputs": (
        "7049c460f181dbf4b32b3ad153387c0ebd79cf271347b4de39a55502883c686d"
    ),
    "dispatch_lane_work_effects": (
        "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c"
    ),
    "dispatch_lane_work_effect": (
        "4f26246db63b064c5b6f6389e9960f36968df9861ae9520d911eedbe4c5b317c"
    ),
}

# Exact joint ownership of the two per-height fair-ingress gates. Each seal was
# bound after a focused mutation refreshed that exact item digest and the
# semantic close/validate/atomic-unbind contract still rejected the change.
_PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256 = {
    "runner::LeaderWireIngressBinding::bind": (
        "4c9551aa9af0eea82b0bcf1f248c958d3e52c13f195bdde2f22f2a530d6b669e"
    ),
    "runner::LeaderWireIngressBinding::retire": (
        "7e6730493a8b093e2ac6f125942073158fbc539660c52f97900e12c9ff5012f0"
    ),
    "runner::LeaderWireIngressBinding::drop": (
        "88ddf5ff64ea693fbb027a716505a017236eb44e435eced813a6e4f6fc9bba61"
    ),
    "runner::HeightIngressBindings::new": (
        "48c192241bdc3ea37cf5f12f3322d5d2b52c4a48a52a1cee0b85335da25493a3"
    ),
    "runner::HeightIngressBindings::retire": (
        "50d7b09d0862c3e5bdaca6bdf829c9d851b766f492bf492fd947b9d534e9f94f"
    ),
    "runner::HeightIngressBindings::drop": (
        "ba86ff1b630d46218acf45e32823871f81a47c82ac31bd71988c0d21e53d3cf6"
    ),
    "runner::close_ingress_for_rollover": (
        "61ae9f7cd71bc2576f9330c5874d2018b873d6144514e9f733f5774343ddd1a5"
    ),
    "ingress::unbind_leader_wire_lifecycle_gate": (
        "4a5b25faedea52bea79ac5041b0f643b5baf7bbb0858d9e6953a5f9e9260869c"
    ),
    "ingress::unbind_height_ingress_gates": (
        "966e4feae1f1ca53553abce8847c1a14f8275eb81d4003bedf95e3591f362714"
    ),
    "ingress::close": (
        "24741c2de73120ea5e1a9564203f5d5e0bd9d80a7f1a89d0bca2511e73dee1a7"
    ),
}
_PRODUCTION_HEIGHT_INGRESS_BINDING_TEST_ITEM_SHA256 = {
    "runner::height_ingress_bindings_retire_both_gates_in_one_closed_cut": (
        "6f21dffceb9d005825946d6c39f5c02fc8c35c6ffc75fd379a825beb7d765208"
    ),
    "runner::height_ingress_bindings_drop_fails_closed_on_mismatched_or_partial_ownership": (
        "e4a8ab5a434808f3cc14752c4fbbfa4d6d9ce7e5fcf63d4da2dbeb2905ddc8d9"
    ),
    "worker::closed_height_atomically_retires_serve_and_leader_ingress": (
        "be7493c6461cb56ae73d8355a7b1da9bf5b6eaa89a95c039107052ba7af82220"
    ),
}

_PRODUCTION_CERTIFIED_SERVE_INGRESS_BINDING_ITEM_SHA256 = {
    "CertifiedServeIngressBinding::bind": (
        "4033d2192ddb54c72c444ba5a53f1d0bfd04de32dbbe78582f57a6abe4b8b013"
    ),
    "CertifiedServeIngressBinding::retire": (
        "eb6d1d6a225610f182464077a030316af3cb66a95322f504b76c754134fa6bb0"
    ),
    "CertifiedServeIngressBinding::drop": (
        "88ddf5ff64ea693fbb027a716505a017236eb44e435eced813a6e4f6fc9bba61"
    ),
}

# Exact source seals for the configuration-to-worker capacity corridor.  The
# constants are checked as exact source tokens below; these item digests bind
# the checked arithmetic kernels, the user-root call site, and production's
# narrow refinement wrapper to the same geometry.
_PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256 = {
    "actual::sumeragi_v2_exact_output_shared_ownership_capacity": (
        "e71d1a2376fc34aac057abebde05a41c5b32bbcbad17a409c954845bc4aa64ef"
    ),
    "actual::validate_sumeragi_v2_exact_output_geometry": (
        "b9ad00e3d2ee76b202fa98f53cab9f7264c63a7d9a3050b0c3d57c0449cfb8f5"
    ),
    "user::Root::parse": (
        "7676351f7b370537454bceb3933c2d543269a141de3f2fcc9183d7c77b506f92"
    ),
    "worker::validate_shared_ownership_geometry": (
        "67026793b1424da887ccec0301157480e43b3585d298ab1545fe98e8cb577411"
    ),
}

# Exact token seals for the serialized runtime's disjoint C/P/K/+1 queue
# geometry.  The retained certificate is classified from the immutable queued
# command, so arrival order cannot consume an ordinary Completion or Progress
# reservation and several distinct certificates still share one physical
# credit.
_PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256 = {
    "RuntimeQueueConfig::validate": (
        "f2bfef98a26b8817ca45fcc55c5669d215ae0768f8f09425baea4bec1024b52f"
    ),
    "RuntimeQueueConfig::normal_limit": (
        "fb3d747ef54e8a0432a6ff94407135ef68f16fc94773609f3975a93ab043dfa4"
    ),
    "RuntimeQueueConfig::progress_limit": (
        "5e87c00d381779d2e6a9af03a4497f1fc6f5eba07cf802de46d14f24e78bf72c"
    ),
    "RuntimeQueueConfig::ordinary_total_limit": (
        "1180f68279ceb396f9d3bf49d2bc3c79e138bb1d7ce2f64aba17f0f75562c27f"
    ),
    "BoundedIngress::certified_fence_escape_credit": (
        "94d47c1be045f5eb4f4b7840550b242df8471480033cdfc839d2614467c96ff0"
    ),
    "BoundedIngress::check_capacity_change_inner": (
        "b32cae42a3a389680b1d3566c21012b9db3aa19729dcb83d0c6a0e127a10cc0c"
    ),
    "BoundedIngress::remaining_capacity": (
        "533205971ebce432b614faaa4fdcdb8c86285ef7aeef75314eab78c12cd59f93"
    ),
    "wire_payload_is_certified_fence_escape": (
        "01d6853abc2e1b0e5f2a84197ce271fff2c4b71ba7467f64e0d7c85b3da8237a"
    ),
    "SerializedV2Runtime::has_certified_fence_escape_credit": (
        "a582bffffe57486d860afa3ad9387ad54c6bcab75dda5f2cd82a856605a7d657"
    ),
    "test::certified_commit_uses_physical_slot_reserved_from_completions": (
        "fe5d361fd09f716cca60ceecab8f4c3acfb1ec01080d16b9851af71277f15e53"
    ),
    "test::certified_commit_arriving_first_preserves_every_ordinary_reserve": (
        "5c1aa2de8dfce58415d400d2d4db69988d63fc43d57090ca31f96202625acadd"
    ),
    "test::distinct_certificates_share_exactly_one_physical_credit": (
        "b8a97dace4bd30ba1cac54a69d12e89640ce9b3cbc6b9c15368a7d17c6728630"
    ),
    "test::invalid_configuration_is_rejected": (
        "6ac0e66e597904f6a3b682eeed89bcb2c185b6ce2a402618071fc0490a645d49"
    ),
    "test::queue_configuration_excludes_one_certified_credit_from_ordinary_limits": (
        "e7ddac2312bc5e327d444eb3e24b022d332032d5534a46c56b09857fb50dd3ea"
    ),
    "test::prepare_qc_cannot_spend_the_certified_physical_credit": (
        "aa348cc05ab9331f2b738eeae41cbd4dc74946fb759ab8e3ed1c41c347e9b376"
    ),
    "test::retiring_the_sole_certificate_does_not_fake_completion_headroom": (
        "4751f1f10ce81f149d80f1a7e65da319d4f397fb21167f3e403e147b872dbd1f"
    ),
    "test::unpublished_body_replacement_cannot_overbook_the_certified_slot": (
        "7e1cff975df51a63a6f0e8a9dbfe11829c22b782622c3a4517feac9c232aadb6"
    ),
}

# Exact source seals for the retained certified-body response's one-shot
# pacemaker escape.  The carrier phase is process-local and deliberately not
# serialized: Fresh may admit one direct authenticated root, Charged records
# an already-owned root, and Spent is absorbing until this carrier retires.
_PRODUCTION_RETAINED_RESPONSE_ESCAPE_LATCH_RUST_ITEM_SHA256 = {
    "effects::RetainedCertifiedBodyResponse": (
        "a02346d6e21ac701d7019d6eb235e23d4bdf9551ab01da5123680c9602df0512"
    ),
    "effects::accept_certified_body_response_with_ingress_ownership": (
        "d05e901df51cb12ad89eaeb87a38771d24874288e933604253dad046198216d1"
    ),
    "effects::retained_response_may_admit_certified_fence_escape": (
        "f29a3e106087c871ba0f5f684b4bd47974b783d81085c7751da2c9d314e6c635"
    ),
    "effects::reconcile_retained_response_certified_fence_escape_phase": (
        "4fcb519c7dfd10f164301a0f2342f8be3a771ce9cd6937d8f49f461334762f42"
    ),
    "runtime::has_certified_fence_escape_credit": (
        "a582bffffe57486d860afa3ad9387ad54c6bcab75dda5f2cd82a856605a7d657"
    ),
    "runner::run_inner": (
        "ab23dad98c55d25f940f2da39ba7c053e6a20fa3d27fa7ed87900a0dd31ee0fe"
    ),
    "test::retained_response_certificate_escape_is_charged_only_once": (
        "e02e3d3dcd687669c86ec284919831836d03986292eab84974d6f3896d0d0aca"
    ),
}

# Comment-normalized TLA+ seals for the production-corresponding latch,
# direct-certificate classifier, runner case split, and strictly descending
# retained-response rank.  File qualification prevents an identically named
# helper in another shard from satisfying the contract.
_PRODUCTION_RETAINED_RESPONSE_ESCAPE_LATCH_TLA_OPERATOR_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncCertifiedFenceEscapeKinds": (
            "786d33483ab9bf86aaa0136d745ff96c8328d8d9e36350ec747f1b382966c7ca"
        ),
        "AsyncCertifiedFenceEscapeItem": (
            "bd98c586610ed4fb438fefa1ec50c77d35e7cb79e9761aa648bddbf08893ffad"
        ),
        "AsyncAuthenticatedCertifiedFenceEscapeAuthorities": (
            "7a26804157995e484ebf8f7915a7cde93edf5a14b3a3a94bae59e5a805bdedb5"
        ),
        "AsyncQueuedCandidateIsCertifiedFenceEscape": (
            "5d63ceac0c420a599ff962b17a038a2366deef93f8835fe1d9b9d756a0fbf26b"
        ),
        "AsyncCandidateHasCertifiedFenceRoot": (
            "a3f22434b15e088720a58cf37b7afa9c9b5d90451878440b64eec1646eb23037"
        ),
        "RetainedCertifiedBodyResponseMayAdmitCertifiedFenceEscape": (
            "6f0f721bafbac4ec2aec426010344a5baa9ca4af56b4a13ce6bc3b26151164d9"
        ),
        "AsyncCertifiedResponseClaimStateAfterRetirement": (
            "a7aff8755d881b501f36057d9afca78185d642821f1cfdeaae7a4c50f7254048"
        ),
        "AsyncCertifiedResponseClaimStateAfterAdmission": (
            "8717fca0467d129da9478a981f060b7648d8a62449d91b19546a9b7192c98d60"
        ),
        "RetainedCertifiedFenceEscapePhaseAfter": (
            "c1dd6b64617c5eee7b4e801985323cd3e43f647a548f8a95fb70c2b08c403508"
        ),
        "AsyncCertifiedFenceEscapeStateAfterRuntime": (
            "f7f3c06acfd57dc457b8abc7487a0e4e1bc9d7d350c7323460a2ab8f900a78bc"
        ),
        "EnqueueCandidate": (
            "8d58a005cea2550753196f96cacc3ed54e62bf2eb1a3e4195215d3b8062754f9"
        ),
        "SerializedCertifiedPacemakerStep": (
            "c1d1fdaf28b0451f5daebde2f068fab93debcdaa77cde0b97c8b69e1cc9b8813"
        ),
        "RunNodeWork": (
            "551f97e97f2ecb6ba67053464f480e2fe1cd6d59290a2b1fed261182e2fd1aab"
        ),
        "AsyncControlServiceStateTypeInvariant": (
            "c23f80802359d9f5ed4d7a5e3c148f37da646e4783ec7557a950515aa28ab989"
        ),
        "AsyncControlServiceSlotTransition": (
            "d7398b0e68c3abd377c5cc36554b47c172e6bb22980c83e324fbccf58d3470e6"
        ),
        "AsyncTransportInit": (
            "f2590d8d4066646f438a372fb1051995ea9839487aa882fc841bb0cd3ea88974"
        ),
        "AsyncCompletionReserveInvariant": (
            "b7756857d8b495cb29494dc89dd3ee626f5999418ec3e37c01dc1ee1216a58c1"
        ),
        "AsyncCertifiedFenceEscapeEpisodeInvariant": (
            "f1a0278402350d50b71621bb4db20ca3b5b785c9243fa5024295983c366351fc"
        ),
    },
    "SumeragiV2AsyncTemporalRankProofs.tla": {
        "CertifiedResponseClaimDirectPacemakerWorkTokens": (
            "4d91f823b39ba6a742f3ff33bcce7c7c1aa770d6b1756b8319232c726a6a3410"
        ),
        "CertifiedResponseClaimCausalPacemakerWorkTokens": (
            "c86cef0ce88f5a7dfd586fc2c21788e8bc3e913350b5a625c42e4b448a980bff"
        ),
        "CertifiedResponseClaimPacemakerWorkDebt": (
            "c2215cbc829f79413da4eda26729f6c024d2af7f304ec2ea5754eb14acb39b2b"
        ),
        "CertifiedResponseClaimFreshEscapePotential": (
            "2e58407806507852a2cb2baa1f2c40c27991c04f73cdfa8f200ef43839ba496b"
        ),
        "CertifiedResponseClaimPacemakerDebtBound": (
            "426bfa46b5f8a8421bfa0832d5343f3b2725ef86d38fd794f7d67faf2f884c8b"
        ),
        "CertifiedResponseClaimCapacityDebt": (
            "d9ad22b46691768a71fe0014fc65eb72aafebc37dc620ab556e78e313480a789"
        ),
        "CertifiedResponseClaimAuxRank": (
            "8e2c0484e46949f9dc137b134b621f17d280ef487e581cee7a392422be76a820"
        ),
        "CertifiedResponseClaimAuxCarrier": (
            "31495b77a98cbe265801912add8b51eeff6f99e371598b3cb9e44d65f4a4b731"
        ),
        "CertifiedResponseClaimAuxOrdering": (
            "1d50cd09000834fdc169eff83eb20d88b4e4a79d2d9caa643e7b97bc02e22a90"
        ),
    },
}

_PRODUCTION_RETAINED_RESPONSE_ESCAPE_LATCH_TLA_THEOREM_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncInternalCertificateSuccessorsCannotRetainFenceCredit": (
            "eb6f36a361852fda4e1bf31b4e45efdb28ced19a05d0a0f4dd61a9f3f4264bd7"
        ),
    },
    "SumeragiV2AsyncInstallRunnerProofs.tla": {
        "SerializedCertifiedPacemakerPreservesClaimIngressOwnership": (
            "8943a6e12fe98c7a816a0f8229964c2304b0e48a208ad672264ee75329f29890"
        ),
        "SerializedCertifiedPacemakerRefinesCoreBracketNext": (
            "b7352d2bf625ed6732277874a52e357bd4188a37d475da900799cdbe231e34d8"
        ),
        "SerializedCertifiedPacemakerPreservesSchedulerType": (
            "72ab5903561b098a378ef39a395f0b0d42a989aec2ff5a7874c056aac43351f0"
        ),
        "RunNodeWorkConcreteActionCaseSplit": (
            "35bd614265c192fb0d22201c20fde0ec55ffc461c449c932c94dfe8088875792"
        ),
    },
    "SumeragiV2AsyncTemporalRankProofs.tla": {
        "ClaimedResponseSerializedCertifiedPacemakerDecreasesAux": (
            "4f38a2ed305c71bc8c80ad245dbb4626bf0954e094d4238259509c97abd2b42d"
        ),
    },
}

# Complete token seals for the process-local fair-ingress carrier and every
# exact-output consumer which must retain it.  These close the route-only gap:
# equal payload bytes cannot substitute a different semantic origin, and
# alternate authenticated sources keep independent non-regressing cursors all
# the way through runner, effect, worker, lane, and sidecar service.
_PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256 = {
    "ingress::merge_downstream": (
        "9542ecd100449b693ebae4c1dbea39f43d651c57533ece26dccb193eab6f77bf"
    ),
    "ingress::merge_downstream_with_observed_receipt": (
        "c097d227ef0a670c2144dffcc30a6a0b18016b781ba6a51728c21b2ee8bf32e2"
    ),
    "ingress::merge_downstream_with_strict_receipt": (
        "e65283e5924e1dfe938cb69289a35067a01d3835027ba65587a32f515ed90032"
    ),
    "ingress::merge_downstream_with_exact_routes": (
        "23e937626dd546ae52105bd4e17033d8785e19dfd6ecca7c0d21ebd840d80231"
    ),
    "ingress::same_semantic_request": (
        "b30874c2662dc448ae573d60fb9892310817975b35dde2bfe5d389d08ada7730"
    ),
    "ingress::matches_message": (
        "258d127f5dabef91f22b0d7cca66637770fe3bd607331068ec6f09bd6567f43b"
    ),
    "ingress::matches_semantic_origin": (
        "222fb8e00e39a211f73c88d8c7a41b83a82897c7428f70339aa38e40f78da719"
    ),
    "ingress::process_local_projection_hash": (
        "613b7e84f8639d218294bdfeb548e573e6e0c207cf23784dd30c08459b0a9328"
    ),
    "ingress::matches_reply_routes": (
        "98580b7c9e69cc4c69cc32ec6212459e93fba573a335151fec476659d2772c2f"
    ),
    "ingress::project_retained_reply_routes": (
        "6a95a61e3dd7ce8da838fd99693205618d3ec3979ea88b46599ecd68ebfd2fdb"
    ),
    "ingress::advance_reply_cursors": (
        "2be088efc3ff0460b06c0316f7b64062d864d22d8fce286a1474fd1e31af3c82"
    ),
    "ingress::validate_exact": (
        "fe23c14b3e612c4e8461912100a22e38a31b9038b06a8948856a6bc3a16bbf58"
    ),
    "effects::accept_payload_chunk_with_ingress_ownership": (
        "04c48786b1db18841d32be400432570fcd268452506af9735c2dc5354a72e5db"
    ),
    "effects::accept_certified_body_response_with_ingress_ownership": (
        "d05e901df51cb12ad89eaeb87a38771d24874288e933604253dad046198216d1"
    ),
    "effects::classify_payload_chunk_lifecycle": (
        "1593a2bcd882c7054cb3e176631931ee8cda5a385c25af3ca510dbf2aaddb787"
    ),
    "effects::begin_apply": (
        "9ec211be03999eb404d75587eeb54e0294ac3009557df1789800154d8edb10bb"
    ),
    "effects::matches_apply": "c09123a634f4d856b967bbbcdc198b09b30d7fbb8b8c949aaf74f33bb06361e4",
    "effects::complete_application": (
        "bf9b678fb096b0c05573f6320c6f9c76f38f20ee276f8a7880e75b3635dc754a"
    ),
    "ingress::leader_wire_token_view": (
        "ef9f78efaa117ac2a787a9791e87d5a199978cf1d5a87d8a9e43b94f12e12b82"
    ),
    "ingress::leader_wire_token_matches_chunk_manifest": (
        "062164107c1c1363e538a4ed827b13510b3b0c2a91505d2b77c097f44dbc7a1b"
    ),
    "ingress::leader_wire_token_matches_body_coordinates": (
        "2bc498a9088c735c18803dde3461d50afd70712ac40f7875457290d9796de385"
    ),
    "ingress::leader_wire_token_matches_exact_body": (
        "645a29b8cff1d68fca203fd037f3c665189ab801c1bbe97eca8b6dfdc04e0b7f"
    ),
    "worker::claimed_with_reply_routes_and_ingress_ownership": (
        "8c95a2604897a3dbf327d36721388bf466784f2203869d1fc6aead594ecd7e44"
    ),
    "worker::serve_certified_request_on_routes": (
        "6cfa46d3647ba72ae53fa12585926eb6803ed38dff9d656894ba7e7693dc06ac"
    ),
    "worker::queue_commit_serve": (
        "73b2b40e5c2255d557c2e67ccdd58c3593381c5d713b4dd6315e48c350ff7dfb"
    ),
    "worker::io_handle_certified_serve_ingress_gate": (
        "87acd6865d179d3911a66692e063ece997fbf7669a81493b19d9f443e1291735"
    ),
    "worker::services_certified_serve_ingress_gate": (
        "95906877d32686cc23249747ff5e31f8552d5abb237277f96fd4091da1a92d77"
    ),
    "worker::route_payload_chunk": (
        "ed0c01f2a93defb44b23ffa108c622bbdc4550588a859f44ac7313490f52c705"
    ),
    "worker::has_exact_reconstructed_completion": (
        "2d494b9f5ea950b8fa1e24c3b6f1a9d5f8917dc166658660300d9e66b9064bfe"
    ),
    "worker::buffer_orphan_payload_chunk_inner": (
        "03c11a6a6438f3a0f7cf47cac8f5310b262fd684a9df7891ff906b7f2e0d7de8"
    ),
    "worker::sweep_buffered_payload_chunk_lifecycles": (
        "04d1c1e43bd4c41a0061d7585580907e6d8e4a1d5a2403c06bda1234b3ee51f7"
    ),
    "worker::replay_buffered_chunks": (
        "1259249ed54174dfcecffbacb6e2f70b3a5fbf31e59bd088f6160d3148648920"
    ),
    "worker::retire_buffered_payload_chunk_tail": (
        "fa77f1610c7d58dd73f05cb353d10a4c2a1db4f28b58777138bcfc2daad475e1"
    ),
    "worker::deliver_payload_chunk": (
        "f4341f1f806d10adab9e14b0c4b13c463fc7c6dae22bd7056863548d0912fb5b"
    ),
    "lane::accept_lane_message_owned": (
        "4c3b13ab1d0821d97604c8f9119ecc27d1c5be5c2a635b228027d35ed3adf96c"
    ),
    "runner::v2_ingress_head_can_drain": (
        "d4e61362952b96d782ee41e1c6081a76132086b8e82cf430f8c0003deb8dbe70"
    ),
}

# Exact comment/literal-free token digests for recovery-scoped eager CommitQC
# discovery. These are production source-fidelity seals, not machine proofs:
# the corresponding starvation-freedom obligation remains specified_unproved.
_PRODUCTION_RECOVERY_EAGER_BLOCK_SYNC_ITEM_SHA256 = {
    "initial_block_sync_deadline": (
        "b3455d656ecac4787561951ec55cc8cde44e2a2c90a3330ad3fe0c65e96c2185"
    ),
    "retain_eager_block_sync": (
        "b5ed7336da6088907bcd26f1b21d2e0bd91a972e8bce68a6e190f238d4c2b56d"
    ),
}


def _transport_geometry_refresh_resistant_errors(
    paths: dict[str, Path], sources: dict[str, str]
) -> list[str]:
    """Check reviewed P2P/Taira semantics without formatting-sensitive seals."""

    errors: list[str] = []
    for role, item_name, required, description in (
        (
            "p2p_network",
            "relay_message_wire_payload_len",
            """
let origin_signature_len = byte_sequence_wire_len(origin_signature_bytes)?;
let field_lens = [
    origin_len,
    target_len,
    ttl_len,
    priority_len,
    origin_signature_len,
    payload_len,
];
""",
            "relay geometry must charge the origin signature and every exact wire field",
        ),
        (
            "p2p_peer",
            "parse_next_encrypted_frame",
            """
let _decode_scratch_lease = self
    .source_byte_budget
    .reserve_decode_scratch(size)
    .await
    .ok_or(Error::FrameTooLarge)?;
let frame_retention = self
    .current_frame_retention
    .take()
    .expect("complete encrypted frame must hold its source byte lease");
""",
            "receiver decode scratch must be reserved before taking the source-owned ciphertext lease",
        ),
        (
            "p2p_network",
            "handle_service_message",
            """
let task = connected_from::<WireMessage<T>, E>(
    self.public_address.clone(),
    self.key_pair.clone(),
    self.soranet_transport_key_pair.clone(),
    Connection::from_split(conn_id, read, write),
    service_message_sender,
    self.idle_timeout,
    self.network_id.clone(),
""",
            "inbound stream handoff must carry separate validated node and transport identities plus the canonical network identity",
        ),
        (
            "p2p_network",
            "run",
            """
Some(update_validator_dial_control) = receive_control_update(
    &mut self.update_validator_dial_roster_receiver,
) => {
    match update_validator_dial_control {
        ValidatorDialControlUpdate::Roster(roster) => {
            self.set_validator_dial_roster(roster);
        }
        ValidatorDialControlUpdate::Topology(topology) => {
            self.set_validator_topology(topology);
        }
    }
}
""",
            "network actor must consume coupled validator dial-roster and topology ownership updates",
        ),
        (
            "p2p_network",
            "peer_connected",
            """
self.validator_dial_scheduler.note_session_established(
    &self.self_id,
    peer.id(),
    tokio::time::Instant::now(),
    self.connect_startup_delay_until,
);
""",
            "accepted peer must publish validator dial session ownership",
        ),
    ):
        items = rust_items(sources[role], item_name)
        if len(items) == 1:
            _require_rust_token_sequence(
                paths[role], items[0], required, description, errors
            )

    kagami_path = paths["kagami_profiles"]
    kagami_source = sources["kagami_profiles"]
    for declaration, description in (
        (
            "const TAIRA_MAX_FRAME_BYTES: usize = 23_068_700;",
            "Kagami Taira encrypted-frame ceiling",
        ),
        (
            "const TAIRA_MAX_FRAME_BYTES_BLOCK_SYNC: usize = 23_068_672;",
            "Kagami Taira block-sync frame ceiling",
        ),
        (
            "const TAIRA_MAX_FRAME_BYTES_TX_GOSSIP: usize = 11_534_336;",
            "Kagami Taira transaction-gossip frame ceiling",
        ),
    ):
        _require_rust_source_token_sequence(
            kagami_path, kagami_source, declaration, description, errors
        )
    render_items = rust_items(kagami_source, "render_peer_config_with_private_keys")
    if len(render_items) != 1:
        errors.append(
            f"{kagami_path}: require exactly one Kagami "
            "render_peer_config_with_private_keys item; "
            f"found {len(render_items)}"
        )
    if len(render_items) == 1:
        render_source = render_items[0].source
        expected_branch = '''let taira_network_frame_overrides = if spec.slug == "iroha3-taira" {
        format!(
            "\\nmax_frame_bytes = {TAIRA_MAX_FRAME_BYTES}\\n\\
             max_frame_bytes_block_sync = {TAIRA_MAX_FRAME_BYTES_BLOCK_SYNC}\\n\\
             max_frame_bytes_tx_gossip = {TAIRA_MAX_FRAME_BYTES_TX_GOSSIP}"
        )
    } else {
        String::new()
    };'''
        expected_installation = '''[network]
address = "{network_address}"
public_address = "{network_public_address}"{taira_network_frame_overrides}

[torii]'''
        if render_source.count(expected_branch) != 1:
            errors.append(
                f"{kagami_path}:{render_items[0].line}: Taira-only Kagami frame "
                "override branch must render all three reviewed constants"
            )
        if (
            render_source.count(expected_installation) != 1
            or render_source.count(
                "taira_network_frame_overrides = taira_network_frame_overrides,"
            )
            != 1
        ):
            errors.append(
                f"{kagami_path}:{render_items[0].line}: Kagami peer config must "
                "install the Taira frame overrides in the network table"
            )

    renderer_description = "Taira renderer scales aggregate bytes by N+H+1"
    try:
        renderer_tree = ast.parse(sources["taira_renderer"])
    except SyntaxError as exc:
        errors.append(
            f"{paths['taira_renderer']}:{exc.lineno or 1}: "
            f"{renderer_description} requires valid Python syntax"
        )
    else:
        functions = [
            node
            for node in renderer_tree.body
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name == "_scaled_sumeragi_body_bytes"
        ]
        assignments = (
            [
                statement
                for statement in functions[0].body
                if isinstance(statement, ast.Assign)
                and len(statement.targets) == 1
                and isinstance(statement.targets[0], ast.Name)
                and statement.targets[0].id == "minimum"
            ]
            if len(functions) == 1
            else []
        )
        expected = ast.parse(
            "minimum = (validator_count + authenticated_non_validator_sources + 1) * source_bytes"
        ).body[0]
        if (
            len(assignments) != 1
            or not isinstance(expected, ast.Assign)
            or ast.dump(assignments[0].value, include_attributes=False)
            != ast.dump(expected.value, include_attributes=False)
        ):
            errors.append(
                f"{paths['taira_renderer']}: {renderer_description} must have "
                "one exact semantic assignment in _scaled_sumeragi_body_bytes"
            )

    config_contracts = (
        (
            "taira_default",
            ("sumeragi", "block"),
            (("max_transactions", 96), ("max_payload_bytes", 16_777_216),
             ("proposal_queue_scan_multiplier", 4)),
            "default Taira profile pins the revision-4 payload ceiling with privacy framing headroom",
        ),
        (
            "taira_default",
            ("sumeragi", "queues"),
            (("authenticated_non_validator_sources", 2), ("body_bytes", 346_030_080),
             ("body_source_bytes", 34_603_008)),
            "default seven-validator Taira profile pins H=2 and ten source partitions",
        ),
        (
            "taira_default",
            ("network",),
            (("max_frame_bytes", 23_068_700), ("max_frame_bytes_block_sync", 23_068_672),
             ("max_frame_bytes_tx_gossip", 11_534_336)),
            "default Taira profile carries maximum privacy transaction and block-sync frames",
        ),
        (
            "taira_config",
            ("sumeragi", "block"),
            (("max_transactions", 96), ("max_payload_bytes", 16_777_216),
             ("proposal_queue_scan_multiplier", 4)),
            "production Taira profile pins the revision-4 payload ceiling with privacy framing headroom",
        ),
        (
            "taira_config",
            ("sumeragi", "queues"),
            (("authenticated_non_validator_sources", 2), ("body_bytes", 242_221_056),
             ("body_source_bytes", 34_603_008)),
            "production Taira profile pins H=2 and seven source partitions",
        ),
        (
            "taira_config",
            ("network",),
            (("max_frame_bytes", 23_068_700), ("max_frame_bytes_block_sync", 23_068_672),
             ("max_frame_bytes_tx_gossip", 11_534_336)),
            "production Taira profile carries maximum privacy transaction and block-sync frames",
        ),
    )
    parsed_configs: dict[str, dict[tuple[str, ...], list[int]]] = {}
    for role in ("taira_default", "taira_config"):
        table: tuple[str, ...] = ()
        values: dict[tuple[str, ...], list[int]] = {}
        for line in sources[role].splitlines():
            section = re.fullmatch(r"\s*\[([A-Za-z0-9_.-]+)\]\s*", line)
            if section:
                table = tuple(section.group(1).split("."))
                continue
            integer = re.fullmatch(
                r"\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*([0-9][0-9_]*)\s*(?:#.*)?",
                line,
            )
            if integer:
                values.setdefault(table + (integer.group(1),), []).append(
                    int(integer.group(2).replace("_", ""))
                )
        parsed_configs[role] = values
    for role, table, expected_values, description in config_contracts:
        observed = {
            field: parsed_configs[role].get(table + (field,), [])
            for field, _value in expected_values
        }
        if any(observed[field] != [value] for field, value in expected_values):
            errors.append(
                f"{paths[role]}: {description} must match exact numeric values "
                f"{dict(expected_values)!r}; found {observed!r}"
            )

    for role, description in (
        ("taira_genesis", "production Taira genesis DA pins the revision-4 protocol ceiling"),
        ("taira_default_genesis", "default Taira genesis DA pins the revision-4 protocol ceiling"),
    ):
        try:
            genesis = json.loads(sources[role])
            observed = genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"]
        except (json.JSONDecodeError, KeyError, TypeError) as exc:
            errors.append(f"{paths[role]}: {description} in valid JSON: {exc}")
            continue
        if type(observed) is not int or observed != 16_777_216:
            errors.append(f"{paths[role]}: {description}; found {observed!r}")
    return errors


# Exact comment/literal-free token digests for the allocation-free P2P frame
# geometry used by Sumeragi height activation.  These helpers are part of the
# production refinement boundary: replacing checked arithmetic with a shorter
# estimate can make a locally valid progress envelope impossible to transmit.
_PRODUCTION_P2P_FRAME_GEOMETRY_ITEM_SHA256 = {
    "checked_len_prefixed": (
        "b6411bf29b1e2517fb2c4151c52634334f712f05127c4efd6440166ee6b65207"
    ),
    "peer_id_wire_len_from_raw_key_bytes": (
        "b13f1926dab04641ff700941aaf854a29a0205e4828831da3f207a0930829844"
    ),
    "peer_id_raw_key_bytes": (
        "8f3464a658389a9eed4a02b26e3117aebe9c11283d825736ea6ef5fe797ec09e"
    ),
    "relay_target_wire_len": (
        "84837f33c9793445071c17cdc11de01ddc1b7b57e061896afd381252743d3c05"
    ),
    "relay_message_wire_payload_len": (
        "9b1634e5ca74ca51e435b2e2f85220545782d1d2fdda1397f79199634b5ec617"
    ),
    "data_frame_wire_len_from_payload_len_with_peer_key_bytes": (
        "6a7f400c67afbc25c834fdb17f782baf374017cca082295f88bbfda501199fb1"
    ),
    "data_frame_wire_len_from_payload_len": (
        "33df4c34f2034f666dfe403b97869e498094f42a6fe35c5ce0ab75164fe0bcbe"
    ),
    "validate_transport_queue_geometry": (
        "2b43cba3a15fb667169280663e960cb6fabf5ccde7272d4276b6480041c66632"
    ),
}
_PRODUCTION_P2P_PEER_FRAME_ITEM_SHA256 = {
    "data_message_wire_len_from_payload_len": (
        "d157ece83c8e700725549e91fcb572b61ebe3d8c3e267e9d3bfe31765ff3310c"
    ),
}
_PRODUCTION_P2P_RUN_FRAME_ITEM_SHA256 = {
    "frame_plaintext_cap_for": (
        "4d66d6b2dc3c139c4df54c7c9d7b7640691ce3aa0765a44298b689e63d360e21"
    ),
    "checked_encoded_frame_len": (
        "e7d2ca074d7c7eb85d16608018e4e8d30950bc2278edbe52e3a63e31c2b15935"
    ),
}
_PRODUCTION_P2P_QUIC_FRAME_ITEM_SHA256 = {
    "try_send": (
        "bd32ba60bc0de89dc1ac9a7062a69c41df4ff6c72dd5c053550385780510891e"
    ),
}
_PRODUCTION_P2P_RECEIVER_FRAME_ITEM_SHA256 = {
    "reserve_for_frame": (
        "cb0e506080c0985c5f81c5c567250e143d072f2f23e06b4072cb3d2b739a8212"
    ),
    "parse_next_encrypted_frame": (
        "576e689feed5c363d31dad75e8afdfd2936066a4f8c91ff71b087594001e7619"
    ),
}
_PRODUCTION_P2P_SOURCE_OWNERSHIP_ITEM_SHA256 = {
    "same_owner": (
        "e6bba8d24c683f3b322752c93d977415e41e8dffcd9221f410d462a4006835db"
    ),
    "merge": (
        "36f4915a83458c73f12ff364ebf4ccdd45e6b826fe305e52429098967d26bb55"
    ),
    "source_credits": (
        "f3c9f0c68484f0685560f896d3f782c5c75eb2edf336184ba3ca230bddced095"
    ),
    "extend": (
        "c0d7b44202b40992e33ae23c530fa6c135ff003e23765dde461a044a3a6d8d46"
    ),
}
_PRODUCTION_P2P_SENDER_FRAME_ITEM_SHA256 = {
    "encrypted_frame_geometry": (
        "fa320e44681f1928dd862e8f07b1ea3c72b2c409ba58bf88556c2edd6d57b929"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_encoded_buffer": (
        "eb243e718f0b02455dcb31f009ea22aca2f33b4c27aaa00cd309846d87a159fb"
    ),
    "check_queue_limit": (
        "097b47794fe8e9d4ded5357204dfe755f99d476323e11d4c50b604efad0dfb48"
    ),
    "account_enqueued": (
        "dc14a3102e45e8487ed71d3cf7c43af270be5bad47b00fe69fac9f85e51dba13"
    ),
    "enqueue_encrypted": (
        "1efc3f5c8f677f117d82e7e1dcec9c489fbddbcf970b3bba74fd184c5f138421"
    ),
}
_PRODUCTION_P2P_START_FRAME_ITEM_SHA256 = {
    "validate_encrypted_frame_cap": (
        "5305bae9d0febfc2a1348f8f3b9737fb5155a62084a80f731d75bc372ea3bbcd"
    ),
    "start_with_crypto_and_initial_authorities": (
        "55efa7e3f6eb9c9eaff558942917b084f5770a0839e3ed893a915e1436748b2d"
    ),
}
_PRODUCTION_P2P_RELIABLE_PEER_ITEM_SHA256 = {
    "post_recover_with_flush_ack": (
        "efe28549106501cb539ea51d42e5c318f5c54bfeb8bef94e1cd1a78a85ea499a"
    ),
    "post_recover_inner": (
        "004f480809bdd5e6f456a5da08b2acd192fb817c44606eec5066141aed2f5b7f"
    ),
    "acknowledge_flush": (
        "b2f9e0921009c50799677284aef793d9a76d184db88b24b1b242eb093e8f71cb"
    ),
    "prepare_owned_or_defer": (
        "0fab3dc65badad08481063c4517f590710ea0592f6e3094d8df05df6a8872f50"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_or_defer_with_ownership": (
        "271a4549dc2075166aa9c647a6477f1748f32b536d790dcef967ca9c32087a4d"
    ),
    "retry_deferred": (
        "37298defcaba75e4f3b1e32b55cc6025629e350e2c1a3efe3b358aaad719b3d9"
    ),
    "flush_plain_high": (
        "38da1a9853b5a4c780a3c4dc898ec9ba328b39c1128b7fce7cd2922d2ed5c32b"
    ),
    "flush_plain_low": (
        "bf7e0c4580f6a886ed26d61258f5d8017821b9ab8246ceed1e1f63e510676dd6"
    ),
    "enqueue_current_buffer": (
        "e105440c89db28ab02b220b287af24f04939d31ef0e26bde32cc0346f0450a69"
    ),
    "acknowledge_flushed_batch": (
        "30d441537005fc905493103db122d17670def105f67d36993341038b674a4c63"
    ),
    "fill_batch": (
        "30bba0f4890fd277ee9b22cd433178d840418dbd22a1a615f2a1153857b09ae7"
    ),
    "pop_high_frame": (
        "e7bd48c39d938732bd44cff707989aca7e8d1f55e75c20d559a35ad788302a74"
    ),
    "pop_low_frame": (
        "3d472752bbb83b40712993959290bacdbebe26d604c38500f701642ea62b33a9"
    ),
    "send": (
        "a10e589b0e785b9d113d693e7946a35bc1b6dabb7f8f72c59ff713efe3c393e5"
    ),
    "send_one_ready_stream": (
        "aa54b3d9df965c06ab3b66175859fa31ef5fd377c1bbeacc049a9d8d69a1c7c3"
    ),
    "next_peer_stream_io": (
        "178e60084380d1ac3b9abf5cdcc866748d8de270d215846f85dcc63a7c3b9539"
    ),
    "run": "3829bbd4b883bd592ebdca24e928fb44007a7e0e39295dd9c19fc4e42c356bb1",
    "reattach_reply_route": (
        "120803740de09553bb9112a556cceed7e2db414f4f5da3a9691a7886b5264be0"
    ),
}
_PRODUCTION_P2P_RELIABLE_PEER_QUALIFIED_ITEM_SHA256 = {
    "OutboundPostOwnership::new": (
        "3fc9b02c402019b5eed09485217d42cdeaabe5bf471e0786f0706ac2ba1af503"
    ),
    "RetainedPost::new": (
        "cbb4bebe6f001066fd4491bd693776fc007a932fc524ffed5de1c92209b561a4"
    ),
    "RetainedPost::into_parts": (
        "91b64ad394e845b083fe498e683d447e6d27c5e15cfbf0aa7853fb9633f45769"
    ),
}
_PRODUCTION_P2P_RELIABLE_NETWORK_ITEM_SHA256 = {
    "new": (
        "b06baebfca688fe1182cab468451157e75320543600ec679cfdd46a79b420f61"
    ),
    "validate_delivery_binding": (
        "f231d50a1984538606124ca88b6bfda2be639cc399e2592bf69c43e7bbc10337"
    ),
    "reliable_progress_class": (
        "542367e1dbca6344eab5fd6d22d8614ad543ee29479dae51fea97963eccc75ae"
    ),
    "is_reliable_progress_route": (
        "513e759a4dee7cd1367971b3d7ebafd462134d609421a5ac96da1cfb3f1f3bfb"
    ),
    "for_route": (
        "9163a97125d966988627b262bfaff254eb55239a9957937b395f52cc580cd516"
    ),
    "semantic_target": (
        "7312c96797e37a2a05381feee6f92a4f127b95b1e31c411a1dc517123e8e789f"
    ),
    "source_key": (
        "555b18d0375cc9268d427b6fb04e05b1d5a48099d8bb48921ea85ea1c19e77b8"
    ),
    "authenticated_via": (
        "3e66e6dd0a33ffb60c71db2a4f306abe6c733d54558b94559664503576695de2"
    ),
    "same_tenure": (
        "6119490d174355c24153db559dc66834d10bee4f81c3dbcbaab9fb460cbb24a5"
    ),
    "same_delivery": (
        "dbab8b1b889a1b4ab4e1b8bac73fe981b38eaf943c0f4ed4f5c259bdacec2160"
    ),
    "equal_ordinal_different_tenure": (
        "22b9f323b4a2ff799b1be14ad0f52b3e30633167151e7ce40a2a1f9cef8363ab"
    ),
    "equal_connection_ordinal_different_tenure": (
        "926e428abdd009f73e23f5569410bf36ab1947fb65eca3dc22225751b912a2b6"
    ),
    "same_source": (
        "b95727867096f1decdc53b785e1414412643e70259488585a5571c5cabbcd573"
    ),
    "source_update_from": (
        "e468f9e4d47b950d751340588efbf3dc18def32cc39eedc823c354a0f531ac8a"
    ),
    "source_update_from_snapshot": (
        "8accccfafb03f27d697224fdfde5a2caa2ec6d06746b8c81cdcb07bd8d44ef24"
    ),
    "source_freshness_from": (
        "de335eea19992a5dd1157b00bb2c8ee9bef7067a6626e2ab2b95a96b1f7d0e37"
    ),
    "is_reply_writable": (
        "d53b0fcdbaa42d480a2ebc08b9fe0fba542be5e7ffcd901c0e7564a7553cc97e"
    ),
    "same_request_authority": (
        "96bbd9cc1360fea28d3d6caa772fb5ead105d0bd1fc44466fd977882cac87be5"
    ),
    "try_from_route": (
        "8766ab36eb93485a362dff5c943c7041b2db370f2e5a6a10254c3ad05c333285"
    ),
    "source_capacity": (
        "8a7caf36391c3d7a43862cbfb3de6dcc23b56553321cc34cbfc3b9b383a19557"
    ),
    "retain_active": (
        "e0e6ea9560f49fc055481959c24ec1aeea0918dd09110bc293954f93452bda85"
    ),
    "retain_active_with_receipt": (
        "50df0a9f34c9a05268b4a723e8813fc5fb09259627a069a6f3056c160ec5b27d"
    ),
    "retain_active_with_receipt_after_snapshot": (
        "9f682998f17416d9dd3adfb73a67fda005abf3ca9b0013ec3f3c63e63c2a8e45"
    ),
    "merge": (
        "8b850f96609375104f69852bca87796e62fb3038c2a2e2ca53330114d6a42e33"
    ),
    "merge_with_receipt": (
        "b5e6d03122445a97b51b13d127ca4ea51a7728df5db54bec518aa8e1136043f2"
    ),
    "merge_observed_with_receipt": (
        "7fde28579ed7717942a7ae9ee7cb680e3d5cca8a772b6b9f547e263055bd96fe"
    ),
    "same_exact_history": (
        "cf777335ed65929280f27fbb395af144781f33875cc5d9e2ea3027cf011a46f1"
    ),
    "has_valid_container_shape": (
        "eb733983ed3ea40244c5d6033290582cb65f5b3e7cb827ecc9bdc5d2742f2f38"
    ),
    "preflight_merge": (
        "b4577057f91f3710657d828e83b59ea09f90e41f6ee6965754c05cfd38bba401"
    ),
    "attach": (
        "0c357d084de3e1dddfecf9a291afcd91b96c126d9a8f1a25922478b146eb5903"
    ),
    "validate_after_retired_delivery": (
        "17aa33a5ba1cae849d7eba47dba76327ac85e6a163c5a14b82517a653ccbf6a9"
    ),
    "merge_retired_delivery": (
        "e211a40c5abb7c3438caa11d0838ac061ca0ea339d0c13f150b431320b18e4cc"
    ),
    "record_retired_delivery": (
        "7a331e89f673fa55472e39a2343881b8754b7612cd22d07817056805e34cf513"
    ),
    "release_retired_tenure_binding": (
        "fcb910d728e26b11a0f555edd7aa7fe85b16bf7617ef4951670e90f9cf294446"
    ),
    "reply_route_source_capacity": (
        "6501b55351f33bd6fe59db3c97c28bd69c586e8e9ff94eab09e1c30876923fa7"
    ),
    "peer_connected": (
        "f7d3ce34db0024c18f49c5a1ca5947f5caadbfc055f31368019dbc6b6e766910"
    ),
    "peer_message": (
        "fc267abe5dfc2417a9d51978ac73cd5c10912b7a2f11593491100c8f203f4485"
    ),
    "progress_ticket_request_digest": (
        "ab51b06be057b794221217b6505e2cd4abbb66c2d7f87504a6f2257c260124c4"
    ),
    "matches": (
        "d7aa1ff1bba2b408eacaa4821e5c8791a3298f8ecbbc10a120e4cf4f18dd00aa"
    ),
    "try_reserve_for_source": (
        "b938440d9f487ac064ca8ac46269d88551da51349ce14b4e470c87d672e82888"
    ),
    "submit_progress_message_to_source": (
        "78d5131df35f3d0972597912413d118efcb28be723116c3e29ffeaf9500ba546"
    ),
    "new_targeted_post": (
        "cfaedf859e6596e63c2806c44afcca063f66ec1e660f75047e433ae5a30e727f"
    ),
    "into_parts": (
        "18df4a3deca18212833fd5179f14b15c77399db9c31374ff1afe4486e93c4982"
    ),
    "push_back": (
        "62083b1dfc8bb3d33a14cdcd5ceb441750ee4fe0ce95477bd01e8a985748eadd"
    ),
    "pop_front": (
        "81ec2f9581812789d6bca5ed2a1c608b7c9f4cda17604c3ebdcaa2c3c7d3cfd0"
    ),
    "retry_back": (
        "8c2994b9ac91ebce8e4d28bf394c902655f2f64a5bcc3547b050ac49fd92c38f"
    ),
    "defer_high_priority_network_message": (
        "1b8948a28bf0ccae5431b18c25abd9507b2b832433306f28af50565e960e2dda"
    ),
    "post_reply_recoverable": (
        "e81939836efa2f08aea3d4cc74424e1c15a17dcd25c3718290c90f68ffa2f463"
    ),
    "post_reply_recoverable_with_flush_ack": (
        "b7b67e256375e22c86ff57fb7ccbb26131b24f0b191a5ed317cab1dcce89b888"
    ),
    "post_reply_recoverable_with_flush_ack_inner": (
        "b3597f519695465158e058cd91b48617aaace25fa496a5f9f64833c010c84c8c"
    ),
    "broadcast_recoverable": (
        "548d457392659fd2c68e3d27ea3796ad8477a2e3afd7f93b9930c49d76a66f96"
    ),
    "into_dispatch_parts": (
        "cca1a2804e76c9ba9d07319bf5256be6c53c2e0c39237ace52aa53b86a75a5c2"
    ),
    "retain_after_dispatch_attempt": (
        "6dfa7ebe62b1a848b55c0ef43f945296b51f9014dfff44a1166bf042a6ca582b"
    ),
    "cancel_reply_route": (
        "9bb0a32ff652a06ef803fb010f0ca3ad5a44cca6a16b047ed77da7eb8a1307d7"
    ),
    "cancel_authority_waiters": (
        "cf554e96f17a00ee8f018d6826d8be8efd0366ff7e0c256349353d9f2df3c39e"
    ),
    "release_cancelled_targets": (
        "88ad8d2a8a0183ec28c8baf59d40af851b135abd7c46af65ed411f9ef7fe5b4d"
    ),
    "dispatch_reliable_actor_message": (
        "21cf34eb5aa68209a6baf29124170f073ea3a65be8c4e5887d6c5fbb69ffe20f"
    ),
    "dispatch_reliable_actor_message_inner": (
        "3988f0adb7eae269376964cbe9bd11e6437b7c9955076523704ee7f9f6633687"
    ),
    "post_reliable_actor_frame_to_writer": (
        "9347732994ea2854fc8185768ca08bbd0b3bac410fdb65b3d724b4ccb71d0c4b"
    ),
    "retry_reliable_actor_messages": (
        "5d59dda16998eea883d6c96f6fedde6fa3f8e35a82f16fff6849d35cd4f3ce7c"
    ),
    "accept_reliable_actor_message": (
        "93884415a18ff4e3f7c0c680d0219b8a1d01ebdca398b9c8534aa678a20a21e0"
    ),
    "mark_connection_terminating": (
        "fdba9b05c60f8a85f3a2c03e0b98959ea0e78f143a17e3fc7a2af6000c1f4238"
    ),
    "finish_reply_route_tenure": (
        "50b38fb6bb32cfab9664e611b559cf93e8a9bf566f13883c7acfabbd1c8b0c1d"
    ),
    "handle_service_message": (
        "90ffbbe99334ccd688b114b7296bbb86f481ef6f3ac56a21a704d31151b5a61c"
    ),
    "peer_terminated": (
        "32c662f6a4e5be0b27d9fc7510076bc203b9a5dd7782b22796e89eeeb89fcd9c"
    ),
    "cancel_all_reply_route_tenures": (
        "85556605246c148cb390b143406a6a5320fc14c57ec964c52b1eb5f0f9395ca1"
    ),
    "drop": (
        "ecd2987b63057f26e35d5753bf94c5c68fc4d720c9529630c4b1ff52e18b69ce"
    ),
    "run": (
        "551a96489f8a92f882a448f41d86193eda7527397d7e162ae03740d158bd284c"
    ),
}

_PRODUCTION_P2P_REPLY_FLUSH_ACK_ITEM_SHA256 = {
    "new": "58bdb00f6634be8d93048d75953b45a4ff2354798f99aff9b2f58c03562f04cf",
    "poll": "512c90e2877329331a6ec24ae26eb1a6d021fe735cd42756def239d1e5101cd2",
}

_PRODUCTION_P2P_PROGRESS_LEASE_DROP_SHA256 = (
    "5786fb4d2c21a18923cc7b636dd6a5634f91c6d47879958977eadde37c419f07"
)
_PRODUCTION_P2P_REPLY_ROUTE_IS_ACTIVE_SHA256 = (
    "e2e80f75efe8739fab1d6030ffc1cd0af070ee6d223eca06b953d81b2c3537a7"
)
_PRODUCTION_P2P_REPLY_ROUTE_IS_AUTHENTICATED_VIA_SHA256 = (
    "817add7dcd219464c6d20df132fbd8bfcfe10a0caf56f5a815d86e975b3d03af"
)
_PRODUCTION_P2P_REPLY_ROUTES_MERGE_OBSERVED_SHA256 = (
    "9d21e1b0114d35991682d2f5f16072d33ffb9439426f551a6fe68183120d3e3f"
)
_PRODUCTION_TRANSPORT_REPLY_ROUTE_ITEM_SHA256 = {
    "try_from_transport_with_reply_route": (
        "614facaea868b070cd9ce0dc08e8bb472f3afc3fff9769f09a85128e54a7c11e"
    ),
    "transport_reply_route_construction_is_fallible_and_target_bound": (
        "4a26c079cad7e57615605b7af734211e3b62ba1e9dc54c36f2e0581fe5c23190"
    ),
}
_PRODUCTION_DAEMON_FRAME_VALIDATION_ITEM_SHA256 = {
    "validate_config": (
        "39a6c98e410ac1a5ddf8af00fbacf8b1a49c167228bddaacf770f4cca616762c"
    ),
    "validate_config_offline": (
        "7cb189cf3f5ac23ca68098a57c655e7e8808642de1c90d76a4270603b5426ed2"
    ),
    "validate_network_frame_runtime_limit": (
        "551d207e8371c5966274fc759d379f75f281127f668c6555aba347c385066b92"
    ),
}
_PRODUCTION_P2P_CAP_ITEM_SHA256 = {
    "frame_plaintext_cap": (
        "d09e8a86d8c9d6d4c3e8a9c9f202debab61e11f3256f2e1361d3895a0dd97b33"
    ),
    "frame_queue_charge": (
        "995d225539dcc72b840fc6c807cf380926b4d552da2da05b022de0eb20f85b5d"
    ),
}
_PRODUCTION_SM_DISTID_GEOMETRY_ITEM_SHA256 = {
    "validate_distid": (
        "205a20a45faa4455d25b8e9d2501f6ab66a2b069a83b8e358a9645d71c94181d"
    ),
}
