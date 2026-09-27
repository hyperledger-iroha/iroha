# Scaling validator assertion migration audit (2026-09-24)

The production first-release scaling entrypoint is
`scripts/nexus/run_multilane_scaling_gate.py`. It obtains the ten-run borrower
from `FixedExperimentCustody`; `ResourceExperiment(...)` and `RunReplayInput`
cannot be used by an independent validator. The old
`scripts/nexus/validate_multilane_scaling_evidence.py` imports the removed type
and cannot collect or run. This audit does **not** qualify a release run or
authorize a compatibility alias.

The table maps each method in
`scripts/tests/validate_multilane_scaling_evidence_test.py`. Test identifiers
below are relative to `pytests/scripts/`. “V1 counterpart” means the named
current test exercises the corresponding invariant through the current owner;
it does not certify the old wire schema or its independent validator. “Open”
means the old assertion has no exact current-source replacement established by
this audit and must not be erased as if covered.

| Old test method (without `test_`) | V1 counterpart or disposition |
| --- | --- |
| `rehashed_full_bundle_rejects_journal_submission_lag_disagreement` | `scaling_completed_authority_test.py::test_completed_owner_rejects_offer_beyond_original_journal_lag_without_digest_drift` joins the original completed owner to the typed retained replay, with valid control and a later offered request beyond the original journal lag. `scaling_native_facts_test.py::test_journal_numeric_fields_reject_boolean_before_any_child` covers boolean lag; digest rehashing is no longer an independent authority. |
| `valid_bundle_recomputes_both_release_thresholds` | `scaling_measurements_test.py::test_exact_throughput_threshold_and_ratio_of_medians`; `test_exact_latency_ratio_boundary_and_one_nanosecond_failure`; `scaling_experiment_execution_test.py::test_complete_ten_runs_keep_order_and_verify_actual_report_file`. |
| `complete_trace_preserves_cross_interval_events_and_late_acknowledgments` | **Partial:** `scaling_measurements_test.py::test_global_applied_measurement_boundary_and_complete_drain_latency`; `test_late_acknowledgment_preserves_earlier_applied_latency_and_inclusive_drain` transfers the late-response/early-apply invariant, while exact native interval counts remain a separate owner assertion. |
| `complete_cohort_p95_includes_tail_that_would_fail_only_after_drain` | `scaling_measurements_test.py::test_experiment_p95_pools_complete_cohorts_instead_of_median_run_p95`. |
| `final_drain_deadline_is_inclusive_without_changing_measurement_boundary` | `scaling_measurements_test.py::test_global_applied_measurement_boundary_and_complete_drain_latency`. |
| `rehashed_trace_rejects_missing_duplicate_reordered_and_unknown_observations` | `scaling_replayed_workload_test.py::test_complete_ordered_tuple_owners_are_mandatory`; `test_duplicate_signed_hashes_are_rejected_even_when_both_scopes_agree`; `scaling_canonical_proof_test.py::test_every_row_field_is_joined_or_strictly_typed`; old status/trace JSON layout is retired. |
| `rehashed_trace_rejects_missed_schedule_rescheduling_and_catchup` | `scaling_replayed_workload_test.py::test_original_native_schedule_cannot_be_replaced`; `scaling_measurements_test.py::test_complete_request_identity_schedule_and_bounds_fail_closed`. |
| `transaction_hash_shape_matches_the_existing_sdk_owner` | `scaling_measurements_test.py::test_signed_transaction_hash_requires_exact_sdk_text_shape` (five malformed shapes transferred in this cut). |
| `warmup_must_be_separate_and_fully_drained` | `scaling_measurements_test.py::test_warmup_is_fully_drained_and_excluded_from_measurement_p95`. |
| `trace_reconciles_counts_and_unclipped_interval_latencies` | **Partial:** `scaling_measurements_test.py::test_global_applied_measurement_boundary_and_complete_drain_latency`; current native count/journal join is in `scaling_completed_authority_test.py::test_original_replay_reconciliation_rejects_all_join_and_schedule_changes`, but this exact interval mutation is not tested. |
| `drain_observations_are_mandatory_bounded_and_budgeted` | `scaling_replayed_workload_test.py::test_resource_sample_coverage_and_bounds_remain_bound`; `scaling_measurements_test.py::test_resource_sample_scope_and_bounds_reject`. |
| `open_loop_bounds_fail_closed_before_trace_processing` | `scaling_native_load_test.py::test_exact_rational_original_offer_counts`; `scaling_native_facts_test.py::test_invalid_independent_schedule_or_sampling_geometry_is_admitted_early`; `scaling_experiment_config_test.py::test_exact_input_size_ceiling_is_accepted_before_rejecting_one_more_byte`. |
| `zero_warmup_has_no_phantom_requests_and_late_acknowledgments_remain_visible` | `scaling_measurements_test.py::test_complete_zero_warmup_counts_are_explicit`; `test_late_acknowledgment_preserves_earlier_applied_latency_and_inclusive_drain`; `scaling_replayed_workload_test.py::test_real_resource_replay_builds_complete_native_plan`. The old rejected-status JSON is retired. |
| `drain_resource_maximum_is_included_in_release_report` | `scaling_measurements_test.py::test_resource_reduction_includes_preflight_final_tail_and_both_rss_edges`; `scaling_experiment_final_projection_test.py::test_report_is_exact_observed_scope_and_has_no_latency_array_or_combined_verdict`. |
| `trace_artifact_and_strict_first_release_fields_cannot_be_omitted_or_extended` | `scaling_canonical_proof_test.py::test_every_row_field_is_joined_or_strictly_typed`; `scaling_experiment_final_projection_test.py::test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps`. |
| `trace_identity_cannot_be_reused_between_warmup_measurement_or_runs` | `scaling_replayed_workload_test.py::test_duplicate_signed_hashes_are_rejected_even_when_both_scopes_agree`; `scaling_measurements_test.py::test_exact_five_matched_pairs_are_required[cross_hash]`. |
| `release_binding_accepts_exact_source_workspace_and_validator` | **Retired schema:** external validator digest is not a V1 authority; current source/input ownership is in `scaling_experiment_invocation_test.py::test_actual_parser_file_worker_and_image_composition`. |
| `release_binding_rejects_source_workspace_or_validator_drift` | **Partial:** `scaling_experiment_invocation_test.py::test_original_output_paths_are_retained_after_launch_object_mutation`; `test_changed_control_file_rejected_before_execution`; old expected-validator digest has no V1 meaning. |
| `release_trust_anchors_bind_all_executable_measurement_inputs` | `scaling_experiment_invocation_test.py::test_original_candidate_and_worker_hashes_reject_mutation_before_runtime_admission` checks the actual input owner refuses changed plan, budget, Python source manifest, Rust toolchain record, binary manifest, Python runtime binding, all four executable images, and all five resource-worker sources before runtime admission. The separate bootstrap receipt consumer still needs same-source fixture regeneration. |
| `component_report_is_machine_readable_and_public_cli_cannot_qualify` | `scaling_experiment_execution_test.py::test_complete_ten_runs_keep_order_and_verify_actual_report_file`; `test_failure_never_returns_success_or_private_details`; old report schema is retired. |
| `report_publication_handles_partial_writes_and_is_deterministic` | `scaling_public_files_test.py::test_copy_publication_checks_actual_writes_and_preserves_conflicting_files`; `scaling_experiment_final_projection_test.py::test_complete_manifest_preserves_exact_fifteen_artifacts_and_all_censuses`. |
| `report_publication_rejects_preexisting_stage_symlink` | `scaling_public_files_test.py::test_stage_descriptor_replacement_is_rejected_before_any_foreign_write`. |
| `report_publication_never_replaces_destination_or_racer` | `scaling_experiment_files_test.py::test_each_fixed_publication_is_single_use_and_ordered`; `scaling_public_files_test.py::test_copy_publication_checks_actual_writes_and_preserves_conflicting_files`. |
| `report_publication_cleans_stage_after_write_or_file_fsync_failure` | `scaling_public_files_test.py::test_copy_publication_checks_actual_writes_and_preserves_conflicting_files`. |
| `report_publication_cleans_owned_paths_after_publish_failure` | `scaling_public_files_test.py::test_stage_replacement_during_directory_fsync_is_never_unlinked`. |
| `requires_exactly_five_complete_pairs` | `scaling_measurements_test.py::test_exact_five_matched_pairs_are_required`; `scaling_experiment_final_projection_test.py::test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps`. |
| `rejects_duplicate_or_unordered_pair_entries` | Same two V1 tests as previous row. |
| `rejects_pair_order_swap` | Same two V1 tests as previous row. |
| `rejects_nondeterministic_or_unpaired_seed` | `scaling_measurements_test.py::test_exact_five_matched_pairs_are_required[seed]`. |
| `rejects_identity_drift` | `scaling_structural_identity_test.py::test_original_experiment_scope_has_no_json_and_rejects_any_later_policy_change`; `scaling_experiment_final_guard_test.py::test_last_runtime_callback_mutations_fail_the_original_public_verification`. |
| `rejects_nexus_lane_load_manifest_drift` | `scaling_native_load_test.py::test_terminal_identity_schema_counts_and_allocations_are_exact`; `scaling_completed_authority_test.py::test_original_replay_reconciliation_rejects_all_join_and_schedule_changes`. |
| `rejects_retired_nexus_status_file_input` | **Retired schema:** no V1 `status_file`; `scaling_experiment_config_test.py::test_every_plan_object_requires_all_and_only_its_declared_fields` rejects extra input fields. |
| `rejects_unmatched_actual_offered_count` | `scaling_measurements_test.py::test_exact_five_matched_pairs_are_required[work]`. |
| `rejects_offered_count_drift_between_pairs` | Same V1 test as previous row. |
| `rejects_nonfinite_json_values` | `scaling_archive_data_test.py::test_canonical_framing_rejects_duplicates_floats_whitespace_and_unbounded_shapes`; `scaling_experiment_config_test.py::test_json_string_escapes_are_values_and_escaped_duplicate_names_still_fail`. |
| `rejects_each_resource_budget_violation` | `scaling_measurements_test.py::test_resource_failure_is_separate_from_performance_criteria_and_no_pass_exists`. |
| `rejects_skipped_and_failed_runs` | `scaling_experiment_execution_test.py::test_failure_never_returns_success_or_private_details`. |
| `rejects_weak_interval_sample_count` | `scaling_replayed_workload_test.py::test_resource_sample_coverage_and_bounds_remain_bound`. |
| `rejects_weak_latency_sample_count` | `scaling_measurements_test.py::test_minimum_latency_sample_count_is_never_weakened`. |
| `rejects_unordered_or_gapped_raw_samples` | `scaling_replayed_workload_test.py::test_resource_sample_coverage_and_bounds_remain_bound`. |
| `rejects_inconsistent_counters_and_maxima` | `scaling_archive_data_test.py::test_refreshed_hashes_cannot_replace_schema_or_recomputed_data`; `scaling_measurements_test.py::test_resource_reduction_includes_preflight_final_tail_and_both_rss_edges`. |
| `rejects_wrong_or_duplicate_active_execution_lanes` | `scaling_native_load_test.py::test_terminal_identity_schema_counts_and_allocations_are_exact`; `scaling_measurements_test.py::test_exact_five_matched_pairs_are_required`. |
| `rejects_active_lane_identity_drift_across_pairs` | **Partial:** `scaling_experiment_custody_test.py::test_last_pair_cannot_change_original_chain_or_lane_geometry` rejects a changed chain id in pair 5 and changed four-lane cardinality in its last run at the originating fixed-plan admission. Actual lane-id equality across separately generated native pairs is not exposed by this projection and remains open for native proof/qualification. |
| `enforces_median_committed_throughput_ratio` | `scaling_measurements_test.py::test_exact_throughput_threshold_and_ratio_of_medians`. |
| `enforces_pooled_p95_commit_latency_ratio` | `scaling_measurements_test.py::test_exact_latency_ratio_boundary_and_one_nanosecond_failure`; `test_experiment_p95_pools_complete_cohorts_instead_of_median_run_p95`. |
| `thresholds_and_sample_floors_cannot_be_weakened` | `scaling_measurements_test.py::test_minimum_latency_sample_count_is_never_weakened`; `test_exact_throughput_threshold_and_ratio_of_medians`; `test_exact_latency_ratio_boundary_and_one_nanosecond_failure`; fixed V1 constants have no public override. |
| `rejects_tampered_or_out_of_bundle_raw_artifacts` | `scaling_archive_data_test.py::test_inventory_rejects_namespace_and_physical_changes`; `scaling_experiment_files_test.py::test_retained_original_file_and_ancestor_mutation_is_permanent`. |
| `rejects_unexpected_file_and_directory_inventory` | `scaling_archive_data_test.py::test_inventory_rejects_namespace_and_physical_changes`. |
| `rejects_bundle_symlinks` | Same V1 inventory test as previous row. |
| `rejects_bundle_hardlink_aliases` | Same V1 inventory test as previous row. |
| `rejects_bundle_nonregular_entries` | Same V1 inventory test as previous row. |
| `rejects_unsafe_bundle_path_components` | Same V1 inventory test as previous row. |
| `rejects_oversize_files_before_hashing` | `scaling_experiment_files_test.py::test_dynamic_exact_cap_is_accepted_and_next_byte_is_rejected`; `scaling_public_files_test.py::test_copy_is_streamed_and_enforces_cap_before_reading`. |
| `rejects_excessive_file_count` | `scaling_archive_data_test.py::test_inventory_rejects_namespace_and_physical_changes`. |
| `rejects_excessive_aggregate_size` | Same V1 inventory test as previous row. |
| `rejects_duplicate_json_object_keys` | `scaling_experiment_config_test.py::test_duplicate_keys_are_rejected_at_every_plan_object_depth`; `scaling_archive_data_test.py::test_canonical_framing_rejects_duplicates_floats_whitespace_and_unbounded_shapes`. |
| `component_failure_report_is_machine_readable_and_cli_stays_closed` | `scaling_experiment_execution_test.py::test_failure_never_returns_success_or_private_details`; old report schema is retired. |

