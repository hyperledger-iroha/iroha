"""Fixed source-derived SoraFS Python case inventory; no SDK import or execution.

The current canonical test source owns 37 test functions and 77 pytest 9.0.3
cases. Changes to names, parameter decorators or profile inputs require review;
this module grants no success or native execution authority. Call
expected_node_ids with the captured, independently authenticated test bytes.
"""
from __future__ import annotations

import ast
import hashlib

TEST_PATH = "python/iroha_python/tests/sorafs_reference_validation_test.py"
PYTEST_VERSION = "9.0.3"
CASE_LAYOUT_SHA256 = 'c6506081dcf4b08802f92fc0e782bac371bc4c1231178ce14ce6cb7d48221289'
FUNCTIONS = ('test_validate_orderbook_payload_accepts_canonical_order_request',
 'test_orderbook_signature_and_noncanonical_outcomes_match_exactly',
 'test_validate_orderbook_payload_reports_malformed_norito',
 'test_sign_orderbook_payload_deterministically_reproduces_signed_fixtures',
 'test_sign_orderbook_payload_rejects_non_signable_and_bad_keys',
 'test_field_level_orderbook_builders_emit_valid_signed_payloads',
 'test_order_id_derivation_matches_cross_sdk_golden_vector',
 'test_orderbook_builders_accept_owner_account_at_v1_byte_ceiling',
 'test_orderbook_owner_account_byte_ceiling_rejects_adversarial_inputs',
 'test_field_level_orderbook_builder_rejects_noncanonical_order_id',
 'test_field_level_orderbook_builder_enforces_exact_provider_binding',
 'test_field_level_settlement_receipt_builder_rejects_imbalanced_amounts',
 'test_field_level_orderbook_builder_rejects_retired_price_fields',
 'test_field_level_receipt_builder_rejects_retired_amount_fields',
 'test_field_level_orderbook_builder_rejects_noncanonical_xor_quantities',
 'test_max_scaled_xor_quantity_uses_the_155_character_boundary',
 'test_field_level_orderbook_builders_reject_retired_field_aliases',
 'test_field_level_orderbook_builders_reject_noncanonical_selectors',
 'test_validate_pdp_payload_accepts_canonical_commitment',
 'test_validate_pdp_pair_and_bundle_helpers_accept_bound_fixtures',
 'test_validate_fixture_bundle_accepts_linked_replication_and_por',
 'test_fixture_bundle_matches_release_wide_outcomes_byte_for_byte',
 'test_fixture_bundle_selectors_and_input_snapshots_are_exact',
 'test_validate_fixture_bundle_rejects_aliases_and_unbounded_input',
 'test_all_pdp_negative_outcomes_match_exactly',
 'test_validate_pdp_payload_reports_malformed_payloads',
 'test_validate_pdp_challenge_proof_reports_signature_failure',
 'test_reference_validation_rejects_bad_arguments_before_native_validation',
 'test_validate_governance_log_node_matches_moderation_outcome_byte_for_byte',
 'test_validate_governance_log_node_rejects_bad_cids_before_native_dispatch',
 'test_validate_governance_log_node_fails_closed_without_native_function',
 'test_validate_governance_dag_block_accepts_canonical_fixture',
 'test_validate_governance_dag_block_rejects_expected_cid_mismatch',
 'test_validate_governance_dag_head_chain_accepts_root_to_head_fixture',
 'test_validate_governance_dag_head_chain_rejects_reordered_blocks',
 'test_governance_dag_negative_vectors_match_reference_outcomes',
 'test_governance_dag_wrappers_enforce_labels_and_block_count')
PARAMETER_IDS = {'test_field_level_orderbook_builder_rejects_retired_price_fields': ['price_per_gib_micro_xor',
                                                                     'pricePerGibMicroXor',
                                                                     'price_per_gib_micro',
                                                                     'pricePerGibMicro'],
 'test_field_level_receipt_builder_rejects_retired_amount_fields': ['xor_debited_micro_xor',
                                                                    'xorDebitedMicroXor',
                                                                    'xor_debited_micro',
                                                                    'xorDebitedMicro',
                                                                    'provider_credit_micro_xor',
                                                                    'providerCreditMicroXor',
                                                                    'provider_credit_micro',
                                                                    'providerCreditMicro',
                                                                    'fee_amount_micro_xor',
                                                                    'feeAmountMicroXor',
                                                                    'fee_amount_micro',
                                                                    'feeAmountMicro'],
 'test_field_level_orderbook_builder_rejects_noncanonical_xor_quantities': ['1',
                                                                            '1.0_0',
                                                                            'True',
                                                                            'None',
                                                                            '',
                                                                            '+1',
                                                                            '-1',
                                                                            ' 1',
                                                                            '1 ',
                                                                            '01',
                                                                            '1.',
                                                                            '.1',
                                                                            '1.0_1',
                                                                            '1.000000000',
                                                                            '1e0',
                                                                            '0.0000000001',
                                                                            '6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048',
                                                                            "1" * 156,
                                                                            "1" * 10_000],
 'test_fixture_bundle_matches_release_wide_outcomes_byte_for_byte': ['bundle_heterogeneous_positive',
                                                                     'bundle_orderbook_bad_signature_negative',
                                                                     'bundle_orderbook_trailing_bytes_negative',
                                                                     'bundle_pdp_duplicate_hot_leaf_negative',
                                                                     'bundle_pdp_missing_signature_negative',
                                                                     'bundle_pdp_wrong_provider_negative',
                                                                     'bundle_repair_manifest_mismatch_negative',
                                                                     'bundle_repair_provider_unassigned_negative',
                                                                     'bundle_routing_admission_positive']}


def expected_node_ids(source: bytes) -> tuple[str, ...]:
    """Return the exact ordered current cases, refusing a different source layout."""
    if type(source) is not bytes or not source or len(source) > 512 * 1024:
        raise ValueError("case source must be bounded immutable bytes")
    tree = ast.parse(source)
    functions = [node for node in tree.body
                 if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")]
    profiles = [node for node in tree.body if isinstance(node, ast.Assign)
                and any(isinstance(target, ast.Name)
                        and target.id == "_REFERENCE_SDK_BUNDLE_PROFILES"
                        for target in node.targets)]
    if tuple(node.name for node in functions) != FUNCTIONS or len(profiles) != 1:
        raise ValueError("the fixed SoraFS case function/profile inventory changed")
    description = repr([(node.name, [ast.dump(decorator, include_attributes=False)
                                     for decorator in node.decorator_list])
                        for node in functions]) + ast.dump(profiles[0].value, include_attributes=False)
    if hashlib.sha256(description.encode()).hexdigest() != CASE_LAYOUT_SHA256:
        raise ValueError("the fixed SoraFS parameter source changed")
    nodes = tuple(f"{TEST_PATH}::{name}{suffix}"
                  for name in FUNCTIONS
                  for suffix in ([f"[{value}]" for value in PARAMETER_IDS[name]]
                                 if name in PARAMETER_IDS else [""]))
    if len(nodes) != 77 or len(set(nodes)) != 77:
        raise ValueError("the fixed source must own exactly 77 unique cases")
    return nodes
