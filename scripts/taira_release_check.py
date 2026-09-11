#!/usr/bin/env python3
"""Qualify basic Taira connectivity, or the full regression census, before a build.

Requires Python 3.11+ and the repository Rust toolchain. Compile focused native
harnesses and run a four-peer network with isolated Cargo and fixture-only inputs.
The existing sibling .taira-testnet-build-targets/routine lane is the default;
--target-dir or TAIRA_TESTNET_CARGO_TARGET_DIR may select another development
lane. Both selectors must agree when supplied. No Cargo lane is created or cleaned.
Native checks retain incremental compilation unless CARGO_INCREMENTAL=0 is
explicitly selected. This preference never changes Linux release compilation.
Private test-executable copies are released after their last subprocess exits,
including failed checks; their observations and test logs remain available.
Native node/client snapshots remain retained for network and CLI capture consumers.
On macOS, descriptor-bound copy-on-write clones avoid full duplicate allocation
while retaining independent inodes and exact content/stat validation. Unsupported
filesystems stream only when all remaining copies fit beside the working reserve;
later Cargo writes can still allocate new blocks for changed cloned content.
The default basic scope keeps deployment custody, authentication, application and
startup admission checks plus real four-validator Applied transactions and restart.
Full additionally executes advanced Core recovery and proof-production matrices.
Both scopes compile the same native graph; no runtime security policy is relaxed.

Configuration and compiler paths match authenticated preparation, while source
remains the mutable checkout. These checks never qualify release artifacts and
accept no live configuration, credentials, SSH, deployment or signing inputs.
"""

from __future__ import annotations

import argparse
import ast
import contextlib
import ctypes
import errno
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
import tomllib


