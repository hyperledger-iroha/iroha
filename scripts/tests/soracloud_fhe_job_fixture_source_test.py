#!/usr/bin/env python3
"""Seal the callback-free Soracloud FHE job fixture consolidation."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs"
INITIAL_FIXTURE_SOURCE = SOURCE.with_name("soracloud_initial_fixture_tests.rs")
INITIAL_FIXTURE_INCLUDE = 'include!("soracloud_initial_fixture_tests.rs");'
MAXIMUM_SOURCE_LINES = 41_794

HELPER_START = 'fn deploy_diagnostic_job_test_service('
HELPER_END = 'const SORACLOUD_BFV_OPERATION_VECTOR_SET'
HELPER_SHA256 = "884bfcac5f87f392ba0ef2da08cb59459b58373562b8e26c7ee17ee23007cb36"

PROTECTED_FUNCTION_SHA256 = {
    'diagnose_fhe_job_preflight': 'bc68b9ba98e38416d2cec77cccc3dfb476173df5c02779ae13c6ca885b1962c4',
    'diagnose_fhe_input_preflight': '3d435c757e169d1e946d81902462ea20ec93a0821ce21ccdaf7c2f48b2e2a908',
    'native_stark_test_digest': 'e6ec9288e3475cafacb4b50bf4ffdd6119d16672449f62ea37669150158088b7',
    'mutate_native_stark_digest': 'c9eb54e4251c4130ea6c930d3ddfe31d013594b7a58a74787c41b83f2d698268',
    "checkpoint_soracloud_training_job_updates_authoritative_state":
        "8f2790e5ead5698a23bb77c9e9478f2040d77c1f0672c9d8b186a34f6d9aed29",
    "deploy_uploaded_model_service":
        "71d348dedc8b2d537a51d470149bd32150d50468a18d05de61bf925a13144188",
    "load_soracloud_fhe_inputs_rejects_bounded_noise_public_key_digest_mismatch":
        "16dad0e126951b8e2a2733c75802c8b0b29208b1d8f0017d3f0e88b985199e2a",
    "model_weight_lifecycle_updates_authoritative_registry_state":
        "d441d849f2eb8fc19da6a40dc66a7647e03c0bfeb370e0e23279e493ab2c47e8",
    "mutate_soracloud_state_rejects_registered_bounded_noise_binding_only_fhe_input_admission_proof":
        "28206ebfce1b3833fbbf3c691479997b786653ee176bbbc5c16b8318971d1bc5",
    "mutate_soracloud_state_rejects_registered_binding_only_fhe_input_admission_proof":
        "b138eb3732230f903b7ce680c6b16c5d45b5ff90e4eb1f46d8ff555f261165f6",
    "mutate_soracloud_state_rejects_malformed_fhe_payload_without_optional_proof":
        "0220d563651d448d16042fb042a26938e654b90b5346601095614380c53fc38c",
    "register_soracloud_model_artifact_records_authoritative_state":
        "6c3e2c57c2abacb486d7b2234985b8ff93dbfda7571e50b2b10cd711e0e768c6",
    "retry_soracloud_training_job_records_retry_pending_state":
        "40b7520a53b85e7a41e84cfcc71c6c8141008d221d6b86f6b73227026ce02eff",
    "run_fhe_input_admission_rejection_cases":
        "6805df9a3c4ddb06bbb269de2419ecca90a7815fdadc48a5189418fe523e9287",
    "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_bounded_add_output":
        "5288fca1ff5a2d0361eb6352441c0f91cad356d1ae9ecaa6c11dd980d8fa6efa",
    "run_soracloud_fhe_job_rejects_binding_only_key_proofs_without_bounded_non_add_outputs":
        "c953c127ec17d096a59f4a0d79c9e1002dee45918ea798c3ac5608ee8d182669",
    "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_exact_output":
        "4299687082094ea49c7c116f49b15150ec02866cc3a29152b4a16e0a00fc48db",
    "run_soracloud_fhe_job_rejects_all_zero_persisted_fhe_input":
        "cb856df218418f803baab74b248dc3bac48fe7a2a888b64a069a61ce126e17df",
    "run_soracloud_fhe_job_rejects_bounded_noise_persisted_fhe_input":
        "b0a197154c7627401ea267490f491399e24dbe90046b43c858cdf6e32d619b43",
    "run_soracloud_fhe_job_rejects_client_mutated_fhe_input_without_residual_metadata":
        "f5ed07833997f76c84f475ffbcd7c57a4db3406964944e260bd4f95b82eea67e",
    "run_soracloud_fhe_job_rejects_input_public_key_digest_mismatch":
        "62ba9723cfb57a5474b6abe8c9d1d11a15e070baab7b79450e27ac64070e78d2",
    "run_soracloud_fhe_job_rejects_missing_policy_bound_public_key_proof":
        "b7a908d6377193a2b1d0643dbbf28048d5205217d88d8389101edd3fae2842a9",
    "run_soracloud_fhe_job_rejects_oversized_persisted_fhe_input_envelope":
        "31369117478f193d0e5e8d71a82fcc03f17f4c0676efad4d8f05ea99ebe041a4",
    "run_soracloud_fhe_job_rejects_persisted_fhe_input_without_bound_mode":
        "ed5d92bda94a56bccb50647e839ce2768fdd8604854e7fc98fa4c2f456252dc0",
    "start_soracloud_training_job_records_authoritative_job_state":
        "9d0acc1ddb6919e3f53b8022a3d913f592d2114b25880541196065ce157d5924",
}

FIXTURE_CORRIDORS = (
    ('permissioned fixture macros', 'macro_rules! permissioned_soracloud_state {', 'macro_rules! full_bootstrap_execution_case {', '2a7dac6ff71e348d212313741057a5e698c36095c680c1e070ce0ef527b930c1'),
    ('training fixture', 'struct TrainingStartFixture {', '#[test]\nfn training_start_rejects_signed_model_and_job_text_aliases_before_mutation', 'e74c18caad6aaa28e88dc601a5dde96d0fb3eef21ddf62f54250968f6e99af08'),
)

EXPECTED_TEST_INVENTORY = (
    (('#[test]',), 'soracloud_provenance_signature_admission_rejects_malformed_ed25519_signature_r'),
    (('#[test]',), 'soracloud_provenance_signature_admission_rejects_malformed_mldsa_signature_lengths'),
    (('#[test]',), 'checked_keypair_helper_preserves_default_algorithm'),
    (('#[test]',), 'checked_keypair_helper_is_deterministic_per_call_site'),
    (('#[test]',), 'soracloud_provenance_rejects_multisig_authority_without_panicking'),
    (('#[test]',), 'soracloud_permission_allows_granted_authority'),
    (('#[test]',), 'soracloud_permission_rejects_ungranted_taira_testnet_authority'),
    (('#[test]',), 'soracloud_permission_rejects_ungranted_authority'),
    (('#[test]',), 'soracloud_permission_accepts_exact_assigned_role'),
    (('#[test]',), 'soracloud_permission_rejects_same_name_wrong_payload_direct_and_role'),
    (('#[test]',), 'soracloud_active_validator_authority_rejects_mismatched_public_lane_validator_rows'),
    (('#[test]',), 'soracloud_active_validator_authority_rejects_future_created_autoscale_lane_record'),
    (('#[test]',), 'service_runtime_mutations_require_exact_validator_placement'),
    (('#[test]',), 'bounded_noise_fhe_fixtures_bind_supported_refresh_capacity'),
    (('#[test]',), 'sample_fhe_payload_uses_fixed_first_release_identifier_width'),
    (('#[test]',), 'soracloud_fhe_proof_boundaries_reject_alternate_outer_and_wrapper_layouts'),
    (('#[test]',), 'soracloud_fhe_output_payload_is_canonical_under_alternate_ambient_layout'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_proof_families_reject_alternate_native_envelope_layout'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_proof_families_reject_alternate_verifier_key_layouts'),
    (('#[test]',), 'governed_full_bootstrap_verifier_artifact_rejects_each_alternate_nested_layout'),
    (('#[test]',), 'soracloud_fhe_stark_native_envelope_preflight_rejects_text_placeholders'),
    (('#[test]',), 'projected_binding_state_total_bytes_rejects_inconsistent_or_overflowing_totals'),
    (('#[test]',), 'fhe_input_admission_envelope_rejects_noncanonical_open_verify_shape'),
    (('#[test]',), 'fhe_input_admission_envelope_rejects_public_input_shape_replay'),
    (('#[test]',), 'fhe_bootstrap_key_envelope_rejects_public_input_shape_replay'),
    (('#[test]',), 'soracloud_fhe_proof_envelopes_reject_all_zero_native_stark_payloads'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_envelope_rejects_wrapper_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_execution_native_air_uses_crypto_domain_and_base_label'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_binding_air_reconstructs_its_typed_composition_root'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_boundary_authenticates_distinct_commitment_domains'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_builder_binds_arithmetic_trace_rows'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_boundary_rejects_private_row_openings'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_boundary_rejects_governed_trace_opening_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_boundary_rejects_malformed_opening_shapes'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_execution_bfv_native_air_rejects_wrapper_statement_retarget'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'full_bootstrap_bfv_native_air_boundary_runs_before_dedicated_verifier_error'),
    (('#[test]',), 'fhe_input_admission_proof_binds_actual_payload_metadata'),
    (('#[test]',), 'fhe_input_admission_bounded_proof_rejects_exact_mode_replay'),
    (('#[test]',), 'fhe_input_admission_backend_requires_attachment_bindings_before_verifier_lookup'),
    (('#[test]',), 'fhe_input_admission_backend_rejects_verifier_record_gas_schedule_drift'),
    (('#[test]', '#[allow(clippy::too_many_lines)]'), 'soracloud_fhe_governance_enforces_exact_scope_and_monotonic_lifecycle'),
    (('#[test]',), 'soracloud_bfv_operation_vectors_match_shared_fixture'),
    (('#[test]',), 'soracloud_fhe_policy_rejects_wrong_evaluation_key_digest'),
    (('#[test]',), 'soracloud_fhe_policy_rejects_wrong_refresh_transcript_digest'),
    (('#[test]',), 'soracloud_fhe_policy_binds_public_key_proof_statement_digest'),
    (('#[test]',), 'soracloud_fhe_public_key_proof_rejects_missing_and_mismatched_policy_bound_proof'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]'), 'soracloud_fhe_public_key_proof_rejects_registered_binding_only_proof'),
    (('#[test]',), 'soracloud_fhe_policy_binds_refresh_transcript_mode'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_is_required_for_bootstrap_jobs'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_requires_policy_statement_digest_for_bootstrap_jobs'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_rejects_non_bootstrap_jobs_before_decoding'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_rejects_statement_hash_mismatch'),
    (('#[cfg(feature = "zk-stark")]', '#[test]', '#[allow(clippy::too_many_lines)]'), 'soracloud_fhe_input_and_bootstrap_proofs_reject_native_air_binding_drift'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]', '#[allow(clippy::too_many_lines)]'), 'soracloud_fhe_input_and_bootstrap_guarded_verifiers_reject_native_air_drift'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]'), 'soracloud_fhe_base_guarded_verifiers_reject_wrong_circuit_stark_vk_payload'),
    (('#[cfg(feature = "zk-preverify")]', '#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_non_bootstrap_jobs_before_decoding'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_artifacts_outside_full_bootstrap_context'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_is_required_for_full_bootstrap_outputs'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_preflight_rejects_non_exact_proof_count'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_multi_count_before_statement_derivation'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_surplus_and_reordered_slot_proofs'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_statement_hash_mismatch'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_bound_mode_replay'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_attachment_metadata_drift'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_input_metadata_shape_drift'),
    (('#[test]',), 'governed_full_bootstrap_execution_verifier_key_rejects_artifact_bundle_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_canonicalizes_native_metadata_payload'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_opaque_stark_payload'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_below_floor_stark_payload'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_backend_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_field_count_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_trace_profile_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_air_constraint_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_generated_body_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_native_profile_label_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_core_stark_payload'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'governed_full_bootstrap_execution_verifier_key_rejects_wrong_circuit_native_payload'),
    (('#[test]', '#[allow(clippy::too_many_lines)]'), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_verifier_record_metadata_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_empty_input_slots'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_output_slot_count_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_wrong_governed_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_role_spliced_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_stale_proof_key_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_stale_transcript_public_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_wrong_circuit_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_zero_statement_hash'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_hash_only_entrypoint_stays_fail_closed_for_valid_shape'),
    (('#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_emits_valid_native_air_proof'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_arithmetic_prover_binds_claims_while_production_qualification_is_unavailable'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_bounded_arithmetic_prover_binds_claims_while_production_qualification_is_unavailable'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_air_root_query_downgrade'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_arithmetic_prover_rejects_output_or_bound_drift'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_role_spliced_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_stale_proof_key_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_untrusted_or_stale_package'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_malformed_evaluation_key_context'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_audited_prover_rejects_wrong_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_unconverted_native_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_unbound_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_stale_prefix_trace_against_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_stale_galois_key_set'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_stale_galois_key_set_digest'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_stale_proof_key_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_prover_rejects_stale_air_binding'),
    (('#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_emits_native_air_proofs'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_helper_rejects_unconverted_native_verifier_key'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_release_prover_rejects_role_spliced_artifacts'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_accepts_bfv_native_zero_composition_air_active_verifier'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_bfv_native_air_when_stark_disabled'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_proof_rejects_bfv_native_air_over_stark_proof_cap'),
    (('#[cfg(feature = "zk-preverify")]', '#[test]'), 'soracloud_fhe_full_bootstrap_execution_guarded_verifier_rejects_invalid_native_air'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_envelope_uses_bootstrap_attachment_context'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_backend_uses_bootstrap_attachment_context'),
    (('#[test]',), 'soracloud_fhe_base_attachments_require_canonical_bfv_backend'),
    (('#[test]',), 'soracloud_fhe_full_bootstrap_execution_attachment_requires_canonical_bfv_backend'),
    (('#[test]',), 'soracloud_fhe_bootstrap_key_proof_rejects_unverified_fake_proof'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_fhe_bootstrap_key_proof_rejects_registered_binding_only_proof'),
    (('#[test]', '#[allow(clippy::too_many_lines)]'), 'soracloud_fhe_bootstrap_key_proof_rejects_verifier_record_metadata_drift'),
    (('#[test]', '#[ignore = "prints refreshed Soracloud BFV operation-vector fixture rows"]'), 'print_soracloud_bfv_operation_vectors'),
    (('#[test]',), 'registered_bfv_key_switch_decomposition_chain_uses_target_limb_prefix'),
    (('#[test]',), 'soracloud_bounded_noise_add_matches_plaintext_slots_and_output_bound'),
    (('#[test]',), 'soracloud_bounded_noise_multiply_uses_target_limb_key_switch_bridge'),
    (('#[test]',), 'soracloud_bounded_noise_outer_rotate_uses_target_limb_refresh_bridge'),
    (('#[test]',), 'soracloud_bounded_noise_packed_rotate_uses_target_limb_key_switch_bridge'),
    (('#[test]',), 'soracloud_bounded_noise_bootstrap_uses_registered_rns_basis_extension_refresh_bridge'),
    (('#[test]',), 'soracloud_bounded_noise_bootstrap_full_mode_rejects_multi_count_before_refresh_capacity'),
    (('#[test]',), 'soracloud_multi_input_add_matches_plaintext_slots'),
    (('#[test]',), 'soracloud_multi_input_multiply_matches_plaintext_slots'),
    (('#[test]',), 'soracloud_multi_input_multiply_rejects_underdeclared_depth'),
    (('#[test]',), 'soracloud_fhe_job_residual_metadata_tracks_non_multiply_operations'),
    (('#[test]',), 'soracloud_fhe_job_residual_metadata_rejects_over_capacity_add'),
    (('#[test]',), 'soracloud_fhe_job_residual_metadata_rejects_bootstrap_count_above_evaluation_budget'),
    (('#[test]',), 'soracloud_fhe_job_residual_metadata_tracks_multiply_output'),
    (('#[test]',), 'soracloud_fhe_job_multiply_metadata_preflights_bounds_before_arity'),
    (('#[test]',), 'soracloud_multi_input_fold_rejects_malformed_late_operand'),
    (('#[test]',), 'soracloud_bootstrap_uses_refresh_key'),
    (('#[test]',), 'soracloud_bootstrap_rejects_missing_refresh_key'),
    (('#[test]',), 'soracloud_bootstrap_rejects_refresh_count_above_evaluation_budget'),
    (('#[test]',), 'soracloud_bootstrap_full_mode_rejects_multi_count_before_refresh_capacity'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'soracloud_full_bootstrap_runtime_requires_policy_pinned_release_audit'),
    (('#[test]',), 'soracloud_refresh_only_runtime_rejects_full_bootstrap_artifact_attachment'),
    (('#[test]',), 'soracloud_rotate_left_uses_rotation_key_refresh'),
    (('#[test]',), 'soracloud_rotate_left_rejects_outer_slot_full_cycle_noop'),
    (('#[test]',), 'soracloud_packed_rotate_left_uses_galois_key_switch'),
    (('#[test]',), 'soracloud_packed_rotate_left_supports_galois_mask_schedule'),
    (('#[test]',), 'soracloud_packed_rotate_left_rejects_missing_galois_key_without_outer_fallback'),
    (('#[test]',), 'soracloud_rotate_left_rejects_missing_rotation_key'),
    (('#[test]',), 'soracloud_fhe_job_rejects_mismatched_ciphertext_slot_counts'),
    (('#[test]',), 'soracloud_fhe_job_rejects_empty_ciphertext_envelope'),
    (('#[test]',), 'soracloud_fhe_job_rejects_missing_input_envelopes'),
    (('#[test]',), 'soracloud_fhe_job_rejects_operation_shape_bypasses_before_evaluation'),
    (('#[test]',), 'soracloud_fhe_job_rejects_malformed_ciphertext_slot_coefficients'),
    (('#[test]',), 'soracloud_multiply_rejects_malformed_relinearization_key'),
    (('#[test]',), 'soracloud_rotate_left_rejects_malformed_rotation_refresh_key'),
    (('#[test]',), 'soracloud_bootstrap_rejects_malformed_refresh_key'),
    (('#[test]',), 'soracloud_registered_bfv_parameters_reject_digest_mismatch'),
    (('#[test]',), 'soracloud_registered_bfv_parameters_accept_shared_governance_fixture'),
    (('#[test]',), 'soracloud_registered_bfv_parameters_reject_descriptor_drift'),
    (('#[test]',), 'fhe_job_provenance_binds_exact_policy_reference'),
    (('#[test]',), 'fhe_job_provenance_binds_public_key_proof_option'),
    (('#[test]',), 'fhe_job_provenance_binds_bootstrap_key_proof_option'),
    (('#[test]',), 'training_model_text_helpers_reject_rewrites_and_preserve_free_form_reasons'),
    (('#[test]',), 'consensus_hf_repo_parser_and_source_id_reject_alias_spellings'),
    (('#[test]',), 'inrou_v1_host_selection_requires_exact_trusted_guest_and_physical_capacity'),
    (('#[test]',), 'inrou_host_reservation_arithmetic_fails_closed_on_overflow'),
    (('#[test]',), 'inrou_capacity_rejects_storage_reservation_arithmetic_overflow'),
    (('#[test]',), 'prorated_window_fee_preserves_subnano_decimal_scale'),
    (('#[test]',), 'prorated_window_fee_rejects_zero_lease_term'),
    (('#[test]',), 'prorated_window_fee_rejects_decimal_domain_overflow'),
    (('#[test]',), 'divide_quantity_by_member_count_rejects_zero_members'),
    (('#[test]',), 'next_soracloud_audit_sequence_includes_hf_shared_lease_events'),
    (('#[test]',), 'inrou_host_advertise_rejects_zero_fields_before_signature_verification'),
    (('#[test]',), 'inrou_host_advertise_records_exact_active_peer_binding'),
    (('#[test]',), 'inrou_host_advertise_requires_exact_active_peer_binding'),
    (('#[test]',), 'inrou_host_advertise_accepts_the_independent_peer_in_the_active_record'),
    (('#[test]',), 'hf_shared_lease_audit_sequence_exhaustion_fails_before_authoritative_writes'),
    (('#[test]',), 'hf_revision_parser_rejects_mutable_and_noncanonical_references'),
    (('#[test]',), 'leave_hf_shared_lease_last_member_uses_configured_drain_grace'),
    (('#[test]',), 'hf_shared_lease_registration_does_not_create_runtime_service'),
    (('#[test]',), 'renew_hf_shared_lease_active_window_queues_next_window'),
    (('#[test]',), 'active_inrou_resolver_rejects_dual_revision_rollout_and_keeps_retained_revision_inactive'),
    (('#[test]',), 'active_inrou_resolver_fails_closed_when_any_lease_volume_expires'),
    (('#[test]',), 'active_inrou_resolver_requires_exact_admitted_lease_volume_economics'),
    (('#[test]',), 'inrou_placement_retention_is_fail_stop_with_exact_same_lease_identity'),
    (('#[test]',), 'inrou_reconciliation_keeps_same_lease_host_sticky_and_reassigns_only_new_lease'),
    (('#[test]',), 'set_inrou_replica_runtime_state_rejects_missing_placement'),
    (('#[test]',), 'clear_inrou_replica_runtime_state_removes_exact_stale_state_without_placement'),
    (('#[test]',), 'set_inrou_replica_runtime_state_records_matching_placement'),
    (('#[test]',), 'set_inrou_replica_runtime_state_rejects_zero_version_and_timestamp'),
    (('#[test]',), 'set_inrou_replica_runtime_state_rejects_unadmitted_bundle_hash_atomically'),
    (('#[test]',), 'set_inrou_replica_runtime_state_rejects_non_assigned_validator'),
    (('#[test]',), 'set_inrou_replica_runtime_state_rejects_mismatched_placement_fields'),
    (('#[test]',), 'clear_inrou_replica_runtime_state_rejects_non_assigned_validator'),
    (('#[test]',), 'deploy_soracloud_service_records_bundle_and_audit_state'),
    (('#[test]',), 'soracloud_audit_sequence_exhaustion_fails_service_and_app_mutations_atomically'),
    (('#[test]',), 'runtime_receipt_sequence_is_ledger_owned_and_cannot_be_poisoned'),
    (('#[test]',), 'ordered_mailbox_admission_fails_closed_without_consensus_reexecution'),
    (('#[test]',), 'soracloud_app_infra_mutation_preconditions_are_signed_atomic_compare_and_set'),
    (('#[test]',), 'soracloud_service_mutation_preconditions_are_signed_atomic_compare_and_set'),
    (('#[test]',), 'inrou_placement_target_allowlist_matches_exact_validator_and_peer'),
    (('#[test]',), 'inrou_host_admission_uses_the_active_record_for_distinct_taira_key_roles'),
    (('#[test]',), 'inrou_reconciliation_counts_only_allowlisted_capable_validators'),
    (('#[test]',), 'inrou_deploy_reconciles_a_preexisting_host_advert_atomically'),
    (('#[test]',), 'retained_inrou_placement_requires_same_trusted_guest_artifact'),
    (('#[test]',), 'inrou_reconciliation_excludes_inactive_validator_with_live_capability'),
    (('#[test]',), 'inrou_peer_rotation_prunes_stale_capability_and_stops_sticky_placement_immediately'),
    (('#[test]',), 'inrou_reconciliation_prunes_expired_host_capability'),
    (('#[test]',), 'deploy_soracloud_service_rejects_missing_replica_private_http_service_data_volume'),
    (('#[test]',), 'initial_executor_soracloud_lease_usage_and_runtime_preserve_exact_assignment'),
    (('#[test]',), 'lease_volume_mutation_fails_before_revision_admission'),
    (('#[test]',), 'service_lease_usage_is_reporter_scoped_exact_and_replay_safe'),
    (('#[test]',), 'deploy_soracloud_service_accepts_required_inline_materials'),
    (('#[test]',), 'service_material_generation_exhaustion_fails_closed'),
    (('#[test]',), 'upgrade_soracloud_service_starts_canary_rollout'),
    (('#[test]',), 'upgrade_inrou_service_rejects_partial_canary_before_revision_admission'),
    (('#[test]',), 'atomic_inrou_upgrade_preserves_the_exact_same_lease_placement'),
    (('#[test]',), 'advance_rollout_rejects_inrou_even_if_active_rollout_state_is_present'),
    (('#[test]',), 'build_rollout_state_rejects_out_of_range_canary_percent'),
    (('#[test]',), 'rollout_step_requires_branch_specific_explicit_promotion_target'),
    (('#[test]',), 'unhealthy_rollout_auto_rolls_back_to_baseline'),
    (('#[test]',), 'upgrade_inrou_service_rejects_execution_plane_and_runtime_change'),
    (('#[test]',), 'rollback_soracloud_service_reuses_admitted_revision'),
    (('#[test]',), 'rollback_soracloud_service_rejects_retained_revision_with_changed_identity'),
    (('#[test]',), 'fhe_production_routes_refuse_before_payload_work_without_state_changes'),
    (('#[test]',), 'fhe_rollback_refusal_preserves_current_deployment_and_audit'),
    (('#[test]',), 'fhe_retirement_preserves_authenticated_state_and_secret_cleanup'),
    (('#[test]',), 'mutate_soracloud_state_records_authoritative_service_state'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_missing_policy_bound_public_key_proof'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_input_public_key_digest_mismatch'),
    (('#[test]',), 'load_soracloud_fhe_inputs_rejects_bounded_noise_public_key_digest_mismatch'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_all_zero_persisted_fhe_input'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]'), 'run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_exact_output'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]'), 'run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_bounded_add_output'),
    (('#[cfg(all(feature = "zk-stark", feature = "zk-preverify"))]', '#[test]'), 'run_soracloud_fhe_job_rejects_binding_only_key_proofs_without_bounded_non_add_outputs'),
    (('#[test]',), 'mutate_soracloud_state_rejects_malformed_fhe_payload_without_optional_proof'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_client_mutated_fhe_input_without_residual_metadata'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_persisted_fhe_input_without_bound_mode'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_bounded_noise_persisted_fhe_input'),
    (('#[test]',), 'run_soracloud_fhe_job_rejects_oversized_persisted_fhe_input_envelope'),
    (('#[test]',), 'mutate_soracloud_state_rejects_bounded_noise_fhe_input_admission_proof_without_registered_verifier'),
    (('#[test]',), 'mutate_soracloud_state_rejects_fhe_input_admission_proof_without_registered_verifier'),
    (('#[test]',), 'mutate_soracloud_state_rejects_oversized_fhe_input_admission_envelope'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'mutate_soracloud_state_rejects_registered_binding_only_fhe_input_admission_proof'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'mutate_soracloud_state_rejects_registered_bounded_noise_binding_only_fhe_input_admission_proof'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_circuit'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_version'),
    (('#[cfg(feature = "zk-stark")]', '#[test]'), 'mutate_soracloud_state_rejects_restored_fhe_input_verifier_metadata_drift'),
    (('#[test]',), 'record_soracloud_decryption_request_persists_policy_snapshot'),
    (('#[test]',), 'training_start_rejects_signed_model_and_job_text_aliases_before_mutation'),
    (('#[test]',), 'start_soracloud_training_job_records_authoritative_job_state'),
    (('#[test]',), 'checkpoint_soracloud_training_job_updates_authoritative_state'),
    (('#[test]',), 'retry_soracloud_training_job_records_retry_pending_state'),
    (('#[test]',), 'register_soracloud_model_artifact_records_authoritative_state'),
    (('#[test]',), 'model_weight_lifecycle_updates_authoritative_registry_state'),
    (('#[test]',), 'rollback_soracloud_model_weight_updates_authoritative_registry_state'),
    (('#[test]',), 'soracloud_uploaded_model_register_uses_approved_sorafs_pin_without_storing_chunks'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_missing_pending_or_retired_sorafs_pin'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_adversarial_sorafs_pin_metadata'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_malformed_identifiers'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_signed_padded_identifiers_without_state_rewrite'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_zero_storage_metadata'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_malformed_bundle_manifest_fields'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_disallowed_service_plane'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_nexus_limit_overrides'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_tampered_signed_bundle'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_tampered_signed_storage_reference'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_provenance_signer_mismatch'),
    (('#[test]',), 'soracloud_uploaded_model_register_rejects_duplicate_model_version'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_uses_sorafs_pin_metadata_without_chunks'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_unregistered_bundle'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_tampered_bundle_root'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_tampered_signed_payload'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_malformed_identifiers'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_duplicate_release_across_model_names'),
    (('#[test]',), 'soracloud_uploaded_model_finalize_rejects_pin_metadata_changed_after_register'),
)

REQUIRED_HELPER_TOKENS = (
    "isi::DeploySoracloudService",
    "record_diagnostic_fhe_input(",
    "SORA_SERVICE_STATE_ENTRY_VERSION_V1",
    "SoraStateEncryptionV1::FheCiphertext",
    "fhe_public_key_digest: public_key_digest",
    "fhe_residual_multiple_bound: residual_bound",
    "fhe_bound_mode: bound_mode",
    "last_update_sequence,",
    "Hash::new(governance_tag)",
    "sample_governed_fhe_material(",
    "install_diagnostic_governed_fhe_material(",
    "sample_fhe_param_set()",
    "fhe_job_provenance(",
    "full_bootstrap_execution_proofs.to_vec()",
)

FORBIDDEN_HELPER_TOKENS = (
    "Box<dyn Fn",
    "dyn Fn",
    "impl Fn",
    "FnMut",
    "FnOnce",
    "macro_rules!",
    "$body",
    "$setup",
    "enum Step",
    "enum Scenario",
    "run_case(",
)


class GuardError(AssertionError):
    """Raised when the protected source contract drifts."""


def _sha256(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()


def _normalized_hash(source: str) -> str:
    return _sha256(re.sub(r"\s+", " ", source).strip())


def _skip_rust_non_code(source: str, index: int) -> int | None:
    if source.startswith("//", index):
        end = source.find("\n", index)
        return len(source) if end < 0 else end
    if source.startswith("/*", index):
        depth = 1
        cursor = index + 2
        while cursor < len(source):
            if source.startswith("/*", cursor):
                depth += 1
                cursor += 2
            elif source.startswith("*/", cursor):
                depth -= 1
                cursor += 2
                if depth == 0:
                    return cursor
            else:
                cursor += 1
        return len(source)
    for prefix in ("br", "r"):
        if source.startswith(prefix, index):
            cursor = index + len(prefix)
            while cursor < len(source) and source[cursor] == "#":
                cursor += 1
            if cursor < len(source) and source[cursor] == '"':
                hashes = cursor - index - len(prefix)
                terminator = '"' + "#" * hashes
                end = source.find(terminator, cursor + 1)
                return len(source) if end < 0 else end + len(terminator)
    if source[index : index + 1] not in {'"', "'"}:
        return None
    quote = source[index]
    cursor = index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    return len(source)


def _matching_brace(source: str, opening: int) -> int:
    depth = 1
    cursor = opening + 1
    while cursor < len(source):
        skipped = _skip_rust_non_code(source, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        if source[cursor] == "{":
            depth += 1
        elif source[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise GuardError("unterminated protected Rust function")


def _function(source: str, name: str) -> str:
    matches = list(re.finditer(rf"(?m)^\s*fn\s+{re.escape(name)}\b", source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected exactly one function")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise GuardError(f"{name}: missing function body")
    return source[matches[0].start() : _matching_brace(source, opening) + 1]


def _test_inventory(source: str) -> tuple[tuple[tuple[str, ...], str], ...]:
    inventory: list[tuple[tuple[str, ...], str]] = []
    attributes: list[str] = []
    for line in source.splitlines():
        stripped = line.strip()
        if stripped.startswith("#["):
            attributes.append(stripped)
            continue
        match = re.match(r"(?:async\s+)?fn\s+([A-Za-z0-9_]+)\b", stripped)
        if match and any(
            attribute == "#[test]" or attribute.startswith("#[tokio::test")
            for attribute in attributes
        ):
            inventory.append((tuple(attributes), match.group(1)))
        if stripped:
            attributes = []
    return tuple(inventory)


def _expanded_test_source(source: str, initial_fixture: str) -> str:
    if source.count(INITIAL_FIXTURE_INCLUDE) != 1:
        raise GuardError("Soracloud initial fixture include changed")
    return source.replace(INITIAL_FIXTURE_INCLUDE, initial_fixture, 1)


def validate_module_owner(owner: str) -> None:
    pattern = r'#\[cfg\(test\)\]\s*mod tests\s*\{\s*use iroha_model_base::domain::DomainId;\s*use iroha_model_base::peer::PeerId;\s*include!\("soracloud_tests.rs"\);\s*mod agent_apartment;\s*\}'
    if len(re.findall(pattern, owner)) != 1 or owner.count('include!("soracloud_tests.rs")') != 1:
        raise GuardError("Soracloud current test leaf lost its compiled cfg(test) owner")


def validate_source(source: str, initial_fixture: str) -> None:
    expanded_source = _expanded_test_source(source, initial_fixture)
    if len(expanded_source.splitlines()) > MAXIMUM_SOURCE_LINES:
        raise GuardError("Soracloud test owner exceeded its source budget")
    if _test_inventory(expanded_source) != EXPECTED_TEST_INVENTORY:
        raise GuardError(
            "Soracloud current ordered test and feature inventory changed"
        )
    if source.count(HELPER_START) != 1 or source.count(HELPER_END) != 1:
        raise GuardError("Soracloud typed helper corridor markers changed")
    start = source.index(HELPER_START)
    end = source.index(HELPER_END, start)
    helper = source[start:end]
    if _normalized_hash(helper) != HELPER_SHA256:
        raise GuardError("Soracloud typed FHE helper corridor changed")
    for token in REQUIRED_HELPER_TOKENS:
        if token not in helper:
            raise GuardError(f"Soracloud helper lost {token!r}")
    for token in FORBIDDEN_HELPER_TOKENS:
        if token in helper:
            raise GuardError(f"Soracloud helper gained escape hatch {token!r}")
    for label, start, end, expected in FIXTURE_CORRIDORS:
        if source.count(start) != 1 or source.count(end) != 1:
            raise GuardError(f"{label}: fixture ownership changed")
        region = source[source.index(start):source.index(end, source.index(start))]
        if _normalized_hash(region) != expected:
            raise GuardError(f"{label}: typed fixture inputs or execution changed")
    for name, expected in PROTECTED_FUNCTION_SHA256.items():
        if _normalized_hash(_function(source, name)) != expected:
            raise GuardError(f"{name}: protected operation/assertion sequence changed")


class SoracloudFheJobFixtureSourceTest(unittest.TestCase):
    def test_current_module_owner_and_redirect_controls(self) -> None:
        owner = (ROOT / "crates/iroha_core/src/smartcontracts/isi/soracloud.rs").read_text()
        validate_module_owner(owner)
        for old, new in (
            ('include!("soracloud_tests.rs")', 'include!("uncompiled_tests.rs")'),
            ('#[cfg(test)]\nmod tests', '#[cfg(any())]\nmod tests'),
        ):
            self.assertEqual(owner.count(old), 1)
            with self.subTest(target=old), self.assertRaises(GuardError):
                validate_module_owner(owner.replace(old, new, 1))

    def test_source_contract(self) -> None:
        validate_source(
            SOURCE.read_text(),
            INITIAL_FIXTURE_SOURCE.read_text(),
        )

    def test_diagnostic_and_shared_fixture_mutations_fail_closed(self) -> None:
        source = SOURCE.read_text()
        initial_fixture = INITIAL_FIXTURE_SOURCE.read_text()
        for old, new in (
            ('production FHE upsert remains unavailable during metadata diagnostics', 'FHE upsert accepted'),
            ('assert_invalid_parameter_contains(error, "soracloud_fhe_unavailable")', 'assert_invariant_contains(error, "soracloud_fhe_unavailable")'),
            ('words[0] ^= 1', 'words[0] ^= 0'),
            ('GoldilocksDigest384V1::new([word; 6])', 'GoldilocksDigest384V1::new([0; 6])'),
            ('worker_group_size: 4,', 'worker_group_size: 0,'),
        ):
            self.assertIn(old, source)
            if old.startswith("assert_invalid_parameter_contains(error,"):
                original = _function(source, "diagnose_fhe_input_preflight")
                self.assertIn(old, original)
                mutated = source.replace(original, original.replace(old, new, 1), 1)
            else:
                mutated = source.replace(old, new, 1)
            with self.subTest(target=old), self.assertRaises(GuardError):
                validate_source(mutated, initial_fixture)

    def test_mutations_fail_closed(self) -> None:
        source = SOURCE.read_text()
        initial_fixture = INITIAL_FIXTURE_SOURCE.read_text()
        mutations = (
            source.replace(
                'b"gov-fhe-missing-bound"', 'b"gov-fhe-mutated-bound"', 1
            ),
            source.replace(
                "fhe_bound_mode: bound_mode", "fhe_bound_mode: None", 1
            ),
            source.replace(
                "last_update_sequence,", "last_update_sequence: 0,", 1
            ),
            source.replace(
                'fn record_diagnostic_fhe_input(',
                'fn record_fhe_job_test_input_removed(',
                1,
            ),
            source.replace(
                "run_soracloud_fhe_job_rejects_all_zero_persisted_fhe_input",
                "run_soracloud_fhe_job_accepts_all_zero_persisted_fhe_input",
                1,
            ),
            source.replace(
                "soracloud_fhe_public_key_proof_rejects_registered_binding_only_proof",
                "soracloud_fhe_public_key_proof_accepts_registered_binding_only_proof",
                1,
            ),
            source.replace("#[test]", "#[ignore]", 1),
            source.replace(
                HELPER_END, 'fn callback_escape(_f: impl Fn()) {}\n' + HELPER_END, 1
            ),
            source.replace(
                "bounded-noise FHE inputs must fail closed for the exact evaluator",
                "bounded-noise FHE inputs may pass",
                1,
            ),
            source + "\n" * (MAXIMUM_SOURCE_LINES - len(source.splitlines()) + 1),
        )
        for mutation in mutations:
            with self.subTest(digest=_sha256(mutation)[:12]):
                with self.assertRaises(GuardError):
                    validate_source(mutation, initial_fixture)

        mutated_initial_fixture = initial_fixture.replace(
            "soracloud_provenance_signature_admission_rejects_malformed_ed25519_signature_r",
            "soracloud_provenance_signature_admission_accepts_malformed_ed25519_signature_r",
            1,
        )
        with self.subTest(digest=_sha256(mutated_initial_fixture)[:12]):
            with self.assertRaises(GuardError):
                validate_source(source, mutated_initial_fixture)


if __name__ == "__main__":
    unittest.main()
