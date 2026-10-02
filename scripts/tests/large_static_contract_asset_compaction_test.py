"""Fail-closed guard for the large static Rust test-contract asset migration."""

from __future__ import annotations

import copy
import json
import hashlib
import hmac
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERSION = "IROHA_STATIC_CONTRACT_ROWS_V1"
SOURCE_PATHS = ('crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs',
 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs',
 'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs',
 'crates/iroha_torii/src/openapi.rs',
 'crates/iroha_torii/src/openapi/tests.rs',
 'crates/iroha_torii/src/openapi/tests/catalog_and_contracts.rs',
 'crates/iroha_torii/src/openapi/tests/vpn_da.rs',
 'crates/iroha_torii/src/openapi/tests/diagnostics_schemas.rs',
 'crates/iroha_torii/src/openapi/tests/fee_quote_contract.rs',
 'crates/iroha_torii/src/openapi/tests/finality_app_contracts.rs',
 'crates/iroha_torii/src/openapi/tests/retail_fee_contract.rs',
 'crates/iroha_torii/src/openapi/tests/iso20022_auth.rs',
 'crates/iroha_torii/src/openapi/tests/json_value_contract.rs',
 'crates/iroha_torii/src/openapi/tests/prepared_account_contracts.rs',
 'crates/iroha_torii/src/openapi/tests/privacy_release_qualification.rs',
 'crates/iroha_torii/src/openapi/tests/private_settlement_contract.rs',
 'crates/iroha_torii/src/openapi/tests/public_contract_call.rs',
 'crates/iroha_torii/src/openapi/tests/query_asset_absence_contract.rs',
 'crates/iroha_torii/src/openapi/tests/sns_contract.rs',
 'crates/iroha_torii/src/openapi/tests/soracloud_lease_contracts.rs',
 'crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs',
 'crates/iroha_torii/src/openapi/tests/sorafs_pop_contracts.rs')
