# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

KURA_PRODUCTION_COMPONENT_FILES = (
    Path("crates/iroha_core/src/kura/startup_finality_support.rs"),
    Path("crates/iroha_core/src/kura/bound_progress_and_retained_support.rs"),
    Path("crates/iroha_core/src/kura/autonomous_reservation_bounds.rs"),
    Path("crates/iroha_core/src/kura/certified_bundle_capacity_reservation_types.rs"),
    Path("crates/iroha_core/src/kura/prune_commit_merge_support.rs"),
    Path("crates/iroha_core/src/kura/merge_ledger_latest_execution_index.rs"),
    Path("crates/iroha_core/src/kura/replica_advert_and_body_status.rs"),
    Path("crates/iroha_core/src/kura/retained_finality_replica_authority.rs"),
    Path("crates/iroha_core/src/kura/wsv_checkpoint_read_helpers.rs"),
    Path("crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs"),
    Path("crates/iroha_core/src/kura/prune_intent_publication.rs"),
    Path("crates/iroha_core/src/kura/prune_recovery_capacity.rs"),
    Path("crates/iroha_core/src/kura/block_store_definition_and_test_controls.rs"),
    Path("crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs"),
    Path("crates/iroha_core/src/kura/autonomous_terminal_capacity.rs"),
    Path("crates/iroha_core/src/kura/autonomous_publication_temp_recovery.rs"),
    Path(
        "crates/iroha_core/src/kura/"
        "historical_autonomous_recovery_temp_reconciliation.rs"
    ),
    Path("crates/iroha_core/src/kura/hot_path_capacity_preflight.rs"),
    Path("crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs"),
    Path("crates/iroha_core/src/kura/certified_bundle_capacity.rs"),
    Path("crates/iroha_core/src/kura/lane_artifact_budget.rs"),
    Path("crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs"),
    Path("crates/iroha_core/src/kura/autonomous_release_authority.rs"),
    Path("crates/iroha_core/src/kura/autonomous_retired_attempt.rs"),
    Path("crates/iroha_core/src/kura/autonomous_application_evidence.rs"),
    Path("crates/iroha_core/src/kura/indexed_sidecar_io.rs"),
    Path("crates/iroha_core/src/kura/indexed_sidecar_rewrite.rs"),
    Path("crates/iroha_core/src/kura/lane_history_compaction.rs"),
    Path("crates/iroha_core/src/kura/prune_block_store_tail.rs"),
    Path("crates/iroha_core/src/kura/test_fault_injection_state.rs"),
    Path("crates/iroha_core/src/kura/test_fault_injection_controls.rs"),
    Path("crates/iroha_core/src/kura/file_error_support.rs"),
)
REVIEWED_RUST_INCLUDE_MANIFESTS = {
    Path('crates/iroha_core/src/commit_roster_journal.rs'): (
        Path('commit_roster_journal/tests.rs'),
    ),
    Path('crates/iroha_config/src/parameters/actual.rs'): (
        Path('actual/torii_tx_history.rs'),
        Path('actual/torii_http_transport.rs'),
        Path('actual/torii_mcp_profile.rs'),
        Path('actual/tests.rs'),
    ),
    Path('crates/iroha_config/src/parameters/user.rs'): (
        Path('user/kura.rs'),
        Path('user/torii_peer_geo.rs'),
        Path('user/torii_soranet_privacy_ingest.rs'),
        Path('user/torii_tx_history.rs'),
        Path('user/sorafs_moderation_query_bound_tests.rs'),
        Path('user/governance_dag_head_mode_tests.rs'),
        Path('user/zk_prover_report_retention_tests.rs'),
        Path('user/query_fanout_memory_tests.rs'),
        Path('user/app_routed_read_body_timeout_tests.rs'),
        Path('user/operator_signature_body_timeout_tests.rs'),
        Path('user/verified_source_ingress_tests.rs'),
        Path('user/iso_bridge_store_memory_tests.rs'),
        Path('user/kura_and_snapshot_tests.rs'),
        Path('user/runtime_tail_tests.rs'),
    ),
    Path('crates/iroha_data_model/src/block/consensus_v2.rs'): (
        Path('consensus_v2_tests.rs'),
    ),
    Path('crates/iroha_core/src/kura.rs'): (
        Path('kura/startup_finality_support.rs'),
        Path('kura/bound_progress_and_retained_support.rs'),
        Path('kura/autonomous_reservation_bounds.rs'),
        Path('kura/certified_bundle_capacity_reservation_types.rs'),
        Path('kura/prune_commit_merge_support.rs'),
        Path('kura/merge_ledger_latest_execution_index.rs'),
        Path('kura/replica_advert_and_body_status.rs'),
        Path('kura/retained_finality_replica_authority.rs'),
        Path('kura/wsv_checkpoint_read_helpers.rs'),
        Path('kura/durable_block_and_atomic_sidecar_io.rs'),
        Path('kura/prune_intent_publication.rs'),
        Path('kura/prune_recovery_capacity.rs'),
        Path('kura/block_store_definition_and_test_controls.rs'),
        Path('kura/pipeline_and_lane_artifacts.rs'),
        Path('kura/autonomous_terminal_capacity.rs'),
        Path('kura/autonomous_publication_temp_recovery.rs'),
        Path('kura/historical_autonomous_recovery_temp_reconciliation.rs'),
        Path('kura/hot_path_capacity_preflight.rs'),
        Path('kura/autonomous_execution_view_capacity.rs'),
        Path('kura/certified_bundle_capacity.rs'),
        Path('kura/lane_artifact_budget.rs'),
        Path('kura/autonomous_lifecycle_terminal_outcomes.rs'),
        Path('kura/autonomous_release_authority.rs'),
        Path('kura/autonomous_retired_attempt.rs'),
        Path('kura/autonomous_application_evidence.rs'),
        Path('kura/indexed_sidecar_io.rs'),
        Path('kura/indexed_sidecar_rewrite.rs'),
        Path('kura/lane_history_compaction.rs'),
        Path('kura/prune_block_store_tail.rs'),
        Path('kura/test_fault_injection_state.rs'),
        Path('kura/test_fault_injection_controls.rs'),
        Path('kura/file_error_support.rs'),
        Path('kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs'),
        Path('kura/tests/01_prune_capacity_support.rs'),
        Path('kura/tests/01a_retained_eviction_and_rewrite_tail.rs'),
        Path('kura/tests/02_replacement_and_preflight.rs'),
        Path('kura/tests/02a_unauthenticated_preflight.rs'),
        Path('kura/tests/03_preflight_and_merge_entry.rs'),
        Path('kura/tests/03a_preflight_and_merge_entry_tail.rs'),
        Path('kura/tests/04_merge_log_and_associations.rs'),
        Path('kura/tests/04b_merge_artifact_budget.rs'),
        Path('kura/tests/04c_canonical_association_capacity.rs'),
        Path('kura/tests/04d_prune_intent_capacity.rs'),
        Path('kura/tests/05_merge_resolution_and_eviction.rs'),
        Path('kura/tests/05a_replica_advert_and_body_eviction.rs'),
        Path('kura/tests/06_eviction_and_autonomous_lanes.rs'),
        Path('kura/tests/07a_autonomous_reservation_reconciliation_support.rs'),
        Path('kura/tests/07_autonomous_lanes_and_sidecars.rs'),
        Path('kura/tests/07b_autonomous_reservation_reconciliation_tests.rs'),
        Path('kura/tests/07c_lane_execution_sidecar_tests.rs'),
        Path('kura/tests/07d_strict_lane_ownership_barrier_tests.rs'),
        Path('kura/tests/07e_autonomous_lifecycle_and_canonical_artifact_tests.rs'),
        Path('kura/tests/07e_autonomous_publication_temp_recovery_tests.rs'),
        Path('kura/tests/07e_terminal_capacity_hardening_tests.rs'),
        Path('kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs'),
        Path('kura/tests/07g_claim_capacity_preflight_tests.rs'),
        Path('kura/tests/07h_autonomous_execution_view_capacity_tests.rs'),
        Path('kura/tests/07i_historical_autonomous_batch_capacity_tests.rs'),
        Path('kura/tests/07j_certified_bundle_capacity_tests.rs'),
        Path('kura/tests/07k_historical_atomic_temp_recovery_tests.rs'),
        Path('kura/tests/07l_pending_canonical_capacity_tests.rs'),
        Path('kura/tests/08_lane_receipts_and_artifacts.rs'),
        Path('kura/tests/08a_certified_lane_block_read_tests.rs'),
        Path('kura/tests/08b_lane_history_compaction_capacity_tests.rs'),
        Path('kura/tests/09_lane_artifacts_and_fastpq.rs'),
        Path('kura/tests/10_native_amx_and_roster.rs'),
        Path('kura/tests/10b_native_amx_prepublication_transition.rs'),
        Path('kura/tests/11_roster_and_progress_sidecars.rs'),
        Path('kura/tests/12_sidecar_index_and_pruning.rs'),
        Path('kura/tests/13_manifests_and_fsync.rs'),
    ),
    Path('crates/iroha_core/src/kura/tests/10_native_amx_and_roster.rs'): (
        Path('10c_native_amx_latest_index_support_and_bounds.rs'),
    ),
    Path('crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs'): (
        Path('autonomous_merge_bundle_support.rs'),
        Path('autonomous_reservation_types.rs'),
        Path('autonomous_reservation_inventory.rs'),
        Path('autonomous_reservation_classifier.rs'),
        Path('historical_autonomous_recovery.rs'),
        Path('native_amx_participant_application_artifacts.rs'),
    ),
    Path('crates/iroha_core/src/kura/lane_geometry.rs'): (
        Path('lane_geometry/bootstrap_path_safety.rs'),
        Path('lane_geometry/bootstrap_relabel.rs'),
        Path('lane_geometry/catalog_validation.rs'),
        Path('lane_geometry/retirement_bounds.rs'),
        Path('lane_geometry_tests/00_support.rs'),
        Path('lane_geometry/native_amx_retained_window_tests.rs'),
        Path('lane_geometry_tests/00_retirement.rs'),
        Path('lane_geometry_tests/01_retirement_and_recovery.rs'),
        Path('lane_geometry_tests/02_geometry_moves_and_journal.rs'),
        Path('lane_geometry_tests/03_gc_and_startup.rs'),
    ),
    Path('crates/iroha_core/src/merge_sidecar.rs'): (
        Path('merge_sidecar_signing_guard_tests.rs'),
    ),
    Path('crates/iroha_core/src/queue.rs'): (
        Path('queue/canonical_terminal_cleanup.rs'),
        Path('queue/nexus_reconfigure_manifest_reload_tests.rs'),
        Path('queue/plan_journal_startup_atomicity_tests.rs'),
        Path('queue/global_guard_claim_conflict_tests.rs'),
        Path('queue/queue_metadata_and_admission_tests.rs'),
        Path('queue/instruction_and_state_routing_tests.rs'),
        Path('queue/routing_batch_admission_tests.rs'),
        Path('queue/teu_limit_and_backlog_tests.rs'),
        Path('queue/routing_projection_resilience_tests.rs'),
        Path('queue/capacity_and_concurrency_tests.rs'),
        Path('queue/pressure_resync_tests.rs'),
        Path('queue/expiry_tracking_tests.rs'),
        Path('queue/inflight_tracking_tests.rs'),
        Path('queue/lane_reservation_tests.rs'),
        Path('queue/lane_reservation_terminal_fault_tests.rs'),
        Path('queue/reservation_recovery_tests.rs'),
    ),
    Path('crates/iroha_core/src/queue/journal.rs'): (
        Path('journal_reservation_commit_preflight.rs'),
        Path('journal_direct_file_io.rs'),
        Path('plan_journal_bounds_tests.rs'),
        Path('plan_journal_replay_tests.rs'),
    ),
    Path('crates/iroha_core/src/state.rs'): (
        Path('state/vpn_lease_validation.rs'),
        Path('state/zk_asset_state.rs'),
        Path('state/passive_lane_diagnostic_methods.rs'),
        Path('state/diagnostic_state_generation.rs'),
        Path('state/autonomous_predecessor_application.rs'),
        Path('state/state_commit_lock_order_tests.rs'),
        Path('state/transfer_transcript_tests.rs'),
        Path('state/block_proof_tests.rs'),
        Path('state/range_bounds.rs'),
        Path('state/deserialize_core.rs'),
        Path('state/deserialize_world.rs'),
        Path('state/default_oracle.rs'),
    ),
    Path('crates/iroha_core/src/snapshot.rs'): (
        Path('snapshot/support_policy_tests.rs'),
        Path('snapshot/write_roundtrip_tests.rs'),
        Path('snapshot/reconciliation_generation_tests.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/evidence.rs'): (
        Path('evidence/missing_signer_pop_test.rs'),
        Path('evidence/signature_missing_test.rs'),
        Path('evidence/roundtrip_matrix_test.rs'),
    ),
    Path('crates/iroha_p2p/src/network.rs'): (
        Path('network/handle_update_tests.rs'),
        Path('network/queue_depth_tests.rs'),
    ),
    Path('crates/iroha_p2p/src/peer.rs'): (
        Path('peer_handshake_config_tests.rs'),
        Path('peer_state_tests.rs'),
        Path('peer_consensus_mode_test.rs'),
        Path('peer_tests.rs'),
    ),
    Path('crates/irohad/src/main.rs'): (
        Path('main/shared_sorafs_provider_cache_tests.rs'),
        Path('main/runtime_deps.rs'),
        Path('main/online_peers_provider.rs'),
        Path('main_tests/governance_dag_publisher_binding_signer.rs'),
        Path('main/governance_dag_launcher_tests.rs'),
        Path('main/runtime_budget_and_config_tests.rs'),
        Path('main/startup_tail_tests.rs'),
    ),
    Path('integration_tests/tests/taira_public_localnet.rs'): (
        Path('taira_public_localnet_config_digest_test.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/mod.rs'): (
        Path('fair_v2_ingress_selector.rs'),
        Path('tests/mod_authoritative_runtime_gate_01_support.rs'),
        Path('tests/mod_authoritative_runtime_gate_02_carrierless_replay.rs'),
        Path('tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs'),
        Path('tests/mod_authoritative_runtime_gate_04_routes_and_dequeue.rs'),
        Path('tests/mod_authoritative_runtime_gate_05_ownership_maintenance.rs'),
        Path('tests/mod_authoritative_runtime_gate_06_source_isolation.rs'),
        Path('tests/mod_authoritative_runtime_gate_07_wire_bounds.rs'),
        Path('tests/mod_authoritative_runtime_gate_08_capacity_and_control.rs'),
        Path('tests/mod_authoritative_runtime_gate_09_snapshot_and_source_lanes.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/status.rs'): (
        Path('status/test_guards.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2.rs'): (
        Path('tests/v2_adapter_main_00.rs'),
        Path('tests/v2_adapter_main_01.rs'),
        Path('tests/v2_adapter_main_02.rs'),
        Path('tests/v2_adapter_main_03.rs'),
        Path('tests/v2_adapter_main_04.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs'): (
        Path('tests/v2_lifecycle_coordinator_explorer_cases.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs'): (
        Path('v2_lifecycle_launch_tests.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs'): (
        Path('v2_lifecycle_ledger_operations.rs'),
        Path('v2_lifecycle_ledger_store.rs'),
        Path('v2_lifecycle_ledger_tests.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests.rs'): (
        Path('v2_lifecycle_ledger_tests_durable_recovery_01.rs'),
        Path('v2_lifecycle_ledger_tests_durable_recovery_02.rs'),
        Path('v2_lifecycle_ledger_tests_frame_and_store.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs'): (
        Path('tests/v2_lifecycle_projection_cases.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs'): (
        Path('v2_lifecycle_replay_authority_certified_serve.rs'),
        Path('v2_lifecycle_replay_authority_certified_body.rs'),
        Path('v2_lifecycle_replay_authority_payload_projection.rs'),
        Path('tests/v2_lifecycle_replay_authority_fixtures.rs'),
        Path('tests/v2_lifecycle_replay_authority_cases.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs'): (
        Path('v2_lifecycle_work_registry_validate_recovery_registry_impl.rs'),
        Path('v2_lifecycle_work_registry_validate_recovery_parent.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs'): (
        Path('v2_lifecycle_work_registry_recovered_wal.rs'),
        Path('v2_lifecycle_work_registry_validate_recovery.rs'),
        Path('v2_lifecycle_work_registry_validate_execution.rs'),
        Path('tests/v2_lifecycle_work_registry_00.rs'),
        Path('tests/v2_lifecycle_work_registry_01.rs'),
        Path('tests/v2_lifecycle_work_registry_02.rs'),
        Path('tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs'),
        Path('tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs'),
        Path('tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs'),
        Path('tests/v2_lifecycle_work_registry_exact_registry_cases.rs'),
        Path('tests/v2_lifecycle_work_registry_recovery_surface_cases.rs'),
        Path('tests/v2_lifecycle_work_registry_replay_evidence_cases.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_runtime.rs'): (
        Path('tests/v2_runtime_pending_binding_cases.rs'),
        Path('tests/v2_runtime_main_00.rs'),
        Path('tests/v2_runtime_main_01.rs'),
        Path('tests/v2_runtime_main_02.rs'),
        Path('tests/v2_runtime_main_03.rs'),
        Path('tests/v2_runtime_main_04.rs'),
        Path('tests/v2_runtime_main_05.rs'),
        Path('tests/v2_runtime_main_06.rs'),
        Path('tests/v2_runtime_unsealed_01b_lifecycle_bounds.rs'),
        Path('tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_worker.rs'): (
        Path('v2_worker/exact_output_rollover_claim.rs'),
        Path('v2_worker/autonomous_lane_output_retirement.rs'),
        Path('v2_worker/kura_replica_advert_refresh.rs'),
        Path('v2_worker/current_lane_output_rollover_claim.rs'),
        Path('tests/v2_worker_main_00.rs'),
        Path('tests/v2_worker_main_01.rs'),
        Path('tests/v2_worker_lifecycle_capacity_cases.rs'),
        Path('tests/v2_worker_equivocation_and_selected_serve_fixture.rs'),
        Path('v2_worker/applied_height_handoff_tests.rs'),
        Path('v2_worker/upstream_reply_route_test.rs'),
        Path('tests/v2_worker_main_02.rs'),
        Path('tests/v2_worker_main_03.rs'),
        Path('tests/v2_worker_main_04.rs'),
        Path('tests/v2_worker_main_05.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_runner.rs'): (
        Path('v2_runner/height_ingress_bindings.rs'),
        Path('v2_runner/lifecycle_terminal_recovery.rs'),
        Path('v2_runner/decided_lane_recovery.rs'),
        Path('v2_runner/outer_ingress_cursor.rs'),
        Path('v2_runner/finalized_output_rollover.rs'),
        Path('v2_runner/canonical_recovery_ingress.rs'),
        Path('v2_runner/reply_route_retention.rs'),
        Path('v2_runner/merge_sidecar_recovery.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_runner_tests.rs'): (
        Path('tests/v2_runner_unsealed_00.rs'),
        Path('tests/v2_runner_unsealed_01.rs'),
        Path('tests/v2_runner_unsealed_02.rs'),
        Path('tests/v2_runner_upstream_recovery.rs'),
        Path('tests/v2_runner_lifecycle_startup_order.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_apply.rs'): (
        Path('v2_apply/autonomous_recovery_types.rs'),
        Path('v2_apply/historical_autonomous_recovery.rs'),
        Path('v2_apply/reconciliation_authority.rs'),
        Path('v2_apply/committed_carrier_cleanup.rs'),
        Path('v2_apply/error_recovery.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_core/reducer.rs'): (
        Path('tests/reducer_timeout_and_projection.rs'),
        Path('tests/v2_core_reducer_primitive_projection.rs'),
        Path('reducer/counterfeit_boundary_capability_test.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_core/refinement.rs'): (
        Path('refinement/first_release_witness.rs'),
        Path('refinement_constructor_test_helpers.rs'),
        Path('refinement/transition_gate_tail.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs'): (
        Path('refinement_cases/effect_candidate.rs'),
        Path('refinement_cases/terminal_body_pipeline.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_core/tests.rs'): (
        Path('tests/committee_fallback_and_retransmit.rs'),
        Path('tests/v2_core_view_zero_parent_binding.rs'),
        Path('tests/empty_replay_resume_test.rs'),
        Path('tests/v2_core_terminal_transactionality.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_effects.rs'): (
        Path('tests/v2_effects_main_00.rs'),
        Path('tests/v2_effects_main_01.rs'),
        Path('tests/v2_effects_main_02.rs'),
        Path('tests/v2_effects_main_03.rs'),
        Path('tests/v2_effects_main_04.rs'),
        Path('tests/v2_effects_main_05.rs'),
        Path('tests/v2_effects_lifecycle_predecessor.rs'),
        Path('tests/v2_effects_03_locked_body_and_sidecar.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/v2_lane_work.rs'): (
        Path('v2_lane_work/canonical_executed_block_application_repair.rs'),
        Path('v2_lane_work/native_amx_signing_guard_capacity_boundary_test.rs'),
        Path('v2_lane_work/typed_finality_handoff_tests.rs'),
        Path('tests/v2_lane_work_native_signing_guard.rs'),
        Path('v2_lane_work/native_amx_route_and_receipt_tests.rs'),
        Path('tests/v2_lane_work_observer_role.rs'),
        Path('tests/v2_lane_work_native_body_recovery.rs'),
        Path('tests/v2_lane_work_lifecycle_and_recovery_cases.rs'),
        Path('v2_lane_work/historical_recovery_and_carrier_tests.rs'),
        Path('v2_lane_work_autonomous_ready_durability_tests.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_lane_work_lifecycle_and_recovery_cases.rs'): (
        Path('v2_lane_work_effect_queue.rs'),
    ),
    Path('integration_tests/tests/sumeragi_v2_runner.rs'): (
        Path('sumeragi_v2_runner/restart_timing_test.rs'),
        Path('sumeragi_v2_runner/status_validation_helpers.rs'),
    ),
    Path('crates/iroha_sumeragi_core/src/verus_proofs.rs'): (
        Path('verus_proofs/production_transition_contracts.rs'),
        Path('verus_proofs/in_flight_first_release_proofs.rs'),
        Path('verus_proofs/production_kernel_tail.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_adapter_main_00.rs'): (
        Path('v2_adapter_activation_context.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_adapter_main_03.rs'): (
        Path('v2_adapter_04_wal_recovery.rs'),
        Path('v2_adapter_04b_lifecycle_startup.rs'),
        Path('v2_adapter_05_direct_lifecycle.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_adapter_main_04.rs'): (
        Path('v2_adapter_01_replay_and_registry.rs'),
        Path('v2_adapter_02_view_and_lock_progress.rs'),
        Path('v2_adapter_03_tc_and_terminal_ingress.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_worker_main_01.rs'): (
        Path('v2_worker_reply_route_cases.rs'),
        Path('v2_worker_backpressure_cases.rs'),
        Path('v2_worker_recovered_lifecycle_output_cases.rs'),
        Path('v2_worker_nonzero_view_restart.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_worker_main_04.rs'): (
        Path('v2_worker_serve_unsealed_cases.rs'),
        Path('v2_worker_serve_decision_restart_cases.rs'),
        Path('v2_worker_certified_serve_budget_cases.rs'),
    ),
    Path('crates/iroha_core/src/sumeragi/tests/v2_effects_main_05.rs'): (
        Path('v2_effects_kura_tip_replay.rs'),
        Path('v2_effects_01_view_churn_and_runtime_steps.rs'),
        Path('v2_effects_02_admission_handoffs.rs'),
    ),
}

REVIEWED_RUST_INCLUDE_MANIFEST_COMPANIONS = {
    Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"): (
        Path(
            "crates/iroha_core/src/sumeragi/tests/"
            "v2_lane_work_lifecycle_and_recovery_cases.rs"
        ),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs"): (
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs"),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs"): (
        Path(
            "crates/iroha_core/src/sumeragi/"
            "v2_lifecycle_work_registry_validate_recovery.rs"
        ),
    ),
}
REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS = tuple(
    companion
    for companions in REVIEWED_RUST_INCLUDE_MANIFEST_COMPANIONS.values()
    for companion in companions
)
REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS = tuple(
    parent
    for parent in REVIEWED_RUST_INCLUDE_MANIFESTS
    if parent not in REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS
)
assert len(REVIEWED_RUST_INCLUDE_MANIFESTS) == 49
assert len(REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS) == 5
assert len(set(REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS)) == 5
assert len(REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS) == 44
assert set(REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS).isdisjoint(
    REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS
)
assert (
    set(REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS)
    | set(REVIEWED_RUST_INCLUDE_MANIFEST_NESTED_PARENTS)
    == set(REVIEWED_RUST_INCLUDE_MANIFESTS)
)
assert set(REVIEWED_RUST_INCLUDE_MANIFEST_COMPANIONS).issubset(
    REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS
)


def copy_merge_runtime_config_fixture(tmp_path: Path) -> Path:
    """Copy only the config-v6 merge/pending projection and its live consumers."""

    for relative in (
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
        Path("crates/iroha_core/src/merge_sidecar.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    kura = tmp_path / "crates/iroha_core/src/kura.rs"
    kura_production_includes = "\n".join(
        f'include!("{relative.relative_to("crates/iroha_core/src").as_posix()}");'
        for relative in KURA_PRODUCTION_COMPONENT_FILES
    )
    kura.write_text(
        """
pub fn new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits() {
    let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(
        sumeragi_limits,
        &config.store_dir.resolve_relative_path(),
    )?;
}

fn pending_merge_entry_paths_unlocked() {
    if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
        return Err(Self::invalid_pending_merge_entry_error(
            directory,
            "pending certified merge entry count exceeds the hard limit",
        ));
    }
}

fn pending_queue_plan_admission_paths_unlocked() {
    if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            directory,
            "pending QueuePlan admission certificate count exceeds the hard limit",
        ));
    }
}

fn validate_pending_merge_entries_on_startup() {
    if !self
        .pending_control_sidecar_limits
        .combined_bytes_within_limit(merge_bytes, admission_bytes)
    {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            self.store_root.clone(),
            "pending merge and QueuePlan admission sidecars exceed their shared hard byte limit",
        ));
    }
}

pub(crate) fn persist_pending_certified_merge_entry() {
    if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
        return Err(Self::invalid_pending_merge_entry_error(
            directory,
            "pending certified merge entry count exceeds the hard limit",
        ));
    }
    if pending_bytes.checked_add(bytes.len()).is_none_or(|total| {
        !self
            .pending_control_sidecar_limits
            .combined_bytes_within_limit(total, admission_bytes)
    }) {
        return Err(error);
    }
}

pub fn persist_pending_queue_plan_admission_certificate() {
    if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            directory,
            "pending QueuePlan admission certificate count exceeds the hard limit",
        ));
    }
    if admission_bytes
        .checked_add(canonical_certificate_bytes.len())
        .is_none_or(|total| {
            !self
                .pending_control_sidecar_limits
                .combined_bytes_within_limit(merge_bytes, total)
        })
    {
        return Err(error);
    }
}

__KURA_PRODUCTION_INCLUDES__

#[cfg(test)]
pub(crate) mod tests {}
""".replace("__KURA_PRODUCTION_INCLUDES__", kura_production_includes),
        encoding="utf-8",
    )
    for relative in KURA_PRODUCTION_COMPONENT_FILES:
        component = tmp_path / relative
        component.parent.mkdir(parents=True, exist_ok=True)
        component.write_text(
            f"// isolated {component.name} fixture\n",
            encoding="utf-8",
        )
    daemon = tmp_path / "crates/irohad/src/main.rs"
    daemon.parent.mkdir(parents=True, exist_ok=True)
    daemon.write_text(
        """
fn production_startup() {
    Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
        &config.snapshot.bootstrap,
        &config.sumeragi.limits,
    );
}
""",
        encoding="utf-8",
    )
    return tmp_path


def merge_runtime_config_errors(repo_root: Path) -> list[str]:
    """Run one mutation check in a fresh process so large Rust tokens are released."""

    probe = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import importlib.util
import json
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("merge_runtime_checker", sys.argv[1])
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
print(json.dumps(module._merge_runtime_config_production_source_fidelity_errors(
    Path(sys.argv[2])
)))
""",
            str(SCRIPT),
            str(repo_root),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert probe.returncode == 0, probe.stderr
    errors = json.loads(probe.stdout)
    assert isinstance(errors, list) and all(isinstance(error, str) for error in errors)
    return errors


def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"


def test_revision4_model_contract_is_registered() -> None:
    module = load_checker()

    assert "SumeragiV2Revision4" in module.REQUIRED_MODEL_MODULES
    assert (
        "SumeragiV2Revision4AdversarialSafety"
        in module.REQUIRED_MODEL_MODULES
    )
    assert "SumeragiV2Revision4.cfg" in module.REQUIRED_TLC_CONFIGS
    assert (
        "SumeragiV2Revision4AdversarialSafety.cfg"
        in module.REQUIRED_TLC_CONFIGS
    )
    assert "SumeragiV2Revision4Liveness.cfg" in module.REQUIRED_TLC_CONFIGS
    assert not module._revision4_model_contract_errors(module.FORMAL_DIR)
    assert not module._revision4_adversarial_safety_contract_errors(
        module.FORMAL_DIR
    )


def copy_revision4_adversarial_contract(tmp_path: Path, module) -> Path:
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2Revision4AdversarialSafety.tla",
        "SumeragiV2Revision4AdversarialSafety.cfg",
    ):
        shutil.copy2(module.FORMAL_DIR / filename, formal_dir / filename)
    return formal_dir


def test_revision4_adversarial_model_rejects_first_qc_global_stop(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_revision4_adversarial_contract(tmp_path, module)
    model = formal_dir / "SumeragiV2Revision4AdversarialSafety.tla"
    source = model.read_text(encoding="utf-8")
    source = source.replace(
        "    /\\ body \\notin commitQCs\n",
        "    /\\ body \\notin commitQCs\n"
        "    /\\ commitQCs = {}\n",
        1,
    )
    model.write_text(source, encoding="utf-8")

    errors = module._revision4_adversarial_safety_contract_errors(formal_dir)
    assert any(
        "FormCommitQC must remain enabled after the first QC or decision"
        in error
        for error in errors
    ), errors


def test_revision4_adversarial_model_rejects_byzantine_sign_once_guard(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_revision4_adversarial_contract(tmp_path, module)
    model = formal_dir / "SumeragiV2Revision4AdversarialSafety.tla"
    source = model.read_text(encoding="utf-8")
    source = source.replace(
        "    /\\ <<validator, body>> \\notin commitVotes\n",
        "    /\\ <<validator, body>> \\notin commitVotes\n"
        "    /\\ VoteBodies(validator) = {}\n",
        1,
    )
    model.write_text(source, encoding="utf-8")

    errors = module._revision4_adversarial_safety_contract_errors(formal_dir)
    assert any(
        "must permit the faulty validator to vote for both bodies" in error
        for error in errors
    ), errors


def test_revision4_model_contract_rejects_output_repair_as_progress_fairness(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2Revision4.tla",
        "SumeragiV2Revision4.cfg",
        "SumeragiV2Revision4Liveness.cfg",
    ):
        shutil.copy2(module.FORMAL_DIR / filename, formal_dir / filename)
    model = formal_dir / "SumeragiV2Revision4.tla"
    source = model.read_text(encoding="utf-8")
    source = source.replace(
        "    /\\ WF_vars(ActivateSuccessor)\n",
        "    /\\ WF_vars(ActivateSuccessor)\n"
        "    /\\ WF_vars(RepairFinalizedOutput)\n",
        1,
    )
    model.write_text(source, encoding="utf-8")

    errors = module._revision4_model_contract_errors(formal_dir)
    assert any(
        "finalized-output repair may not be" in error for error in errors
    ), errors


def copy_audited_rank_leaf_contract_fixture(tmp_path: Path, module) -> Path:
    """Install the reviewed Stage-4/5 contracts around the current proof source."""

    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    vocabulary_source = vocabulary.read_text(encoding="utf-8")
    property_block = r'''
ProtectedStage4RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<4, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<4, position>>))

ProtectedStage5RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<5, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<5, position>>))
'''
    if "ProtectedStage4RankProgressProperty" not in vocabulary_source:
        vocabulary_source = vocabulary_source.replace(
            "=============================================================================\n",
            property_block + "\n=============================================================================\n",
            1,
        )
        vocabulary.write_text(vocabulary_source, encoding="utf-8")

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof.read_text(encoding="utf-8")
    wrapper_block = r'''
THEOREM ProtectedStage4RankProgressFromFairScheduler ==
  \A initialContext:
    ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage4RankDescent
   DEF ProtectedStage4RankProgressProperty

THEOREM ProtectedStage5RankProgressFromFairFifo ==
  \A initialContext:
    ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage5RankDescent
   DEF ProtectedStage5RankProgressProperty
'''
    if "ProtectedStage4RankProgressFromFairScheduler" not in proof_source:
        proof_source = proof_source.replace(
            "=============================================================================\n",
            wrapper_block + "\n=============================================================================\n",
            1,
        )
        proof.write_text(proof_source, encoding="utf-8")
    return formal_dir


def audited_rank_leaf_contract_errors(module, formal_dir: Path) -> list[str]:
    """Run both source and ledger-target guards for the audited rank leaves."""

    proof_source = (
        formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    ).read_text(encoding="utf-8")
    errors = module._async_proof_architecture_errors(formal_dir)
    errors.extend(
        module._proof_obligation_architecture_errors(
            module.load_ledger()["obligations"],
            {"SumeragiV2AsyncLivenessProofs": proof_source},
        )
    )
    return errors


def test_audited_rank_leaf_synthetic_contract_is_green(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)

    assert audited_rank_leaf_contract_errors(module, formal_dir) == []


@pytest.mark.parametrize(
    ("filename", "kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => <>AsyncProgressOwnershipInvariant",
            "AsyncSpecAlwaysProgressOwnershipInvariant must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncBracketNextPreservesProgressOwnership",
            "AsyncBracketNextPreservesStrongTypeInvariant",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage4RankProgressProperty",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "ProtectedStage4RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage4RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage4RankProgressFromFairScheduler must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "BY FairProtectedStage4RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage5RankProgressProperty",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "ProtectedStage5RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage5RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage5RankProgressFromFairFifo must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "BY FairProtectedStage5RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
    ),
)
def test_audited_rank_leaf_source_mutations_fail_closed(
    tmp_path: Path,
    filename: str,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)
    path = formal_dir / filename
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = audited_rank_leaf_contract_errors(module, formal_dir)
    assert any(
        expected_error in error and symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ProtectedServeStage5CarrierFacts",
            "ServeOccurrenceIndexCharacterization",
        ),
        (
            "ProtectedServeStage5EnablesFairWorker",
            "QueuedIoEnablesPostGstService",
        ),
        (
            "ProtectedServeStage5WorkerStrictlyProgresses",
            "TailRemovesUniqueServeOccurrence",
        ),
        (
            "ProtectedServeStage5UnlessProgress",
            "AsyncBracketNextPreservesStrongTypeInvariant",
        ),
        (
            "FairProtectedServeStage5RankDescent",
            "ProtectedServeStage5EnablesFairWorker",
        ),
        (
            "ProtectedServeRankProgressFromFairFifo",
            "FairProtectedServeStage5RankDescent",
        ),
    ),
)
def test_protected_serve_fifo_proof_dependency_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof.write_text(
        delete_tla_theorem_token(
            proof.read_text(encoding="utf-8"),
            symbol,
            token,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error
        and "omits explicit transition/fairness inventory" in error
        and token in error
        for error in errors
    ), errors


def test_serve_occurrence_rank_and_starvation_conjunct_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ServeJobRank(node, job) == <<5, ServeJobIndex(node, job)>>",
            "ServeJobRank(node, job) == <<5, CandidateIoIndex("
            "job.candidate, asyncIoQueues[node])>>",
            1,
        ).replace(
            "     \\/ ProtectedServeRankDecreaseStep\n",
            "",
            1,
        ).replace(
            "  /\\ ProtectedServeStarvationProperty(specification)\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ServeJobRank must equal only" in error for error in errors)
    assert any("PostGstProductiveStep must equal only" in error for error in errors)
    assert any("StarvationFreedomProperty must equal only" in error for error in errors)


def test_exact_removal_and_protected_slot_geometry_theorems_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    removal = source.index("THEOREM OneRemovalIncreasesSourceProtectionByAtMostOne")
    universe = source.index("THEOREM ProtectedProgressSlotUniverseSize")
    mutated = (
        source[:removal]
        + source[removal:universe].replace(
            "LET after == SequenceWithoutIndex(before, selected)",
            "LET after == Tail(before)",
            1,
        )
        + source[universe:].replace(
            "Cardinality(ProtectedProgressSlotUniverse) = 2 * N + 3",
            "Cardinality(ProtectedProgressSlotUniverse) = N + 3",
            1,
        )
    )
    proofs.write_text(mutated, encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "OneRemovalIncreasesSourceProtectionByAtMostOne must state only" in error
        for error in errors
    )
    assert any(
        "ProtectedProgressSlotUniverseSize must state only" in error
        for error in errors
    )


def test_normal_proposal_prepare_protection_contract_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "     \\/ NormalProposalPrepareCandidate(candidate)\n", "", 1
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_kind_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"Proposal", "PrepareVote", "CommitVote"}',
            '{"Proposal", "PrepareVote"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkKinds must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_requires_canonical_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ candidate \\in AsyncCandidateSet\n",
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ AsyncCandidateTyped(candidate)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_delivery_class_is_frozen_at_admission(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    frozen_network = (
        "    /\\ candidate = FrozenNormalDeliveryCandidate(\n"
        "                     item, consumerContext, consumerView,\n"
        "                     consumerGeneration)\n"
    )
    assert frozen_network in source
    vocabulary.write_text(
        source.replace(
            frozen_network,
            "    /\\ candidate = NormalDeliveryCandidate(item)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkCandidate must equal only" in error
        for error in errors
    )

    frozen_identity = (
        "       consumerContext, consumerView, consumerGeneration, item,\n"
    )
    assert frozen_identity in source
    vocabulary.write_text(
        source.replace(
            frozen_identity,
            "       context, nodeView[item.envelope.recipient],\n"
            "       generation[item.envelope.recipient], item,\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenNormalDeliveryCandidate must equal only" in error
        for error in errors
    )


def test_normal_install_successor_is_required_and_frozen(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    install_successor_branch = (
        "     \\/ \\E command \\in AsyncCandidateSet,\n"
        "            installedContext \\in ContextRecords,\n"
        "            priorGeneration \\in Generations,\n"
        "            subject \\in SubjectOrNone:\n"
        "          /\\ command.kind = \"PersistInstallTC\"\n"
        "          /\\ command.view + 1 \\in Views\n"
        "          /\\ candidate = FrozenInstallProposalSuccessor(\n"
        "                           command, installedContext,\n"
        "                           priorGeneration, subject)\n"
    )
    assert install_successor_branch in source
    vocabulary.write_text(
        source.replace(install_successor_branch, "", 1),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNoItemCandidate must equal only" in error
        for error in errors
    )

    frozen_generation = "NextCandidateGeneration(priorGeneration)"
    assert frozen_generation in source
    vocabulary.write_text(
        source.replace(
            frozen_generation,
            "generation[command.node]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenInstallProposalSuccessor must equal only" in error
        for error in errors
    )


def test_begin_prepare_parent_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"DeliverProposal", "ValidateBody"}',
            '{"DeliverProposal"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalBeginPrepareParentKinds must equal only" in error
        for error in errors
    )


def test_normal_candidate_step_stability_theorem_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    proofs.write_text(
        source.replace(
            "    /\\ AsyncNext\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            "    /\\ PostGstSchedulerActionEnabled\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncNextPreservesNormalProposalPrepareCandidate must state only"
        in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_enablement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    property_offset = source.index("DeadlockFreedomProperty(specification) ==")
    enabled_offset = source.index(
        "PostGstProductiveActionEnabled", property_offset
    )
    vocabulary.write_text(
        source[:enabled_offset]
        + source[enabled_offset:].replace(
            "PostGstProductiveActionEnabled",
            "PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "DeadlockFreedomProperty must equal only" in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_productive_alias(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep",
            "PostGstProductiveActionEnabled == PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "PostGstProductiveActionEnabled must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_dual_progress_ingress_geometry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",\n',
            '   "TimeoutCertificate", "CommitCertificateRequest",\n',
            1,
        ).replace(
            "    + Cardinality(\n"
            "        IngressTransportCompletionProtectedSourcesFor(lanes, recipient))\n",
            "",
            1,
        ).replace(
            'IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}',
            'IngressTransportCompletionKinds == {"Chunk"}',
            1,
        ).replace(
            "  \\/ ~IngressLaneHasTransportCompletionIn(\n"
            "       asyncIngressLanes, item.envelope.recipient, item.source)\n",
            "  \\/ TRUE\n",
            1,
        ).replace(
            '                    "TimeoutCertificate", "Chunk", "CertifiedResponse",\n'
            '                    "CommitCertificateResponse",\n',
            '                    "TimeoutCertificate", "Chunk", "CertifiedRequest",\n'
            '                    "CertifiedResponse", "CommitCertificateRequest",\n'
            '                    "CommitCertificateResponse",\n',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "IngressTransportCompletionKinds must equal only" in error
        for error in errors
    )
    assert any("IngressProgressKinds must equal only" in error for error in errors)
    assert any(
        "IngressProtectedSlotCountFor must equal only" in error for error in errors
    )
    assert any(
        "AsyncTransportCompletionOwnerGateAllows must equal only" in error
        for error in errors
    )
    assert any("DeliveryClass must equal only" in error for error in errors)


def test_async_source_fidelity_pins_untrusted_transport_completion_exclusion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in AsyncIngressSources,\n",
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in ValidatorIds,\n",
            1,
        )
        .replace(
            '  /\\ (item.kind \\notin {"Noise", "Chunk", "CertifiedResponse"}\n'
            "        => item.source \\in ValidatorIds)",
            '  /\\ (item.kind # "Noise" => item.source \\in ValidatorIds)',
            1,
        )
        .replace(
            "  IN /\\ kind \\in IngressTransportCompletionKinds\n",
            '  IN /\\ kind = "Chunk"\n',
            1,
        )
        .replace("     /\\ nonce = 0\n", "", 1)
        .replace(
            "       InjectUntrustedTransportCompletion(kind, recipient, nonce)\n",
            "       TRUE\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncNetworkItems omits required production behavior" in error
        for error in errors
    )
    assert any(
        "AsyncItemTyped omits required production behavior" in error
        for error in errors
    )
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )
    assert any(
        "AsyncFaultStep omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        source.replace("     /\\ nonce = 0\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )


def test_async_source_fidelity_pins_timeout_signer_partition_without_displacement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            "AsyncDeferredProgressCapacity >= 2 * N + 3",
            "AsyncDeferredProgressCapacity >= N + 3",
            1,
        ).replace(
            '    [] command.kind = "DeliverTimeout" ->\n'
            '         command.item.kind = "TimeoutVote"\n',
            "",
            1,
        ).replace(
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN queue\n",
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN SequenceWithoutIndex(queue, 1)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncConfiguration omits required production behavior" in error
        for error in errors
    )
    assert any("ProtectedProgressCommand must equal only" in error for error in errors)
    assert any("DeferredProgressAfter must equal only" in error for error in errors)


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "       /\\ candidate.kind \\in AsyncTimeoutLifecycleKinds\n",
            "       /\\ candidate.causalOrigin.phase "
            "\\in AsyncTimeoutLifecycleKinds\n",
        ),
        (
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates \\cup TrackedWorkCandidates:\n",
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates:\n",
        ),
    ),
)
def test_async_source_fidelity_pins_current_timeout_lifecycle_stage_classifier(
    tmp_path: Path, old: str, new: str
) -> None:
    """Retained timeout origins must not turn proposal successors into clocks."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    assert old in source
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncOlderOrEqualTimeoutLifecycleOwned must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_live_serve_occurrence_identity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            'AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))',
            'AsyncIoJob("Serve", candidate, 0)',
            1,
        ).replace(
            "    /\\ AsyncIoServeNonceOwnership(asyncIoQueues[node])\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncIoCertifiedServeJob must equal only" in error for error in errors)
    assert any(
        "AsyncIoQueueContentTypeInvariant must equal only" in error
        for error in errors
    )


def reviewed_rust_item_provider(
    module, repo_root: Path, relative: Path, item_name: str
) -> Path:
    """Resolve one item through its authenticated parent/include closure."""

    parent = repo_root / relative
    candidates = (parent,) + tuple(
        parent.parent / component
        for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(
            relative.as_posix(), ()
        )
    )
    providers = tuple(
        path for path in candidates
        if path.is_file() and module.rust_items(path.read_text(encoding="utf-8"), item_name)
    )
    assert len(providers) == 1, (relative, item_name, providers)
    return providers[0]


def copy_timeout_vote_window_fixture(tmp_path: Path, module) -> Path:
    """Copy the bounded TimeoutVote production and regression sources."""

    relatives = (
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"),
    )
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(module.ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)
    return tmp_path / relatives[0]


def test_async_source_fidelity_pins_timeout_vote_semantic_capacity_bypass(
    tmp_path: Path,
) -> None:
    """The bounded TimeoutVote production and regression sources are sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert errors == []


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "admit_authenticated_payload",
            (
                "if !reducer::timeout_vote_view_is_admissible("
                "current_view, vote.round.view)"
            ),
            "if false",
            "current/adjacent view window",
        ),
        (
            "admit_authenticated_payload",
            (
                "locked_commit_progress || matches!(key, "
                "IngressSemanticKey::TimeoutVote { .. })"
            ),
            "locked_commit_progress",
            "bypass only ordinary semantic capacity",
        ),
        (
            "prune_ingress_records",
            (
                "round.height == current_height\n"
                "                        "
                "&& reducer::timeout_vote_view_is_admissible("
                "current_view, round.view)"
            ),
            "round.height == current_height",
            "retained only at the current height and current/adjacent view",
        ),
        (
            "prune_ingress_records",
            (
                "matches_current_lock(*key, record.fingerprint) "
                "|| matches_retained_timeout(*key)"
            ),
            "matches_current_lock(*key, record.fingerprint)",
            "preserve either the exact lock or retained TimeoutVote",
        ),
    ),
)
def test_timeout_vote_semantic_capacity_rejects_real_source_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Bounded admission and both protected prune arms fail closed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    rust_path = reviewed_rust_item_provider(
        module, tmp_path, Path("crates/iroha_core/src/sumeragi/v2.rs"), item_name
    )
    mutate_rust_item_source_in_context(
        module,
        rust_path,
        item_name,
        (("impl", "SumeragiV2Adapter"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


def test_timeout_vote_semantic_capacity_rejects_two_roster_sets(
    tmp_path: Path,
) -> None:
    """The semantic table reserves lock plus both bounded timeout rounds."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    rust_path = reviewed_rust_item_provider(
        module, tmp_path, Path("crates/iroha_core/src/sumeragi/v2.rs"),
        "semantic_ingress_capacity",
    )
    mutate_rust_item_source(
        module,
        rust_path,
        "semantic_ingress_capacity",
        "roster_len.saturating_mul(3)",
        "roster_len.saturating_mul(2)",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        "three roster-bounded protected sets" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 1",
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 2",
            "lookahead must remain exactly one view",
        ),
        (
            "current_view.saturating_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "current_view.wrapping_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "lower bound and saturating one-view upper bound",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_predicate_mutations(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """The one-round helper cannot widen, wrap, or lose its exact bound."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    types_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/types.rs"
    mutate_source_once(types_path, old, new)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "on_timeout_vote",
            (
                "if !timeout_vote_view_is_admissible("
                "self.durable.current_view(), vote.round().view())"
            ),
            "if false",
            "admission must use the bounded current/adjacent predicate",
        ),
        (
            "on_persisted",
            "self.timeout_votes.retain(|round, _| {",
            "self.timeout_votes.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
        (
            "on_persisted",
            "self.formed_timeouts.retain(|round| {",
            "self.formed_timeouts.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_reducer_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Reducer admission and both install-retention pools stay bounded."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    reducer_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
    mutate_rust_item_source_in_context(
        module,
        reducer_path,
        item_name,
        (("impl", "Reducer"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "adjacent_future_timeout_votes_form_a_catch_up_certificate",
        "timeout_install_preserves_adjacent_shares_for_the_new_current_view",
        "timeout_votes_beyond_adjacent_lookahead_are_ignored",
    ),
)
def test_timeout_vote_view_window_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Catch-up, install preservation, and far-future rejection stay sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    tests_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/tests.rs"
    mutate_rust_item_source(
        module,
        tests_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "capacity_bypass_records_follow_current_lock_and_timeout_view",
        "certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote",
        "full_normal_deferred_lane_cannot_drop_absolute_timeout",
        "busy_deferred_source_identity_coalesces_across_consumer_view_change",
    ),
)
def test_timeout_vote_semantic_capacity_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Capacity, adjacent, full-lane, and cross-view regressions stay exact."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    rust_path = reviewed_rust_item_provider(
        module, tmp_path, Path("crates/iroha_core/src/sumeragi/v2.rs"), test_name
    )
    mutate_rust_item_source(
        module,
        rust_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    (
        "relative",
        "test_name",
        "old",
        "new",
        "digest_map_name",
        "expected_error",
    ),
    (
        (
            "crates/iroha_core/src/sumeragi/tests/"
            "v2_adapter_02_view_and_lock_progress.rs",
            "capacity_bypass_records_follow_current_lock_and_timeout_view",
            "roster_len * 3",
            "roster_len * 2",
            "_TIMEOUT_VOTE_SEMANTIC_CAPACITY_REGRESSION_TEST_SHA256",
            "exactly one lock plus current and adjacent TimeoutVote rosters",
        ),
        (
            "crates/iroha_core/src/sumeragi/tests/"
            "v2_adapter_03_tc_and_terminal_ingress.rs",
            "certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote",
            "assert_eq!(adapter.current_tag().view(), current_round.view + 1);",
            "assert_eq!(adapter.current_tag().view(), current_round.view);",
            "_TIMEOUT_VOTE_SEMANTIC_CAPACITY_REGRESSION_TEST_SHA256",
            "advance the hung signer exactly one view",
        ),
        (
            "crates/iroha_core/src/sumeragi/tests/"
            "v2_adapter_03_tc_and_terminal_ingress.rs",
            "busy_deferred_source_identity_coalesces_across_consumer_view_change",
            "assert_eq!(retagged_candidate.0, original_candidate.0);",
            "assert_ne!(retagged_candidate.0, original_candidate.0);",
            "_SERVICED_CANDIDATE_REGRESSION_TEST_SHA256",
            "retain source identity while advancing only its consumer episode",
        ),
    ),
)
def test_externalized_timeout_capacity_semantics_survive_digest_refresh(
    tmp_path: Path,
    relative: str,
    test_name: str,
    old: str,
    new: str,
    digest_map_name: str,
    expected_error: str,
) -> None:
    """Externalized adapter regressions remain semantic, not digest-only, seals."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    rust_path = tmp_path / relative
    mutate_rust_item_source(module, rust_path, test_name, old, new)
    item = module.rust_items(
        rust_path.read_text(encoding="utf-8"), test_name
    )[0]
    getattr(module, digest_map_name)[test_name] = module._rust_item_token_sha256(
        item
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


def test_recovered_lifecycle_proposal_owner_semantics_survive_digest_refresh(
    tmp_path: Path,
) -> None:
    """The opaque recovered-attempt regression remains a semantic seal."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2Core.tla",
        "SumeragiV2InductiveProofs.tla",
    )
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_candidate.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    replay_path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/v2_runner_unsealed_02.rs"
    )
    test_name = (
        "recovered_lifecycle_proposal_attempt_binds_only_the_exact_current_lock_owner"
    )
    mutate_rust_item_source(
        module,
        replay_path,
        test_name,
        'let foreign_lock = directive(Some(proposal_subject(b"foreign replay lock")), None);',
        "let foreign_lock = directive(Some(subject), None);",
    )
    item = module.rust_items(
        replay_path.read_text(encoding="utf-8"), test_name
    )[0]
    module._LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[test_name] = (
        module._rust_item_token_sha256(item)
    )

    errors = module._locked_body_reproposal_source_fidelity_errors(
        formal_dir, tmp_path
    )

    assert any(
        "the recovered-attempt regression must prove exact, affine runner binding and reject foreign locks, rounds, and decisions"
        in error
        for error in errors
    ), errors


def test_frontier_preflight_semantics_survive_item_digest_refresh(
    tmp_path: Path,
) -> None:
    """Refreshing the preflight seal cannot move EnterView away from the head."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    effects_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    item_name = "preflight_effect_batch_frontier"
    mutate_rust_item_source(
        module,
        effects_path,
        item_name,
        "!matches!(effects.first(), Some(AdapterEffect::EnterView { .. }))",
        "!matches!(effects.last(), Some(AdapterEffect::EnterView { .. }))",
    )
    item = module.rust_items(
        effects_path.read_text(encoding="utf-8"), item_name
    )[0]
    module._PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256[item_name] = (
        module._rust_item_token_sha256(item)
    )

    errors = module._effect_capacity_production_source_fidelity_errors(tmp_path)

    assert any(
        "frontier preflight must require the unique EnterView at the batch head"
        in error
        for error in errors
    ), errors


MERGE_RUNTIME_PROJECTED_FIELDS = (
    "merge_sidecar_inbound_session_capacity",
    "merge_sidecar_inbound_sessions_per_peer",
    "merge_sidecar_inbound_assembly_bytes",
    "merge_sidecar_inbound_assembly_bytes_per_peer",
    "merge_sidecar_deferred_block_capacity",
    "merge_sidecar_future_block_distance",
    "merge_sidecar_request_timeout_ms",
    "merge_sidecar_outbound_sessions_per_source",
    "merge_sidecar_outbound_bytes_per_source",
    "merge_sidecar_server_request_gates_per_source",
    "pending_certified_merge_entry_capacity",
    "pending_queue_plan_admission_capacity",
    "pending_control_sidecar_bytes",
    "merge_signing_guard_record_capacity",
    "merge_signing_guard_record_bytes",
    "merge_signing_guard_total_bytes",
)


def test_merge_runtime_config_v6_inventory_is_static_and_current() -> None:
    module = load_checker()
    checker_source = "\n".join(
        path.read_text(encoding="utf-8") for path in checker_source_paths()
    )

    assert tuple(
        Path("crates/iroha_core/src") / relative
        for relative in module._KURA_PRODUCTION_COMPONENT_FILES
    ) == KURA_PRODUCTION_COMPONENT_FILES
    assert tuple(
        projected_field
        for projected_field, *_rest in module.MERGE_RUNTIME_CONFIG_FIELDS
    ) == MERGE_RUNTIME_PROJECTED_FIELDS
    assert len(module.MERGE_RUNTIME_CONFIG_FIELDS) == 16
    assert (
        checker_source.count(
            '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;"'
        )
        == 2
    )
    assert (
        '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 3;"'
        not in checker_source
    )


def test_merge_runtime_config_v6_source_binding_accepts_repository() -> None:
    module = load_checker()

    assert module._merge_runtime_config_production_source_fidelity_errors() == []


def test_reviewed_rust_include_manifests_are_static_and_current() -> None:
    module = load_checker()
    observed = {
        Path(parent): tuple(Path(component) for component in components)
        for parent, components in module._REVIEWED_RUST_INCLUDE_MANIFESTS.items()
    }
    assert observed == REVIEWED_RUST_INCLUDE_MANIFESTS
    assert module._reviewed_rust_include_manifest_errors() == []


def test_reviewed_rust_include_manifest_rejects_ignored_untracked_component(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    parent = repo_root / "parent.rs"
    component = repo_root / "test_ignored_component.rs"
    parent.write_text('include!("test_ignored_component.rs");\n', encoding="utf-8")
    component.write_text("// ignored include component\n", encoding="utf-8")
    (repo_root / ".gitignore").write_text("test_*\n", encoding="utf-8")
    module.subprocess.run(["git", "init", "-q"], cwd=repo_root, check=True)
    module.subprocess.run(
        ["git", "add", ".gitignore", "parent.rs"], cwd=repo_root, check=True
    )
    monkeypatch.setattr(
        module,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"parent.rs": ("test_ignored_component.rs",)},
    )
    errors = module._reviewed_rust_include_manifest_errors(repo_root)

    assert any(
        str(component) in error and "must be Git-tracked" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "parent_relative",
    REVIEWED_RUST_INCLUDE_MANIFEST_OWNERS,
)
def test_each_reviewed_rust_include_manifest_fails_closed(
    tmp_path: Path,
    parent_relative: Path,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    parent = repo_root / parent_relative
    parent.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT_DIR / parent_relative, parent)
    components = REVIEWED_RUST_INCLUDE_MANIFESTS[parent_relative]
    pending_parents = [parent_relative]
    copied_components: set[Path] = set()
    while pending_parents:
        include_parent = pending_parents.pop()
        for component_relative in REVIEWED_RUST_INCLUDE_MANIFESTS[include_parent]:
            component = include_parent.parent / component_relative
            if component in copied_components:
                continue
            copied_components.add(component)
            destination = repo_root / component
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / component, destination)
            if component in REVIEWED_RUST_INCLUDE_MANIFESTS:
                pending_parents.append(component)
    relative = parent_relative.as_posix()
    errors: list[str] = []
    _path, expanded = module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert errors == []
    assert expanded

    first_component = parent.parent / components[0]
    canonical_component = first_component.read_text(encoding="utf-8")
    first_component.unlink()
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any(
        str(first_component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    substitute = first_component.with_name(
        f"{first_component.stem}_symlink_substitute.rs"
    )
    substitute.write_text(canonical_component, encoding="utf-8")
    first_component.symlink_to(substitute.name)
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any(
        str(first_component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    first_component.unlink()
    first_component.write_text(canonical_component, encoding="utf-8")
    canonical_parent = parent.read_text(encoding="utf-8")
    canonical_include = f'include!("{components[0].as_posix()}");'
    substituted_include = 'include!("substituted_manifest_component.rs");'
    assert canonical_parent.count(canonical_include) == 1
    parent.write_text(
        canonical_parent.replace(canonical_include, substituted_include, 1),
        encoding="utf-8",
    )
    (parent.parent / "substituted_manifest_component.rs").write_text(
        canonical_component,
        encoding="utf-8",
    )
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any("reviewed Rust include inventory must equal" in error for error in errors)

    parent.write_text(
        canonical_parent + '\ninclude!("extra_manifest_component.rs");\n',
        encoding="utf-8",
    )
    (parent.parent / "extra_manifest_component.rs").write_text(
        "// extra split source\n",
        encoding="utf-8",
    )
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any("reviewed Rust include inventory must equal" in error for error in errors)
    for index, companion in enumerate(
        REVIEWED_RUST_INCLUDE_MANIFEST_COMPANIONS.get(parent_relative, ())
    ):
        try:
            test_each_reviewed_rust_include_manifest_fails_closed(
                tmp_path / f"companion-{index:02d}", companion
            )
        except Exception as error:
            raise AssertionError(
                "reviewed Rust include-manifest companion failed: "
                f"index={index}; owner={parent_relative.as_posix()!r}; "
                f"parent={companion.as_posix()!r}"
            ) from error


@pytest.mark.parametrize("component_relative", KURA_PRODUCTION_COMPONENT_FILES)
def test_kura_production_inventory_rejects_missing_and_symlinked_components(
    tmp_path: Path,
    component_relative: Path,
) -> None:
    module = load_checker()
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert errors == []

    component = repo_root / component_relative
    canonical = component.read_text(encoding="utf-8")
    component.unlink()
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        str(component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    substitute = component.with_name(f"{component.stem}_substitute.rs")
    substitute.write_text(canonical, encoding="utf-8")
    component.symlink_to(substitute.name)
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        str(component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors


def test_kura_production_inventory_rejects_substituted_and_extra_includes(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    kura_path = repo_root / "crates/iroha_core/src/kura.rs"
    canonical = kura_path.read_text(encoding="utf-8")
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert errors == []
    expected = 'include!("kura/startup_finality_support.rs");'
    substituted = 'include!("kura/substituted_finality_support.rs");'
    assert canonical.count(expected) == 1
    kura_path.write_text(
        canonical.replace(expected, substituted, 1),
        encoding="utf-8",
    )
    extra = kura_path.parent / "kura/substituted_finality_support.rs"
    extra.write_text("// substituted production component\n", encoding="utf-8")
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        "direct production include inventory must equal" in error for error in errors
    ), errors

    marker = "#[cfg(test)]\npub(crate) mod tests {}"
    assert canonical.count(marker) == 1
    extra_include = 'include!("kura/extra_production_support.rs");\n\n'
    kura_path.write_text(
        canonical.replace(marker, extra_include + marker, 1),
        encoding="utf-8",
    )
    (kura_path.parent / "kura/extra_production_support.rs").write_text(
        "// extra production component\n",
        encoding="utf-8",
    )
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        "direct production include inventory must equal" in error for error in errors
    ), errors

    kura_path.write_text(
        canonical.replace(marker, "#[cfg(test)]\nmod tests {}", 1),
        encoding="utf-8",
    )
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any("terminal cfg(test) module boundary" in error for error in errors), errors


@pytest.mark.parametrize("component_relative", KURA_PRODUCTION_COMPONENT_FILES)
def test_merge_runtime_rejects_retired_ttl_hidden_in_each_kura_component(
    tmp_path: Path,
    component_relative: Path,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    component = repo_root / component_relative
    component.write_text(
        component.read_text(encoding="utf-8")
        + "\nconst MERGE_SIDECAR_SERVER_REQUEST_GATE_TTL_MS: u64 = 1;\n",
        encoding="utf-8",
    )
    errors = merge_runtime_config_errors(repo_root)
    assert any(
        str(component) in error
        and "retired wall-clock sidecar gate TTL must remain absent from production"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "injected"),
    (
        (
            Path("crates/iroha_config/src/parameters/actual.rs"),
            "\nfn retired_ttl_config_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl_ms = 1_u64;\n"
            "    drop(merge_sidecar_server_request_gate_ttl_ms);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            "\nfn retired_ttl_runner_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl = "
            "core::time::Duration::from_secs(1);\n"
            "    drop(merge_sidecar_server_request_gate_ttl);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/merge_sidecar.rs"),
            "\nconst SERVER_REQUEST_GATE_TTL: u64 = 1;\n",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_reintroduced_wall_clock_gate_ttl(
    tmp_path: Path,
    relative: Path,
    injected: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    canonical_source = path.read_text(encoding="utf-8")
    module = load_checker()
    assert (
        module._retired_sidecar_gate_ttl_source_errors(
            path,
            canonical_source,
            str(relative),
        )
        == []
    )
    path.write_text(
        canonical_source + injected,
        encoding="utf-8",
    )

    errors = module._retired_sidecar_gate_ttl_source_errors(
        path,
        path.read_text(encoding="utf-8"),
        str(relative),
    )

    assert any(
        "retired wall-clock sidecar gate TTL must remain absent from production"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize("field", MERGE_RUNTIME_PROJECTED_FIELDS)
def test_merge_runtime_config_v6_rejects_each_projection_field_substitution(
    tmp_path: Path,
    field: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    actual_path = repo_root / "crates/iroha_config/src/parameters/actual.rs"
    source = actual_path.read_text(encoding="utf-8")
    projection_start = source.index("limits: SumeragiV2Limits {")
    projection_end = source.index(
        "native_amx_signing_guard_record_capacity,", projection_start
    )
    needle = f"                {field},"
    position = source.index(needle, projection_start, projection_end)
    replacement = f"                {field}: 0,"
    actual_path.write_text(
        source[:position] + replacement + source[position + len(needle) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(
        "shared fingerprint projection carries all 16 config-v6 merge fields"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "region", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION:",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 5;",
            "merge-runtime shared-config format version 6",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY:",
            "V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "V2_RETIRED_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "config-v6 default V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES:",
            "V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "V2_RETIRED_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "merge-signing metadata headroom has one named config source",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub struct SumeragiV2RuntimeLimits {",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER",
            "user config field merge_sidecar_inbound_session_capacity",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "limits: actual::SumeragiV2RuntimeLimits {",
            ".merge_sidecar_inbound_session_capacity,",
            ".merge_sidecar_inbound_sessions_per_peer,",
            "user parsing maps all 16 config-v6 merge fields without substitution",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "let merge_sidecar_inbound_session_capacity = canonical_bounded_size(",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_session_capacity,",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_sessions_per_peer,",
            "config validation preserves decided and ordinary inbound session corridors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "let merge_sidecar_limits = MergeSidecarLimits::new(",
            "non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?",
            "non_zero(config.limits.merge_sidecar_inbound_session_capacity)?",
            "runner constructs live sidecar and signing limits from all projected merge fields",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "const fn merge_sidecar_server_stream_capacity(",
            "roster_len + wire::MAX_VALIDATORS_PER_HEIGHT",
            "roster_len",
            "adapter sidecar server stream capacity reserves current and predecessor committees",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "None => MergeSidecarTransport::open_durable_with_server_stream_capacity(",
            "limits.merge_sidecar_limits,",
            "MergeSidecarLimits::defaults(),",
            "adapter must derive the canonical responder roster and restore or open only its exact durable source, stream, and roster geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let adapter = Self {",
            "merge_sidecars,\n"
            "            predecessor_sidecar_requesters: None,\n"
            "            exact_output_handoff_owner,\n"
            "            authenticated_merge_qcs:",
            "merge_sidecars,\n"
            "            predecessor_sidecar_requesters: None,\n"
            "            authenticated_merge_qcs:",
            "adapter hands the exact rehydrated sidecar transport into the live production field",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                server_stream_capacity,\n"
            "            )?",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,\n"
            "            )?",
            "live sidecar transport derives checked source-partition capacities",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(",
            "limits.merge_signing_guard_limits,",
            "MergeSigningGuardLimits::defaults(),",
            "adapter opens the durable merge-signing journal with fingerprinted limits",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn defer_block_with_priority(",
            "self.limits.future_block_distance",
            "u64::MAX",
            "live sidecar carrier admission consumes configured future distance",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if bytes.len() > self.limits.max_record_bytes",
            "total > self.limits.max_total_bytes",
            "total > usize::MAX",
            "merge-signing authorization consumes configured aggregate bytes",
        ),
        (
            "crates/irohad/src/main.rs",
            "Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(",
            "&config.sumeragi.limits,",
            "&iroha_config::parameters::actual::SumeragiV2RuntimeLimits::default(),",
            "daemon passes fingerprinted pending-control limits into production Kura",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(",
            "sumeragi_limits,",
            "&SumeragiV2RuntimeLimits::default(),",
            "Kura validates pending-control limits before opening its store",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub(crate) fn persist_pending_certified_merge_entry(",
            "paths.len() == self.pending_control_sidecar_limits.certified_merge_entries",
            "paths.len() == usize::MAX",
            "Kura merge admission consumes the configured pending-entry count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub fn persist_pending_queue_plan_admission_certificate(",
            "paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions",
            "paths.len() == usize::MAX",
            "Kura QueuePlan admission consumes the configured certificate count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "fn validate_pending_merge_entries_on_startup(",
            ".combined_bytes_within_limit(merge_bytes, admission_bytes)",
            ".merge_bytes_within_limit(merge_bytes)",
            "Kura startup consumes the configured shared pending byte limit",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_disconnected_production_seams(
    tmp_path: Path,
    relative: str,
    region: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    source = path.read_text(encoding="utf-8")
    region_start = source.index(region)
    mutation = source.index(old, region_start)
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "check_queue_limit",
            ".checked_add(frame_len)",
            ".saturating_add(frame_len)",
            "checked byte/frame queue admission and overflow rejection",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "encrypted_frame_geometry",
            "u32::try_from(encrypted_size).map_err(|_| Error::FrameTooLarge)?",
            "encrypted_size as u32",
            "checked encrypted sender geometry encrypted_frame_geometry",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
            "crate::peer::data_message_wire_len_from_payload_len::<RelayMessage<T>>(relay_len)",
            "relay_len",
            "checked P2P transport geometry "
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "enqueue_encrypted",
            "if encrypted_size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if encrypted_size > self.max_frame_bytes {",
            "checked runtime-clamped encrypted geometry before cap/queue admission",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "account_enqueued",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .checked_add(frame_len)",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .saturating_add(frame_len)",
            "checked admitted queue-byte accounting",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_plaintext_cap",
            ".min(MAX_ENCRYPTED_FRAME_BYTES)",
            ".min(usize::MAX)",
            "checked P2P transport geometry frame_plaintext_cap",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_queue_charge",
            ".checked_add(P2P_FRAME_LENGTH_PREFIX_BYTES)",
            ".checked_add(0)",
            "checked P2P transport geometry frame_queue_charge",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_p2p_frame_math(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "checked_encoded_frame_len",
            "let encoded_len = ncore::encoded_frame_len(message)?;",
            "let encoded_len = 0;",
            "exact Norito counting preflight before P2P allocation",
        ),
        (
            "try_send",
            "if encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if false && encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "QUIC counting preflight and post-encryption runtime-cap check",
        ),
        (
            "reserve_for_frame",
            "if size > self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if size > self.max_frame_bytes {",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "reserve_for_frame",
            ".ok_or(Error::FrameTooLarge)?\n                .min(needed);",
            ".ok_or(Error::FrameTooLarge)?\n                .min(usize::MAX);",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "prepare_message",
            "let encoded_len = "
            "checked_encoded_frame_len::<T, E>(msg, self.max_frame_bytes)?;",
            "let encoded_len = 0;",
            "counting sender preflight before material encoding",
        ),
        (
            "prepare_encoded_buffer",
            "let max_plaintext = frame_plaintext_cap_for::<E>(self.max_frame_bytes);",
            "let max_plaintext = usize::MAX;",
            "generic AEAD cap before sender batching",
        ),
        (
            "enqueue_encrypted",
            "if self.encrypted.len() != encrypted_size {",
            "if false && self.encrypted.len() != encrypted_size {",
            "post-encryption sender geometry agreement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_runtime_frame_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "merge",
            "other.bytes = 0;",
            "let _released_on_drop = other.bytes;",
            "already-accounted source leases coalesce without release and reacquisition",
        ),
        (
            "credit_owner",
            "if required.len() > self.max_sources {",
            "if false && required.len() > self.max_sources {",
            "shared authenticated-source registry preserves identity, protected sources, and capacity",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_source_owner_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "try_reserve_for_source",
            "(Some(retained), Some(candidate)) => !retained.matches(candidate),",
            "(Some(_), Some(_)) => false,",
            "queued progress tickets must retain the exact weak delivery authority rather than reusing ordinal-equivalent tenure",
        ),
        (
            "try_reserve_for_source",
            "if source_retained.is_some_and(|retained| retained.items >= 1) {",
            "if source_retained.is_some_and(|retained| retained.items >= 2) {",
            "distinct broadcast or direct requests remain FIFO-ranked behind a target owner",
        ),
        (
            "submit_progress_message_to_source",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => return Ok(None),",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => "
            "return Ok(Some(NetworkActorAdmittedTicketIdentity::forged())),",
            (
                "same-request and cancelled admission return no new ticket identity, "
                "while invalid ownership cannot substitute for the original request"
            ),
        ),
        (
            "broadcast_recoverable",
            "&& Arc::ptr_eq(&ticket.topology, &self.reliable_broadcast_topology)",
            "&& true",
            "broadcast retry tickets bind digest, actor budget, and topology publication",
        ),
        (
            "broadcast_recoverable",
            "if !target.membership.is_active() {",
            "if false && !target.membership.is_active() {",
            "broadcast fanout admits each active topology authority through an isolated target source",
        ),
        (
            "progress_ticket_request_digest",
            "let metadata = [0_u8, priority_tag(post.priority)];",
            "let metadata = [1_u8, priority_tag(post.priority)];",
            "canonical progress digest keeps Post and Broadcast request identities disjoint",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_local_actor_split_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_rejects_ordinal_equivalent_weak_authority_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        "matches",
        (("impl", "WeakProgressDeliveryAuthority"),),
        "Arc::ptr_eq(&retained, &candidate.tenure)",
        "retained.connection_ordinal == candidate.tenure.connection_ordinal",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "weak progress authority matching must preserve exact Arc ownership"
        in error
        for error in errors
    ), errors


def _synthetic_production_trace_certificate(module) -> dict[str, object]:
    digest = "a" * 64
    symbol = {
        "path": "production.rs",
        "kind": "method",
        "symbol": "Runtime::authorize",
        "source_sha256": digest,
        "token_sha256": "b" * 64,
    }
    theorem = {
        "path": "proofs.rs",
        "kind": "verus_proof_fn",
        "symbol": "runtime_refines_next",
        "source_sha256": "c" * 64,
        "token_sha256": "d" * 64,
    }
    return {
        "schema_version": module.PRODUCTION_TRACE_EXTRACTION_EVIDENCE_SCHEMA_VERSION,
        "certificate_type": "production_trace_extraction_theorem",
        "theorem": module.PRODUCTION_TRACE_EXTRACTION_THEOREM,
        "canonical_encoding": module.PRODUCTION_TRACE_EXTRACTION_CANONICAL_ENCODING,
        "backend_verification": True,
        "workspace_source_manifest_sha256": "e" * 64,
        "formal_source_manifest_sha256": "f" * 64,
        "multilane_source_manifest_sha256": "1" * 64,
        "artifacts": [
            {"role": "proof_ledger", "sha256": "2" * 64, "size_bytes": 17}
        ],
        "model_sources": [
            {"path": "model.tla", "sha256": "3" * 64, "size_bytes": 31}
        ],
        "model_symbols": [
            {
                "path": "model.tla",
                "kind": "tla_operator",
                "symbol": "ApplyCarrier",
                "token_sha256": "4" * 64,
            }
        ],
        "refinement_symbols": [copy.deepcopy(symbol)],
        "production_symbols": [copy.deepcopy(symbol)],
        "verus_theorems": [copy.deepcopy(theorem)],
        "source_bindings": [
            {
                "id": "canonical_wsv_commit_authorization",
                "action_tags": ["APPLY_CARRIER"],
                "model_symbols": ["ApplyCarrier"],
                "production_symbol": copy.deepcopy(symbol),
                "authorization_source": None,
                "checked_transition_consumer": None,
                "checked_transition_adapter": None,
                "canonical_commit_sink": copy.deepcopy(symbol),
                "carrier_identity_projection": copy.deepcopy(symbol),
                "refinement_kernel": copy.deepcopy(symbol),
                "verus_theorem": copy.deepcopy(theorem),
                "authenticated": True,
            }
        ],
        "proof_linkage": {
            "ledger_document_sha256": "5" * 64,
            "tlaps_document_sha256": "6" * 64,
            "verus_document_sha256": "7" * 64,
            "cross_tool_document_sha256": "8" * 64,
            "cross_tool_ledger_sha256": "5" * 64,
            "component_evidence": {
                "tlaps_sha256": "6" * 64,
                "verus_sha256": "7" * 64,
            },
            "verus_log_sha256": "9" * 64,
            "multilane_dependency_completion": True,
            "multilane_ledger_dependencies": [
                {"id": obligation_id, "status": status}
                for obligation_id, status in (
                    module.PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES
                )
            ],
            "global_machine_checked_completion": False,
        },
    }


def _synthetic_trace_artifact_paths(module, path: Path):
    return module.ProductionTraceExtractionArtifactPaths(
        ledger=path,
        evidence=path,
        verus_evidence=path,
        verus_log=path,
        cross_tool_evidence=path,
    )


def test_production_trace_certificate_links_exact_tlaps_ledger_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    ledger = {
        "obligations": [
            {"id": obligation_id, "status": status}
            for obligation_id, status in (
                module.PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES
            )
        ],
        "machine_checked_completion": False,
    }
    canonical_ledger_sha256 = module._canonical_json_sha256(ledger)
    exact_ledger_sha256 = "a" * 64
    assert exact_ledger_sha256 != canonical_ledger_sha256
    tlaps_evidence = {
        "source_manifest": {"sha256": "b" * 64},
        "ledger_sha256": exact_ledger_sha256,
    }
    verus_evidence = {
        "source_manifest_sha256": "c" * 64,
        "log_sha256": "d" * 64,
    }
    cross_tool_evidence = {
        "source_manifests": {
            "formal_sha256": tlaps_evidence["source_manifest"]["sha256"],
            "workspace_sha256": verus_evidence["source_manifest_sha256"],
        },
        "ledger_sha256": exact_ledger_sha256,
        "component_evidence": {
            "tlaps_sha256": module._canonical_json_sha256(tlaps_evidence),
            "verus_sha256": module._canonical_json_sha256(verus_evidence),
        },
    }
    source_snapshot = {
        "multilane_source_manifest_sha256": "e" * 64,
        "model_sources": [],
        "model_symbols": [],
        "refinement_symbols": [],
        "production_symbols": [],
        "verus_theorems": [],
        "operational_correspondence": {},
        "source_bindings": [],
    }
    monkeypatch.setattr(
        module, "_production_trace_extraction_action_partition_errors", lambda: []
    )
    monkeypatch.setattr(
        module,
        "_production_trace_extraction_source_snapshot",
        lambda **kwargs: source_snapshot,
    )
    monkeypatch.setattr(
        module,
        "_production_trace_artifact_entry",
        lambda role, path: {"role": role, "sha256": "f" * 64, "size_bytes": 1},
    )
    artifacts = _synthetic_trace_artifact_paths(module, tmp_path / "artifact")

    certificate = module.build_production_trace_extraction_evidence(
        ledger,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        cross_tool_evidence=cross_tool_evidence,
        artifacts=artifacts,
    )

    assert certificate["proof_linkage"]["ledger_document_sha256"] == (
        canonical_ledger_sha256
    )
    assert certificate["proof_linkage"]["cross_tool_ledger_sha256"] == (
        exact_ledger_sha256
    )
    substituted = copy.deepcopy(cross_tool_evidence)
    substituted["ledger_sha256"] = "0" * 64
    with pytest.raises(ValueError, match="does not link the exact proof ledger"):
        module.build_production_trace_extraction_evidence(
            ledger,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            cross_tool_evidence=substituted,
            artifacts=artifacts,
        )


def test_production_trace_certificate_authenticates_all_runtime_links() -> None:
    module = load_checker()
    snapshot = module._production_trace_extraction_source_snapshot()
    operational = snapshot["operational_correspondence"]
    assert operational["id"] == "first_release_transition_witness_v1"
    assert operational["schema_version"] == 1
    assert operational["authenticated"] is True
    assert operational["model_source_sha256"] == hashlib.sha256(
        (ROOT_DIR / "formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla").read_bytes()
    ).hexdigest()
    assert len(operational["action_mappings"]) == 27
    assert tuple(
        mapping["model_action"] for mapping in operational["action_mappings"]
    ) == module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
    assert {mapping["discriminant"] for mapping in operational["action_mappings"]} == set(
        range(1, 28)
    )
    assert all(
        mapping["shared_kernel_occurrences"] == 1
        for mapping in operational["action_mappings"]
    )
    for field in (
        "canonical_state_encoder",
        "state_digest_builder",
        "witness_builder",
        "witness_authenticator",
        "trace_replay_reducer",
        "production_transition_checker",
        "replay_classification",
        "witness_schema_version",
        "model_source_identity",
        "shared_transition_kernel",
        "shared_witness_binding_kernel",
        "verus_witness_binding_kernel",
        "verus_witness_theorem",
    ):
        assert operational[field] is not None, field
    assert operational["digest_proof_boundary"] == (
        "canonical-recomputation-plus-trusted-cryptography-contract"
    )
    bindings = {
        binding["id"]: binding for binding in snapshot["source_bindings"]
    }
    assert set(bindings) == {
        "queue_plan_selection_and_reservation_fsync",
        "reservation_cleanup_prefixes",
        "pre_kura_direct_reservation_release",
        "producer_kura_activation",
        "startup_generation_crash_cas",
        "startup_generation_recover_cas",
        "startup_snapshot_recovery_authorization",
        "startup_local_kura_custody_rehydration_cas",
        "producer_payload_transport_fanout",
        "producer_payload_fanout_queue_fence",
        "producer_payload_retransmission_fanout",
        "authenticated_autonomous_late_body_service",
        "execution_input_persistence",
        "durable_autonomous_bundle",
        "ready_qc_persistence",
        "lane_commit_persistence",
        "kura_slot_retirement_persistence",
        "kura_claim_release_prefixes",
        "queue_release_preparation_handoff",
        "queue_release_completion_publication",
        "ready_authorization",
        "ready_signature",
        "canonical_wsv_commit_authorization",
        "live_post_carrier_evidence_repair",
        "startup_reverse_carrier_evidence_repair",
        "state_replay_post_carrier_evidence_repair",
        "recover_reservation_snapshot_parametric_noninterference",
    }
    assert all(binding["authenticated"] is True for binding in bindings.values())
    assert all(
        binding["operational_correspondence_id"] == operational["id"]
        for binding in bindings.values()
    )
    shared_identities = {
        (
            binding["carrier_identity_projection"]["path"],
            binding["carrier_identity_projection"]["symbol"],
            binding["carrier_identity_projection"]["token_sha256"],
        )
        for binding in bindings.values()
    }
    assert len(shared_identities) == 1
    assert next(iter(shared_identities))[1] == (
        "canonical_lane_queue_reservation_group_identity_projection"
    )


def test_production_trace_certificate_extracts_every_required_action() -> None:
    module = load_checker()
    assert module.PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS == ()
    bound = {
        action
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
        for action in binding["model_actions"]
    }
    assert bound == set(module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS)
    mappings = module.PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS
    assert tuple(mapping[0] for mapping in mappings) == (
        module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
    )
    assert len({mapping[0] for mapping in mappings}) == 27
    assert len({mapping[1] for mapping in mappings}) == 27
    assert {mapping[2] for mapping in mappings} == set(range(1, 28))


@pytest.mark.parametrize(
    ("symbol", "required_token"),
    (
        (
            "check_in_flight_transition",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
        ),
        ("from_replay", "canonical_reconciliation_owners_from_state"),
        (
            "canonical_reconciliation_owners_from_snapshot",
            "snapshot.ordered_groups",
        ),
        (
            "canonical_reconciliation_identity",
            "SNAPSHOT_RECONCILIATION_FINAL_DOMAIN",
        ),
        (
            "recover_snapshot_transition_projection",
            "optional_owner_refinement_projection(None)",
        ),
        (
            "transition_projection_coverage_identity",
            "CHECKED_TRANSITION_COVERAGE_FINAL_DOMAIN",
        ),
        (
            "binds_reconciliation_snapshot",
            "transition_projection_coverage_identity",
        ),
        ("consume_snapshot_replay_seal", "checked_file_content_identity"),
        (
            "install_lane_reservation_journal",
            "consume_snapshot_replay_seal",
        ),
        (
            "bind_lane_reservation_startup_reconciliation_receipt",
            "binds_reconciliation_snapshot",
        ),
        (
            "plan_lane_reservation_ownership",
            "bind_lane_reservation_startup_reconciliation_receipt",
        ),
        (
            "publish_autonomous_lifecycle_bootstrap_cursor_stage",
            "&authority.bootstrap.body.prepared_activate",
        ),
        (
            "revalidate_autonomous_lifecycle_bootstrap_for_completion",
            "if bytes != authority.expected_bytes",
        ),
        (
            "authenticate_autonomous_lifecycle_bootstrap_recovery",
            "authority.stage != expected_stage",
        ),
        (
            "authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody",
            "AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue",
        ),
        (
            "consume_autonomous_lifecycle_bootstrap_completion_fence",
            "drop(authorization",
        ),
        (
            "complete_autonomous_lifecycle_bootstrap",
            "Self::consume_autonomous_lifecycle_bootstrap_completion_fence(fence",
        ),
        (
            "apply_lane_reservation_reconciliation_plan",
            "revalidate_lane_reservation_startup_reconciliation_receipt",
        ),
        (
            "run_non_pending_lifecycle_loop",
            "construct_after_pending_tip_application_recovery",
        ),
        (
            "reconcile_pending_lane_startup",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning",
        ),
        (
            "run_pending_kura_lifecycle_height",
            "pending.prepare_lane_recovery",
        ),
        (
            "prepare_lane_recovery",
            "lane_work.install_lane_drain_queue",
        ),
        (
            "complete_lane_reservation_startup_reconciliation",
            "receipt.initial_snapshot",
        ),
    ),
)
def test_production_trace_certificate_rejects_snapshot_recovery_bridge_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    symbol: str,
    required_token: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["symbol"] == symbol)
    source = (ROOT_DIR / binding["path"]).read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=binding["path"],
        symbol=binding["symbol"],
        impl_name=binding["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    assert required_token in item.source
    mutated_item = item.source.replace(
        required_token,
        f"{required_token}_MUTATED",
        1,
    )
    assert mutated_item != item.source
    mutated_path = tmp_path / f"snapshot-recovery-{symbol}.rs"
    mutated_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(
        module,
        "PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS",
        tuple(bindings),
    )

    with pytest.raises(ValueError, match="RecoverReservationSnapshot parametric bridge"):
        module._production_trace_extraction_source_snapshot()


def test_production_trace_certificate_rejects_roster_gated_lifecycle_process_generation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "startup_snapshot_recovery_authorization"
    )
    claim_source = next(
        source
        for source in binding["supporting_sources"]
        if source["symbol"] == "claim_runner_lifecycle_process_generation"
    )
    source_path = ROOT_DIR / claim_source["path"]
    source = source_path.read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=claim_source["path"],
        symbol=claim_source["symbol"],
        impl_name=claim_source["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    configured_role_dispatch = "match role {"
    roster_membership_gate = """if local_validator_index(context, local_peer, role)?.is_none() {
        return Ok(None);
    }
    match role {"""
    assert configured_role_dispatch in item.source
    mutated_item = item.source.replace(
        configured_role_dispatch,
        roster_membership_gate,
        1,
    )
    mutated_path = tmp_path / "roster-gated-lifecycle-process-generation.rs"
    mutated_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    claim_source["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "configured-role lifecycle process-generation claim" in message
    assert "contains forbidden exact code tokens" in message
    assert "local_validator_index" in message


def test_production_trace_certificate_rejects_modular_lifecycle_branch_handoff_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The bootstrap owner must hand each recovered mode to its typed child."""

    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "startup_snapshot_recovery_authorization"
    )
    handoff = next(
        source
        for source in binding["supporting_sources"]
        if source["role"] == "runner lifecycle process-generation branch handoff"
    )
    source_path = ROOT_DIR / handoff["path"]
    source = source_path.read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=handoff["path"],
        symbol=handoff["symbol"],
        impl_name=handoff["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    old = "lifecycle_pending_kura::run_pending_kura_lifecycle_height("
    assert item.source.count(old) == 1
    mutated_path = tmp_path / "missing-pending-lifecycle-branch.rs"
    mutated_path.write_text(
        source.replace(item.source, item.source.replace(old, "drop(", 1), 1),
        encoding="utf-8",
    )
    handoff["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError, match="branch handoff"):
        module._production_trace_extraction_source_snapshot()


@pytest.mark.parametrize(
    "replacement",
    (
        "iroha_data_model::NetworkId::default()",
        "foreign_context.network_id",
    ),
)
def test_production_trace_certificate_rejects_non_context_lifecycle_network_id(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    replacement: str,
) -> None:
    """The runner may not substitute or alias the authenticated context NetworkId."""

    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "startup_snapshot_recovery_authorization"
    )
    claim_source = next(
        source
        for source in binding["supporting_sources"]
        if source["symbol"] == "claim_runner_lifecycle_process_generation"
    )
    source_path = ROOT_DIR / claim_source["path"]
    source = source_path.read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=claim_source["path"],
        symbol=claim_source["symbol"],
        impl_name=claim_source["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    assert item.source.count("context.network_id") == 1
    mutated_item = item.source.replace("context.network_id", replacement, 1)
    mutated_path = tmp_path / "non-context-lifecycle-network-id.rs"
    mutated_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    claim_source["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "configured-role lifecycle process-generation claim" in message
    assert "context.network_id" in message


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "network_id: iroha_data_model::NetworkId",
            "network_id: iroha_crypto::Hash",
        ),
        (
            """network_id,
            local_peer_id: local_peer_id.clone(),""",
            """network_id: iroha_data_model::NetworkId::default(),
            local_peer_id: local_peer_id.clone(),""",
        ),
    ),
)
def test_production_trace_certificate_rejects_untyped_or_foreign_kura_claim(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    old: str,
    new: str,
) -> None:
    """Kura must consume and return the same typed NetworkId claim."""

    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "startup_snapshot_recovery_authorization"
    )
    claim_source = next(
        source
        for source in binding["supporting_sources"]
        if source["role"]
        == "typed Kura lifecycle process-generation claim propagation"
    )
    source_path = ROOT_DIR / claim_source["path"]
    source = source_path.read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=claim_source["path"],
        symbol=claim_source["symbol"],
        impl_name=claim_source["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    assert item.source.count(old) == 1
    mutated_item = item.source.replace(old, new, 1)
    mutated_path = tmp_path / "foreign-kura-lifecycle-claim.rs"
    mutated_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    claim_source["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "typed Kura lifecycle process-generation claim propagation" in message
    assert "missing exact code tokens" in message or "forbidden exact code tokens" in message


def test_production_trace_certificate_rejects_missing_context_claim_revalidation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Startup recovery must reject a process claim from another NetworkId."""

    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "startup_snapshot_recovery_authorization"
    )
    recovery_source = next(
        source
        for source in binding["supporting_sources"]
        if source["symbol"] == "reconcile_autonomous_lifecycle_startup"
    )
    source_path = ROOT_DIR / recovery_source["path"]
    source = source_path.read_text(encoding="utf-8")
    extraction_errors: list[str] = []
    item = module._production_trace_unique_function(
        root_dir=ROOT_DIR,
        relative=recovery_source["path"],
        symbol=recovery_source["symbol"],
        impl_name=recovery_source["impl"],
        errors=extraction_errors,
    )
    assert extraction_errors == []
    assert item is not None
    exact_binding = "|| process_generation.network_id() != network_id\n"
    assert item.source.count(exact_binding) == 1
    mutated_item = item.source.replace(exact_binding, "|| false\n", 1)
    mutated_path = tmp_path / "missing-context-claim-revalidation.rs"
    mutated_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    recovery_source["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "signed startup lifecycle recovery coordinator" in message
    assert "process_generation.network_id() != network_id" in message


def test_production_trace_certificate_rejects_reintroduced_open_action_debt(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    monkeypatch.setattr(
        module, "PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS", ("Crash",)
    )

    with pytest.raises(ValueError, match="do not partition the exact model-action"):
        module.build_production_trace_extraction_evidence(
            module.load_ledger(),
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=None,
        )


def test_production_trace_certificate_rejects_model_action_inventory_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    with monkeypatch.context() as scoped:
        scoped.setattr(
            module,
            "PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS",
            module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS[:-1],
        )

        with pytest.raises(ValueError) as failure:
            module._production_trace_extraction_source_snapshot()

        assert (
            "required model actions differ from the multilane source-binding ledger"
            in str(failure.value)
        )

    with monkeypatch.context() as scoped:
        mappings = module.PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS
        scoped.setattr(
            module,
            "PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS",
            (*mappings[:-1], mappings[-2]),
        )
        with pytest.raises(ValueError) as failure:
            module._production_trace_extraction_source_snapshot()
        assert "duplicate model action" in str(failure.value)

    original_reader = module._bounded_regular_file_bytes
    operational_path = (
        ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_core.rs"
    ).resolve()

    def mutated_operational_reader(path, *args, **kwargs):
        payload = original_reader(path, *args, **kwargs)
        if Path(path).resolve() != operational_path:
            return payload
        source = payload.decode("utf-8")
        source = source.replace(
            "fn production_in_flight_first_release_transition_witness_v1(",
            "#[cfg(test)]\nfn production_in_flight_first_release_transition_witness_v1(",
            1,
        )
        return source.encode("utf-8")

    with monkeypatch.context() as scoped:
        scoped.setattr(
            module,
            "_bounded_regular_file_bytes",
            mutated_operational_reader,
        )
        with pytest.raises(ValueError) as failure:
            module._production_trace_extraction_source_snapshot()
        assert "transition_witness_v1 is test-only" in str(failure.value)

    def unchecked_operational_reader(path, *args, **kwargs):
        payload = original_reader(path, *args, **kwargs)
        if Path(path).resolve() != operational_path:
            return payload
        return payload.replace(
            b"checked.with_first_release_witness(witness)",
            b"checked",
            1,
        )

    with monkeypatch.context() as scoped:
        scoped.setattr(
            module,
            "_bounded_regular_file_bytes",
            unchecked_operational_reader,
        )
        with pytest.raises(ValueError) as failure:
            module._production_trace_extraction_source_snapshot()
        assert "checked.with_first_release_witness(witness)" in str(failure.value)


@pytest.mark.parametrize(
    ("binding_id", "action_tag"),
    (
        (
            "queue_plan_selection_and_reservation_fsync",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4",
        ),
        (
            "queue_plan_selection_and_reservation_fsync",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5",
        ),
        (
            "reservation_cleanup_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED",
        ),
        (
            "pre_kura_direct_reservation_release",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
        ),
        (
            "producer_kura_activation",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA",
        ),
        (
            "producer_payload_transport_fanout",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        (
            "producer_payload_fanout_queue_fence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        (
            "producer_payload_retransmission_fanout",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        (
            "authenticated_autonomous_late_body_service",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY",
        ),
        (
            "execution_input_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
        ),
        (
            "durable_autonomous_bundle",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
        ),
        (
            "ready_qc_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC",
        ),
        (
            "lane_commit_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT",
        ),
        (
            "kura_slot_retirement_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT",
        ),
        (
            "kura_claim_release_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING",
        ),
        (
            "kura_claim_release_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED",
        ),
        (
            "queue_release_preparation_handoff",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE",
        ),
        (
            "ready_authorization",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY",
        ),
        (
            "ready_signature",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY",
        ),
        (
            "canonical_wsv_commit_authorization",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER",
        ),
        (
            "live_post_carrier_evidence_repair",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
        (
            "startup_reverse_carrier_evidence_repair",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
        (
            "state_replay_post_carrier_evidence_repair",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
    ),
)
def test_production_trace_certificate_rejects_each_disconnected_runtime_link(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
    action_tag: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    source_path = ROOT_DIR / binding["path"]
    source = source_path.read_text(encoding="utf-8")
    assert action_tag in source
    mutated_path = tmp_path / f"{binding_id}.rs"
    mutated_path.write_text(
        source.replace(action_tag, f"{action_tag}_MUTATED"),
        encoding="utf-8",
    )
    binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert binding_id in message
    assert "missing exact code tokens" in message


@pytest.mark.parametrize(
    "binding_id",
    (
        "queue_plan_selection_and_reservation_fsync",
        "reservation_cleanup_prefixes",
        "pre_kura_direct_reservation_release",
        "producer_kura_activation",
        "producer_payload_transport_fanout",
        "producer_payload_fanout_queue_fence",
        "producer_payload_retransmission_fanout",
        "authenticated_autonomous_late_body_service",
        "execution_input_persistence",
        "durable_autonomous_bundle",
        "ready_qc_persistence",
        "lane_commit_persistence",
        "kura_slot_retirement_persistence",
        "kura_claim_release_prefixes",
        "queue_release_preparation_handoff",
        "queue_release_completion_publication",
        "ready_authorization",
        "ready_signature",
        "canonical_wsv_commit_authorization",
        "live_post_carrier_evidence_repair",
        "startup_reverse_carrier_evidence_repair",
        "state_replay_post_carrier_evidence_repair",
    ),
)
def test_production_trace_certificate_rejects_each_disconnected_carrier_identity(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    helper = "canonical_lane_queue_reservation_group_identity_projection"
    endpoint = binding
    authority = binding.get("authorization_source")
    if authority is not None and helper in authority["required_tokens"]:
        endpoint = authority
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    if helper not in source:
        endpoint = binding["authorization_source"]
        source_path = ROOT_DIR / endpoint["path"]
        source = source_path.read_text(encoding="utf-8")
    assert helper in source
    mutated_path = tmp_path / f"{binding_id}-identity.rs"
    mutated_path.write_text(
        source.replace(helper, f"{helper}_DISCONNECTED"),
        encoding="utf-8",
    )
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert binding_id in message
    assert helper in message


def test_production_trace_certificate_rejects_disconnected_selection_authority(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "queue_plan_selection_and_reservation_fsync"
    )
    authority = binding["authorization_source"]
    source_path = ROOT_DIR / authority["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "self.reservation_scope()"
    assert required in source
    mutated_path = tmp_path / "selection-authority-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "self.disconnected_selection_scope()"),
        encoding="utf-8",
    )
    authority["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical authorization source tokens" in message
    assert "reservation_scope" in message


def test_production_trace_certificate_rejects_disconnected_ready_signature_sink(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "ready_signature"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "authorization.consume_signing_request"
    assert required in source
    mutated_path = tmp_path / "ready-signature-sink-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "authorization.disconnected_signing_request"),
        encoding="utf-8",
    )
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "consume_signing_request" in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_consumer(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    consumer = binding["checked_transition_consumer"]
    source_path = ROOT_DIR / consumer["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "checked.into_projection()"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-consumer-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "checked.discard_without_projection()"),
        encoding="utf-8",
    )
    consumer["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing move-only consumer canonical_wsv_commit_authorization" in message
    assert "checked.into_projection()" in message


@pytest.mark.parametrize(
    ("binding_token", "source_token", "replacement"),
    (
        (
            "CheckedCarrierApplications::for_block",
            "CheckedCarrierApplications::for_block",
            "CheckedCarrierApplications::disconnected_for_block",
        ),
        (
            "checked_carrier_applications.bind_execution_batch(reference, applications.len())",
            "checked_carrier_applications.bind_execution_batch(reference, applications.len())",
            "checked_carrier_applications.disconnected_execution_batch(reference, applications.len())",
        ),
        (
            "checked_carrier_applications.push",
            "checked_carrier_applications.push(",
            "checked_carrier_applications.discard(",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_apply_carrier_batch_binding(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_token: str,
    source_token: str,
    replacement: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    orchestration = binding["supporting_sources"][0]
    source_path = ROOT_DIR / orchestration["path"]
    source = source_path.read_text(encoding="utf-8")
    assert source_token in source
    mutated_path = tmp_path / "apply-carrier-batch-binding-disconnected.rs"
    mutated_path.write_text(source.replace(source_token, replacement), encoding="utf-8")
    orchestration["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing finality-to-State ApplyCarrier orchestration" in message
    assert binding_token in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_adapter(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    adapter = binding["checked_transition_adapter"]
    source_path = ROOT_DIR / adapter["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "CheckedCarrierApplications::consume_for_state_commit"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-adapter-disconnected.rs"
    mutated_path.write_text(
        source.replace(
            required,
            "CheckedCarrierApplications::discard_before_state_commit",
        ),
        encoding="utf-8",
    )
    adapter["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing move-only State commit adapter" in message
    assert required in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_forwarder(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    source_binding = binding["authorization_source"]
    source_path = ROOT_DIR / source_binding["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "Box::new(checked_carrier_applications)"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-forwarder-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "Box::new(disconnected_carrier_applications)"),
        encoding="utf-8",
    )
    source_binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical authorization source tokens" in message
    assert required in message


def test_production_trace_certificate_rejects_disconnected_state_commit_sink(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    required = ".consume_for_state_commit(block_header_hash, staged_merge_entry.as_ref())"
    assert required in source
    mutated_path = tmp_path / "state-commit-sink-disconnected.rs"
    mutated_path.write_text(
        source.replace(
            required,
            ".discard_before_state_commit(block_header_hash, staged_merge_entry.as_ref())",
        ),
        encoding="utf-8",
    )
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "authorization.consume_for_state_commit" in message


def test_production_trace_certificate_rejects_apply_carrier_after_state_commit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    authorization_start = source.index(
        "        if tx_validate_accepted && !replay_prevalidation {\n"
        "            match state_commit_authorization.take()"
    )
    authorization_end = source.index(
        "        let autoscale_storage_hold", authorization_start
    )
    authorization_block = source[authorization_start:authorization_end]
    without_authorization = (
        source[:authorization_start] + source[authorization_end:]
    )
    transaction_commit = "            let tx_commit_result = transactions.commit();"
    insertion = without_authorization.index(transaction_commit) + len(transaction_commit)
    mutated = (
        without_authorization[:insertion]
        + "\n"
        + authorization_block
        + without_authorization[insertion:]
    )
    mutated_path = tmp_path / "apply-carrier-after-state-commit.rs"
    mutated_path.write_text(mutated, encoding="utf-8")
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "moved before its predecessor" in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "write_autonomous_lane_block_view_state_record_locked",
            "disconnect_autonomous_lane_block_view_state_record_locked",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_ready_qc_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "ready_qc_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"ready-qc-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "sync_bound_progress_append_data",
            "disconnect_bound_progress_append_data",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_execution_input_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "execution_input_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"execution-input-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "promote_bound_progress_temp",
            "disconnect_bound_progress_temp",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_lane_commit_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "lane_commit_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"lane-commit-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("binding_id", "edge", "required", "replacement", "expected_message"),
    (
        (
            "kura_slot_retirement_persistence",
            "authorization_source",
            "canonical_lane_queue_reservation_group_identity_projection",
            "disconnected_lane_queue_reservation_group_identity_projection",
            "missing canonical authorization source tokens",
        ),
        (
            "kura_slot_retirement_persistence",
            "commit_sink",
            "write_atomic_synced_replace",
            "disconnected_atomic_synced_replace",
            "missing canonical commit sink tokens",
        ),
        (
            "kura_claim_release_prefixes",
            "authorization_source",
            "replacement.retirement_hash()",
            "replacement.disconnected_retirement_hash()",
            "missing canonical authorization source tokens",
        ),
        (
            "kura_claim_release_prefixes",
            "commit_sink",
            "persist(path)",
            "disconnected_persist(path)",
            "missing canonical commit sink tokens",
        ),
        (
            "queue_release_preparation_handoff",
            "authorization_source",
            "autonomous_lane_entrypoint_claim_release_progress_locked",
            "disconnected_lane_entrypoint_claim_release_progress_locked",
            "missing canonical authorization source tokens",
        ),
        (
            "queue_release_preparation_handoff",
            "commit_sink",
            "journal.prepare_release(barrier.clone())",
            "journal.disconnected_prepare_release(barrier.clone())",
            "missing canonical commit sink tokens",
        ),
        (
            "queue_release_completion_publication",
            "authorization_source",
            "consume_for_claim_transition",
            "disconnected_claim_transition",
            "missing canonical authorization source tokens",
        ),
        (
            "queue_release_completion_publication",
            "primary",
            "release_barrier_has_exact_fifo_ownership_locked",
            "disconnected_release_barrier_fifo_ownership",
            "missing authenticated binding",
        ),
        (
            "pre_kura_direct_reservation_release",
            "primary",
            "journal.release_batch",
            "journal.disconnected_release_batch",
            "missing authenticated binding",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_kura_release_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    endpoint = binding if edge == "primary" else binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"{binding_id}-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "authorization.height_context_id()",
            "authorization.disconnected_height_context_id()",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "finalize_autonomous_lane_entrypoint_claims_locked",
            "disconnect_autonomous_lane_entrypoint_claims_locked",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_kura_activation_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "producer_kura_activation"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"producer-kura-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


def test_production_trace_certificate_writer_emits_one_canonical_encoding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    path = tmp_path / "production_trace_extraction_evidence.json"

    module.write_production_trace_extraction_evidence(path, certificate)

    assert module.load_production_trace_extraction_evidence(path) == certificate
    assert path.read_bytes() == module._production_trace_canonical_json_bytes(
        certificate
    )


def test_production_trace_certificate_rejects_duplicate_json_keys(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "duplicate.json"
    path.write_text('{"schema_version":1,"schema_version":1}\n', encoding="utf-8")

    with pytest.raises(ValueError, match="duplicate JSON key"):
        module.load_production_trace_extraction_evidence(path)


def test_production_trace_certificate_rejects_malformed_and_noncanonical_json(
    tmp_path: Path,
) -> None:
    module = load_checker()
    malformed = tmp_path / "malformed.json"
    malformed.write_text('{"schema_version":\n', encoding="utf-8")
    noncanonical = tmp_path / "noncanonical.json"
    noncanonical.write_text('{ "schema_version": 1 }\n', encoding="utf-8")

    with pytest.raises(ValueError, match="invalid JSON"):
        module.load_production_trace_extraction_evidence(malformed)
    with pytest.raises(ValueError, match="not canonical"):
        module.load_production_trace_extraction_evidence(noncanonical)


def test_production_trace_certificate_rejects_oversize_and_symlink_inputs(
    tmp_path: Path,
) -> None:
    module = load_checker()
    oversized = tmp_path / "oversized.json"
    oversized.write_bytes(
        b"{" + b" " * module.PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES + b"}\n"
    )
    target = tmp_path / "target.json"
    target.write_text("{}\n", encoding="utf-8")
    alias = tmp_path / "alias.json"
    alias.symlink_to(target)

    with pytest.raises(ValueError, match="at most"):
        module.load_production_trace_extraction_evidence(oversized)
    with pytest.raises(ValueError, match="non-symlink"):
        module.load_production_trace_extraction_evidence(alias)
    with pytest.raises(ValueError, match="non-symlink"):
        module._production_trace_artifact_entry("proof_ledger", alias)


def test_production_trace_certificate_rejects_hardlink_inputs_and_output(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    original = tmp_path / "original.json"
    original.write_bytes(module._production_trace_canonical_json_bytes(certificate))
    input_alias = tmp_path / "input-hardlink.json"
    output_alias = tmp_path / "output-hardlink.json"
    try:
        os.link(original, input_alias)
        os.link(original, output_alias)
    except OSError as error:
        pytest.skip(f"hard links unavailable: {error}")

    with pytest.raises(ValueError, match="exactly one hard link"):
        module.load_production_trace_extraction_evidence(input_alias)
    with pytest.raises(ValueError, match="exactly one hard link"):
        module._production_trace_artifact_entry("proof_ledger", input_alias)
    with pytest.raises(ValueError, match="exactly one hard link"):
        module.write_production_trace_extraction_evidence(
            output_alias, certificate
        )
    assert output_alias.samefile(original)


def test_production_trace_certificate_rejects_symlinked_parent_components(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    real_parent = tmp_path / "real"
    real_parent.mkdir()
    real_certificate = real_parent / "certificate.json"
    real_certificate.write_bytes(
        module._production_trace_canonical_json_bytes(certificate)
    )
    linked_parent = tmp_path / "linked-parent"
    try:
        linked_parent.symlink_to(real_parent, target_is_directory=True)
    except OSError as error:
        pytest.skip(f"symlinks unavailable: {error}")

    with pytest.raises(ValueError, match="parent path contains a symlink component"):
        module.load_production_trace_extraction_evidence(
            linked_parent / real_certificate.name
        )
    with pytest.raises(ValueError, match="parent path contains a symlink component"):
        module.write_production_trace_extraction_evidence(
            linked_parent / "output.json", certificate
        )


def test_production_trace_certificate_rejects_every_top_level_field_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )

    for field in expected:
        observed = copy.deepcopy(expected)
        del observed[field]
        errors = module._production_trace_extraction_evidence_errors(
            {},
            observed,
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=paths,
        )
        assert errors and "canonical current theorem certificate" in errors[0], field


def test_production_trace_certificate_rejects_every_nested_field_hash_and_source_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )

    def leaf_paths(value, prefix=()):
        if isinstance(value, dict):
            for key in sorted(value):
                yield from leaf_paths(value[key], (*prefix, key))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                yield from leaf_paths(item, (*prefix, index))
        else:
            yield prefix

    for path in leaf_paths(expected):
        observed = copy.deepcopy(expected)
        owner = observed
        for component in path[:-1]:
            owner = owner[component]
        original = owner[path[-1]]
        if isinstance(original, bool):
            replacement = not original
        elif isinstance(original, int):
            replacement = original + 1
        else:
            replacement = f"{original}-drift"
        owner[path[-1]] = replacement
        errors = module._production_trace_extraction_evidence_errors(
            {},
            observed,
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=paths,
        )
        assert errors and "canonical current theorem certificate" in errors[0], path


def test_production_trace_certificate_rejects_missing_proof_linkage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )
    observed = copy.deepcopy(expected)
    del observed["proof_linkage"]["component_evidence"]

    errors = module._production_trace_extraction_evidence_errors(
        {},
        observed,
        tlaps_evidence={},
        verus_evidence={},
        cross_tool_evidence={},
        artifacts=paths,
    )

    assert errors == [
        "production trace-extraction evidence does not match the canonical "
        "current theorem certificate at $.proof_linkage"
    ]


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new"),
    (
        (
            "operator",
            "AsyncIngressSchedulerBarrierActive",
            "  \\/ AsyncOrdinaryIngressProtectedRecordsAt(node) # {}",
            "  \\/ FALSE",
        ),
        (
            "operator",
            "AsyncEarliestIngressSchedulerOrdinal",
            "       ELSE AsyncOrdinaryIngressEarliestPhysicalRecord(\n"
            "              node).schedulerOrdinal",
            "       ELSE AsyncLeaderWireEarliestPhysicalIngressRecord(\n"
            "              node).schedulerOrdinal",
        ),
        (
            "operator",
            "AsyncOlderRuntimeLifecyclePrecedesIngressScheduler",
            "  /\\ AsyncSelectedRuntimeSourcePhysicalOrdinal(node)\n"
            "       < AsyncEarliestIngressPhysicalOrdinal(node)",
            "  /\\ TRUE",
        ),
        (
            "operator",
            "AsyncOlderLocalLifecyclePrecedesServeIngress",
            "  /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "       node, SelectedLocalSource(node))\n"
            "       < AsyncEarliestIngressPhysicalOrdinal(node)",
            "  /\\ TRUE",
        ),
        (
            "operator",
            "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
            "     !.retransmitLifecycleOrdinal =",
            "     !.timeoutLifecycleOrdinal =",
        ),
        (
            "operator",
            "AsyncSharedSchedulerOrdinalInjectionInvariant",
            "  /\\ \\A admission \\in asyncServeIngressAdmissions:\n"
            "       AsyncRetransmitLifecycleOwned(admission.node)\n"
            "         => admission.schedulerOrdinal #\n"
            "              AsyncRetransmitLifecycleOrdinal(admission.node)\n",
            "",
        ),
        (
            "theorem",
            "SerializedLocalPrecedesServeIngressExactFrame",
            "         /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "              node, SelectedLocalSource(node))\n"
            "              < AsyncEarliestIngressPhysicalOrdinal(node)",
            "         /\\ TRUE",
        ),
        (
            "theorem",
            "AsyncLaterServeTicketInterleavesOlderRuntimeEpisode",
            "    /\\ AsyncSelectedRuntimeSourcePhysicalOrdinal(node)\n"
            "         < AsyncEarliestIngressPhysicalOrdinal(node)",
            "    /\\ TRUE",
        ),
        (
            "theorem",
            "AsyncLaterServeTicketInterleavesOlderLocalEpisode",
            "    /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "         node, SelectedLocalSource(node))\n"
            "         < AsyncEarliestIngressPhysicalOrdinal(node)",
            "    /\\ TRUE",
        ),
    ),
)
def test_serve_scheduler_ordinal_release_contract_rejects_current_weakening(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_serve_scheduler_ordinal_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    mutate = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutate(source, symbol, old, new), encoding="utf-8")
    module.SERVE_SCHEDULER_ORDINAL_RELEASE_SOURCE_SHA256[path.name] = (
        hashlib.sha256(path.read_bytes()).hexdigest()
    )

    errors = module._serve_scheduler_ordinal_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    prefix = "theorem " if kind == "theorem" else ""
    assert any(
        f"{prefix}{symbol} must equal only" in error for error in errors
    ), errors


def _assert_commit_import_release_or_stale_artifact(
    tmp_path: Path, artifact_name: str
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_commit_import_provenance_mutation_fixture(
        tmp_path, module
    )
    path = repo_root / artifact_name if "/" in artifact_name else formal_dir / artifact_name
    release_mutations = {
        "SumeragiV2AsyncNetwork.tla": (
            "DirectCommitQcCandidateHasExactImportLineage",
            "    /\\ item.envelope.qc.context = context\n",
            "    /\\ TRUE\n",
        ),
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla": (
            "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
            "  IndexedChainSpec\n"
            "    => IndexedHistoricalCertificateLocalImportCandidateEntryProperty\n",
            "  IndexedChainSpec\n    => TRUE\n",
        ),
    }
    release_mutation = release_mutations.get(artifact_name)
    if release_mutation is None:
        path.write_text(
            path.read_text(encoding="utf-8") + "\n\\* stale import provenance\n",
            encoding="utf-8",
        )
    else:
        symbol, old, new = release_mutation
        source = path.read_text(encoding="utf-8")
        path.write_text(
            mutate_tla_theorem(source, symbol, old, new), encoding="utf-8"
        )
        module.COMMIT_IMPORT_PROVENANCE_RELEASE_SOURCE_SHA256[path.name] = (
            hashlib.sha256(path.read_bytes()).hexdigest()
        )

    errors = module._commit_import_provenance_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    if release_mutation is None:
        assert any(
            str(path) in error
            and (
                "must match exact reviewed SHA-256" in error
                or "must match frozen SHA-256" in error
            )
            for error in errors
        ), errors
    else:
        assert any(
            f"Commit-import release theorem {symbol} must state only" in error
            for error in errors
        ), errors


@pytest.mark.parametrize(
    ("replacement", "expected_counts"),
    (
        ("let linked_before = match removed_statat(", "(0, 0, 0)"),
        (
            """if false {
                let linked_before = match rustix::fs::statat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                ) {
                    Ok(stat) => stat,
                    Err(_) => unreachable!(),
                };
                let _ = linked_before;
            }
            let linked_before = match rustix::fs::statat(""",
            "(0, 0, 2)",
        ),
    ),
)
def test_serviced_candidate_read_discriminator_fails_closed_without_crashing(
    tmp_path: Path,
    replacement: str,
    expected_counts: str,
) -> None:
    """Missing or duplicate bounded-read ownership returns a diagnostic."""

    module = load_checker()
    copy_serviced_candidate_production_fixture(tmp_path)
    assert module._serviced_candidate_production_source_fidelity_errors(tmp_path) == []
    safety_path = tmp_path / "crates/iroha_core/src/sumeragi/safety_wal.rs"
    mutate_source_once(
        safety_path,
        "let linked_before = match rustix::fs::statat(",
        replacement,
    )

    errors = module._serviced_candidate_production_source_fidelity_errors(tmp_path)
    assert any(
        "require exactly one parsed bounded adjacent read" in error
        and f"discriminator_counts={expected_counts}" in error
        for error in errors
    ), errors
