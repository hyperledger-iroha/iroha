"""Bounded portable preflight data; never reconstructs an execution owner.

The original parent authenticates this archive through its final execution
record. Historical commands, PIDs, paths and timestamps remain inert data.
"""
from __future__ import annotations

from dataclasses import dataclass
import ast
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat

INVENTORY_PATH = 'pytests/scripts/scaling_preflight/inventory.json'

PHASE_DRIVER = 'pytests/scripts/run_scaling_preflight.py'

PYTEST_DRIVER = 'pytests/scripts/run_scaling_collector_preflight.py'

PHASE_COUNTS = {'bootstrap': 29, 'runtime': 28, 'provisioning': 37, 'dependency': 6, 'acquisition': 8, 'test_dependencies': 29}

MAX_INVENTORY_BYTES = 8 * 1024 * 1024

MAX_RESULT_BYTES = 8 * 1024 * 1024

MAX_SOURCE_BYTES = 8 * 1024 * 1024

MAX_OUTPUT_BYTES = 8 * 1024 * 1024

MAX_SOURCE_TOTAL_BYTES = 64 * 1024 * 1024

REQUIRED_PYTEST_SUITES = ('pytests/scripts/applied_request_journal_test.py', 'pytests/scripts/kura_resource_metrics_test.py', 'pytests/scripts/resource_bundle_test.py', 'pytests/scripts/resource_completed_experiment_test.py', 'pytests/scripts/resource_evidence_budget_test.py', 'pytests/scripts/resource_original_custody_test.py', 'pytests/scripts/resource_originating_replay_test.py', 'pytests/scripts/resource_probe_test.py', 'pytests/scripts/resource_probe_worker_test.py', 'pytests/scripts/resource_process_test.py', 'pytests/scripts/resource_reconciliation_pure_test.py', 'pytests/scripts/resource_replay_test.py', 'pytests/scripts/scaling_archive_data_test.py', 'pytests/scripts/scaling_archive_descriptor_test.py', 'pytests/scripts/scaling_canonical_proof_test.py', 'pytests/scripts/scaling_command_test.py', 'pytests/scripts/scaling_completed_authority_test.py', 'pytests/scripts/scaling_completed_scan_boundary_test.py', 'pytests/scripts/scaling_experiment_cli_inputs_test.py', 'pytests/scripts/scaling_experiment_config_test.py', 'pytests/scripts/scaling_experiment_custody_test.py', 'pytests/scripts/scaling_experiment_execution_test.py', 'pytests/scripts/scaling_experiment_files_test.py', 'pytests/scripts/scaling_experiment_final_guard_test.py', 'pytests/scripts/scaling_experiment_final_projection_test.py', 'pytests/scripts/scaling_experiment_invocation_test.py', 'pytests/scripts/scaling_experiment_replay_test.py', 'pytests/scripts/scaling_fixed_command_test.py', 'pytests/scripts/scaling_fixed_trial_test.py', 'pytests/scripts/scaling_generator_test.py', 'pytests/scripts/scaling_launch_inputs_test.py', 'pytests/scripts/scaling_launcher_regression_test.py', 'pytests/scripts/scaling_launcher_test.py', 'pytests/scripts/scaling_measurements_test.py', 'pytests/scripts/scaling_native_facts_test.py', 'pytests/scripts/scaling_native_load_test.py', 'pytests/scripts/scaling_native_outputs_test.py', 'pytests/scripts/scaling_preflight/harness_cases.py', 'pytests/scripts/scaling_preflight_archive_test.py', 'pytests/scripts/scaling_proof_sequence_test.py', 'pytests/scripts/scaling_public_descriptors_test.py', 'pytests/scripts/scaling_public_files_test.py', 'pytests/scripts/scaling_readiness_test.py', 'pytests/scripts/scaling_replayed_workload_test.py', 'pytests/scripts/scaling_seed_pipe_test.py', 'pytests/scripts/scaling_structural_identity_test.py', 'pytests/scripts/scaling_trial_captures_test.py', 'pytests/scripts/scaling_vector_collection_test.py', 'pytests/scripts/scaling_worker_sources_test.py', 'pytests/scripts/signed_request_journal_test.py', 'pytests/scripts/sumeragi_v2_release_python_isolation_test.py', 'pytests/scripts/sumeragi_v2_release_receipt_handoff_test.py', 'pytests/scripts/sumeragi_v2_release_receipt_python_probe_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_cache_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_dependencies_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_handoff_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_inventory_contracts_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_main_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_operation_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_preflight_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_python_profile_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_record_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_seal_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_selection_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_shell_contract_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_staging_test.py', 'pytests/scripts/sumeragi_v2_release_scaling_validator_test.py')