The old validator still has test-only release receipt callers:
`pytests/scripts/sumeragi_v2_release_receipt_scaling.py` copies it, and
`pytests/scripts/sumeragi_v2_release_receipt_test.py` executes it to regenerate
an old-style report. `scripts/tests/multilane_scaling_gate_contract.sh` also
invokes its help. `pytests/scripts/scaling_main_journal_trace_test.py` imports
`scaling_main_control_reader_test.py`, which imports that validator; the
numeric-schema suite imports its old `EvidenceBundle`. These are real stale
dependencies, not a reason to add a `RunReplayInput` alias. Retiring the
validator, suite and fixture requires replacing those test-owned receipt and
journal assertions against the fixed experiment's manifest/report authority.
No source deletion is claimed here.

The new completed-authority test uses one real completed fixture and its
original public/capture owners. It first reconciles the unchanged replay. It
then changes a measurement offer to one nanosecond beyond the original lag,
leaves the journal digest and capture byte count unchanged, and observes
rejection both by the canonical replay-plan builder and by a second committed
owner. The failed owner cannot subsequently accept the original replay. This
is a V1 source-owner test, not a synthetic rehashed bundle or native proof.

Verification for this cut: five transferred hash-shape cases and the late
acknowledgement case passed as six focused tests, then
the current measurement, replay, final-projection and execution suites passed
`297` tests with `PYTHONPATH=scripts:scripts/nexus:pytests/scripts` under
`target/first-release-python-governance-host/venv/bin/python -m pytest -q`.
The new `scaling_completed_authority_test.py::test_completed_owner_rejects_offer_beyond_original_journal_lag_without_digest_drift`
passed `1/1` in a focused run.
The direct old-suite collection remains failed at the removed `RunReplayInput`
import; it is not counted as a passing release suite.

