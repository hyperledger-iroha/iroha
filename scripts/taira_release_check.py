#!/usr/bin/env python3
"""Catch Taira application, consensus and proof regressions before cross-compilation.

Requires Python 3.11+ and the repository Rust toolchain. Compile focused native
harnesses and run a four-peer network with isolated Cargo and fixture-only inputs.
The existing sibling .taira-testnet-build-targets/routine lane is the default;
--target-dir or TAIRA_TESTNET_CARGO_TARGET_DIR may select another development
lane. Both selectors must agree when supplied. No Cargo lane is created or cleaned.
Native checks retain incremental compilation unless CARGO_INCREMENTAL=0 is
explicitly selected. This preference never changes Linux release compilation.

Configuration and compiler paths match authenticated preparation, while source
remains the mutable checkout. These checks never qualify release artifacts and
accept no live configuration, credentials, SSH, deployment or signing inputs.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import shutil
import stat
from pathlib import Path
import subprocess
import sys
import tempfile
import time


STAGES = (
    ("private config descriptors", (
        "client_config::tests::inherited_config_loads_exact_descriptor_without_reopening_provenance",
        "client_config::tests::inherited_private_descriptor_rejects_writable_unsafe_and_nonregular_inputs",
        "client_config::tests::inherited_private_descriptor_rejects_pipe_socket_and_closed_fd",
        "client_config::tests::inherited_config_errors_never_include_source_values",
        "tests::inherited_config_cli_requires_explicit_provenance_and_rejects_mixed_sources",
        "taira_public_reset::inputs::tests::inherited_owner_key_is_bounded_private_and_matches_the_independent_public_key",
        "taira_public_reset::inputs::tests::signing_key_rejects_hardlinks_and_nonregular_descriptors_without_reading_them",
    )),
    ("explicit operator signing custody", (
        "taira_public_reset::operator_admission_tests::operator_public_key_is_canonical_ed25519_and_authorization_bound",
        "taira_public_reset::operator_admission_tests::operator_policy_requires_explicit_enabled_allowlist_and_rejects_inference",
        "taira_public_reset::executor_model::tests::recovery_args_accept_identical_forward_inputs_without_admitting_unused_paths",
        "operator_key::tests::loads_one_absolute_owner_only_operator_key",
        "operator_key::tests::rejects_indirect_or_non_owner_only_operator_key_files",
        "operator_key::tests::rejects_relative_oversized_and_secret_echoing_operator_key_inputs",
        "operator_key::tests::loads_borrowed_operator_fd_positionally_without_reopening_its_path",
        "operator_key::tests::rejects_operator_fd_numbers_outside_the_inherited_range",
        "operator_key::tests::rejects_non_readonly_nonregular_linked_and_non_owner_only_operator_fds",
        "operator_key::tests::rejects_empty_oversized_and_noncanonical_operator_fds_without_secret_echo",
        "operator_key::tests::positional_operator_read_rejects_mutation_before_final_metadata_check",
        "operator_key::tests::positional_operator_read_handles_short_reads_and_redacts_io_errors",
        "tests::operator_private_key_file_is_an_explicit_global_runtime_option",
        "tests::operator_private_key_fd_is_explicit_bounded_and_exclusive",
        "tests::credential_free_commands_reject_operator_fd_without_reading_it",
        "tests::inherited_operator_key_load_installs_the_explicit_run_context_signer",
        "tests::run_context_installs_only_the_explicit_operator_key",
        "tests::taira_public_reset_local_inputs_require_a_dedicated_operator_key",
        "tests::taira_public_reset_operator_keygen_has_explicit_private_output",
        "taira_public_reset::host::tests::validator_operator_key_custody_rejects_wrong_identity_and_mutation",
        "taira_public_reset::host::tests::candidate_operator_status_child_binds_both_inherited_signers",
        "taira_public_reset::host::tests::candidate_operator_status_child_rejects_missing_or_replaced_operator_key",
    )),
    ("native validator config preparation", (
        "taira_public_reset::config::tests::config_rebase_operator_key_is_canonical_and_changes_only_explicit_operator_fields",
        "taira_public_reset::config::tests::operator_keygen_publishes_canonical_private_key_and_only_public_report",
        "taira_public_reset::config::tests::operator_keygen_rejects_repository_existing_symlink_and_unsafe_parent_paths",
        "taira_public_reset::config::tests::config_rebase_changes_only_genesis_file_and_keeps_errors_secret_free",
        "taira_public_reset::config::tests::config_rebase_inherited_fd_publishes_private_file_without_stdout_or_overwrite",
        "taira_public_reset::config::tests::config_rebase_rejects_drift_malformed_input_and_unsafe_descriptors_before_output",
        "taira_public_reset::config::tests::config_rebase_preserves_caller_offset_and_rejects_writable_or_unlinked_descriptors",
        "taira_public_reset::host::tests::config_upload_admission_requires_owner_private_artifact_mode",
        "taira_public_reset::executor_model::tests::shared_validator_artifacts_are_hashed_once_per_local_source",
        "taira_public_reset::executor_model::tests::shared_artifact_declarations_must_agree_on_content_size_and_mode",
        "taira_public_reset::executor_model::tests::shared_artifact_descriptor_clone_rejects_path_identity_drift",
        "taira_public_reset::inputs::tests::fresh_output_is_private_durable_and_never_overwrites_or_follows_a_symlink",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_accepts_exact_artifact_and_checked_identity",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_rejects_unbound_manifest_identity_and_inheritance",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_rejects_foreign_path_and_network",
    )),
    ("network 369 inventory boundaries", (
        "taira_public_reset::executor_model::tests::inventory_wire_roundtrip_scopes_nonempty_placements_before_decode",
        "taira_public_reset::executor_model::tests::inventory_file_boundary_preserves_original_bytes_and_decode_guard",
    )),
    ("aggregate execution budget before custody", (
        "taira_public_reset::inputs::tests::aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads",
        "taira_public_reset::inputs::tests::aggregate_timeout_policy_accepts_deployment_defaults_and_preserves_individual_bounds",
    )),
    ("physical preseed and start budgets", (
        "taira_public_reset::inputs::tests::preseed_timeout_is_required_and_has_its_own_physical_work_bound",
        "taira_public_reset::host::tests::preseed_and_start_deadlines_charge_only_each_physical_host_carrier",
        "taira_public_reset::host::tests::host_admission_accepts_combined_carrier_verification_but_rejects_borrowed_time",
        "taira_public_reset::executor_model::tests::execution_lifetime_uses_the_exact_first_release_action_ledger",
    )),
    ("generated stage through frozen consumer", (
        "soracloud::tests::taira_inrou_workspace_generator_emits_exact_private_deploy_layout",
        "soracloud::tests::taira_stage_reads_require_private_custody_for_prepared_and_frozen_files",
        "soracloud::tests::prepared_inrou_pin_preserves_exact_sponsor_fee_identity",
    )),
    ("preseed receipt ordering", (
        "taira_public_reset::host::tests::preseed_receipt_targets_follow_receipt_order_for_reversed_stores",
    )),
    ("KVM ioctl error handling", (
        "taira_public_reset::host::tests::kvm_api_query_preserves_notty_for_regular_files",
    )),
    ("bounded duplex process streaming", (
        "taira_public_reset::host::tests::process_runner_streams_large_closure_with_bidirectional_backpressure",
        "taira_public_reset::host::tests::process_runner_output_budget_bounds_continuous_and_interrupted_readers",
        "taira_public_reset::host::tests::process_runner_deadline_kills_descendant_holding_output_pipes",
        "taira_public_reset::host::tests::process_runner_enforces_one_absolute_timeout",
        "taira_public_reset::host::tests::process_runner_handles_child_that_closes_stdin_early",
        "taira_public_reset::host::tests::process_runner_preserves_rejection_after_child_closes_stdin",
        "taira_public_reset::host::tests::process_runner_deadline_reaps_child_after_stdin_closure",
    )),
    ("canonical receipt namespace", (
        "taira_public_reset::host::tests::host_receipt_names_cover_every_action_and_artifact_role",
        "taira_public_reset::host::tests::receipt_names_reject_path_control_and_unicode_escape",
    )),
    ("systemd operation and validator lifecycle", (
        "taira_public_reset::host::tests::validator_http_readiness_retries_cold_backends_before_strict_checks",
        "taira_public_reset::host::tests::validator_http_readiness_rejects_permanent_http_errors",
        "taira_public_reset::host::tests::validator_http_readiness_keeps_deadline_and_authorization",
        "taira_public_reset::host::tests::doctor_failure_reports_only_fixed_checks_and_status_codes",
        "taira_public_reset::host::tests::manager_evidence_stays_pending_until_exact_terminal_job",
        "taira_public_reset::host::tests::manager_recovery_uses_immutable_mutation_deadline_but_observes_terminal_state",
        "taira_public_reset::host::tests::manager_evidence_rejects_wrong_or_duplicate_exec_identity",
        "taira_public_reset::host::tests::manager_evidence_accepts_captured_systemd_numeric_exit_after_deadline",
        "taira_public_reset::host::tests::manager_evidence_requires_exact_numeric_exit_code_and_status",
        "taira_public_reset::host::tests::manager_evidence_keeps_unexecuted_and_running_operations_pending",
        "taira_public_reset::host::tests::validator_restart_evidence_requires_running_service_and_settled_job",
        "taira_public_reset::host::tests::validator_process_readiness_waits_for_launcher_then_daemon",
        "taira_public_reset::host::tests::validator_process_readiness_preserves_original_deadline",
        "taira_public_reset::host::tests::validator_process_readiness_rejects_changed_launcher_immediately",
    )),
    ("read-only host preflight", (
        "taira_public_reset::host::tests::preflight_dispatches_five_read_only_hosts_without_runtime_custody",
    )),
    ("candidate qualification before edge cutover", (
        "taira_public_reset::host::tests::public_reset_convergence_waits_for_first_commit_without_accepting_pending_proof",
        "taira_public_reset::host::tests::public_reset_convergence_waits_for_applied_successor_before_canary",
        "taira_public_reset::host::tests::public_reset_convergence_rejects_fatal_identity_during_startup",
        "taira_public_reset::host::tests::public_reset_convergence_deadline_reports_last_public_progress",
        "taira_public_reset::host::tests::public_reset_convergence_accepts_same_decision_across_certificate_rounds",
        "taira_public_reset::host::tests::public_reset_convergence_rejects_changed_execution_or_subject_at_same_height",
        "taira_public_reset::host::tests::public_reset_convergence_rejects_omitted_nullable_status_fields",
        "taira_public_reset::host::tests::convergence_wave_receipt_rejects_unknown_first_release_fields",
        "taira_public_reset::executor_model::tests::candidate_qualification_completes_before_public_cutover",
        "taira_public_reset::executor_model::tests::candidate_failure_never_exposes_the_public_edge",
        "taira_public_reset::executor_model::tests::candidate_probe_origins_reject_cross_host_or_substituted_sockets",
        "taira_public_reset::executor_model::tests::public_verification_failure_rolls_back_edge_before_validators",
        "taira_public_reset::executor_model::tests::every_classifier_reachable_recovery_phase_reopens_with_exact_cursor",
        "taira::tests::candidate_inrou_qualifies_runtime_before_public_discovery_exists",
        "taira::tests::candidate_inrou_scope_rejects_remote_or_implicit_probe_destinations",
        "taira::tests::inrou_check_separates_selected_status_origin_from_public_route_origin",
        "taira_public_reset::host::tests::candidate_client_fd_preserves_signer_and_expires_with_child_custody",
        "taira_public_reset::host::tests::candidate_probe_host_key_rejects_another_host_before_mutation",
        "taira_public_reset::host::tests::prepared_candidate_write_cannot_be_reinterpreted_as_public_evidence",
        "taira_public_reset::host::tests::typed_write_envelope_producer_reaches_authenticated_host_consumer",
        "taira::tests::prepared_binding_metadata_matches_objects_before_submission_and_after_commit",
        "taira::tests::typed_inrou_envelopes_reach_fd_and_exact_predecessor_consumers",
        "taira::tests::inrou_predecessor_decoder_rejects_unknown_fields_at_every_envelope_layer",
        "taira_public_reset::host::tests::inrou_restart_evidence_binds_ordered_host_and_exact_guest_transition",
        "taira_public_reset::host::tests::prepared_inrou_report_rejects_every_missing_or_extra_v1_field",
        "taira_public_reset::host::tests::readiness_http_server_waits_for_request_bytes_after_accept",
        "taira_public_reset::host::tests::journaled_restart_waits_for_four_http_backends_before_onboarding",
        "taira_public_reset::host::tests::journaled_restart_readiness_preserves_its_pre_restart_deadline",
        "taira_public_reset::host::tests::journaled_restart_readiness_stops_on_expired_authorization_or_ambiguous_restart",
    )),
    ("explicit unresolved testnet abandonment", (
        "taira_public_reset::executor_model::tests::abandonment_admits_original_signed_revision_without_relaxing_current_dispatcher_identity",
        "taira_public_reset::executor_model::tests::abandonment_cli_requires_explicit_flag_digest_and_original_authority",
        "taira_public_reset::executor_model::tests::abandonment_preserves_exact_unresolved_evidence_before_rollback_and_after_crash",
        "taira_public_reset::executor_model::tests::abandonment_rejects_wrong_digest_edge_and_proven_state_without_host_actions",
        "taira_public_reset::executor_model::tests::abandonment_partial_rollback_resumes_only_remaining_hosts_with_original_digest",
    )),
    ("server-prepared transaction confirmation", (
        "taira::tests::prepared_server_confirmation_polls_queued_then_verifies_exact_applied_wire",
        "taira::tests::prepared_server_confirmation_preserves_fixed_failure_and_deadline",
        "taira::tests::prepared_server_confirmation_rejects_malformed_status_without_resubmission",
    )),
    ("stopped owner runtime cleanup", (
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cleanup_releases_only_empty_own_workers_and_replays",
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cleanup_rejects_live_nested_forged_and_replaced_workers",
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cleanup_keeps_barriers_when_process_absence_is_unproven",
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cleanup_lock_rejects_replaced_or_shared_custody",
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cleanup_authority_requires_exact_config_slot",
        "taira_public_reset::host::stopped_runtime::tests::stopped_firewall_accepts_only_exact_crash_cuts",
        "taira_public_reset::host::stopped_runtime::tests::stopped_firewall_rejects_foreign_references_and_rule_drift",
        "taira_public_reset::host::stopped_runtime::tests::stopped_firewall_cleanup_is_exact_and_idempotent",
        "taira_public_reset::host::stopped_runtime::tests::stopped_firewall_stops_after_command_failure_or_snapshot_drift",
        "taira_public_reset::host::stopped_runtime::tests::stopped_firewall_read_only_never_mutates",
    )),
)

if sys.platform == "linux":
    STAGES += (("OpenSSH parent descriptor custody", (
        "taira_public_reset::host::tests::openssh_parent_pinned_inputs_survive_descriptor_sweep_without_network",
    )),)


TORII_STAGES = (("routed onboarding and faucet contracts", (
    "accounts_faucet::accounts_faucet_accepts_alias_selector_config",
    "accounts_faucet::accounts_faucet_adds_amount_to_prefunded_accounts",
    "accounts_faucet::accounts_faucet_allows_repeated_claims_for_same_account",
    "accounts_faucet::accounts_faucet_puzzle_exposes_current_anchor",
    "accounts_faucet::accounts_faucet_puzzle_raises_difficulty_after_recent_claim",
    "accounts_faucet::accounts_faucet_registers_missing_account_before_transfer",
    "accounts_faucet::accounts_faucet_rejects_missing_pow_when_required",
    "accounts_faucet::accounts_faucet_transfers_starter_balance_to_empty_account",
    "accounts_faucet::faucet_account_fixture_uses_checked_ed25519_key_generation",
    "accounts_faucet::faucet_block_leader_fixture_uses_checked_bls_key_generation",
    "accounts_faucet::faucet_prepared_envelope_survives_pow_anchor_aging",
    "accounts_faucet::faucet_submit_rejects_old_and_tampered_shapes_and_deduplicates_exact_replay",
    "accounts_onboard::expired_onboarding_envelope_only_reconciles_an_already_known_hash",
    "accounts_onboard::sponsored_onboarding_catalog_contains_plan_prepare_submit_and_readiness",
    "accounts_onboard::sponsored_onboarding_fresh_receipt_and_submit_work_after_idle_anchor",
    "accounts_onboard::sponsored_onboarding_prepare_is_non_mutating_and_exact_submit_is_replay_safe",
    "accounts_onboard::sponsored_onboarding_receipt_binds_exact_network_and_active_signer",
    "accounts_onboard::sponsored_onboarding_receipt_rejects_genesis_and_retired_network_keys",
    "accounts_onboard::sponsored_onboarding_rejects_signed_expired_receipt_without_block_progress",
    "accounts_onboard::sponsored_onboarding_stale_create_receipt_returns_redacted_conflict",
    "accounts_onboard::sponsored_onboarding_submit_rejects_old_and_tampered_envelopes",
)),)

CRYPTO_STAGES = (("puzzle cancellation and exact solution predicate", (
    "soranet::puzzle::tests::mint_cancellation_stops_before_first_evaluation",
    "soranet::puzzle::tests::mint_cancellation_discards_inflight_solutions_and_stops_search",
    "soranet::puzzle::tests::mint_and_verify_ticket",
    "soranet::puzzle::tests::invalid_solution_rejected",
    "soranet::puzzle::tests::mint_reanchors_each_candidate_across_long_search",
    "soranet::puzzle::tests::mint_discards_valid_candidate_that_completed_below_ttl_floor",
)),)

P2P_STAGES = (("bounded peer authentication and validator retry ownership", (
    "peer::run::tests::peer_run_authentication_deadline_precedes_long_idle_and_retires_exact_connection",
    "peer::handshake_config_tests::puzzle_work_is_offloaded_serialized_and_remains_bounded_after_cancellation",
    "peer::handshake_config_tests::authentication_deadline_cancels_puzzle_work_without_releasing_inflight_memory",
    "peer::handshake_config_tests::puzzle_work_gate_bounds_concurrency_and_keeps_the_async_runtime_responsive",
    "peer::handshake_config_tests::inbound_puzzle_pressure_cannot_consume_outbound_recovery_capacity",
    "peer::handshake_config_tests::closed_puzzle_work_gate_fails_closed_without_running_work",
    "peer::handshake_config_tests::inbound_puzzle_verification_accepts_a_fresh_valid_ticket",
    "peer::handshake_config_tests::inbound_puzzle_verification_rejects_an_invalid_ticket",
    "peer::handshake_config_tests::inbound_puzzle_ticket_expiring_while_queued_is_rejected",
    "network::accept_stream_tests::tls_listener_closes_silent_transport_at_absolute_preauth_deadline",
    "network::accept_stream_tests::tls_source_gate_precedes_global_capacity_and_deadline_releases_it",
    "network::tests::outbound_authentication_lifetime_rejects_unrepresentable_budgets",
    "network::tests::four_validator_full_mesh_has_exactly_six_balanced_initial_dial_owners",
    "network::tests::validator_standby_dials_after_authentication_tenure_despite_long_idle_timeout",
    "network::tests::failed_pre_handshake_dial_retains_exact_backoff_retry_owner",
    "network::tests::authenticated_session_restart_has_one_immediate_reconnector_and_stable_backup_deadline",
    "network::tests::authenticated_session_cancels_obsolete_standby_attempt_without_reschedule_loop",
)),)

CORE_STAGES = (("consensus scheduling and multi-route progress", (
    "sumeragi::v2_runner::tests::finalized_rollover_drains_source_effects_after_handoff_reopens_capacity",
    "sumeragi::v2_runner::tests::terminal_finalization_limits_open_ingress_to_lane_preflight_before_the_finite_closed_drain",
    "sumeragi::v2_lifecycle_coordinator::launch::tests::pending_kura_actor_backpressure_reaches_durable_rollover_after_closed_prefix",
    "sumeragi::v2_lifecycle_coordinator::launch::tests::pending_kura_mixed_decision_fetch_services_older_cold_output_before_producer_turn",
    "sumeragi::lane_planner::tests::autonomous_reservation_retries_only_transient_planning_failures",
    "sumeragi::v2_effects::tests::decided_apply_retries_after_exact_merge_sidecar_recovery",
    "sumeragi::v2_worker::tests::deferred_apply_retry_full_queue_preserves_output_and_exact_task",
    "sumeragi::v2_worker::tests::deferred_apply_retry_disconnected_or_conflicting_queue_fails_closed",
    "sumeragi::v2_lane_work::tests::completed_merge_sidecar_stays_ready_until_retry_admission_acknowledged",
    "sumeragi::v2_lane_work::tests::autonomous_producer_retains_reservations_until_participant_predecessor_repair",
    "sumeragi::v2_lane_work::tests::autonomous_producer_retains_reserved_batch_until_coordinator_predecessor_repair",
    "sumeragi::v2_lane_work::tests::queue_plan_nonleader_handoff_targets_frozen_leader_with_exact_bytes",
    "sumeragi::v2_lane_work::tests::queue_plan_leader_stages_exact_handoff_idempotently",
    "sumeragi::v2_lane_work::tests::queue_plan_exact_marker_retains_certificate_until_transaction_application",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_retains_future_but_rejects_nonleader_stale_conflict_and_corrupt",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_retires_future_after_current_source_incarnation_drifts",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_cursor_rotates_under_effect_pressure",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_preserves_fresh_admission_before_height_adapter_rollover",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_preserves_materialized_fifo_before_height_adapter_rollover",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_retains_new_admission_while_worker_height_is_obsolete",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_rearms_for_new_view_without_an_arrival_notification",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_new_inventory_preserves_prior_exact_transfers",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_stale_generation_cannot_complete_a_new_destination",
    "sumeragi::v2_lane_work::tests::queue_plan_handoff_is_not_retired_by_unrelated_merge_broadcast_cleanup",
    "sumeragi::v2_lane_work::tests::candidate_provider_admits_ordinary_work_in_multiroute_world_and_excludes_queue_plan_synced",
    "sumeragi::v2_lane_work::tests::candidate_provider_anchors_pending_autonomous_payload_and_defers_queue_conflict",
    "fastpq::lane::tests::persisted_proof_encoding_is_canonical_bounded_and_digest_bound",
)),)

CORE_STAGES += (("durable output capacity and strict handoff", (
    "sumeragi::v2_worker::tests::final_exact_output_seal_is_one_shot_and_blocks_late_enqueue",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
    "sumeragi::v2_worker::tests::applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
    "sumeragi::v2_worker::tests::independent_applied_handoff_releases_covered_states_and_retains_lane_owners",
    "sumeragi::v2_worker::tests::independent_applied_handoff_retains_active_historical_recovery_request",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_unbound_lane_output_atomically",
    "sumeragi::v2_worker::tests::autonomous_payload_carrier_comparison_promotes_only_a_missing_advisory_hint",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_wrong_height_global_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_historical_kura_global_responses_atomically",
    "sumeragi::v2_worker::tests::prepared_historical_body_retries_after_exact_output_capacity_rejection",
    "sumeragi::v2_worker::tests::prepared_historical_body_capacity_recovers_from_applied_finality_without_peer_delivery",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
    "sumeragi::v2_worker::tests::applied_height_handoff_authenticates_exact_payload_chunk_fanout",
    "sumeragi::v2_worker::tests::production_exact_output_observes_finality_only_after_state_commit",
    "sumeragi::v2_worker::tests::applied_height_finality_releases_only_ticketless_global_topology_target",
    "sumeragi::v2_worker::tests::applied_height_finality_releases_only_covered_ticketless_payload_chunks",
    "sumeragi::v2_worker::tests::terminal_retry_revalidates_exact_kura_advert_before_retiring_ranked_output",
    "sumeragi::v2_worker::tests::terminal_retry_revalidates_exact_kura_queue_plan_admission_before_retiring_ranked_output",
    "sumeragi::v2_worker::tests::closed_flush_racing_final_receiver_retirement_is_nonfatal",
)),)

CLIENT_STAGES = (("public contract SDK envelope", (
    "client::evidence_http_tests::post_contract_call_accepts_only_the_caller_trusted_draft_intent",
    "client::evidence_http_tests::post_contract_call_authenticates_bound_account_and_rejects_foreign_authority",
    "client::evidence_http_tests::post_contract_call_rejects_ordinary_draft_before_signing_or_submission",
    "client::evidence_http_tests::post_contract_call_rejects_substituted_operation_receipt",
    "client::evidence_http_tests::post_contract_call_rejects_omitted_operation_receipt_fields",
    "client::evidence_http_tests::post_contract_call_rejects_unsupported_response_root_fields",
    "client::evidence_http_tests::post_contract_call_rejects_omitted_response_root_fields",
)),)

TORII_UNIT_STAGES = (("public contract retained payload and certified ingress", (
    "routing::multisig_selector_tests::contract_call_detached_submission_retains_exact_queue_plan_payload",
    "routing::multisig_selector_tests::contract_call_detached_submission_preserves_retained_fee_limits_without_requote",
    "routing::multisig_selector_tests::contract_call_detached_submission_rejects_changed_or_noncanonical_payload",
    "routing::multisig_selector_tests::contract_call_detached_handler_requires_certified_public_admission",
    "routing::multisig_selector_tests::contract_call_detached_submission_requires_complete_retained_envelope",
    "routing::multisig_selector_tests::contract_call_prepare_serializes_complete_canonical_response",
    "openapi::tests::public_contract_call_schema_matches_exact_queue_plan_handoff",
    "openapi::tests::checked_openapi_assets_match_package_authority",
)),)

TORII_STAGES += (("public contract HTTP preparation and strict admission", (
    "contracts_call_integration::contracts_call_prepares_exact_payload_and_requires_certified_admission",
)),)

CORE_STAGES += (("authenticated admission and coherent State publication", (
    "state::tests::pending_queue_plan_authentication_does_not_hold_the_publication_fence",
    "state::tests::pending_queue_plan_admission_accepts_unchanged_source_after_height_only_advance",
    "state::tests::pending_queue_plan_admission_is_future_until_its_canonical_frontier_arrives",
    "state::tests::pending_queue_plan_admission_checks_historical_predecessor_roster_and_incarnation",
    "state::tests::pending_queue_plan_admission_checks_historical_native_amx_participant_sources",
    "state::tests::pending_queue_plan_persistence_serializes_alternate_quorum_subsets",
    "state::tests::pending_queue_plan_persistence_yields_to_one_ahead_state_publication",
    "state::tests::pending_queue_plan_persistence_bounds_one_ahead_wait_and_rejects_larger_skew",
    "state::tests::pending_queue_plan_old_carrier_retains_only_valid_current_sources",
    "state::tests::pending_queue_plan_admission_defers_obsolete_carrier_without_rejecting_current_source",
    "state::tests::queue_plan_conflict_requires_pending_or_applied_owner_evidence",
    "state::tests::queue_plan_carrier_validation_uses_one_generation_coherent_state_view",
)),)

PROOF_STAGES = (("canonical proof resource bounds", (
    "proof::tests::default_resource_profile_covers_canonical_opening_shapes_and_wire_frames",
    "proof::tests::raw_fixture_verifier_preserves_explicit_admission_limits",
    "proof::tests::enforce_verify_limits_allows_values_at_exact_boundaries",
    "proof::tests::verify_limits_reject_oversized_proof_payload",
)),)

PROOF_FLOW_STAGES = (("default proof production and verification", (
    "resource_profile::public_transfer_default_profile_accepts_eight_rows",
    "resource_profile::public_transfer_default_profile_accepts_sixteen_rows",
)),)

CONFIG_STAGES = (("production configuration schema", (
    "lane_descriptor_collection_defaults_match_config_defaults",
    "lane_descriptor_collection_defaults_reject_malformed_values",
    "taira_profile_nexus_collections_deserialize_without_runtime_inputs",
    "nexus_routing_and_governance_collection_defaults_match_config_defaults",
)),)

TEST_NETWORK_STAGES = (("isolated validator fixture configuration", (
    "config::tests::base_config_applies_bounded_storage_caps",
    "config::tests::base_config_preserves_caller_storage_budget_and_smaller_component_cap",
    "tests::peer_client_ignores_ambient_identity_and_endpoint_overrides",
)),)

NETWORK_STAGES = (("four-validator multi-route transaction commit", (
    "four_peer_multiroute_public_transaction_reaches_applied",
)),)

# Four peers use the shared test-network 1 GiB/node cap. Keep another 4 GiB
# available for fixture logs, temporary files and concurrent build output.
NETWORK_FIXTURE_FREE_BYTES = 8 * 1024**3

HARNESS_TARGETS = {
    "config": ("native configuration contracts", "taira_config_contracts", "test", ["-p", "iroha_config", "--test", "taira_config_contracts"]),
    "cli": ("native CLI", "iroha", "bin", ["-p", "iroha_cli", "--bin", "iroha"]),
    "crypto": ("native puzzle cryptography", "iroha_crypto", "lib", ["-p", "iroha_crypto", "--lib"]),
    "p2p": ("native peer transport", "iroha_p2p", "lib", ["-p", "iroha_p2p", "--lib"]),
    "torii": ("native Torii contracts", "taira_app_contracts", "test", ["-p", "iroha_torii", "--test", "taira_app_contracts"]),
    "client": ("native Rust SDK", "iroha", "lib", ["-p", "iroha", "--lib"]),
    "torii-unit": ("native Torii envelope contracts", "iroha_torii", "lib", ["-p", "iroha_torii", "--lib"]),
    "core": ("native Core", "iroha_core", "lib", ["-p", "iroha_core", "--lib"]),
    "proof": ("native proof bounds", "fastpq_prover", "lib", ["-p", "fastpq_prover", "--lib"]),
    "proof-flows": ("native proof flows", "fastpq_integration", "test", ["-p", "fastpq_prover", "--test", "fastpq_integration"]),
    "test-network": ("native validator fixture configuration", "iroha_test_network", "lib", ["-p", "iroha_test_network", "--lib"]),
    "network": ("native consensus contracts", "taira_consensus_contracts", "test", ["-p", "iroha_test_network", "--test", "taira_consensus_contracts"]),
}


class CheckError(Exception):
    """A build or selected regression did not pass."""


def selected_regression_count() -> int:
    """Return the complete native census shared by execution and result capture."""
    return sum(len(names)
               for stages in (STAGES, CONFIG_STAGES, CRYPTO_STAGES, P2P_STAGES, CORE_STAGES,
                              TEST_NETWORK_STAGES, NETWORK_STAGES, PROOF_STAGES,
                              PROOF_FLOW_STAGES, TORII_STAGES, CLIENT_STAGES, TORII_UNIT_STAGES)
               for _, names in stages)


def compile_command(root: Path, env: dict[str, str], *, harness: str = "cli") -> list[str]:
    if harness not in HARNESS_TARGETS:
        raise CheckError("invalid native regression harness selection")
    return _compile_command(root, env, HARNESS_TARGETS[harness][3])


def _compile_command(root: Path, env: dict[str, str], selection: list[str]) -> list[str]:
    return [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "test",
            "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
            *selection, "--no-run",
            "--message-format=json-render-diagnostics"]


def test_artifact(line: str, *, harness: str = "cli") -> str | None:
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        return None  # The accelerator wrapper also emits ordinary progress.
    if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
        return None
    target = event.get("target", {})
    _, name, kind, _ = HARNESS_TARGETS[harness]
    if (target.get("name") == name and kind in target.get("kind", [])
            and event.get("profile", {}).get("test") is True):
        executable = event.get("executable")
        if isinstance(executable, str) and executable:
            return executable
    return None


def show_build_diagnostic(line: str) -> None:
    """Keep Cargo's rendered compiler errors visible while consuming JSON events."""
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        sys.stdout.write(line)  # Preserve accelerator progress, not raw JSON events.
        sys.stdout.flush()
        return
    if isinstance(event, dict) and event.get("reason") == "compiler-message":
        message = event.get("message")
        rendered = message.get("rendered") if isinstance(message, dict) else None
        if isinstance(rendered, str):
            sys.stderr.write(rendered)
            sys.stderr.flush()


