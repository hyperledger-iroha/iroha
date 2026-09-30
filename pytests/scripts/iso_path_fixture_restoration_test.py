"""Ensure ISO filesystem fault fixtures preserve inherited Path method ownership."""

import importlib
from pathlib import Path
import unittest

import pytest


# Each case executes the real existing fault assertions, then checks shared class custody.
CASES = (
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_input_directory_inspection_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_source_path_exists_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_source_path_symlink_inspection_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_same_existing_path_stat_failures_return_false'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_audit_notary_adapter_test', 'IsoAuditNotaryAdapterTest', 'test_output_directory_inspection_failures_do_not_echo_detail'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_same_existing_path_stat_failures_return_false'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_operator_canary_test', 'IsoOperatorCanaryTest', 'test_text_output_parent_creation_failures_do_not_echo_detail'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_same_existing_file_stat_failures_return_false'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_operator_evidence_verify_test', 'IsoOperatorEvidenceVerifyTest', 'test_text_output_parent_creation_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_input_directory_inspection_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_source_path_exists_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_source_path_symlink_inspection_failures_do_not_echo_detail'),
    ('iso_operator_receipt_verify_test', 'IsoOperatorReceiptVerifyTest', 'test_source_path_resolve_failures_do_not_echo_detail'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_same_existing_file_stat_failures_return_false'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_production_readiness_test', 'IsoProductionReadinessTest', 'test_text_output_parent_creation_failures_do_not_echo_detail'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_same_existing_file_stat_failures_return_false'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_trust_bundle_verify_test', 'IsoTrustBundleVerifyTest', 'test_text_output_parent_creation_failures_do_not_echo_detail'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_symlink_ancestor_inspection_failures_do_not_echo_detail'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_read_regular_file_lstat_failures_do_not_echo_detail'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_same_existing_file_stat_failures_return_false'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_path_resolve_failures_do_not_echo_detail'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_text_output_target_inspection_failures_do_not_echo_detail'),
    ('iso_xsd_fixture_verify_test', 'IsoXsdFixtureVerifyTest', 'test_text_output_parent_creation_failures_do_not_echo_detail'),
 )


@pytest.mark.parametrize("module_name,class_name,case_name", CASES)
def test_iso_fault_fixture_restores_the_path_class_namespace(
    module_name: str, class_name: str, case_name: str
) -> None:
    module = importlib.import_module(f"pytests.scripts.{module_name}")
    path_type = type(Path("."))
    before = dict(vars(path_type))
    result = unittest.TestResult()
    getattr(module, class_name)(case_name).run(result)
    assert not result.failures and not result.errors, (result.failures, result.errors)
    assert not result.skipped, result.skipped
    after = dict(vars(path_type))
    changed = {
        name for name in before.keys() | after.keys()
        if name not in before or name not in after or before[name] is not after[name]
    }
    assert not changed, f"{module_name}::{case_name} changed Path method owners: {sorted(changed)}"