## Receipt-fixture migration boundary

The historical `test_receipt_requires_exact_existing_scaling_pass_report`
calls `make_scaling_evidence()`, which runs the retired validator; its
`regenerate_scaling_report()` helper repeats that path. Current V1 has an
originating `FixedExperimentCustody` report and a separate parent execution
record consumed by `_validate_fixed_scaling_archive` in
`scripts/write_sumeragi_v2_release_receipt_gate_evidence.py`.

The current archive inspector now has explicit missing-report and
well-formed-but-changed-criterion cases in
`scaling_archive_data_test.py::test_inventory_rejects_namespace_and_physical_changes`;
both passed (`2/2`). The actual fixed-experiment owner's report mutation case
is `scaling_experiment_replay_test.py::test_ten_original_trials_replay_and_derived_report[late_report_corruption]`.
It passed `1/1` with the actual ten-run owner and simulated native pipes
(`992.34s`).
A proposed receipt-consumer mutation test was removed because it could not
reach its assertion: the existing source-pinned preflight fixture stops first
because 37 of its 269 recorded source digests differ from the active checkout.
The protected inventory must be regenerated from the **same frozen source** as
the receipt consumer and then the exact V1 receipt-consumer tests must pass.
Neither the historical receipt test nor its validator was deleted; no receipt
consumer qualification is claimed from the passing owner/archive tests.