def compile_harness(root: Path, env: dict[str, str], *, lock_fds: tuple[int, ...] = (),
                    harness: str = "cli") -> str:
    command = compile_command(root, env, harness=harness)
    return _build_harnesses(root, command, env, (harness,), lock_fds)[harness]


def compile_test_harnesses(root: Path, env: dict[str, str], *,
                          harnesses: tuple[str, ...],
                          lock_fds: tuple[int, ...] = ()) -> dict[str, str]:
    """Build selected library and integration harnesses with one feature graph."""
    if not harnesses or len(harnesses) != len(set(harnesses)):
        raise CheckError("native test batch requires distinct harness selections")
    packages: list[str] = []
    targets: list[str] = []
    for harness in harnesses:
        target = HARNESS_TARGETS.get(harness)
        if target is None:
            raise CheckError("invalid native regression harness selection")
        _, name, kind, arguments = target
        if kind == "lib" and len(arguments) == 3 and arguments == ["-p", arguments[1], "--lib"]:
            if "--lib" not in targets:
                targets.append("--lib")
        elif kind == "test" and len(arguments) == 4 and arguments == ["-p", arguments[1], "--test", name]:
            targets.extend(["--test", name])
        else:
            raise CheckError("native test batch requires explicit library or integration targets")
        if arguments[1] not in packages:
            packages.append(arguments[1])
    selection = [argument for package in packages for argument in ("-p", package)]
    command = _compile_command(root, env, [*selection, *targets])
    return _build_harnesses(root, command, env, harnesses, lock_fds)