REQUIRED_MIGRATION_OUTCOMES = ('pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[cap]', 'pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[digest]', 'pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[equal_clone]', 'pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[label]', 'pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[path]', 'pytests/scripts/resource_experiment_control_test.py::test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify[unknown]', 'pytests/scripts/resource_experiment_control_test.py::test_control_bytes_before_and_after_actual_ten_run_replay_remain_in_one_owner', 'pytests/scripts/resource_experiment_control_test.py::test_control_read_never_substitutes_for_required_replay', 'pytests/scripts/resource_experiment_control_test.py::test_reader_baseexception_invalidates_prior_actual_replay_results[KeyboardInterrupt]', 'pytests/scripts/resource_experiment_control_test.py::test_reader_baseexception_invalidates_prior_actual_replay_results[MemoryError]', 'pytests/scripts/resource_experiment_control_test.py::test_reader_baseexception_invalidates_prior_actual_replay_results[OSError]', 'pytests/scripts/resource_experiment_control_test.py::test_reader_baseexception_invalidates_prior_actual_replay_results[RuntimeError]', 'pytests/scripts/resource_experiment_control_test.py::test_scope_drift_after_actual_reader_rejects_returned_bytes', 'pytests/scripts/resource_experiment_control_test.py::test_scope_drift_rejected_before_control_reader_io[control]', 'pytests/scripts/resource_experiment_control_test.py::test_scope_drift_rejected_before_control_reader_io[executable]', 'pytests/scripts/resource_experiment_control_test.py::test_scope_drift_rejected_before_control_reader_io[root]', 'pytests/scripts/resource_experiment_control_test.py::test_scope_drift_rejected_before_control_reader_io[run_geometry]', 'pytests/scripts/resource_experiment_control_test.py::test_successful_read_cannot_mask_later_physical_capture_mutation', 'pytests/scripts/resource_experiment_test.py::test_actual_shared_journal_exact_allocation_and_one_byte_below[-1]', 'pytests/scripts/resource_experiment_test.py::test_actual_shared_journal_exact_allocation_and_one_byte_below[0]', 'pytests/scripts/resource_experiment_test.py::test_all_ten_actual_publishers_and_raw_replays_link_exact_allocations_and_maxima', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[0]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[1]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[2]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[3]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[4]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[5]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[6]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[7]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[8]', 'pytests/scripts/resource_experiment_test.py::test_any_failed_actual_run_never_promotes_the_successful_prefix[9]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[failed_finish]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[identity]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[image]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[journal_pair]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[journal_variant]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[missing_last]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[missing_preflight]', 'pytests/scripts/resource_experiment_test.py::test_complete_physical_bundle_cannot_replace_actual_replay_authority[unsampled]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[drain_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[interval_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[max_start_lag_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[measurement_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[preparation_ahead_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[response_deadline_ns]', 'pytests/scripts/resource_experiment_test.py::test_every_time_scope_field_is_independent_and_consistent_before_scan[warmup_ns]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[bad_digest]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[bool_pair]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[duplicate]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[duplicate_peer]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[extra]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[geometry]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[hash]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[labels]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[list]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[missing]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[old_unsampled]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[order]', 'pytests/scripts/resource_experiment_test.py::test_exact_trusted_ten_run_scope_rejected_before_bundle_io[peer_count]', 'pytests/scripts/resource_experiment_test.py::test_interrupted_replay_cannot_publish_a_prefix[KeyboardInterrupt]', 'pytests/scripts/resource_experiment_test.py::test_interrupted_replay_cannot_publish_a_prefix[MemoryError]', 'pytests/scripts/resource_experiment_test.py::test_interrupted_replay_cannot_publish_a_prefix[RuntimeError]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[-1-1]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[0-9223372036854775808]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[1-1]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[2-1]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[4000000-5000000]', 'pytests/scripts/resource_experiment_test.py::test_invalid_or_empty_observed_window_cannot_invent_maxima[True-1]', 'pytests/scripts/resource_experiment_test.py::test_journal_mapping_cannot_relabel_another_fully_bound_run', 'pytests/scripts/resource_experiment_test.py::test_missing_and_repeated_replay_poison_the_context', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-budget]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-controls]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-executable]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-root]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-runs]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[after-stage]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-budget]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-controls]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-executable]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-root]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-runs]', 'pytests/scripts/resource_experiment_test.py::test_mutable_trusted_scope_cannot_drift_across_replay[during-stage]', 'pytests/scripts/resource_experiment_test.py::test_new_reported_context_requires_fresh_ten_run_replay', 'pytests/scripts/resource_experiment_test.py::test_old_unsampled_signature_has_no_entrypoint', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[control_omit]', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[journal_digest]', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[journal_missing]', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[raw_extra]', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[raw_missing]', 'pytests/scripts/resource_experiment_test.py::test_physical_linkage_rejects_missing_extra_or_unbound_files[report_present]', 'pytests/scripts/resource_experiment_test.py::test_resource_finish_does_not_extend_transaction_drain', 'pytests/scripts/resource_experiment_test.py::test_root_namespace_replacement_while_semantics_run_fails', 'pytests/scripts/resource_experiment_test.py::test_tamper_after_a_run_was_replayed_is_caught_by_final_census[after]', 'pytests/scripts/resource_experiment_test.py::test_tamper_after_a_run_was_replayed_is_caught_by_final_census[during]', 'pytests/scripts/resource_reconciliation_test.py::test_context_owns_geometry_reconciles_before_final_scan_and_rejects_closed_use', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[before_replay]', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[geometry_drift]', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[interrupt]', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[wrong_pair]', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[wrong_report]', 'pytests/scripts/resource_reconciliation_test.py::test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan[wrong_variant]')

class ScalingPreflightError(ValueError):
    """A missing, changed, failed or unobserved required preflight check."""

def require(value, message='fixed scaling preflight contract failed'):
    if not value:
        raise ScalingPreflightError(message)

def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True)+'\n').encode('ascii')

def bounded_json(raw, maximum):
    require(type(raw) is bytes and 0 < len(raw) <= maximum, 'preflight JSON size is invalid')
    def pairs(rows):
        result = {}
        for key, value in rows:
            require(key not in result, 'duplicate preflight JSON field')
            result[key] = value
        return result
    # Bound nesting before JSON constructs nested objects. Strings may contain
    # braces; escapes do not end strings or alter structural depth.
    depth, quoted, escaped = 0, False, False
    for byte in raw:
        if quoted:
            if escaped: escaped = False
            elif byte == 92: escaped = True
            elif byte == 34: quoted = False
        elif byte == 34: quoted = True
        elif byte in (91, 123):
            depth += 1
            require(depth <= 12, 'preflight JSON nesting is too deep')
        elif byte in (93, 125): depth -= 1
    try:
        return json.loads(raw, object_pairs_hook=pairs,
            parse_constant=lambda value: require(False, 'nonfinite preflight number'))
    except (UnicodeError, ValueError, RecursionError) as error:
        raise ScalingPreflightError('invalid preflight JSON') from error

NATIVE_FIXTURES = ('crates/iroha_cli/src/transaction_load/allocation/fixtures/admission.jsonl', 'crates/iroha_cli/src/transaction_load/allocation/fixtures/public-run-budget.json')

# Exact non-imported files read as data by the registered source contracts.
DATA_FIXTURES = (
    'pytests/fixtures/kura_resource_metric_projection_v1.json',
    'specs/sumeragi_v2_liveness.md',
    'specs/torii/api_contract.md',
    'docs/source/taira_dataspace_deploy.md',
    'ci/check_sumeragi_v2_multilane_release_inventory.sh',
)

def _relative(value):
    if type(value) is str and value in (*NATIVE_FIXTURES,*DATA_FIXTURES): return value
    require(type(value) is str and len(value) <= 512 and
        re.fullmatch(r'(?:scripts|pytests/scripts|crates)/(?:[A-Za-z0-9_./+-]+)', value)
        and str(Path(value)) == value and '..' not in Path(value).parts)
    return value

def decode_inventory(raw):
    value = bounded_json(raw, MAX_INVENTORY_BYTES)
    require(type(value) is dict and set(value) ==
        {'schema','sources','pytest_suites','phases','pending_outcomes','migrated_outcomes'})
    require(value['schema'] == 'iroha.sumeragi_v2.fixed_scaling.preflight.v1')
    require(type(value['sources']) is dict and 58 <= len(value['sources']) <= 4096)
    for name, digest in value['sources'].items():
        _relative(name)
        require(type(digest) is str and re.fullmatch('[a-f0-9]{64}', digest))
    require(type(value['pytest_suites']) is dict and
        tuple(sorted(value['pytest_suites'])) == REQUIRED_PYTEST_SUITES)
    all_nodes = []
    for name, nodes in value['pytest_suites'].items():
        require(name in value['sources'])
        require(type(nodes) is list and 0 < len(nodes) <= 10000 and
            nodes == sorted(set(nodes)) and all(type(node) is str and
            re.fullmatch('[a-f0-9]{64}',node) for node in nodes))
        all_nodes.extend(nodes)
    require(len(all_nodes) >= 4027 and len(all_nodes) == len(set(all_nodes)),
        'empty or partial collector inventory')
    require(type(value['phases']) is dict and tuple(value['phases']) == tuple(sorted(PHASE_COUNTS)))
    for phase, nodes in value['phases'].items():
        require(type(nodes) is list and len(nodes) == PHASE_COUNTS[phase] and
            nodes == sorted(set(nodes)) and all(type(node) is str and
            node.startswith(phase+'_cases.') and len(node) <= 4096 for node in nodes))
    pending = value['pending_outcomes']
    require(type(pending) is list and len(pending) <= 4096)
    require(all(type(row) is dict and set(row) == {'id','reason'} and
        all(type(row[key]) is str and 0 < len(row[key]) <= 4096 for key in row) for row in pending))
    require(len({row['id'] for row in pending}) == len(pending))
    migrated = value['migrated_outcomes']
    require(type(migrated) is dict and all(type(name) is str and type(node) is str
        and node in all_nodes for name,node in migrated.items()))
    require(not (set(migrated) & {row['id'] for row in pending}) and
        set(migrated) | {row['id'] for row in pending} == set(REQUIRED_MIGRATION_OUTCOMES)
        and len(set(migrated.values())) == len(migrated), 'required outcome mapping is incomplete')
    require(PHASE_DRIVER in value['sources'] and PYTEST_DRIVER in value['sources']
        and set((*NATIVE_FIXTURES,*DATA_FIXTURES)) <= value['sources'].keys())
    return value


def validate_unit_result(name, raw, inventory, inventory_sha256, source_buffers):
    """Validate unchanged child result bytes against the exact required selection."""
    expected = next((nodes for _, unit, nodes in ordered_units(inventory) if unit == name), None)
    require(expected is not None)
    value = bounded_json(raw, MAX_RESULT_BYTES)
    common = {'passed','tests_run','failures','errors','skipped','isolated',
        'no_site','no_bytecode','inputs_before','inputs_after','inputs_unchanged',
        'subtest_observations'}
    if name in PHASE_COUNTS:
        fields = common | {'phase','source_registry_count','expected_node_ids',
            'external_native_processes','qualification','node_ids','copied_sources_after',
            'forbidden_process_attempts','elapsed_ns'}
        if name == 'bootstrap':
            fields |= {'actual_private_blake3_import','actual_bootstrap_composition'}
    else:
        fields = common | {'suite','inventory_sha256','node_sha256s',
            'collected_node_sha256s','outcome_node_sha256s'}
    require(type(value) is dict and set(value) == fields
        and type(value.get('subtest_observations')) is int and value['subtest_observations'] >= 0)
    require(type(value) is dict and value.get('passed') is True
        and value.get('node_ids' if name in PHASE_COUNTS else 'node_sha256s') == list(expected)
        and type(value.get('tests_run')) is int and value['tests_run'] == len(expected)
        and all(type(value.get(key)) is int and value[key] == 0 for key in ('failures','errors','skipped'))
        and all(value.get(key) is True for key in ('isolated','no_site','no_bytecode','inputs_unchanged')))
    if name in PHASE_COUNTS:
        inputs, copies = phase_inputs(inventory, inventory_sha256, source_buffers)
        require(value.get('phase') == name and value.get('source_registry_count') == 58
            and value.get('inputs_before') == inputs == value.get('inputs_after')
            and value.get('copied_sources_after') == copies
            and value.get('forbidden_process_attempts') == []
            and value.get('expected_node_ids') == list(expected)
            and value.get('external_native_processes') is False
            and type(value.get('elapsed_ns')) is int and value['elapsed_ns'] >= 0)
        if name == 'bootstrap':
            require(value.get('actual_private_blake3_import') is True and value.get('actual_bootstrap_composition') is True)
    else:
        require(value.get('suite') == name and value.get('inventory_sha256') == inventory_sha256
            and value.get('inputs_before') == inventory['sources'] == value.get('inputs_after')
            and value.get('collected_node_sha256s') == list(expected)
            and value.get('outcome_node_sha256s') == list(expected))
    return value


ARCHIVE_ID = 'release-scaling.complete-preflight.v1'
INDEX_SCHEMA = 'iroha.sumeragi_v2.scaling_preflight.index.v1'
COMMAND_SCHEMA = 'iroha.sumeragi_v2.scaling_preflight.command.v1'
UNIT_OPERATION = 'multilane-scaling.complete-preflight.unit.v1'
MAX_COMMAND_BYTES = 1024 * 1024
MAX_INDEX_BYTES = 1024 * 1024
_CENSUS_DOMAIN = b'iroha.sumeragi_v2.scaling_preflight.archive.v1\n'
_NS = 1_000_000_000
_MAX_INT = (1 << 63) - 1


def _fields(value, names):
    require(type(value) is dict and set(value) == set(names))
    return value


def _integer(value, minimum=0, maximum=_MAX_INT):
    require(type(value) is int and minimum <= value <= maximum)
    return value


def _digest(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None)
    return value


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _decoded(raw, cap):
    value = bounded_json(raw, cap)
    require(canonical(value) == raw)
    return value


def _owned(value):
    return json.loads(canonical(value))


def _path_text(value):
    require(type(value) is str and 0 < len(value) <= MAX_COMMAND_BYTES
            and '\0' not in value and Path(value).is_absolute()
            and str(Path(value)) == value and '..' not in Path(value).parts)
    return value


def _candidate(value):
    _fields(value, ('head_commit', 'head_tree', 'workspace_source_manifest_sha256'))
    lengths = set()
    for name in ('head_commit', 'head_tree'):
        require(type(value[name]) is str and re.fullmatch('(?:[0-9a-f]{40}|[0-9a-f]{64})', value[name]))
        lengths.add(len(value[name]))
    require(len(lengths) == 1)
    _digest(value['workspace_source_manifest_sha256'])
    return value


def _scope(value, completed):
    _fields(value, ('timeout_seconds', 'original_started_ns', 'deadline_ns', completed))
    timeout = _integer(value['timeout_seconds'], 600, 86400)
    start = _integer(value['original_started_ns'], 1)
    deadline = _integer(value['deadline_ns'], start + 1)
    require(deadline - start == timeout * _NS)
    _integer(value[completed], start + 1, deadline)
    return value


def ordered_units(inventory):
    """Return one fixed ordering of phases followed by sorted exact pytest suites."""
    inventory = decode_inventory(canonical(inventory))
    return tuple(('phase', name, tuple(inventory['phases'][name])) for name in PHASE_COUNTS) + tuple(
        ('pytest', name, tuple(inventory['pytest_suites'][name])) for name in sorted(inventory['pytest_suites']))


def _unit_name(index, role):
    _integer(index, 0, len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES) - 1)
    require(role in ('command', 'result', 'stdout', 'stderr'))
    return 'unit-%03d.%s%s' % (index, role, '.json' if role in ('command', 'result') else '')


def member_caps(inventory):
    """Derive the exact flat namespace; combined output has a separate bound."""
    result = {'inventory.json': MAX_INVENTORY_BYTES, 'index.json': MAX_INDEX_BYTES}
    for index, _ in enumerate(ordered_units(inventory)):
        for role, cap in (('command', MAX_COMMAND_BYTES), ('result', MAX_RESULT_BYTES),
                          ('stdout', MAX_OUTPUT_BYTES), ('stderr', MAX_OUTPUT_BYTES)):
            result[_unit_name(index, role)] = cap
    return result


def _total_cap():
    return MAX_INVENTORY_BYTES + MAX_INDEX_BYTES + (len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES)) * (
        MAX_COMMAND_BYTES + MAX_RESULT_BYTES + MAX_OUTPUT_BYTES)


def archive_census(rows):
    """Commit path, exact byte count and content; never historical inode metadata."""
    require(type(rows) in (list, tuple))
    names = {'inventory.json', 'index.json'} | {_unit_name(index, role)
        for index in range(len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES))
        for role in ('command', 'result', 'stdout', 'stderr')}
    require(len(rows) == len(names))
    by_name = {}
    for row in rows:
        require(type(row) is dict and {'relative_path', 'size_bytes', 'sha256'} <= set(row))
        name = row['relative_path']
        require(type(name) is str and name in names and name not in by_name)
        cap = (MAX_INDEX_BYTES if name == 'index.json' else MAX_COMMAND_BYTES if name.endswith('.command.json')
               else MAX_INVENTORY_BYTES if name == 'inventory.json' else MAX_RESULT_BYTES)
        size = _integer(row['size_bytes'], 0 if name.endswith(('.stdout', '.stderr')) else 1, cap)
        by_name[name] = (size, _digest(row['sha256']))
    require(set(by_name) == names)
    for index in range(len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES)):
        require(sum(by_name[_unit_name(index, role)][0] for role in ('stdout', 'stderr')) <= MAX_OUTPUT_BYTES)
    size = sum(value[0] for value in by_name.values())
    require(0 < size <= _total_cap())
    payload = _CENSUS_DOMAIN + b''.join((name + '\t' + str(count) + '\t' + digest + '\n').encode('ascii')
        for name, (count, digest) in sorted(by_name.items()))
    return dict(files=len(rows), bytes=size, sha256=_sha(payload))