## Canonical runner contract and source pins

`scripts/tests/multilane_scaling_gate_contract.sh` now exercises the sole fixed
runner's descriptor-only parser through both shell and Python entrypoints. It
rejects the retired independent `--report` argument and no longer invokes the
old validator. The original invocation owner's 15 parameterized mutation
cases above transfer the source and executable pin assertion to actual V1
file/image/worker admission. They do not create a second report or proof
authority. Python 3.12 validation passed: the shell contract, all 31
`scaling_experiment_invocation_test.py` cases, and 108
`resource_completed_experiment_test.py` plus
`scaling_experiment_execution_test.py` cases. The old validator and synthetic
receipt fixture are still present because their remaining assertions and
source-pinned receipt consumers have not all migrated.

The final-pair chain/lane-geometry test passed `1/1`; its full fixed-experiment
custody suite passed `35/35` with Python 3.12. This preserves the plan-level
part of the old cross-pair assertion without claiming native lane-id proof.

## Retired component numeric schema

The four methods in the former `scaling_main_numeric_schema_test.py` used
retired `scaling_evidence.json` roles and the deleted independent resource
borrower. Their current-V1 assertion mapping is explicit:

| Retired method | Current V1 assertion |
| --- | --- |
| `test_rehashed_role_integer_rejects_equal_bool_or_float` | `scaling_archive_numeric_schema_test.py::test_equal_boolean_or_float_cannot_replace_original_run_index` covers manifest/raw-run/run-receipt/report indices after ordinary hashes are refreshed. `test_fixed_plan_lane_cardinality_requires_an_exact_integer` covers the sole originating lane-count field. The old independent manifest sequence is replaced by exact `RUN_KEYS` ordering in `scaling_experiment_final_projection_test.py::test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps`. |
| `test_rehashed_pair_count_rejects_equal_float` | V1 has no scalar `pair_count`; the exact ten-entry tuple/list and order are enforced by `scaling_experiment_custody_test.py::test_plan_rejects_bad_pairing_and_late_native_preconditions`, `scaling_experiment_final_projection_test.py::test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps`, and archive readback. |
| `test_rehashed_support_version_rejects_equal_bool_or_float` | The old Nexus support-version artifact is retired. V1 has an exact run-receipt schema and rejects extra or changed fields after rehash in `scaling_archive_data_test.py::test_refreshed_hashes_cannot_replace_schema_or_recomputed_data`. |
| `test_rehashed_raw_identity_snapshot_rejects_equal_float` | V1 has one original identity control. `scaling_archive_numeric_schema_test.py::test_equal_float_cannot_replace_original_hardware_integer_after_rehash` covers physical/logical cores and memory under the exact original static byte cap. |