def _build_harnesses(root: Path, command: list[str], env: dict[str, str],
                     harnesses: tuple[str, ...], lock_fds: tuple[int, ...]) -> dict[str, str]:
    label = "; ".join(HARNESS_TARGETS[harness][0] for harness in harnesses)
    print(f"[taira-check] build {label} test harness", flush=True)
    started = time.monotonic()
    artifacts: dict[str, set[str]] = {harness: set() for harness in harnesses}
    records: dict[str, dict[str, object]] = {}
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds) as child:
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            for harness in harnesses:
                artifact = test_artifact(line, harness=harness)
                if artifact is not None:
                    artifacts[harness].add(artifact)
                    record = native_artifact_record(json.loads(line))
                    if (harness in records and records[harness]["executable"] == artifact
                            and records[harness] != record):
                        raise CheckError("native harness has conflicting Cargo metadata")
                    records[harness] = record
        code = child.wait()
    elapsed = time.monotonic() - started
    if code:
        raise CheckError(f"{label} build failed (exit {code}, {elapsed:.1f}s)")
    for harness, executables in artifacts.items():
        if len(executables) != 1:
            raise CheckError(f"{HARNESS_TARGETS[harness][0]} build reported "
                             f"{len(executables)} test executables; expected one")
    result = {harness: next(iter(executables)) for harness, executables in artifacts.items()}
    if len(set(result.values())) != len(result):
        raise CheckError("native build reused one executable for distinct test harnesses")
    print(f"[taira-check] {label} build passed in {elapsed:.1f}s", flush=True)
    return isolate_native_artifacts(root, env, records)


