#!/usr/bin/env python3
"""Qualify basic Taira connectivity, or the full regression census, before a build.

Requires Python 3.11+, the repository Rust toolchain, and executable lsof at
/usr/sbin/lsof on macOS or /usr/bin/lsof on Linux. Both qualification and focused
checks validate this artifact-inspection prerequisite before compilation. Install
lsof with the platform package manager if absent; runtime inspection failures still
retain artifacts. Compile focused native harnesses and run a four-peer network
with isolated Cargo and fixture-only inputs. Selected beacon workloads validate
an owner-only external runtime root and its ancestors before source checks or
compilation; observation-only selections require neither that root nor peer capacity.
The existing sibling .taira-testnet-build-targets/routine lane is the default;
--target-dir or TAIRA_TESTNET_CARGO_TARGET_DIR may select another development
lane. Both selectors must agree when supplied. No Cargo lane is created or cleaned.
Native checks retain incremental compilation unless CARGO_INCREMENTAL=0 is
explicitly selected. This preference never changes Linux release compilation.
Cargo and native test children create owner-private locks, directories and
outputs independently of the caller's umask; existing unsafe artifacts still
fail custody admission. Test fixtures retain the production custody guards.
Temporary executable copies are released after their last subprocess exits,
including non-CLI native network binaries and failed checks; observations and logs
remain. The published native `iroha` CLI is retained for operator consumers; the
`cli` test harness is temporary. Busy or unverified copies are retained. Verified Cargo test outputs are recorded
in a bounded lane ledger before copy allocation; later captures retire only
recorded superseded closed test executables, preserving current outputs and caches.
On macOS, descriptor-bound copy-on-write clones avoid full duplicate allocation
while retaining independent inodes and exact content/stat validation. Unsupported
filesystems stream only when all remaining copies fit beside the working reserve;
later Cargo writes can still allocate new blocks for changed cloned content.
The default basic scope keeps deployment custody, authentication, application and
startup admission checks plus real four-validator Applied transactions and restart.
After configuration, explicit MV ownership stages and the complete admitted-map,
Concread admission/writer/checkpoint source census execute before exact Pending Kura
recovery controls. Either prerequisite stops qualification on failure before
other startup checks, shipping builds or network execution. These controls use
the same complete native compile graph. Focused development checks execute
selected MV targets before building mandatory configuration in the same lane;
that separate diagnostic graph never qualifies a release.
After mandatory startup checks, both scopes separately metadata-check every
authoritative shipping binary with default production features before CLI and
long independent tests. This check also reruns when an independent checkpoint
is reused; it supplies no test pass or artifact qualification. Later shipping
codegen and network execution remain required.
Full additionally executes advanced Core recovery and proof-production matrices.
Both scopes require strict runtime catalog readback codecs and the lifecycle HTTP
endpoint, compiled in the same native graph; no runtime security policy is relaxed.

Configuration and compiler paths match authenticated preparation, while source
remains the mutable checkout. These checks never qualify release artifacts and
accept no live configuration, credentials, SSH, deployment or signing inputs.
Repeat --focus-regression HARNESS=EXACT_TEST for prequalification: metadata-check
and compile only explicitly selected harnesses. Selected portable MV/Concread
controls run first in a separate diagnostic Cargo graph. Configuration and the
remaining targets build afterward in the same warm lane; configuration must pass
before nonportable tests and overall success. Unselected harnesses wait for
immutable preparation, whose complete compile graph is unchanged. Selected Pending
Kura recovery runs immediately after configuration and must pass before the
remaining nonportable focused regressions.
The metadata pass catches type/import errors early; the
selected build still detects codegen-only errors. This diagnostic writes no qualification checkpoint and
does not replace immutable preparation or its complete gate.
Linux development checks default to LLVM 18, requiring executable /usr/bin/clang-18
and /usr/bin/ld.lld-18 before compilation. Missing tools fail without fallback;
install clang-18 and lld-18 with the platform package manager, or explicitly select
--native-linker system for diagnosis. macOS keeps Apple ld. Switching linkers
invalidates Cargo fingerprints and can rebuild dependencies once. Authenticated
preparation pins its native linker pair separately from shipping Zig. The same
coordinated pair reaches Cargo and the direct-rustc standalone checks; arbitrary
inherited compiler flags remain excluded.
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
import threading
import time
import tomllib
import uuid


STAGES = (
    ("core canary command composition", (
        "taira_public_reset::host::tests::coordinator_write_canary_argv_passes_child_validation_for_all_core_actions",
        "taira::tests::final_canary_predecessor_requires_its_independent_faucet_policy",
        "taira::tests::write_canary_policy_inputs_are_operation_and_action_scoped",
    )),
    ("native public reset input preparation", (
        "taira_public_reset::public_inputs::tests::derives_native_genesis_identity_and_exact_canary_request",
        "taira_public_reset::public_inputs::tests::rejects_wrong_network_key_and_resultless_genesis",
        "taira_public_reset::public_inputs::tests::rejects_noncanonical_identity_and_non_ed25519_canary",
        "taira_public_reset::public_inputs::tests::publishes_complete_public_bundle_and_reuses_identical_request",
        "taira_public_reset::public_inputs::tests::refuses_changed_bundle_and_never_overwrites_existing_output",
        "taira_public_reset::public_inputs::tests::rejects_symlink_input_and_partial_or_surplus_output",
        "taira_public_reset::public_inputs::tests::cli_public_input_preparation_never_accepts_private_credentials",
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
        "tests::authorized_transaction_lifetime_uses_exact_creation_and_preserves_shorter_ttl",
        "tests::authorized_transaction_lifetime_rejects_empty_window_and_missing_ttl",
        "taira::tests::final_canary_expired_window_rejects_before_fee_quote_or_dispatch",
        "taira::tests::final_canary_submit_uses_original_deadline_after_initial_read_and_post",
        "taira::tests::final_canary_submit_verifies_exact_proof_without_replaying_post",
        "taira::tests::faucet_preparation_deadline_stops_http_before_dispatch",
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
        "taira::tests::doctor_faucet_policy_checks_both_scopes_without_authentication",
        "taira::tests::doctor_tools_list_consumes_pages_and_rejects_invalid_cursors",
        "taira::tests::doctor_reports_bounded_mcp_application_error_codes",
        "taira::tests::doctor_mock_healthy_flow_reports_ok",
        "taira::tests::time_snapshot_requires_network_time_and_every_health_axis",
        "taira::tests::doctor_rejects_unknown_namespaces_or_malformed_mcp_tools",
        "taira::tests::doctor_mock_required_tool_missing_reports_failure",
        "taira_public_reset::host::tests::doctor_report_requires_the_exact_first_release_check_surface",
    )),
    ("complete effective account permission reads", (
        "tests::account_permission_list_reads_complete_effective_fanout_before_global_pagination",
        "tests::account_permission_list_rejects_partial_or_non_effective_pages_without_output",
        "tests::account_permission_list_rejects_zero_pagination_before_http",
        "tests::account_permission_list_propagates_server_page_cap_rejection",
    )),
    ("public account key conversion", (
        "address::tests::public_key_output_roundtrips_taira_i105_and_cli_format",
        "address::tests::public_key_output_rejects_multisig",
        "address::tests::public_key_output_rejects_malformed_and_wrong_prefix",
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
    ("candidate funding policy admission", (
        "taira_public_reset::inputs::tests::validator_faucet_policy_requires_enabled_exact_signed_intent",
        "taira_public_reset::inputs::tests::pinned_validator_configs_reject_faucet_policy_mismatch_before_dispatch",
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
    ("occupied runtime and service unit recovery", (
        "taira_public_reset::host::occupied::tests::occupied_runtime_rejects_builder_tools_and_each_missing_runtime_role",
        "taira_public_reset::host::tests::cleanup_preserves_prior_release_during_discovery_and_replay",
        "taira_public_reset::host::occupied::tests::occupied_runtime_accepts_split_source_and_configuration_binding",
        "taira_public_reset::host::occupied::tests::occupied_runtime_rejects_incomplete_or_foreign_artifact_custody",
        "taira_public_reset::host::occupied::tests::occupied_runtime_wire_requires_explicit_artifacts_and_argv",
        "taira_public_reset::host::occupied::tests::occupied_runtime_process_binding_distinguishes_prior_and_candidate",
        "taira_public_reset::host::occupied::tests::occupied_runtime_cleanup_preserves_every_prior_artifact_root",
        "taira_public_reset::host::occupied::tests::occupied_unit_transition_rejects_unbound_recovery_and_preserves_rollback",
        "taira_public_reset::host::occupied::tests::occupied_unit_publication_recovers_interrupted_forward_and_rollback",
        "taira_public_reset::host::occupied::tests::occupied_unit_publication_rejects_destination_or_staging_drift",
        "taira_public_reset::host::occupied::tests::occupied_unit_reload_requires_exact_terminal_manager_evidence",
        "taira_public_reset::host::occupied::tests::occupied_unit_copy_preserves_exact_mode_and_rejects_retained_mode_drift",
        "taira_public_reset::host::occupied::tests::pinned_reader_rejects_oversized_snapshot_before_allocation_and_allows_empty",
    )),
    ("generated validator reset layout", (
        "taira_public_reset::validator_config::tests::materialization_binds_every_validator_state_path_and_preserves_other_fields",
        "taira_public_reset::validator_config::tests::materialization_projects_split_torii_bind_without_changing_p2p_or_signer_custody",
        "taira_public_reset::validator_config::tests::materialization_rejects_invalid_torii_listener_and_port_drift",
        "taira_public_reset::validator_config::tests::materialization_torii_bind_argument_requires_canonical_ip_and_nonzero_port",
        "taira_public_reset::validator_config::tests::materialization_rejects_changed_missing_and_wrong_peer_state_paths",
        "taira_public_reset::validator_config::tests::materialization_rejects_inheritance_identity_drift_and_source_bindings",
        "taira_public_reset::validator_config::tests::materialization_requires_exact_public_genesis_identity_bytes",
        "taira_public_reset::validator_config::tests::materialization_cli_requires_explicit_custody_and_canonical_identities",
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
        "taira_public_reset::executor_model::tests::validator_public_origins_require_distinct_canonical_https_roots",
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
    ("signed stopped occupied predecessor", (
        'taira_public_reset::executor_model::tests::occupied_service_state_is_explicit_strict_and_signed',
        'taira_public_reset::executor_model::tests::stopped_state_identity_survives_archive_restore_and_rejects_substitution',
        'taira_public_reset::host::tests::stopped_predecessor_absence_checks_cgroup_and_escaped_references',
        'taira_public_reset::host::tests::prior_service_state_never_restarts_stopped_or_falls_back_from_running',
        'taira_public_reset::host::occupied::tests::stopped_unit_admission_requires_the_exact_prior_or_durable_successor',
    )),
    ("stopped owner runtime cleanup", (
        "taira_public_reset::host::maintenance::tests::maintenance_scope_binds_all_four_units_and_failed_installed_runtime",
        "taira_public_reset::host::maintenance::tests::maintenance_flock_requires_one_exact_live_updater_owner",
        "taira_public_reset::host::maintenance::tests::maintenance_process_identity_handles_names_and_rejects_dead_owner",
        "taira_public_reset::host::stopped_runtime::tests::stopped_owner_cohort_preflight_preserves_workers_until_every_slot_is_admitted",
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

STAGES += (("exact authenticated transaction lookup", (
    "tests::transaction_get_uses_exact_authenticated_details_and_preserves_rejection",
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
CLIENT_STAGES += (("native query failure envelope and absence semantics", (
    "query::tests::garbled_not_found_remains_a_protocol_error",
    "query::tests::garbled_gone_remains_a_protocol_error",
    "query::query_errors_handling::query_error_envelope_preserves_missing_asset_diagnostic",
    "query::query_errors_handling::query_error_envelope_rejects_wrong_media_and_noncanonical_bytes",
    "query::query_errors_handling::query_error_envelope_preserves_service_failure_without_cursor_inference",
)),)
CLIENT_STAGES += (("strict lifecycle status client contract", (
    "client::status_tests::lane_lifecycle_status_decodes_json_and_norito",
    "client::status_tests::lane_lifecycle_status_rejects_missing_or_empty_runtime_catalog_hash",
    "client::status_tests::lane_lifecycle_status_rejects_forged_commitment_and_malformed_payload",
    "client::status_tests::lane_lifecycle_status_requires_declared_current_media_type",
    "client::tests::get_lane_lifecycle_status_requests_typed_negotiated_snapshot",
)),)

CLIENT_STAGES += (("strict native parameters response", (
    "client::tests::decode_parameters_response_parses_json_payload",
)),)

CLIENT_STAGES += (("canonical executed block execution commitments", (
    "client::evidence_http_tests::canonical_executed_block_reader_binds_route_wire_and_committed_evidence",
    "client::evidence_http_tests::canonical_executed_block_reader_rejects_trailing_wire_and_wrong_carrier_hash",
    "client::evidence_http_tests::canonical_executed_block_reader_requires_authenticated_execution_commitment",
)),)

STAGES += (("exact signed asset lookup and native missing-asset diagnostics", (
    "tests::ledger_asset_get_uses_exact_singular_query_and_preserves_missing_asset_diagnostic",
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
)), ("committed catalog startup reconstruction", (
    "startup_runtime_catalog_tests::startup_catalog_freezes_only_baseline_files_and_reconstructs_world_manifest",
    "startup_runtime_catalog_tests::startup_catalog_handoff_includes_additions_committed_during_replay",
)),)
DAEMON_STAGES += DAEMON_STARTUP_STAGES

TORII_STARTUP_STAGES = (("configured initial catalog and explicit network identity", (
    "tests_runtime_handlers::configured_catalog_fixture_binds_initial_geometry_and_explicit_network",
)), ("HTTP admission waits for Queue startup reconciliation", (
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
TORII_STARTUP_STAGES += (("contract read authority and delegated ingress", (
    "tests_runtime_handlers::contract_route_mounts_authenticate_mutation_and_compute_before_decode",
    "tests_runtime_handlers::contract_compute_routes_bind_authenticated_authority_before_work",
    "tests_runtime_handlers::torii_delegated_reads_reject_online_only_observers",
    "torii_routed_read_tests::routed_contract_views_require_bound_caller",
    "torii_routed_read_tests::protected_contract_views_ignore_unsigned_public_upstream",
    "app_api::tests::contract_view_dispatch_requires_bound_authenticated_authority",
)),)
TORII_STARTUP_STAGES += (("onboarding DPN grant authority", (
    "tests::onboarding_readiness_dpn_user_requires_exact_direct_admin",
    "tests::onboarding_readiness_dpn_user_rejects_role_derived_admin",
    "tests::onboarding_readiness_default_permissions_do_not_require_dpn_admin",
    "tests::onboarding_readiness_dpn_user_is_pending_while_joining_state_is_empty",
    "tests::onboarding_readiness_is_pending_while_joining_state_is_empty",
    "tests::onboarding_readiness_payment_asset_mismatch_is_blocked_while_joining_state_is_empty",
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

TORII_UNIT_STAGES += (("signed account permission query preservation", (
    "torii_routed_read_tests::account_permissions_handler_query_preserves_signed_pagination_and_count_mode",
)),)

DISPATCHER_TRANSITION_STAGES = (("reversible dispatcher upgrade and native plan preparation", (
    "taira_public_reset::host::dispatcher_transition::tests::copy::dispatcher_transition_private_copy_modes_survive_restrictive_umask",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_apply_and_rollback_preserve_exact_original_bytes",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_completed_replays_do_not_republish",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_interrupted_publication_resumes_every_checked_boundary",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rollback_from_every_partial_guard_publication",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_interrupted_rollback_resumes",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_changed_predecessor_refuses_rollback_before_barrier",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rejects_foreign_guard_and_backup",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rejects_same_bytes_replaced_inode",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rejects_foreign_namespace_and_plan",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rejects_dangling_symlink_as_absence",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_refuses_unowned_missing_guard",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_cli_requires_exact_plan_pin_and_action",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_accepts_exact_sealed_completed_predecessor",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_rejects_unsealed_rollback_and_foreign_lease",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_derives_guards_without_changing_existing_trust_or_roles",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_staging_publication_crashes_resume_only_owned_prefixes",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_inode_scan_rejects_alias_executable_own_fd_and_maps",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_requires_complete_qualified_transfer_producer_join",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_requires_native_aarch64_elf_header",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_prepare_reuses_current_typed_split_source_bindings",
    "taira_public_reset::host::dispatcher_transition::tests::dispatcher_transition_prepare_cli_requires_pinned_native_inputs",
)), )
STAGES += DISPATCHER_TRANSITION_STAGES

TORII_ADMISSION_HANDOFF_STAGES = (("bounded transaction admission and exact receipt ownership", (
    "queue_plan_capacity_wait::tests::closed_owner_waits_and_rechecks_until_activation",
    "queue_plan_capacity_wait::tests::only_inactive_is_waited_and_terminal_change_is_immediate",
    "queue_plan_capacity_wait::tests::original_monotonic_and_wire_deadlines_are_not_renewed",
    "queue_plan_capacity_wait::tests::cancellation_drops_wait_without_detached_checks",
    "tests_runtime_handlers::incoming_queue_plan_handoff_waits_without_claim_then_attests_exact_request",
    "tests_runtime_handlers::ingress_queue_plan_handoff_cancellation_releases_memory_without_claim",
    "tests_runtime_handlers::forwarded_queue_plan_handoff_preflight_preserves_request_and_deadline",
    "tests_runtime_handlers::incoming_queue_plan_handoff_expiry_never_creates_journal_claim",
    "tests_runtime_handlers::queue_plan_handoff_after_quorum_retains_certificate_and_times_out_indeterminate",
    "tests_runtime_handlers::incoming_queue_plan_handoff_partial_journal_retry_preserves_uncertainty",
    "tests_runtime_handlers::queue_plan_handoff_after_quorum_resumes_exact_certificate_publication",
    "tests_runtime_handlers::queue_plan_handoff_expiry_before_aggregation_preserves_partial_journal_uncertainty",
    "tests_runtime_handlers::incoming_queue_plan_expired_retry_preserves_partial_journal_uncertainty",
    "tests_runtime_handlers::incoming_queue_plan_capacity_unavailable_never_creates_a_journal_claim",
    "tests_runtime_handlers::queue_plan_native_capacity_refuses_direct_and_ingress_promises_before_journal",
    "tests_runtime_handlers::queue_plan_capacity_loss_after_quorum_remains_indeterminate",
    "tests_runtime_handlers::queue_plan_synced_future_authority_retries_same_request_until_quorum",
    "tests_runtime_handlers::queue_plan_synced_persistent_future_preserves_partial_claim_at_deadline",
    "tests_runtime_handlers::queue_plan_synced_deadline_cancels_only_its_owned_waiter",
    "tests_runtime_handlers::queue_plan_synced_other_rejections_do_not_rearm_partial_admission",
)),)
TORII_STARTUP_STAGES += TORII_ADMISSION_HANDOFF_STAGES
TORII_UNIT_STAGES += TORII_ADMISSION_HANDOFF_STAGES

TORII_UNIT_STAGES += (("canonical lifecycle status runtime root and public schema", (
    "routing::nexus_lane_lifecycle_tests::lane_lifecycle_status_binds_exact_current_catalog",
    "routing::nexus_lane_lifecycle_tests::lane_lifecycle_status_exposes_native_runtime_root_and_propagates_invalid_state",
    "openapi::tests::compact_finality_app_contracts::generated_spec_documents_read_only_nexus_lifecycle_status",
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
    "sumeragi::v2_apply::tests::ordinary_lane_frontier_preserves_third_certified_source_after_merge_execution_rejection",
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
)), ("fee sponsor activation and public fee admission", (
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
    "block::tests::public_contract_creation_fees::public_contract_artifact_stages_pay_fees_without_management_grants",
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
CORE_ADMISSION_STARTUP_STAGES += (("autonomous lane gas selection and shared merge budget", (
    "block::valid::tests::autonomous_anchor_gas_budget_enforces_complete_source_before_anchoring",
    "sumeragi::v2_lane_work::tests::autonomous_full_block_gas_call_reserves_with_idle_catalog_route",
    "state::tests::autonomous_full_gas_sources_share_one_merge_budget_before_execution",
    "state::tests::autonomous_merge_gas_priority_preserves_old_source_and_canonical_order",
    "state::tests::autonomous_merge_gas_accounting_rejects_missing_limit_and_overflow",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("current reducer mode and fresh queue pressure", (
    "telemetry::tests::public_mode_tracks_frozen_reducer_context_and_clears_without_owner",
    "telemetry::tests::queue_backpressure_metrics_updated",
    "telemetry::tests::queue_age_pressure_is_not_capacity_backpressure",
    "telemetry::tests::fresh_queue_metrics_replace_stale_pressure_on_an_idle_node",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("nested failure closes admission without blocking its owner", (
    "sumeragi::v2_effects::tests::executor_fatal_callbacks_close_before_outer_operation_releases",
    "sumeragi::v2_worker::tests::service_failure_and_drop_finish_before_outer_operation_drains",
    "sumeragi::v2_worker::tests::abnormal_io_worker_exit_finishes_before_outer_operation_drains",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("live Decision cleanup after an idle runtime turn", (
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::live_idle_decision_cleanup_reconciles_runner_frontier",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("recovered Decision Fetch and periodic runtime ownership", (
    "sumeragi::v2_effects::tests::recovered_decision_fetch_fences_later_ordinary_body_coordinates",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_body_owners_cold_reopen_under_durable_commit",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_validate_cold_reopen_after_timeout_and_durable_commit",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("cold Decision body publication and owner-open ledger recovery", (
    "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_commits_catalogs_and_marker_together",
    "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_rejects_partial_or_conflicting_catalogs",
    "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_rejects_overlapping_body_stage",
    "sumeragi::v2_effects::tests::certified_body_fence_supersession::cold_decision_fetch_publishes_first_network_body_through_completion_and_apply",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_decision_factory_publishes_one_authenticated_owner_open_chain",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_nonempty_successor_consumes_only_the_exact_owner_open_witness",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::owner_open_publication_chain_requires_every_exact_cas_and_is_consumed_once",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("authenticated retained body custody and proposal recovery", (
    "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_restores_work_without_voting_authority",
    "sumeragi::v2_core::reducer::source_link_tests::retained_local_body_custody_coalesces_without_downgrading_or_revalidating",
    "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_rejects_foreign_identity_and_safety_debt_atomically",
    "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_respects_the_exact_durable_decision",
    "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_preserves_normal_proposal_validation_vote_authority",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_restores_proposal_validate_without_wal_authority",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_coalesces_proposal_validate_with_retained_prepare_qc",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_cancels_timeout_superseded_body_before_replay",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_preserves_current_body_after_timeout_recovery",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_rejects_future_body_generation_without_retirement",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("Proposal authority handoff and exact restart recovery", (
    "sumeragi::v2_effects::tests::hybrid_proposal_fetch_completes_store_and_validate_with_exact_replay_root",
    "sumeragi::v2_effects::tests::proposal_fetch_store_refinement_rejects_foreign_root_and_coordinates",
    "sumeragi::v2_runtime::tests::authenticated_proposal_store_retains_root_after_fetch_or_queued_completion_upgrade",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_cancels_only_exact_proposal_child_below_installed_view",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_cancels_same_view_proposal_after_authenticated_decision_without_timeout",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_decision_proposal_cancellation_preserves_authentication_boundaries",
    "sumeragi::v2::tests::production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    "sumeragi::v2::tests::production_complete_tip_activates_recovered_unapplied_decision",
    "sumeragi::v2::tests::complete_tip_decision_activation_requires_exact_replayed_wal",
    "sumeragi::v2::tests::complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state",
    "sumeragi::v2::tests::complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache",
    "sumeragi::v2_core::refinement::tests::recovered_decided_successor_kernel_keeps_canonical_parent_and_commit_frontier_distinct",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_current_and_future_proposal_cancellation",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_proposal_without_authenticated_installed_timeout",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_foreign_installed_timeout_frontier",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_unlinked_proposal_despite_another_exact_sign_parent",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_tampered_proposal_even_with_exact_parent_and_later_timeout",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_rejects_forged_timeout_cancellation_frontier",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_proposal_cancellation_fsync_preserves_row_and_skips_output_service",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_proposal_cancellation_waits_for_older_ready_output",
    "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_proposal_cancellation_fsync_failure_retains_ready_owner_without_output",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("bounded fair-ingress ownership projection work", (
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::frozen_ownership_peer_encoding_work_is_bounded_by_distinct_peers",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::cached_peer_encodings_preserve_forged_history_and_sender_rejection",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("same-round timeout recovery and bounded frontier reads", (
    "sumeragi::v2::tests::same_round_timeout_cancellation_uses_exact_durable_proposal_intent",
    "sumeragi::v2::tests::same_round_timeout_cold_owner_cancels_exact_retained_proposal",
    "sumeragi::lane_planner::tests::canonical_frontier_reads_scale_with_distinct_routes_including_absence",
    "sumeragi::lane_planner::tests::canonical_frontier_reads_preserve_first_storage_failure_and_stop",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("terminal validation history and shared outcome recovery", (
    "sumeragi::v2::tests::same_round_timeout_cold_owner_preserves_retired_terminal_validation_history",
    "sumeragi::v2::tests::same_round_timeout_cold_owner_publishes_broadcast_after_retired_validation_history",
    "sumeragi::v2::tests::same_round_timeout_cold_owner_reconciles_standalone_broadcast",
    "sumeragi::v2::tests::same_round_timeout_cold_owner_rejects_foreign_standalone_broadcast",
    "sumeragi::v2_body_store::tests::terminal_validate_shared_outcomes_keep_one_latest_retry_origin",
    "sumeragi::v2_body_store::tests::retired_terminal_claim_comparison_never_promotes_marker_authority",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("atomic committed catalog authority and preserved history", (
    "lane_consensus::tests::canonical_recovery_restores_only_an_exact_complete_drained_handoff",
    "sumeragi::v2_lane_work::tests::canonical_lane_recovery_restores_handoff_after_losing_carrier_retirement",
    "kura::tests::consensus_certificate_read_rejects_occupied_corruption_without_repair",
    "sumeragi::v2_lane_work::tests::same_proposal_shortcut_rejects_unvalidated_certificate_variants",
    "kura::tests::canonical_autonomous_replica_corruption_and_wrong_context_fail_closed",
    "state::runtime_configuration_tests::runtime_nexus_setter_preserves_configured_dataspaces_and_rejects_post_genesis_drift",
    "state::runtime_configuration_tests::runtime_nexus_setter_requires_exact_protected_dataspace_projection",
    "state::runtime_catalog_tests::runtime_catalog_preflight_preserves_prior_additions_and_rejects_replacement",
    "state::runtime_catalog_tests::runtime_catalog_stages_dataspace_lane_manifest_atomically_with_four_live_pops",
    "state::runtime_catalog_tests::runtime_catalog_rejects_ineligible_committee_without_partial_state",
    "state::runtime_catalog_tests::runtime_catalog_rejects_stale_roots_and_genesis_without_partial_state",
    "state::runtime_catalog_tests::runtime_catalog_accessor_rejects_malformed_protected_state_and_baseline_drift",
    "state::runtime_catalog_tests::runtime_catalog_final_overlay_rejects_unstaged_changed_and_removed_parameter",
    "state::runtime_catalog_tests::runtime_catalog_applied_transaction_publishes_manifest_to_next_transaction",
    "state::runtime_catalog_tests::runtime_catalog_final_overlay_rechecks_late_validator_invalidation",
    "state::runtime_catalog_tests::runtime_catalog_final_overlay_rejects_removal_and_unchanged_malformed_state",
    "state::runtime_catalog_tests::runtime_catalog_startup_reconstructs_manifest_without_files_and_preserves_policy",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_preserves_baseline_and_rebuilds_cumulatively",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_rejects_takeover_duplicates_and_schema_drift_atomically",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_quorum_tracks_dataspace_fault_tolerance",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_never_loads_deferred_paths_and_preserves_manual_rebind",
    "governance::manifest::runtime_overlay::tests::manifest_catalog_binding_rejects_stale_refresh_after_source_preserving_lifecycle",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_retains_frozen_file_source_and_rejects_alias_takeover",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_governed_lane_is_ready_without_a_filesystem_path",
    "governance::manifest::runtime_overlay::tests::runtime_manifest_overlay_rejects_private_torii_urls_before_publication",
    "governance::manifest::tests::manifest_rejects_invalid_validator_torii_url",
    "governance::manifest::tests::builder_allows_runtime_sources_but_requires_parsed_rules",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("committed runtime catalog readback and next transition authority", (
    "state::runtime_catalog_tests::runtime_catalog_readback_tracks_committed_state_and_rejects_malformed_parameter",
    "state::runtime_catalog_tests::runtime_catalog_readback_binds_next_transition_and_rejects_stale_root",
)),)
CORE_ADMISSION_STARTUP_STAGES += (("certified runtime catalog and parameter commit effects", (
    "state::tests::autonomous_runtime_catalog_effects_commit_and_recover_exactly",
    "state::tests::autonomous_bootstrap_parameter_effects_commit_and_recover_exactly",
    "state::tests::autonomous_runtime_catalog_effects_reject_post_stage_tampering",
    "state::tests::autonomous_parameter_effects_reject_post_stage_tampering",
    "state::tests::autonomous_runtime_catalog_effects_require_matching_pending_transition",
)),)

CORE_ADMISSION_STARTUP_STAGES += (("governance sweep execution fragments", (
    "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
    "state::tests::block_sweeps_expired_governance_locks_and_records_height",
    "state::tests::block_retains_expired_governance_lock_when_atomic_release_fails",
)),)

CORE_ADMISSION_STARTUP_STAGES += (("retained and compacted Kura replay floors", (
    "kura::lane_geometry::tests::configured_primary_replay_preflight_is_read_only_when_floor_is_retained",
    "kura::lane_geometry::tests::configured_primary_replay_preflight_requires_snapshot_after_compaction",
)),)

CORE_ADMISSION_STARTUP_STAGES += (("certified historical recovery and exact live QueuePlan ownership", (
    "state::tests::historical_autonomous_merge_recovers_certified_carrier_before_world_replay",
    "state::tests::live_autonomous_merge_requires_exact_pending_queue_plan_owner",
    "state::tests::autonomous_merge_rejects_reforged_reservation_bindings",
    "state::tests::historical_autonomous_merge_rejects_restored_registry_conflict",
)),)

CORE_ADMISSION_STARTUP_STAGES += (('typed native SNS registration absence', (
    'sns::tests::registration_absence_is_distinct_from_policy_and_malformed_state',
)), )

CORE_ADMISSION_STARTUP_STAGES += (("authenticated replay against isolated committed state", (
    "block::valid::tests::authenticated_replay_added_lane_uses_replicated_frontier_without_published_storage",
    "block::valid::tests::authenticated_replay_authority_rejects_different_proposal_wire_and_state_prefix",
    "block::valid::tests::authenticated_replay_lane_predecessor_requires_exact_replicated_height_and_hash",
    "block::valid::tests::authenticated_replay_ordinary_and_native_amx_keep_exact_predecessors_without_live_slots",
    "state::tests::da_hydration_test_cases::replay_private_da_hydration_reconstructs_exact_prefix_and_rejects_wrong_body",
    "state::replay_validation_tests::replay_uncached_da_prefix_keeps_shared_journal_unchanged_until_publication",
    "state::replay_lane_drain::tests::replay_native_drain_frontier_binds_marker_prefix_and_certificate_evidence",
    "state::tests::pending_drain_body_and_candidate_use_embedded_close_committee_after_roster_change",
    "state::tests::retired_lane_cleanup_preserves_frontier_for_historical_drain_recovery",
)),)

CORE_ADMISSION_STARTUP_STAGES += (("authenticated replay geometry and deferred startup writers", (
    "kura::tests::startup_replay_geometry_transition_preserves_shared_binding_for_added_lane",
    "kura::tests::startup_replay_geometry_transition_rejects_checkpoint_and_manifest_drift",
    "kura::tests::startup_replay_geometry_transition_rejects_restored_lane_sidecar_drift",
    "kura::tests::startup_replay_geometry_transition_preserves_relabelled_and_retired_path_guards",
    "kura::tests::startup_replay_geometry_transition_rejects_unretained_request",
    "kura::tests::startup_replay_geometry_transition_creates_only_missing_retained_namespace_and_cleans_failure",
    "sumeragi::startup_recovery::tests::maintenance_waits_for_recovery_before_budget_or_snapshot_writes",
    "sumeragi::startup_recovery::tests::maintenance_refuses_failed_dropped_and_shutdown_recovery",
    "sumeragi::startup_recovery::tests::maintenance_retains_success_for_delayed_readonly_snapshot_subscriber",
    "sumeragi::startup_recovery::tests::snapshot_loop_stops_on_worker_failure_without_final_shutdown_write",
    "sumeragi::v2_runner::tests::authenticated_terminal_startup_idles_without_constructing_a_successor",
    "block::valid::tests::account_profile_validation_preserves_delegated_metadata_results",
    "block::valid::tests::account_profile_validation_rejects_foreign_permission_payloads",
)),)

CORE_ADMISSION_STARTUP_STAGES += (("unconditional alias registry admission and replay", (
    "queue::router::alias_registry_routing_tests::alias_registry_routing_paid_post_genesis_dataspace_domain_and_renewal",
    "queue::router::alias_registry_routing_tests::alias_registry_routing_is_independent_of_height_and_catalog",
    "queue::router::alias_registry_routing_tests::alias_registry_routing_nested_walkers_use_universal_registry",
    "queue::router::alias_registry_routing_tests::alias_registry_routing_does_not_bypass_id_owner_quote_or_catalog_guards",
    "queue::router::alias_registry_routing_tests::alias_registry_routing_keeps_real_private_participants_in_mixed_transactions",
    "queue::router::alias_registry_routing_tests::alias_registry_routing_cold_replay_with_expanded_catalog_preserves_paid_bootstrap",
    "queue::router::tests::alias_registry_routing_is_unconditional_for_queue_and_replay",
)), )

CORE_ADMISSION_STARTUP_STAGES += (("finite closed ingress and fresh finalized handoff", (
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_snapshot_tracks_live_depth_and_oldest_age",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_closed_drained_cut_rejects_each_stale_lane_account",
    "sumeragi::v2_runner::tests::finalized_closed_prefix_retires_historical_lane_certificate_without_adapter_admission",
    "sumeragi::v2_worker::tests::prepared_historical_body_capacity_recovers_from_applied_finality_without_peer_delivery",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_terminal_apply_store_join_rejects_store_drift",
    "sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff",
)), )

CORE_ADMISSION_STARTUP_STAGES += (("bounded deterministic IPA startup parameters", (
    'zk::zkparse::production_parameter_cache_tests::finite_production_cache_initializes_once_across_threads',
    'zk::zkparse::production_parameter_cache_tests::finite_production_cache_matches_native_parameter_bytes_and_fingerprint',
    'zk::zkparse::production_parameter_cache_tests::finite_production_cache_rejects_unadmitted_domains_without_construction',
    'zk::halo2_ipa_parameter_source_tests::production_parameter_source_rejects_duplicate_and_mismatched_metadata',
    'zk::halo2_ipa_parameter_source_tests::production_parameter_source_rejects_unbounded_k_before_construction',
    'zk::debug_backend_tests::halo2_ivm_execution_rejects_relabelled_demo_verifying_key',
)), )

CORE_PENDING_KURA_RECOVERY_STAGES = (("standalone and linked Apply recovery across retained Kura shutdown", (
    'sumeragi::v2::tests::pending_kura_standalone_apply_recovers_real_kura_shutdown_cut',
    'sumeragi::v2::tests::pending_kura_standalone_apply_rejects_foreign_owner_without_mutation',
    'sumeragi::v2::tests::pending_kura_linked_apply_recovers_real_kura_shutdown_cut',
    'sumeragi::v2::tests::pending_kura_linked_apply_rejects_changed_parent_and_decision_without_mutation',
    'sumeragi::v2::tests::pending_kura_recovered_decision_chain_recovers_real_kura_shutdown_cut',
    'sumeragi::v2::tests::pending_kura_validated_apply_preview_rejects_foreign_authority_and_fence_exhaustion_inertly',
)), )
CORE_ADMISSION_STARTUP_STAGES += CORE_PENDING_KURA_RECOVERY_STAGES


CORE_ADMISSION_STARTUP_STAGES += (("typed State status contention and integrity boundary", (
    'state::telemetry_status::tests::status_source_busy_is_distinct_from_changed_or_invalid_journal',
)), )


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

CORE_READ_BOUNDARY_STAGES = (("bounded lane recovery and strict durable evidence reads", (
    'kura::tests::autonomous_latest_snapshot_reuses_validated_current_cursor',
    'kura::tests::autonomous_completion_selected_view_rejects_corruption_and_foreign_suffix',
    'kura::tests::certified_lane_block_read_rejects_qc_signature_mismatch',
    'kura::tests::certified_lane_block_read_rejects_qc_body_mismatch',
    'sumeragi::v2_runner::tests::open_preflight_batch_services_queued_prepare_and_commit_before_reaudit',
    'sumeragi::v2_runner::tests::open_preflight_batch_preserves_budget_completion_yield_and_errors',
    'sumeragi::v2_runner::tests::open_preflight_batch_does_not_admit_global_traffic_as_lane_recovery',
    'sumeragi::v2_lane_work::tests::historical_autonomous_hydration_replaces_same_slot_conflict_at_capacity',
    'sumeragi::v2_lane_work::tests::historical_autonomous_hydration_preserves_conflicting_quorum_at_capacity',
    'sumeragi::v2_lane_work::tests::finalized_carrier_nonmember_cache_invalid_commit_certificate_rolls_back_hydration',
    'sumeragi::v2_lane_work::tests::global_validator_outside_lane_committee_uses_canonical_replica_for_rollover',
    'kura::tests::certified_lane_block_rejects_foreign_active_dataspace',
    'kura::tests::autonomous_completion_missing_view_state_keeps_full_payload_validation',
    'kura::tests::autonomous_completion_selected_view_validates_artifact_once',
)), )
CORE_STAGES = CORE_READ_BOUNDARY_STAGES + CORE_STAGES
# Keep the existing full-scope position while requiring this affected control early in basic.
CORE_ADMISSION_STARTUP_STAGES = CORE_READ_BOUNDARY_STAGES + (("bounded open preflight and finite closed ingress", (
    'sumeragi::v2_runner::tests::terminal_finalization_limits_open_ingress_to_lane_preflight_before_the_finite_closed_drain',
)), ) + CORE_ADMISSION_STARTUP_STAGES

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

CONFIG_UNIT_STAGES = (("explicit onboarding permission configuration", (
    "parameters::user::duration_clamp_tests::account_onboarding_accepts_explicit_dpn_user_permission",
    "parameters::user::duration_clamp_tests::account_onboarding_defaults_to_no_additional_permissions",
    "parameters::user::duration_clamp_tests::account_onboarding_rejects_unsupported_and_scoped_additional_permissions",
    "parameters::user::duration_clamp_tests::account_onboarding_rejects_duplicate_dpn_user_permission",
)), ("immutable baseline and committed runtime catalog policy", (
    "parameters::actual::tests::nexus_consensus_policy_digest_keeps_configured_dataspaces_during_runtime_addition",
    "parameters::actual::tests::nexus_consensus_policy_digest_changes_for_execution_and_da_policy_drift",
    "parameters::actual::tests::nexus_consensus_policy_digest_canonicalizes_dataspace_catalog_order",
    "parameters::actual::tests::sumeragi_v2_default_nexus_amx_hash_is_stable",
    "parameters::actual::tests::sumeragi_v2_nexus_amx_hash_binds_committed_catalog_policy",
)),)

DATA_MODEL_STAGES = (("bounded canonical additive catalog parameters", (
    "nexus::runtime_catalog::tests::additive_catalog_parameters_roundtrip_without_losing_canonical_identity",
    "nexus::runtime_catalog::tests::additive_catalog_rejects_unknown_duplicate_fields_and_wrong_versions",
    "nexus::runtime_catalog::tests::additive_catalog_rejects_empty_noncanonical_and_duplicate_entries",
    "nexus::runtime_catalog::tests::additive_catalog_binds_identity_hashes_and_committee_geometry",
    "nexus::runtime_catalog::tests::additive_catalog_bounds_manifest_sources_and_counts",
    "nexus::runtime_catalog::tests::runtime_root_binds_baselines_descriptors_and_exact_manifest_content",
)),)

DATA_MODEL_STAGES += (("required nullable lifecycle runtime root codecs", (
    "nexus::tests::lane_lifecycle_status_roundtrips_json_and_norito",
    "nexus::tests::lane_lifecycle_status_rejects_empty_runtime_catalog_hash",
    "nexus::tests::lane_lifecycle_status_requires_explicit_unique_nullable_runtime_catalog_hash",
    "nexus::tests::lane_lifecycle_status_rejects_forged_hash_version_and_order",
    "nexus::tests::lane_lifecycle_status_json_rejects_duplicate_unknown_and_missing_fields",
)),)

DATA_MODEL_STAGES += (("exact canonical asset identifier decoders", (
    "asset::id::tests::asset_definition_id_requires_exact_canonical_text_across_decoders",
)),)

DATA_MODEL_STAGES += (("authenticated executed transaction inclusion", (
    "query::canonical_output_inclusion_tests::ordinary_committed_transaction_verifies_against_exact_carrier_block",
    "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_binds_complete_carrier_and_rejects_merge_authority",
    "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_rejects_unbound_wire_and_header_material",
    "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_joins_network_indices_without_time_inputs",
    "query::canonical_output_inclusion_tests::committed_query_rejects_retired_parallel_result_and_merge_wire",
)),)

TEST_NETWORK_STAGES = (("isolated validator fixture configuration", (
    "config::tests::base_config_applies_bounded_storage_caps",
    "config::tests::base_config_preserves_caller_storage_budget_and_smaller_component_cap",
    "tests::peer_client_ignores_ambient_identity_and_endpoint_overrides",
    "tests::profile_account_defaults_materialize_selected_chain_before_root_parse",
    "tests::profile_account_defaults_preserve_explicit_foreign_and_invalid_overrides",
    "tests::peer_clients_preserve_selected_network_profile_after_builder_scope",
    "tests::genesis_preexecution_preserves_selected_profile_across_threads",
    "tests::validated_genesis_cache_reuses_exact_block_and_network_identity",
    "tests::file_backed_genesis_keeps_fresh_preexecution_validation",
)),)

NETWORK_OBSERVATION_STAGES = (("signed genesis paid authority and public failure observation", (
    'dataspace_deploy_cli::signed_genesis_validator_mapping_preserves_runtime_accounts',
    'dataspace_deploy_cli::phase_failure_summary_excludes_signed_payloads',
)), ("inherited native deployment deadline", (
    "dataspace_deploy_cli::remaining_cli_budget_keeps_original_deadline_and_never_rounds_up",
)), ("complete bounded effective permission observation", (
    "runtime_catalog_transition::permission_page_tests::permission_page_requires_complete_short_fanout",
    "runtime_catalog_transition::permission_page_tests::permission_page_rejects_saturation_and_duplicate_items",
    "runtime_catalog_transition::permission_page_tests::permission_page_preserves_failure_context_and_rejects_invalid_metadata",
)), ("bounded validator status observation", (
    "status_observation_tests::status_observation_retries_typed_busy_json_and_norito_with_remaining_budget",
    "status_observation_tests::status_observation_stops_at_original_deadline_during_retry_after",
    "status_observation_tests::status_observation_propagates_auth_other_service_and_decode_failures",
)), ("private production beacon fixture root admission", (
    "production_beacon_bootstrap::production_beacon_fixture_root_rejects_git_symlink_and_shared_custody",
)), ("exact retained-height replay observation", (
    "production_beacon_bootstrap::production_beacon_exact_height_wait_preserves_retained_tip",
)),)
# Every platform qualifies retained authority generations against the same four
# independent genesis-anchored chains and full application workload.
BEACON_NETWORK_TEST = (
    'production_beacon_bootstrap::four_peer_fresh_custody_bootstrap_reaches_mandatory_pulse'
)
BEACON_NETWORK_STAGES = (("fresh beacon custody, retained authority, paid deployment, catalog replay and both route snapshot sequences", (
    BEACON_NETWORK_TEST,
)),)

NETWORK_OBSERVATION_STAGES += (('authenticated epoch retention fixture admission', (
    'production_beacon_bootstrap::epoch_retention::production_epoch_retention_requires_exact_source_identity_before_setup',
    'production_beacon_bootstrap::epoch_retention::production_epoch_retention_binds_exact_generation_beacon_and_interval',
    'production_beacon_bootstrap::epoch_retention::production_epoch_retention_rejects_changed_generation_beacon_parent_and_schedule',
)),)

NETWORK_OBSERVATION_STAGES += (('retained native canary failure evidence', (
    'production_beacon_bootstrap::canary_receipt::failed_canary_receipts_are_retained_before_parse_and_outcome_checks',
    'production_beacon_bootstrap::canary_receipt::retained_canary_receipt_requires_every_binding_and_applied_height',
)),)

# One genuine custody ceremony owns every retained network assertion: paid
# deployment, additive catalog/full replay, and both public routing sequences.
# Independent read/permission/root contracts still run before any peer starts.
BASIC_NETWORK_STAGES = NETWORK_OBSERVATION_STAGES + BEACON_NETWORK_STAGES
NETWORK_STAGES = BASIC_NETWORK_STAGES

# Four peers use the shared test-network 1 GiB/node cap. Keep another 4 GiB
# available for fixture logs, temporary files and concurrent build output.
NETWORK_FIXTURE_FREE_BYTES = 8 * 1024**3

TORII_LIFECYCLE_STAGES = (("lifecycle HTTP representation and access policy", (
    "nexus_lifecycle_endpoint::lifecycle_get_returns_valid_exact_json_status",
    "nexus_lifecycle_endpoint::lifecycle_get_returns_valid_exact_norito_status",
    "nexus_lifecycle_endpoint::lifecycle_get_returns_exact_present_runtime_root_in_both_formats",
    "nexus_lifecycle_endpoint::lifecycle_get_honors_api_token_access_policy",
    "nexus_lifecycle_endpoint::lifecycle_post_and_normalization_variants_are_unregistered_without_mutation",
)),)

STAGES += (('native core scope and durable dataspace deployment', (
    'taira_public_reset::executor_model::tests::qualification_scope_requires_exact_nullable_inrou_closure',
    'taira_public_reset::executor_model::tests::qualification_scope_reopens_exact_durable_execution_boundaries',
    'taira_public_reset::inputs::tests::candidate_runtime_scope_requires_disabled_core_and_exact_full_owner',
    'taira_dataspace_deploy::tests::manifest_binds_native_identity_and_spending_limits',
    'taira_dataspace_deploy::tests::operation_id_is_stable_for_equivalent_intent',
    'taira_dataspace_deploy::tests::catalog_transition_preserves_sparse_baseline_and_cas',
    'taira_dataspace_deploy::tests::retained_phase_rejects_changed_wire_owner_fee_or_instructions',
    'taira_dataspace_deploy::tests::namespace_plan_requires_two_bounded_paid_creates',
    'taira_dataspace_deploy::tests::journal_dispatch_claim_is_durable_and_exclusive',
    'taira_dataspace_deploy::tests::journal_rejects_links_replacement_and_incomplete_records',
    'taira_dataspace_deploy::tests::journal_content_revalidation_preserves_offset_and_rejects_metadata_collisions',
    'taira_dataspace_deploy::tests::status_requires_exact_global_and_peer_state_applied',
    'taira_dataspace_deploy::tests::init_builds_native_restricted_intent_from_policy_and_profile',
    'taira_dataspace_deploy::tests::init_rejects_policy_drift_and_parses_explicit_caps',
    'taira_dataspace_deploy::lane_manifest::tests::generates_typed_manifest_from_executed_signed_genesis',
    'taira_dataspace_deploy::lane_manifest::tests::rejects_genesis_without_explicit_bindings_or_with_malformed_wire',
    'taira_dataspace_deploy::lane_manifest::tests::requires_unique_complete_activated_genesis_bindings',
    'taira_dataspace_deploy::lane_manifest::tests::rejects_changed_binding_and_non_four_trusted_roster',
    'taira_dataspace_deploy::finality::tests::deployment_trust_derives_exact_genesis_roster_and_network',
    'taira_dataspace_deploy::finality::tests::deployment_trust_rejects_wrong_network_key_and_changed_genesis_wire',
    'taira_dataspace_deploy::finality::tests::deployment_trust_requires_four_distinct_genesis_peers_and_public_endpoints',
    'taira_public_reset::host::stopped_runtime::tests::stopped_owner_projection_ignores_unrelated_credentials_and_keeps_typed_defaults',
)), )

CLIENT_STAGES += (('typed authoritative SNS optional reads', (
    'sns::tests::optional_name_http_absence_requires_exact_typed_json',
    'sns::tests::optional_name_http_success_binds_canonical_namespace_and_record',
)), )

TORII_UNIT_STAGES += (('typed SNS absence HTTP contract', (
    'sns::tests::registration_absence_http_response_is_typed_and_other_not_found_is_not',
    'openapi::tests::sns_name_absence_openapi_is_typed_and_selector_bound',
)), )

TORII_SHARED_STAGES = (('strict native SNS missing-registration DTO', (
    'sns::tests::missing_registration_response_requires_exact_fields_and_selector',
)), )

STAGES += (("bounded native deployment and concurrent validator completion", (
    "taira_dataspace_deploy::finality::tests::deployment_peer_reads_overlap_and_preserve_input_order",
    "taira_dataspace_deploy::finality::tests::deployment_peer_reads_reject_non_four_cardinality_before_dispatch",
    "taira_dataspace_deploy::finality::tests::deployment_peer_reads_join_all_workers_and_report_first_error",
    "taira_dataspace_deploy::finality::tests::deployment_peer_reads_inherit_configured_address_profile",
    "taira_dataspace_deploy::finality::tests::deployment_peer_reads_recover_worker_panic_after_joining_all",
    "taira_dataspace_deploy::finality::tests::deployment_carrier_results_require_exact_bytes_before_publication",
    "taira_dataspace_deploy::finality::tests::deployment_peer_progress_requires_valid_complete_status_pair",
    "taira_dataspace_deploy::finality::tests::deployment_peer_progress_retries_only_pending_or_newer_carrier",
    "taira_dataspace_deploy::finality::tests::deployment_peer_progress_never_masks_fixed_worker_errors",
    "taira_dataspace_deploy::tests::saved_commands_require_positive_budget_and_default_to_three_minutes",
    "taira_dataspace_deploy::tests::saved_zero_budget_stops_before_journal_or_client_access",
    "taira_dataspace_deploy::tests::expired_operation_never_observes_or_starts_completion",
    "taira_dataspace_deploy::tests::apply_observes_pending_until_applied_without_reentering_dispatch",
    "taira_dataspace_deploy::tests::status_observes_once_and_terminal_apply_does_not_retry",
    "taira_dataspace_deploy::tests::phase_deadline_rejects_late_applied_and_clips_pending_sleep",
    "taira_dataspace_deploy::tests::completion_retries_only_explicit_sync_progress_and_status_is_one_attempt",
    "taira_dataspace_deploy::tests::completion_deadline_rejects_a_late_success",
)), )

CLIENT_STAGES += (("challenge-bound public finality attestations", (
    'client::evidence_http_tests::bridge_finality_attestation_reader_binds_exact_request_headers_and_signed_body',
    'client::evidence_http_tests::bridge_finality_attestation_reader_rejects_wrong_bindings_and_invalid_http_body',
)), )

STAGES += (("native public deployment profile export", (
    'taira_public_reset::deployment_profile::tests::deployment_profile_binds_native_genesis_and_ordered_inventory_peers',
    'taira_public_reset::deployment_profile::tests::deployment_profile_rejects_genesis_artifact_peer_and_slot_substitution',
    'taira_public_reset::deployment_profile::tests::deployment_profile_command_parses_without_private_or_runtime_arguments',
)), )

CLIENT_STAGES += (("strict scoped pipeline and alias client responses", (
    'client::evidence_http_tests::get_transaction_status_response_global_sets_global_scope',
    'client::evidence_http_tests::pipeline_status_404_returns_none_from_exact_global_query',
    'client::transaction_wait_tests::transaction_wait_timeout_identifies_the_exact_pending_transaction',
    'client::tests::typed_account_alias_reads_map_not_found_to_none',
)), )

TORII_UNIT_STAGES += (("complete typed pipeline and alias HTTP errors", (
    'tests_runtime_handlers::pipeline_status_global_read_skips_non_terminal_local_cache',
    'torii_routed_read_tests::pipeline_status_fanout_requires_exact_scoped_absence',
    'openapi::tests::pipeline_status_openapi_exposes_only_the_exact_first_release_scope',
    'mcp::tests::canonical_paths_and_status::applied_wait_status_poll_accepts_only_exact_200_or_404',
    'tests::alias_error_envelopes_preserve_reports_and_exact_absence_through_middleware',
    'tests::alias_account_absence_requires_complete_scoped_fanout',
    'openapi::tests::alias_errors_openapi_match_native_reports_and_bound_absence',
)), )

TORII_SHARED_STAGES += (("strict scoped read error details", (
    'tests::pipeline_transaction_status_roundtrip_is_status_only',
    'aliases::tests::alias_error_details_roundtrip_and_reject_unknown_fields',
)), )

CLIENT_STAGES += (("one absolute transaction wait deadline and fixed failure evidence", (
    'client::transaction_wait_tests::wait_for_transaction_applied_rejects_fixed_failures',
    'client::transaction_wait_tests::transaction_wait_zero_timeout_never_dispatches_an_initial_read',
    'client::transaction_wait_tests::transaction_wait_unrepresentable_deadline_fails_before_dispatch',
    'client::transaction_wait_tests::transaction_wait_expired_context_deadline_cannot_be_extended',
    'client::transaction_wait_tests::transaction_wait_late_http_status_is_unresolved_in_both_transports',
    'client::transaction_wait_tests::transaction_wait_retries_spend_one_remaining_http_budget',
    'client::transaction_wait_tests::transaction_wait_outcome_admission_rechecks_deadline_after_decoding',
    'client::transaction_wait_tests::transaction_wait_async_deadline_retires_the_pending_status_future',
)), )

STAGES += (("native deployment completion exit contract and exact tip progress", (
    "taira_dataspace_deploy::tests::saved_apply_emits_report_before_rejecting_incomplete_success",
    "taira_dataspace_deploy::tests::saved_report_preserves_output_failure",
    "taira_dataspace_deploy::finality::tests::deployment_attestation_progress_retries_only_exact_sdk_type",
    "taira_dataspace_deploy::finality::tests::deployment_attestation_progress_joins_all_peers_and_preserves_fixed_errors",
)), )

CLIENT_STAGES += (("request-bound finality attestation tip progress", (
    "client::evidence_http_tests::bridge_finality_attestation_reader_preserves_only_bound_typed_tip_progress",
    "client::evidence_http_tests::bridge_finality_attestation_reader_rejects_malformed_or_unbound_tip_progress",
    "client::evidence_http_tests::bridge_finality_attestation_reader_rejects_untyped_or_noncanonical_progress_http",
)), )

TORII_UNIT_STAGES += (("native finality attestation tip progress and public contract", (
    "routing::bridge_finality_attestation_progress_tests::exact_tip_snapshot_races_are_bound_negotiated_progress",
    "routing::bridge_finality_attestation_progress_tests::proof_identity_and_signature_failures_are_never_tip_progress",
    "routing::bridge_finality_attestation_progress_tests::canonical_boundary_keeps_only_valid_tip_progress_status_and_code",
    "routing::bridge_finality_attestation_progress_tests::invalid_height_progress_shapes_remain_fixed_errors",
    "openapi::tests::finality_attestation_tip_progress_openapi_matches_native_bindings",
    "openapi::tests::compact_finality_app_contracts::bridge_finality_operations_describe_durable_v2_evidence",
)), )

TORII_SHARED_STAGES += (("strict finality attestation tip progress details", (
    "bridge_finality::tests::tip_mismatch_requires_exact_selector_and_real_height_progress",
)), )

TORII_STAGES += (("public faucet policy discovery", (
    "accounts_faucet::accounts_faucet_policy_exposes_exact_public_configuration",
    "accounts_faucet::accounts_faucet_policy_resolves_configured_asset_alias",
    "accounts_faucet::accounts_faucet_policy_preserves_disabled_forbidden_response",
)), )

TORII_UNIT_STAGES += (("public faucet policy discovery", (
    "mcp::tests::faucet_policy_tool_is_read_only_and_runtime_gated",
    "mcp::tests::faucet_policy_tool_dispatches_only_get_without_body",
    "openapi::tests::faucet_policy_schema_is_exact_public_discovery",
)), )

TORII_SHARED_STAGES += (("public faucet policy discovery", (
    "route_catalog::tests::account_faucet_policy_is_public_read_only_discovery",
)), )

STAGES += (("retained-network public deployment profile export", (
    'taira_dataspace_deploy::profile::tests::retained_profile_export_uses_native_trust_and_exact_input_hashes',
    'taira_dataspace_deploy::profile::tests::retained_profile_export_rejects_unbound_or_malformed_public_inputs',
    'taira_dataspace_deploy::profile::tests::retained_profile_export_rejects_changed_linked_and_unsafe_files',
    'taira_dataspace_deploy::profile::tests::retained_profile_export_dispatch_rejects_credential_and_transaction_globals',
)), )

CLIENT_STAGES += (("blocking SDK transport ownership between calls", (
    'blocking::tests::borrowed_async_client_reuses_keepalive_connection_between_blocking_calls',
    'blocking::tests::background_tasks_progress_with_a_clone_and_cancel_after_final_owner_drop',
)), )

TORII_UNIT_STAGES = (("released State snapshots and exact canonical outcome authority", (
    'tests_runtime_handlers::canonical_outcome_releases_state_snapshot_before_kura_authentication',
    'tests_runtime_handlers::canonical_outcome_preserves_exact_committed_rejection',
    'tests_runtime_handlers::canonical_outcome_absent_membership_never_authenticates',
    'tests_runtime_handlers::canonical_outcome_accepts_unrelated_state_append_after_authentication',
    'tests_runtime_handlers::canonical_outcome_rejects_removed_membership_after_authentication',
    'tests_runtime_handlers::canonical_outcome_rejects_rebound_membership_after_authentication',
    'tests_runtime_handlers::canonical_outcome_rejects_replaced_journal_after_authentication',
    'tests_runtime_handlers::canonical_outcome_rejects_missing_journal_after_authentication',
    'tests_runtime_handlers::canonical_outcome_rejects_result_substitution_under_the_same_header_hash',
    'tests_runtime_handlers::canonical_outcome_authentication_error_cannot_fall_back_to_terminal_cache',
)), ) + TORII_UNIT_STAGES

HARNESS_TARGETS = {
    "mv": ("native MV ownership", "mv", "lib", ["-p", "mv", "--lib"]),
    "mv-ebr": ("native EBR allocation custody", "ebr_allocation_custody", "test", ["-p", "mv", "--test", "ebr_allocation_custody"]),
    "mv-map": ("native owned map generations", "map_owned_generations", "test", ["-p", "mv", "--test", "map_owned_generations"]),
    "mv-admitted-map": ("native admitted map custody", "admitted_map_custody", "test", ["-p", "mv", "--test", "admitted_map_custody"]),
    "concread": ("native admitted B+ tree ownership", "concread", "lib", ["-p", "concread", "--lib"]),
    "wallet": ("native wallet resource bounds", "iroha_wallet", "lib", ["-p", "iroha_wallet", "--lib"]),
    "daemon": ("native offline genesis qualification", "irohad", "lib", ["-p", "irohad", "--lib"]),
    "config-unit": ("native configuration unit contracts", "iroha_config", "lib", ["-p", "iroha_config", "--lib"]),
    "data-model": ("native canonical catalog parameters", "iroha_data_model", "lib", ["-p", "iroha_data_model", "--lib"]),
    "config": ("native configuration contracts", "taira_config_contracts", "test", ["-p", "iroha_config", "--test", "taira_config_contracts"]),
    "cli": ("native CLI", "iroha", "bin", ["-p", "iroha_cli", "--bin", "iroha"]),
    "kagami": ("native Kagami", "kagami", "bin", ["-p", "iroha_kagami", "--bin", "kagami"]),
    "sorafs-bin": ("native SoraFS shipping target", "sorafs-node", "bin", ["-p", "sorafs_node", "--bin", "sorafs-node"]),
    "taira-launcher": ("native Taira shipping launcher", "iroha3d_taira", "bin", ["-p", "irohad", "--bin", "iroha3d_taira"]),
    "crypto": ("native puzzle cryptography", "iroha_crypto", "lib", ["-p", "iroha_crypto", "--lib"]),
    "p2p": ("native peer transport", "iroha_p2p", "lib", ["-p", "iroha_p2p", "--lib"]),
    "torii": ("native Torii contracts", "taira_app_contracts", "test", ["-p", "iroha_torii", "--test", "taira_app_contracts"]),
    "torii-shared": ("native Torii shared protocol", "iroha_torii_shared", "lib", ["-p", "iroha_torii_shared", "--lib"]),
    "torii-lifecycle": ("native Torii lifecycle endpoint", "torii_nexus_sorafs", "test", ["-p", "iroha_torii", "--test", "torii_nexus_sorafs"]),
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
)), ("native Taira genesis and independent localnet profiles", (
    "genesis::sign::tests::default_genesis_staging_authenticates_catalog_and_reproduces_signed_context",
    "genesis::sign::tests::public_taira_auto_bootstrap_uses_alias_bound_xor_without_config",
    "genesis::sign::tests::private_key_file_round_trips_owner_only_canonical_material",
    "genesis::sign::tests::private_key_file_rejects_unsafe_mode_links_whitespace_and_oversize",
    "localnet::tests::generated_taira_genesis_grants_deployment_only_to_generated_client",
    "localnet::tests::localnet_asset_defaults_are_selected_by_exact_taira_chain_context",
    "localnet::tests::localnet_asset_validation_rejects_selected_builtin_identity_or_alias_collision",
    "localnet::tests::canonical_taira_generation_binds_four_runtime_signers_to_validator_peers",
    "localnet::tests::localnet_runtime_bundle_separates_ledger_and_http_operator_custody",
    "localnet::tests::generated_nexus_localnet_serves_xor_faucet_from_client_signer",
    "localnet::tests::generated_permissioned_localnet_grants_operator_exact_fee_asset_mint_permission",
    "localnet::tests::generated_localnet_bootstraps_universal_kagemusha_asset",
    "localnet::tests::generated_localnet_registers_requested_asset_definition_for_client_owner",
    "localnet::tests::private_dataspace_manifests_use_the_selected_lane_alias",
)),)


KAGAMI_STAGES += (("retired epoch key derivation commands are rejected", (
    'kagemusha::tests::parser_rejects_epoch_key_derivation_commands',
)),)


KAGAMI_STAGES += (("typed public beacon history candidates and explicit proof limits", (
    'kura::beacon_history::tests::beacon_history_projects_only_typed_public_candidates_and_keeps_proof_limits',
    'kura::beacon_history::tests::beacon_history_distinguishes_admission_from_recorded_execution_and_nested_effects',
    'kura::beacon_history::tests::beacon_history_projects_nested_callbacks_once_and_distinguishes_rejected_roots',
    'kura::beacon_history::tests::beacon_history_requires_exact_bounded_range_and_preserves_read_only_journals',
    'kura::beacon_history::tests::beacon_history_rejects_malformed_sidecars_and_preserves_their_source',
    'kura::beacon_history::tests::beacon_history_rejects_block_height_mismatch_without_publishing_partial_json',
    'kura::beacon_history::tests::beacon_history_never_emits_opaque_install_state_or_unrelated_parameter_payloads',
    'kura::beacon_history::tests::beacon_history_cli_exposes_explicit_bounded_scope',
)),)


# Production beacon setup must fail before unrelated tests and network fixtures.
CORE_BEACON_STAGES = (('height-bound beacon readiness and actual custody', (
    'state::tests::autonomous_merge_beacon_composition_preserves_certified_roots_and_commits_once',
    'state::tests::autonomous_merge_beacon_composition_rejects_invalid_effects_and_post_seal_drift',
    'sumeragi::v2_candidate::tests::proposal_work_gate_rejects_beacon_pulse_only',
    'sumeragi::v2_candidate::tests::proposal_work_gate_preserves_non_beacon_effects',
    'sumeragi::v2_candidate::tests::mandatory_beacon_wait_requires_independent_work',
    'sumeragi::v2_candidate::tests::mandatory_beacon_wait_releases_same_queue_prefix_for_retry',
    'beacon::tests::threshold_beacon_deferred_mandatory_height_stays_idle_until_real_work',
    'beacon::tests::threshold_beacon_live_v2_producer_is_bound_restartable_and_persists_effect',
    'beacon::tests::runtime_beacon_capability_requires_exact_live_session_and_seat_without_signing',
    'beacon::readiness::tests::readiness_authenticates_exact_session_once_without_signing_on_http_checks',
    'beacon::readiness::tests::readiness_reuses_only_exact_authenticated_transcripts_across_heights',
    'beacon::readiness::tests::readiness_requires_pending_session_to_cover_the_mandatory_pulse',
    'beacon::readiness::tests::readiness_requires_both_current_parliament_and_future_npos_pulses',
    'beacon::readiness::tests::readiness_rejects_missing_foreign_and_corrupt_public_sessions',
    'beacon::readiness::tests::readiness_rejects_absent_unavailable_and_wrong_seat_providers',
    'beacon::readiness::tests::readiness_invalidates_old_height_roster_and_publication_owner',
    'beacon::readiness::tests::readiness_does_not_require_local_custody_for_observers_or_unused_permissioned_beacons',
    'sumeragi::emergency_fast_handle_tests::missing_beacon_readiness_preserves_bootstrap_ingress',
)), )
DAEMON_BEACON_STAGES = (('native beacon bootstrap, broker and consumed credential custody', (
    'taira_runtime_signer::tests::production_beacon_fixture_guard_keeps_exact_core_only_taira_identity',
    'runtime_provider_broker::protocol::platform::tests::global_beacon_capability_attestation_round_trips_over_authenticated_broker',
    'runtime_provider_broker::protocol::platform::tests::global_beacon_capability_typed_proxy_requalifies_before_and_after_lookup',
    'runtime_provider_broker::protocol::platform::tests::correlated_wrong_beacon_session_id_is_rejected_by_typed_proxy',
    'runtime_provider_broker::protocol::platform::tests::correlated_wrong_beacon_transcript_hash_is_rejected_by_typed_proxy',
    'runtime_provider_broker::protocol::platform::tests::correlated_wrong_beacon_signer_index_is_rejected_by_typed_proxy',
    'runtime_provider_broker::protocol::platform::tests::correlated_truncated_beacon_capability_is_rejected_by_typed_proxy',
    'runtime_provider_broker::protocol::platform::tests::global_beacon_capability_request_rejects_foreign_network_transcript_and_invalid_seat',
    'runtime_provider_broker::protocol::platform::tests::global_beacon_capability_server_rejects_a_qualified_backend_claiming_the_wrong_seat',
    'runtime_provider_broker::protocol::primitives::operation_ordinal_tests::post_soracloud_operation_ids_are_exact_and_contiguous',
    'taira_runtime_signer::tests::beacon_loader_consumes_exact_credential_and_verifies_native_signature',
    'taira_runtime_signer::tests::beacon_loader_rejects_wrong_network_qualification_and_corruption',
    'taira_runtime_signer::tests::beacon_loader_rejects_untrusted_descriptor_and_size',
    'taira_runtime_signer::tests::registry_allows_bootstrap_without_beacon_and_rejects_extra_or_duplicate_slots',
    'taira_runtime_signer::tests::registry_resolves_exact_configured_beacon_and_preserves_soracloud_binding',
    'taira_runtime_signer::tests::offline_introspection_never_requires_the_runtime_signer',
    'taira_runtime_signer::tests::mint_seed_loader_consumes_exact_private_record_and_preserves_restart_source',
    'taira_runtime_signer::tests::descriptor_loader_accepts_only_canonical_owner_only_ed25519',
    'beacon_bootstrap::tests::fresh_four_seat_bootstrap_roundtrips_native_custody_and_lifecycle_quorum',
    'beacon_bootstrap::tests::bootstrap_rejects_foreign_genesis_rosters_transcripts_and_lifecycle_substitution',
    'beacon_bootstrap::tests::bootstrap_phase_eof_and_deadline_abort_without_fabricated_height',
    'beacon_bootstrap::tests::bootstrap_output_custody_is_exclusive_and_lifecycle_key_is_consumed',
    'beacon_bootstrap::tests::bootstrap_config_descriptor_uses_only_exact_native_consensus_identity',
    'beacon_bootstrap::tests::bootstrap_records_observed_height_jumps_and_rejects_pulse_collision',
)), )
TORII_BEACON_STAGES = (('production beacon readiness leaves setup ingress open', (
    'tests_runtime_handlers::readiness_rejects_uninitialized_beacon_without_closing_bootstrap_ingress',
)), )
STAGES += (("exact-height native lifecycle installation", (
    'tests::fee_quote_signing_preserves_explicit_ordinary_payload_and_expiry',
    'taira_public_reset::host::beacon::tests::beacon_install_envelope_requires_ordinary_exact_certificate',
    'taira_public_reset::public_inputs::tests::beacon_bootstrap_window_reserves_real_queue_plan_canary_and_install',
)), )
TORII_BEACON_STAGES += (("authenticated exact-roster Ordinary lifecycle ingress", (
    'tests_runtime_handlers::lifecycle_ordinary_ingress_accepts_exact_quorum_and_preserves_wire_identity',
    'tests_runtime_handlers::lifecycle_ordinary_ingress_rejects_general_and_mixed_transactions',
    'tests_runtime_handlers::lifecycle_ordinary_ingress_rejects_invalid_certificate_authority',
    'tests_runtime_handlers::lifecycle_ordinary_ingress_requires_authenticated_parent_and_global_route',
)), )
CORE_STARTUP_STAGES = CORE_BEACON_STAGES + CORE_STARTUP_STAGES
CORE_ADMISSION_STARTUP_STAGES = CORE_BEACON_STAGES + CORE_ADMISSION_STARTUP_STAGES
CORE_STAGES = CORE_BEACON_STAGES + CORE_STAGES
DAEMON_STARTUP_STAGES = DAEMON_BEACON_STAGES + DAEMON_STARTUP_STAGES
DAEMON_STAGES = DAEMON_BEACON_STAGES + DAEMON_STAGES
TORII_STARTUP_STAGES = TORII_BEACON_STAGES + TORII_STARTUP_STAGES
TORII_UNIT_STAGES = TORII_BEACON_STAGES + TORII_UNIT_STAGES


CLIENT_STAGES += (("bounded read-only finality backpressure and preserved verifier state", (
    'client::evidence_http_tests::bridge_finality_reader_retries_only_backpressure_within_original_deadline',
    'client::evidence_http_tests::bridge_finality_reader_rejects_unbounded_or_invalid_backpressure_without_advancing',
    'client::evidence_http_tests::activation_evidence_backpressure_preserves_challenge_and_response_bounds',
    'client::evidence_http_tests::bridge_finality_reader_expired_deadline_does_not_dispatch_or_advance',
    'client::evidence_http_tests::bridge_finality_next_reader_rejects_height_mismatch_before_advancing',
    'client::evidence_http_tests::bridge_finality_next_reader_response_contract_failures_do_not_advance',
    'client::evidence_http_tests::bridge_finality_next_reader_verification_failure_does_not_advance',
    'client::transaction_wait_tests::transaction_wait_backpressure_retries_only_reads_and_requires_state_applied',
    'client::transaction_wait_tests::transaction_wait_backpressure_honors_retry_after_without_extending_deadline',
    'client::transaction_wait_tests::transaction_wait_backpressure_without_retry_after_uses_poll_interval',
    'client::transaction_wait_tests::transaction_wait_backpressure_is_still_an_error_for_one_shot_reads',
    'client::transaction_wait_tests::transaction_wait_backpressure_does_not_retry_malformed_instructions_or_other_errors',
    'client::transaction_wait_tests::transaction_wait_backpressure_preserves_fixed_failure_and_hash_binding',
)), )
STAGES += (("genesis-rooted four-validator committed height observation", (
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_requires_prepared_genesis_roster_and_exact_peer_selection',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_verifies_contiguous_chain_and_fresh_four_peer_evidence',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_rejects_skips_signatures_and_changed_identity',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_progress_never_emits_a_mixed_or_racing_checkpoint',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_deadline_prevents_dispatch_and_late_completion',
)), )

TEST_NETWORK_STAGES += (("prebuilt binary portability and authenticated Taira bootstrap", (
    "tests::release_prebuilt_taira_launcher_is_mandatory_and_separately_bound",
    "tests::program_absolute_prebuilt_override_does_not_require_checkout",
    "tests::program_discovery_requires_checkout_with_context",
    "tests::program_absolute_prebuilt_override_rejects_partial_release_identity",
    "tests::program_absolute_prebuilt_override_requires_active_release_checkout",
)), )


class CheckError(Exception):
    """A build or selected regression did not pass."""


class SelectedRegressionFailures(CheckError):
    """Completed selected tests failed; other isolated fixtures can still run."""

    def __init__(self, failures: list[str]):
        self.failures = tuple(failures)
        super().__init__(f"{len(self.failures)} selected regressions failed: "
                         + "; ".join(self.failures))


def native_package_root(root: Path, package: str) -> Path:
    """Bind each maintained native package to its one captured source owner."""
    if package not in {target[3][1] for target in HARNESS_TARGETS.values()}:
        raise CheckError("native package lacks a maintained source owner")
    return root / ("vendor" if package == "concread" else "crates") / package


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
            package_root = native_package_root(root, package)
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


STAGES += (("native beacon reset authority, bounded recovery and public input assembly", (
    'taira_public_reset::host::beacon::tests::signed_beacon_plan_binds_roster_seats_and_exact_final_units',
    'taira_public_reset::host::beacon::tests::beacon_config_projection_changes_only_exact_provider_fields',
    'taira_public_reset::host::beacon::tests::lost_beacon_ceremony_cannot_restart_or_repeat_committed_canaries',
    'taira_public_reset::host::beacon::tests::beacon_owned_child_deadline_retains_private_attempt',
    'taira_public_reset::host::tests::beacon_activation_barrier_preserves_pre_ready_bootstrap_and_blocks_later_mutations',
    'taira_public_reset::executor_model::tests::beacon_submitted_continuation_retains_exact_host_cursor_and_excludes_ledger_work',
    'taira_public_reset::executor_model::tests::beacon_continuation_outcome_cannot_reclassify_submitted_ledger_transaction',
    'taira_public_reset::host::beacon::tests::beacon_successful_early_child_exit_cannot_authorize_another_operation',
    'taira_public_reset::host::beacon::tests::beacon_unit_publication_preserves_completed_inode_and_rejects_substitution',
    'taira_public_reset::inputs::tests::topology_intent_forbids_generated_pins_and_plans',
    'taira_public_reset::public_inputs::tests::beacon_public_preparation_derives_native_nonce_bound_seats_and_rejects_substitution',
    'taira_public_reset::public_inputs::tests::public_bundle_requires_authenticated_raw_manifest_without_four_file_fallback',
    'taira_public_reset::public_inputs::tests::public_bundle_derives_canary_from_topology_intent_without_key_file',
    'taira_public_reset::host::tests::recovery_intent_exposes_every_ordered_child_mutation',
    'taira_public_reset::inputs::tests::assembler_rejects_incomplete_topology_before_reading_runtime_inputs',
)), )

STAGES += (('authenticated current height', (
    'taira::tests::retired_epoch_maintenance_commands_are_rejected',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_repeat_current_preserves_freshness_and_advancing_contract',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_restart_transport_never_masks_fixed_peer_identity',
)),)







KAGAMI_STAGES += (('native beacon history physical input and execution root distinction', (
    'kura::beacon_history::tests::beacon_history_separates_external_and_time_execution_roots_without_weakening_results',
)), )




STAGES += (("shared deployment exclusion and durable reset ownership", (
    'taira_public_reset::host::deployment_lifecycle::tests::deployment_owner_rejects_foreign_missing_and_expired_recovery',
    'taira_public_reset::host::deployment_lifecycle::tests::deployment_owner_cannot_restart_from_existing_host_progress',
    'taira_public_reset::host::deployment_lifecycle::tests::deployment_terminal_replay_requires_exact_action_and_retained_intent',
    'taira_public_reset::host::deployment_lifecycle::tests::deployment_owner_waits_for_all_local_seals_and_rollbacks',
    'taira_public_reset::host::deployment_lifecycle::tests::deployment_lock_excludes_other_descriptors_and_rejects_rebinding',
)),)

STAGES += (('native reset schema and ordered service barriers', (
    'taira_public_reset::executor_model::tests::reset_execution_preserves_beacon_and_service_order_without_epoch_writer',
    'taira_public_reset::executor_model::tests::retired_inventory_fields_and_seven_artifact_closure_are_rejected',
    'taira_public_reset::executor_model::tests::retired_epoch_supervisor_commands_and_inputs_are_rejected',
    'taira_public_reset::inputs::tests::authorization_rejects_retired_supervisor_fields',
    'taira_public_reset::host::tests::host_frontier_preserves_four_beacon_activations_before_restart',
)), )






STAGES += (("native public reset input producer closure", (
    'taira_public_reset::inputs::tests::topology_context_checks_scope_and_budget_before_custody',
    'taira_public_reset::inputs::tests::native_context_rejects_scope_before_opening_actual_inputs',
    'taira_public_reset::deployment_profile::tests::deployment_profile_public_context_precedes_artifact_closure_without_weakening_export',
    'taira_public_reset::deployment_profile::tests::deployment_profile_public_context_rejects_truncated_or_extra_slot_vectors',
    'taira_public_reset::inputs::context_release::tests::reset_context_artifact_derives_real_bytes_and_retains_drift_custody',
    'taira_public_reset::inputs::context_release::tests::reset_context_artifact_rejects_wrong_mode_and_symlink_before_projection',
)), )


TORII_SHARED_STAGES += (('exact public status failure reason codes', (
    'status::failure::tests::status_failure_codes_are_exact_and_distinct',
    'status::failure::tests::status_failure_codes_do_not_accept_unclassified_input_as_a_reason',
)), )


TORII_UNIT_STAGES += (('typed status producer HTTP failure projection', (
    'routing::status_failure_reason_tests::snapshot_failure_reasons_match_json_norito_and_header',
)), )


CLIENT_STAGES += (('typed status failure SDK decoding without implicit retries', (
    'client::status_http_tests::status_unavailable_reasons_are_safe_in_errors_and_do_not_trigger_retries',
    'client::status_http_tests::status_unavailable_rejects_missing_unknown_invalid_and_duplicate_reason_headers',
)), )


QUALIFICATION_SCOPES = ("basic", "full")


STAGES += (('independently verified finality certificate witnesses', (
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_accepts_independent_certificate_witnesses',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_rejects_invalid_current_and_parent_witnesses',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_rejects_signed_conflicting_decisions',
    'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_requires_authenticated_predecessor_for_alternate_witnesses',
)), )

STAGES += (('invocation-owned authenticated finality prefix', (
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_batches_and_pending_retries_authenticate_each_height_once',
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_rejects_changed_or_deleted_authenticated_disk_proof',
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_fresh_owner_reauthenticates_corrupted_disk_prefix',
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_invalid_successor_does_not_advance_retained_verifier',
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_publication_or_deadline_failure_does_not_commit_trial',
    'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_lower_tip_keeps_frontier_and_rejects_conflicting_decision',
)), )

KAGAMI_STAGES += (('read-only bounded native finality inspection', (
    'kura::tests::finality_inspection_rejects_invalid_height_before_store_access',
    'kura::tests::finality_inspection_failure_preserves_output_and_store',
    'kura::tests::finality_command_rejects_output_inside_store',
)), )

CORE_FINALITY_INSPECTION_STAGES = (('read-only retained finality native validation', (
    'kura::tests::block_store_read_only_finality_verifies_without_mutation',
    'kura::tests::block_store_read_only_finality_rejects_invalid_signature_and_binding',
    'kura::tests::block_store_read_only_finality_rejects_noncanonical_and_missing_records',
    'kura::tests::block_store_read_only_finality_rejects_unpublished_journal_boundary',
)), )
CORE_STAGES += CORE_FINALITY_INSPECTION_STAGES
CORE_STARTUP_STAGES += CORE_FINALITY_INSPECTION_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_FINALITY_INSPECTION_STAGES

CORE_EXECUTION_PUBLICATION_STAGES = (("actual execution fixture finality and publication ownership", (
    'state::execution_publication_test_support::tests::executed_genesis_and_successor_publish_real_finality_and_witnesses',
    'state::execution_publication_test_support::tests::publication_rejects_an_overlay_from_another_state_before_durable_writes',
    'state::execution_publication_test_support::tests::publication_rejects_changed_sealed_wire_with_the_same_header',
    'state::execution_publication_test_support::tests::publication_requires_the_original_captured_witness',
    'state::execution_publication_test_support::tests::publication_refuses_other_signed_genesis_validator_keys',
)),)
CORE_STAGES += CORE_EXECUTION_PUBLICATION_STAGES
CORE_STARTUP_STAGES += CORE_EXECUTION_PUBLICATION_STAGES

CORE_WORLD_ACQUISITION_STAGES = (("original World and trigger aggregate acquisition and abandonment", (
    'state::tests::world_complete_drop_tests::ordinary_world_drop_unlocks_peers_before_parameters_notification',
    'state::tests::world_complete_drop_tests::replacement_world_drop_unlocks_peers_before_parameters_notification',
    'state::tests::world_complete_drop_tests::ordinary_world_explicit_retirement_defers_original_notifications',
    'state::tests::world_complete_drop_tests::replacement_world_explicit_retirement_defers_original_notifications',
    'state::tests::world_complete_drop_tests::world_block_owner_preserves_canonical_json_and_checked_writer',
    'smartcontracts::isi::triggers::set::detachment::tests::acquisition_tests::ordinary_trigger_drop_unlocks_active_index_before_ids_notification',
    'smartcontracts::isi::triggers::set::detachment::tests::acquisition_tests::replacement_trigger_drop_unlocks_active_index_before_ids_notification',
)), )
CORE_STAGES += CORE_WORLD_ACQUISITION_STAGES
CORE_STARTUP_STAGES += CORE_WORLD_ACQUISITION_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_WORLD_ACQUISITION_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_EXECUTION_PUBLICATION_STAGES


CORE_WORLD_CAPTURE_STAGES = (("original World and trigger capture custody", (
    'state::tests::world_capture_tests::ordinary_world_capture_unlocks_peers_before_parameters_notification',
    'state::tests::world_capture_tests::replacement_world_capture_unlocks_peers_before_parameters_notification',
    'state::tests::world_capture_tests::refused_world_capture_releases_all_writers_before_original_notifications',
    'state::tests::world_capture_tests::panicked_world_capture_releases_all_writers_and_preserves_native_poison',
    'smartcontracts::isi::triggers::set::detachment::tests::capture_tests::ordinary_trigger_capture_unlocks_active_index_before_ids_notification',
    'smartcontracts::isi::triggers::set::detachment::tests::capture_tests::replacement_trigger_capture_unlocks_active_index_before_ids_notification',
    'smartcontracts::isi::triggers::set::detachment::tests::capture_tests::ordinary_nested_world_capture_unlocks_later_cell_before_trigger_ids_notification',
    'smartcontracts::isi::triggers::set::detachment::tests::capture_tests::replacement_nested_world_capture_unlocks_later_cell_before_trigger_ids_notification',
)), )
CORE_STAGES += CORE_WORLD_CAPTURE_STAGES
CORE_STARTUP_STAGES += CORE_WORLD_CAPTURE_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_WORLD_CAPTURE_STAGES


CORE_STATE_CAPTURE_STAGES = (("original State, runtime and membership capture custody", (
    'state::carrier_preparation::journals::tests::state_capture_tests::carrier_capture_unlocks_state_topology_before_world_parameters_notification',
    'state::carrier_preparation::journals::tests::state_capture_tests::carrier_capture_refused_original_drop_releases_membership_before_world_notification',
    'state::carrier_preparation::journals::tests::state_capture_tests::carrier_capture_admission_panic_releases_healthy_membership_before_world_notification',
    'state::carrier_preparation::journals::tests::state_capture_tests::state_capture_late_membership_refusal_retains_completed_world_and_runtime_until_joint_drop',
    'state::carrier_preparation::journals::runtime_journals::publication_tests::capture_tests::ordinary_runtime_capture_unlocks_contexts_before_runtime_notification',
    'state::carrier_preparation::journals::runtime_journals::publication_tests::capture_tests::replacement_runtime_capture_unlocks_contexts_before_runtime_notification',
    'state::storage_transactions::block::capture_tests::membership_capture_retains_original_ordinary_and_replacement_journals_and_releases',
    'state::storage_transactions::block::capture_tests::membership_capture_real_refusal_keeps_original_writer_until_joint_release',
    'state::storage_transactions::block::capture_tests::membership_capture_outer_unwind_releases_prepared_or_captured_and_attached_sibling',
    'state::storage_transactions::block::capture_tests::membership_terminal_release_rejects_read_mutation_preparation_and_publication',
    'state::storage_transactions::block::capture_tests::membership_capture_wake_panic_keeps_other_original_release_healthy',
)), )
CORE_STAGES += CORE_STATE_CAPTURE_STAGES
CORE_STARTUP_STAGES += CORE_STATE_CAPTURE_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_STATE_CAPTURE_STAGES


CORE_STATE_ACQUISITION_STAGES = (("original State acquisition and executing-block retirement", (
    'state::carrier_preparation::tests::state_acquisition_drop_tests::pristine_stage_refusal_releases_membership_before_world_notification',
    'state::carrier_preparation::tests::state_acquisition_drop_tests::complete_state_block_drop_releases_membership_before_world_notification',
    'state::carrier_preparation::tests::state_acquisition_drop_tests::pristine_stage_panic_releases_healthy_membership_before_world_notification',
    'state::carrier_preparation::tests::state_acquisition_drop_tests::acquired_runtime_result_drop_releases_membership_before_world_notification',
)), )
CORE_STAGES += CORE_STATE_ACQUISITION_STAGES
CORE_STARTUP_STAGES += CORE_STATE_ACQUISITION_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_STATE_ACQUISITION_STAGES


# Portable ownership prerequisites; every selected leaf runs in both scopes.
MV_OWNERSHIP_HARNESSES = ("mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread")

MV_OWNERSHIP_STAGES = (
    ("caller-owned capture and original notification custody", (
        'capture_tests::capture_slots_keep_exact_ordinary_and_replacement_journals_until_all_writers_release',
        'capture_tests::capture_slots_keep_successful_sibling_through_admission_refusal_and_caught_panic',
        'capture_tests::capture_slots_failed_map_precheck_keeps_original_block_for_joint_abandonment',
        'capture_tests::capture_slots_outer_unwind_preserves_actual_attached_writer_poison_only',
        'capture_tests::capture_slots_admission_cleanup_panic_happens_after_all_physical_unlocks',
    )),
    ("caller-owned aggregate acquisition and terminal retirement", (
        'cell::aggregate_acquisition_tests::caller_owned_cell_slots_release_all_before_later_clone_unwind_cleanup',
        'cell::aggregate_acquisition_tests::caller_owned_cell_slots_retain_known_poison_until_earlier_slot_unlocks',
        'cell::aggregate_acquisition_tests::completed_cell_slots_transfer_without_wake_and_release_without_retirement',
        'storage::aggregate_acquisition_tests::caller_owned_storage_slots_retain_replacement_prefix_until_all_writers_release',
        'storage::aggregate_acquisition_tests::completed_storage_slots_release_physical_writers_before_retirement_and_refuse_reuse',
    )),
    ('original Cell pair acquisition and abandonment', (
        'cell::fresh_pair_acquisition_tests::cell_second_clone_panic_releases_both_before_native_notifications',
        'cell::fresh_pair_acquisition_tests::cell_second_clone_panic_reclaims_completed_undo_only_after_pair_unlock',
        'cell::fresh_pair_acquisition_tests::cell_known_undo_poison_rejects_before_waiting_for_current',
        'cell::fresh_pair_acquisition_tests::cell_first_clone_panic_releases_pair_before_unused_current_charge',
        'cell::fresh_pair_acquisition_tests::cell_known_current_poison_precedes_both_clones_and_charge_cleanup',
        'cell::fresh_pair_acquisition_tests::cell_successful_pair_acquisition_keeps_clones_locked_and_notifications_pending',
        'cell::fresh_pair_acquisition_tests::cell_complete_block_and_current_replacement_abandonment_unlocks_before_cleanup',
        'cell::fresh_pair_acquisition_tests::cell_explicit_detach_keeps_original_generations_and_defers_both_notifications',
        'cell::fresh_pair_acquisition_tests::cell_publication_poison_preserves_pair_through_commit_refusal_cleanup',
    )),
    ('funded publication identity release', (
        'publication::nonblocking_tests::funded_identity_refund_observes_unlocked_publication_even_on_release_unwind',
    )),
    ('finite resident allocation pool', (
        'allocation::tests::charge_keeps_original_pool_alive_after_budget_handle_is_dropped',
        'allocation::tests::concurrent_reservations_cannot_oversubscribe_the_same_finite_pool',
        'allocation::tests::exact_pool_release_wakes_waiters_including_before_their_first_poll',
        'allocation::tests::finite_limit_overflow_and_zero_never_change_credit_on_refusal',
        'allocation::tests::real_epoch_reclamation_returns_capacity_and_its_release_notification',
        'allocation::tests::splitting_prepaid_credits_refunds_only_unused_remainder_and_owned_charges',
        'allocation::tests::partition_retains_exact_original_pool_and_conserves_real_credits',
    )),
    ('actual writer release observations', (
        'release_tests::a_nonpoisoning_guard_unwind_does_not_poison_later_contention',
        'release_tests::inner_guard_destructor_panic_still_signals_after_its_physical_lock_releases',
        'release_tests::acquisition_unwind_notifies_after_raw_lock_release_without_a_published_guard',
        'release_tests::cell_abort_detach_and_publication_release_the_actual_busy_writer',
        'release_tests::cell_prepared_and_storage_original_guards_notify_every_release_path',
        'release_tests::partial_writer_acquisition_does_not_wake_its_own_refused_lock',
        'release_tests::storage_revert_preimage_clone_panic_wakes_an_already_registered_retry',
        'release_tests::storage_prepared_drop_abort_and_publish_release_the_original_writers',
    )),
    ('charged current and undo Cell ownership', (
        'cell::charged_allocation_tests::detached_abort_keeps_original_journal_and_publish_never_returns_generation_charges',
        'cell::charged_allocation_tests::first_undo_clone_panic_wakes_existing_busy_waiter_and_retains_original_successors',
        'cell::charged_allocation_tests::refusal_and_writer_contention_return_original_charged_journal_without_extra_clones',
        'cell::charged_allocation_tests::repeated_block_and_transaction_mutation_capture_each_preimage_only_once',
        'cell::charged_allocation_tests::same_cut_abort_refunds_writers_and_publish_retains_current_with_original_undo',
        'cell::charged_allocation_tests::startup_ordinary_and_revert_charges_follow_all_actual_generations',
        'cell::charged_allocation_tests::untouched_detached_publication_releases_only_unused_current_charge',
    )),
    ('fresh Storage pair construction retains actual acquisition', (
        'storage::admitted_tests::fresh_pair_acquisition::admitted_second_policy_refusal_releases_both_before_callbacks',
        'storage::admitted_tests::fresh_pair_acquisition::admitted_second_policy_panic_releases_both_before_callbacks',
        'storage::admitted_tests::fresh_pair_acquisition::ordinary_current_poison_releases_both_before_callbacks',
        'storage::admitted_tests::fresh_pair_acquisition::current_busy_releases_only_acquired_undo',
        'storage::admitted_tests::fresh_pair_acquisition::successful_pair_construction_emits_no_early_release',
        'storage::admitted_tests::fresh_pair_acquisition::admitted_refusal_wake_panic_preserves_healthy_pair_and_surviving_waiters',
        'storage::admitted_tests::fresh_pair_acquisition::admitted_undo_poison_precedes_busy_current_without_policy',
        'storage::admitted_tests::fresh_pair_acquisition::ordinary_undo_poison_does_not_wait_for_current',
    )),
    ('scoped admitted Storage acquisition retains caller custody', (
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::admitted_scoped_acquisition_rejects_foreign_scope_before_locks_or_policy',
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::admitted_scoped_start_and_reset_refusals_retain_original_owners_for_release',
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::admitted_scoped_second_provider_failure_defers_all_aggregate_native_wakes',
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::admitted_scoped_busy_current_retains_undo_until_caller_release',
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::scoped_original_block_capture_releases_scope_and_keeps_replacement_custody',
        'storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::ordinary_acquisition_retains_zero_sized_scope_and_original_protocol',
    )),
    ('original Storage successor publication', (
        'storage::publication_tests::acquired_map_refusal_never_fabricates_foreign_or_busy_release',
        'storage::publication_tests::stale_map_pair_refusal_defers_actual_releases_through_enclosing_fence',
        'storage::admitted_tests::admitted_block_abandonment_unlocks_both_writers_before_native_wakes',
        'storage::publication_tests::abort_keeps_original_owner_available_after_another_component_refuses',
        'storage::publication_tests::busy_writers_return_same_journal_and_release_partial_acquisition',
        'storage::publication_tests::changed_raw_map_generation_refuses_original_owner_before_any_installation',
        'storage::publication_tests::executing_block_abandonment_releases_both_writers_and_preserves_actual_poison',
        'storage::publication_tests::foreign_aba_and_admission_race_cannot_publish_a_stale_journal',
        'storage::publication_tests::installation_retains_original_successors_and_both_reservations_survive_publication',
        'storage::publication_tests::original_map_and_undo_survive_both_busy_writers_abort_and_publication_without_clones',
        'storage::publication_tests::preparation_retains_readers_and_identity_and_published_cleanup_spans_the_aggregate',
        'storage::publication_tests::prepared_delta_matches_direct_commit_and_preserves_existing_readers',
        'storage::publication_tests::prepared_pair_abandonment_releases_both_writers_and_preserves_actual_poison',
        'storage::publication_tests::replacement_opening_panic_releases_both_original_writers_before_notifying',
        'storage::publication_tests::replacement_restores_discarded_tip_only_keys_and_candidate_undo',
        'storage::publication_tests::untouched_noop_and_absent_touches_publish_exact_undo_transitions',
        'cell::publication_tests::prepared_cell_identity_and_cleanup_remain_owned_through_aggregate_unlock',
    )),
    ('move-only Storage journal detachment', (
        'storage::detached_tests::aborted_children_and_noop_touches_survive_detachment_without_invented_entries',
        'storage::detached_tests::capture_and_abort_release_both_writers_before_native_wake_even_on_unwind',
        'storage::detached_tests::detached_values_outlive_the_storage_without_a_reader_pin',
        'storage::detached_tests::detachment_retains_original_values_without_clones_and_releases_reservation_last',
        'storage::detached_tests::direct_insert_and_reverted_predecessor_cannot_reuse_original_identity',
        'storage::detached_tests::disjoint_candidates_are_owned_send_journals_and_release_all_writers',
        'storage::detached_tests::ordinary_capture_retains_applied_noop_and_absence_touches_without_publication',
        'storage::detached_tests::replacement_mode_retains_discarded_tip_only_changes_and_real_undo',
        'storage::detached_tests::snapshot_json_and_history_projection_create_new_owners_with_exact_images',
        'storage::detached_tests::unchanged_replacement_and_undo_only_commit_have_distinct_pair_identity',
    )),
    ('funded replacement restores original current and undo owners', (
        'storage::admitted_tests::admitted_replacement_retains_mode_and_restored_preimages_through_callback_abort',
        'storage::admitted_tests::admitted_replacement_contention_names_only_the_original_held_writer',
        'storage::admitted_tests::admitted_replacement_of_empty_undo_still_records_replace_mode',
        'storage::admitted_tests::admitted_replacement_final_undo_clear_refusal_preserves_original_pair',
        'storage::admitted_tests::admitted_replacement_callback_cleanup_cannot_publish_a_partial_owner',
        'storage::admitted_tests::admitted_replacement_second_plan_refusal_returns_a_healthy_original_pair',
        'storage::admitted_tests::admitted_replacement_planning_refusal_discards_the_restored_private_prefix',
    )),
    ('funded snapshot restoration retains exact history and source custody', (
        'storage::admitted_tests::admitted_snapshot_preserves_exact_current_undo_and_replacement_history',
        'storage::admitted_tests::admitted_snapshot_capacity_and_planning_refusals_leave_source_reusable',
    )),
    ('finite transaction touch-key custody', (
        'storage::touches::tests::touch_sorted_unique_growth_moves_original_key_allocations_without_copying',
        'storage::touches::tests::touch_exact_joined_capacity_and_one_byte_below_preserve_original_state',
        'storage::touches::tests::touch_preparation_abandonment_and_copy_panic_leave_old_array_and_keys_exact',
        'storage::touches::tests::touch_arbitrary_key_drop_panic_drains_remaining_prefix_and_refunds_real_owners',
        'storage::touches::tests::touch_refused_payload_and_checked_growth_overflow_allocate_nothing',
        'storage::touches::tests::touch_plan_extends_original_demand_and_preserves_exact_provider_remainder',
    )),

)

MV_EBR_STAGES = (("actual epoch allocation and retained capacity custody", (
    'admission_refusal_and_contention_never_clone_and_abort_frees_before_charge',
    'committed_allocation_and_charge_wait_for_unrelated_epoch_pin',
    'clone_panic_conservatively_retains_admitted_charge',
    'destructor_panic_conservatively_retains_charge_even_if_outer_allocation_frees',
    'detached_generation_retries_with_original_allocation_and_no_installation_clone',
)),)

MV_MAP_STAGES = (("original owned map successors across refusal and publication", (
    'storage_reads_allocate_nothing_across_retained_views_edits_and_rollback',
    'storage_history_and_borrowed_string_ranges_allocate_nothing',
    'original_payloads_survive_detach_busy_retry_abort_and_publication_without_clones',
    'foreign_stale_and_equal_content_aba_refusals_return_the_exact_original_owner',
    'detached_owner_keeps_shared_nodes_after_source_drop_and_cross_thread_transfer',
    'old_reader_chain_retains_removed_payloads_across_splits_abort_and_later_commits',
    'final_map_destruction_allocates_nothing_and_frees_every_original_layout',
    'retained_reader_chain_and_final_tree_reclamation_do_not_allocate',
    'final_detached_owner_reclaims_unpublished_nodes_and_retained_root_without_allocation',
    'stale_detached_owner_reclaims_its_old_base_and_newer_committed_root_without_allocation',
    'sibling_candidate_cannot_adopt_after_another_commit_but_retains_its_shared_base',
    'poisoned_writer_refuses_adoption_without_consuming_the_original_generation',
    'clear_successor_preserves_old_reader_until_its_exact_payloads_are_released',
    'scalar_detach_contention_retry_abort_and_commit_allocate_no_new_successor',
    'fresh_map_first_commit_without_a_reader_allocates_no_new_successor',
    'storage_transaction_abort_restores_both_parent_trees_without_allocating_or_cloning',
    'storage_transaction_apply_keeps_original_current_and_undo_payloads_without_allocating',
    'caught_transaction_preimage_clone_panic_cannot_apply_partial_touches',
    'caught_remove_query_destructor_panic_cannot_apply_partial_transaction',
    'caught_block_insert_preimage_panic_cannot_publish_an_unrevertible_mutation',
    'caught_block_remove_preimage_panic_cannot_publish_an_unrevertible_mutation',
    'caught_block_mutable_preimage_panic_cannot_reuse_or_publish_the_owner',
    'caught_block_query_destructor_panic_cannot_commit_or_detach',
    'caught_child_undo_cursor_panic_cannot_publish_the_healthy_current_tree',
)),)


# Native cutover owners run before process/network qualification. These checks
# retain exact sources and resource obligations; they do not open live ingress.
CORE_NATIVE_CONNECTION_STAGES = (
    ('native producer assembly retains exact decisions and independent work', (
        'state::tests::native_candidate_uses_exact_decisions_and_canonical_recorded_execution',
        'state::tests::native_candidate_fits_whole_priority_prefix_before_signing',
        'state::tests::native_candidate_stale_observation_waits_without_signing_or_custody_loss',
        'state::tests::native_candidate_controls_fit_without_displacing_or_duplicating_economic_input',
        'state::tests::native_candidate_refuses_unsupported_carrier_controls_before_signing',
        'state::tests::native_candidate_proof_rejects_foreign_state_and_network',
        'state::tests::native_candidate_handoff_rejects_retired_merge_before_signing',
        'state::tests::native_candidate_handoff_rejects_foreign_original_state',
        'state::tests::native_candidate_partial_atomic_handoff_retains_waits_and_independent_work',
        'sumeragi::v2_candidate::tests::native_source_wait_never_selects_ordinary_fallback',
        'state::tests::native_preparation_preserves_local_recorder_conflict',
    )),
    ('native preparation and recorded controls preserve original validation', (
        'state::tests::native_preparation_single_retains_real_suffix_controls_and_unpublished_outputs',
        'state::tests::native_preparation_atomic_retains_real_suffix_controls_and_unpublished_outputs',
        'state::tests::native_preparation_single_authenticates_original_durable_sources_under_lease',
        'state::tests::native_preparation_atomic_authenticates_original_durable_sources_under_lease',
        'state::tests::native_preparation_rejects_signed_noncanonical_time',
        'state::tests::native_preparation_rejects_signed_confidential_policy_substitution',
        'state::tests::native_preparation_rejects_wrong_and_multiple_origin_signatures',
        'state::tests::native_preparation_rejects_stale_source_without_execution_or_publication',
        'state::tests::native_preparation_retained_prefix_does_not_authorize_raw_state_commit',
        'state::tests::native_preparation_refreshes_source_after_actual_finalized_height_advance',
        'state::tests::native_recorded_control_rejects_changed_opening_and_stale_verified_height',
        'state::tests::native_recorded_control_rejects_missing_corrupt_and_foreign_parent_beacon',
    )),
    ('native service preparation retains original source and archive owners', (
        'state::tests::native_service_preparation_single_preserves_original_sources_and_archives',
        'state::tests::native_service_preparation_atomic_preserves_original_sources_and_archives',
        'state::tests::native_service_preparation_index_busy_precedes_execution',
        'state::tests::native_service_preparation_capture_busy_releases_partial_owner',
        'state::tests::native_service_preparation_stale_source_skips_archives_and_execution',
        'state::tests::native_service_preparation_foreign_source_and_body_are_rejected',
        'state::tests::native_service_preparation_recorder_conflict_releases_archives',
    )),
    ('native failure provenance retains local dependencies', (
        'sumeragi::v2_apply::tests::native_preparation_errors::hash_admission_retains_original_release_and_runner_through_all_native_origins',
        'sumeragi::v2_apply::tests::native_preparation_errors::native_controls_preserve_local_storage_failure_and_semantic_rejection',
        'sumeragi::v2_apply::tests::native_preparation_errors::metadata_and_recorder_diagnostics_cannot_authorize_negative_markers',
        'sumeragi::v2_apply::tests::native_preparation_errors::governed_native_batch_limit_remains_a_semantic_body_verdict',
    )),
    ('preexecution archive reservation preserves original service and release', (
        'sumeragi::v2_apply::tests::archive_reservations::acquires_original_pair_without_execution',
        'sumeragi::v2_apply::tests::archive_reservations::index_busy_wakes_original_runner',
        'sumeragi::v2_apply::tests::archive_reservations::second_capture_refusal_releases_first',
        'sumeragi::v2_apply::tests::archive_reservations::original_capture_drop_wakes_runner_and_preserves_old_wait',
        'sumeragi::v2_apply::tests::archive_reservations::rejects_mismatch_before_acquisition',
        'sumeragi::v2_apply::tests::archive_reservations::handoff_retains_owner_on_context_wire_and_service_mismatch',
        'sumeragi::v2_apply::tests::archive_reservations::local_archive_failure_requires_recovery',
    )),
    ('archive capture reservations retain exact release identity', (
        'query::archive_capture::tests::only_the_exact_original_gate_accepts_its_retained_owner',
        'query::archive_capture::tests::observers_neither_own_nor_cancel_the_reservation',
        'query::archive_capture::tests::release_before_wait_registration_cannot_be_missed',
        'query::archive_capture::tests::active_wait_is_woken_by_the_actual_owner_drop',
        'query::archive_capture::tests::old_wait_remains_released_while_a_new_owner_is_active',
        'query::archive_capture::tests::move_to_another_worker_preserves_custody_without_retaining_the_archive',
        'query::archive_capture::tests::concurrent_attempts_retain_exactly_one_original_owner',
    )),
    ('retained validation dispatch preserves original request and carrier', (
        'sumeragi::v2_lifecycle_coordinator::work_registry::tests::retained_dispatch::retained_dispatch_marker_failures_return_exact_wait_and_original_owner',
        'sumeragi::v2_lifecycle_coordinator::work_registry::tests::retained_dispatch::retained_dispatch_capture_refusal_keeps_exact_wait_without_success_marker',
        'sumeragi::v2_lifecycle_coordinator::work_registry::tests::retained_dispatch::retained_dispatch_cache_and_reproposal_reuse_original_owner',
        'sumeragi::v2_lifecycle_coordinator::work_registry::tests::retained_dispatch::retained_dispatch_foreign_store_returns_request_before_execution',
        'sumeragi::v2_lifecycle_coordinator::work_registry::tests::retained_dispatch::retained_dispatch_cached_scalar_receipt_cannot_replace_missing_owner',
    )),
    ('original successor admission and reader readiness', (
        'state::block_hashes_admission::tests::successor_reader_contention_wakes_from_original_reader_release',
        'state::block_hashes_admission::tests::successor_admission_signals_only_actual_writer_after_unlock',
    )),
    ('actual hash writer refusal custody', (
        'state::block_hashes_publication::tests::stale_hash_refusal_retains_release_and_installation_until_outer_unlock',
    )),
    ('joint Kura partial and cold release ownership', (
        'kura::publication_lease::tests::partial_kura_refusal_releases_every_acquired_fence_before_callbacks',
        'kura::publication_lease::tests::full_and_partial_kura_abandonment_release_jointly_even_on_unwind',
        'kura::publication_lease::tests::cold_kura_sidecar_wakes_after_joint_success_and_real_storage_refusal',
        'kura::publication_lease::tests::repeated_cold_kura_lookups_retain_one_batch_through_outer_unwind',
        'kura::publication_lease::tests::foreign_cold_batch_returns_original_guard_for_joint_cleanup',
        'kura::tests::native_amx_live_custody_wrappers_unlock_together_before_callbacks',
    )),
    ('partial publication refusals release before notification', (
        'queue::tests::lane_retirement_observer::refused_cut_retains_original_notifications_through_outer_fence',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::route_refusal_retains_original_cut_cleanup_through_lifecycle',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::queue_publication_tests::state_fence_refusal_defers_callbacks_through_original_queue_and_kura',
        'sumeragi::v2_apply::retirement_release_tests::autoscale_queue_scan_and_refusal_release_lifecycle_before_queue_wake',
    )),
    ('native process transport and exact source recovery', (
        'state::tests::native_transport_production_poll_retries_real_actor_pressure_without_substitution',
        'state::tests::native_transport_production_poll_fences_closed_actor_without_losing_fanout',
        'state::tests::native_transport_production_decision_reaches_global_nonmembers_after_rollover',
        'state::tests::native_driver_source_recovery_rejoins_original_owner_after_foreign_refusal',
    )),
    ('finite World journal shell planning', (
        'state::world_journals::tests::publication_tests::world_publication_retains_original_busy_notification_until_aggregate_unlock',
        'state::world_journals::resources::tests::world_shell_plan_matches_constructed_capture_and_installation_layouts',
        'state::world_journals::resources::tests::world_shell_reservation_holds_capture_abort_retry_and_refunds_after_drop',
        'state::world_journals::resources::tests::world_shell_planning_never_reads_targets_or_acquires_held_writers',
        'state::world_journals::resources::tests::world_shell_planning_checks_each_sum_count_and_vector_layout_overflow',
        'state::carrier_preparation::journals::tests::carrier_journal_shell_plan_precedes_execution_and_survives_capture',
    )),
    ('scoped original World storage publication', (
        'state::world_journals::storage_mode::tests::prepaid_world_storage_adapter_refuses_missing_and_foreign_scope_before_writers',
        'state::world_journals::storage_mode::tests::prepaid_world_storage_adapter_preserves_original_pair_through_abort_and_publish',
        'state::world_journals::storage_mode::tests::prepaid_world_storage_adapter_busy_retry_keeps_exact_original_values',
    )),
    ('retained candidate descriptors and exact marker custody', (
        'sumeragi::v2_body_store::tests::retained_validation_tests::incomplete_retained_owner_cannot_authorize_a_marker_even_when_resume_reports_success',
        'sumeragi::v2_body_store::tests::retained_validation_tests::ready_retained_owner_skips_capture_resume_through_marker_retry_and_cache',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_marker_file_sync_refusal_keeps_owner_through_retry_abort_and_consume',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_reproposal_directory_sync_refusal_preserves_prior_confirmed_receipt',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_consumption_tombstone_rejects_delayed_earlier_round_without_execution',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_validation_requires_exact_store_and_existing_cached_owner',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_descriptor_capacity_refuses_before_execution_or_marker_write',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_descriptor_byte_admission_precedes_allocation_and_execution',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_descriptor_charge_outlives_payload_and_wakes_exact_pool_retry',
        'sumeragi::v2_body_store::tests::retained_validation_tests::retained_descriptor_zero_and_overflow_do_not_allocate_or_execute',
    )),
    ('original service Queue retirement publication', (
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::original_queue_cut_binds_retirement_and_replacement_until_drop',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::empty_decoy_queue_and_foreign_state_never_supply_original_cut',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::malformed_captured_retirement_route_releases_original_queue_cut',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::pending_queue_work_releases_without_applying_the_blocked_carrier',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::queue_cut_does_not_replace_kura_or_original_geometry_authority',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::sticky_queue_fault_revokes_retained_retirement_before_storage_or_visibility',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::immutable_apply_service_exposes_only_its_actual_state_and_queue',
        'state::carrier_geometry_preparation::tests::queue_retirement_tests::original_queue_cut_completes_retirement_storage_without_publishing_state',
    )),
    ('retained carrier physical publication and release', (
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::retained_execution_phases_survive_marker_reproposal_and_publication_refusals',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::retained_capture_refusal_resumes_original_archives_before_any_validation_marker',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::physical_preparation_diagnostics_retain_storage_cause_and_busy_owner',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::original_state_and_header_are_required_before_witness_or_archive_writes',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::joint_publication_persists_both_original_archives_without_state_effects_or_relocking',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::foreign_archive_refusal_precedes_state_acquisition_and_returns_complete_retry',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::source_substitution_refuses_before_state_acquisition_and_retains_original_retry',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::changed_carrier_wire_refuses_source_join_and_restored_owner_reauthenticates',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::every_busy_carrier_family_releases_earlier_writers_and_retains_exact_retry',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::aggregate_acquisition_holds_every_family_without_publishing_or_losing_originals',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::geometry_refusal_returns_original_decision_and_releases_every_physical_writer',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::geometry_backend_contention_releases_writers_and_waits_for_actual_backend_release',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::lifecycle_effect_refusal_precedes_storage_and_preserves_exact_retry',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::installation_refusal_precedes_all_fences_and_returns_the_decided_carrier',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::changed_world_predecessor_releases_all_earlier_families_without_rebinding',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::actual_validation_overlay_defers_at_hash_before_taking_its_world_writers',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::identical_foreign_state_cannot_replace_the_original_physical_owners',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::all_reservations_outlive_component_writers_and_state_fences_on_drop_and_abort',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::original_kura_contention_returns_exact_decided_carrier_and_release_driven_retry',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::original_kura_storage_failure_returns_carrier_and_releases_all_acquired_owners',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::checkpoint_storage_refusal_precedes_state_and_retains_exact_originals',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::exact_checkpoint_retry_preserves_receipt_across_physical_abort',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::attached_foreign_checkpoint_never_grants_state_acquisition',
        'state::carrier_preparation::journals::decision_binding::physical_publication::tests::queue_publication_tests::signed_retirement_and_replacement_publish_once_under_original_service_queue_cut',
    )),
    ('original carrier geometry retries and retirement', (
        'state::carrier_geometry_preparation::tests::carrier_geometry_captures_original_predecessor_and_drop_does_not_publish',
        'state::carrier_geometry_preparation::tests::carrier_geometry_identity_requires_its_exact_captured_header',
        'state::carrier_geometry_preparation::tests::carrier_geometry_rejects_changed_header_and_forged_pending_predecessor',
        'state::carrier_geometry_preparation::tests::carrier_geometry_rejects_ownerless_successor_and_ignores_physical_cache',
        'state::carrier_geometry_preparation::tests::carrier_geometry_replacement_uses_actual_undo_including_retired_lineage',
        'state::carrier_geometry_preparation::tests::carrier_geometry_completion_requires_original_state_and_exact_header_before_effects',
        'state::carrier_geometry_preparation::tests::carrier_geometry_retirement_and_replacement_require_original_queue_custody',
        'state::carrier_geometry_preparation::tests::carrier_geometry_foreign_lease_refuses_before_descriptor_capture_or_effects',
        'state::carrier_geometry_preparation::tests::carrier_geometry_preparation_is_pure_and_root_change_refuses_before_raw_effects',
        'state::carrier_geometry_preparation::tests::carrier_geometry_retries_sync_failure_under_held_lease_without_state_publication',
        'state::carrier_geometry_preparation::tests::carrier_geometry_completion_requires_original_prepared_descriptors',
        'state::carrier_geometry_preparation::tests::carrier_geometry_catalog_sync_retry_preserves_original_mapping_and_state',
        'state::carrier_geometry_preparation::tests::carrier_geometry_completed_catalog_refuses_identical_replacement_journal',
        'state::carrier_geometry_preparation::tests::carrier_geometry_no_change_completion_has_no_mapping_or_storage_owner',
    )),
    ('actual terminal carrier publication', (
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::tests::consumes_original_journals_once_with_one_visibility_interval',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::tests::wrong_retained_header_returns_original_decision_and_releases_every_writer',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::tests::prevalidation_returns_owner_without_visibility_then_real_owner_publishes',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::tests::foreign_geometry_returns_original_owner_before_any_visibility_change',
    )),
    ('native publication and original driver Apply settlement', (
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_single_publishes_original_sources_and_exact_checkpoint_once',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_atomic_publishes_original_sources_and_exact_checkpoint_once',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_driver_settles_original_closed_apply_only_after_real_publication',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_published_terminal_retires_closed_body_without_local_qc',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_published_terminal_checks_unacknowledged_durable_decision',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_published_terminal_refuses_conflicting_durable_decision',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_published_terminal_checks_unlaunched_decision',
        'state::carrier_preparation::journals::decision_binding::physical_publication::publication::native_tests::native_published_terminal_refuses_conflicting_unlaunched_decision',
    )),
)
CORE_STARTUP_STAGES += CORE_NATIVE_CONNECTION_STAGES
CORE_ADMISSION_STARTUP_STAGES += CORE_NATIVE_CONNECTION_STAGES
CORE_STAGES += CORE_NATIVE_CONNECTION_STAGES


MV_ADMITTED_MAP_STAGES = (
    ('finite admitted map custody and original successors', (
        'complete_demand_refusal_allocates_nothing_and_retries_the_original_input_after_release',
        'nonuniform_nested_payloads_split_and_grow_while_original_readers_retain_actual_credits',
        'detached_public_owner_rejects_foreign_and_busy_maps_without_readmission_or_copy',
        'replacement_and_detached_successor_keep_their_original_storage_after_map_drop',
        'every_partial_leaf_clone_unwind_reclaims_new_storage_and_preserves_published_references',
        'old_reader_and_abort_refunds_wake_only_after_the_original_writer_unlocks',
        'retained_successor_grows_and_replaces_entries_before_one_atomic_publication',
        'retained_capacity_refusal_preserves_private_entries_and_input_then_retries',
        'retained_edits_refuse_foreign_busy_and_changed_generations_before_admission',
        'fully_exhausted_budget_can_abort_all_retained_edits_without_allocating',
        'later_copy_unwind_aborts_the_whole_private_successor_and_preserves_published_storage',
        'private_leaf_split_unwind_reclaims_all_previous_edits_without_publication',
        'retired_tracking_charge_unwind_sees_installed_bookkeeping_and_aborts_all_private_nodes',
        'full_budget_checkpoint_abort_restores_original_private_entries_buffers_and_credits',
        'nested_checkpoint_apply_abort_and_sibling_apply_preserve_original_parent_until_commit',
        'caught_checkpoint_edit_panic_cannot_read_detach_or_publish_the_original_cursor',
        'checkpoint_capacity_refusal_keeps_child_state_and_original_input_for_retry',
        'checkpoint_buffer_refund_panic_restores_parent_ownership_and_forbids_publication',
    )),
    ('funded map removal and original retained generations', (
        'admitted_removal_funds_all_path_sibling_and_separator_copies_until_empty',
        'admitted_removal_refusal_and_absence_preserve_original_private_generation',
        'admitted_removal_nested_abort_restores_original_nodes_at_full_capacity',
        'admitted_removal_clone_unwind_cannot_publish_and_refunds_private_copies',
    )),
    ('original admitted writer start', (
        'admitted_empty_writer_starts_without_edits_and_grows_under_separate_admission',
        'admitted_populated_writer_shares_original_entries_and_aborts_without_allocations',
        'admitted_writer_start_refuses_one_byte_below_and_accepts_exact_complete_demand',
    )),
    ('joint current and undo map admission', (
        'pair_complete_demand_refusal_preserves_original_inputs_and_exact_budget_retry',
        'pair_first_none_and_some_preimages_survive_replacement_growth_and_reader_custody',
        'pair_callback_and_nested_clone_panics_preserve_both_published_roots_and_reclaim_private_storage',
        'pair_foreign_and_busy_roles_return_original_nested_inputs_without_readmission',
    )),
    ('prepared map inputs and exact shared reservation', (
        'prepared_checkpoint_cancel_and_exact_capacity_refusal_retain_original_input_and_root',
        'paired_preparations_share_one_reservation_and_preserve_independent_checkpoint_rollback',
        'dropping_prepared_writer_input_never_edits_or_clones_the_original_cursor',
        'current_undo_and_touch_preparations_share_original_credit_and_abort_all_three_roots',
        'admitted_optional_none_is_retained_without_value_copy_and_survives_sibling_abort',
        'incoming_preimage_copy_unwind_reclaims_copies_and_poison_prevents_publication',
        'copied_preimage_tracking_cleanup_panic_restores_original_parent_and_blocks_publication',
    )),
    ('production Storage admission and original block custody', (
        'storage_custody::actual_storage_resets_first_none_and_some_between_blocks_and_aborts_parent',
        'storage_custody::actual_storage_joined_refusal_precedes_clone_and_exact_budget_retry_preserves_input',
        'storage_custody::actual_storage_old_reader_owns_nested_bytes_and_credits_until_physical_release',
        'storage_custody::actual_storage_refund_wake_reenters_only_after_both_original_writers_release',
        'storage_custody::actual_storage_constructor_rejects_foreign_and_short_policy_without_retained_credits',
        'storage_custody::actual_storage_edit_rejects_foreign_and_short_policy_before_cloning_or_mutation',
        'storage_custody::actual_storage_summed_startup_and_reset_refusal_preserve_both_committed_images',
        'storage_custody::actual_storage_caught_edit_panic_cannot_publish_and_reclaims_private_credits',
    )),
    ('production Transaction admission and original parent custody', (
        'storage_custody::actual_transaction_joined_touch_and_pair_refusal_preserves_inputs_for_exact_retry',
        'storage_custody::actual_transaction_ordered_unique_touches_preserve_noop_and_sibling_preimages',
        'storage_custody::actual_transaction_full_budget_abort_restores_parent_and_outer_abort_preserves_readers',
        'storage_custody::actual_transaction_caught_touch_and_pair_copy_panics_cannot_apply_or_publish',
        'storage_custody::actual_transaction_touch_destructor_panic_cannot_apply_or_publish',
    )),
    ('production Block and Transaction deletion custody', (
        'storage_custody::actual_block_removal_preserves_first_preimages_and_absent_dirty_semantics',
        'storage_custody::actual_transaction_removal_orders_explicit_absence_and_sibling_preimages',
        'storage_custody::actual_block_removal_refusal_preserves_exact_query_for_complete_budget_retry',
        'storage_custody::actual_transaction_removal_refusal_joins_touch_and_pair_before_exact_query_retry',
        'storage_custody::actual_transaction_removal_exhausted_abort_restores_parent_and_outer_reader_custody',
        'storage_custody::actual_removal_caught_copy_and_consumed_query_panics_cannot_publish',
    )),
    ('funded Storage reads and retained removal ownership', (
        'storage_custody::prepaid_storage_reads_need_no_heap_credit_or_payload_copy',
        'storage_custody::missing_removal_planning_refusal_preserves_original_query_and_touch_owners',
        'storage_custody::removed_value_retains_its_original_allocation_after_transaction_and_block_abort',
    )),
    ('funded replacement restores original Storage preimages', (
        'storage_custody::actual_storage_replacement_funds_copies_and_preserves_original_readers',
        'storage_custody::actual_storage_replacement_capacity_refusal_restores_roots_after_partial_work',
        'storage_custody::actual_storage_replacement_copy_panic_aborts_original_pair_and_poisons_retry',
    )),
    ('funded snapshot restoration preserves original source and complete undo custody', (
        'storage_custody::storage_snapshot_restore_preserves_nested_custody_and_allocation_free_history',
        'storage_custody::storage_snapshot_undo_prefix_refusal_preserves_source_for_exact_retry',
        'storage_custody::storage_snapshot_undo_copy_and_factory_unwind_leave_source_healthy',
    )),
    ('funded Storage publication identities', (
        'storage_custody::storage_publication_identity_is_prepaid_and_retained_after_storage_drop',
        'storage_custody::storage_writer_identity_refusal_precedes_policies_and_preserves_retry',
    )),
    ('funded detached Storage capture and scoped publication', (
        'storage_custody::capture::captured_prepaid_successors_detach_abort_and_publish_at_full_capacity_without_copy',
        'storage_custody::capture::captured_prepaid_refusals_return_original_owner_for_exact_retry',
        'storage_custody::capture::captured_prepaid_replacement_retains_mode_and_rejects_a_changed_pair',
        'storage_custody::capture::captured_prepaid_pair_shares_one_scope_through_publication_abort_and_unwind',
        'storage_custody::capture::captured_prepaid_caught_edit_panic_cannot_escape_as_a_journal',
    )),
)

CONCREAD_STAGES = (
    ('original EBR acquisition and unlocked reclamation', (
        'ebrcell::acquisition_tests::raw_acquisition_and_refusal_retain_the_exact_writer_without_cloning',
        'ebrcell::acquisition_tests::acquired_clone_and_attachment_keep_the_original_allocation',
        'ebrcell::acquisition_tests::consumed_clone_panic_releases_and_poisons_before_caller_recovery',
        'ebrcell::acquisition_tests::poisoned_attachment_returns_both_original_owners',
    )),
    ('original map acquisition custody', (
        'bptree::acquisition_tests::acquired_map_validation_retains_stale_and_poisoned_physical_writers',
        'bptree::acquisition_tests::acquired_map_foreign_busy_success_and_unwind_preserve_original_custody',
    )),
    ('native physical release ownership', (
        'release::tests::release_before_registration_is_retained_and_other_sources_do_not_wake',
        'release::tests::first_registered_wake_can_reenter_both_initialized_notification_locks',
        'release::tests::panicking_first_waker_still_notifies_the_remaining_original_cohort',
        'release::tests::cancellation_and_waker_replacement_do_not_steal_another_wait',
        'release::tests::replacing_a_waker_allows_its_destructor_to_observe_the_same_source',
        'release::tests::ready_wait_releases_its_last_waker_outside_the_notification_lock',
        'release::tests::release_racing_first_poll_cannot_be_lost',
        'release::tests::ownership_phase_transfer_defers_original_release_until_final_owner_drops',
        'release::tests::ownership_phase_transfer_unwind_releases_and_poisons_original_observation',
        'release::tests::physical_release_disarms_only_later_retirement_poisoning',
        'release::tests::paired_release_uses_actual_poison_and_unlocks_both_before_callback_unwind',
        'release::tests::observed_release_reports_existing_physical_poison_and_excludes_later_wake_panic',
        'release::tests::pair_construction_transfers_both_original_guards_without_early_release',
        'release::tests::deferred_release_keeps_original_wait_and_ignores_later_cleanup_unwind',
        'release::tests::fallible_phase_transfer_retains_the_original_guard_and_owned_cleanup',
        'release::tests::release_batch_empty_and_foreign_transfer_preserve_original_custody',
        'release::tests::release_batch_coalesces_reacquisitions_without_allocating_or_early_wakes',
        'release::tests::release_batch_records_actual_physical_poison_without_later_cleanup_poison',
        'release::tests::retained_phase_transfer_and_refusal_keep_original_source_without_early_wake',
        'release::tests::retained_phase_unwind_records_actual_release_without_running_waiter',
        'release::tests::retained_observed_release_preserves_poison_predating_normal_cleanup',
    )),
    ('failed native cursor retains cleanup after unlock', (
        'bptree::abandonment_tests::failed_cursor_abandonment_unlocks_without_reopening_publication_authority',
    )),
    ('actual reader mutex readiness', (
        'internals::lincowcell::identity_preparation_tests::reader_wait_survives_refused_writer_release_and_registration_races',
        'internals::lincowcell::identity_preparation_tests::reader_release_covers_reads_advice_abort_and_both_commit_paths',
        'internals::lincowcell::identity_preparation_tests::reader_abort_retains_notification_until_the_original_writer_releases',
        'internals::lincowcell::identity_preparation_tests::reader_wake_unwind_preserves_physical_poison_and_original_commit',
    )),
    ('admitted B+ tree planning and retained edits', (
        'bptree::admission::tests::acquired_admission_refusal_retains_actual_writer_and_deferred_release',
        'bptree::admission::tests::acquired_admission_busy_poison_and_unwind_preserve_real_custody',
        'bptree::admission::tests::acquired_admission_success_and_planning_refusal_preserve_original_input',
        'bptree::admission::tests::demand_overflow_preserves_the_original_sum_and_zero_layout_needs_no_allocation',
        'bptree::admission::tests::empty_map_plan_includes_both_shells_fixed_buffers_and_full_root_growth_bound',
        'bptree::admission::tests::exhausted_generation_refuses_before_admission_or_successor_allocation',
        'bptree::admission::tests::unsupported_payload_returns_original_owners_without_calling_admission',
        'bptree::admission::tests::initial_node_admission_refusal_constructs_no_root_or_reader',
        'bptree::admission::tests::retained_edits_keep_the_original_cursor_and_refused_tracking_then_publish_once',
        'bptree::admission::tests::tracking_growth_checks_overflow_before_changing_demand_or_allocating',
        'bptree::admission::tests::held_writer_demand_is_allocation_free_and_matches_admission_before_any_growth',
        'bptree::admission::tests::nested_checkpoint_demand_tracks_retirement_growth_and_exact_abort_restoration',
        'bptree::admission::tests::stale_observed_demand_never_skips_replanning_after_a_private_edit',
        'bptree::admission::tests::held_demand_rechecks_actual_nested_layouts_and_returns_unsupported_original_input',
        'bptree::admission::tests::whole_operation_demand_sum_checks_both_counts_and_preserves_refused_total',
        'bptree::admission::tests::empty_writer_acquisition_funds_only_shells_and_retains_the_original_root',
        'bptree::admission::tests::nested_writer_and_clear_callbacks_hold_both_original_locks_before_joint_refusal',
        'bptree::admission::tests::clear_retires_the_entire_multilevel_tree_only_after_original_reader_release',
        'bptree::admission::tests::checkpoint_clear_refusal_and_nested_apply_preserve_exact_outer_rollback',
        'bptree::admission::tests::admission_panic_poison_rejects_new_writer_and_clear_before_callbacks',
        'bptree::admission::tests::applying_retaining_transfers_both_checkpoints_before_any_original_charge_drop',
        'bptree::admission::tests::completed_clear_cannot_publish_after_caught_unused_funding_panic',
        'bptree::admission::tests::writer_and_clear_generation_exhaustion_refuse_before_admission_or_allocation',
        'bptree::admission::tests::staged_pair_commit_retains_all_cleanup_until_both_original_locks_are_released',
        'bptree::admission::tests::prepared_pair_abort_releases_original_owners_without_publishing_either_tree',
        'bptree::admission::tests::removal_planning_rejects_descendant_minimum_before_admission_or_mutation',
        'bptree::admission::tests::removal_planning_payload_visits_are_height_bounded_and_match_exact_refusal',
        'bptree::admission::tests::original_owned_prepaid_edits_replan_and_refuse_before_copies_without_a_writer',
        'bptree::admission::tests::original_owned_copy_panic_blocks_read_edit_and_reacquisition_without_poisoning_map',
        'bptree::admission::tests::original_owned_stale_refusal_retains_actual_private_allocations_until_abort',
        'bptree::admission::tests::prepaid_private_copy_updates_append_and_replacement_without_allocating_or_replanning',
        'bptree::admission::tests::prepaid_private_copy_refuses_shared_leaf_without_allocating_or_mutating_it',
        'bptree::admission::tests::prepaid_private_copy_retains_actual_allocation_custody_through_stale_abort',
        'bptree::admission::tests::prepaid_private_copy_comparison_unwind_requires_original_owner_abort',
        'bptree::admission::tests::prepaid_current_footprint_tracks_original_split_merge_overwrite_and_clear_nodes',
        'bptree::admission::tests::prepaid_current_footprint_preserves_parent_through_checkpoint_and_publication_abort',
        'bptree::admission::tests::prepaid_current_footprint_distinguishes_resident_floor_from_refundable_old_custody',
        'bptree::admission::tests::prepaid_reader_predecessor_retains_original_generation_without_allocating_and_rejects_aba',
    )),
    ('prepared B+ tree inputs and retained preimages', (
        'bptree::admission::tests::prepared_writer_and_checkpoint_planning_refuse_without_copying_original_inputs',
        'bptree::admission::tests::copied_key_planning_refusal_keeps_the_source_and_original_owned_value',
        'bptree::admission::tests::optional_copy_planning_refusal_does_not_clone_sources_or_edit_either_guard',
        'bptree::admission::tests::optional_none_is_an_existing_preimage_and_checkpoint_abort_restores_it',
        'bptree::admission::tests::original_preparation_retains_key_and_preimage_through_dependent_copy_then_cancel',
        'bptree::admission::tests::key_copy_cancel_returns_original_value_and_checkpoint_copy_uses_same_cursor',
        'bptree::admission::tests::incoming_shared_mutable_preimage_refuses_when_its_borrow_cannot_freeze_copy_demand',
    )),
    ('admitted original B+ tree writer start', (
        'bptree::admission::tests::writer_start::start_plan_is_only_two_shells_and_empty_tracking_with_checked_generation',
        'bptree::admission::tests::writer_start::empty_and_populated_starts_keep_exact_tree_and_zero_buffers_without_payload_work',
        'bptree::admission::tests::writer_start::start_busy_and_admission_refusal_allocate_nothing_and_preserve_published_state',
        'bptree::admission::tests::writer_start::exhausted_start_refuses_before_callback_or_allocation',
        'bptree::admission::tests::writer_start::provider_drop_panic_poisoned_start_never_publishes_or_returns_a_writer',
        'bptree::admission::tests::writer_start::started_original_writer_admits_later_growth_and_checkpoint_abort_without_new_credit',
    )),
    ('original cursor checkpoint ownership', (
        'internals::bptree::cursor::checkpoint::tests::abort_restores_original_root_tag_length_and_both_buffers_after_repeated_growth',
        'internals::bptree::cursor::checkpoint::tests::nested_apply_transfers_original_buffers_and_outer_abort_restores_them',
        'internals::bptree::cursor::checkpoint::tests::applied_newest_tag_survives_and_sibling_reuse_follows_actual_child_reclamation',
        'internals::bptree::cursor::checkpoint::tests::exhausted_private_tag_refuses_without_allocating_or_changing_any_owner',
        'internals::bptree::cursor::checkpoint::tests::untracked_final_generation_refuses_nested_checkpoint_and_restores_parent',
        'internals::bptree::cursor::checkpoint::tests::untracked_abort_restores_parent_nodes_and_cuts_without_allocating_after_growth',
        'internals::bptree::cursor::checkpoint::tests::public_untracked_checkpoint_reads_saved_values_and_applies_without_allocation',
        'internals::bptree::cursor::checkpoint::tests::caught_untracked_insert_remove_and_mutable_clone_panics_fail_the_original_cursor',
        'internals::bptree::cursor::checkpoint::tests::untracked_checkpoint_retains_only_live_rollback_metadata',
    )),
    ('joint current and undo admission custody', (
        'bptree::admission::pair_admission::tests::exact_joined_limit_and_one_byte_below_preserve_original_owners',
        'bptree::admission::pair_admission::tests::joined_growth_keeps_first_none_and_some_without_rewriting_existing_undo',
        'bptree::admission::pair_admission::tests::both_checkpoints_abort_to_prior_private_roots_without_credit',
        'bptree::admission::pair_admission::tests::undo_generation_is_required_only_for_missing_first_preimage',
        'bptree::admission::pair_admission::tests::current_generation_refusal_preserves_both_inputs_before_callback',
        'bptree::admission::pair_admission::tests::foreign_stale_busy_and_poisoned_roles_refuse_before_joined_admission',
        'bptree::admission::pair_admission::tests::callback_clone_and_remainder_drop_panics_poison_both_without_publication',
        'bptree::admission::pair_admission::tests::single_borrowed_edit_keeps_failure_armed_through_provider_drop',
        'bptree::admission::pair_admission::tests::joined_sum_rejects_byte_and_allocation_overflow_without_changing_demand',
    )),
    ('borrowed current and undo admission custody', (
        'bptree::admission::pair_admission::tests::borrowed::borrowed_writers_keep_original_locks_and_first_preimages',
        'bptree::admission::pair_admission::tests::borrowed::parent_refusal_and_full_budget_abort_preserve_exact_original_buffers',
        'bptree::admission::pair_admission::tests::borrowed::parent_apply_keeps_private_successors_and_outer_abort_restores_custody',
        'bptree::admission::pair_admission::tests::borrowed::borrowed_parent_generation_refuses_before_callback_and_skips_existing_undo',
        'bptree::admission::pair_admission::tests::borrowed::caught_callback_clone_and_provider_panics_invalidate_both_borrowed_parents',
        'bptree::admission::pair_admission::tests::borrowed::first_and_second_apply_cleanup_failures_invalidate_both_attached_writers',
        'bptree::admission::pair_admission::tests::borrowed::prefailed_parent_entry_invalidates_the_other_original_cursor_before_admission',
    )),
    ('admitted clear and original cursor reset custody', (
        'bptree::admission::pair_admission::tests::clear::clear_exact_limit_and_refusal_preserve_original_checkpoint_allocations',
        'bptree::admission::pair_admission::tests::clear::clear_publication_retains_actual_old_reader_preimages_and_charges',
        'bptree::admission::pair_admission::tests::clear::clear_nested_apply_and_repeated_resets_keep_full_budget_outer_abort',
        'bptree::admission::pair_admission::tests::clear::clear_generation_refusal_precedes_callback_and_preserves_writer_and_parent',
        'bptree::admission::pair_admission::tests::clear::clear_caught_callback_and_provider_panics_leave_original_parent_unusable',
        'bptree::admission::pair_admission::tests::clear::clear_apply_cleanup_panic_cannot_expose_a_usable_partial_writer',
        'bptree::admission::pair_admission::tests::clear::clear_empty_root_has_exact_finite_layouts_and_never_copies_payloads',
    )),

    ('canonical B+ tree deletion and rebalance', (
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_01_p0',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_01_p1',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_02',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_03',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_04p0',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_04p1',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_05',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_06',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_07',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_08',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_09',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_10',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_11',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_12',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_13',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_14',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_15',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_1',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_2',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_3',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_4',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_5',
        'internals::bptree::cursor::tests::test_bptree2_cursor_remove_stress_6',
    )),
    ('checked original removal tracking custody', (
        'internals::bptree::cursor::remove::tests::remove_tracking_bound_is_checked_for_root_branch_and_overflow',
        'internals::bptree::cursor::remove::tests::remove_fixed_slot_preflight_preserves_original_root_and_skips_absent_keys',
        'internals::bptree::cursor::remove::tests::remove_exact_root_slots_preserve_original_reader_and_skip_absent_clone',
    )),

    ('admitted paired removal and nested sibling custody', (
        'bptree::admission::pair_admission::tests::deletion::removal_retains_first_some_and_explicit_none_under_original_writers',
        'bptree::admission::pair_admission::tests::deletion::repeated_absent_joint_removal_needs_no_current_or_undo_allocation',
        'bptree::admission::pair_admission::tests::deletion::removal_whole_demand_refusal_and_exact_retry_preserve_parent_custody',
        'bptree::admission::pair_admission::tests::deletion::removal_orders_rebalance_and_abort_without_credit_or_reader_changes',
        'bptree::admission::pair_admission::tests::deletion::removal_generation_refuses_before_callback_and_skips_existing_undo',
        'bptree::admission::pair_admission::tests::deletion::removal_caught_callback_clone_and_provider_panics_invalidate_both_parents',
        'bptree::admission::pair_admission::tests::deletion::removal_prefailed_parent_invalidates_other_owner_before_admission',
        'bptree::admission::pair_admission::tests::deletion::payload::nonuniform_sibling_and_repair_payloads_fit_complete_demand_and_old_readers',
        'bptree::admission::pair_admission::tests::deletion::payload::nonuniform_copy_and_query_drop_failures_poison_both_original_parents',
        'bptree::admission::pair_admission::tests::deletion::payload::replacement_input_key_drop_preserves_previous_value_custody',
    )),

)

WALLET_STAGES = (("bounded faucet proof-of-work deadline", (
    "faucet_pow::resource_tests::faucet_preparation_deadline_stops_cpu_work_before_dispatch",
)),)

CLIENT_STAGES += (("canonical public faucet advertisement decoder", (
    "account_bootstrap::tests::faucet_discovery_requires_exact_canonical_v1_fields",
)),)

def qualification_stages(qualification_scope: str = "basic") -> dict[str, tuple]:
    """Select honest test coverage without changing shipping features or artifacts."""
    if qualification_scope not in QUALIFICATION_SCOPES:
        raise CheckError("native qualification scope must be basic or full")
    selected = {
        "mv": MV_OWNERSHIP_STAGES, "mv-ebr": MV_EBR_STAGES, "mv-map": MV_MAP_STAGES,
        "mv-admitted-map": MV_ADMITTED_MAP_STAGES, "concread": CONCREAD_STAGES,
        "config": CONFIG_STAGES, "config-unit": CONFIG_UNIT_STAGES, "data-model": DATA_MODEL_STAGES,
        "kagami": KAGAMI_STAGES,
        "proof": PROOF_STAGES, "proof-flows": PROOF_FLOW_STAGES,
        "crypto": CRYPTO_STAGES, "p2p": P2P_STAGES, "core": CORE_STAGES,
        "test-network": TEST_NETWORK_STAGES, "client": CLIENT_STAGES, "wallet": WALLET_STAGES,
        "torii-unit": TORII_UNIT_STAGES, "torii": TORII_STAGES,
        "torii-shared": TORII_SHARED_STAGES, "torii-lifecycle": TORII_LIFECYCLE_STAGES,
        "daemon": DAEMON_STAGES, "network": NETWORK_STAGES, "cli": STAGES,
    }
    if qualification_scope == "basic":
        # These affected startup regressions and the consolidated real-custody network
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
    command = _compile_command(root, env, native_harness_selection(harnesses))
    return _build_harnesses(root, command, env, harnesses, lock_fds)


CARGO_PROGRESS_INTERVAL_SECONDS = 30


class CargoBuildProgress:
    """Describe observed Cargo work without changing selection or qualification."""

    def __init__(self, phase: str, requested: set[tuple[str, str]], *, test_profile: bool):
        self._lock = threading.RLock()
        self.phase = phase
        self.requested = requested
        self.test_profile = test_profile
        self.units: dict[str, bool | None] = {}
        self.targets: set[tuple[str, str]] = set()
        self.started = self.last_report = time.monotonic()
        print(f"[taira-cargo] {phase}: {len(requested)} requested targets "
              + ", ".join(f"{kind}:{name}" for kind, name in sorted(requested))
              + "; Cargo also resolves dependencies and implicit targets", flush=True)

    def observe(self, line: str) -> None:
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            return
        if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
            return
        target, profile = event.get("target"), event.get("profile")
        if not isinstance(target, dict) or not isinstance(profile, dict):
            return
        name, kinds = target.get("name"), target.get("kind")
        if (not isinstance(name, str) or not isinstance(kinds, list)
                or not all(isinstance(kind, str) for kind in kinds)):
            return
        # Include profile/features/output identity so distinct compilation units
        # remain distinct, while repeated reports of one artifact do not inflate work.
        identity = json.dumps({key: event.get(key) for key in
                               ("package_id", "target", "profile", "features", "filenames", "executable")},
                              sort_keys=True)
        fresh = event.get("fresh")
        if type(fresh) is not bool:
            fresh = None
        with self._lock:
            previous = self.units.get(identity)
            if identity not in self.units or previous is None or fresh is False:
                self.units[identity] = fresh
            if profile.get("test") is self.test_profile:
                for kind in kinds:
                    if (self.test_profile and kind in {"lib", "bin", "test", "example", "bench"}
                            or not self.test_profile and kind == "bin"):
                        self.targets.add((kind, name))
            now = time.monotonic()
            if now - self.last_report >= CARGO_PROGRESS_INTERVAL_SECONDS:
                self.report("running", now=now)

    def report(self, state: str, *, now: float | None = None) -> None:
        with self._lock:
            now = time.monotonic() if now is None else now
            self.last_report = now
            counts = {"fresh": 0, "rebuilt": 0, "unknown": 0}
            for fresh in self.units.values():
                counts["fresh" if fresh is True else "rebuilt" if fresh is False else "unknown"] += 1
            print("[taira-cargo] " + json.dumps({
                "phase": self.phase, "state": state, "elapsed_seconds": round(now - self.started, 1),
                "requested_targets": [f"{kind}:{name}" for kind, name in sorted(self.requested)],
                "observed_targets": [f"{kind}:{name}" for kind, name in sorted(self.targets)],
                "additional_targets": [f"{kind}:{name}" for kind, name in sorted(self.targets - self.requested)],
                "observed_artifact_units": counts,
            }, sort_keys=True), flush=True)

    @contextlib.contextmanager
    def heartbeat(self):
        """Report elapsed work during quiet compilation without requiring a Cargo event."""
        stopped = threading.Event()

        def report_while_running():
            while not stopped.wait(CARGO_PROGRESS_INTERVAL_SECONDS):
                with self._lock:
                    now = time.monotonic()
                    if now - self.last_report >= CARGO_PROGRESS_INTERVAL_SECONDS:
                        self.report("running", now=now)

        reporter = threading.Thread(target=report_while_running, name="taira-cargo-progress", daemon=True)
        reporter.start()
        try:
            yield
        finally:
            stopped.set()
            reporter.join()


def native_harness_selection(harnesses: tuple[str, ...]) -> list[str]:
    """Share the package/target/default-feature union for check and build.

    Cargo applies --lib to every selected package; requested harnesses are the
    qualification census, not an exact count of compiler targets. Progress uses
    artifact events to expose the additional targets without changing this graph.
    """
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
    return [*selection, *targets]


def check_test_harnesses(root: Path, env: dict[str, str], *,
                        harnesses: tuple[str, ...], lock_fds: tuple[int, ...] = ()) -> None:
    """Check the selected test graph before codegen; never produce qualification.

    Stable Cargo's --profile test enables cfg(test) for the explicit lib/bin/test
    targets. --tests would broaden the selection to unrelated integration tests.
    Reuse the caller's coordinated target, toolchain, features and jobserver.
    Build scripts and procedural macros can still require host code generation.
    """
    selection = native_harness_selection(harnesses)
    command = [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "check",
               "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
               *selection, "--profile", "test", "--message-format=json-render-diagnostics"]
    progress = CargoBuildProgress("test metadata", {
        (HARNESS_TARGETS[harness][2], HARNESS_TARGETS[harness][1]) for harness in harnesses
    }, test_profile=True)
    started = time.monotonic()
    observed = set()
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds,
                          umask=0o077) as child, progress.heartbeat():
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            progress.observe(line)
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if (not isinstance(event, dict) or event.get("reason") != "compiler-artifact"
                    or event.get("profile", {}).get("test") is not True):
                continue
            target = event.get("target", {})
            for harness in harnesses:
                _, name, kind, _ = HARNESS_TARGETS[harness]
                if target.get("name") == name and kind in target.get("kind", []):
                    observed.add(harness)
        code = child.wait()
    elapsed = time.monotonic() - started
    progress.report(f"Cargo exited {code}")
    if code:
        raise CheckError(f"native test metadata check failed (exit {code}, {elapsed:.1f}s)")
    missing = [harness for harness in harnesses if harness not in observed]
    if missing:
        raise CheckError("native test metadata check omitted selected test targets: " + ", ".join(missing))
    print(f"[taira-check] native test metadata check passed in {elapsed:.1f}s; "
          "full harness compilation remains required", flush=True)


def check_shipping_binaries(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> None:
    """Check the authoritative production graph without test/dev feature unification.

    Keep the caller's warm target, toolchain and locks. Cargo check compiles
    metadata only for the exact default-feature shipping binaries; it neither
    copies executables nor contributes evidence to the independent test pass.
    """
    harnesses = shipping_harnesses(root)
    selection = native_harness_selection(harnesses)
    if any(HARNESS_TARGETS[name][2] != "bin" for name in harnesses):
        raise CheckError("shipping metadata requires only authoritative binary targets")
    requested = {("bin", HARNESS_TARGETS[name][1]) for name in harnesses}
    fixture_features = {"iroha_core": "iroha-core-tests", "iroha_torii": "test-fixtures"}
    command = [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "check",
               "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
               *selection, "--message-format=json-render-diagnostics"]
    progress = CargoBuildProgress("shipping metadata", requested, test_profile=False)
    started = time.monotonic()
    observed = set()
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds,
                          umask=0o077) as child, progress.heartbeat():
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            progress.observe(line)
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
                continue
            target, profile = event.get("target"), event.get("profile")
            if (isinstance(target, dict) and isinstance(target.get("name"), str)
                    and target["name"] in fixture_features
                    and isinstance(target.get("kind"), list) and "lib" in target["kind"]):
                features = event.get("features")
                if not isinstance(features, list) or not all(isinstance(feature, str) for feature in features):
                    raise CheckError("shipping metadata omitted production library features: " + target["name"])
                forbidden = fixture_features[target["name"]]
                if forbidden in features:
                    raise CheckError(f"shipping metadata enabled forbidden fixture feature: {target['name']}/{forbidden}")
            if (isinstance(target, dict) and isinstance(profile, dict)
                    and profile.get("test") is False and isinstance(target.get("kind"), list)
                    and isinstance(target.get("name"), str)
                    and "bin" in target["kind"] and ("bin", target.get("name")) in requested):
                observed.add(("bin", target["name"]))
        code = child.wait()
    elapsed = time.monotonic() - started
    progress.report(f"Cargo exited {code}")
    if code:
        raise CheckError(f"shipping metadata check failed (exit {code}, {elapsed:.1f}s)")
    missing = sorted(name for kind, name in requested - observed)
    if missing:
        raise CheckError("shipping metadata check omitted production binary targets: " + ", ".join(missing))
    print(f"[taira-check] shipping metadata check passed in {elapsed:.1f}s; "
          "shipping codegen and network qualification remain required", flush=True)


def _build_harnesses(root: Path, command: list[str], env: dict[str, str],
                     harnesses: tuple[str, ...], lock_fds: tuple[int, ...]) -> NativeArtifactCopies:
    label = "; ".join(HARNESS_TARGETS[harness][0] for harness in harnesses)
    progress = CargoBuildProgress("test codegen", {
        (HARNESS_TARGETS[harness][2], HARNESS_TARGETS[harness][1]) for harness in harnesses
    }, test_profile=True)
    started = time.monotonic()
    artifacts: dict[str, set[str]] = {harness: set() for harness in harnesses}
    records: dict[str, dict[str, object]] = {}
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds,
                          umask=0o077) as child, progress.heartbeat():
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            progress.observe(line)
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
    progress.report(f"Cargo exited {code}")
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
    """Own temporary copies while retaining the published native CLI for operators."""

    def __init__(self, output: Path, copied: dict[str, str],
                 identities: dict[Path, tuple[int, ...]], observations: list[dict[str, object]],
                 *, retain_operator_cli: bool = True):
        super().__init__(copied)
        self.output = output
        self.observations = tuple(observations)
        self.pending = {row["selection"]: (identities[Path(row["path"])], row)
                        for row in observations
                        if not (retain_operator_cli and row["selection"] == "iroha"
                                and row["cargo_artifact"]["profile"].get("test") is False)}
        self.directory_fd = (os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                             if self.pending else None)

    def __enter__(self):
        return self

    def release(self, selection: str) -> None:
        """Release one closed exact copy after its last consumer has exited."""
        if selection not in self.pending:
            return
        expected, observation = self.pending[selection]
        fd = self.directory_fd
        assert fd is not None

        def verify_identity():
            held, named = os.fstat(fd), self.output.lstat()
            if ((held.st_dev, held.st_ino) != (named.st_dev, named.st_ino)
                    or held.st_uid != os.geteuid() or not stat.S_ISDIR(named.st_mode)
                    or stat.S_IMODE(named.st_mode) != 0o500):
                raise CheckError("native copy directory changed before release")
            named = os.stat(selection, dir_fd=fd, follow_symlinks=False)
            actual = (named.st_dev, named.st_ino, named.st_size, named.st_mode,
                      named.st_uid, named.st_nlink, named.st_mtime_ns, named.st_ctime_ns)
            if actual != expected:
                raise CheckError("native copy changed before release: " + selection)

        verify_identity()
        # A failed network harness can leave a daemon alive. Never unlink its
        # executable, or guess that an unavailable OS observation means closed.
        if native_test_output_confirmed_closed(self.output / selection) is not True:
            print("[taira-check] retained native artifact copy: busy or unverified: "
                  + selection, file=sys.stderr, flush=True)
            return
        verify_identity()  # The pathname must still name our inode after the OS query.
        os.fchmod(fd, 0o700)
        try:
            os.unlink(selection, dir_fd=fd)
            os.fsync(fd)
        finally:
            os.fchmod(fd, 0o500)
        del self.pending[selection]
        print("[taira-check] released native artifact "
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
            message = "native artifact release failed; copies retained: " + "; ".join(failures)
            if exception is None:
                raise CheckError(message)
            print("[taira-check] " + message, file=sys.stderr, flush=True)
        return False


def discard_unpublished_native_artifacts(output: Path, directory_identity: tuple[int, int],
                                         published: dict[Path, tuple[int, ...]],
                                         records: dict[str, dict[str, object]]) -> None:
    """Discard verified earlier copies when a later copy fails; never adopt partial files."""
    if not published:
        return
    directory = None
    try:
        directory = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        held, named = os.fstat(directory), output.lstat()
        if ((held.st_dev, held.st_ino) != directory_identity
                or (named.st_dev, named.st_ino) != directory_identity
                or not stat.S_ISDIR(named.st_mode) or held.st_uid != os.geteuid()
                or stat.S_IMODE(held.st_mode) not in {0o700, 0o500}):
            raise CheckError("unpublished native copy directory changed before cleanup")
        os.fchmod(directory, 0o500)
        observations = [{"selection": path.name, "path": str(path),
                         "cargo_artifact": records[path.name]} for path in published]
        # No copy in a failed isolation batch was handed to an operator. Its
        # verified CLI copy is temporary too; incomplete/unknown files stay out.
        with NativeArtifactCopies(output, {}, published, observations, retain_operator_cli=False):
            pass
    except (CheckError, OSError, ValueError) as error:
        print("[taira-check] unpublished native artifact cleanup retained files: "
              + str(error), file=sys.stderr, flush=True)
    finally:
        if directory is not None:
            os.close(directory)


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


NATIVE_TEST_OUTPUT_MAX_RECORDS = 128
NATIVE_TEST_OUTPUT_MAX_LEDGER_BYTES = 256 * 1024
NATIVE_TEST_OUTPUT_SCHEMA = "taira.native-test-outputs.v1"


def native_test_output_identity(info: os.stat_result) -> list[int]:
    """Use metadata captured with the producer's existing content verification."""
    return [info.st_dev, info.st_ino, info.st_size, info.st_mode, info.st_uid,
            info.st_nlink, info.st_mtime_ns, info.st_ctime_ns]