The new Python 3.12 suite passed `14/14`; combined with the canonical final
projection and custody suites it passed `143/143`. This transfers type and
rehash-resistance assertions, not native proof authority. The retired numeric
suite was removed only after this mapping and focused V1 tests passed.

## Retired control-reader assertion migration

The old `scaling_main_control_reader_test.py` contains nine test methods and
still imports the retired validator. Its journal-trace neighbor imports `main`
from that test module, so this cut does **not** remove either suite. Current V1
coverage for its control-custody assertions is:

| Retired method | Current V1 assertion or remaining boundary |
| --- | --- |
| `test_actual_main_reader_retains_same_owner_before_after_replay_and_final_scan` | `scaling_experiment_files_test.py::test_exact_five_controls_and_original_bindings_survive_physical_verification` tests the same retained binding/read owner; `scaling_experiment_execution_test.py::test_complete_ten_runs_keep_order_and_verify_actual_report_file` exercises the completed ten-run owner. The retired direct borrower is not a V1 authority. |
| `test_real_main_decoder_rejects_bad_bounded_json_without_path_reopen` | Current JSON framing is covered by `scaling_experiment_config_test.py::test_preparse_framing_rejects_depth_tokens_strings_mismatches_and_size`, `test_invalid_numbers_are_rejected_without_large_integer_conversion`, and `scaling_archive_data_test.py::test_canonical_framing_rejects_duplicates_floats_whitespace_and_unbounded_shapes`; the original FD and namespace guard are covered by the fixed-file tests below. |
| `test_main_decoder_numeric_json_and_string_braces_preserve_values` | The arbitrary JSON payload and float are not V1 control schemas. `scaling_experiment_config_test.py::test_json_string_escapes_are_values_and_escaped_duplicate_names_still_fail` preserves valid escaped-string parsing while fixed plan/control schemas reject unexpected numeric fields. |
| `test_main_semantic_cap_is_explicit_bounded_integer` | New `scaling_archive_control_binding_test.py::test_original_control_owner_rejects_invalid_semantic_cap_before_read` covers false, negative, absent, float, and over-allocation caps before any body read; `scaling_experiment_files_test.py::test_read_control_requires_original_binding_and_independent_semantic_cap` also covers true and undersized caps. |
| `test_main_smaller_semantic_cap_rejects_before_return_and_poison_reader` | `scaling_experiment_files_test.py::test_read_control_requires_original_binding_and_independent_semantic_cap[small_cap]` verifies fail-closed owner poisoning before body read. |
| `test_main_reference_requires_exact_independent_role_path_and_hash` | New `scaling_archive_control_binding_test.py::test_rehashed_equal_content_cannot_replace_original_artifact_path` covers equal-content, equal-hash cross-role and cross-run substitutions after ordinary hashes are refreshed, including a passing same-content control. `scaling_experiment_final_projection_test.py::test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps` checks original path, label, and digest shape. |
| `test_main_reference_returns_original_object_and_records_exact_control_path` | `scaling_experiment_files_test.py::test_exact_five_controls_and_original_bindings_survive_physical_verification` checks identical originating binding objects and paths; `test_read_control_requires_original_binding_and_independent_semantic_cap[equal_binding]` rejects a value-equal replacement. The retired mutable referenced-path set has no V1 owner. |
| `test_main_semantic_read_rejects_actual_post_admission_file_replacement` | New `scaling_archive_control_binding_test.py::test_original_control_owner_rejects_same_byte_symlink_replacement_before_read` checks original-path replacement before any body read and permanent owner failure. |
| `test_unfinished_public_entrypoint_cannot_accept_old_unadmitted_bundle_or_write_report` | The current `scripts/tests/multilane_scaling_gate_contract.sh` rejects the old report CLI; the retired validator still has test-only receipt/journal dependencies and stays fail-closed until those consumers migrate. |