NATIVE_ARTIFACT_MAX_BYTES = 4 * 1024**3


def native_artifact_record(event: dict[str, object]) -> dict[str, object]:
    """Retain Cargo metadata independently from the immutable execution path."""
    return {"name": event["target"]["name"], "executable": event["executable"],
            "profile": event["profile"], "manifest_path": event.get("manifest_path")}


@contextlib.contextmanager
def native_artifact_guard(root: Path, target: Path, env: dict[str, str]):
    """Lock actual Cargo outputs only after Cargo exits, through source validation and copy."""
    if root.is_relative_to(target):
        from taira_cargo_cache import local_package_names, source_fingerprints
        # Metadata has no artifact authority and must run before acquiring Cargo's locks.
        packages = local_package_names(root, env)
        with source_fingerprints(root, target, "aarch64-unknown-linux-gnu", packages, repair=False):
            yield
        return
    # Mutable development checks cannot claim captured-source fingerprint authority.
    # They still execute private copies and exclude Cargo writers while copying.
    profile = target / "debug"
    if profile.resolve(strict=True) != profile:
        raise CheckError("native Cargo profile must not traverse symlinks")
    fd = os.open(profile / ".cargo-lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                or info.st_nlink != 1 or info.st_mode & 0o022):
            raise CheckError("unsafe native Cargo profile lock")
        fcntl.flock(fd, fcntl.LOCK_EX)
        yield
    finally:
        os.close(fd)


def isolate_native_artifacts(root: Path, env: dict[str, str],
                             records: dict[str, dict[str, object]]) -> dict[str, str]:
    """Execute copied artifacts, never mutable Cargo paths returned by an earlier build."""
    from release_artifact_contract import ReleaseArtifactError, stable_hash_path, stable_open_relative
    target = Path(env["CARGO_TARGET_DIR"])
    try:
        if not records or any(key not in HARNESS_TARGETS and key not in {"iroha3d", "iroha"} for key in records):
            raise CheckError("native artifact isolation requires known nonempty selections")
        for directory in (root, target):
            info = directory.stat()
            if (not directory.is_absolute() or directory.resolve(strict=True) != directory
                    or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
                    or info.st_mode & 0o022):
                raise CheckError("native artifact source and target must be direct owner-held directories")
        paths = {}
        for selection, record in records.items():
            package = {"iroha3d": "irohad", "iroha": "iroha_cli"}.get(selection)
            if package is None:
                package = HARNESS_TARGETS[selection][3][1]
            if record["manifest_path"] != str(root / "crates" / package / "Cargo.toml"):
                raise CheckError("native Cargo artifact manifest differs from the selected source")
            path = Path(record["executable"])
            if (not path.is_absolute() or path.resolve(strict=True) != path
                    or not path.is_relative_to(target / "debug")):
                raise CheckError("native Cargo artifact must be a direct path below the selected debug target")
            paths[selection] = path
        with native_artifact_guard(root, target, env):
            identities = {}
            for selection, path in paths.items():
                info = path.lstat()
                if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                        or not info.st_mode & stat.S_IXUSR or info.st_mode & 0o022
                        or info.st_nlink != 1 or not 0 < info.st_size <= NATIVE_ARTIFACT_MAX_BYTES):
                    raise CheckError("native Cargo artifact must be a bounded owner-held executable without hardlinks")
                identities[selection] = stable_hash_path(path, max_size=NATIVE_ARTIFACT_MAX_BYTES)
            required = sum(info.size for info in identities.values()) + NETWORK_FIXTURE_FREE_BYTES
            if shutil.disk_usage(target).free < required:
                raise CheckError("native artifact copies would consume the required working-space reserve")
            output = Path(tempfile.mkdtemp(prefix="taira-native-artifacts-", dir=target))
            copied, observations, published = {}, [], {}
            for selection, path in paths.items():
                expected = identities[selection]
                destination = output / selection
                digest, size = hashlib.sha256(), 0
                with stable_open_relative(target, str(path.relative_to(target)), expected=expected) as source:
                    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
                    try:
                        while block := os.read(source, 1024 * 1024):
                            size += len(block)
                            if size > expected.size:
                                raise CheckError("native artifact grew during descriptor copy")
                            digest.update(block)
                            view = memoryview(block)
                            while view:
                                written = os.write(fd, view)
                                if written <= 0:
                                    raise CheckError("native artifact copy made no progress")
                                view = view[written:]
                        if size != expected.size or digest.hexdigest() != expected.sha256:
                            raise CheckError("native artifact changed during descriptor copy")
                        os.fchmod(fd, 0o500)
                        os.fsync(fd)
                        opened, named = os.fstat(fd), destination.lstat()
                        if ((opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino)
                                or opened.st_size != expected.size or named.st_size != expected.size
                                or named.st_uid != os.geteuid() or named.st_nlink != 1
                                or stat.S_IMODE(named.st_mode) != 0o500):
                            raise CheckError("native artifact destination changed during copy")
                        published[destination] = (named.st_dev, named.st_ino, named.st_size,
                            named.st_mode, named.st_uid, named.st_nlink, named.st_mtime_ns, named.st_ctime_ns)
                    finally:
                        os.close(fd)
                copied[selection] = str(destination)
                observations.append({"selection": selection, "path": str(destination),
                    "sha256": expected.sha256, "size": expected.size, "cargo_artifact": records[selection]})
            for destination, expected_identity in published.items():
                named = destination.lstat()
                if expected_identity != (named.st_dev, named.st_ino, named.st_size, named.st_mode,
                        named.st_uid, named.st_nlink, named.st_mtime_ns, named.st_ctime_ns):
                    raise CheckError("native artifact destination changed before publication")
            directory_fd = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            try:
                os.fchmod(directory_fd, 0o500)
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
            parent_fd = os.open(target, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            try:
                os.fsync(parent_fd)
            finally:
                os.close(parent_fd)
        # Publish observations only when the complete batch is frozen and the locks released.
        for observation in observations:
            print("[taira-check] isolated native artifact " + json.dumps(observation, sort_keys=True), flush=True)
        return copied
    except (OSError, ValueError, ReleaseArtifactError, subprocess.SubprocessError) as error:
        raise CheckError(f"native artifact isolation failed: {error}") from error


def require_tests(listing: str, stages=None) -> None:
    available = {line.removesuffix(": test") for line in listing.splitlines()
                 if line.endswith(": test")}
    missing = [name for _, names in (STAGES if stages is None else stages) for name in names if name not in available]
    if missing:
        raise CheckError("required regressions missing from native harness: " + ", ".join(missing))


def require_one_pass(name: str, result: subprocess.CompletedProcess[str]) -> None:
    if (result.returncode != 0
            or f"test {name} ... ok" not in result.stdout.splitlines()
            or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in result.stdout):
        # These tests use disposable fixtures, never operator runtime inputs.
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)
        raise CheckError(f"regression did not execute and pass: {name} (exit {result.returncode})")