ASSETS = {
    'cleanup': ('crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_contracts_v1.txt', 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs', 'sha3_256', 'CLEANUP_CONTRACT_ASSET_LEN', 'CLEANUP_CONTRACT_ASSET_SHA3_256'),
    'proof': ('crates/iroha_data_model/src/soracloud/tests/proof_schema_contracts_v1.txt', 'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs', 'sha256', 'PROOF_SCHEMA_CONTRACT_ASSET_LEN', 'PROOF_SCHEMA_CONTRACT_ASSET_SHA256'),
    'openapi': ('crates/iroha_torii/src/openapi/tests/openapi_static_contracts_v1.txt', 'crates/iroha_torii/src/openapi/tests/vpn_da.rs', 'sha256', 'OPENAPI_STATIC_CONTRACT_ASSET_LEN', 'OPENAPI_STATIC_CONTRACT_ASSET_SHA256'),
}
SECTION_ORDER = {'cleanup': ('secret_scalar_owner_clears_constructor_and_transfer_slots.1',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.1',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.2',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.3',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.4',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.5',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.6',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.7',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.8',
             'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values.9',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.1',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.2',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.3',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.4',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.5',
             'secret_msm_point_owner_and_borrowed_publication_boundary_are_static.6',
             'prover_scalar_publication_borrows_every_private_response.1',
             'prover_scalar_publication_borrows_every_private_response.2',
             'scalar_vector_borrowed_product_preallocates_and_clears_every_exit.1',
             'scalar_vector_borrowed_product_preallocates_and_clears_every_exit.2',
             'scalar_vector_borrowed_scaled_accumulation_source_boundary.1',
             'scalar_vector_borrowed_scaled_accumulation_source_boundary.2',
             'scalar_vector_borrowed_scaled_accumulation_source_boundary.3',
             'scalar_vector_borrowed_scaled_accumulation_source_boundary.4',
             'vector_padding_and_split_clear_replaced_allocations.1',
             'vector_padding_and_split_clear_replaced_allocations.2',
             'vector_padding_and_split_clear_replaced_allocations.3',
             'vector_padding_and_split_clear_replaced_allocations.4',
             'vector_padding_and_split_clear_replaced_allocations.5',
             'vector_padding_and_split_clear_replaced_allocations.6',
             'vector_padding_and_split_clear_replaced_allocations.7',
             'vector_padding_and_split_clear_replaced_allocations.8',
             'vector_padding_and_split_clear_replaced_allocations.9',
             'random_vector_clears_success_and_partial_failure.1',
             'random_vector_clears_success_and_partial_failure.2',
             'random_vector_clears_success_and_partial_failure.3',
             'vector_commitment_values_rehome_without_copy_or_allocation.1',
             'scalar_commitment_opening_source_boundary_stays_private_and_zeroizing.1'),
 'proof': ('soracloud_fhe_input_admission_schema_advertises_backend.1.1',
           'soracloud_fhe_input_admission_schema_advertises_backend.2.1',
           'soracloud_fhe_input_admission_schema_advertises_backend.3.1',
           'soracloud_fhe_input_admission_schema_advertises_backend.4.1',
           'soracloud_fhe_input_admission_schema_advertises_backend.5.1',
           'soracloud_fhe_public_key_schema_advertises_statement_material.1.1',
           'soracloud_fhe_public_key_schema_advertises_statement_material.2.1',
           'soracloud_fhe_public_key_schema_advertises_statement_material.3.1',
           'soracloud_fhe_public_key_schema_advertises_statement_material.4.1',
           'soracloud_fhe_public_key_schema_advertises_statement_material.5.1',
           'soracloud_fhe_public_key_schema_advertises_proof_input_material.1.1',
           'soracloud_fhe_public_key_schema_advertises_proof_input_material.1.2',
           'soracloud_fhe_public_key_schema_advertises_proof_input_material.2.1',
           'soracloud_fhe_public_key_schema_advertises_proof_input_material.3.1',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.1.1',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.1.2',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.1.3',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.1.4',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.2.1',
           'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary.3.1',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.1.1',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.1',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.2',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.3',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.4',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.5',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.6',
           'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.7'),
 'openapi': ('openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.1',
             'openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.strings.1',
             'openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.2',
             'openapi.static_account_operations_publish_exact_auth_and_private_responses.strings.1',
             'openapi.static_account_operations_publish_exact_auth_and_private_responses.rows.1',
             'openapi.sccp_schema_serialization_excludes_secret_fields.strings.1',
             'openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.rows.1',
             'openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.strings.1',
             'openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.strings.2',
             'openapi.retired_sorafs_economics_surface_is_absent.strings.1',
             'openapi.retired_sorafs_economics_surface_is_absent.strings.2',
             'openapi.converted_catalog_families_have_exact_openapi_operations.strings.1',
             'openapi.content_route_documents_conditional_cache_and_auth_contract.strings.1',
             'openapi.content_route_documents_conditional_cache_and_auth_contract.rows.1',
             'openapi.generated_spec_includes_documented_paths.strings.1',
             'openapi.generated_spec_includes_documented_paths.strings.2',
             'openapi.generated_spec_includes_documented_paths.strings.3',
             'openapi.generated_spec_includes_documented_paths.strings.4',
             'openapi.generated_spec_includes_documented_paths.strings.5',
             'openapi.generated_spec_includes_documented_paths.strings.6',
             'openapi.generated_spec_includes_documented_paths.strings.7',
             'openapi.generated_spec_includes_documented_paths.strings.8',
             'openapi.generated_spec_includes_documented_paths.strings.9',
             'openapi.generated_operations_declare_tool_effects.strings.1',
             'openapi.generated_operations_declare_tool_effects.strings.2',
             'openapi.openapi_schemas_include_system_keys.strings.1',
             'openapi.openapi_schemas_include_system_keys.retired',
             'openapi.generated_spec_includes_documented_paths.path_present.1',
             'openapi.generated_spec_includes_documented_paths.path_present.2',
             'openapi.generated_spec_includes_documented_paths.path_present.3',
             'openapi.generated_spec_includes_documented_paths.path_present.5',
             'openapi.generated_spec_includes_documented_paths.path_absent.6',
             'openapi.generated_spec_includes_documented_paths.path_present.7',
             'openapi.generated_spec_includes_documented_paths.path_present.8',
             'openapi.generated_spec_includes_documented_paths.path_absent.9',
             'openapi.generated_spec_includes_documented_paths.path_present.10',
             'openapi.generated_spec_includes_documented_paths.path_absent.11',
             'openapi.generated_spec_includes_documented_paths.path_present.12',
             'openapi.generated_spec_includes_documented_paths.path_present.13',
             'openapi.generated_spec_includes_documented_paths.path_absent.14',
             'openapi.generated_spec_includes_documented_paths.path_present.15',
             'openapi.generated_spec_includes_documented_paths.path_absent.16',
             'openapi.generated_spec_includes_documented_paths.path_present.17',
             'openapi.generated_spec_includes_documented_paths.path_present.18',
             'openapi.generated_spec_includes_documented_paths.path_present.19',
             'openapi.generated_spec_includes_documented_paths.path_present.20',
             'openapi.generated_spec_includes_documented_paths.path_present.21',
             'vpn.vpn_openapi_paths_are_typed_signed_and_use_runtime_success_statuses.strings.1',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.strings.1',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.strings.2',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.strings.3',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.1',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.2',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.strings.4',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.3',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.4',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.strings.5',
             'vpn.sorafs_tag_documents_exact_canonical_quantity_contract.strings.1',
             'vpn.detached_asset_transfer_openapi_is_strict_and_two_phase.strings.1',
             'vpn.detached_asset_transfer_openapi_is_strict_and_two_phase.strings.2',
             'vpn.retired_server_contract_deployment_paths_are_absent.strings.1',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.1',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.2',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.rows.1',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.3',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.4',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.rows.2',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.5',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.6',
             'vpn.governance_mutation_openapi_is_typed_closed_and_secret_free.strings.7',
             'vpn.subscription_mutations_publish_exact_unsigned_v1_draft_contract.strings.1',
             'vpn.subscription_mutations_publish_exact_unsigned_v1_draft_contract.strings.2',
             'vpn.subscription_mutations_publish_exact_unsigned_v1_draft_contract.strings.3',
             'vpn.local_signing_openapi_contracts_are_closed_and_secret_free.strings.1',
             'vpn.local_signing_openapi_contracts_are_closed_and_secret_free.strings.2',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.1',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.strings.1',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.strings.2',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.2',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.3',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.4',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.5',
             'vpn.vpn_openapi_paths_are_typed_signed_and_use_runtime_success_statuses.rows.1',
             'openapi.static_account_operations_publish_exact_auth_and_private_responses.method_rows',
             'openapi.musubi_provider_bundle_attestation.schema_rows',
             'openapi.transaction_payload.required',
             'vpn.governance_mutation.request_property_rows',
             'vpn.governance_mutation.required_field_rows',
             'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.6',
             'vpn.governance_read_path_parameters_publish_exact_runtime_grammars.rows.1',
             'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.6')}
TEST_INVENTORY = {'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs': ('soracloud_fhe_public_input_schema_hashes_are_stable',
                                                                  'soracloud_fhe_input_admission_schema_advertises_backend',
                                                                  'soracloud_fhe_public_key_schema_advertises_statement_material',
                                                                  'soracloud_fhe_public_key_schema_advertises_proof_input_material',
                                                                  'soracloud_fhe_bootstrap_key_schema_advertises_refresh_summary',
                                                                  'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest',
                                                                  'soracloud_fhe_proof_validate_rejects_zero_prehash_statement_hashes',
                                                                  'soracloud_fhe_proof_validate_rejects_textual_placeholder_native_envelope_only',
                                                                  'soracloud_fhe_native_envelope_placeholder_scan_is_text_only',
                                                                  'fhe_input_admission_proof_validate_requires_vk_commitment_and_matching_envelope_hash',
                                                                  'fhe_input_admission_proof_validate_requires_public_key_and_ciphertext_digests',
                                                                  'fhe_input_admission_proof_validate_rejects_over_capacity_bounds',
                                                                  'fhe_input_admission_proof_validate_preflights_attachment_metadata_before_bounds',
                                                                  'fhe_input_admission_proof_validate_rejects_open_verify_envelope_drift'),
 'crates/iroha_torii/src/openapi.rs': (),
 'crates/iroha_torii/src/openapi/tests.rs': ('canonical_output_contract_has_one_details_owner_and_header_without_result_root',
                                             'openapi_authorities_have_only_resolvable_component_refs',
                                             'package_openapi_authority_is_canonical_norito_json',
                                             'privacy_proposed_lifecycle_schema_matches_explicit_governance_payload',
                                             'standalone_ballot_drafts_publish_one_exact_success_and_standard_bad_request',
                                             'account_onboarding_current_state_openapi_is_one_closed_v1_observation',
                                             'connect_status_openapi_separates_session_and_operator_aggregate',
                                             'retired_apartment_execution_history_is_absent',
                                             'uploaded_private_model_runtime_openapi_surface_is_absent',
                                             'soracloud_release_openapi_matches_the_exact_closed_catalog_surface',
                                             'pipeline_preflight_schema_exposes_only_per_scheme_signature_batch_caps',
                                             'checked_openapi_assets_match_package_authority',
                                             'public_lane_staking_schema_closes_status_variants_and_unbond_cutoff',
                                             'compiled_projection_matches_served_bytes',
                                             'transaction_payload_schema_requires_closed_domain_and_positive_ttl',
                                             'authenticated_transaction_nullable_fields_are_required_and_nullable',
                                             'incoming_static_openapi_contracts_remain_bound_to_runtime_routes',
                                             'static_account_operations_publish_exact_auth_and_private_responses',
                                             'compiled_private_cache_contract_follows_the_route_catalog',
                                             'operator_credential_management_contract_is_closed_and_two_factor',
                                             'musubi_provider_bundle_attestation_and_exact_release_contract_is_static',
                                             'openapi_uint64_bounds_keep_exact_integer_tokens_recursively'),
 'crates/iroha_torii/src/openapi/tests/catalog_and_contracts.rs': ('static_authority_is_the_complete_catalog_projection_with_exact_effects',
                                                                   'sccp_schema_serialization_excludes_secret_fields',
                                                                   'sccp_governance_openapi_tracks_the_v1_parliament_proposal',
                                                                   'production_constants_embedded_in_openapi_remain_frozen',
                                                                   'openapi_route_auth_metadata_matches_enabled_catalog_projection',
                                                                   'openapi_standard_security_matches_enabled_catalog_authentication',
                                                                   'protocol_specific_bootle_bearer_security_is_preserved',
                                                                   'openapi_operations_equal_the_enabled_catalog_projection',
                                                                   'every_operation_uses_one_declared_top_level_tag',
                                                                   'exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent',
                                                                   'retired_sorafs_economics_surface_is_absent',
                                                                   'converted_catalog_families_have_exact_openapi_operations',
                                                                   'soracloud_status_documents_only_the_canonical_routing_count',
                                                                   'canonical_stream_operations_publish_fail_closed_contract',
                                                                   'retired_alias_voprf_surface_does_not_reappear',
                                                                   'content_route_documents_conditional_cache_and_auth_contract',
                                                                   'ledger_executed_block_wire_cached_loading_is_safe_from_256_kib_callers',
                                                                   'account_capabilities_document_exact_public_bootstrap_policy',
                                                                   'generated_spec_includes_documented_paths',
                                                                   'generated_spec_exposes_only_kagemusha_v1',
                                                                   'musubi_v1_openapi_matches_the_complete_catalog_and_declares_models',
                                                                   'musubi_instruction_previews_discriminate_equal_payload_shapes_by_wire_id',
                                                                   'musubi_crypto_text_schemas_do_not_impose_single_key_size_limits',
                                                                   'musubi_cursor_and_ordered_prefix_bounds_match_the_wire_types',
                                                                   'musubi_chunker_text_bounds_match_the_wire_type',
                                                                   'multisig_propose_schema_binds_complete_native_retail_assessment',
                                                                   'multisig_cancel_response_requires_typed_fee_payment_property',
                                                                   'multisig_propose_instruction_schema_matches_native_norito_json',
                                                                   'generated_operations_declare_tool_effects',
                                                                   'sumeragi_evidence_audit_contract_is_closed_and_bounded',
                                                                   'retired_sumeragi_vrf_surfaces_are_absent',
                                                                   'validation_fee_plaintext_contracts_stay_retired_and_parliament_capabilities_are_exact',
                                                                   'pipeline_fastpq_recovery_documents_operator_auth_and_bounds',
                                                                   'signed_transaction_submission_documents_exact_preadmission_contract',
                                                                   'transaction_submission_503s_document_exact_outcome_unknown_identity',
                                                                   'signed_transaction_reject_code_inventory_matches_runtime_metadata',
                                                                   'openapi_schemas_include_system_keys'),
 'crates/iroha_torii/src/openapi/tests/diagnostics_schemas.rs': ('pipeline_status_openapi_exposes_only_the_exact_first_release_scope',
                                                                 'npos_schema_excludes_retired_process_local_and_vrf_surfaces',
                                                                 'status_openapi_uses_only_exact_scalar_probe_paths',
                                                                 'operator_webauthn_openapi_is_closed_bounded_and_capacity_aware',
                                                                 'finality_attestation_tip_progress_openapi_matches_native_bindings'),
 'crates/iroha_torii/src/openapi/tests/fee_quote_contract.rs': ('fee_quote_decision_schema_is_an_exact_closed_payer_union',),
 'crates/iroha_torii/src/openapi/tests/finality_app_contracts.rs': ('inrou_guest_image_schema_requires_one_concrete_published_artifact',
                                                                    'inrou_first_release_openapi_matches_block_clock_and_exact_admission',
                                                                    'native_finality_schemas_are_exact_closed_and_bounded',
                                                                    'native_finality_schema_matches_executed_norito_json_and_rejects_retired_fields',
                                                                    'ledger_state_endpoints_expose_one_closed_authenticated_native_schema',
                                                                    'bridge_finality_operations_describe_current_durable_evidence',
                                                                    'signed_status_documents_actual_driver_fields',
                                                                    'current_finality_schemas_match_portable_wire_bounds',
                                                                    'generated_spec_documents_read_only_nexus_lifecycle_status',
                                                                    'generated_spec_documents_exact_current_sumeragi_status',
                                                                    'generated_spec_documents_exact_soracloud_priority_contracts',
                                                                    'generated_spec_documents_app_query_page_metadata',
                                                                    'alias_openapi_documents_optional_public_and_exact_restricted_auth',
                                                                    'protected_contract_identity_openapi_is_signed_and_exact',
                                                                    'multisig_read_auth_contract_is_path_specific',
                                                                    'scoped_artifact_openapi_binds_network_full_dataspace_and_content'),
 'crates/iroha_torii/src/openapi/tests/retail_fee_contract.rs': ('retail_validation_fee_api_contract_has_calendar_rates_and_authenticated_reads',),
 'crates/iroha_torii/src/openapi/tests/iso20022_auth.rs': ('iso20022_operations_require_fresh_operator_signatures',
                                                           'iso20022_openapi_documents_party_scope_durable_admission_and_signed_xml',
                                                           'iso20022_v2_status_and_audit_responses_are_exact_and_bounded'),
 'crates/iroha_torii/src/openapi/tests/json_value_contract.rs': ('json_value_openapi_schema_is_the_canonical_arbitrary_json_union',
                                                                 'recovered_torii_component_schemas_match_their_wire_fields'),
 'crates/iroha_torii/src/openapi/tests/prepared_account_contracts.rs': ('prepared_account_transaction_schemas_are_closed_and_exactly_typed',
                                                                        'faucet_policy_schema_is_exact_public_discovery'),
 'crates/iroha_torii/src/openapi/tests/privacy_release_qualification.rs': ('privacy_release_qualification_records_match_native_json_fields',
                                                                           'privacy_release_qualification_enums_match_native_json_tags',
                                                                           'privacy_release_qualification_inventory_counts_match_native_contract',
                                                                           'privacy_release_qualification_is_required_and_explicitly_nullable',
                                                                           'privacy_release_qualification_keeps_java_source_kotlin_as_one_of_ten_consumers'),
 'crates/iroha_torii/src/openapi/tests/private_settlement_contract.rs': ('private_settlement_openapi_covers_the_exact_catalog_family',
                                                                         'private_settlement_operations_are_typed_authenticated_and_redacted',
                                                                         'private_settlement_top_level_v1_dtos_are_strict',
                                                                         'private_settlement_tagged_v1_dtos_have_exact_closed_variants'),
 'crates/iroha_torii/src/openapi/tests/public_contract_call.rs': ('public_contract_call_schema_matches_exact_current_admission',),
 'crates/iroha_torii/src/openapi/tests/query_asset_absence_contract.rs': ('query_asset_absence_openapi_requires_the_exact_selector',),
 'crates/iroha_torii/src/openapi/tests/sns_contract.rs': ('sns_name_absence_openapi_is_typed_and_selector_bound',
                                                          'alias_errors_openapi_match_native_reports_and_bound_absence'),
 'crates/iroha_torii/src/openapi/tests/soracloud_lease_contracts.rs': ('soracloud_control_plane_openapi_exposes_authoritative_lease_accounting',
                                                                       'soracloud_openapi_uses_the_consensus_block_lease_clock',
                                                                       'soracloud_runtime_execution_host_openapi_is_first_release_exact',
                                                                       'soracloud_mailbox_and_agent_request_openapi_are_first_release_exact'),
 'crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs': ('evidence_audit_openapi_requires_and_returns_exact_cursors',
                                                              'evidence_openapi_matches_authenticated_protocol_contract',
                                                              'sorafs_pin_register_openapi_is_caller_signed_transaction_transport',
                                                              'sorafs_storage_token_openapi_requires_operator_and_diagnostic_headers',
                                                              'sorafs_storage_and_inventory_openapi_matches_authenticated_catalog',
                                                              'sorafs_pin_list_openapi_is_finalized_bounded_keyset_readback',
                                                              'sorafs_pin_manifest_openapi_is_finalized_native_readback',
                                                              'sorafs_replication_openapi_is_a_strict_chain_authoritative_v1_projection',
                                                              'moderation_dead_letter_openapi_is_typed_bounded_and_dual_control',
                                                              'hedging_billing_openapi_is_authenticated_bounded_and_private',
                                                              'proof_stream_openapi_matches_the_closed_canonical_envelope',
                                                              'sorafs_account_storage_token_declares_exact_account_auth_and_explicit_policy'),
 'crates/iroha_torii/src/openapi/tests/sorafs_pop_contracts.rs': ('sorafs_pop_openapi_requires_closed_recipient_bound_requests',
                                                                  'sorafs_pop_openapi_native_requests_match_advertised_fields',
                                                                  'sorafs_pop_openapi_digest_context_and_proof_bounds_match_native_limits'),
 'crates/iroha_torii/src/openapi/tests/vpn_da.rs': ('vpn_openapi_paths_are_typed_signed_and_use_runtime_success_statuses',
                                                    'vpn_openapi_schemas_are_strict_and_use_canonical_quantities',
                                                    'sorafs_tag_documents_exact_canonical_quantity_contract',
                                                    'tags_section_includes_push_tag',
                                                    'detached_asset_transfer_openapi_is_strict_and_two_phase',
                                                    'retired_ivm_binding_preparation_and_proof_jobs_are_absent_from_openapi',
                                                    'retired_server_contract_deployment_paths_are_absent',
                                                    'governance_mutation_openapi_is_typed_closed_and_secret_free',
                                                    'governance_digest_and_parliament_phase_schemas_are_exact',
                                                    'parliament_attempt_openapi_is_closed_authenticated_and_bounded',
                                                    'governance_read_path_parameters_publish_exact_runtime_grammars',
                                                    'subscription_mutations_publish_exact_unsigned_v1_draft_contract',
                                                    'local_signing_openapi_contracts_are_closed_and_secret_free',
                                                    'da_proof_openapi_contracts_match_exact_norito_json_wire_shapes'),
 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs': ('inner_product_owner_source_boundary_covers_every_production_caller',
                                                                                     'vector_padding_and_split_clear_replaced_allocations',
                                                                                     'random_vector_clears_success_and_partial_failure',
                                                                                     'scalar_commitment_openings_clear_on_success_error_and_unwind',
                                                                                     'vector_commitment_mask_slot_handoff_clears_on_success_and_unwind',
                                                                                     'vector_commitment_values_rehome_without_copy_or_allocation',
                                                                                     'scalar_commitment_opening_source_boundary_stays_private_and_zeroizing',
                                                                                     'polynomial_storage_fills_only_public_unassigned_slots_after_owner_moves',
                                                                                     'polynomial_storage_preserves_dense_product_and_evaluation_reference',
                                                                                     'polynomial_storage_errors_and_unwind_preserve_zeroizing_owners',
                                                                                     'polynomial_storage_completion_follows_all_commitment_moves_in_actual_prover'),
 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs': ('secret_scalar_owner_clears_constructor_and_transfer_slots',
                                                                                'proof_scalar_one_attempt_returns_only_owned_candidates',
                                                                                'random_scalar_owner_clears_success_error_and_unwind',
                                                                                'scoped_guards_clear_named_scalar_and_direct_msm_owners',
                                                                                'secret_builder_private_push_copy_handoffs_and_clears_every_exit',
                                                                                'secret_builder_source_boundaries_copy_borrows_and_handoff_owned_values',
                                                                                'secret_builder_rejects_overflow_without_reallocation_and_wipes_terms',
                                                                                'secret_builder_returned_owner_clears_on_success_and_comparison_mismatch',
                                                                                'secret_msm_point_owner_and_borrowed_publication_boundary_are_static',
                                                                                'secret_builder_matches_public_and_naive_msm_across_chunks',
                                                                                'public_two_term_straus_matches_independent_scaling_at_scalar_edges',
                                                                                'symbolic_initial_h_matches_eager_materialization_at_small_powers_of_two',
                                                                                'prover_scalar_publication_borrows_every_private_response',
                                                                                'symbolic_h_proof_bytes_are_worker_count_independent',
                                                                                'secret_chunk_fold_clears_successes_after_peer_error',
                                                                                'secret_point_owner_clears_constructor_scaled_pair_success_and_unwind',
                                                                                'secret_builder_unwind_wipes_terms_encodings_tables_and_named_points',
                                                                                'inner_product_owner_clears_success_error_length_panic_and_unwind',
                                                                                'scalar_vector_borrowed_scaled_accumulation_clears_without_copy_or_allocation',
                                                                                'scalar_vector_borrowed_product_preallocates_and_clears_every_exit',
                                                                                'output_witness_polynomial_rehome_moves_allocation_and_clears_exactly_once',
                                                                                'right_witness_polynomial_rehome_scales_without_copy_or_allocation',
                                                                                'scalar_vector_borrowed_scaled_accumulation_source_boundary',
                                                                                'secret_byte_cleanup_accounting_is_isolated_between_threads')}