def validate_binding(value):
    """Admit a compact record commitment, without granting execution authority."""
    _fields(value, ('archive_id', 'scope', 'index', 'inventory'))
    require(value['archive_id'] == ARCHIVE_ID)
    _scope(value['scope'], 'completed_ns')
    index = _fields(value['index'], ('sha256', 'size_bytes', 'mode'))
    _digest(index['sha256']); _integer(index['size_bytes'], 1, MAX_INDEX_BYTES)
    require(index['mode'] == '0400')
    inventory = _fields(value['inventory'], ('files', 'bytes', 'sha256'))
    require(_integer(inventory['files'], 1) == 4 * (len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES)) + 2)
    _integer(inventory['bytes'], index['size_bytes'] + 1, _total_cap())
    _digest(inventory['sha256'])
    return _owned(value)


def phase_inputs(inventory, inventory_sha256, source_buffers):
    require(type(source_buffers) is dict)
    raw = source_buffers.get('scripts/nexus/scaling_cli_bootstrap.py')
    require(type(raw) is bytes and _sha(raw) == inventory['sources']['scripts/nexus/scaling_cli_bootstrap.py'])
    tree = ast.parse(raw)
    rows = [node.value for node in tree.body if isinstance(node, ast.Assign)
        and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id == 'PYTHON_SOURCE_FILES']
    require(len(rows) == 1)
    sources = ast.literal_eval(rows[0])
    require(type(sources) is tuple and len(sources) == 58)
    names = set(sources) | {PHASE_DRIVER, 'scripts/nexus/scaling_release_provisioning.py', 'scripts/nexus/scaling_preflight_archive.py'}
    names.update(name for name in inventory['sources']
        if name.startswith('pytests/scripts/scaling_preflight/') and Path(name).suffix in ('.py', '.json'))
    names.add(INVENTORY_PATH)
    expected = dict(inventory['sources'])
    expected[INVENTORY_PATH] = _digest(inventory_sha256)
    require(names <= expected.keys())
    return {name: expected[name] for name in sorted(names)}, {name: expected[name] for name in sources}