STAGES = (
    ("core canary command composition", (
        "taira_public_reset::host::tests::coordinator_write_canary_argv_passes_child_validation_for_all_core_actions",
        "taira::tests::final_canary_predecessor_requires_its_independent_faucet_policy",
        "taira::tests::write_canary_policy_inputs_are_operation_and_action_scoped",
    )),
    ("submitted canary recovery state machine", (
        "taira_public_reset::executor_model::tests::never_attempted_next_mutation_preserves_authorized_continuation",
        "taira_public_reset::executor_model::tests::recovered_partial_mutation_reopens_and_dispatches_only_prepared_suffix",
        "taira_public_reset::executor_model::tests::partial_mutation_continuation_rejects_expired_forward_authorization",
        "taira_public_reset::executor_model::tests::submitted_child_failures_preserve_parent_intent_until_read_only_recovery",
        "taira_public_reset::executor_model::tests::authenticated_submitted_child_rejection_remains_terminal",
        "taira_public_reset::host::tests::submitted_child_process_failures_require_read_only_recovery",
        "taira_public_reset::host::tests::interrupted_onboarding_proof_recovers_from_its_authenticated_prepared_envelope",
        "taira_public_reset::host::tests::retained_proof_required_pending_report_accepts_only_live_state_classes",
    )),
    ("complete prepared canary transport lifecycle", (
        "taira::tests::final_canary_submit_uses_original_deadline_after_initial_read_and_post",
        "taira::tests::final_canary_submit_verifies_exact_proof_without_replaying_post",
        "taira::tests::faucet_preparation_deadline_stops_http_and_cpu_work_before_dispatch",
        "taira::tests::core_pending_reason_codec_is_closed_and_round_trips_every_variant",
        "taira_public_reset::host::tests::every_core_pending_report_variant_reaches_the_exact_host_consumer",
        "taira_public_reset::host::tests::core_terminal_reports_map_to_exact_executor_recovery_classes",
    )),
    ("explicit core testnet qualification", (
        "taira_public_reset::executor_model::tests::qualification_scope_is_required_and_canonical_in_all_authority_documents",
        "taira_public_reset::executor_model::tests::qualification_scope_is_bound_before_normal_and_recovery_signature_admission",
        "taira_public_reset::executor_model::tests::qualification_scope_is_immutable_in_recovery_and_reported_explicitly",
        "taira_public_reset::host::tests::core_testnet_scope_preserves_baseline_recovery_and_host_plan",
        "taira_public_reset::host::tests::restart_recovery_reconstructs_only_the_final_frontier_receipt",
        "taira_public_reset::host::tests::cohost_mutation_boundaries_share_the_complete_plan_and_lock_namespace",
    )),
    ("public doctor producer and deployment contract", (
        "taira::tests::doctor_basic_scope_accepts_unsynchronized_time_and_excludes_advanced_routes",
        "taira::tests::doctor_tools_list_consumes_pages_and_rejects_invalid_cursors",
        "taira::tests::doctor_reports_bounded_mcp_application_error_codes",
        "taira::tests::doctor_mock_healthy_flow_reports_ok",
        "taira::tests::time_snapshot_requires_network_time_and_every_health_axis",
        "taira::tests::doctor_rejects_unknown_namespaces_or_malformed_mcp_tools",
        "taira::tests::doctor_mock_required_tool_missing_reports_failure",
        "taira_public_reset::host::tests::doctor_report_requires_the_exact_first_release_check_surface",
    )),
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
        "taira_public_reset::host::tests::client_network_identity_rejects_wrong_chain_genesis_and_discriminant",
        "taira_public_reset::host::tests::pinned_client_inventory_loader_rejects_wrong_generation_without_child_custody",
    )),
    ("native validator config preparation", (
        "taira_public_reset::config::tests::config_rebase_network_identity_uses_exact_cas_and_preserves_other_config",
        "taira_public_reset::config::tests::config_rebase_network_identity_rejects_competing_and_noncanonical_sources",
        "taira_public_reset::config::tests::config_rebase_network_identity_cli_requires_paired_checked_values",
        "taira_public_reset::config::tests::client_config_rebase_network_identity_uses_exact_cas_and_preserves_other_config",
        "taira_public_reset::config::tests::client_config_rebase_inherited_fd_preserves_custody_and_has_no_stdout",
        "taira_public_reset::config::tests::client_config_rebase_rejects_drift_and_unsafe_custody_before_output",
        "taira_public_reset::inputs::tests::validator_pin_fee_asset_must_match_the_typed_faucet_funding_asset",
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
        "taira_public_reset::host::tests::prepared_child_rejects_failed_exit_even_with_authenticated_applied_report",
        "taira_public_reset::host::tests::prepared_child_failure_reports_matching_fixed_cli_kind_without_message",
        "taira_public_reset::host::tests::prepared_child_failure_does_not_trust_unknown_or_mismatched_error_kind",
        "taira_public_reset::host::tests::prepared_child_zero_exit_protocol_failure_never_echoes_output",
        "taira_public_reset::host::tests::prepared_child_zero_exit_preserves_typed_write_and_inrou_outcomes",
    )),
    ("canonical receipt namespace", (
        "taira_public_reset::host::tests::host_receipt_names_cover_every_action_and_artifact_role",
        "taira_public_reset::host::tests::receipt_names_reject_path_control_and_unicode_escape",
    )),
    ("systemd operation and validator lifecycle", (
        "taira_public_reset::host::tests::validator_http_readiness_retries_cold_backends_before_strict_checks",
        "taira_public_reset::host::tests::validator_http_readiness_rejects_permanent_http_errors",
        "taira_public_reset::host::tests::validator_http_readiness_keeps_deadline_and_authorization",
        "taira_public_reset::host::tests::doctor_failure_reports_bounded_public_check_diagnostics",
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
        "taira::tests::prepared_applied_confirmation_waits_for_exact_details_visibility",
        "taira::tests::prepared_applied_confirmation_rejects_unauthorized_or_malformed_exact_details",
        "taira::tests::prepared_predecessor_wait_retries_delayed_exact_proof_with_one_deadline",
        "taira::tests::prepared_inrou_observation_preserves_auth_and_proof_errors",
        "taira::tests::prepared_server_confirmation_preserves_fixed_failure_and_deadline",
        "taira::tests::prepared_server_confirmation_retries_deadline_timeout_until_fixed_failure",
        "taira::tests::prepared_server_confirmation_preserves_configured_timeout_errors",
        "taira::tests::prepared_server_confirmation_preserves_other_transport_errors",
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

CRYPTO_STAGES += (("optimized BLS arithmetic preserves verification boundaries", (
    "signature::bls::tests::normal::signature_verification",
    "signature::bls::tests::normal::aggregate_same_message_roundtrip",
    "signature::bls::tests::normal::parse_public_key_rejects_non_subgroup_point",
    "signature::bls::tests::normal::verify_cache_rejects_variable_length_tuple_splice",
    "signature::bls::tests::small::signature_verification",
    "signature::bls::tests::small::signature_verification_different_keys",
    "signature::bls::tests::small::parse_public_key_rejects_non_subgroup_point",
    "signature::bls::tests::small::verify_cache_rejects_variable_length_tuple_splice",
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

P2P_STAGES += (("immutable reply identity and exact dynamic history", (
    "network::tests::reply_source_key_shares_identity_without_retaining_delivery_tenure",
    "network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
    "network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
    "network::tests::reply_route_binding_rejects_evicted_tombstone_collision",
    "network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
    "network::tests::reply_route_history_projection_tracks_live_and_retired_transitions",
)),)

CORE_STAGES = (("consensus scheduling and multi-route progress", (
    "sumeragi::v2_runner::tests::runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
    "sumeragi::v2_lifecycle_coordinator::work_registry::tests::registered_deferred_validate_passes_ordinary_completion_without_releasing_wait",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_canonical_wire_seals_only_complete_classified_messages",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
    "sumeragi::v2::tests::adapter_hot_context_projections_retain_the_verified_registry_identity",
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

CORE_STAGES += (("descriptor-bound storage namespace identity", (
    "kura::tests::progress_witness_durability::bound_progress_directory_binding_allows_child_mutation_but_rejects_replacement",
    "kura::tests::progress_witness_durability::bound_progress_directory_chain_rejects_replaced_or_symlinked_ancestors",
    "kura::tests::progress_witness_durability::bound_progress_directory_chain_rejects_inconsistent_child_paths",
    "kura::tests::progress_witness_durability::progress_sidecar_mutation_rejects_symlinks_without_external_writes",
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

CORE_STAGES += (("resolved validation and exact application ownership", (
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_live_validate_retained_terminal_publishes_one_current_commit_apply",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_recovered_validate_retained_terminal_publishes_one_current_commit_apply",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_validate_retained_terminal_rejects_changed_outcome_digest_before_apply",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_validate_historical_prepare_repair_then_same_tag_commit_publishes_once",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::physical_validate_busy_retains_exact_result_until_timeout_quorum_then_commit",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_rejected_validate_replays_report_once_without_revalidation_or_apply",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_published_validate_retained_terminal_publishes_one_current_commit_apply",
    "sumeragi::v2_runtime::tests::historical_prepare_rejection_retains_exact_report_authority",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::already_terminal_validate_cold_reopen_preserves_success_and_rejection",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::rejected_terminal_and_published_report_cold_reopen_preserves_one_output_owner",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_recovery_accepts_standalone_report_with_exact_terminal_rejection",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_recovery_rejects_standalone_report_without_exact_terminal_authority",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::committed_standalone_prepare_pair_preserves_inert_validate_without_a_link",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::same_view_resolved_validation_publishes_commit_sign_and_cold_reopens_exact_owner",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::resolved_validate_survives_unprotected_view_until_current_commit",
    "sumeragi::v2_lifecycle_coordinator::work_registry::tests::cold_ready_validate_retry_census_is_complete_inert_and_installed_before_live_clocks",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::recovered_released_decision_apply_does_not_hide_current_source_with_changed_owner",
    "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::resolved_report_owner_tracks_terminal_and_statement_not_retry_encoding",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_body_owners_cold_reopen_under_durable_commit",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_validate_cold_reopen_after_timeout_and_durable_commit",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_body_keys_distinguish_prepare_and_decision_authority",
    "sumeragi::v2_lifecycle_coordinator::ledger::lifecycle_phase_codes_round_trip_without_aliases",
    "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::decision_body_retirement_preserves_current_winner_and_rejects_future_tags",
    "sumeragi::v2::tests::recovered_decision_validate_cold_projection_installs_with_body_census",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_preserves_historical_prepare_owners_through_terminal_recovery",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_rejects_corrupt_historical_prepare_before_terminalization",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_terminal_join_binds_the_full_finality_family",
    "sumeragi::v2_effects::tests::current_prepare_body_replay_requires_exact_current_durable_authority",
    "sumeragi::v2_effects::tests::active_validate_retry_owners_preserve_single_admission",
    "sumeragi::v2_effects::tests::bound_validate_retry_rejects_stale_and_conflicting_authority",
    "sumeragi::v2_effects::tests::validate_retry_lifecycle_transitions_require_exact_owner",
    "sumeragi::v2_effects::tests::later_decision_apply_uses_its_runtime_owner_after_validate_successor_release",
    "sumeragi::v2_effects::tests::protected_prepare_validate_reseeds_missing_replay_from_exact_recovered_body",
    "sumeragi::v2_effects::tests::protected_prepare_bound_retry_rolls_back_with_a_malformed_later_effect",
    "sumeragi::v2_effects::tests::admitted_validate_retry_seal_coalesces_exact_authority_upgrade_without_replay_reuse",
    "sumeragi::v2_effects::tests::cold_active_rejection_denies_local_adoption_without_live_pipeline_owner",
    "sumeragi::v2_effects::tests::recovered_apply_releases_only_its_authenticated_validate_retry_predecessor",
    "sumeragi::v2_effects::tests::decision_cleanup_defers_live_validate_authority_retirement_until_exact_resolution",
    "sumeragi::v2_effects::tests::durable_decision_preserves_stored_proposal_replay_for_commit_refined_validate",
    "sumeragi::v2_effects::tests::protected_commit_validate_reseeds_missing_replay_without_applying",
    "sumeragi::v2_effects::tests::missing_replay_commit_rejects_foreign_decision_and_commitment",
    "sumeragi::v2_lifecycle_coordinator::work_registry::tests::validator_apply_drains_exact_suffix_after_delayed_commit_qc_admission",
    "sumeragi::v2_lane_work::tests::durable_merge_refresh_retains_journal_across_real_parent_publication",
    "sumeragi::v2_lane_work::tests::merge_signing_fence_refuses_private_key_after_parent_publication",
    "sumeragi::v2_lane_work::tests::autonomous_fixture_binds_final_lane_context_before_opening_signing_guards",
)),)

CLIENT_STAGES = (("public compatibility discovery before account bootstrap", (
    "client::tests::prospective_account_submission_discovers_capabilities_without_account_auth",
    "client::tests::get_node_capabilities_json_requests_json_accept",
    "client::tests::get_node_capabilities_json_accepts_torii_utf8_json_content_type",
    "client::tests::get_node_capabilities_json_rejects_ambiguous_representation",
    "client::tests::submit_transaction_rejects_mismatched_data_model_version",
    "client::tests::submit_transaction_rejects_missing_data_model_version",
    "client::tests::submit_transaction_rejects_missing_signed_transaction_schema_hash",
    "client::tests::submit_transaction_rejects_invalid_signed_transaction_schema_hash",
    "client::tests::submit_transaction_rejects_mismatched_signed_transaction_schema_hash",
)), ("shared absolute HTTP operation deadline", (
    "http_default::tests::operation_deadline_bounds_sequential_blocking_dispatches",
    "http_default::tests::expired_operation_deadline_prevents_dispatch_and_cannot_be_extended",
    "http_default::tests::operation_deadline_cancels_injected_async_transport",
    "client::context_tests::request_deadline_clones_context_and_survives_rebuilding",
    "client::context_tests::request_deadline_bounds_waiting_for_blocking_compatibility_probe",
    "client::context_tests::request_deadline_bounds_waiting_for_async_compatibility_probe",
    "client::context_tests::shared_capability_probe_preserves_typed_timeout_classification",
)), ("public contract SDK envelope", (
    "client::evidence_http_tests::post_contract_call_accepts_only_the_caller_trusted_draft_intent",
    "client::evidence_http_tests::post_contract_call_authenticates_bound_account_and_rejects_foreign_authority",
    "client::evidence_http_tests::post_contract_call_rejects_ordinary_draft_before_signing_or_submission",
    "client::evidence_http_tests::post_contract_call_rejects_substituted_operation_receipt",
    "client::evidence_http_tests::post_contract_call_rejects_omitted_operation_receipt_fields",
    "client::evidence_http_tests::post_contract_call_rejects_unsupported_response_root_fields",
    "client::evidence_http_tests::post_contract_call_rejects_omitted_response_root_fields",
)),)

CLIENT_STAGES += (("exact transaction details error protocol", (
    "query::query_errors_handling::transaction_details_failure_only_maps_the_exact_missing_envelope_to_absence",
    "query::query_errors_handling::transaction_details_failure_rejects_absence_with_a_non_404_status",
    "query::query_errors_handling::transaction_details_failure_rejects_plain_404_and_malformed_norito_without_codec_io",
    "query::query_errors_handling::transaction_details_failure_requires_one_exact_norito_media_type",
    "query::query_errors_handling::transaction_details_failure_rejects_response_over_the_wire_bound",
)),)

DAEMON_STAGES = (("offline final genesis deployment authority", (
    "tests::manifest_crypto_checks::manifest_crypto_matches_config",
    "tests::manifest_crypto_checks::detects_hash_mismatch",
    "tests::manifest_crypto_checks::detects_allowed_signing_mismatch",
    "tests::manifest_crypto_checks::detects_allowed_curve_ids_mismatch",
    "tests::manifest_crypto_checks::verify_genesis_metadata_rejects_crypto_mismatch_in_block",
    "tests::manifest_crypto_checks::fresh_v2_genesis_staging_does_not_commit_state_or_kura",
    "tests::manifest_crypto_checks::check_config_offline_executes_available_genesis",
    "tests::manifest_crypto_checks::check_config_accepts_taira_without_offline_backend_settings",
    "tests::manifest_crypto_checks::check_config_qualifies_the_fixed_moderation_strict_ingress",
    "tests::manifest_crypto_checks::check_config_offline_rejects_genesis_instruction_failure",
    "tests::manifest_crypto_checks::consensus_config_caps_use_canonical_v2_fields",
    "tests::manifest_crypto_checks::consensus_caps_use_frozen_height_context_mode",
    "tests::manifest_crypto_checks::verify_genesis_metadata_rejects_consensus_mode_mismatch",
    "tests::manifest_crypto_checks::verify_genesis_metadata_rejects_fingerprint_mismatch",
    "tests::cli_args::inrou_deployment_authority_requires_offline_check_config",
    "tests::manifest_crypto_checks::check_config_offline_accepts_final_inrou_deployment_capability",
    "tests::manifest_crypto_checks::check_config_offline_rejects_absent_or_revoked_inrou_deployment_capability",
    "tests::manifest_crypto_checks::check_config_offline_rejects_malformed_inrou_management_grants",
    "tests::manifest_crypto_checks::check_config_inrou_authority_requires_canonical_account_and_signed_genesis",
)),)

DAEMON_STARTUP_STAGES = (("frozen startup policy before snapshot authentication and replay", (
    "startup_runtime_policy_tests::startup_compliance_is_installed_before_execution_policy_derivation_and_reused",
    "startup_runtime_policy_tests::startup_compliance_rejects_missing_and_wrong_lane_policy_before_replay",
)),)
DAEMON_STAGES += DAEMON_STARTUP_STAGES

TORII_STARTUP_STAGES = (("HTTP admission waits for Queue startup reconciliation", (
    "tests_runtime_handlers::readiness_rejects_empty_queue_startup_reconciliation",
    "tests_runtime_handlers::readiness_rejects_closed_consensus_ingress",
)), ("actual public MCP catalogue and response bounds", (
    "mcp::tests::tools_list_writer_catalog_roundtrips_through_modern_http_byte_limit",
    "mcp::tests::tools_list_byte_budget_includes_envelope_and_rejects_oversized_single_tool",
    "mcp::tests::advertised_schema_factoring_preserves_subschemas_and_literal_values",
    "mcp::tests::registry_security::musubi_v1_mcp_bodies_are_self_contained_closed_schemas",
    "mcp::tests::whole_catalog_publishes_self_contained_input_schemas",
    "mcp::tests::registry_security::tools_list_list_changed_tracks_toolset_version",
)),)
TORII_UNIT_STAGES = TORII_STARTUP_STAGES + (("public node capabilities and exact route authentication", (
    "tests_runtime_handlers::node_capabilities_http_bootstraps_without_registered_account",
    "openapi::tests::catalog_and_contracts::account_capabilities_document_exact_public_bootstrap_policy",
    "mcp::tests::target_policy_requires_inner_canonical_proof_only_for_canonical_route",
)), ("public contract retained payload and certified ingress", (
    "routing::multisig_selector_tests::contract_call_detached_submission_retains_exact_queue_plan_payload",
    "routing::multisig_selector_tests::contract_call_detached_submission_preserves_retained_fee_limits_without_requote",
    "routing::multisig_selector_tests::contract_call_detached_submission_rejects_changed_or_noncanonical_payload",
    "routing::multisig_selector_tests::contract_call_detached_handler_requires_certified_public_admission",
    "routing::multisig_selector_tests::contract_call_detached_submission_requires_complete_retained_envelope",
    "routing::multisig_selector_tests::contract_call_prepare_serializes_complete_canonical_response",
    "openapi::tests::public_contract_call_schema_matches_exact_queue_plan_handoff",
    "openapi::tests::checked_openapi_assets_match_package_authority",
)),)

TORII_UNIT_STAGES += (("exact transaction visibility and restricted history isolation", (
    "tests_runtime_handlers::transaction_details_http_sdk_preserves_exact_absence_and_authorization",
    "tests_runtime_handlers::transaction_details_allows_sender_and_batch_recipient_but_rejects_other_accounts",
    "tests_runtime_handlers::transaction_details_native_beneficiaries_preserve_restricted_history_isolation",
    "tests_runtime_handlers::transaction_details_allows_operator_and_rejects_wrong_network_and_replay",
    "tests_runtime_handlers::transaction_details_rejects_unsigned_and_broadened_queries",
)),)

CORE_STAGES += (("native storage and workload Initial executor admission", (
    "smartcontracts::isi::registry_dispatch_tests::every_soracloud_wire_instruction_has_a_reviewed_initial_disposition",
    "smartcontracts::isi::registry_dispatch_tests::every_sorafs_wire_instruction_has_a_reviewed_initial_disposition",
    "smartcontracts::isi::soracloud::tests::initial_executor_soracloud_host_lifecycle_preserves_exact_validator_authority",
    "smartcontracts::isi::soracloud::tests::initial_executor_soracloud_roles_preserve_exact_permission_payloads_and_delegation",
    "smartcontracts::isi::soracloud::tests::initial_executor_soracloud_lease_usage_and_runtime_preserve_exact_assignment",
    "smartcontracts::isi::soracloud::tests::service_runtime_mutations_require_exact_validator_placement",
    "smartcontracts::isi::sorafs::sorafs_tests::initial_executor_sorafs_direct_provider_owner_instructions_remain_closed",
    "smartcontracts::isi::sorafs::sorafs_tests::initial_executor_sorafs_role_grant_use_and_revoke_are_exact",
    "smartcontracts::isi::sorafs::sorafs_tests::initial_executor_sorafs_rejects_malformed_unit_and_foreign_role_permissions",
    "smartcontracts::isi::sorafs::sorafs_tests::register_pin_manifest_allows_public_submission",
    "smartcontracts::isi::sorafs::sorafs_tests::public_pin_cannot_reserve_alias_without_alias_permission",
    "smartcontracts::isi::sorafs::sorafs_tests::register_pin_manifest_rejects_unfunded_public_submission_without_side_effects",
    "smartcontracts::isi::sorafs::sorafs_tests::threshold_approval_may_be_relayed_without_broad_permission",
    "smartcontracts::isi::sorafs::sorafs_tests::retire_pin_manifest_requires_exact_authenticated_submitter",
    "smartcontracts::isi::sorafs::sorafs_tests::bind_manifest_alias_requires_permission",
    "smartcontracts::isi::sorafs::sorafs_tests::bind_manifest_alias_registers_record",
)),)

TORII_STAGES += (("public contract HTTP preparation and strict admission", (
    "contracts_call_integration::contracts_call_prepares_exact_payload_and_requires_certified_admission",
)),)

CORE_STAGES += (("authenticated admission and coherent State publication", (
    "state::tests::ordinary_lane_frontier_publishes_once_and_rejects_invalid_successors_atomically",
    "state::tests::ordinary_lane_frontier_extends_autonomous_application_and_unblocks_next_merge",
    "sumeragi::v2_apply::tests::ordinary_lane_frontier_unblocks_third_autonomous_source_after_timeout_views",
    "state::tests::sparse_merge_execution_frontier_rejects_replay_conflict_and_malformed_predecessor",
    "kura::tests::carrier_lookup_requires_finality_even_while_body_is_present",
    "kura::tests::finality_store_rejects_missing_or_wrong_merge_carrier_projection",
    "kura::tests::finality_authenticated_carrier_survives_body_removal_and_restart",
    "state::tests::autonomous_merge_admission_intent_follower_and_historical_reject_ordinary_external",
    "state::tests::live_autonomous_merge_rejects_historical_sealed_signed_execution_alias",
    "state::tests::malformed_merge_execution_batch_rejects_empty_lane_set",
    "state::tests::staged_merge_missing_transaction_block_mutates_nothing",
    "state::tests::durable_kura_carrier_requires_exact_committed_state_carrier_before_publication",
    "state::tests::same_block_merge_and_lane_replacement_preserves_history_and_prunes_old_progress",
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

CORE_ADMISSION_STARTUP_STAGES = (("empty Queue startup admission fence", (
    "queue::tests::empty_replayed_journals_keep_ingress_closed_until_reconciliation_completion",
    "sumeragi::v2_runner::tests::lane_evidence_repair_fence_accepts_an_empty_quarantined_replay",
    "sumeragi::v2_runner::tests::startup_reconciles_lifecycle_before_lane_work_activation",
    "sumeragi::v2_lifecycle_recovery::tests::empty_queue_reconciliation_returns_the_same_checked_receipt",
    "sumeragi::v2_lifecycle_recovery::tests::retired_nonqueue_replica_release_pending_resumes_on_startup_without_queue_owner",
    "sumeragi::authoritative_runtime_gate_tests::ingress_stays_closed_until_replay_owner_acknowledges_ready",
)), ("fee sponsor activation and prospective account bootstrap", (
    "smartcontracts::isi::world::isi::tests::fee_sponsor_activation_instruction_uses_requested_height_as_lower_bound",
    "smartcontracts::isi::world::isi::tests::fee_sponsor_elapsed_activation_preserves_readiness_and_authority_guards",
    "smartcontracts::isi::world::isi::tests::prospective_fee_sponsor_enrollment_funds_only_exact_self_bootstrap",
    "smartcontracts::isi::world::isi::tests::prospective_fee_sponsor_enrollment_preserves_authority_and_closed_guards",
    "state::tests::fee_sponsor_safe_activation_height_clamps_elapsed_lower_bound",
    "state::tests::fee_sponsor_safe_activation_height_preserves_later_request",
    "state::tests::fee_sponsor_safe_activation_height_fails_closed_for_non_draining_lease",
    "state::tests::fee_sponsor_revision_activation_materializes_at_scheduled_block_height",
    "state::tests::fee_sponsor_revision_activation_waits_for_old_lease_to_drain",
    "executor::tests::sponsor_resolution_predicts_scheduled_revision_only_after_old_leases_drain",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("completed consensus outputs after durable restart", (
    "sumeragi::v2_lifecycle_coordinator::concrete_admission::tests::terminal_signed_outputs_rejoin_after_durable_restart",
    "sumeragi::v2_lifecycle_coordinator::concrete_admission::tests::terminal_timeout_certificate_reservices_only_sealed_periodic_episode",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("current Prepare recovery and durable validation retry", (
    "sumeragi::v2_runtime::tests::periodic_current_prepare_retries_bind_store_and_validate_before_lock",
    "sumeragi::v2_effects::tests::missing_replay_validate_rejects_ordinary_phase_none_binding",
    "sumeragi::v2_body_store::tests::validation_marker_publication_reuses_exact_durable_outcomes",
    "sumeragi::v2_body_store::tests::validation_marker_publication_rejects_changed_or_linked_artifacts",
)),)
CORE_STARTUP_STAGES = CORE_ADMISSION_STARTUP_STAGES + (("authenticated snapshot owner policy and startup custody", (
    "state::tests::snapshot_owner_policy_survives_startup_with_live_nondefault_staking",
    "state::tests::snapshot_owner_policy_rejects_changed_owner_before_and_after_hydration",
    "state::tests::snapshot_owner_policy_requires_complete_canonical_fields",
    "state::tests::set_nexus_rejects_two_step_staking_mode_toggle_with_live_shared_state",
    "state::tests::set_nexus_rejects_live_single_lane_stake_owner_reassignment",
    "state::tests::state_json_rejects_prior_nexus_runtime_version",
    "state::tests::emergency_fast_restored_config_rejects_dataspace_catalog_replacement",
    "sumeragi::v2_recovery::tests::imported_snapshot_authenticates_explicit_frozen_policy_without_replacing_state",
    "sumeragi::v2_recovery::tests::all_hash_only_snapshot_recovers_exact_authenticated_successor",
    "sumeragi::v2_recovery::tests::snapshot_bootstrap_authentication_rejects_future_kaigi_feedback_and_rolls_back",
    "sumeragi::v2_recovery::tests::all_hash_only_snapshot_without_authenticated_record_fails_closed",
    "sumeragi::v2_recovery::tests::later_snapshot_before_first_full_finality_is_rejected_without_mutation",
    "sumeragi::v2_recovery::tests::later_snapshot_rejects_lineage_changed_from_immutable_first_height",
    "sumeragi::v2_recovery::tests::hash_only_snapshot_rejects_an_intermediate_hash_vector_substitution",
    "state::tests::startup_sumeragi_key_policy_matches_canonical_state_without_mutation",
    "state::tests::startup_sumeragi_key_policy_rejects_each_mismatch_without_mutation",
)), ("cold certified history and exact publication recovery", (
    "kura::tests::sequential_autonomous_certificates_advance_the_durable_frontier",
    "kura::tests::mixed_ordinary_autonomous_certificates_cold_restore_preserves_completed_history",
    "kura::tests::certified_bundle_cold_restore_repairs_only_latest_partial_publication",
    "kura::tests::certified_bundle_cold_restore_rejects_corrupt_or_missing_completed_history_without_mutation",
    "kura::tests::certified_frontier_build_only_restart_promotes_then_rebuilds_remaining_obligation",
    "kura::tests::certified_pair_crash_rebuilds_only_bundle_obligation",
    "kura::tests::durable_bundle_pair_crash_rebuild_consumes_obligation_from_exact_readback",
    "kura::tests::bundle_pair_append_intent_rebuilds_then_repairs_exact_obligation",
    "kura::tests::append_intent_and_build_restart_preflight_reject_one_under_without_mutation",
    "kura::tests::latest_certified_frontier_rejects_equal_height_conflict_before_publication",
    "kura::tests::latest_certified_frontier_corruption_and_post_validation_substitution_fail_closed",
)), ("authenticated history compaction and cold recovery", (
    "kura::tests::lane_history_cold_restore_accepts_independent_authenticated_prefix_cuts",
    "kura::tests::lane_history_cold_restore_recovers_certified_and_bundle_rewrite_cuts",
    "kura::tests::lane_history_cold_restore_rejects_untrusted_frontier_and_retained_evidence_loss",
    "kura::tests::lane_history_capacity_blocked_cold_restore_keeps_authenticated_prefix",
    "kura::tests::lane_history_cold_restore_does_not_resurrect_terminal_local_frontier",
    "kura::tests::lane_history_cold_restore_admits_obsolete_append_at_exact_capacity",
    "kura::tests::lane_history_compaction_recovers_crash_temp_before_tight_capacity_refusal",
    "kura::tests::lane_history_compaction_rejects_data_only_temp_before_capacity_refusal",
    "kura::tests::lane_history_compaction_rejects_corrupt_temp_index_before_capacity_refusal",
)),)
CORE_STAGES += CORE_STARTUP_STAGES

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

BASIC_NETWORK_STAGES = (("four-validator universal-route commit and signed snapshot restart", (
    "four_peer_universal_public_transaction_sequence_reaches_applied",
)),)
NETWORK_STAGES = BASIC_NETWORK_STAGES + (("four-validator multi-route commit and signed snapshot restart", (
    "four_peer_multiroute_public_transaction_sequence_reaches_applied",
)),)

# Four peers use the shared test-network 1 GiB/node cap. Keep another 4 GiB
# available for fixture logs, temporary files and concurrent build output.
NETWORK_FIXTURE_FREE_BYTES = 8 * 1024**3

HARNESS_TARGETS = {
    "daemon": ("native offline genesis qualification", "irohad", "lib", ["-p", "irohad", "--lib"]),
    "config": ("native configuration contracts", "taira_config_contracts", "test", ["-p", "iroha_config", "--test", "taira_config_contracts"]),
    "cli": ("native CLI", "iroha", "bin", ["-p", "iroha_cli", "--bin", "iroha"]),
    "kagami": ("native Kagami", "kagami", "bin", ["-p", "iroha_kagami", "--bin", "kagami"]),
    "sorafs-bin": ("native SoraFS shipping target", "sorafs-node", "bin", ["-p", "sorafs_node", "--bin", "sorafs-node"]),
    "taira-launcher": ("native Taira shipping launcher", "iroha3d_taira", "bin", ["-p", "irohad", "--bin", "iroha3d_taira"]),
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


KAGAMI_STAGES = (("canonical Kagami export projection", (
    "kura::scaling_evidence::export::tests::unix::strict_projection_has_exact_types_order_and_signed_hash_identity",
)), ("generated Taira operator deployment authority", (
    "localnet::tests::generated_taira_genesis_grants_deployment_only_to_generated_client",
)),)


class CheckError(Exception):
    """A build or selected regression did not pass."""


class SelectedRegressionFailures(CheckError):
    """Completed selected tests failed; other isolated fixtures can still run."""

    def __init__(self, failures: list[str]):
        self.failures = tuple(failures)
        super().__init__(f"{len(self.failures)} selected regressions failed: "
                         + "; ".join(self.failures))


def shipping_harnesses(root: Path) -> tuple[str, ...]:
    """Reconcile shipping binaries with manifests and early native compilation.

    Read the one literal shipping table from the captured preparation source,
    without importing another gate or executing it. Native tests catch shared
    source errors; the separate Linux build still qualifies platform code.
    """
    try:
        source = ast.parse((root / "scripts/taira_release.py").read_text())
        tables = [node.value for node in source.body if isinstance(node, ast.Assign)
                  and any(isinstance(target, ast.Name) and target.id == "BINARIES"
                          for target in node.targets)]
        if len(tables) != 1:
            raise CheckError("shipping binaries require one literal authoritative table")
        binaries = ast.literal_eval(tables[0])
        if (not isinstance(binaries, tuple) or not binaries
                or any(not isinstance(row, tuple) or len(row) != 2
                       or any(not isinstance(value, str) or re.fullmatch(r"[a-z0-9_-]+", value) is None
                              for value in row) for row in binaries)
                or len({row[0] for row in binaries}) != len(binaries)):
            raise CheckError("invalid authoritative shipping binary table")
        selected = []
        for name, package in binaries:
            matches = [key for key, (_, target, kind, arguments) in HARNESS_TARGETS.items()
                       if target == name and kind == "bin"
                       and arguments == ["-p", package, "--bin", name]]
            if len(matches) != 1:
                raise CheckError("shipping binary lacks exact early native coverage: " + name)
            package_root = root / "crates" / package
            manifest = tomllib.loads((package_root / "Cargo.toml").read_text())
            targets = [target for target in manifest.get("bin", []) if target.get("name") == name]
            if manifest.get("package", {}).get("name") != package or len(targets) != 1:
                raise CheckError("shipping binary differs from its Cargo manifest: " + name)
            target = targets[0]
            path = target.get("path")
            if (not isinstance(path, str) or Path(path).is_absolute() or ".." in Path(path).parts
                    or not (package_root / path).is_file()):
                raise CheckError("shipping binary requires an existing explicit source path: " + name)
            features = manifest.get("features", {})
            enabled, pending = set(), list(features.get("default", []))
            while pending:
                feature = pending.pop()
                if feature not in enabled:
                    enabled.add(feature)
                    pending.extend(features.get(feature, []))
            if not set(target.get("required-features", [])).issubset(enabled):
                raise CheckError("shipping binary requires non-default features: " + name)
            # Catch the specific invalid exported-macro namespace on every
            # source branch. This is not a Rust parser or Linux type check.
            for path in (package_root / "src").rglob("*.rs"):
                if re.search(r"\bnorito\s*::\s*json\s*::\s*json\s*!", path.read_text()):
                    raise CheckError("invalid Norito JSON macro path in " + str(path.relative_to(root)))
            selected.append(matches[0])
        return tuple(selected)
    except (OSError, SyntaxError, ValueError, TypeError) as error:
        raise CheckError("shipping native coverage audit failed: " + str(error)) from error


QUALIFICATION_SCOPES = ("basic", "full")


def qualification_stages(qualification_scope: str = "basic") -> dict[str, tuple]:
    """Select honest test coverage without changing shipping features or artifacts."""
    if qualification_scope not in QUALIFICATION_SCOPES:
        raise CheckError("native qualification scope must be basic or full")
    selected = {
        "config": CONFIG_STAGES, "kagami": KAGAMI_STAGES,
        "proof": PROOF_STAGES, "proof-flows": PROOF_FLOW_STAGES,
        "crypto": CRYPTO_STAGES, "p2p": P2P_STAGES, "core": CORE_STAGES,
        "test-network": TEST_NETWORK_STAGES, "client": CLIENT_STAGES,
        "torii-unit": TORII_UNIT_STAGES, "torii": TORII_STAGES,
        "daemon": DAEMON_STAGES, "network": NETWORK_STAGES, "cli": STAGES,
    }
    if qualification_scope == "basic":
        # These affected startup regressions and the real universal-route network
        # exercise admission/restart. Advanced storage/fault matrices remain
        # selectable with full. Crypto, proof bounds and custody stay mandatory.
        selected["core"] = CORE_ADMISSION_STARTUP_STAGES
        selected["proof-flows"] = ()
        selected["network"] = BASIC_NETWORK_STAGES
    return selected


def selected_regression_count(qualification_scope: str = "basic") -> int:
    """Return this scope's selected native census, including the real network."""
    return sum(len(names) for stages in qualification_stages(qualification_scope).values()
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
                    harness: str = "cli") -> NativeArtifactCopies:
    command = compile_command(root, env, harness=harness)
    return _build_harnesses(root, command, env, (harness,), lock_fds)


def compile_test_harnesses(root: Path, env: dict[str, str], *,
                          harnesses: tuple[str, ...],
                          lock_fds: tuple[int, ...] = ()) -> NativeArtifactCopies:
    """Build selected library, integration and binary tests with one feature graph."""
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
        elif kind in {"test", "bin"} and len(arguments) == 4 and arguments == ["-p", arguments[1], "--" + kind, name]:
            targets.extend(["--" + kind, name])
        else:
            raise CheckError("native test batch requires explicit library, integration or binary targets")
        if arguments[1] not in packages:
            packages.append(arguments[1])
    selection = [argument for package in packages for argument in ("-p", package)]
    command = _compile_command(root, env, [*selection, *targets])
    return _build_harnesses(root, command, env, harnesses, lock_fds)


def _build_harnesses(root: Path, command: list[str], env: dict[str, str],
                     harnesses: tuple[str, ...], lock_fds: tuple[int, ...]) -> NativeArtifactCopies:
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


class NativeArtifactCopies(dict[str, str]):
    """Own only the private copied test files from one completed isolation call."""

    def __init__(self, output: Path, copied: dict[str, str],
                 identities: dict[Path, tuple[int, ...]], observations: list[dict[str, object]]):
        super().__init__(copied)
        self.output = output
        self.observations = tuple(observations)
        self.pending = {row["selection"]: (identities[Path(row["path"])], row)
                        for row in observations if row["selection"] in HARNESS_TARGETS
                        and row["cargo_artifact"]["profile"].get("test") is True}
        self.directory_fd = (os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                             if self.pending else None)

    def __enter__(self):
        return self

    def release(self, selection: str) -> None:
        """Release a completed test's exact copy; production snapshots are retained."""
        if selection not in self.pending:
            return
        expected, observation = self.pending[selection]
        fd = self.directory_fd
        assert fd is not None
        held, named = os.fstat(fd), self.output.lstat()
        if ((held.st_dev, held.st_ino) != (named.st_dev, named.st_ino)
                or held.st_uid != os.geteuid() or not stat.S_ISDIR(named.st_mode)
                or stat.S_IMODE(named.st_mode) != 0o500):
            raise CheckError("native test copy directory changed before release")
        named = os.stat(selection, dir_fd=fd, follow_symlinks=False)
        actual = (named.st_dev, named.st_ino, named.st_size, named.st_mode,
                  named.st_uid, named.st_nlink, named.st_mtime_ns, named.st_ctime_ns)
        if actual != expected:
            raise CheckError("native test copy changed before release: " + selection)
        os.fchmod(fd, 0o700)
        try:
            os.unlink(selection, dir_fd=fd)
            os.fsync(fd)
        finally:
            os.fchmod(fd, 0o500)
        del self.pending[selection]
        print("[taira-check] released native test artifact "
              + json.dumps(observation, sort_keys=True), flush=True)

    def __exit__(self, exception_type, exception, traceback):
        failures = []
        try:
            for selection in tuple(self.pending):
                try:
                    self.release(selection)
                except (CheckError, OSError) as error:
                    failures.append(str(error))
        finally:
            if self.directory_fd is not None:
                os.close(self.directory_fd)
                self.directory_fd = None
        if failures:
            message = "native test artifact release failed; copies retained: " + "; ".join(failures)
            if exception is None:
                raise CheckError(message)
            print("[taira-check] " + message, file=sys.stderr, flush=True)
        return False


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


def native_artifact_clone_function():
    """Return macOS descriptor cloning, or None where this API is unavailable.

    fclonefileat creates an absent destination atomically with independent inode
    and copy-on-write contents. It never falls back internally to a full copy.
    """
    if sys.platform != "darwin":
        return None
    library = ctypes.CDLL(None, use_errno=True)
    try:
        clone = library.fclonefileat
    except AttributeError:
        return None
    clone.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
    clone.restype = ctypes.c_int

    def clone_descriptor(source: int, directory: int, name: str) -> bool:
        if not name or name in {".", ".."} or "/" in name or "\x00" in name:
            raise CheckError("native clone requires one exact destination basename")
        # sys/clonefile.h: CLONE_NOFOLLOW | CLONE_NOOWNERCOPY. The source is
        # already pinned; destination resolution is beneath a private dirfd.
        if clone(source, directory, os.fsencode(name), 0x0001 | 0x0002) == 0:
            return True
        error = ctypes.get_errno()
        if error in {errno.ENOTSUP, errno.EXDEV, errno.ENOSYS}:
            return False
        # EEXIST, EINVAL, ENOSPC and I/O errors are not permission to retry via
        # a different copy mechanism. Atomic clone failure creates no file.
        raise OSError(error, os.strerror(error), name)

    return clone_descriptor


def isolate_native_artifacts(root: Path, env: dict[str, str],
                             records: dict[str, dict[str, object]]) -> NativeArtifactCopies:
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
            clone = native_artifact_clone_function()
            remaining_bytes = sum(info.size for info in identities.values())
            # Clones need metadata, not another logical-size data allocation.
            # Preserve the working reserve and explicit metadata headroom; also
            # recheck actual free bytes after every copy before publication.
            clone_headroom = 64 * 1024 * 1024
            required = NETWORK_FIXTURE_FREE_BYTES + (min(clone_headroom, remaining_bytes) if clone else remaining_bytes)
            if shutil.disk_usage(target).free < required:
                raise CheckError("native artifact copies would consume the required working-space reserve")
            output = Path(tempfile.mkdtemp(prefix="taira-native-artifacts-", dir=target))
            copied, observations, published = {}, [], {}
            for selection, path in paths.items():
                expected = identities[selection]
                destination = output / selection
                digest, size = hashlib.sha256(), 0
                with stable_open_relative(target, str(path.relative_to(target)), expected=expected) as source:
                    cloned = False
                    if clone is not None:
                        if shutil.disk_usage(target).free < NETWORK_FIXTURE_FREE_BYTES + min(clone_headroom, remaining_bytes):
                            raise CheckError("native artifact clones would consume the required working-space reserve")
                        directory_fd = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                        try:
                            cloned = clone(source, directory_fd, selection)
                        finally:
                            os.close(directory_fd)
                    if not cloned:
                        # An unsupported filesystem may stream only after all
                        # remaining full copies plus the working reserve fit.
                        if shutil.disk_usage(target).free < remaining_bytes + NETWORK_FIXTURE_FREE_BYTES:
                            raise CheckError("native artifact copies would consume the required working-space reserve")
                        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
                    else:
                        fd = os.open(destination, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
                    try:
                        # Verify the actual clone's contents against the stable
                        # source capture; a successful syscall alone is not evidence.
                        reader = fd if cloned else source
                        while block := os.read(reader, 1024 * 1024):
                            size += len(block)
                            if size > expected.size:
                                raise CheckError("native artifact grew during descriptor copy")
                            digest.update(block)
                            if not cloned:
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
                        opened, named, origin = os.fstat(fd), destination.lstat(), os.fstat(source)
                        if ((opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino)
                                or (opened.st_dev, opened.st_ino) == (origin.st_dev, origin.st_ino)
                                or not stat.S_ISREG(opened.st_mode)
                                or opened.st_size != expected.size or named.st_size != expected.size
                                or named.st_uid != os.geteuid() or named.st_nlink != 1
                                or stat.S_IMODE(named.st_mode) != 0o500):
                            raise CheckError("native artifact destination changed during copy")
                        published[destination] = (named.st_dev, named.st_ino, named.st_size,
                            named.st_mode, named.st_uid, named.st_nlink, named.st_mtime_ns, named.st_ctime_ns)
                    finally:
                        os.close(fd)
                remaining_bytes -= expected.size
                if shutil.disk_usage(target).free < NETWORK_FIXTURE_FREE_BYTES:
                    raise CheckError("native artifact copies consumed the required working-space reserve")
                copied[selection] = str(destination)
                observations.append({"selection": selection, "path": str(destination),
                    "sha256": expected.sha256, "size": expected.size, "cargo_artifact": records[selection]})
            if shutil.disk_usage(target).free < NETWORK_FIXTURE_FREE_BYTES:
                raise CheckError("native artifact copies consumed the required working-space reserve")
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
        return NativeArtifactCopies(output, copied, published, observations)
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
        raise SelectedRegressionFailures(failures)


def compile_network_binaries(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> dict[str, str]:
    """Build the shipping package graph plus the ordinary fixture launcher."""
    expected = {"iroha3d": ("iroha3d", "irohad"), "iroha": ("iroha", "iroha_cli")}
    for selection in shipping_harnesses(root):
        _, name, _, arguments = HARNESS_TARGETS[selection]
        expected[name] = ("iroha" if name == "iroha" else selection, arguments[1])
    packages = tuple(dict.fromkeys(package for _, package in expected.values()))
    command = [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "build",
               "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
               *(argument for package in packages for argument in ("-p", package)),
               *(argument for name in expected for argument in ("--bin", name)),
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
            if (isinstance(name, str) and name in expected and "bin" in target.get("kind", [])
                    and event.get("profile", {}).get("test") is False
                    and isinstance(executable, str) and executable):
                if name in artifacts and artifacts[name] != executable:
                    raise CheckError("native network binary has conflicting Cargo artifacts")
                record = native_artifact_record(event)
                selection = expected[name][0]
                if selection in records and records[selection] != record:
                    raise CheckError("native network binary has conflicting Cargo metadata")
                artifacts[name] = executable
                records[selection] = record
                print("[taira-check] native network artifact " + json.dumps(record, sort_keys=True), flush=True)
        code = child.wait()
    if code or set(artifacts) != set(expected):
        raise CheckError(f"native network build did not produce every required executable artifact (exit {code})")
    print(f"[taira-check] network binary build passed in {time.monotonic() - started:.1f}s", flush=True)
    return isolate_native_artifacts(root, env, records)


def run_config_checks(harnesses: NativeArtifactCopies, fixture_root: Path, env: dict[str, str],
                      lock_fds: tuple[int, ...]) -> None:
    """Execute the shared graph's configuration artifact before all other tests."""
    if CONFIG_STAGES:
        run_stages(harnesses["config"], fixture_root, env, CONFIG_STAGES, lock_fds)
        harnesses.release("config")


def run_network_checks(root: Path, fixture_root: Path, env: dict[str, str], lock_fds: tuple[int, ...],
                       *, harness: str, stages: tuple) -> None:
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
    run_stages(harness, fixture_root, network_env, stages, lock_fds)


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
    """Reject invalid source assets, then run shared contracts before Cargo."""
    started = time.monotonic()
    print("[taira-check] start source-asset grammar and inventory audit", flush=True)
    checked = subprocess.run(
        [sys.executable, "-I", "-B", str(root / "scripts/tests/sumeragi_source_contract_asset_compaction_test.py")],
        cwd="/", env=env, stdin=subprocess.DEVNULL, text=True, capture_output=True,
        check=False, pass_fds=lock_fds, timeout=120)
    if checked.returncode:
        sys.stderr.write(checked.stdout + checked.stderr)
        raise CheckError(f"source-asset grammar and inventory audit failed (exit {checked.returncode})")
    print(f"[taira-check] source-asset grammar and inventory audit passed in {time.monotonic() - started:.1f}s", flush=True)
    checked = subprocess.run(
        [sys.executable, "-I", "-B", str(root / "scripts/check_taira_initial_executor.py"),
         "--repo", str(root), "--self-test"],
        cwd="/", env=env, stdin=subprocess.DEVNULL, text=True, capture_output=True,
        check=False, pass_fds=lock_fds, timeout=120)
    if checked.returncode:
        sys.stderr.write(checked.stdout + checked.stderr)
        raise CheckError(f"native Initial instruction source audit failed (exit {checked.returncode})")
    print("[taira-check] native Initial instruction source audit passed", flush=True)
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


def independent_check_evidence(harnesses: NativeArtifactCopies, stages, *,
                               qualification_scope: str = "basic") -> dict[str, object]:
    """Bind a complete independent pass to its exact census and copied Cargo artifacts."""
    qualification_stages(qualification_scope)
    artifacts = {row["selection"]: row for row in harnesses.observations}
    selections = [name for name, _ in stages]
    if len(artifacts) != len(harnesses.observations) or any(name not in artifacts for name in selections):
        raise CheckError("independent check artifact observations are incomplete or duplicated")
    return {
        "passed": True,
        "qualification_scope": qualification_scope,
        "selected_tests": [
            {"selection": name, "stages": [
                {"label": label, "tests": list(tests)} for label, tests in selected_stages
            ]} for name, selected_stages in stages
        ],
        # The temporary copy path changes on retry. Its content and actual Cargo
        # target metadata must still agree before an earlier test pass can apply.
        "artifacts": [
            {key: artifacts[name][key] for key in ("selection", "sha256", "size", "cargo_artifact")}
            for name in selections
        ],
    }


def run_checks(root: Path, *, qualification_scope: str = "basic",
               environment: dict[str, str] | None = None,
               source_commit: str | None = None, lock_fds: tuple[int, ...] = (),
               completed_independent_checks: dict[str, object] | None = None,
               update_independent_checks=None) -> None:
    """Run the gate; preparation alone may supply its exact-request checkpoint.

    The callback receives None before rerunning independent tests, then complete
    evidence after every selected independent test passed. A network failure
    never publishes scope success. Neither scope permits skipping a selected test.
    """
    scoped_stages = qualification_stages(qualification_scope)
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
    print(f"[taira-check] qualification scope {qualification_scope}; "
          f"{selected_regression_count(qualification_scope)} selected native regressions", flush=True)
    fixture_root = Path(env["CARGO_TARGET_DIR"]) if source_commit is not None else root
    if scoped_stages["network"]:
        require_network_fixture_capacity(fixture_root)
    run_pure_fsm_checks(root, env, lock_fds)
    run_lifecycle_source_checks(root, env, lock_fds)
    shipping = shipping_harnesses(root)
    print(f"[taira-check] shipping source coverage passed ({len(shipping)} binaries)", flush=True)
    # Include the configuration integration target in this same Cargo graph:
    # its separate narrower dependency feature union rebuilt shared prefixes.
    # Configuration still executes first and gates all other tests and node builds.
    # Proof bounds and CPU proof flows belong to this same independent test
    # graph. A later proof-only Cargo invocation narrows dependency features
    # and recompiles shared prefixes without adding release coverage.
    # Build early library/HTTP, network and CLI test harnesses in one Cargo graph.
    # A separate CLI test build after the production node build changes the
    # package/dev-dependency feature union and recompiles shared dependencies.
    # Run CLI contracts first so deployment argv defects surface before the
    # expensive consensus regressions, without changing the combined Cargo graph.
    # Run every independent immutable test copy before starting
    # the shipping binary graph or four-peer fixture. Aggregate test failures;
    # missing tests, artifact custody failures and other infrastructure errors
    # still stop immediately. Production binaries use a separate graph below.
    # Keep the full compile graph and its warm Cargo feature union in both
    # scopes. Deferred cases have compile coverage, never fabricated test passes.
    full_stages = qualification_stages("full")
    full_early = tuple((name, stages) for name, stages in full_stages.items()
                       if name not in {"config", "network", "cli"}
                       and (name != "kagami" or name in shipping) and stages)
    early_stages = tuple((name, scoped_stages[name]) for name, _ in full_early
                         if scoped_stages[name])
    selections = (("config",) if CONFIG_STAGES else ()) + tuple(name for name, _ in full_early)
    if full_stages["network"]:
        selections += ("network",)
    if STAGES:
        selections += ("cli",)
    selected_names = {name for name, _ in early_stages} | {"config", "network", "cli"}
    compile_only = tuple(name for name in selections if name not in selected_names)
    shipping_only = tuple(name for name in shipping if name not in selections)
    selections += shipping_only
    compile_only += shipping_only
    if selections:
        with compile_test_harnesses(root, env, lock_fds=lock_fds,
                                    harnesses=selections) as harnesses:
            for name in compile_only:
                harnesses.release(name)
            # Always rerun configuration, including exact independent-pass reuse.
            # A schema failure propagates immediately and releases the whole batch.
            run_config_checks(harnesses, fixture_root, env, lock_fds)
            failures = []
            independent_stages = ((("cli", STAGES),) if STAGES else ()) + early_stages
            checkpoint_enabled = update_independent_checks is not None
            evidence = independent_check_evidence(harnesses, independent_stages,
                                                  qualification_scope=qualification_scope) if checkpoint_enabled else None
            reuse_independent = checkpoint_enabled and completed_independent_checks == evidence
            if reuse_independent:
                print("[taira-check] reused exact independent test census and artifact pass", flush=True)
            elif update_independent_checks is not None:
                # Retire a mismatched old pass before a failed rerun could leave
                # it available to a later attempt whose artifacts happen to match.
                update_independent_checks(None)
            # Startup fixtures are part of the same canonical census/checkpoint, but
            # execute before long consensus/proof groups. Retain each immutable copy
            # until its remaining stages finish; no test runs twice or gains a skip flag.
            startup = {"core": CORE_STARTUP_STAGES, "daemon": DAEMON_STARTUP_STAGES, "torii-unit": TORII_STARTUP_STAGES}
            preflight = tuple((name, tuple(stage for stage in stages if stage in startup.get(name, ())))
                              for name, stages in early_stages)
            if STAGES:
                if not reuse_independent:
                    try:
                        run_stages(harnesses["cli"], fixture_root, env, STAGES, lock_fds)
                    except SelectedRegressionFailures as error:
                        failures.extend(error.failures)
                harnesses.release("cli")
            if not reuse_independent:
                startup_failures = []
                for name, stages in preflight:
                    if stages:
                        try:
                            run_stages(harnesses[name], fixture_root, env, stages, lock_fds)
                        except SelectedRegressionFailures as error:
                            startup_failures.extend(error.failures)
                # Collect all startup groups, then avoid expensive unrelated tests
                # when a restart's mandatory storage or policy boundary already failed.
                if startup_failures:
                    raise SelectedRegressionFailures(failures + startup_failures)
            for name, stages in early_stages:
                remaining = tuple(stage for stage in stages if stage not in startup.get(name, ()))
                if not reuse_independent and remaining:
                    try:
                        run_stages(harnesses[name], fixture_root, env, remaining, lock_fds)
                    except SelectedRegressionFailures as error:
                        failures.extend(error.failures)
                harnesses.release(name)
            if failures:
                raise SelectedRegressionFailures(failures)
            if not reuse_independent and update_independent_checks is not None:
                update_independent_checks(evidence)
            if scoped_stages["network"]:
                run_network_checks(root, fixture_root, env, lock_fds,
                                   harness=harnesses["network"], stages=scoped_stages["network"])
                harnesses.release("network")
    if source_commit is None and subprocess.check_output(["git", "--no-replace-objects", "rev-parse", "HEAD"], cwd=root, env=env,
                               stdin=subprocess.DEVNULL, text=True).strip() != head:
        raise CheckError("HEAD changed during checks; rerun against the intended source")
    print(f"[taira-check] PASS: {selected_regression_count(qualification_scope)} {qualification_scope} regressions "
          f"in {time.monotonic() - started:.1f}s", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="repository root (default: this maintained script's parent repository)")
    parser.add_argument("--target-dir", type=Path, help="existing development Cargo lane (default: sibling routine lane)")
    parser.add_argument("--native-check-scope", choices=QUALIFICATION_SCOPES, default="basic",
                        help="basic application/startup checks (default), or full advanced regressions")
    args = parser.parse_args()
    # Lazy import keeps the low-level gate loadable from an authenticated source capture.
    import taira_release as release
    try:
        release.development_check(args.repo_root, args.target_dir, dict(os.environ),
                                  native_check_scope=args.native_check_scope)
    except (CheckError, release.PrepareError, release.ReleaseArtifactError,
            release.gate.CheckError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"[taira-check] FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