def run_stages(harness: str, fixture_root: Path, env: dict[str, str], stages,
               lock_fds: tuple[int, ...]) -> None:
    listing = subprocess.run([harness, "--list", "--format", "terse"], cwd=fixture_root,
                             env=env, stdin=subprocess.DEVNULL, text=True, capture_output=True, check=False, pass_fds=lock_fds)
    if listing.returncode:
        raise CheckError(f"cannot list native harness tests (exit {listing.returncode})")
    require_tests(listing.stdout, stages)
    failures = []
    for label, names in stages:
        failed_before = len(failures)
        stage_start = time.monotonic()
        print(f"[taira-check] start {label} ({len(names)} tests)", flush=True)
        for name in names:
            test_start = time.monotonic()
            print(f"[taira-check] start {name}", flush=True)
            result = subprocess.run([harness, name, "--exact", "--color", "never"],
                                    cwd=fixture_root, env=env, stdin=subprocess.DEVNULL,
                                    text=True, capture_output=True, check=False, pass_fds=lock_fds)
            try:
                require_one_pass(name, result)
            except CheckError as error:
                failures.append(str(error))
                print(f"[taira-check] failed {name} ({time.monotonic() - test_start:.1f}s)", flush=True)
            else:
                print(f"[taira-check] passed {name} ({time.monotonic() - test_start:.1f}s)", flush=True)
        outcome = "passed" if len(failures) == failed_before else "failed"
        print(f"[taira-check] {outcome} {label} ({time.monotonic() - stage_start:.1f}s)", flush=True)
    if failures:
        raise CheckError(f"{len(failures)} selected regressions failed: " + "; ".join(failures))