The eight new V1 control tests and 71 existing fixed-file tests passed together
under Python 3.12 (`79/79`). The old control-reader suite has nine methods and
the old journal-trace suite has twelve; neither is removed or counted as a
passing V1 release suite. The source-pinned receipt fixture still has 37 stale
digests and is unchanged. These archive checks prove bounded data consistency,
not native proof or original-parent release authority.

## Retired journal-trace assertion migration

The old `scaling_main_journal_trace_test.py` has twelve methods against the
retired synthetic journal/trace reconciler. Current V1 splits the original
owner between retained signed requests, applied observations, bounded resource
replay, native facts, and completed-experiment reconciliation. Four new cases
in `scaling_journal_terminal_owner_test.py` run a valid current journal first,
then rehash a changed final or collection row: numeric `1` cannot replace
boolean success, and a non-null failure cannot become an accepted terminal.

| Retired method | Current V1 counterpart or open edge |
| --- | --- |
| `test_full_collector_rows_bind_trace_and_observed_postconditions_without_mutation` | `resource_replay_test.py::test_actual_frozen_worker_captures_replay_exact_values_and_final_deadline`, `scaling_replayed_workload_test.py::test_real_resource_replay_builds_complete_native_plan`, and `scaling_completed_authority_test.py::test_original_replay_reconciliation_rejects_all_join_and_schedule_changes` cover the current split owners. **Open:** one actual native account-postcondition-to-finalized-state join across the completed run has not been established by this test migration. |
| `test_omitting_any_required_plan_schedule_preflight_final_or_terminal_row_fails` | `resource_replay_test.py::test_exact_clock_sequence_cadence_and_complete_outcomes`, `signed_request_journal_test.py::test_complete_schedule_retention_prepared_offer_and_final_coverage`, and `applied_request_journal_test.py::test_every_required_event_has_exact_presence_and_schema` cover mandatory current V1 row families. |
| `test_native_workload_and_each_final_value_must_match_exactly` | `applied_request_journal_test.py::test_summary_cannot_invent_terminals_offsets_heights_or_attempt_counts`, `scaling_replayed_workload_test.py::test_both_applied_observations_must_join_the_original_offer_and_tip`, and the new terminal-owner cases cover current final rows. The retired pre/postcondition JSON fields do not appear unchanged in V1. |
| `test_nested_plan_booleans_cannot_equal_integer_sequence_or_account_identity` | `scaling_replayed_workload_test.py::test_original_native_schedule_cannot_be_replaced` covers boolean sequence and account index; current journal readers require exact integer fields. |
| `test_reordering_or_duplicating_same_content_rows_cannot_preserve_success` | The signed and applied required-event tests above cover ordering/duplicates; `resource_replay_test.py::test_exact_clock_sequence_cadence_and_complete_outcomes` covers resource row order. |
| `test_account_pool_is_exact_bounded_and_has_no_duplicates` | `scaling_generator_test.py::test_invalid_plan_fails_before_new_namespace_or_spawn` enforces four-to-64 account count and `scaling_replayed_workload_test.py::test_real_resource_replay_builds_complete_native_plan` binds the generated account tuple. `scaling_native_facts_test.py::test_duplicate_account_projection_cannot_reach_native_facts` rejects a post-admission duplicate before either native command boundary (2/2). **Open:** this uses fake native children; the finalized native facts/readback join still needs a real-source adversarial test. |
| `test_journal_raw_framing_and_complete_record_bounds_are_mandatory` | `resource_replay_test.py::test_journal_raw_framing_digest_owner_and_schema_are_independent` and `signed_request_journal_test.py::test_complete_schedule_retention_prepared_offer_and_final_coverage` cover current framing, row bounds, and after-finish rejection. |
| `test_complete_native_cohort_cannot_be_replaced_by_a_rejected_trace_row` | New `scaling_journal_terminal_owner_test.py::test_rehashed_terminal_journal_cannot_convert_failure_to_completed_run` covers a failed `request_final` and collection terminal under actual resource replay. |
| `test_journal_submission_lag_must_equal_independent_schedule_with_exact_integer_type` | `scaling_native_facts_test.py::test_journal_numeric_fields_reject_boolean_before_any_child` and `test_invalid_independent_schedule_or_sampling_geometry_is_admitted_early` cover current native-input bounds. `scaling_completed_authority_test.py::test_completed_owner_rejects_equal_valued_mistyped_original_journal_lag` passes 2/2 for `False` and `0.0` replacing the original exact integer zero before completed-owner admission. The actual native source/readback remains a separate release obligation. |
| `test_journal_and_trace_cannot_agree_on_an_offer_outside_the_independent_bound` | `scaling_completed_authority_test.py::test_completed_owner_rejects_offer_beyond_original_journal_lag_without_digest_drift` binds a changed offer to the original lag without digest drift. |
| `test_trace_lag_field_must_equal_the_exact_offer_minus_schedule` | Current V1 retains no independently mutable trace-lag field. `scaling_replayed_workload_test.py::test_original_native_schedule_cannot_be_replaced` checks the native schedule, and `test_both_applied_observations_must_join_the_original_offer_and_tip` bounds the observed offer. `scaling_completed_authority_test.py::test_completed_owner_rejects_offer_beyond_original_journal_lag_without_digest_drift` runs the completed owner with the retained request and an offer changed to `scheduled + original lag + 1`; the valid control and changed request exercise the exact cross-owner difference without inventing a retired field. Native proof/readback remains open. |
| `test_submission_lag_input_itself_requires_a_bounded_exact_integer` | New `scaling_journal_lag_owner_test.py` rejects `None`, boolean, float, negative, overflowing integer, and string at both originating `FixedNativeLoad` and `NativeFacts` admission before any child or output. The paired valid controls admit zero and each plan's exact quarter-period boundary. This closes the original bound-type assertion, not the separate collector-plan-to-original equality or native proof. |