def validate_command_inputs(argv, cwd):
    """Bound the complete argv before spawning, reserving scalar envelope space."""
    require(type(argv) is tuple and 1 <= len(argv) <= 64)
    require(all(type(item) is str and len(item) <= MAX_COMMAND_BYTES and '\0' not in item for item in argv))
    _path_text(cwd)
    require(sum(len(item) for item in argv) + len(cwd) <= MAX_COMMAND_BYTES)
    require(len(canonical(dict(argv=list(argv), cwd=cwd))) <= MAX_COMMAND_BYTES - 4096,
            'preflight command envelope exceeds its bound')


def _command_value(value):
    _fields(value, ('schema', 'operation_id', 'index', 'name', 'pid', 'argv', 'argv_sha256',
        'cwd', 'environment_sha256', 'descriptors', 'started_ns', 'deadline_ns', 'completed_ns',
        'returncode', 'stdout', 'stderr', 'violations'))
    require(value['schema'] == COMMAND_SCHEMA and value['operation_id'] == UNIT_OPERATION)
    _integer(value['index'], 0, len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES) - 1)
    require(type(value['name']) is str and value['name'] in (*PHASE_COUNTS, *REQUIRED_PYTEST_SUITES))
    _integer(value['pid'], 1)
    require(type(value['argv']) is list)
    validate_command_inputs(tuple(value['argv']), value['cwd'])
    require(_digest(value['argv_sha256']) == _sha(canonical(value['argv'])))
    _digest(value['environment_sha256'])
    require(type(value['descriptors']) is list and value['descriptors'] == []
            and type(value['violations']) is list and value['violations'] == []
            and type(value['returncode']) is int and value['returncode'] == 0)
    start = _integer(value['started_ns'], 1)
    end = _integer(value['completed_ns'], start + 1)
    _integer(value['deadline_ns'], end)
    for name in ('stdout', 'stderr'):
        stream = _fields(value[name], ('bytes', 'sha256'))
        count = _integer(stream['bytes'], 0, MAX_OUTPUT_BYTES)
        digest = _digest(stream['sha256'])
        require(count != 0 or digest == _sha(b''))
    require(value['stdout']['bytes'] + value['stderr']['bytes'] <= MAX_OUTPUT_BYTES)
    return value