def native_artifact_inspector_path() -> Path:
    """Use the platform inspector without consulting the caller's PATH."""
    return Path("/usr/sbin/lsof" if sys.platform == "darwin" else "/usr/bin/lsof")


def require_native_artifact_inspector() -> None:
    """Reject missing artifact-inspection prerequisites before compilation."""
    executable = native_artifact_inspector_path()
    if not executable.is_file():
        reason = "missing or not a regular file"
    elif not os.access(executable, os.X_OK):
        reason = "not executable"
    else:
        return
    raise CheckError(
        f"native artifact inspector {executable} is {reason}; install lsof with "
        "the platform package manager and ensure that path is executable before "
        "running qualification or focused checks"
    )


def native_test_output_confirmed_closed(path: Path) -> bool | None:
    """True means closed, false means busy, and None means inspection failed."""
    executable = native_artifact_inspector_path()
    if not executable.is_file():
        return None
    try:
        result = subprocess.run(
            [str(executable), "-nP", "-F", "p", "--", str(path)],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=5, check=False,
            env={"PATH": os.defpath, "LANG": "C", "LC_ALL": "C"},
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.stderr:
        return None
    if result.returncode == 1 and not result.stdout:
        return True
    if result.returncode == 0 and result.stdout:
        return False
    return None


def _native_test_output_directory(path: Path) -> int:
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("test-output directory is indirect")
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info, named = os.fstat(fd), path.lstat()
        if (info.st_uid != os.geteuid() or info.st_mode & 0o022
                or (info.st_dev, info.st_ino) != (named.st_dev, named.st_ino)):
            raise ValueError("test-output directory custody differs")
        return fd
    except BaseException:
        os.close(fd)
        raise


def _save_native_test_outputs(fd: int, state: dict) -> None:
    raw = (json.dumps(state, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if len(raw) > NATIVE_TEST_OUTPUT_MAX_LEDGER_BYTES:
        raise ValueError("test-output ledger exceeds its bound")
    name = ".pending-" + uuid.uuid4().hex
    output = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600,
                     dir_fd=fd)
    try:
        os.fchmod(output, 0o600)
        view = memoryview(raw)
        while view:
            written = os.write(output, view)
            if written <= 0:
                raise OSError("test-output ledger made no write progress")
            view = view[written:]
        os.fsync(output)
    finally:
        os.close(output)
    os.replace(name, "ledger.json", src_dir_fd=fd, dst_dir_fd=fd)
    os.fsync(fd)


def _valid_native_test_output(row, target: Path) -> bool:
    if not isinstance(row, dict) or set(row) != {"selection", "path", "identity", "quarantine"}:
        return False
    identity = row["identity"]
    path = Path(row["path"]) if isinstance(row["path"], str) else Path()
    selection = row["selection"]
    if not isinstance(selection, str) or selection not in HARNESS_TARGETS:
        return False
    name = HARNESS_TARGETS[selection][1]
    return (any(re.fullmatch(re.escape(prefix) + r"-[0-9a-f]{16}", path.name)
                for prefix in (name, name.replace("-", "_")))
            and path.parent == target / "debug/deps"
            and isinstance(identity, list) and len(identity) == 8
            and all(type(value) is int and value >= 0 for value in identity)
            and stat.S_ISREG(identity[3]) and identity[3] & stat.S_IXUSR
            and not identity[3] & 0o022 and identity[4] == os.geteuid()
            and identity[5] == 1 and 0 < identity[2] <= 4 * 1024**3
            and (row["quarantine"] is None or isinstance(row["quarantine"], str)
                 and re.fullmatch(r"retiring-[0-9a-f]{32}", row["quarantine"]) is not None))


def _load_native_test_outputs(fd: int, root: Path, target: Path) -> dict:
    empty = {"schema": NATIVE_TEST_OUTPUT_SCHEMA, "source": str(root), "target": str(target),
             "current": {}, "pending": []}
    try:
        source = os.open("ledger.json", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
    except FileNotFoundError:
        return empty
    try:
        info = os.fstat(source)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                or stat.S_IMODE(info.st_mode) != 0o600 or info.st_nlink != 1
                or info.st_size > NATIVE_TEST_OUTPUT_MAX_LEDGER_BYTES):
            raise ValueError("test-output ledger custody differs")
        raw = os.read(source, NATIVE_TEST_OUTPUT_MAX_LEDGER_BYTES + 1)
        if (len(raw) != info.st_size or native_test_output_identity(os.fstat(source)) != native_test_output_identity(info)
                or native_test_output_identity(os.stat("ledger.json", dir_fd=fd, follow_symlinks=False)) != native_test_output_identity(info)):
            raise ValueError("test-output ledger changed during read")
    finally:
        os.close(source)
    state = json.loads(raw)
    if raw != (json.dumps(state, sort_keys=True, separators=(",", ":")) + "\n").encode():
        raise ValueError("test-output ledger is not canonical")
    if (not isinstance(state, dict) or set(state) != set(empty)
            or any(state[key] != empty[key] for key in ("schema", "source", "target"))
            or not isinstance(state["current"], dict) or not isinstance(state["pending"], list)):
        raise ValueError("test-output ledger belongs to different inputs")
    rows = list(state["current"].values()) + state["pending"]
    if (len(rows) > NATIVE_TEST_OUTPUT_MAX_RECORDS or not all(_valid_native_test_output(row, target) for row in rows)
            or any(key != row["selection"] or row["quarantine"] is not None
                   for key, row in state["current"].items())):
        raise ValueError("test-output ledger contains invalid records")
    return state


def _retire_native_test_output_pending(directory: int, deps: int, control: Path,
                                      state: dict, protected: list[dict]) -> int:
    """Retry only durable pending ownership, including interrupted rename windows."""
    pending = state["pending"]
    retired = 0
    for row in tuple(pending):
        if any(row["path"] == item["path"] or row["identity"][:2] == item["identity"][:2]
               for item in protected):
            pending.remove(row)
            continue
        expected = row["identity"]
        name = row["quarantine"] or Path(row["path"]).name
        parent = directory if row["quarantine"] else deps
        path = control / name if row["quarantine"] else Path(row["path"])
        try:
            actual = native_test_output_identity(os.stat(name, dir_fd=parent, follow_symlinks=False))
        except FileNotFoundError:
            # A crash after publishing rename intent may leave the original.
            if row["quarantine"]:
                name, parent, path = Path(row["path"]).name, deps, Path(row["path"])
                try:
                    actual = native_test_output_identity(os.stat(name, dir_fd=deps, follow_symlinks=False))
                except FileNotFoundError:
                    pending.remove(row)
                    continue
                if actual != expected:
                    continue  # Never adopt a replacement at the original path.
                row["quarantine"] = None
                _save_native_test_outputs(directory, state)
            else:
                pending.remove(row)
                continue
        if actual != expected:
            # The durable intent names one exact inode moved into our private
            # directory. Rename may advance ctime before its refreshed record
            # reaches disk; all other coordinates must still match.
            if row["quarantine"] is None or actual[:-1] != expected[:-1]:
                continue
        closed = native_test_output_confirmed_closed(path)
        if closed is None:
            break  # One unavailable OS query must not become 128 timeout waits.
        if not closed:
            continue
        if actual != expected:
            row["identity"] = expected = actual
            _save_native_test_outputs(directory, state)
        if row["quarantine"] is None:
            quarantine = "retiring-" + uuid.uuid4().hex
            row["quarantine"] = quarantine
            _save_native_test_outputs(directory, state)  # A crash cannot turn a moved file into an orphan.
            # Recheck after the OS query; Cargo cannot write while its lock is held.
            if native_test_output_identity(os.stat(name, dir_fd=deps, follow_symlinks=False)) != expected:
                continue
            os.rename(name, quarantine, src_dir_fd=deps, dst_dir_fd=directory)
            os.fsync(deps)
            os.fsync(directory)
            renamed = native_test_output_identity(os.stat(quarantine, dir_fd=directory, follow_symlinks=False))
            # Rename may advance ctime, but must preserve every other coordinate.
            if renamed[:-1] != expected[:-1]:
                continue
            row["identity"] = expected = renamed
            _save_native_test_outputs(directory, state)
            name, path = quarantine, control / quarantine
            # The original name is now unavailable to racing old consumers.
            # If one opened before rename, retain its inode and retry later.
            closed = native_test_output_confirmed_closed(path)
            if closed is None:
                break
            if not closed:
                continue
        if native_test_output_identity(os.stat(name, dir_fd=directory, follow_symlinks=False)) != expected:
            continue
        os.unlink(name, dir_fd=directory)
        os.fsync(directory)
        pending.remove(row)
        retired += expected[2]
    _save_native_test_outputs(directory, state)
    return retired


def retire_superseded_native_test_outputs(root: Path, target: Path, current: dict[str, dict]) -> None:
    """Record verified final tests and retire closed predecessors under Cargo locks.

    Callers pass only successful Cargo test outputs, using the source identity
    already verified before copying. Failures retain files and never fail a build.
    """
    if not current:
        return
    control = target / ("taira-native-test-outputs-" + hashlib.sha256(os.fsencode(root)).hexdigest()[:20])
    directory = deps = None
    retired = 0
    try:
        if not all(_valid_native_test_output(row, target) and key == row["selection"]
                   and row["quarantine"] is None for key, row in current.items()):
            raise ValueError("current test-output ownership is invalid")
        control.mkdir(mode=0o700, exist_ok=True)
        directory = _native_test_output_directory(control)
        if stat.S_IMODE(os.fstat(directory).st_mode) != 0o700:
            raise ValueError("test-output ledger directory must remain private")
        deps = _native_test_output_directory(target / "debug/deps")
        previous = _load_native_test_outputs(directory, root, target)

        def admit(prior):
            next_current = dict(prior["current"])
            pending = list(prior["pending"])
            for selection, row in current.items():
                old = next_current.get(selection)
                if old is not None and old != row:
                    pending.append(old)
                next_current[selection] = row
            protected = list(next_current.values())
            candidates = []
            for row in pending:
                if any(row["path"] == item["path"] or row["identity"][:2] == item["identity"][:2]
                       for item in protected):
                    continue
                if row not in candidates:
                    candidates.append(row)
            return prior | {"current": next_current, "pending": candidates}

        state = admit(previous)
        if len(state["current"]) + len(state["pending"]) > NATIVE_TEST_OUTPUT_MAX_RECORDS:
            # A full ledger must still retry old pending work when readers close.
            # Protect this capture even though it has not been enrolled yet.
            protected = list(previous["current"].values()) + list(current.values())
            retired += _retire_native_test_output_pending(directory, deps, control, previous, protected)
            state = admit(previous)
        protected = list(state["current"].values())
        pending = state["pending"]
        if len(protected) + len(pending) > NATIVE_TEST_OUTPUT_MAX_RECORDS:
            raise ValueError("test-output ledger is full; old outputs retained")
        _save_native_test_outputs(directory, state)  # No deletion precedes durable ownership publication.
        retired += _retire_native_test_output_pending(directory, deps, control, state, protected)
        if retired:
            print(f"[taira-check] retired {retired} bytes of recorded superseded Cargo test executables", flush=True)
        if pending:
            print(f"[taira-check] retained {len(pending)} superseded test outputs: busy, changed, or unverified", flush=True)
    except (OSError, ValueError, TypeError, RecursionError) as error:
        print(f"[taira-check] test-output retirement skipped: {type(error).__name__}; remaining outputs retained", flush=True)
    finally:
        if deps is not None:
            os.close(deps)
        if directory is not None:
            os.close(directory)


def isolate_native_artifacts(root: Path, env: dict[str, str],
                             records: dict[str, dict[str, object]]) -> NativeArtifactCopies:
    """Execute copied artifacts, never mutable Cargo paths returned by an earlier build."""
    from release_artifact_contract import ReleaseArtifactError
    from taira_cargo_artifact import cargo_hash_path, cargo_open_relative
    target = Path(env["CARGO_TARGET_DIR"])
    output = directory_identity = None
    published = {}
    copies = None
    completed = False
    try:
        if not records or any(key not in HARNESS_TARGETS and key not in {"iroha3d", "iroha", "iroha3d-message-control"} for key in records):
            raise CheckError("native artifact isolation requires known nonempty selections")
        for directory in (root, target):
            info = directory.stat()
            if (not directory.is_absolute() or directory.resolve(strict=True) != directory
                    or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
                    or info.st_mode & 0o022):
                raise CheckError("native artifact source and target must be direct owner-held directories")
        paths = {}
        for selection, record in records.items():
            package = {"iroha3d": "irohad", "iroha": "iroha_cli",
                       "iroha3d-message-control": "irohad"}.get(selection)
            if package is None:
                package = HARNESS_TARGETS[selection][3][1]
            if record["manifest_path"] != str(native_package_root(root, package) / "Cargo.toml"):
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
                        or not 0 < info.st_size <= NATIVE_ARTIFACT_MAX_BYTES):
                    raise CheckError("native Cargo artifact must be a bounded owner-held executable")
                identities[selection] = cargo_hash_path(path, max_size=NATIVE_ARTIFACT_MAX_BYTES)
            # Reuse source identities already verified above. The ledger owns
            # only final tests, never shipping binaries or Cargo cache entries.
            retire_superseded_native_test_outputs(root, target, {
                selection: {"selection": selection, "path": str(paths[selection]),
                    "identity": [expected.device, expected.inode, expected.size,
                        stat.S_IFREG | expected.mode, os.geteuid(), expected.link_count,
                        expected.mtime_ns, expected.ctime_ns], "quarantine": None}
                for selection, expected in identities.items()
                if selection in HARNESS_TARGETS and records[selection]["profile"].get("test") is True
            })
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
            directory_stat = output.lstat()
            directory_identity = (directory_stat.st_dev, directory_stat.st_ino)
            copied, observations = {}, []
            for selection, path in paths.items():
                expected = identities[selection]
                destination = output / selection
                digest, size = hashlib.sha256(), 0
                with cargo_open_relative(target, str(path.relative_to(target)), expected=expected) as source:
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
        copies = NativeArtifactCopies(output, copied, published, observations)
        for observation in observations:
            print("[taira-check] isolated native artifact " + json.dumps(observation, sort_keys=True), flush=True)
        completed = True
        return copies
    except (OSError, ValueError, ReleaseArtifactError, subprocess.SubprocessError) as error:
        raise CheckError(f"native artifact isolation failed: {error}") from error
    finally:
        if not completed:
            if copies is not None and copies.directory_fd is not None:
                try:
                    os.close(copies.directory_fd)
                except OSError:
                    pass  # Preserve the publication error; cleanup still checks exact copy identities.
                finally:
                    copies.directory_fd = None
            if output is not None and directory_identity is not None:
                discard_unpublished_native_artifacts(output, directory_identity, published, records)


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


NATIVE_TEST_PROGRESS_INTERVAL_SECONDS = 30


@contextlib.contextmanager
def native_test_batch_progress(count: int):
    """Keep a quiet captured CLI batch visible without changing its result stream."""
    stopped = threading.Event()
    started = time.monotonic()

    def report_while_running():
        while not stopped.wait(NATIVE_TEST_PROGRESS_INTERVAL_SECONDS):
            print(f"[taira-check] CLI batch running ({count} tests; "
                  f"{time.monotonic() - started:.1f}s elapsed)", flush=True)

    reporter = threading.Thread(target=report_while_running, name="taira-test-progress", daemon=True)
    reporter.start()
    try:
        yield
    finally:
        stopped.set()
        reporter.join()


def native_test_batch_failures(names: tuple[str, ...], filtered_out: int,
                               result: subprocess.CompletedProcess[str]) -> list[str]:
    """Admit a closed libtest census; captured failure diagnostics are never results."""
    lines = result.stdout.splitlines()
    nonempty = [index for index, line in enumerate(lines) if line.strip()]
    failures = []
    summary = None
    last = nonempty[-1] if nonempty else len(lines)
    if nonempty:
        summary = re.fullmatch(
            r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
            r"(\d+) measured; (\d+) filtered out; finished in \d+(?:\.\d+)?s", lines[last])
    if summary is None:
        failures.append("CLI batch has no canonical final libtest summary")
        last = len(lines)
    first = nonempty[0] if nonempty else len(lines)
    expected_header = f"running {len(names)} {'test' if len(names) == 1 else 'tests'}"
    if first == len(lines) or lines[first] != expected_header:
        failures.append(f"CLI batch did not declare its exact selected census ({len(names)} tests)")
    observed = {}
    totals = {"ok": 0, "FAILED": 0, "ignored": 0, "measured": 0}
    diagnostics = False
    selected = set(names)
    for line in lines[first + 1:last]:
        if not line.strip():
            continue
        if line == "failures:":
            diagnostics = True
        if diagnostics:
            continue
        match = re.fullmatch(r"test (\S+) \.\.\. (ok|FAILED|ignored(?:, .*)?|bench: .*)", line)
        if match is None:
            failures.append(f"CLI batch has malformed result output: {line}")
            continue
        name, status = match.groups()
        status = ("ignored" if status.startswith("ignored") else
                  "measured" if status.startswith("bench:") else status)
        totals[status] += 1
        if name not in selected:
            failures.append(f"CLI batch executed an unexpected test: {name}")
        if name in observed:
            failures.append(f"CLI batch has duplicate result: {name}")
        observed[name] = status
    for name in names:
        status = observed.get(name)
        if status is None:
            failures.append(f"regression did not execute to a terminal result: {name}")
        elif status != "ok":
            failures.append(f"regression did not pass: {name} ({status})")
    if summary is not None:
        status, *counts = summary.groups()
        expected = (totals["ok"], totals["FAILED"], totals["ignored"], totals["measured"], filtered_out)
        if tuple(map(int, counts)) != expected or sum(expected[:4]) != len(names):
            failures.append("CLI batch summary counts differ from the exact named result census")
        if status != ("FAILED" if totals["FAILED"] else "ok"):
            failures.append("CLI batch summary status contradicts its named results")
    if diagnostics and not totals["FAILED"]:
        failures.append("CLI batch has a failure diagnostic section without a failed result")
    if result.returncode != 0 and not (result.returncode == 101 and totals["FAILED"]):
        failures.append(f"CLI batch exited abnormally (exit {result.returncode})")
    elif result.returncode == 0 and totals["FAILED"]:
        failures.append("CLI batch returned success despite failed named results")
    return failures


def run_native_test_batch(harness: str, fixture_root: Path, env: dict[str, str], stages,
                          lock_fds: tuple[int, ...], names: tuple[str, ...], listing: str) -> None:
    available = {line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")}
    started = time.monotonic()
    print(f"[taira-check] start CLI batch ({len(names)} exact tests; one serial process)", flush=True)
    command = [harness, *names, "--exact", "--test-threads=1", "--format", "pretty", "--color", "never"]
    with native_test_batch_progress(len(names)):
        result = subprocess.run(command, cwd=fixture_root, env=env, stdin=subprocess.DEVNULL,
                                text=True, capture_output=True, check=False, pass_fds=lock_fds, umask=0o077)
    failures = native_test_batch_failures(names, len(available) - len(names), result)
    if failures:
        # Preserve every fixture diagnostic once; never replay successful tests after a partial batch.
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)
        print(f"[taira-check] failed CLI batch ({time.monotonic() - started:.1f}s)", flush=True)
        raise SelectedRegressionFailures(failures)
    for label, selected in stages:
        print(f"[taira-check] passed {label} ({len(selected)} tests in CLI batch)", flush=True)
    print(f"[taira-check] passed CLI batch ({len(names)} tests; {time.monotonic() - started:.1f}s)", flush=True)


def run_stages(harness: str, fixture_root: Path, env: dict[str, str], stages,
               lock_fds: tuple[int, ...], *, batch: bool = False) -> None:
    names = tuple(name for _, selected in stages for name in selected) if batch else ()
    if batch and (not names or len(set(names)) != len(names)):
        raise CheckError("CLI batch selection must be nonempty and contain unique exact test names")
    listing = subprocess.run([harness, "--list", "--format", "terse"], cwd=fixture_root,
                             env=env, stdin=subprocess.DEVNULL, text=True, capture_output=True, check=False,
                             pass_fds=lock_fds, umask=0o077)
    if listing.returncode:
        raise CheckError(f"cannot list native harness tests (exit {listing.returncode})")
    require_tests(listing.stdout, stages)
    if batch:
        run_native_test_batch(harness, fixture_root, env, stages, lock_fds, names, listing.stdout)
        return
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
                                    text=True, capture_output=True, check=False, pass_fds=lock_fds, umask=0o077)
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