Python 3.12 current-owner verification passed `527/527` across the new terminal
tests and the resource, signed-request, and applied-request journal suites.
The old control-reader and journal-trace suites remain together: nine and
twelve methods respectively. The open current-owner joins above, plus the
retired validator's receipt consumer with 37 stale source-pinned digests, rule
out deleting both suites as a collection workaround. No V1 production gate or
source pin was changed.

The later original-bound cut passed `16/16` under Python 3.12; combined with
the current native-load, native-facts, terminal-journal and replayed-workload
suites it passed `333/333`. It adds current owner admission coverage only; the
other open journal/trace rows above still prevent deletion of the retired
journal-trace or control-reader suites.

## Retired synthetic runner assertion migration

The 13 methods in `scripts/tests/run_multilane_scaling_gate_test.py` drive an
external `--trial-command` and the retired `scaling_evidence.json` /
`validation_report.json` pair. The current runner accepts only inherited
launch/seed descriptors and consumes a typed `ExperimentOutcome` from the
originating fixed-experiment owner. New
`pytests/scripts/scaling_runner_result_gate_test.py` exercises that current
boundary: all three exact boolean criteria determine exit status; foreign
report paths, malformed hashes, integer-as-boolean criteria, foreign outcome
types, and failed dependency closure cannot yield `PASS`. It also proves a
third failed trial never opens later pairs or publishes a manifest/report,
rejects retired options before owner admission, and checks the current source
and runbook contracts.