def project_unit_command(index, name, terminal):
    """Serialize original terminal attributes as inert bounded canonical data."""
    require(type(terminal.argv) is tuple and type(terminal.descriptors) is tuple and terminal.descriptors == ()
            and type(terminal.violations) is tuple and terminal.violations == ())
    validate_command_inputs(terminal.argv, terminal.cwd)
    value = dict(schema=COMMAND_SCHEMA, operation_id=UNIT_OPERATION, index=index, name=name,
        pid=terminal.pid, argv=list(terminal.argv), argv_sha256=_sha(canonical(list(terminal.argv))),
        cwd=terminal.cwd, environment_sha256=terminal.environment_sha256, descriptors=[],
        started_ns=terminal.started_ns, deadline_ns=terminal.deadline_ns, completed_ns=terminal.completed_ns,
        returncode=terminal.returncode, stdout=dict(bytes=terminal.stdout_bytes, sha256=terminal.stdout_sha256),
        stderr=dict(bytes=terminal.stderr_bytes, sha256=terminal.stderr_sha256), violations=[])
    raw = canonical(_command_value(value))
    require(len(raw) <= MAX_COMMAND_BYTES)
    return raw


def _member(value, maximum, *, empty=False):
    _fields(value, ('sha256', 'size_bytes'))
    size = _integer(value['size_bytes'], 0 if empty else 1, maximum)
    digest = _digest(value['sha256'])
    require(size != 0 or digest == _sha(b''))
    return value


