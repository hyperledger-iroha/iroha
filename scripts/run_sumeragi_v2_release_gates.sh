#!/usr/bin/env bash
# Execute the source-bound Sumeragi v2 PR or production-release corridor.

set -euo pipefail

profile="${1:---pr}"
if [[ $# -gt 1 ]] || [[ "$profile" != "--pr" && "$profile" != "--release" ]]; then
  echo "usage: $0 [--pr|--release]" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
# Every real-network leg in this parent shell must fail rather than translate a
# socket/sandbox denial into a successful developer skip.
export IROHA_TEST_REQUIRE_NETWORK=1
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
unset IROHA_TEST_SKIP_BUILD IROHA_TEST_ALLOW_REENTRANT_BUILD
unset IROHA_TEST_TARGET_DIR IROHA_TEST_BUILD_PROFILE IROHA_TEST_BUILD_TIMEOUT_MS PROFILE
unset TLAPM_BIN TLAPM_STDLIB TLA2TOOLS_JAR

# Both profiles are release evidence. Bind the complete corridor, including the
# PR matrix, to one checkout manifest and one content-addressed build root.
release_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest" >&2
  exit 1
fi
readonly release_source_manifest_sha256
readonly release_source_bound_root="${repo_root}/target/sumeragi-v2-release/${release_source_manifest_sha256}"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$release_source_manifest_sha256"
export CARGO_TARGET_DIR="${release_source_bound_root}/test-suite"
export IROHA_TEST_TARGET_DIR="${release_source_bound_root}/programs"
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
export IROHA_TEST_BUILD_TIMEOUT_MS=3600

# Inventory and execute the production adapter/runtime ownership boundary before
# the slower network and formal corridors. The pure reducer harness cannot
# exercise worker cancellation, queued completion rebinding, or watchdog
# classification, so these exact tests are source-bound release inputs.
required_production_liveness_tests=(
  sumeragi::v2_core::tests::prior_view_commit_votes_rebuild_the_exact_locked_round_quorum
  sumeragi::v2_core::tests::higher_tc_lock_prunes_superseded_commit_retransmission
  sumeragi::v2_core::tests::same_lock_tc_resigns_local_commit_and_rebuilds_quorum_without_self_delivery
  sumeragi::v2_core::tests::tc_highest_prepare_missed_locally_persists_historical_commit_after_validation
  sumeragi::v2_core::tests::historical_locked_commit_replays_only_after_exact_tc_lock_installation
  sumeragi::v2_core::tests::higher_conflicting_prepare_intent_fences_historical_commit_reconstruction
  sumeragi::v2_core::tests::higher_same_subject_prepare_allows_historical_commit_reconstruction
  sumeragi::v2_core::tests::replay_does_not_resign_commit_superseded_by_higher_tc_lock
  sumeragi::v2_core::tests::current_view_commit_waits_for_the_exact_durable_lock
  sumeragi::v2_core::tests::decision_retains_in_flight_body_pipeline_without_duplicate_fetch
  sumeragi::v2_core::refinement::tests::retransmit_may_reconstruct_one_final_decision_body_stage
  sumeragi::v2_core::reducer::source_link_tests::retransmit_body_stage_requires_an_exact_durable_decision_capability
  sumeragi::v2::tests::deferred_locked_commit_delivery_tracks_generation_after_tc
  sumeragi::v2::tests::prelock_current_commit_is_readmitted_after_exact_lock_persistence
  sumeragi::v2::tests::tc_promoted_historical_commit_is_fsynced_before_sign_and_status
  sumeragi::v2::tests::timeout_vote_installs_embedded_qc_before_forming_tc
  sumeragi::v2::tests::exact_local_completion_after_decision_reports_body_validated_progress
  sumeragi::v2::tests::busy_local_completion_during_decision_wal_reaches_apply_once
  sumeragi::v2::tests::enter_view_conversion_uses_effect_carried_lock_not_reducer_lock
  sumeragi::v2::tests::saturated_normal_lane_retains_exact_local_proposal_completion
  sumeragi::v2::tests::unsafe_proposal_admission_preserves_duplicate_and_equivocation_semantics
  sumeragi::v2::tests::admission_keeps_only_the_exact_locked_commit_vote_beyond_one_rotation
  sumeragi::v2::tests::deferred_service_cursor_cycles_nonempty_classes
  sumeragi::v2::tests::deferred_service_cursor_advances_across_busy_front_requeue
  sumeragi::v2::tests::unowned_busy_certificates_roll_back_staged_registry_and_active_subject
  sumeragi::v2::tests::unowned_busy_exact_locked_vote_rolls_back_and_remains_retryable
  sumeragi::v2::tests::protected_locked_vote_uses_reserved_capacity_without_evicting_certificate_ownership
  sumeragi::v2::tests::leader_without_owned_candidate_work_reports_missing_proposal_state
  sumeragi::v2::tests::authentication_rejects_valid_commitment_conflicts_without_mutating_adapter
  sumeragi::v2_apply::tests::committed_merge_reservation_rejects_bare_norito
  sumeragi::v2_effects::tests::retained_locked_body_survives_same_lock_view_churn_before_fetch_adopts_it
  sumeragi::v2_effects::tests::higher_different_lock_releases_retained_cache_before_replacement_staging
  sumeragi::v2_effects::tests::higher_round_same_subject_reuses_only_the_view_independent_locked_cache
  sumeragi::v2_effects::tests::queued_protected_store_keeps_one_work_id_across_repeated_tcs
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_the_exact_fetch_until_reconstruction_completes
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_certified_request_ownership_through_signed_response
  sumeragi::v2_effects::tests::tc_body_rebind_uses_the_effective_local_lock_when_the_tc_omits_or_lowers_it
  sumeragi::v2_effects::tests::enter_view_rejects_a_tc_high_without_an_effective_protected_body
  sumeragi::v2_effects::tests::tc_body_rebind_retags_a_queued_body_available_completion
  sumeragi::v2_effects::tests::tc_body_rebind_retires_a_superseded_completion_and_releases_capacity
  sumeragi::v2_effects::tests::serialized_runtime_rebinds_busy_deferred_body_completion_before_service
  sumeragi::v2_effects::tests::tc_body_rebind_cancels_fetch_superseded_by_a_higher_different_qc
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_reproposal_round_ownership_before_staging
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::first_lock_retires_unlocked_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::first_lock_retires_queued_store_validation_and_local_proposal_completions
  sumeragi::v2_effects::tests::lock_reconciliation_rejects_same_round_conflict_and_late_lower_lock
  sumeragi::v2_effects::tests::failed_lock_cleanup_keeps_exact_owner_and_requires_restart
  sumeragi::v2_effects::tests::lock_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::lock_cleanup_status_failure_preserves_committed_replacement
  sumeragi::v2_effects::tests::higher_round_same_subject_preserves_current_proposal_pipeline_with_same_tag
  sumeragi::v2_effects::tests::decision_installation_frees_losing_capacity_before_fetch
  sumeragi::v2_effects::tests::failed_decision_cleanup_keeps_losing_owner_and_requires_restart
  sumeragi::v2_effects::tests::decision_cleanup_fetch_failure_preserves_exact_local_pipeline_consumer
  sumeragi::v2_effects::tests::decision_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::decision_converts_queued_local_proposal_to_body_progress
  sumeragi::v2_effects::tests::decision_rebinds_exact_local_validation_to_reducer_progress
  sumeragi::v2_effects::tests::decision_preserves_current_tag_local_proposal_for_direct_apply
  sumeragi::v2_effects::tests::decision_commitment_mismatch_fails_closed_before_apply
  sumeragi::v2_effects::tests::stale_generation_local_completion_uses_durable_recovery
  sumeragi::v2_effects::tests::missing_merge_sidecar_retains_exact_validation_until_retry
  sumeragi::v2_effects::tests::decided_apply_retries_after_exact_merge_sidecar_recovery
  sumeragi::v2_effects::tests::production_certified_body_request_rejects_locally_conflicting_qc_without_fail_close
  sumeragi::v2_effects::tests::production_commit_certificate_response_conflict_keeps_discovery_outstanding_and_runtime_open
  sumeragi::v2_effects::tests::runtime_step_dispatches_entire_effect_batch_before_returning
  sumeragi::v2_effects::tests::failed_view_cleanup_keeps_stale_fetch_and_requires_restart
  sumeragi::v2_effects::tests::view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation
  sumeragi::v2_effects::tests::view_cleanup_second_cancellation_failure_commits_no_fetch_retirement
  sumeragi::v2_lane_work::tests::direct_decision_quiesces_losing_lane_and_retransmission_work
  sumeragi::v2_lane_work::tests::persisted_lane_session_uses_only_selected_qc_signer_pops
  sumeragi::v2_lane_work::tests::planner_view_one_binds_rotated_global_leader_to_fresh_lane_view
  sumeragi::v2_lane_work::tests::enabled_nexus_binds_independent_lane_author_distinct_from_global_leader
  sumeragi::v2_lane_work::tests::lane_work_stays_quiescent_until_the_exact_global_prepare_lock
  sumeragi::v2_lane_work::tests::global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject
  sumeragi::v2_lane_work::tests::superseded_commit_protected_lane_session_cannot_retransmit
  sumeragi::v2_lane_work::tests::same_body_binds_after_prepare_lock_advances_beyond_header_view
  sumeragi::v2_runtime::tests::retiring_exact_body_completion_releases_a_capacity_one_ingress_slot
  sumeragi::v2_runtime::tests::exact_authenticated_progress_retransmission_is_queue_coalesced
  sumeragi::v2_runtime::tests::completion_retries_coalesce_across_ingress_and_busy_deferred_ownership
  sumeragi::v2_runtime::tests::body_available_rebind_rejects_uninstalled_destination_without_mutation
  sumeragi::v2_runtime::tests::body_available_rebind_coalesces_exact_busy_deferred_destination_owner
  sumeragi::v2_runtime::tests::body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::unbound_direct_vote_authentication_is_recoverable_and_becomes_admissible_after_validation
  sumeragi::v2_runtime::tests::conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning
  sumeragi::v2_runtime::tests::conflicting_local_and_validated_receipts_do_not_coalesce
  sumeragi::v2_runtime::tests::production_busy_transfer_retains_exact_validation_evidence_for_retry_and_cleanup
  sumeragi::v2_runtime::tests::body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates
  sumeragi::v2_runtime::tests::decision_retires_proposal_owners_but_preserves_body_and_application_completions
  sumeragi::v2_runtime::tests::decision_retires_stale_local_completion_for_durable_recovery
  sumeragi::v2_runtime::tests::progress_cursor_decision_preserves_outer_ingress_completion_until_apply
  sumeragi::v2_runtime::tests::decision_cleanup_preserves_unique_busy_deferred_completion
  sumeragi::v2_runtime::tests::decision_commitment_mismatch_fails_closed_before_retirement
  sumeragi::v2_runner::tests::same_tag_higher_lock_retires_all_local_proposal_owners
  sumeragi::v2_runner::tests::first_same_subject_lock_preserves_pending_local_proposal_events
  sumeragi::v2_runner::tests::higher_same_subject_lock_keeps_one_local_proposal_owner
  sumeragi::v2_runner::tests::late_old_rejection_cannot_arm_heartbeat_for_replacement_lock
  sumeragi::v2_runner::tests::decision_retires_local_work_before_prepared_delivery
  sumeragi::v2_worker::tests::fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner
  sumeragi::v2_worker::tests::invalid_fetch_consumer_rebind_fails_closed_without_consuming_owner
  sumeragi::v2_worker::tests::locked_candidate_requests_coalesce_by_immutable_subject
  sumeragi::v2_worker::tests::locked_candidate_completion_uses_latest_consumer_without_reloading
  sumeragi::v2_worker::tests::locked_candidate_consumer_rebind_rejects_stale_or_regressive_tags
  sumeragi::v2_worker::tests::locked_candidate_duplicate_or_wrong_completion_is_rejected
  sumeragi::v2_worker::tests::higher_different_lock_replaces_load_and_retires_stale_completion
  sumeragi::v2_worker::tests::superseded_locked_candidate_failure_starts_latest_acquisition
  sumeragi::v2_worker::tests::unavailable_locked_candidate_waits_for_matching_durable_store
  sumeragi::v2_worker::tests::outbound_payload_retention_is_constant_across_many_view_changes
  sumeragi::v2_worker::tests::late_stale_proposal_signature_cannot_restore_pruned_outbound_payload
  sumeragi::v2_worker::tests::nonzero_view_proposal_intent_replays_through_production_services
  sumeragi::v2_worker::tests::decision_retires_candidate_and_outbound_work_but_keeps_exact_sidecar_deferral
  sumeragi::v2_worker::tests::worker_completion_is_retained_behind_a_full_runtime_fifo
  sumeragi::v2_worker::tests::production_drain_publishes_worker_completion_behind_full_runtime_fifo
  sumeragi::v2_worker::tests::successful_auxiliary_drain_republishes_cleared_completion_ownership
  sumeragi::v2_worker::tests::auxiliary_completion_drain_is_batch_bounded
  sumeragi::status::v2_liveness_watchdog_tests::blocker_classifier_has_stable_specific_precedence
  sumeragi::status::v2_liveness_watchdog_tests::locked_candidate_load_overlay_precedes_commit_quorum_diagnosis
  sumeragi::status::v2_liveness_watchdog_tests::aged_queue_without_service_debt_does_not_claim_scheduler_starvation
  sumeragi::status::v2_liveness_watchdog_tests::network_ingress_service_clock_distinguishes_stopped_and_active_scans
  sumeragi::status::v2_liveness_watchdog_tests::repeated_tc_reconstruction_of_same_locked_commit_pool_does_not_reset_height_clock
  sumeragi::status::v2_liveness_watchdog_tests::apply_waiting_on_merge_sidecar_is_application_pending_not_body_unavailable
  sumeragi::status::v2_liveness_watchdog_tests::successor_handoff_is_visible_and_completion_advances_height_progress
  sumeragi::status::v2_liveness_watchdog_tests::effect_completion_overlay_preserves_capacity_age_and_service_debt
  sumeragi::status::v2_liveness_watchdog_tests::live_effect_completion_observer_survives_stopped_runner_and_clears_stale_depth
)
readonly expected_production_liveness_test_count=124
if (( ${#required_production_liveness_tests[@]} != expected_production_liveness_test_count )); then
  echo "expected exactly ${expected_production_liveness_test_count} production Sumeragi v2 liveness tests, found ${#required_production_liveness_tests[@]}" >&2
  exit 1
fi
production_unit_list="$(cargo test --locked -p iroha_core --lib -- --list)"
production_ignored_unit_list="$(
  cargo test --locked -p iroha_core --lib -- --list --ignored
)"
for required_test in "${required_production_liveness_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_unit_list"; then
    echo "missing required production Sumeragi v2 liveness test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_ignored_unit_list"; then
    echo "required production Sumeragi v2 liveness test is ignored: ${required_test}" >&2
    exit 1
  fi
done
production_liveness_modules=(
  sumeragi::v2_core::tests
  sumeragi::v2_core::refinement::tests
  sumeragi::v2_core::reducer::source_link_tests
  sumeragi::v2::tests
  sumeragi::v2_apply::tests
  sumeragi::v2_effects::tests
  sumeragi::v2_lane_work::tests
  sumeragi::v2_runtime::tests
  sumeragi::v2_runner::tests
  sumeragi::v2_worker::tests
  sumeragi::status::v2_liveness_watchdog_tests
)
for module in "${production_liveness_modules[@]}"; do
  cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1
done

required_data_model_status_test="block::consensus_v2::tests::status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry"
data_model_unit_list="$(cargo test --locked -p iroha_data_model --lib -- --list)"
data_model_ignored_unit_list="$(
  cargo test --locked -p iroha_data_model --lib -- --list --ignored
)"
if ! grep -Fqx -- "${required_data_model_status_test}: test" <<<"$data_model_unit_list"; then
  echo "missing required Sumeragi v2 status-contract test: ${required_data_model_status_test}" >&2
  exit 1
fi
if grep -Fqx -- "${required_data_model_status_test}: test" <<<"$data_model_ignored_unit_list"; then
  echo "required Sumeragi v2 status-contract test is ignored: ${required_data_model_status_test}" >&2
  exit 1
fi
cargo test --locked -p iroha_data_model --lib "$required_data_model_status_test" -- --test-threads=1

# Pin the production-soak execution profile and its serialized evidence schema
# before any real network is started. Cargo's filter succeeds on zero tests, so
# require every exact non-ignored contract before executing it with `--exact`.
required_taira_release_contract_tests=(
  taira_public_localnet::release_execution_profile_accepts_only_the_exact_positive_profile
  taira_public_localnet::release_execution_profile_rejects_wrong_or_blank_build_profiles
  taira_public_localnet::release_execution_profile_rejects_cargo_profile_mismatch
  taira_public_localnet::release_execution_profile_rejects_non_exact_offline_values
  taira_public_localnet::simulation_summary_json_records_release_profile_and_status_evidence
)
taira_release_contract_target="consensus_and_da"
taira_release_contract_list="$(
  cargo test --locked -p integration_tests --test "$taira_release_contract_target" -- --list
)"
taira_release_ignored_contract_list="$(
  cargo test --locked -p integration_tests --test "$taira_release_contract_target" -- --list --ignored
)"
for required_test in "${required_taira_release_contract_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$taira_release_contract_list"; then
    echo "missing required Taira release-evidence contract test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$taira_release_ignored_contract_list"; then
    echo "required Taira release-evidence contract test is ignored: ${required_test}" >&2
    exit 1
  fi
  cargo test --locked -p integration_tests --test "$taira_release_contract_target" \
    "$required_test" -- --exact --test-threads=1
done

# Keep the canonical Rust wire authority and the maintained lightweight SDK
# parsers in the same source-bound corridor. These commands use only checked-in
# fixtures and in-memory responses; dependency installation is deliberately a
# caller prerequisite so this gate never reaches the network.
required_cross_sdk_fixture_tests=(
  sumeragi_v2_cross_sdk_fixtures::shared_sdk_accept_fixtures_are_exact_current_rust_encodings
  sumeragi_v2_cross_sdk_fixtures::shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation
)
cross_sdk_fixture_target="iroha_data_model_group_02"
cross_sdk_fixture_list="$(
  cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list
)"
cross_sdk_ignored_fixture_list="$(
  cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list --ignored
)"
for required_test in "${required_cross_sdk_fixture_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$cross_sdk_fixture_list"; then
    echo "missing required Rust cross-SDK Sumeragi v2 fixture test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$cross_sdk_ignored_fixture_list"; then
    echo "required Rust cross-SDK Sumeragi v2 fixture test is ignored: ${required_test}" >&2
    exit 1
  fi