def compile_network_binaries(root: Path, env: dict[str, str], lock_fds: tuple[int, ...],
                             *, message_control: bool = False) -> NativeArtifactCopies:
    """Copy each build before a separate fixture feature graph can replace Cargo outputs."""
    expected = {"iroha3d": ("iroha3d-message-control", "irohad")} if message_control else {
        "iroha3d": ("iroha3d", "irohad"), "iroha": ("iroha", "iroha_cli"),
        "iroha3d_taira": ("taira-launcher", "irohad")}
    if not message_control:
        for selection in shipping_harnesses(root):
            _, name, _, arguments = HARNESS_TARGETS[selection]
            expected[name] = ("iroha" if name == "iroha" else selection, arguments[1])
    packages = tuple(dict.fromkeys(package for _, package in expected.values()))
    command = [env["CARGO"], "--config", str(root / ".cargo/config.toml"), "build",
               "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
               *(argument for package in packages for argument in ("-p", package)),
               *(argument for name in expected for argument in ("--bin", name)),
               *(["--features", "irohad/test-network-message-control"] if message_control else []),
               "--message-format=json-render-diagnostics"]
    phase = "message-control fixture codegen" if message_control else "shipping codegen"
    print(f"[taira-check] build native network binaries: {phase}", flush=True)
    progress = CargoBuildProgress(phase, {("bin", name) for name in expected},
                                  test_profile=False)
    started = time.monotonic()
    artifacts: dict[str, str] = {}
    records: dict[str, dict[str, object]] = {}
    with subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace", pass_fds=lock_fds,
                          umask=0o077) as child, progress.heartbeat():
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            progress.observe(line)
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
    progress.report(f"Cargo exited {code}")
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