def _index_value(value, inventory=None):
    _fields(value, ('schema', 'invocation_sha256', 'candidate', 'scope', 'selection', 'units', 'command_context'))
    require(value['schema'] == INDEX_SCHEMA)
    validate_command_context(value['command_context'])
    _digest(value['invocation_sha256']); _candidate(value['candidate'])
    _scope(value['scope'], 'verification_completed_ns')
    _member(value['selection'], MAX_INVENTORY_BYTES)
    units = value['units']
    require(type(units) is list and len(units) == len(PHASE_COUNTS) + len(REQUIRED_PYTEST_SUITES))
    order = tuple(('phase', name) for name in PHASE_COUNTS) + tuple(('pytest', name) for name in REQUIRED_PYTEST_SUITES)
    if inventory is not None:
        require(order == tuple((kind, name) for kind, name, _ in ordered_units(inventory)))
    for index, ((kind, name), unit) in enumerate(zip(order, units, strict=True)):
        _fields(unit, ('index', 'kind', 'name', 'command', 'result', 'stdout', 'stderr'))
        require(type(unit['index']) is int and unit['index'] == index and type(unit['kind']) is str and unit['kind'] == kind
                and type(unit['name']) is str and unit['name'] == name)
        _member(unit['command'], MAX_COMMAND_BYTES); _member(unit['result'], MAX_RESULT_BYTES)
        _member(unit['stdout'], MAX_OUTPUT_BYTES, empty=True); _member(unit['stderr'], MAX_OUTPUT_BYTES, empty=True)
        require(unit['stdout']['size_bytes'] + unit['stderr']['size_bytes'] <= MAX_OUTPUT_BYTES)
    return value


def encode_index(candidate, invocation_sha256, scope, selection, units, *, command_context):
    """Encode the complete fixed manifest for last-file publication."""
    value = dict(schema=INDEX_SCHEMA, invocation_sha256=invocation_sha256,
        candidate=candidate, scope=scope, selection=selection, units=units, command_context=command_context)
    raw = canonical(_index_value(value))
    require(len(raw) <= MAX_INDEX_BYTES)
    return raw


def validate_command_context(value):
    """Validate historical fixed path roles without opening any archived path."""
    _fields(value, ('python', 'repository_root', 'work_root', 'environment_sha256', 'pytest', 'blake3'))
    for name in ('python', 'repository_root', 'work_root'): _path_text(value[name])
    require(value['repository_root'] != value['work_root'])
    _digest(value['environment_sha256'])
    work = Path(value['work_root'])
    for name, prefix in (('pytest', 'dependencies'), ('blake3', 'blake3')):
        dependency = _fields(value[name], ('source_root', 'bundle_root', 'inventory', 'inventory_sha256'))
        for field, suffix in (('source_root', '-source'), ('bundle_root', '-bundle'), ('inventory', '-inventory.json')):
            require(type(dependency[field]) is str and dependency[field] == str(work / (prefix + suffix)))
        _digest(dependency['inventory_sha256'])
    require(len(canonical(value)) <= MAX_COMMAND_BYTES - 4096)
    return _owned(value)


def unit_argv(index, kind, name, context):
    """Construct the sole fixed command tuple, usable as current or historical data."""
    context = validate_command_context(context)
    order = tuple(('phase', row) for row in PHASE_COUNTS) + tuple(('pytest', row) for row in REQUIRED_PYTEST_SUITES)
    _integer(index, 0, len(order) - 1)
    require((kind, name) == order[index])
    root, work = Path(context['repository_root']), Path(context['work_root'])
    argv = (context['python'], '-I', '-B', '-S', str(root / (PHASE_DRIVER if kind == 'phase' else PYTEST_DRIVER)),
        '--phase' if kind == 'phase' else '--suite', name, '--repository-root', str(root),
        '--work-root', str(work / ('unit-%03d' % index)), '--result', str(work / ('result-%03d.json' % index)))
    if kind == 'phase':
        dependency = context['pytest' if name == 'test_dependencies' else 'blake3']
        argv += ('--dependency-root', dependency['source_root'])
    else:
        for prefix, dependency in (('', context['pytest']), ('blake3-', context['blake3'])):
            argv += ('--' + prefix + 'dependency-source', dependency['source_root'],
                '--' + prefix + 'dependency-bundle', dependency['bundle_root'],
                '--' + prefix + 'dependency-inventory', dependency['inventory'],
                '--' + prefix + 'dependency-inventory-sha256', dependency['inventory_sha256'])
    validate_command_inputs(argv, str(root))
    return argv


def _metadata(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_rdev)


def _identity(info):
    # Directory link counts change when unrelated child directories appear.
    # They do not identify an open slot. The checked archive root keeps its
    # complete metadata fence, and regular files retain the single-link checks.
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid,
            None if stat.S_ISDIR(info.st_mode) else info.st_nlink, info.st_rdev)


def _slot_identity(info):
    # Permission and link-count changes are semantic failures, not proof that
    # a still-open numeric slot was reused. Type/owner and descriptor flags
    # remain part of its ownership pin.
    return (info.st_dev, info.st_ino, stat.S_IFMT(info.st_mode), info.st_uid, info.st_rdev)


def _pin(fd):
    return (_slot_identity(os.fstat(fd)), fcntl.fcntl(fd, fcntl.F_GETFL), fcntl.fcntl(fd, fcntl.F_GETFD))


def _same(fd, pin):
    try: return _pin(fd) == pin
    except OSError: return False


def _close(fd, pin):
    try:
        if pin is not None and _same(fd, pin): os.close(fd)
    except OSError:
        pass


_READ = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_DIRECTORY = _READ | os.O_DIRECTORY


