"""Fail-closed guard for the large static Rust test-contract asset migration."""

from __future__ import annotations

import hashlib
import hmac
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERSION = "IROHA_STATIC_CONTRACT_ROWS_V1"
MINIMUM_NET_REDUCTION = 2_000
# The compaction merge's two parents each contained 12,327 Rust lines in the
# guarded files, and its postimage contained 10,313. The current test/schema
# surface first added 1,206 lines to both sides of that honest comparison.
# Current OpenAPI/Parliament/SCCP and security-audit hardening adds another
# 1,694 lines without
# changing the three migrated contract assets or their reduction delta.
ORIGINAL_PREIMAGE_RUST_LINES = 12_327
ORIGINAL_POSTIMAGE_RUST_LINES = 10_313
PREVIOUS_TEST_SURFACE_GROWTH_RUST_LINES = 1_206
CURRENT_OPENAPI_SURFACE_GROWTH_RUST_LINES = 1_694
CURRENT_TEST_SURFACE_GROWTH_RUST_LINES = (
    PREVIOUS_TEST_SURFACE_GROWTH_RUST_LINES
    + CURRENT_OPENAPI_SURFACE_GROWTH_RUST_LINES
)
BASELINE_RUST_LINES = ORIGINAL_PREIMAGE_RUST_LINES + CURRENT_TEST_SURFACE_GROWTH_RUST_LINES
MAX_POSTIMAGE_RUST_LINES = BASELINE_RUST_LINES - MINIMUM_NET_REDUCTION
SOURCE_PATHS = (
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs',
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs',
    'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs',
    'crates/iroha_torii/src/openapi.rs',
    'crates/iroha_torii/src/openapi/tests/vpn_da.rs',
)
SOURCE_LINE_LEDGER = {
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs': 2_262,
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs': 841,
    'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs': 1_544,
    'crates/iroha_torii/src/openapi.rs': 5_816,
    'crates/iroha_torii/src/openapi/tests/vpn_da.rs': 2_749,
}
ASSETS = {
    'cleanup': ('crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_contracts_v1.txt', 'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs', 'sha3_256', 'CLEANUP_CONTRACT_ASSET_LEN', 'CLEANUP_CONTRACT_ASSET_SHA3_256'),
    'proof': ('crates/iroha_data_model/src/soracloud/tests/proof_schema_contracts_v1.txt', 'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs', 'sha256', 'PROOF_SCHEMA_CONTRACT_ASSET_LEN', 'PROOF_SCHEMA_CONTRACT_ASSET_SHA256'),
    'openapi': ('crates/iroha_torii/src/openapi/tests/openapi_static_contracts_v1.txt', 'crates/iroha_torii/src/openapi/tests/vpn_da.rs', 'sha256', 'OPENAPI_STATIC_CONTRACT_ASSET_LEN', 'OPENAPI_STATIC_CONTRACT_ASSET_SHA256'),
}
SECTION_ORDER = {
    'cleanup': (
        'secret_scalar_owner_clears_constructor_and_transfer_slots.1',
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
        'scalar_commitment_opening_source_boundary_stays_private_and_zeroizing.1',
),
    'proof': (
        'soracloud_fhe_input_admission_schema_advertises_backend.1.1',
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
        'soracloud_fhe_full_bootstrap_execution_schema_advertises_witness_digest.2.7',
),
    'openapi': (
        'openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.1',
        'openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.strings.1',
        'openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.2',
        'openapi.static_account_operations_publish_exact_auth_and_private_responses.strings.1',
        'openapi.static_account_operations_publish_exact_auth_and_private_responses.rows.1',
        'openapi.sccp_schema_serialization_excludes_retired_and_secret_fields.strings.1',
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
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.rows.1',
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.rows.2',
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.strings.1',
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.strings.2',
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.strings.3',
        'openapi.generated_spec_exposes_only_the_closed_verifier_backend_registry_v1.strings.1',
        'openapi.generated_spec_offline_typed_graph_is_closed_and_publicly_named.strings.1',
        'openapi.generated_spec_matches_offline_negotiation_and_operation_lifecycle.rows.1',
        'openapi.generated_spec_matches_offline_negotiation_and_operation_lifecycle.strings.1',
        'openapi.generated_spec_matches_offline_negotiation_and_operation_lifecycle.strings.2',
        'openapi.generated_operations_declare_tool_effects.strings.1',
        'openapi.generated_operations_declare_tool_effects.strings.2',
        'openapi.openapi_schemas_include_system_keys.strings.1',
        'openapi.generated_spec_includes_documented_paths.path_present.1',
        'openapi.generated_spec_includes_documented_paths.path_present.2',
        'openapi.generated_spec_includes_documented_paths.path_present.3',
        'openapi.generated_spec_includes_documented_paths.path_absent.4',
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
        'vpn.zk_ivm_openapi_uses_compact_state_dependent_schemas.strings.1',
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
        'openapi.transaction_admission_intent.labels',
        'openapi.offline_request.properties.1',
        'openapi.offline_request.properties.2',
        'openapi.offline_backend.labels',
        'vpn.governance_mutation.request_property_rows',
        'vpn.governance_mutation.required_field_rows',
        'vpn.vpn_openapi_schemas_are_strict_and_use_canonical_quantities.rows.6',
        'vpn.governance_read_path_parameters_publish_exact_runtime_grammars.rows.1',
        'vpn.da_proof_openapi_contracts_match_exact_norito_json_wire_shapes.rows.6',
        'openapi.generated_spec_documents_strict_typed_offline_request_schemas_and_states.integer_bounds',
    ),
}
TEST_INVENTORY = {
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs': (
        'secret_scalar_owner_clears_constructor_and_transfer_slots',
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
),
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs': (
        'inner_product_owner_source_boundary_covers_every_production_caller',
        'vector_padding_and_split_clear_replaced_allocations',
        'random_vector_clears_success_and_partial_failure',
        'scalar_commitment_openings_clear_on_success_error_and_unwind',
        'vector_commitment_mask_slot_handoff_clears_on_success_and_unwind',
        'vector_commitment_values_rehome_without_copy_or_allocation',
        'scalar_commitment_opening_source_boundary_stays_private_and_zeroizing',
),
    'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs': (
        'soracloud_fhe_public_input_schema_hashes_are_stable',
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
        'fhe_input_admission_proof_validate_rejects_open_verify_envelope_drift',
),
    'crates/iroha_torii/src/openapi.rs': (
        'openapi_authorities_have_only_resolvable_component_schema_refs',
        'package_openapi_authority_is_canonical_norito_json',
        'account_onboarding_current_state_openapi_is_one_closed_v1_observation',
        'connect_status_openapi_separates_session_and_operator_aggregate',
        'retired_apartment_execution_history_is_absent',
        'uploaded_private_model_runtime_openapi_surface_is_absent',
        'soracloud_release_openapi_matches_the_exact_closed_catalog_surface',
        'pipeline_preflight_schema_exposes_only_per_scheme_signature_batch_caps',
        'checked_openapi_assets_match_and_compiled_projection_matches_served_bytes',
        'transaction_payload_schema_requires_closed_domain_admission_and_positive_ttl',
        'authenticated_transaction_nullable_fields_are_required_and_nullable',
        'incoming_static_openapi_contracts_remain_bound_to_runtime_routes',
        'static_account_operations_publish_exact_auth_and_private_responses',
        'compiled_private_cache_contract_follows_the_route_catalog',
        'musubi_provider_bundle_attestation_and_exact_release_contract_is_static',
        'static_authority_is_the_complete_catalog_projection_with_exact_effects',
        'sccp_schema_serialization_excludes_retired_and_secret_fields',
        'sccp_ton_openapi_tracks_state_init_and_curve_neutral_wire_contract',
        'production_constants_embedded_in_openapi_remain_frozen',
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
        'generated_spec_includes_documented_paths',
        'generated_spec_documents_strict_typed_offline_request_schemas_and_states',
        'generated_spec_exposes_only_the_closed_verifier_backend_registry_v1',
        'generated_spec_offline_typed_graph_is_closed_and_publicly_named',
        'offline_json_adapter_schemas_match_actual_norito_serializers',
        'generated_spec_matches_offline_negotiation_and_operation_lifecycle',
        'musubi_v1_openapi_matches_the_complete_catalog_and_declares_models',
        'musubi_instruction_previews_discriminate_equal_payload_shapes_by_wire_id',
        'musubi_crypto_text_schemas_do_not_impose_single_key_size_limits',
        'musubi_cursor_and_ordered_prefix_bounds_match_the_wire_types',
        'musubi_chunker_text_bounds_match_the_wire_type',
        'multisig_propose_schema_exposes_optional_validation_fee_bindings_as_strings',
        'multisig_cancel_response_requires_typed_fee_payment_property',
        'multisig_propose_instruction_schema_matches_native_norito_json',
        'generated_operations_declare_tool_effects',
        'retired_sumeragi_vrf_surfaces_are_absent',
        'validation_fee_plaintext_contracts_stay_retired_and_parliament_capabilities_are_exact',
        'pipeline_fastpq_recovery_documents_operator_auth_and_bounds',
        'signed_transaction_submission_documents_exact_preadmission_contract',
        'transaction_submission_503s_document_exact_outcome_unknown_identity',
        'signed_transaction_reject_code_inventory_matches_runtime_metadata',
),
    'crates/iroha_torii/src/openapi/tests/vpn_da.rs': (
        'vpn_openapi_paths_are_typed_signed_and_use_runtime_success_statuses',
        'vpn_openapi_schemas_are_strict_and_use_canonical_quantities',
        'sorafs_tag_documents_exact_canonical_quantity_contract',
        'tags_section_includes_push_tag',
        'detached_asset_transfer_openapi_is_strict_and_two_phase',
        'zk_ivm_openapi_uses_compact_state_dependent_schemas',
        'retired_server_contract_deployment_paths_are_absent',
        'governance_mutation_openapi_is_typed_closed_and_secret_free',
        'governance_digest_and_parliament_phase_schemas_are_exact',
        'parliament_attempt_openapi_is_closed_authenticated_and_bounded',
        'governance_read_path_parameters_publish_exact_runtime_grammars',
        'subscription_mutations_publish_exact_unsigned_v1_draft_contract',
        'local_signing_openapi_contracts_are_closed_and_secret_free',
        'da_proof_openapi_contracts_match_exact_norito_json_wire_shapes',
),
}
ATTRIBUTE_SIGNATURE = {
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_tests.rs': 'a91e1c3bbf4e2512564f795b197544667aae798efb4c609a30f94853ddf9085d',
    'crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_more_tests.rs': '8a61371f2409f09729a5ccfe5ea016c79d7f2168100feab501bd9b2c218263d0',
    'crates/iroha_data_model/src/soracloud/tests/proof_schemas.rs': 'd8bb84caecce3d9dc46322b7fba4c6510a53df96d4ad7ca6f45df4d8d218c471',
    'crates/iroha_torii/src/openapi.rs': '28c7c0725d41ef027b4830971334c062f170f08f937d2eb5f19490134bda8b19',
    'crates/iroha_torii/src/openapi/tests/vpn_da.rs': '6117af48b2adb690add8256579bfdddda01db37bc04025e1b345aaa65acec8c0',
}


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

    def test_exact_rust_line_budget_and_cargo_lock_are_preserved(self) -> None:
        line_ledger = {
            path: len((ROOT / path).read_text(encoding="utf-8").splitlines())
            for path in SOURCE_PATHS
        }
        self.assertEqual(line_ledger, SOURCE_LINE_LEDGER)
        postimage = sum(line_ledger.values())
        self.assertLessEqual(postimage, MAX_POSTIMAGE_RUST_LINES)
        self.assertGreaterEqual(BASELINE_RUST_LINES - postimage, MINIMUM_NET_REDUCTION)
        self.assertLessEqual(
            max(len(line) for path in SOURCE_PATHS for line in (ROOT / path).read_text().splitlines()),
            400,
        )
        self.assertEqual(
            hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
            "d5b8bf5efbdc3ce2a8b1c0d2d75e1c5d1a343a072f836cfb76205bc6ea4cf15f",
        )


if __name__ == "__main__":
    unittest.main()