| Retired runner method | Current V1 assertion or disposition |
| --- | --- |
| `test_shell_entrypoint_parses_and_exposes_runner_help` | `scripts/tests/multilane_scaling_gate_contract.sh` checks shell/Python help equality and the three inherited descriptor arguments. Old trial-command/memory override help is rejected. |
| `test_checked_in_shell_contract` | Same current shell contract, executed under Python 3.12 in this cut. |
| `test_runner_executes_exactly_five_pairs_in_canonical_order` | `scaling_experiment_execution_test.py::test_complete_ten_runs_keep_order_and_verify_actual_report_file` tests the originating run order; `scaling_runner_result_gate_test.py::test_runner_reports_only_all_three_observed_criteria` tests CLI exit status; exact rational 3/2 and 5/4 calculations are in `scaling_measurements_test.py`. The old synthetic 1.6/1.2 report is not a native qualification. |
| `test_runner_archives_identity_config_harness_validator_and_existing_tools` | `scaling_experiment_invocation_test.py::test_actual_parser_file_worker_and_image_composition` checks current source/image/worker pins; `scaling_archive_data_test.py::test_complete_real_file_resource_replay_recomputes_all_ten_runs` checks the fixed archive. The independent validator artifact is retired. |
| `test_runner_fail_fast_records_failed_and_pending_trials` | New `scaling_runner_result_gate_test.py::test_third_failed_trial_never_starts_a_later_pair_or_publishes_report` checks original run ordering and failure closure. V1 does not mint a public failed/pending manifest as a substitute for a completed owner. |
| `test_runner_surfaces_identity_drift_from_raw_observation` | `scaling_experiment_invocation_test.py::test_changed_control_file_rejected_before_execution` and `scaling_experiment_final_guard_test.py::test_last_runtime_callback_mutations_fail_the_original_public_verification` reject original input/runtime drift. |
| `test_runner_rejects_zero_exit_without_raw_sample_file` | `scaling_native_outputs_test.py::test_reply_and_output_group_must_be_complete` requires actual native output files after command completion; `scaling_fixed_trial_test.py::test_each_native_nonzero_is_terminal_and_cleanup_retains_original_children` checks failed child status. |
| `test_runner_refuses_weak_sample_floor` | `scaling_measurements_test.py::test_minimum_latency_sample_count_is_never_weakened` and `scaling_experiment_config_test.py::test_every_native_policy_enforces_declared_integer_types` enforce current fixed plan/sample inputs; the old override is rejected by the new parser test. |
| `test_runner_rejects_invalid_drain_and_submission_bounds_before_trials` | `scaling_native_facts_test.py::test_invalid_independent_schedule_or_sampling_geometry_is_admitted_early`, current plan admission, and the new parser's retired-option rejection cover the V1 owners. |
| `test_runner_rejects_rehashed_incomplete_or_non_authoritative_traces` | `resource_replay_test.py::test_exact_clock_sequence_cadence_and_complete_outcomes`, `scaling_fixed_trial_test.py::test_actual_resource_replay_rejects_global_only_journal_before_reader_stop`, and the new `scaling_journal_terminal_owner_test.py::test_rehashed_terminal_journal_cannot_convert_failure_to_completed_run` cover complete current journal/application observations. |
| `test_runner_refuses_to_overwrite_existing_artifact_directory` | `scaling_experiment_invocation_test.py::test_output_namespace_exclusion_before_runtime_admission` refuses preexisting output before runtime; the current runner does not accept an artifact directory override. |
| `test_runner_has_no_cargo_execution_path_or_skip_option` | New `scaling_runner_result_gate_test.py::test_fixed_runner_source_has_no_cargo_or_external_trial_command` and `test_runner_parser_rejects_retired_overrides_before_owner_admission` cover the current entrypoint. |
| `test_runbook_records_gate_math_raw_evidence_and_open_handoff` | New `scaling_runner_result_gate_test.py::test_runbook_names_current_owner_measurements_and_parent_handoff` checks the current owner, exact fractions, native replay and parent record, and rejects the retired file names. |

The 19 new runner tests passed alone; together with current execution, CLI
input and invocation tests they passed `119/119`. The current shell contract
passed. This is a source/test migration only: the full native five-pair run,
same-candidate parent receipt and physical scaling qualification remain open.
The retired runner suite was removed only after this thirteen-method mapping,
the current-owner tests, and an exact reference inventory. Its former
trial-command fixture is not counted as a passing V1 suite or used to excuse
the 37 stale source-pinned receipt digests. The retired validator, control-reader
and journal-trace suites remain open separately.

After removing the retired suite, the combined current runner/execution/CLI/
invocation/release-shell test selection passed 168 tests and failed one
unrelated source-inventory assertion in
`sumeragi_v2_release_scaling_shell_contract_test.py`. That assertion expects
`20f24456…` for a shell-source region whose `HEAD` content itself hashes to
`7e00224c…`; this migration did not modify that shell file or rewrite its
pin. The current `multilane_scaling_gate_contract.sh` passes independently.

## 24 September exact-account and lag owner controls

The current NativeFacts owner now has two additional adversarial tests. After
admission, replacing the generated account tuple with one duplicate account
identity is rejected before the stopped-tip command, or after that command and
before the facts command. The original retained input is never refreshed from
the changed projection. The completed-run owner also rejects `False` and
`0.0` in place of its original integer zero submission lag before admitting a
new completed authority; the typed native journal-plan admission independently
rejects both values even though they compare equal to zero. These tests use
fake native children and do not
establish the required genuine finalized native facts/readback proof.

On this `optimizations` checkout under Python 3.12,
`target/py312-venv/bin/python -m pytest -q
pytests/scripts/scaling_native_facts_test.py` passed **109/109**. The focused
equal-valued lag selector passed **2/2**, followed by the complete
`pytests/scripts/scaling_completed_authority_test.py` module at **26/26**.
The selector passed **2/2** again after adding the independent typed-plan
assertion.
The combined selection of both current-owner modules then passed **135/135**
on the settled Python source. It still uses fake native children and cannot
replace the production proof/receipt and source-pinned candidate inventory.
The first lag-test attempt failed in a test-only post-rejection accessor: a
failed owner correctly poisons that accessor. The assertion was removed, and
the focused and complete module runs above are the settled evidence. The
source-pinned preflight inventory, native proof/receipt consumer, five paired
measured runs, and release candidate remain open.