done
cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" \
  sumeragi_v2_cross_sdk_fixtures:: -- --test-threads=1

js_status_contract_file="javascript/iroha_js/test/toriiClient.test.js"
required_js_status_contract_tests=(
  "getSumeragiStatusTyped validates and normalizes authoritative v2 status"
  "getSumeragiStatusTyped accepts the local-control liveness blocker"
  "getSumeragiStatusTyped accepts the unsafe-proposal ignore reason"
  "getSumeragiStatusTyped accepts all twelve ignore reasons at the bound"
)
for required_test in "${required_js_status_contract_tests[@]}"; do
  if ! grep -Fq -- "test(\"${required_test}\"," "$js_status_contract_file"; then
    echo "missing required JavaScript Sumeragi v2 status-contract test: ${required_test}" >&2
    exit 1
  fi
done
node --test \
  --test-name-pattern='getSumeragiStatusTyped (validates and normalizes authoritative v2 status|accepts the local-control liveness blocker|accepts the unsafe-proposal ignore reason|accepts all twelve ignore reasons at the bound)' \
  "$js_status_contract_file"

if ! python3 -c 'import pytest, requests' >/dev/null 2>&1; then
  echo "Python Sumeragi v2 status-contract tests require the pinned scripts/requirements.txt dependencies to be installed before this offline gate" >&2
  exit 1