ATTRIBUTE_SIGNATURE = {'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs': 'd8bb84caecce3d9dc46322b7fba4c6510a53df96d4ad7ca6f45df4d8d218c471',
 'crates/iroha_torii/src/openapi.rs': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
 'crates/iroha_torii/src/openapi/tests.rs': 'fd68bfc0a7fd23918b87ee9eaecc2bc14ec8c1009091e451d1a12c0d8ba3e41c',
 'crates/iroha_torii/src/openapi/tests/catalog_and_contracts.rs': 'afd54a58437a53ff56657fba610c58a1552124c71494ee2657a6c185b6d02411',
 'crates/iroha_torii/src/openapi/tests/diagnostics_schemas.rs': 'e9c818b7a47d03eeafa5846b96838acaca60a99594e6b125d8bc564f7d1e5d1a',
 'crates/iroha_torii/src/openapi/tests/fee_quote_contract.rs': '32dc22a816d915f8cc2dd33fc257a38306ba0061f330594d4287a45ddc604ad6',
 'crates/iroha_torii/src/openapi/tests/finality_app_contracts.rs': 'c7a4d2c750f2df0bc55fcf6390ed29eef60cd2362f6cfc34e97f0933358c8554',
 'crates/iroha_torii/src/openapi/tests/retail_fee_contract.rs': '1b46a214f2d8db475f194c3652c131f3dfee789f919c7d905e84400400d7c275',
 'crates/iroha_torii/src/openapi/tests/iso20022_auth.rs': 'b1b49d0c5d309eb98a9db7bcba90c415ad86ed10d41fb55fd9216bbf7fb2cd12',
 'crates/iroha_torii/src/openapi/tests/json_value_contract.rs': 'a0721c177ae54f8127c3f74c46c2435970238f7e49c3d1fc1d0149dfea3280e7',
 'crates/iroha_torii/src/openapi/tests/prepared_account_contracts.rs': 'c67c92d10270de49b0a2c592728382b8c8e6f4a717803e74ac4e57f501bb8152',
 'crates/iroha_torii/src/openapi/tests/privacy_release_qualification.rs': 'c70ff1c24aa1be36619c595433648aeb1ecf5dabab055c17b8e2142c4a07b5ac',
 'crates/iroha_torii/src/openapi/tests/private_settlement_contract.rs': 'da767015a4c6a6722af8a9060d9efb1f9a2ab1198c97eb187f74ecf14c718ad4',
 'crates/iroha_torii/src/openapi/tests/public_contract_call.rs': 'ff2e4ff211194e5f3688e2500aabe394515c8c9a8398862ad4bb98cab509edb6',
 'crates/iroha_torii/src/openapi/tests/query_asset_absence_contract.rs': '6c4b10417f73afe762037ed2bd96476225a438bd5a14df295a1d1bb9c82afa28',
 'crates/iroha_torii/src/openapi/tests/sns_contract.rs': 'fd7e2d02d8e03ceaa5f0d2a8ca6776d64ff29a12e581525ac8a5e11e890fbd28',
 'crates/iroha_torii/src/openapi/tests/soracloud_lease_contracts.rs': '4083f46e0a01b55d52174ecdceac2c36c6ef24237d351743a763f0f48ede7729',
 'crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs': '6a667bd8626703dbac8667fa77570df05768ba722251c68db4614eb63e9c3423',
 'crates/iroha_torii/src/openapi/tests/sorafs_pop_contracts.rs': 'a0fd8f91511aa7c7738a5d1ca63d70874ee65af0caf8cf85e730b44c03be2d5f',
 'crates/iroha_torii/src/openapi/tests/vpn_da.rs': '70b0df36a39e7566d126b5a4bd150056d66a54bdf0b0f6d933dcded447655f4b',
 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs': 'aed623324c65ab4a0fac04f1f3fc6a4e760d25242522935da4deedece7484c67',
 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs': 'b6c135b93185398bfd5dfdc95bc5331eac71cea2ecd6b8778b9634924d800d39'}
class ContractAssetError(ValueError):
    """Raised when a static contract asset fails its pinned envelope."""


def _digest(payload: bytes, algorithm: str) -> str:
    return hashlib.new(algorithm, payload).hexdigest()


def _load_asset(
    payload: bytes,
    pinned_length: int,
    pinned_digest: str,
    algorithm: str,
    expected_order: tuple[str, ...],
) -> dict[str, list[tuple[str, ...]]]:
    if len(payload) != pinned_length:
        raise ContractAssetError("asset length drift")
    if not hmac.compare_digest(_digest(payload, algorithm), pinned_digest):
        raise ContractAssetError("asset digest drift")
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ContractAssetError("asset UTF-8") from error
    if not lines or lines[0] != VERSION:
        raise ContractAssetError("asset version")
    sections: dict[str, list[tuple[str, ...]]] = {}
    order: list[str] = []
    active: str | None = None
    closed: set[str] = set()
    for line in lines[1:]:
        fields = line.split("\t")
        section_id, encoded = fields[0], fields[1:]
        if not section_id or not encoded:
            raise ContractAssetError("empty asset row")
        if section_id != active:
            if active is not None:
                closed.add(active)
            if section_id in closed:
                raise ContractAssetError("non-contiguous section")
            active = section_id
            order.append(section_id)
        row: list[str] = []
        for cell in encoded:
            if not cell or len(cell) % 2 or re.fullmatch(r"[0-9a-f]+", cell) is None:
                raise ContractAssetError("non-canonical cell hex")
            try:
                decoded = bytes.fromhex(cell).decode("utf-8")
            except (ValueError, UnicodeDecodeError) as error:
                raise ContractAssetError("invalid cell") from error
            if not decoded:
                raise ContractAssetError("empty decoded cell")
            row.append(decoded)
        sections.setdefault(section_id, []).append(tuple(row))
    if tuple(order) != expected_order or not sections:
        raise ContractAssetError("section order")
    return sections


def _rust_pin(source: str, name: str) -> str:
    match = re.search(
        rf'{name}: (?:usize|&str) =\s*(?:"([0-9a-f]{{64}})"|([\d_]+));',
        source,
    )
    if match is None:
        raise AssertionError(f"missing Rust pin {name}")
    return match.group(1) or match.group(2).replace("_", "")


def _test_inventory_and_signature(source: str) -> tuple[tuple[str, ...], str]:
    lines = source.splitlines()
    names: list[str] = []
    signatures: list[str] = []
    for index, line in enumerate(lines):
        if line.strip() != "#[test]":
            continue
        fn_index = index + 1
        while fn_index < len(lines) and re.match(r"\s*fn\s+[a-z0-9_]+\(\)", lines[fn_index]) is None:
            fn_index += 1
        if fn_index == len(lines):
            raise AssertionError("test attribute without function")
        name = re.search(r"fn\s+([a-z0-9_]+)", lines[fn_index]).group(1)
        names.append(name)
        blocks = ["#[test]"]
        cursor = index - 1
        while cursor >= 0 and lines[cursor].rstrip().endswith("]"):
            end = cursor
            while cursor >= 0 and not lines[cursor].lstrip().startswith("#["):
                cursor -= 1
            if cursor < 0:
                break
            blocks.insert(0, "\n".join(part.strip() for part in lines[cursor : end + 1]))
            cursor -= 1
        signatures.append(name + "\n" + "\n".join(blocks))
    digest = hashlib.sha256("\n--\n".join(signatures).encode()).hexdigest()
    return tuple(names), digest


OPENAPI_AUTHORITIES = (
    "artifacts/openapi/torii.json",
    "crates/iroha_torii/assets/openapi/torii.json",
    "artifacts/openapi/versions/current/torii.json",
)
RETAIL_FEE_OPERATIONS = (
    ("/v1/validation-fee/quote", "post", "quote", "RetailFeeQuoteResponseV1"),
    ("/v1/validation-fee/accounts/{account_id}/status", "get", "status", "RetailFeeStatusResponseV1"),
    ("/v1/validation-fee/accounts/{account_id}/receipts", "get", "receipts", "RetailFeeReceiptsResponseV1"),
    ("/v1/validation-fee/accounts/{account_id}/statement", "post", "statement", "RetailFeeStatementResponseV1"),
    ("/v1/validation-fee/accounts/{account_id}/statement/head", "get", "statement_head", "RetailFeeCurrentHeadResponseV1"),
)


def _validate_retail_fee_openapi(document: dict) -> None:
    """Check signed assessment, calendar and proof owners without refreshing pins."""
    def require(condition: bool, message: str) -> None:
        if not condition:
            raise ContractAssetError(message)

    paths = document["paths"]
    schemas = document["components"]["schemas"]
    require("/v1/validation-fee/hijiri/quote" not in paths, "retired quote route")
    for name in (
        "ValidationFeeHijiriQuoteRequestV1",
        "ValidationFeeHijiriQuoteResponseV1",
        "GovernanceParliamentProposalPayloadValidationFeePayoutRecipientV1",
    ):
        require(name not in schemas, "retired schema")
    for path, method, suffix, response in RETAIL_FEE_OPERATIONS:
        operation = paths[path][method]
        route_id = "validation_fee.retail." + suffix
        require(operation["x-iroha-operation"] == route_id, "operation owner")
        require(operation["x-iroha-route-auth"] == {
            "schemaVersion": 1,
            "stableRouteId": route_id,
            "authentication": "canonical_account_signature",
            "admission": "authenticated_account",
        }, "authentication owner")
        require(operation["x-iroha-tool-effect"] == "read", "read operation")
        require(operation["security"] == [
            {"IrohaCanonicalAccount": [], "IrohaCanonicalNonce": [], "IrohaCanonicalSignature": [], "IrohaCanonicalTimestampMs": []},
            {"IrohaCanonicalWitness": []},
        ], "authentication alternatives")
        require(operation["x-iroha-canonical-auth-v1"]["body_hash_bound"] is True, "signed body")
        require(operation["x-iroha-canonical-auth-v1"]["exact_request_target"] is True, "signed target")
        responses = operation["responses"]
        require(responses["200"]["content"]["application/json"]["schema"] == {
            "$ref": "#/components/schemas/" + response,
        }, "response owner")
        require(responses["200"]["headers"]["Cache-Control"]["schema"]["const"] == "private, no-store", "private response")
        require(responses["401"]["headers"]["x-iroha-reject-code"]["schema"]["enum"] == ["canonical_authentication_required"], "missing authentication")
        require(responses["403"]["headers"]["x-iroha-reject-code"]["schema"]["enum"] == ["retail_fee_account_mismatch"], "wallet authorization")
    assessment = schemas["RetailFeeAssessmentV1"]
    require(set(assessment["properties"]) == {
        "account_id", "retail_enrolled", "billing_month_start_ms", "policy_revision",
        "payments_used_before", "qualifying_payments", "fee_minor", "state_commitment",
        "intent_hash", "expires_at_ms",
    }, "complete assessment")
    require(set(assessment["required"]) == set(assessment["properties"]), "assessment required fields")
    require(assessment["additionalProperties"] is False, "assessment closed layout")
    for field in ("state_commitment", "intent_hash"):
        require(assessment["properties"][field]["type"] == "string", "fixed bytes JSON type")
        require(assessment["properties"][field]["pattern"] == "^[0-9A-F]{64}$", "fixed bytes canonical form")
    request = schemas["MultisigProposeRequest"]
    require(request["properties"]["validation_fee_assessment"] == {"oneOf": [
        {"$ref": "#/components/schemas/RetailFeeAssessmentV1"}, {"type": "null"},
    ]}, "complete optional assessment")
    require("validation_fee_assessment" not in request["required"], "optional assessment input")
    require(not set(request["properties"]) & {
        "validation_fee_policy_version", "validation_fee_policy_hash", "validation_fee_hijiri_fee_quote_hash",
        "validation_fee_instruction_index", "validation_fee_transfer_entry_index",
    }, "retired positional fee input")
    policy = schemas["GovernanceParliamentProposalPayloadValidationFeePolicyV1"]
    require(set(policy["properties"]) == {
        "schema_version", "network_id", "policy_version", "previous_policy_hash", "ds_asset_id", "ds_scale",
        "retail_schedule", "effective_from_ms", "notice_published_at_ms", "fee", "treasury_account_id",
        "charging_mode", "exemption_classes", "reward_custody",
    }, "calendar policy fields")
    require(set(policy["required"]) == set(policy["properties"]), "calendar policy required fields")
    mode = schemas["GovernanceParliamentProposalPayloadValidationFeeChargingModeV1"]
    require(mode["properties"]["charging_mode"]["const"] == "RETAIL_MONTHLY_ALLOWANCE" and mode["properties"]["value"]["type"] == "null" and set(mode["required"]) == {"charging_mode", "value"} and mode["additionalProperties"] is False, "sole charging mode")
    require("enum" not in policy["properties"]["fee"], "institutional price is governed")
    schedule = schemas["RetailFeeScheduleV1"]["properties"]
    require(schedule["maintenance_tiers"]["minItems"] == 1 and schedule["maintenance_tiers"]["maxItems"] == 32, "bounded tariff tiers")
    conversion = schemas["GovernanceParliamentProposalPayloadValidationFeePayoutBindingV1"]
    require(set(conversion["properties"]) == {
        "contract_address", "code_hash", "entrypoint", "treasury_account_id", "ds_asset_id", "xor_asset_id",
        "pool_contract_address", "pool_code_hash", "pool_vault_account_id", "reward_pool_account_id", "reference_feed_id",
        "reference_feed_config_version", "reference_provider_accounts", "max_sbd_per_attempt_minor", "max_sbd_per_day_minor",
        "min_interval_ms", "max_source_age_ms", "max_slippage_bps", "validator_lane_id", "min_reward_claim_xor_minor",
    }, "independent conversion fields")
    require(set(conversion["required"]) == set(conversion["properties"]), "conversion required fields")
    require(conversion["properties"]["max_slippage_bps"]["maximum"] == 9999, "strict slippage bound")
    providers = conversion["properties"]["reference_provider_accounts"]
    require(providers["minItems"] == providers["maxItems"] == 5 and providers["uniqueItems"] is True, "provider quorum")
    quote = paths["/v1/validation-fee/quote"]["post"]
    require(quote["requestBody"]["x-iroha-max-bytes"] == 256 * 1024, "quote body bound")
    require(schemas["RetailFeeQuoteRequestV1"]["properties"]["transfers"]["maxItems"] == 1000, "payment intent bound")
    statement = paths["/v1/validation-fee/accounts/{account_id}/statement"]["post"]
    require(statement["requestBody"]["x-iroha-max-bytes"] == 16 * 1024, "statement request bound")
    require(statement["responses"]["200"]["x-iroha-max-bytes"] == 1024 * 1024, "statement response bound")
    for field in ("state_commitment", "intent_hash"):
        require(field in assessment["required"], "assessment commitment binding")
    for schema, field in (("RetailFeeCurrentHeadResponseV1", "finality_proof"),):
        require(schemas[schema]["properties"][field] == {"$ref": "#/components/schemas/SumeragiFinalityProof"}, "native finality owner")
    receipts = schemas["RetailFeeReceiptsResponseV1"]["properties"]
    require(receipts["finality_proofs"]["items"] == {"$ref": "#/components/schemas/SumeragiFinalityProof"}, "receipt finality owner")
    require(receipts["receipt_proofs"]["items"] == {"$ref": "#/components/schemas/RetailFeeEvidenceRecordProofV1"}, "native receipt membership owner")
    record = schemas["RetailFeeEvidenceRecordV1"]["properties"]
    require(record["payload"]["properties"]["kind"]["const"] == "RetailReceipt", "matching receipt payload")
    require(record["payload"]["properties"]["value"] == {"$ref": "#/components/schemas/RetailFeeReceiptV1"}, "complete native receipt")
    for schema, field in (("FeeEvidenceWitnessProofV1", "siblings"), ("RetailFeeCurrentHeadProofV1", "head_siblings")):
        proof = schemas[schema]["properties"][field]
        require(proof["minItems"] == proof["maxItems"] == 256 and proof["items"] == {"$ref": "#/components/schemas/Hash"}, "complete sparse proof")



MULTISIG_MUTATION_DTOS = (
    ("MultisigProposeRequest", "MultisigProposeDtoV1"),
    ("MultisigApproveRequest", "MultisigApproveDto"),
    ("MultisigCancelRequest", "MultisigCancelRequestDto"),
    ("MultisigContractCallProposeRequest", "MultisigContractCallProposeDto"),
    ("MultisigContractCallApproveRequest", "MultisigContractCallApproveDto"),
)


def _validate_multisig_mutation_schemas(document: dict) -> None:
    """Keep each closed draft DTO under one owner instead of closed allOf branches."""
    for name, _ in MULTISIG_MUTATION_DTOS:
        schema = document["components"]["schemas"][name]
        if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
            raise ContractAssetError("closed flattened multisig request")
        properties = schema["properties"]
        if "multisig_account_id" not in schema["required"]:
            raise ContractAssetError("canonical draft authority required")
        if properties["multisig_account_id"] != {"$ref": "#/components/schemas/GovernanceCanonicalAccountIdV1"}:
            raise ContractAssetError("canonical draft authority schema")
        if properties["multisig_account_alias"].get("type") != "null":
            raise ContractAssetError("unsigned alias draft forbidden")
        for field in ("public_key_hex", "signature_b64", "creation_time_ms"):
            if properties[field].get("oneOf", [])[-1:] != [{"type": "null"}]:
                raise ContractAssetError("optional DTO null representation")
        if name.startswith("MultisigContractCall"):
            if not {"contract_alias", "entrypoint", "payload"} <= set(schema["required"]):
                raise ContractAssetError("complete exact contract target")
            if "contract_address" in properties or properties["payload"].get("type") != "object":
                raise ContractAssetError("canonical contract alias and object payload")



class LargeStaticContractAssetTests(unittest.TestCase):
    def test_assets_are_pinned_strict_and_fully_consumed(self) -> None:
        consumers = {path: (ROOT / path).read_text(encoding="utf-8") for path in SOURCE_PATHS}
        for name, (asset_path, source_path, algorithm, length_name, digest_name) in ASSETS.items():
            payload = (ROOT / asset_path).read_bytes()
            source = consumers[source_path]
            length = int(_rust_pin(source, length_name))
            digest = _rust_pin(source, digest_name)
            sections = _load_asset(payload, length, digest, algorithm, SECTION_ORDER[name])
            combined = "\n".join(consumers.values())
            self.assertEqual(set(sections), set(SECTION_ORDER[name]))
            for section_id in sections:
                self.assertIn(f'"{section_id}"', combined)

    def test_length_digest_version_shape_and_order_mutations_fail_closed(self) -> None:
        for name, (asset_path, _source_path, algorithm, _length_name, _digest_name) in ASSETS.items():
            payload = (ROOT / asset_path).read_bytes()
            digest = _digest(payload, algorithm)
            order = SECTION_ORDER[name]
            with self.subTest(asset=name, mutation="length"), self.assertRaises(ContractAssetError):
                _load_asset(payload + b"\n", len(payload), digest, algorithm, order)
            mutated = bytearray(payload)
            mutated[-2] ^= 1
            with self.subTest(asset=name, mutation="digest"), self.assertRaises(ContractAssetError):
                _load_asset(bytes(mutated), len(payload), digest, algorithm, order)
            lines = payload.decode().splitlines()
            hostile_payloads = [
                "WRONG_VERSION\n" + "\n".join(lines[1:]) + "\n",
                "\n".join([lines[0], lines[1].replace("\t", "", 1), *lines[2:]]) + "\n",
                "\n".join([lines[0], lines[1].rsplit("\t", 1)[0] + "\tGG", *lines[2:]]) + "\n",
            ]
            for index, hostile in enumerate(hostile_payloads):
                encoded = hostile.encode()
                with self.subTest(asset=name, mutation=f"shape-{index}"), self.assertRaises(ContractAssetError):
                    _load_asset(encoded, len(encoded), _digest(encoded, algorithm), algorithm, order)

    def test_historical_tests_attributes_and_rust_action_architecture_are_frozen(self) -> None:
        combined = ""
        for path in SOURCE_PATHS:
            source = (ROOT / path).read_text(encoding="utf-8")
            names, signature = _test_inventory_and_signature(source)
            self.assertEqual(names, TEST_INVENTORY[path], path)
            self.assertEqual(signature, ATTRIBUTE_SIGNATURE[path], path)
            self.assertNotIn("#[ignore]", source)
            combined += source
        for forbidden in (
            "Box<dyn Fn",
            "impl Fn",
            "dyn Fn",
            "ActionContract",
            "BodyContract",
            "StepContract",
            "callback",
        ):
            self.assertNotIn(forbidden, combined)
        self.assertGreaterEqual(combined.count("assert!("), 300)
        self.assertGreaterEqual(combined.count("assert_eq!("), 300)

    def test_retail_fee_authorities_bind_current_native_schema_and_routes(self) -> None:
        payloads = [(ROOT / path).read_bytes() for path in OPENAPI_AUTHORITIES]
        self.assertTrue(all(payload == payloads[0] for payload in payloads))
        _validate_retail_fee_openapi(json.loads(payloads[0]))
        mount = (ROOT / "crates/iroha_torii/src/lib.rs").read_text()
        runtime = (ROOT / "crates/iroha_torii/src/validation_fee_api.rs").read_text()
        self.assertIn("VALIDATION_FEE_RETAIL_QUOTE => canonical_account_post", mount)
        self.assertIn("VALIDATION_FEE_RETAIL_STATEMENT => canonical_account_post", mount)
        self.assertIn("proof.record.payload", runtime)
        self.assertIn("FeeEvidencePayloadV1::RetailReceipt", runtime)
        self.assertIn("proof.verify(root)", runtime)
        self.assertIn("page.verify(&request.cursor)", runtime)

    def test_multisig_propose_schema_accepts_complete_current_inputs_and_rejects_retired_forms(self) -> None:
        from jsonschema import Draft202012Validator
        document = json.loads((ROOT / OPENAPI_AUTHORITIES[0]).read_bytes())
        validator = Draft202012Validator({
            "$ref": "#/components/schemas/MultisigProposeRequest",
            "components": document["components"],
        })
        # Structural JSON controls only. Native account parsing, instruction
        # decoding, signatures, state and execution admission remain required.
        request = {
            "multisig_account_id": "canonical-account-id",
            "signer_account_id": "canonical-signer-id",
            "fee_payment": {"payer": "authority", "value": {"charge_limits": [], "gas_limit": 1000}},
            "instructions": ["AQ=="],
        }
        self.assertTrue(validator.is_valid(request))
        optional_nulls = {name: None for name in (
            "multisig_account_alias", "public_key_hex", "signature_b64", "creation_time_ms", "memo", "validation_fee_assessment",
        )}
        self.assertTrue(validator.is_valid(request | optional_nulls))
        assessment = {
            "account_id": request["multisig_account_id"], "retail_enrolled": True,
            "billing_month_start_ms": 1, "policy_revision": 1, "payments_used_before": 0,
            "qualifying_payments": 1, "fee_minor": 0, "state_commitment": "01" * 32,
            "intent_hash": "03" * 32, "expires_at_ms": 2,
        }
        self.assertTrue(validator.is_valid(request | {"validation_fee_assessment": assessment}))
        for name in ("state_commitment", "intent_hash", "policy_revision", "expires_at_ms"):
            partial = dict(assessment); del partial[name]
            with self.subTest(missing=name):
                self.assertFalse(validator.is_valid(request | {"validation_fee_assessment": partial}))
        for field in (
            "validation_fee_policy_version", "validation_fee_policy_hash", "validation_fee_hijiri_fee_quote_hash",
            "validation_fee_instruction_index", "validation_fee_transfer_entry_index", "unexpected",
        ):
            with self.subTest(retired=field):
                self.assertFalse(validator.is_valid(request | {field: "1"}))
        self.assertFalse(validator.is_valid(request | {"multisig_account_alias": "treasury@universal"}))
        self.assertFalse(validator.is_valid(request | {"multisig_account_id": None}))
        missing_id = dict(request); del missing_id["multisig_account_id"]
        self.assertFalse(validator.is_valid(missing_id))

    def test_retail_fee_schema_authority_mutations_are_rejected(self) -> None:
        document = json.loads((ROOT / OPENAPI_AUTHORITIES[0]).read_bytes())
        path = "/v1/validation-fee/quote"
        mutations = (
            ("retired route", ("paths", "/v1/validation-fee/hijiri/quote"), {}),
            ("unsigned principal", ("paths", path, "post", "security"), [{}]),
            ("unsigned body", ("paths", path, "post", "x-iroha-canonical-auth-v1", "body_hash_bound"), False),
            ("changed wallet error", ("paths", path, "post", "responses", "403", "headers", "x-iroha-reject-code", "schema", "enum"), ["other_account"]),
            ("unbounded request", ("paths", path, "post", "requestBody", "x-iroha-max-bytes"), 256 * 1024 + 1),
            ("missing signed intent", ("components", "schemas", "RetailFeeAssessmentV1", "required"), ["account_id"]),
            ("array hash alias", ("components", "schemas", "RetailFeeAssessmentV1", "properties", "intent_hash", "type"), "array"),
            ("retired positional field", ("components", "schemas", "MultisigProposeRequest", "properties", "validation_fee_policy_hash"), {"type": "string"}),
            ("retired height tariff", ("components", "schemas", "GovernanceParliamentProposalPayloadValidationFeePolicyV1", "properties", "effective_from_height"), {"type": "integer"}),
            ("retired recipients", ("components", "schemas", "GovernanceParliamentProposalPayloadValidationFeePayoutBindingV1", "properties", "recipients"), {"type": "array"}),
            ("full-loss conversion boundary", ("components", "schemas", "GovernanceParliamentProposalPayloadValidationFeePayoutBindingV1", "properties", "max_slippage_bps", "maximum"), 10000),
            ("insufficient provider quorum", ("components", "schemas", "GovernanceParliamentProposalPayloadValidationFeePayoutBindingV1", "properties", "reference_provider_accounts", "minItems"), 4),
            ("missing native finality", ("components", "schemas", "RetailFeeCurrentHeadResponseV1", "properties", "finality_proof"), {"$ref": "#/components/schemas/JsonValue"}),
            ("unrelated record", ("components", "schemas", "RetailFeeEvidenceRecordV1", "properties", "payload", "properties", "kind", "const"), "RewardClaim"),
            ("truncated sparse proof", ("components", "schemas", "RetailFeeCurrentHeadProofV1", "properties", "head_siblings", "minItems"), 255),
        )
        for name, keys, value in mutations:
            changed = copy.deepcopy(document)
            target = changed
            for key in keys[:-1]:
                target = target[key]
            target[keys[-1]] = value
            with self.subTest(mutation=name), self.assertRaises(ContractAssetError):
                _validate_retail_fee_openapi(changed)

    def test_all_five_multisig_request_schemas_match_current_flattened_dtos(self) -> None:
        from jsonschema import Draft202012Validator
        document = json.loads((ROOT / OPENAPI_AUTHORITIES[0]).read_bytes())
        _validate_multisig_mutation_schemas(document)
        source = (ROOT / "crates/iroha_torii/src/routing.rs").read_text()
        carrier_body = re.search(r"pub struct MultisigAccountSelectorDto \{(.*?)\n\}", source, re.S).group(1)
        carrier_fields = set(re.findall(r"pub (\w+):", carrier_body))
        for name, dto in MULTISIG_MUTATION_DTOS:
            schema = document["components"]["schemas"][name]
            body = re.search(r"pub struct " + dto + r" \{(.*?)\n\}", source, re.S).group(1)
            fields = set(re.findall(r"pub (\w+):", body))
            with self.subTest(schema=name):
                self.assertEqual(set(schema["properties"]), (fields - {"selector"}) | carrier_fields)
                self.assertIn("#[norito(flatten)]", body)
                self.assertNotIn("contract_address", body)
                Draft202012Validator.check_schema(schema)
        for handler in (
            "handle_post_multisig_propose", "handle_post_multisig_approve", "handle_post_multisig_cancel",
            "handle_post_contract_call_multisig_propose", "handle_post_contract_call_multisig_approve",
        ):
            body = source.split("pub async fn " + handler + "(", 1)[1].split("pub async fn ", 1)[0]
            with self.subTest(handler=handler):
                self.assertIn("reject_unverified_multisig_alias_selector(&selector)?", body)
        self.assertIn("exactly one of proposal_id or instructions_hash must be set", source)
        self.assertIn("exact_target.contract_alias != contract_alias", source)
        self.assertIn("exact_target.contract_entrypoint != entrypoint", source)
        self.assertIn("exact_target.payload != payload", source)

    def test_multisig_requests_accept_current_optional_json_and_reject_ambiguous_authority(self) -> None:
        from jsonschema import Draft202012Validator
        document = json.loads((ROOT / OPENAPI_AUTHORITIES[0]).read_bytes())
        # These are structural JSON probes. Native AccountId/instruction parsing,
        # detached signatures, live spec/state and monetary admission still run.
        base = {
            "multisig_account_id": "canonical-account-id", "signer_account_id": "canonical-signer-id",
            "fee_payment": {"payer": "authority", "value": {"charge_limits": [], "gas_limit": 1000}},
        }
        for name, _ in MULTISIG_MUTATION_DTOS:
            validator = Draft202012Validator({"$ref": "#/components/schemas/" + name, "components": document["components"]})
            request = copy.deepcopy(base)
            selector = name in ("MultisigApproveRequest", "MultisigCancelRequest", "MultisigContractCallApproveRequest")
            contract_call = name.startswith("MultisigContractCall")
            optional = ["multisig_account_alias", "public_key_hex", "signature_b64", "creation_time_ms"]
            if name == "MultisigProposeRequest":
                request["instructions"] = ["AQ=="]
                optional += ["memo", "validation_fee_assessment"]
            if selector:
                request["proposal_id"] = "ab" * 32
                optional += ["instructions_hash"]
            if contract_call:
                request.update(contract_alias="router::universal", entrypoint="ping", payload={"amount": 1})
            with self.subTest(schema=name):
                self.assertTrue(validator.is_valid(request))
                self.assertTrue(validator.is_valid(request | {field: None for field in optional}))
                self.assertTrue(validator.is_valid(request | {"public_key_hex": "01" * 32, "signature_b64": "AQ=="}))
                for field in ("unexpected", "private_key", "contract_address"):
                    self.assertFalse(validator.is_valid(request | {field: "retired-or-private"}), field)
                for field, value in (("multisig_account_id", None), ("multisig_account_id", " spaced id"),
                                     ("multisig_account_alias", "treasury@universal"), ("creation_time_ms", -1),
                                     ("creation_time_ms", 2 ** 64)):
                    self.assertFalse(validator.is_valid(request | {field: value}), (field, value))
                for field in ("public_key_hex", "signature_b64"):
                    self.assertFalse(validator.is_valid(request | {field: "non-null-without-pair"}), field)
                    other = "signature_b64" if field == "public_key_hex" else "public_key_hex"
                    self.assertFalse(validator.is_valid(request | {field: "x", other: None}), field)
                missing = dict(request); del missing["multisig_account_id"]
                self.assertFalse(validator.is_valid(missing))
                alias_only = dict(missing); alias_only["multisig_account_alias"] = "treasury@universal"
                self.assertFalse(validator.is_valid(alias_only))
                if selector:
                    by_hash = dict(request); del by_hash["proposal_id"]; by_hash["instructions_hash"] = "cd" * 32
                    self.assertTrue(validator.is_valid(by_hash))
                    self.assertTrue(validator.is_valid(by_hash | {"proposal_id": None}))
                    self.assertFalse(validator.is_valid(request | {"instructions_hash": request["proposal_id"]}))
                    self.assertFalse(validator.is_valid(request | {"instructions_hash": "cd" * 32}))
                    for value in (None, "", "AB" * 32, " " + "ab" * 32, "ab" * 31):
                        self.assertFalse(validator.is_valid(request | {"proposal_id": value}), value)
                    absent = dict(request); del absent["proposal_id"]
                    self.assertFalse(validator.is_valid(absent))
                if contract_call:
                    for field in ("contract_alias", "entrypoint", "payload"):
                        missing = dict(request); del missing[field]
                        self.assertFalse(validator.is_valid(missing), field)
                        self.assertFalse(validator.is_valid(request | {field: None}), field)
                    self.assertFalse(validator.is_valid(request | {"payload": []}))
                    self.assertFalse(validator.is_valid(request | {"payload": "opaque"}))
                    if name == "MultisigContractCallProposeRequest":
                        no_gas = copy.deepcopy(request); no_gas["fee_payment"]["value"]["gas_limit"] = None
                        self.assertFalse(validator.is_valid(no_gas))
        # Signed read schemas retain their separate, permission-checked alias
        # selection surface; changing draft composition must not retire it.
        for name in ("MultisigAccountSelector", "MultisigSpecRequest", "MultisigProposalsQueryRequest"):
            validator = Draft202012Validator({"$ref": "#/components/schemas/" + name, "components": document["components"]})
            self.assertTrue(validator.is_valid({"multisig_account_alias": "treasury@universal"}))

    def test_multisig_request_schema_authority_mutations_are_rejected(self) -> None:
        document = json.loads((ROOT / OPENAPI_AUTHORITIES[0]).read_bytes())
        for name, _ in MULTISIG_MUTATION_DTOS:
            for mutation in ("open", "missing-canonical", "alias-draft", "null-option"):
                changed = copy.deepcopy(document); schema = changed["components"]["schemas"][name]
                if mutation == "open": schema["additionalProperties"] = True
                elif mutation == "missing-canonical": schema["required"].remove("multisig_account_id")
                elif mutation == "alias-draft": schema["properties"]["multisig_account_alias"] = {"type": "string"}
                else: schema["properties"]["creation_time_ms"]["oneOf"].pop()
                with self.subTest(schema=name, mutation=mutation), self.assertRaises(ContractAssetError):
                    _validate_multisig_mutation_schemas(changed)
            if name.startswith("MultisigContractCall"):
                for mutation in ("missing-target", "retired-address", "opaque-payload"):
                    changed = copy.deepcopy(document); schema = changed["components"]["schemas"][name]
                    if mutation == "missing-target": schema["required"].remove("contract_alias")
                    elif mutation == "retired-address": schema["properties"]["contract_address"] = {"type": "string"}
                    else: schema["properties"]["payload"]["type"] = "string"
                    with self.subTest(schema=name, mutation=mutation), self.assertRaises(ContractAssetError):
                        _validate_multisig_mutation_schemas(changed)



if __name__ == "__main__":
    unittest.main()