def _acquire(name, flags, original, *, parent=None):
    # Before a complete pin exists, use the selected entry and the explicit
    # read-only/nonblocking/non-inheritable acquisition contract for cleanup.
    descriptor = os.open(name, flags, **({} if parent is None else {'dir_fd': parent}))
    pin = None
    try:
        pin = _pin(descriptor)
        require(pin[0] == _slot_identity(original))
        require(pin[1] & (os.O_ACCMODE | os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC) == os.O_NONBLOCK
                and pin[2] == fcntl.FD_CLOEXEC)
        return descriptor, pin
    except BaseException:
        if pin is not None:
            if (pin[0] == _slot_identity(original)
                and pin[1] & (os.O_ACCMODE | os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC) == os.O_NONBLOCK
                and pin[2] == fcntl.FD_CLOEXEC):
                _close(descriptor, pin)
        else:
            try:
                current = _pin(descriptor)
                if (current[0] == _slot_identity(original)
                    and current[1] & (os.O_ACCMODE | os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC) == os.O_NONBLOCK
                    and current[2] == fcntl.FD_CLOEXEC):
                    _close(descriptor, current)
            except OSError:
                pass
        raise


def _names(descriptor, maximum):
    names = set()
    with os.scandir(descriptor) as entries:
        for ordinal, entry in enumerate(entries):
            require(ordinal < maximum)
            names.add(entry.name)
    return names


class _Reader:
    """Bounded retained root and original-slot checks for this fixed archive."""
    def __init__(self, root, *, archive):
        self.handles, self.links, self.rows = [], [], {}
        self.root, self.archive = root, archive
        require(type(root) is type(Path('/')))
        _path_text(str(root))
        try:
            descriptor, pin = _acquire('/', _DIRECTORY, os.stat('/', follow_symlinks=False))
            self.handles.append((descriptor, pin))
            for part in root.parts[1:]:
                parent = descriptor
                before = os.stat(part, dir_fd=parent, follow_symlinks=False)
                require(stat.S_ISDIR(before.st_mode))
                descriptor, pin = _acquire(part, _DIRECTORY, before, parent=parent)
                self.handles.append((descriptor, pin))
                require(_identity(os.fstat(descriptor)) == _identity(before))
                self.links.append((parent, part, _identity(before)))
            self.fd, self.original = descriptor, os.fstat(descriptor)
            require(self.original.st_uid == os.geteuid())
            if archive: require(stat.S_IMODE(self.original.st_mode) == 0o700)
            else: require(stat.S_IMODE(self.original.st_mode) & 0o022 == 0)
            self.check()
        except BaseException:
            self.close()
            raise

    def check(self):
        require(all(_same(fd, pin) for fd, pin in self.handles))
        for parent, name, identity in self.links:
            require(_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
        if self.archive: require(_metadata(os.fstat(self.fd)) == _metadata(self.original))

    def _parent(self, name):
        require(type(name) is str and name and str(Path(name)) == name
                and not Path(name).is_absolute() and '..' not in Path(name).parts)
        if self.archive: require('/' not in name)
        held, links = [], []
        parent = self.fd
        try:
            for part in Path(name).parts[:-1]:
                original = os.stat(part, dir_fd=parent, follow_symlinks=False)
                require(stat.S_ISDIR(original.st_mode))
                descriptor, pin = _acquire(part, _DIRECTORY, original, parent=parent)
                held.append((descriptor, pin))
                require(_identity(os.fstat(descriptor)) == _identity(original))
                links.append((parent, part, _identity(original)))
                parent = descriptor
            return parent, Path(name).name, held, links
        except BaseException:
            for fd, pin in reversed(held): _close(fd, pin)
            raise

    def read(self, name, maximum, *, retain=True):
        self.check()
        parent, leaf, held, links = self._parent(name)
        descriptor, pin = None, None
        try:
            original = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
            require(stat.S_ISREG(original.st_mode) and original.st_uid == os.geteuid()
                    and original.st_nlink == 1 and 0 <= original.st_size <= maximum)
            mode = stat.S_IMODE(original.st_mode)
            require(mode == 0o400 if self.archive else mode & 0o022 == 0)
            descriptor, pin = _acquire(leaf, _READ, original, parent=parent)
            require(_metadata(os.fstat(descriptor)) == _metadata(original))
            digest, chunks, offset = hashlib.sha256(), [], 0
            while offset < original.st_size:
                require(_same(descriptor, pin))
                chunk = os.pread(descriptor, min(65536, original.st_size - offset), offset)
                require(type(chunk) is bytes and 0 < len(chunk) <= min(65536, original.st_size - offset))
                require(_same(descriptor, pin))
                digest.update(chunk)
                if retain: chunks.append(chunk)
                offset += len(chunk)
            require(os.pread(descriptor, 1, offset) == b'' and _same(descriptor, pin)
                    and _metadata(os.fstat(descriptor)) == _metadata(original)
                    and _metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)) == _metadata(original))
            require(all(_same(fd, token) for fd, token in held))
            for source, part, identity in links:
                require(_identity(os.stat(part, dir_fd=source, follow_symlinks=False)) == identity)
            self.check()
            row = dict(relative_path=name, size_bytes=offset, sha256=digest.hexdigest(), mode=f'{mode:04o}', max_bytes=maximum)
            if name in self.rows: require(self.rows[name] == (row, _metadata(original)))
            self.rows[name] = (row, _metadata(original))
            return b''.join(chunks) if retain else None
        finally:
            if descriptor is not None: _close(descriptor, pin)
            for fd, token in reversed(held): _close(fd, token)

    def verify(self, names=None):
        self.check()
        if names is not None: require(_names(self.fd, len(names)) == set(names))
        for name, (_, expected) in self.rows.items():
            parent, leaf, held, links = self._parent(name)
            try:
                require(_metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)) == expected)
                require(all(_same(fd, pin) for fd, pin in held))
                for source, part, identity in links:
                    require(_identity(os.stat(part, dir_fd=source, follow_symlinks=False)) == identity)
            finally:
                for fd, pin in reversed(held): _close(fd, pin)
        self.check()

    def close(self):
        for fd, pin in reversed(self.handles): _close(fd, pin)
        self.handles.clear()