def compile_network_binaries(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> dict[str, str]:
    """Use real Cargo artifacts so the network harness never starts a fallback build."""
    command = [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "build",
               "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
               "-p", "irohad", "--bin", "iroha3d", "-p", "iroha_cli", "--bin", "iroha",
               "--message-format=json-render-diagnostics"]
    print("[taira-check] build native network binaries", flush=True)
    started = time.monotonic()
    artifacts: dict[str, str] = {}
    records: dict[str, dict[str, object]] = {}
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds) as child:
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
                continue
            target = event.get("target", {})
            name = target.get("name")
            executable = event.get("executable")
            if (name in ("iroha3d", "iroha") and "bin" in target.get("kind", [])
                    and event.get("profile", {}).get("test") is False
                    and isinstance(executable, str) and executable):
                if name in artifacts and artifacts[name] != executable:
                    raise CheckError("native network binary has conflicting Cargo artifacts")
                record = native_artifact_record(event)
                if name in records and records[name] != record:
                    raise CheckError("native network binary has conflicting Cargo metadata")
                artifacts[name] = executable
                records[name] = record
                print("[taira-check] native network artifact " + json.dumps(record, sort_keys=True), flush=True)
        code = child.wait()
    if code or set(artifacts) != {"iroha3d", "iroha"}:
        raise CheckError(f"native network build did not produce both executable artifacts (exit {code})")
    print(f"[taira-check] network binary build passed in {time.monotonic() - started:.1f}s", flush=True)
    return isolate_native_artifacts(root, env, records)