fi
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q \
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_parses_authoritative_v2_snapshot \
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_local_control_pending_liveness_blocker \
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_unsafe_proposal_ignore_reason \
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_all_twelve_ignore_reasons_at_the_bound

# The release identity must include every checkout source plus the ignored
# workspace lockfile, and it must reject an unresolved Git index. Keep this
# exact inventory in the pre-network corridor so weakening the manifest helper
# cannot silently reuse source-bound build or evidence roots.
source_manifest_contract_tests=(
  pytests/scripts/workspace_source_manifest_test.py::test_manifest_is_order_independent_and_content_sensitive
  pytests/scripts/workspace_source_manifest_test.py::test_manifest_distinguishes_deleted_and_symlink_entries
  pytests/scripts/workspace_source_manifest_test.py::test_manifest_tracks_executable_mode
  pytests/scripts/workspace_source_manifest_test.py::test_workspace_manifest_binds_ignored_cargo_lock
  pytests/scripts/workspace_source_manifest_test.py::test_git_unmerged_paths_are_parsed_and_deduplicated
  pytests/scripts/workspace_source_manifest_test.py::test_workspace_manifest_rejects_unmerged_index
)
source_manifest_contract_log="$(mktemp "${TMPDIR:-/tmp}/sumeragi-v2-source-manifest-contract.XXXXXX")"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q \
  "${source_manifest_contract_tests[@]}" 2>&1 | tee "$source_manifest_contract_log"