def split_network_stages(stages: tuple) -> tuple[tuple, tuple]:
    """Keep exact focused observation subsets independent of shipping binaries."""
    observation_names = {name for _, names in NETWORK_OBSERVATION_STAGES for name in names}
    observations, runtime = [], []
    for label, names in stages:
        selected_observations = tuple(name for name in names if name in observation_names)
        selected_runtime = tuple(name for name in names if name not in observation_names)
        if selected_observations:
            observations.append((label, selected_observations))
        if selected_runtime:
            runtime.append((label, selected_runtime))
    return tuple(observations), tuple(runtime)


def run_network_checks(root: Path, fixture_root: Path, env: dict[str, str], lock_fds: tuple[int, ...],
                       *, harness: str, stages: tuple) -> None:
    observations, runtime = split_network_stages(stages)
    # These contracts use the completed test harness alone. Run every selected
    # observation, including a focused subset of a group, before compiling any
    # shipping executable or creating a four-peer workspace.
    if observations:
        run_stages(harness, fixture_root, env, observations, lock_fds)
    if not runtime:
        return
    with compile_network_binaries(root, env, lock_fds) as binaries:
        require_network_fixture_capacity(fixture_root)
        # Keep attempt-owned fixtures and logs for diagnosis; they contain no live inputs.
        directory = Path(tempfile.mkdtemp(prefix="taira-consensus-check-", dir=fixture_root))
        network_env = env | {
            "TEST_NETWORK_BIN_IROHAD": binaries["iroha3d"],
            "TEST_NETWORK_BIN_IROHAD_TAIRA": binaries["taira-launcher"],
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
        for stage in runtime:
            if BEACON_NETWORK_TEST in stage[1]:
                if "kagami" not in binaries:
                    raise CheckError("beacon fixture requires the isolated shipping Kagami artifact")
                private_fixture_root = beacon_fixture_root()
                with compile_network_binaries(root, env, lock_fds, message_control=True) as control:
                    beacon_env = network_env | {
                        "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL": control["iroha3d-message-control"],
                        "TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(private_fixture_root),
                        "KAGAMI_BIN": binaries["kagami"],
                    }
                    run_stages(harness, fixture_root, beacon_env, (stage,), lock_fds)
            else:
                run_stages(harness, fixture_root, network_env, (stage,), lock_fds)


def beacon_fixture_root() -> Path:
    """Keep disposable generated signing material outside Git and public build artifacts."""
    configured = os.environ.get("TAIRA_TESTNET_BEACON_FIXTURE_DIR")
    root = Path(configured) if configured else Path.home() / ".taira-native-beacon-fixtures"
    if not root.is_absolute() or root.resolve() != root:
        raise CheckError("beacon fixture root must be an absolute direct path outside Git")
    # Match the native fixture's ancestor custody before creating any leaf.
    # A private directory below shared /tmp still fails native admission.
    for ancestor in root.parents:
        info = ancestor.lstat()
        if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_mode & 0o022:
            raise CheckError(f"beacon fixture root ancestor must be a direct directory without group or world write permission: {ancestor}")
        if (ancestor / ".git").exists():
            raise CheckError(f"beacon fixture root must be outside a Git repository: {ancestor}")
    root.mkdir(mode=0o700, exist_ok=True)
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise CheckError("beacon fixture root must be an owner-only 0700 directory")
    if (root / ".git").exists():
        raise CheckError(f"beacon fixture root must be outside a Git repository: {root}")
    result = subprocess.run(["git", "-C", str(root), "rev-parse", "--is-inside-work-tree"],
                            stdin=subprocess.DEVNULL, text=True, capture_output=True, check=False)
    if result.returncode == 0 or (result.returncode != 128 or "not a git repository" not in result.stderr):
        raise CheckError("beacon fixture root must be outside a Git repository")
    return root


def require_network_fixture_prerequisites(directory: Path, stages: tuple) -> None:
    """Admit only selected peer workloads before source checks and compilation."""
    _, runtime = split_network_stages(stages)
    if any(BEACON_NETWORK_TEST in names for _, names in runtime):
        beacon_fixture_root()
    if runtime:
        require_network_fixture_capacity(directory)


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


def validate_mv_test_registration(root: Path) -> None:
    """Reject stale registered MV names before Cargo; native listing stays authoritative.

    This is a bounded lexical guard for explicit, flat test modules, not a
    Rust parser or a claim that the selected subset exhausts each module.
    The existing pure lexer runs from the same captured source as this gate.
    """
    owners = (
        ("publication::nonblocking_tests::", "publication.rs", "publication_nonblocking_tests.rs", "nonblocking_tests"),
        ("allocation::tests::", "allocation.rs", "allocation_tests.rs", "tests"),
        ("release_tests::", "lib.rs", "release_tests.rs", "release_tests"),
        ("capture_tests::", "lib.rs", "capture_tests.rs", "capture_tests"),
        ("cell::charged_allocation_tests::", "cell.rs", "cell/charged_allocation_tests.rs", "charged_allocation_tests"),
        ("cell::publication_tests::", "cell.rs", "cell/publication_tests.rs", "publication_tests"),
        ("cell::aggregate_acquisition_tests::", "cell.rs", "cell/aggregate_acquisition_tests.rs", "aggregate_acquisition_tests"),
        ("cell::fresh_pair_acquisition_tests::", "cell.rs", "cell/fresh_pair_acquisition_tests.rs", "fresh_pair_acquisition_tests"),
        ("storage::publication_tests::", "storage.rs", "storage/publication_tests.rs", "publication_tests"),
        ("storage::aggregate_acquisition_tests::", "storage.rs", "storage/aggregate_acquisition_tests.rs", "aggregate_acquisition_tests"),
        ("storage::detached_tests::", "storage.rs", "storage/detached_tests.rs", "detached_tests"),
        ("storage::admitted_tests::", "storage.rs", "storage/admitted_tests.rs", "admitted_tests"),
        ("storage::admitted_tests::fresh_pair_acquisition::", "storage/admitted_tests.rs", "storage/fresh_pair_acquisition_tests.rs", "fresh_pair_acquisition"),
        ("storage::admitted_tests::fresh_pair_acquisition::scoped_acquisition::", "storage/fresh_pair_acquisition_tests.rs", "storage/scoped_acquisition_tests.rs", "scoped_acquisition"),
        ("storage::touches::tests::", "storage/touches.rs", "storage/touches_tests.rs", "tests"),
    )
    try:
        helper = root / "scripts/formal/sumeragi_v2_rust_text.py"
        namespace = {"__name__": "taira_mv_source_text", "__file__": str(helper)}
        exec(compile(helper.read_bytes(), str(helper), "exec"), namespace)
        mask = namespace["mask_rust_comments"]
        package = root / "crates/mv/src"
        sources = {}
        def source(relative):
            if relative not in sources:
                text = (package / relative).read_text()
                sources[relative] = (text, mask(text))
            return sources[relative]
        def edge(parent, child, name):
            text, masked = source(parent)
            pattern = r'^#\[path = "' + re.escape(child) + r'"\]\s*\nmod ' + re.escape(name) + r';'
            matches = [match for match in re.finditer(pattern, text, re.MULTILINE)
                       if masked[match.start():match.start() + 2] == "#["]
            declarations = list(re.finditer(r'^mod ' + re.escape(name) + r';$', masked, re.MULTILINE))
            if not matches and child == name + ".rs":
                # Rust's default sibling path is exact too. Inspect the complete
                # attribute prefix so a foreign #[path] cannot masquerade as it.
                for declaration in declarations:
                    prefix = masked[:declaration.start()]
                    boundary = max(prefix.rfind(";"), prefix.rfind("}")) + 1
                    if (prefix.count("{") == prefix.count("}")
                            and prefix[boundary:].strip() == "#[cfg(test)]"):
                        matches.append(declaration)
            if len(matches) != 1 or len(declarations) != 1:
                raise ValueError(f"registered MV module edge differs: {parent} -> {child}")
        lib = source("lib.rs")[1]
        for module in ("allocation", "cell", "storage", "publication"):
            if len(re.findall(r'^(?:pub )?mod ' + module + r';$', lib, re.MULTILINE)) != 1:
                raise ValueError(f"registered MV crate module differs: {module}")
        edge("storage.rs", "storage/touches.rs", "touches")
        available = []
        for prefix, parent, child, module in owners:
            relative = str(Path(child).relative_to(Path(parent).parent))
            edge(parent, relative, module)
            masked = source(child)[1]
            for match in re.finditer(r'^#\[test\]\s*\nfn (\w+)\s*\(', masked, re.MULTILINE):
                before = masked[:match.start()]
                if before.count("{") == before.count("}"):
                    available.append(prefix + match.group(1))
        selected = [name for _, names in MV_OWNERSHIP_STAGES for name in names]
        missing = [name for name in selected if available.count(name) != 1]
        if len(selected) != len(set(selected)) or missing:
            raise ValueError("registered MV test lacks one actual source definition: " + ", ".join(missing))
    except (OSError, UnicodeError, KeyError, TypeError, ValueError) as error:
        raise CheckError("MV test source registration failed: " + str(error)) from error


def validate_torii_lifecycle_test_registration(root: Path) -> None:
    """Bind lifecycle selectors to the explicit grouped Cargo target before build."""
    try:
        package = root / "crates/iroha_torii"
        manifest = tomllib.loads((package / "Cargo.toml").read_text())
        _, target, kind, arguments = HARNESS_TARGETS["torii-lifecycle"]
        if (manifest["package"]["name"] != "iroha_torii" or kind != "test"
                or arguments != ["-p", "iroha_torii", "--test", target]):
            raise ValueError("lifecycle target selection differs")
        rows = [row for row in manifest.get("test", []) if row.get("name") == target]
        if len(rows) != 1:
            raise ValueError("lifecycle test target must be explicitly registered once")
        features = manifest.get("features", {})
        active, pending = set(), ["default"]
        while pending:
            feature = pending.pop()
            if feature not in active:
                active.add(feature)
                pending.extend(value for value in features.get(feature, []) if value in features)
        if not set(rows[0].get("required-features", [])).issubset(active):
            raise ValueError("lifecycle target requires non-default features")
        grouped = package / rows[0]["path"]
        if not grouped.resolve().is_relative_to(package.resolve()):
            raise ValueError("lifecycle target path leaves package")
        modules = re.findall(r'^#\[path = "([^"\n]+)"\]\s*\nmod nexus_lifecycle_endpoint;',
                             grouped.read_text(), re.MULTILINE)
        if len(modules) != 1:
            raise ValueError("grouped lifecycle module must be registered once")
        source = grouped.parent / modules[0]
        if not source.resolve().is_relative_to(package.resolve()):
            raise ValueError("lifecycle module path leaves package")
        functions = re.findall(r'^#\[tokio::test\]\s*\nasync fn (\w+)\(', source.read_text(), re.MULTILINE)
        available = {"nexus_lifecycle_endpoint::" + name for name in functions}
        selected = [name for _, names in TORII_LIFECYCLE_STAGES for name in names]
        if (not selected or len(selected) != len(set(selected))
                or any(name not in available for name in selected)):
            raise ValueError("lifecycle selector lacks its registered module prefix or test")
    except (OSError, KeyError, TypeError, ValueError) as error:
        raise CheckError("Torii lifecycle test source registration failed: " + str(error)) from error


def run_lifecycle_source_checks(root: Path, env: dict[str, str], lock_fds: tuple[int, ...]) -> None:
    """Reject invalid source assets, then run shared contracts before Cargo."""
    validate_mv_test_registration(root)
    validate_torii_lifecycle_test_registration(root)
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


def native_linker_rustc_arguments(env: dict[str, str]) -> list[str]:
    """Forward only the coordinated native linker pair to direct rustc owners."""
    encoded = env.get("CARGO_ENCODED_RUSTFLAGS")
    plain = env.get("RUSTFLAGS")
    if encoded is None and plain is None:
        return []
    if encoded is not None and plain is not None:
        raise CheckError("native linker flags must have one coordinated encoding")
    arguments = encoded.split("\x1f") if encoded is not None else plain.split()
    prefixes = ("-Clinker=", "-Clink-arg=-fuse-ld=")
    if len(arguments) != len(prefixes):
        raise CheckError("standalone checks accept only the coordinated native linker pair")
    for argument, prefix in zip(arguments, prefixes):
        path = argument.removeprefix(prefix)
        if (not argument.startswith(prefix) or not Path(path).is_absolute()
                or os.path.abspath(path) != path or any(char in path for char in "\0\r\n\x1f")):
            raise CheckError("standalone checks require exact absolute native linker paths")
    return arguments


def _run_standalone_checks(root: Path, env: dict[str, str], lock_fds: tuple[int, ...], *,
                           source: str, output_name: str, label: str, description: str) -> None:
    compiler = env.get("RUSTC")
    if not compiler or not Path(compiler).is_absolute():
        raise CheckError(f"{label} checks require the coordinated pinned RUSTC")
    linker_arguments = native_linker_rustc_arguments(env)
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
                  capture_output=True, check=False, pass_fds=lock_fds, timeout=120, umask=0o077)
    compiled = subprocess.run([compiler, *linker_arguments, "--edition=2024", "--test",
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


def native_harness_plan(scoped_stages, shipping: tuple[str, ...]):
    """Keep one full native compile graph for qualification and focused diagnostics."""
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
    return early_stages, selections, compile_only


def focused_regression_stages(qualification_scope: str, requested):
    """Resolve explicit exact test names without broad patterns or implicit skips."""
    scoped = qualification_stages(qualification_scope)
    if not requested or isinstance(requested, str):
        raise CheckError("prequalification requires one or more HARNESS=EXACT_TEST selections")
    selected = set()
    for item in requested:
        if not isinstance(item, str) or item.count("=") != 1:
            raise CheckError("focused regression must be HARNESS=EXACT_TEST")
        harness, test = item.split("=", 1)
        names = {name for _, names in scoped.get(harness, ()) for name in names}
        if not harness or not test or test not in names:
            raise CheckError("focused regression is not selected in this scope: " + item)
        if (harness, test) in selected:
            raise CheckError("duplicate focused regression: " + item)
        selected.add((harness, test))
    return {harness: tuple((label, tuple(name for name in names if (harness, name) in selected))
                           for label, names in stages
                           if any((harness, name) in selected for name in names))
            for harness, stages in scoped.items()
            if any(key == harness for key, _ in selected)}


def run_prequalification(root: Path, *, focused_regressions, qualification_scope: str = "basic",
                         environment: dict[str, str], lock_fds: tuple[int, ...]) -> None:
    """Run exact portable controls before configuration and heavier diagnostics.

    Each phase retains its own Cargo feature graph and artifact observations in
    the same coordinated lane. Configuration is mandatory for overall success;
    immutable qualification still compiles and executes its complete graph.
    Called only from the coordinated mutable development lane. This function has
    no signed-source, qualification checkpoint, or release-result interface.
    """
    focused = focused_regression_stages(qualification_scope, focused_regressions)
    scoped = qualification_stages(qualification_scope)
    if sys.platform not in {"darwin", "linux"}:
        raise CheckError("native prequalification requires macOS or Linux")
    if not all(environment.get(name) for name in ("CARGO", "CARGO_HOME", "CARGO_TARGET_DIR")):
        raise CheckError("prequalification requires the coordinated development Cargo environment")
    require_native_artifact_inspector()
    fixture_root = Path(environment["CARGO_TARGET_DIR"])
    require_network_fixture_prerequisites(fixture_root, focused.get("network", ()))
    env = dict(environment)
    head = subprocess.check_output(
        ["git", "--no-replace-objects", "rev-parse", "HEAD"], cwd=root, env=env,
        stdin=subprocess.DEVNULL, text=True).strip()
    env.pop("CARGO_BUILD_TARGET", None)
    env.update(VERGEN_GIT_SHA=head, IROHA_GIT_COMMIT_HASH=head)
    print(f"[taira-prequalify] mutable source {head}; {root}; diagnostic only", flush=True)
    run_pure_fsm_checks(root, env, lock_fds)
    run_lifecycle_source_checks(root, env, lock_fds)
    shipping = shipping_harnesses(root)
    _, complete_selections, _ = native_harness_plan(scoped, shipping)
    if not set(focused).issubset(complete_selections):
        raise CheckError("focused regression lacks its selected native compile target")
    # This mutable diagnostic cannot publish qualification evidence. Request only
    # configuration and focused harnesses; Cargo can add implicit targets within
    # their shared package graph. Prepare owns the complete qualification graph.
    selections = tuple(name for name in complete_selections
                       if name == "config" or name in focused)
    selected_harness_count = len(selections)
    portable = tuple(name for name in selections if name in MV_OWNERSHIP_HARNESSES)
    selections = tuple(name for name in selections if name not in portable)
    if portable:
        print("[taira-prequalify] portable diagnostic Cargo graph: " + ", ".join(portable)
              + "; mandatory configuration and remaining targets follow; NOT qualification", flush=True)
        started = time.monotonic()
        check_test_harnesses(root, env, harnesses=portable, lock_fds=lock_fds)
        with compile_test_harnesses(root, env, harnesses=portable, lock_fds=lock_fds) as copies:
            failures = []
            for name in portable:
                try:
                    run_stages(copies[name], fixture_root, env, focused[name], lock_fds)
                except SelectedRegressionFailures as error:
                    failures.extend(error.failures)
                copies.release(name)
            if failures:
                raise SelectedRegressionFailures(failures)
        # Finish all consumers and close their artifact context before another
        # Cargo graph can replace original outputs. This pass earns no checkpoint.
        portable_count = sum(len(tests) for name in portable for _, tests in focused[name])
        print(f"[taira-prequalify] portable diagnostic passed: {portable_count} exact tests "
              f"in {time.monotonic() - started:.1f}s; mandatory configuration and "
              "remaining diagnostics pending", flush=True)
    print("[taira-prequalify] remaining diagnostic Cargo graph: " + ", ".join(selections)
          + "; execute mandatory configuration before nonportable regressions", flush=True)
    check_test_harnesses(root, env, harnesses=selections, lock_fds=lock_fds)
    with compile_test_harnesses(root, env, harnesses=selections, lock_fds=lock_fds) as harnesses:
        # Configuration remains mandatory for final success and gates the
        # nonportable phase. Explicit focused config tests must not run twice.
        run_config_checks(harnesses, fixture_root, env, lock_fds)
        for name in selections:
            if name != "config" and name not in focused:
                harnesses.release(name)
        pending_kura_names = {test for _, tests in CORE_PENDING_KURA_RECOVERY_STAGES
                              for test in tests}
        core_stages = focused.get("core", ())
        pending_kura = tuple((label, tuple(test for test in tests if test in pending_kura_names))
                             for label, tests in core_stages
                             if any(test in pending_kura_names for test in tests))
        remaining_core = tuple((label, tuple(test for test in tests if test not in pending_kura_names))
                               for label, tests in core_stages
                               if any(test not in pending_kura_names for test in tests))
        # Retain the same Core copy for its remaining tests. A partial focus must
        # neither expand to all recovery tests nor bury their failures in later work.
        if pending_kura:
            run_stages(harnesses["core"], fixture_root, env, pending_kura, lock_fds)
        failures = []
        for name in selections:
            if name in {"config", "network"} or name not in focused:
                continue
            stages = remaining_core if name == "core" else focused[name]
            try:
                if name == "cli":
                    run_stages(harnesses[name], fixture_root, env, stages, lock_fds, batch=True)
                elif stages:
                    run_stages(harnesses[name], fixture_root, env, stages, lock_fds)
            except SelectedRegressionFailures as error:
                failures.extend(error.failures)
            harnesses.release(name)
        if failures:
            raise SelectedRegressionFailures(failures)
        if "network" in focused:
            run_network_checks(root, fixture_root, env, lock_fds,
                               harness=harnesses["network"], stages=focused["network"])
            harnesses.release("network")
    if subprocess.check_output(["git", "--no-replace-objects", "rev-parse", "HEAD"],
                               cwd=root, env=env, stdin=subprocess.DEVNULL, text=True).strip() != head:
        raise CheckError("HEAD changed during prequalification; rerun the focused diagnostic")
    requested_count = sum(len(names) for stages in focused.values() for _, names in stages)
    print(f"[taira-prequalify] diagnostic passed: {selected_harness_count} selected harnesses compiled; "
          f"{requested_count} focused regressions and mandatory configuration passed. "
          "NOT release qualification; immutable prepare still runs its complete gate.", flush=True)


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
    require_native_artifact_inspector()
    fixture_root = Path(environment["CARGO_TARGET_DIR"]) if source_commit is not None else root
    require_network_fixture_prerequisites(fixture_root, scoped_stages["network"])
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
    # Run startup recovery first, then CLI contracts, so mandatory restart
    # failures surface before unrelated groups without changing the Cargo graph.
    # After mandatory startup controls, check the production feature graph before
    # long tests. Run every independent immutable test copy before shipping
    # codegen or the four-peer fixture. Aggregate test failures;
    # missing tests, artifact custody failures and other infrastructure errors
    # still stop immediately. Production binaries use a separate graph below.
    # Keep the full compile graph and its warm Cargo feature union in both
    # scopes. Deferred cases have compile coverage, never fabricated test passes.
    early_stages, selections, compile_only = native_harness_plan(scoped_stages, shipping)
    if selections:
        # Expand macros and type-check the exact test graph before expensive codegen.
        # Keep the same feature union, environment, target lane and held locks; a
        # metadata pass neither publishes test executables nor qualifies a regression.
        check_test_harnesses(root, env, harnesses=selections, lock_fds=lock_fds)
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
            # execute before CLI and long consensus/proof groups. Retain each immutable copy
            # until its remaining stages finish; no test runs twice or gains a skip flag.
            startup = {"core": CORE_STARTUP_STAGES, "daemon": DAEMON_STARTUP_STAGES, "torii-unit": TORII_STARTUP_STAGES}
            pending_kura = tuple(stage for stage in scoped_stages["core"]
                                 if stage in CORE_PENDING_KURA_RECOVERY_STAGES)
            preflight = tuple((name, tuple(stage for stage in stages
                                          if stage in startup.get(name, ())
                                          and not (name == "core" and stage in pending_kura)))
                              for name, stages in early_stages)
            if not reuse_independent:
                # These exact immutable copies belong to the same complete Cargo
                # graph and checkpoint. Refuse before Core runtime work; this
                # does not claim to run before Core harness compilation.
                for name in MV_OWNERSHIP_HARNESSES:
                    if scoped_stages[name]:
                        run_stages(harnesses[name], fixture_root, env,
                                   scoped_stages[name], lock_fds)
                # Actual post-Kura recovery is a prerequisite for every later
                # stage. Keep the shared Cargo graph and exact checkpoint census,
                # but do not bury a publication failure among other startup cases.
                if pending_kura:
                    run_stages(harnesses["core"], fixture_root, env, pending_kura, lock_fds)
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
                    raise SelectedRegressionFailures(startup_failures)
            # Test-harness dev dependencies can conceal production-only errors.
            # Always check the separate shipping graph, even when the exact
            # independent test pass is reused. This creates no checkpoint claim.
            if shipping:
                check_shipping_binaries(root, env, lock_fds)
            if STAGES:
                if not reuse_independent:
                    try:
                        run_stages(harnesses["cli"], fixture_root, env, STAGES, lock_fds, batch=True)
                    except SelectedRegressionFailures as error:
                        failures.extend(error.failures)
                harnesses.release("cli")
            for name, stages in early_stages:
                # Ownership stages already passed above (or share the exact
                # reused checkpoint). Release their copies here, without a second run.
                remaining = () if name in MV_OWNERSHIP_HARNESSES else tuple(
                    stage for stage in stages if stage not in startup.get(name, ()))
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
    # Lazy import keeps the low-level gate loadable from an authenticated source capture.
    import taira_release as release

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="repository root (default: this maintained script's parent repository)")
    parser.add_argument("--target-dir", type=Path, help="existing development Cargo lane (default: sibling routine lane)")
    parser.add_argument("--native-check-scope", choices=QUALIFICATION_SCOPES, default="basic",
                        help="basic application/startup checks (default), or full advanced regressions")
    parser.add_argument("--native-linker", choices=("system", "llvm"), default=release.default_development_linker(),
                        help="development only: LLVM 18 by default on Linux (clang-18/lld-18 required), system on macOS; explicit system selects the diagnostic fallback; changing selection rebuilds Cargo dependencies")
    parser.add_argument("--focus-regression", action="append", metavar="HARNESS=EXACT_TEST",
                        help="development diagnostic: run selected portable ownership targets first, then mandatory configuration and remaining explicit harnesses; not qualification")
    args = parser.parse_args()
    try:
        options = {"native_check_scope": args.native_check_scope, "native_linker": args.native_linker}
        if args.focus_regression is not None:
            options["focused_regressions"] = tuple(args.focus_regression)
        release.development_check(args.repo_root, args.target_dir, dict(os.environ), **options)
    except (CheckError, release.PrepareError, release.ReleaseArtifactError,
            OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"[taira-check] FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