def run_config_checks(root: Path, fixture_root: Path, env: dict[str, str],
                      lock_fds: tuple[int, ...]) -> None:
    """Reject schema failures before compiling the Core and network harnesses."""
    if CONFIG_STAGES:
        harness = compile_harness(root, env, lock_fds=lock_fds, harness="config")
        run_stages(harness, fixture_root, env, CONFIG_STAGES, lock_fds)


def run_network_checks(root: Path, fixture_root: Path, env: dict[str, str], lock_fds: tuple[int, ...],
                       *, harness: str) -> None:
    binaries = compile_network_binaries(root, env, lock_fds)
    require_network_fixture_capacity(fixture_root)
    # Keep attempt-owned fixtures and logs for diagnosis; they contain no live inputs.
    directory = Path(tempfile.mkdtemp(prefix="taira-consensus-check-", dir=fixture_root))
    network_env = env | {
        "TEST_NETWORK_BIN_IROHAD": binaries["iroha3d"],
        "TEST_NETWORK_BIN_IROHA": binaries["iroha"],
        "IROHA_TEST_TARGET_DIR": env["CARGO_TARGET_DIR"],
        "TEST_NETWORK_TMP_DIR": str(directory),
        "IROHA_TEST_NETWORK_KEEP_DIRS": "1",
        "IROHA_TEST_SKIP_BUILD": "1",
        "IROHA_FAIL_ON_SANDBOX_SKIP": "1",
        "IROHA_TEST_REQUIRE_NETWORK": "1",
        "IROHA_TEST_SERIALIZE_NETWORKS": "1",
    }
    print(f"[taira-check] consensus fixture logs: {directory}", flush=True)
    run_stages(harness, fixture_root, network_env, NETWORK_STAGES, lock_fds)


def require_network_fixture_capacity(directory: Path) -> None:
    """Reject an undersized shared test volume before building or starting peers."""
    available = shutil.disk_usage(directory).free
    if available < NETWORK_FIXTURE_FREE_BYTES:
        raise CheckError(
            f"four-peer fixtures require {NETWORK_FIXTURE_FREE_BYTES} free bytes "
            f"for bounded storage and scratch space; {available} available at {directory}")