source_manifest_pipeline_status=("${PIPESTATUS[@]}")
set -e
source_manifest_pass_summary="$(
  grep -Ec '^6 passed in [0-9]+([.][0-9]+)?s$' "$source_manifest_contract_log" || true
)"
rm -f -- "$source_manifest_contract_log"
if ((source_manifest_pipeline_status[0] != 0 || source_manifest_pipeline_status[1] != 0)) \
  || [[ "$source_manifest_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 source-manifest contract preflight did not run exactly six passing tests (pytest=${source_manifest_pipeline_status[0]}, tee=${source_manifest_pipeline_status[1]})" >&2
  exit 1
fi

# Exercise the shell/evidence contract with a mocked Cargo before spending a
# fresh network attempt. Explicit node IDs make a rename or missing adversarial
# case fail collection, and the final summary rejects skipped/xfail coverage.
seed_launcher_contract_tests=(
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_preserves_prior_invocation_evidence
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_ambiguous_test_summary
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_preserves_cargo_failure_through_tee
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_source_drift_before_completion
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_refuses_uninspected_stale_lock
)
seed_launcher_contract_log="$(mktemp "${TMPDIR:-/tmp}/sumeragi-v2-seed-launcher-contract.XXXXXX")"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q \
  "${seed_launcher_contract_tests[@]}" 2>&1 | tee "$seed_launcher_contract_log"
seed_launcher_pipeline_status=("${PIPESTATUS[@]}")
set -e
seed_launcher_pass_summary="$(
  grep -Ec '^10 passed in [0-9]+([.][0-9]+)?s$' "$seed_launcher_contract_log" || true
)"
rm -f -- "$seed_launcher_contract_log"
if ((seed_launcher_pipeline_status[0] != 0 || seed_launcher_pipeline_status[1] != 0)) \
  || [[ "$seed_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 seed-launcher contract preflight did not run exactly ten passing tests (pytest=${seed_launcher_pipeline_status[0]}, tee=${seed_launcher_pipeline_status[1]})" >&2
  exit 1
fi

# Run the complete mocked soak launcher/evidence corpus as one exact file-bound
# preflight. The 38-pass summary rejects missing, added, skipped, or xfailed
# cases before the release corridor can trust the 24-hour evidence path.
taira_soak_contract_files=(
  pytests/scripts/taira_v2_soak_test.py
  pytests/scripts/taira_v2_soak_evidence_test.py
)
taira_soak_contract_log="$(mktemp "${TMPDIR:-/tmp}/taira-v2-soak-contract.XXXXXX")"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q \
  "${taira_soak_contract_files[@]}" 2>&1 | tee "$taira_soak_contract_log"
taira_soak_pipeline_status=("${PIPESTATUS[@]}")
set -e
taira_soak_pass_summary="$(
  grep -Ec '^38 passed in [0-9]+([.][0-9]+)?s$' "$taira_soak_contract_log" || true
)"
rm -f -- "$taira_soak_contract_log"
if ((taira_soak_pipeline_status[0] != 0 || taira_soak_pipeline_status[1] != 0)) \
  || [[ "$taira_soak_pass_summary" != 1 ]]; then
  echo "Taira v2 soak launcher/evidence preflight did not run exactly 38 passing tests (pytest=${taira_soak_pipeline_status[0]}, tee=${taira_soak_pipeline_status[1]})" >&2
  exit 1
fi

if [[ "$profile" == "--release" ]]; then
  # Fail before 128 real-network runs when the strict deductive ledger or its
  # source-bound backend evidence is not release-complete.
  bash ci/check_sumeragi_formal.sh
fi

bash scripts/run_sumeragi_v2_seed_matrix.sh "$profile"

if [[ "$profile" == "--pr" ]]; then
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
  bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
  final_pr_source_manifest_sha256="$(
    python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
  )"
  if [[ ! "$final_pr_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "workspace source manifest helper returned an invalid digest after the PR corridor" >&2
    exit 1
  fi
  if [[ "$final_pr_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
    echo "workspace sources changed during the PR release corridor" >&2
    exit 1
  fi
  echo "Sumeragi v2 PR gate passed: cross-SDK fixture/status parity, 4 seeds × 4 scenarios (16 runs), reducer invariants, adversarial simulations, and trace replay" >&2
  exit 0
fi

bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k
pre_soak_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$pre_soak_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest before the Taira production soak" >&2
  exit 1
fi
if [[ "$pre_soak_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
  echo "workspace sources changed before the Taira production soak" >&2
  exit 1
fi
bash scripts/run_taira_v2_24h_soak.sh

final_release_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$final_release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest after the production release corridor" >&2
  exit 1
fi
if [[ "$final_release_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
  echo "workspace sources changed during the production release corridor" >&2
  exit 1
fi
# Revalidate the deductive evidence after every long-running gate so a TLA+
# edit during chaos or soak execution cannot inherit stale TLAPS success.
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json

echo "Sumeragi v2 production release gates passed, including 100,000 heights and the 24-hour Taira soak" >&2