@dataclass(frozen=True, slots=True)
class PreflightArchiveData:
    """Immutable validated data; deliberately no success or execution capability."""
    binding_json: bytes
    index_json: bytes
    selection_json: bytes
    file_rows: tuple[tuple, ...]


def _inspect(root, binding, *, source_root, candidate_identity, invocation_sha256,
             collector_started_ns, timeout_seconds):
    archive = source = None
    try:
        binding = validate_binding(binding)
        _candidate(candidate_identity); _digest(invocation_sha256)
        _integer(collector_started_ns, 1); _integer(timeout_seconds, 600, 86400)
        require(binding['scope']['timeout_seconds'] == timeout_seconds
                and binding['scope']['completed_ns'] <= collector_started_ns)
        archive = _Reader(root, archive=True)
        source = _Reader(source_root, archive=False)
        selection_raw = archive.read('inventory.json', MAX_INVENTORY_BYTES)
        require(selection_raw == source.read(INVENTORY_PATH, MAX_INVENTORY_BYTES))
        inventory = decode_inventory(selection_raw)
        require(not inventory['pending_outcomes'])
        source_buffers, total = {}, 0
        for name, digest in inventory['sources'].items():
            raw = source.read(name, MAX_SOURCE_BYTES, retain=name == 'scripts/nexus/scaling_cli_bootstrap.py')
            row = source.rows[name][0]
            total += row['size_bytes']
            require(total <= MAX_SOURCE_TOTAL_BYTES and row['sha256'] == digest)
            if raw is not None: source_buffers[name] = raw
        caps = member_caps(inventory)
        require(_names(archive.fd, len(caps)) == set(caps))
        index_raw = archive.read('index.json', MAX_INDEX_BYTES)
        require(len(index_raw) == binding['index']['size_bytes'] and _sha(index_raw) == binding['index']['sha256'])
        index = _index_value(_decoded(index_raw, MAX_INDEX_BYTES), inventory)
        require(index['candidate'] == candidate_identity and index['invocation_sha256'] == invocation_sha256)
        for name in ('timeout_seconds', 'original_started_ns', 'deadline_ns'):
            require(index['scope'][name] == binding['scope'][name])
        require(index['scope']['verification_completed_ns'] <= binding['scope']['completed_ns'])
        require(index['selection'] == dict(sha256=_sha(selection_raw), size_bytes=len(selection_raw)))
        expected_rows = [dict(relative_path='inventory.json', **index['selection']),
                         dict(relative_path='index.json', sha256=_sha(index_raw), size_bytes=len(index_raw))]
        expected_rows.extend(dict(relative_path=_unit_name(ordinal, role), **unit[role])
            for ordinal, unit in enumerate(index['units']) for role in ('command', 'result', 'stdout', 'stderr'))
        require(archive_census(expected_rows) == binding['inventory'])
        previous = index['scope']['original_started_ns']
        for ordinal, ((kind, name, _), unit) in enumerate(zip(ordered_units(inventory), index['units'], strict=True)):
            buffers = {}
            for role in ('command', 'result', 'stdout', 'stderr'):
                member = _unit_name(ordinal, role)
                raw = archive.read(member, caps[member], retain=role in ('command', 'result'))
                row = archive.rows[member][0]
                require(unit[role] == dict(sha256=row['sha256'], size_bytes=row['size_bytes']))
                if raw is not None: buffers[role] = raw
            command = _command_value(_decoded(buffers['command'], MAX_COMMAND_BYTES))
            require(command['index'] == ordinal and command['name'] == name
                and tuple(command['argv']) == unit_argv(ordinal, kind, name, index['command_context'])
                and command['cwd'] == index['command_context']['repository_root']
                and command['environment_sha256'] == index['command_context']['environment_sha256']
                and previous <= command['started_ns'] < command['completed_ns'] <= index['scope']['verification_completed_ns']
                and command['deadline_ns'] == index['scope']['deadline_ns'])
            previous = command['completed_ns']
            for role in ('stdout', 'stderr'):
                require(command[role] == dict(bytes=unit[role]['size_bytes'], sha256=unit[role]['sha256']))
            validate_unit_result(name, buffers['result'], inventory, _sha(selection_raw), source_buffers)
        rows = tuple(value[0] for _, value in sorted(archive.rows.items()))
        require(archive_census(rows) == binding['inventory'])
        source.verify()
        archive.verify(caps)
        encoded_rows = tuple((row['relative_path'], row['size_bytes'], row['sha256'], row['mode'], row['max_bytes']) for row in rows)
        return PreflightArchiveData(canonical(binding), index_raw, selection_raw, encoded_rows)
    except Exception:
        raise ScalingPreflightError('fixed scaling preflight archive is invalid') from None
    finally:
        if source is not None: source.close()
        if archive is not None: archive.close()


def inspect_preflight_archive(root, binding, *, source_root, candidate_identity,
                              invocation_sha256, collector_started_ns, timeout_seconds):
    """Verify copied data against externally selected source and parent context."""
    return _inspect(root, binding, source_root=source_root, candidate_identity=candidate_identity,
        invocation_sha256=invocation_sha256, collector_started_ns=collector_started_ns,
        timeout_seconds=timeout_seconds)


def capture_preflight_archive(root, binding, *, source_root, candidate_identity,
                              invocation_sha256, collector_started_ns, timeout_seconds):
    """Return a fully checked census for the receipt owner's own publication fences."""
    checked = _inspect(root, binding, source_root=source_root, candidate_identity=candidate_identity,
        invocation_sha256=invocation_sha256, collector_started_ns=collector_started_ns,
        timeout_seconds=timeout_seconds)
    files = tuple(dict(zip(('relative_path', 'size_bytes', 'sha256', 'mode', 'max_bytes'), row, strict=True))
                  for row in checked.file_rows)
    return files, ({'relative_path': '', 'mode': '0700'},)