def run_pure_fsm_checks(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> None:
    """Run every production reducer test without Cargo or adapter dependencies."""
    _run_standalone_checks(root, env, lock_fds,
        source="crates/iroha_sumeragi_core/src/lib.rs", output_name="sumeragi-core-tests",
        label="pure FSM", description="pure consensus FSM (exact production reducer)")


def run_lifecycle_source_checks(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> None:
    """Check the same lifecycle source contracts before compiling Core dependencies."""
    _run_standalone_checks(root, env, lock_fds,
        source="crates/iroha_core/src/sumeragi/v2_lifecycle_source_contract_harness.rs",
        output_name="lifecycle-source-tests", label="lifecycle source contracts",
        description="lifecycle source contracts (shared Core assertions)")


def _run_standalone_checks(root: Path, env: dict[str, str], lock_fds: tuple[int, ...], *,
                           source: str, output_name: str, label: str, description: str) -> None:
    compiler = env.get("RUSTC")
    if not compiler or not Path(compiler).is_absolute():
        raise CheckError(f"{label} checks require the coordinated pinned RUSTC")
    target = Path(env["CARGO_TARGET_DIR"])
    output = target / "taira-consensus-fsm-check"
    output.mkdir(mode=0o700, exist_ok=True)
    if output.is_symlink() or not output.is_dir():
        raise CheckError(f"{label} output must be a direct directory in the existing target")
    executable = output / output_name
    if executable.is_symlink():
        raise CheckError(f"{label} executable cannot be a symlink")
    started = time.monotonic()
    print(f"[taira-check] start {description}", flush=True)
    common = dict(cwd="/", env=env, stdin=subprocess.DEVNULL, text=True,
                  capture_output=True, check=False, pass_fds=lock_fds, timeout=120)
    compiled = subprocess.run([compiler, "--edition=2024", "--test",
        str(root / source), "-o", str(executable)], **common)
    if compiled.returncode:
        sys.stderr.write(compiled.stdout + compiled.stderr)
        raise CheckError(f"{label} compilation failed (exit {compiled.returncode})")
    listing = subprocess.run([str(executable), "--list", "--format", "terse"], **common)
    lines = listing.stdout.splitlines()
    names = [line.removesuffix(": test") for line in lines if line.endswith(": test")]
    if (listing.returncode or not names or len(names) != len(set(names))
            or len(names) != len(lines) or any(not name for name in names)):
        raise CheckError(f"{label} test census is missing, duplicated, or malformed")
    result = subprocess.run([str(executable), "--color", "never", "--test-threads=6"], **common)
    passed = [line.removeprefix("test ").removesuffix(" ... ok")
              for line in result.stdout.splitlines()
              if line.startswith("test ") and line.endswith(" ... ok")]
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [^\n]+$",
        result.stdout, re.MULTILINE)
    if (result.returncode or len(passed) != len(names) or set(passed) != set(names)
            or summaries != [str(len(names))]):
        sys.stderr.write(result.stdout + result.stderr)
        raise CheckError(f"{label} suite did not execute every listed test successfully without skips")
    print(f"[taira-check] {label} PASS: {len(names)} listed, {len(passed)} passed, 0 ignored "
          f"in {time.monotonic() - started:.1f}s", flush=True)


def run_checks(root: Path, *, environment: dict[str, str] | None = None,
               source_commit: str | None = None, lock_fds: tuple[int, ...] = ()) -> None:
    if sys.platform not in {"darwin", "linux"}:
        raise CheckError("the Taira descriptor/stage gate requires macOS or Linux")
    started = time.monotonic()
    if environment is None or not all(environment.get(name) for name in ("CARGO", "CARGO_HOME", "CARGO_TARGET_DIR")):
        raise CheckError("checks require the coordinated isolated Cargo environment; use either check CLI")
    env = dict(environment)
    head = source_commit if source_commit is not None else subprocess.check_output(
        ["git", "--no-replace-objects", "rev-parse", "HEAD"], cwd=root, env=env,
        stdin=subprocess.DEVNULL, text=True).strip()
    env.pop("CARGO_BUILD_TARGET", None)  # This check executes a host-native harness.
    env["VERGEN_GIT_SHA"] = head
    env["IROHA_GIT_COMMIT_HASH"] = head
    print(f"[taira-check] source {head}; {root}", flush=True)
    fixture_root = Path(env["CARGO_TARGET_DIR"]) if source_commit is not None else root
    if NETWORK_STAGES:
        require_network_fixture_capacity(fixture_root)
    run_pure_fsm_checks(root, env, lock_fds)
    run_lifecycle_source_checks(root, env, lock_fds)
    run_config_checks(root, fixture_root, env, lock_fds)
    # Build all early library/HTTP checks and the network harness in one graph.
    # Adding named test targets from these same packages preserves default and
    # dev-dependency feature unification. Node/CLI binaries wait for early checks.
    early_stages = tuple((name, stages) for name, stages in (
        ("crypto", CRYPTO_STAGES), ("p2p", P2P_STAGES), ("core", CORE_STAGES),
        ("test-network", TEST_NETWORK_STAGES),
        ("client", CLIENT_STAGES), ("torii-unit", TORII_UNIT_STAGES),
        ("torii", TORII_STAGES)) if stages)
    selections = tuple(name for name, _ in early_stages)
    if NETWORK_STAGES:
        selections += ("network",)
    if selections:
        harnesses = compile_test_harnesses(root, env, lock_fds=lock_fds,
                                          harnesses=selections)
        for name, stages in early_stages:
            run_stages(harnesses[name], fixture_root, env, stages, lock_fds)
        if NETWORK_STAGES:
            run_network_checks(root, fixture_root, env, lock_fds, harness=harnesses["network"])
    harness = compile_harness(root, env, lock_fds=lock_fds)
    run_stages(harness, fixture_root, env, STAGES, lock_fds)
    for name, stages in (("proof", PROOF_STAGES),
                         ("proof-flows", PROOF_FLOW_STAGES)):
        if stages:
            selected_harness = compile_harness(root, env, lock_fds=lock_fds, harness=name)
            run_stages(selected_harness, fixture_root, env, stages, lock_fds)
    if source_commit is None and subprocess.check_output(["git", "--no-replace-objects", "rev-parse", "HEAD"], cwd=root, env=env,
                               stdin=subprocess.DEVNULL, text=True).strip() != head:
        raise CheckError("HEAD changed during checks; rerun against the intended source")
    print(f"[taira-check] PASS: {selected_regression_count()} regressions in {time.monotonic() - started:.1f}s", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="repository root (default: this maintained script's parent repository)")
    parser.add_argument("--target-dir", type=Path, help="existing development Cargo lane (default: sibling routine lane)")
    args = parser.parse_args()
    # Lazy import keeps the low-level gate loadable from an authenticated source capture.
    import taira_release as release
    try:
        release.development_check(args.repo_root, args.target_dir, dict(os.environ))
    except (CheckError, release.PrepareError, release.ReleaseArtifactError,
            release.gate.CheckError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"[taira-check] FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
