//! Soracloud manifest schema for deterministic service hosting on Sora3.
//!
//! The initial Soracloud release uses canonical Norito payloads:
//! [`SoraContainerManifestV1`], [`SoraServiceManifestV1`],
//! [`SoraStateBindingV1`], [`AgentApartmentManifestV1`], [`FheParamSetV1`],
//! and [`FheExecutionPolicyV1`]. Together they describe executable bundles,
//! deployment/routing policy, state mutation limits, agent-policy envelopes,
//! and deterministic confidential-compute policy in a form suitable for
//! validator admission and audit trails.
#![allow(clippy::module_name_repetitions)]
use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    proof::ProofAttachment,
    sorafs::pin_registry::{
        MANIFEST_ROOT_CID_LENGTH, ManifestDigest, ManifestRootCid, StorageClass,
    },
    zk::{BackendTag, OpenVerifyEnvelope, OpenVerifyEnvelopeBounds, StarkFriOpenProofV1},
};
use iroha_crypto::{
    Hash, PublicKey, Signature,
    fhe_bfv::{
        BFV_BOOTSTRAP_KEY_ID_MAX_BYTES, BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS,
        BFV_DETERMINISTIC_SEED_MAX_BYTES, BFV_EVALUATION_KEY_MAX_ROTATION_KEYS,
        BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1, BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1,
        BfvBootstrapKeyTranscriptSeed, BfvCiphertext, BfvEvaluationBudget, BfvEvaluationKeyBundle,
        BfvFullBootstrapCircuitArtifactBundleV1, BfvFullBootstrapReleaseAuditPackageV1,
        BfvPublicKey, BfvRotationKeyTranscriptSeed, RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT,
        bfv_balanced_multiplication_depth, bfv_public_key_digest, ram_lfe_bfv_parameters_v1,
        validate_bfv_bounded_noise_bound, validate_bfv_exact_residual_multiple_capacity,
        validate_bfv_full_bootstrap_release_audit_trusted_reviewer_id_v1,
        validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1,
        validate_public_key as validate_bfv_public_key,
    },
    kex::{KeyExchangeScheme as _, X25519Sha256},
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericOperationError, Quantity},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, Parser, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    sync::OnceLock,
};
use thiserror::Error;
/// Schema version for [`SoraContainerManifestV1`].
pub const SORA_CONTAINER_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraInrouManifestV1`].
pub const SORA_INROU_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceManifestV1`].
pub const SORA_SERVICE_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraHttpServiceEconomicsV1`].
pub const SORA_HTTP_SERVICE_ECONOMICS_VERSION_V1: u16 = 1;
/// Ledger precision used by XOR-denominated Soracloud quantities.
pub const SORACLOUD_XOR_SCALE: u32 = 9;
fn xor_quantity_from_nanos(value: u128) -> Quantity {
    Quantity::from_canonical_numeric(Numeric::new(value, SORACLOUD_XOR_SCALE))
        .expect("u128 nano-XOR value fits the bounded Quantity domain")
}
/// Schema version for [`SoraStateBindingV1`].
pub const SORA_STATE_BINDING_VERSION_V1: u16 = 1;
/// Schema version for [`SoraDeploymentBundleV1`].
pub const SORA_DEPLOYMENT_BUNDLE_VERSION_V1: u16 = 1;
/// Schema version for [`AgentApartmentManifestV1`].
pub const AGENT_APARTMENT_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`FheParamSetV1`].
pub const FHE_PARAM_SET_VERSION_V1: u16 = 1;
/// Registered Soracloud BFV backend profile admitted by first-release FHE manifests.
pub const REGISTERED_SORACLOUD_BFV_BACKEND_V1: &str = "fhe/bfv-rns/v1";
/// Schema version for [`FheExecutionPolicyV1`].
pub const FHE_EXECUTION_POLICY_VERSION_V1: u16 = 1;
/// Schema version for [`FheGovernanceBundleV1`].
pub const FHE_GOVERNANCE_BUNDLE_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFhePolicyReferenceV1`].
pub const SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFheGovernedMaterialV1`].
pub const SORACLOUD_FHE_GOVERNED_MATERIAL_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFhePolicyRecordV1`].
pub const SORACLOUD_FHE_POLICY_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFheGovernancePermissionScopeV1`].
pub const SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1: u16 = 1;
/// Maximum public rotation refresh transcript entries admitted for one BFV key bundle.
pub const BFV_REFRESH_TRANSCRIPT_MAX_ROTATION_TRANSCRIPTS: usize =
    BFV_EVALUATION_KEY_MAX_ROTATION_KEYS;
/// Maximum byte length for public BFV bootstrap refresh transcript key ids.
pub const BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES: usize = BFV_BOOTSTRAP_KEY_ID_MAX_BYTES;
/// Maximum public bootstrap refresh rounds admitted by BFV refresh transcripts.
pub const BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS: u16 =
    BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS;
/// Maximum byte length for public BFV refresh transcript seeds.
pub const BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES: usize = BFV_DETERMINISTIC_SEED_MAX_BYTES;
/// Schema version for [`SoracloudFheInputAdmissionProofV1`].
pub const SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFhePublicKeyProofV1`].
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudFheBootstrapKeyProofV1`].
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1: u16 = 1;
/// Schema version for [`SecretEnvelopeV1`].
pub const SECRET_ENVELOPE_VERSION_V1: u16 = 1;
/// Schema version for [`CiphertextStateRecordV1`].
pub const CIPHERTEXT_STATE_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`FheJobSpecV1`].
pub const FHE_JOB_SPEC_VERSION_V1: u16 = 1;
/// Schema version for [`DecryptionAuthorityPolicyV1`].
pub const DECRYPTION_AUTHORITY_POLICY_VERSION_V1: u16 = 1;
/// Schema version for [`DecryptionRequestV1`].
pub const DECRYPTION_REQUEST_VERSION_V1: u16 = 1;
/// Schema version for [`CiphertextQuerySpecV1`].
pub const CIPHERTEXT_QUERY_SPEC_VERSION_V1: u16 = 1;
/// Schema version for [`CiphertextQueryResponseV1`].
pub const CIPHERTEXT_QUERY_RESPONSE_VERSION_V1: u16 = 1;
/// Schema version for [`CiphertextInclusionProofV1`].
pub const CIPHERTEXT_QUERY_PROOF_VERSION_V1: u16 = 1;
/// Public-input schema for Soracloud BFV input-admission proofs.
pub const SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1: &[u8] =
    br#"{"schema":"soracloud_fhe_input_admission_v1","public_inputs":["statement_hash"],"proof_backend":"stark/fri/sha256-goldilocks","statement_layout":"((service_name,binding_name,key,operation,value_size_bytes,payload_commitment,encryption,governance_tx_hash),(bfv_parameter_digest,bfv_rns_modulus_chain_digest,bfv_key_switch_decomposition_chain_digest),ciphertext_proof_statement_digests,residual_multiple_bound,bound_mode)","bound_contract":{"modes":["exact_residual_multiple","bounded_noise"],"validates_exact_residual_capacity":true,"validates_bounded_noise_capacity":true},"ciphertext_statement_digest_domains":{"exact":"iroha.crypto.fhe.bfv.ciphertext_proof_statement.v1","bounded":"iroha.crypto.fhe.bfv.bounded_noise_ciphertext_proof_statement.v1","separates_exact_and_bounded":true},"ciphertext_statement_material":{"version":1,"field_count":8,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true,"binds_ciphertext":true,"binds_ciphertext_digest":true,"binds_declared_bound":true,"validates_exact_seeded_encryption_capacity":true,"rejects_all_zero_ciphertext":true},"ciphertext_generation":{"resamples_all_zero_ephemeral_mask":true,"resamples_all_zero_exact_error":true,"resamples_all_zero_bounded_noise":true},"ciphertext_proof_input_material":{"digest_domains":{"exact":"iroha.crypto.fhe.bfv.exact_residual_ciphertext_proof_input_material_digest.v1","bounded":"iroha.crypto.fhe.bfv.bounded_noise_ciphertext_proof_input_material_digest.v1","separates_exact_and_bounded":true},"exact":{"version":1,"field_count":14,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true,"binds_ciphertext":true,"binds_ciphertext_digest":true,"binds_secret_key_witness":true,"binds_plaintext":true,"binds_scaled_coefficients":true,"binds_residual_multiples":true,"binds_declared_bound":true,"binds_actual_residual_max":true,"rejects_zero_residual_witness":true,"binds_statement_hash":true},"bounded":{"version":1,"field_count":14,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true,"binds_ciphertext":true,"binds_ciphertext_digest":true,"binds_secret_key_witness":true,"binds_plaintext":true,"binds_scaled_coefficients":true,"binds_noise_polynomial":true,"binds_declared_bound":true,"binds_actual_noise_max":true,"rejects_zero_noise_witness":true,"binds_statement_hash":true},"hashes_proof_input_material":true},"persists_public_key_digest":true}"#;
/// Canonical STARK/FRI circuit id for Soracloud BFV input-admission proofs.
pub const SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1: &str = "soracloud_fhe_input_admission_v1";
/// Canonical gas schedule id for Soracloud BFV input-admission proofs.
pub const SORACLOUD_FHE_INPUT_ADMISSION_GAS_SCHEDULE_ID_V1: &str =
    "stark_fri_soracloud_input_admission_v1";
/// Public-input schema for Soracloud BFV public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1: &[u8] =
    br#"{"schema":"soracloud_fhe_public_key_v1","public_inputs":["statement_hash"],"proof_backend":"stark/fri/sha256-goldilocks","statement_layout":"public_key_proof_statement_digest(version,field_count,params,public_key,public_key_digest)","statement_digest_domains":{"exact":"iroha.crypto.fhe.bfv.public_key_proof_statement.v1","bounded":"iroha.crypto.fhe.bfv.bounded_noise_public_key_proof_statement.v1","separates_exact_and_bounded":true},"proof_statement_material":{"version":1,"field_count":5,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true},"key_generation":{"resamples_all_zero_public_key_a":true,"resamples_all_zero_exact_error":true,"resamples_all_zero_bounded_noise":true},"proof_input_material":{"digest_domains":{"exact":"iroha.crypto.fhe.bfv.exact_residual_public_key_proof_input_material_digest.v1","bounded":"iroha.crypto.fhe.bfv.bounded_noise_public_key_proof_input_material_digest.v1","separates_exact_and_bounded":true},"exact":{"version":1,"field_count":10,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true,"binds_secret_key_witness":true,"binds_public_key_residual":true,"binds_residual_multiples":true,"binds_residual_max":true,"rejects_zero_residual_witness":true,"binds_statement_hash":true},"bounded":{"version":1,"field_count":9,"binds_params":true,"binds_public_key":true,"binds_public_key_digest":true,"binds_secret_key_witness":true,"binds_public_key_noise":true,"binds_noise_max":true,"rejects_zero_noise_witness":true,"binds_statement_hash":true},"hashes_proof_input_material":true}}"#;
/// Canonical STARK/FRI circuit id for Soracloud BFV public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1: &str = "soracloud_fhe_public_key_v1";
/// Canonical gas schedule id for Soracloud BFV public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_GAS_SCHEDULE_ID_V1: &str =
    "stark_fri_soracloud_public_key_v1";
/// Public-input schema for Soracloud BFV bootstrap-key zero-refresh proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1: &[u8] =
    br#"{"schema":"soracloud_fhe_bootstrap_key_zero_refresh_v1","public_inputs":["statement_hash"],"proof_backend":"stark/fri/sha256-goldilocks","statement_layout":"bootstrap_key_transcript_zero_refresh_proof_statement_digest(version,field_count,params,public_key,evaluation_key_digest,refresh_transcript_digest,bootstrap_transcript,bootstrap_round_count,zero_refresh_digest,bootstrap_round_digests,bootstrap_key)","statement_digest_domains":{"exact_raw":"iroha.crypto.fhe.bfv.bootstrap_key_zero_refresh_proof_statement.v1","bounded_raw":"iroha.crypto.fhe.bfv.bounded_noise_bootstrap_key_zero_refresh_proof_statement.v1","exact_transcript":"iroha.crypto.fhe.bfv.bootstrap_key_transcript_zero_refresh_proof_statement.v1","bounded_transcript":"iroha.crypto.fhe.bfv.bounded_noise_bootstrap_key_transcript_zero_refresh_proof_statement.v1","separates_exact_and_bounded":true,"separates_raw_and_transcript":true},"refresh_transcript_digest_domains":{"exact":"iroha.crypto.fhe.bfv.refresh_transcript_digest.v1","bounded":"iroha.crypto.fhe.bfv.bounded_noise_refresh_transcript_digest.v1","separates_exact_and_bounded":true},"refresh_transcript_seed_derivation_domains":{"exact_rotation":"iroha.crypto.fhe.bfv.encrypt.v1","bounded_rotation":"iroha.crypto.fhe.bfv.encrypt.bounded_noise.v1","exact_bootstrap_round":"iroha.crypto.fhe.bfv.bootstrap_refresh_round.v1","bounded_bootstrap_round":"iroha.crypto.fhe.bfv.bootstrap_refresh_round.bounded_noise.v1","separates_exact_and_bounded":true,"separates_rotation_and_bootstrap_round":true},"refresh_transcript_material":{"version":1,"field_count":7,"binds_params":true,"binds_public_key":true,"rejects_all_zero_public_key":true,"binds_evaluation_key_digest":true,"binds_rotation_transcripts":true,"binds_bootstrap_transcript":true,"rejects_all_zero_transcript_seeds":true},"proof_statement_material":{"version":1,"field_count":11,"binds_evaluation_key_digest":true,"binds_refresh_transcript_digest":true,"binds_bootstrap_transcript":true,"binds_bootstrap_round_count":true,"binds_zero_refresh_digest":true,"binds_round_refresh_digests":true,"binds_bootstrap_key":true,"round_refresh_digest_domain":"iroha.crypto.fhe.bfv.bootstrap_key.round_refresh_digest.v1","zero_refresh_digest_domain":"iroha.crypto.fhe.bfv.bootstrap_key.zero_refresh_digest.v1"}}"#;
/// Canonical STARK/FRI circuit id for Soracloud BFV bootstrap-key proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1: &str =
    "soracloud_fhe_bootstrap_key_zero_refresh_v1";
/// Canonical gas schedule id for Soracloud BFV bootstrap-key proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_GAS_SCHEDULE_ID_V1: &str =
    "stark_fri_soracloud_bootstrap_key_v1";
/// Schema version for [`SoracloudFheFullBootstrapExecutionProofV1`].
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1: u16 = 1;
/// Public-input schema for Soracloud BFV full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1: &[u8] =
    concat!(
        r#"{"schema":"soracloud_fhe_full_bootstrap_execution_v1","public_inputs":["statement_hash"],"proof_backend":"stark/fri/sha256-goldilocks","statement_layout":"full_bootstrap_execution_proof_statement_digest(version,field_count,params,public_key,bootstrap_key,full_bootstrap_material_digest,artifact_bundle_digest,(slot_index,input_ciphertext,output_ciphertext,bound_mode,input_bound,output_bound,galois_key_set_digest,execution_witness_digest))","statement_digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_execution_proof_statement.v1","preflight":["bootstrap_key.public_key_digest matches public_key","claim.galois_key_set_digest matches supplied galois_keys"],"execution_witness_layout":{"digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_execution_witness_digest.v1","material_version":1,"material_field_count":15,"trace_field_count":7,"trace_bounds_field_count":6,"binds_galois_key_set_digest":true,"binds_trace":true,"binds_trace_bounds":true},"arithmetic_trace_profile":{"version":1,"field_count":34,"material_version":1,"material_field_count":8,"row_width":34,"private_row_count":64,"private_row_kind":1,"public_row_kind":0,"forbids_unmasked_private_row_openings":true,"forbids_duplicate_openings":true},"arithmetic_air_contract":{"version":1,"field_count":37,"binds_constraint_system_digest":true,"enforces_goldilocks_field_canonicality":true,"enforces_row_kind_partition":true,"enforces_active_rows_match_witness_material":true,"enforces_full_bootstrap_arithmetic_constraints":true,"enforces_public_padding_rows":true,"enforces_statement_hash_nonzero":true,"enforces_trace_output_matches_claim":true,"enforces_trace_bound_matches_claim":true,"enforces_no_unmasked_private_row_openings":true,"enforces_duplicate_free_openings":true,"derives_opening_schedule_from_statement_hash":true,"derives_opening_schedule_from_trace_material_digest":true,"bounds_opening_schedule_rejection_sampling":true,"validates_transcript_public_padding_openings":true,"composition_challenge_domain":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_composition_challenge.v1","composition_challenge_digest_bytes":32,"binds_composition_challenges_to_statement_hash":true,"binds_composition_challenges_to_trace_material_digest":true,"binds_composition_challenges_to_row_index":true,"binds_composition_challenges_to_column_index":true,"maps_zero_composition_challenge_to_one":true},"native_air_envelope":{"validates_stark_parameter_profile":true,"binds_domain_tag_to_statement_hash":true,"validates_circuit_id":true,"validates_trace_width":true,"validates_query_opening_count":true,"requires_public_padding_context":true,"requires_verifier_owned_trace_material_digest":true,"rejects_auxiliary_composition_value_commitments":true,"binds_public_digest_to_statement_hash":true,"validates_merkle_path_shape":true,"validates_merkle_path_roots":true,"validates_fri_query_chain":true,"binds_first_fri_values_to_opened_air_values":true,"binds_fri_queries_to_air_commitment_roots":true,"binds_trace_root_to_governed_arithmetic_trace":true,"binds_composition_root_to_governed_air_evaluation":true,"binds_opened_rows_to_governed_arithmetic_trace":true,"binds_opened_composition_values_to_governed_air_evaluation":true,"validates_public_padding_openings":true,"requires_zero_public_padding_composition_values":true,"requires_canonical_base_transcript_label":true,"rejects_suffixed_transcript_label_aliases":true,"rejects_blank_native_envelope_bytes":true,"rejects_placeholder_native_envelope_text":true},"artifact_bundle":{"artifact_digest_count":9,"binds_arithmetic_air_constraint_system_artifact":true,"validates_arithmetic_air_constraint_system_material":true},"release_prover_input":{"proof_input_material_version":1,"proof_input_material_field_count":5,"proof_input_material_digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_execution_proof_input_material_digest.v1","release_prover_digest_domains":{"proof_input_material":"iroha.crypto.fhe.bfv.full_bootstrap_execution_proof_input_material_digest.v1","prover_input_material":"iroha.crypto.fhe.bfv.full_bootstrap_execution_prover_input_material_digest.v1","air_evaluation_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_evaluation_material_digest.v1","public_opening_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_trace_public_opening_material_digest.v1","arithmetic_trace_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_trace_material_digest.v1","arithmetic_air_constraint_system":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_constraint_system_digest.v1","separates_release_prover_material_domains":true},"hashes_proof_input_material":true,"prover_input_material_version":1,"prover_input_material_field_count":13,"air_evaluation_material_version":1,"air_evaluation_material_field_count":8,"public_opening_material_version":1,"public_opening_material_field_count":13,"binds_release_prover_arithmetic_air_constraint_system_digest":true,"binds_release_prover_arithmetic_air_constraint_system_artifact_digest":true,"binds_release_prover_arithmetic_air_evaluation_material_digest":true,"binds_arithmetic_air_evaluation_trace_material_digest":true,"requires_zero_arithmetic_air_composition_values":true,"binds_arithmetic_trace_material_digest":true,"binds_trace_proof_input_consistency":true,"binds_generated_proof_key_pair":true,"proof_key_commitment_domains":{"material":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_material_commitment.v1","pair":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_pair_commitment.v1","separates_material_and_pair":true},"binds_release_prover_verifier_key":true,"validates_artifact_bound_prover_input":true,"rejects_stale_galois_key_set_replay":true,"rejects_stale_proof_key_artifacts":true,"derives_opening_schedule_from_statement_hash":true,"derives_opening_schedule_from_trace_material_digest":true,"bounds_opening_schedule_rejection_sampling":true,"validates_transcript_public_padding_openings":true,"validates_transcript_public_opening_material":true,"requires_verifier_owned_trace_material_digest":true,"requires_canonical_base_transcript_label":true,"rejects_suffixed_transcript_label_aliases":true},"#,
        r#""release_audit_evidence":{"version":1,"field_count":23,"digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_release_audit_evidence_digest.v1","proof_profile_field_count":58,"proof_profile_air_evaluation_material_version":1,"proof_profile_air_evaluation_material_field_count":8,"proof_profile_public_opening_material_version":1,"proof_profile_public_opening_material_field_count":13,"proof_profile_release_prover_digest_domains":{"proof_input_material":"iroha.crypto.fhe.bfv.full_bootstrap_execution_proof_input_material_digest.v1","prover_input_material":"iroha.crypto.fhe.bfv.full_bootstrap_execution_prover_input_material_digest.v1","air_evaluation_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_evaluation_material_digest.v1","public_opening_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_trace_public_opening_material_digest.v1","arithmetic_trace_material":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_trace_material_digest.v1","arithmetic_air_constraint_system":"iroha.crypto.fhe.bfv.full_bootstrap_arithmetic_air_constraint_system_digest.v1","separates_release_prover_material_domains":true},"proof_profile_proof_key_commitment_domains":{"material":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_material_commitment.v1","pair":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_pair_commitment.v1","separates_material_and_pair":true},"proof_profile_validates_air_evaluation_material_digest":true,"proof_profile_validates_air_evaluation_trace_material_digest":true,"proof_profile_requires_zero_air_composition_values":true,"proof_profile_validates_artifact_bound_prover_input":true,"proof_profile_rejects_stale_galois_key_set_replay":true,"proof_profile_rejects_stale_proof_key_artifacts":true,"proof_profile_derives_opening_schedule_from_statement_hash":true,"proof_profile_derives_opening_schedule_from_trace_material_digest":true,"proof_profile_bounds_opening_schedule_rejection_sampling":true,"proof_profile_validates_transcript_public_padding_openings":true,"proof_profile_validates_transcript_public_opening_material":true,"proof_profile_requires_verifier_owned_trace_material_digest":true,"proof_profile_requires_canonical_base_transcript_label":true,"proof_profile_rejects_suffixed_transcript_label_aliases":true,"proof_profile_validates_merkle_path_shape":true,"proof_profile_validates_merkle_path_roots":true,"proof_profile_validates_fri_query_chain":true,"proof_profile_binds_first_fri_values_to_opened_air_values":true,"proof_profile_binds_fri_queries_to_air_commitment_roots":true,"proof_profile_binds_centered_scale_round_source_chain_digest":true,"key_evidence_field_count":11,"key_evidence_binds_centered_scale_round_source_chain_digest":true,"artifact_digest_domains":{"circuit_material":"iroha.crypto.fhe.bfv.full_bootstrap_circuit_material_digest.v1","evaluator_artifact_set":"iroha.crypto.fhe.bfv.full_bootstrap_evaluator_artifact_set_digest.v1","circuit_artifact_bundle":"iroha.crypto.fhe.bfv.full_bootstrap_circuit_artifact_bundle_digest.v1","separates_material_set_and_bundle":true},"binds_artifact_bundle_digest":true,"binds_generated_circuit_body_digest":true,"requires_nonzero_generated_circuit_body":true,"binds_evaluator_artifact_set_digest":true,"binds_coefficient_to_slot_key_digest":true,"binds_slot_to_coefficient_key_digest":true,"binds_blind_rotation_key_digest":true,"binds_sample_extraction_key_digest":true,"binds_accumulator_digest":true,"binds_proof_public_input_schema_digest":true,"binds_arithmetic_trace_profile_digest":true,"binds_arithmetic_air_constraint_system_digest":true,"binds_arithmetic_air_constraint_system_artifact_digest":true,"binds_generated_proof_key_pair":true,"proof_key_commitment_domains":{"material":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_material_commitment.v1","pair":"iroha.crypto.fhe.bfv.full_bootstrap_proof_key_pair_commitment.v1","separates_material_and_pair":true},"binds_native_payload_digests":true,"rejects_inert_native_payload_digest_sentinels":true,"rejects_non_production_native_payload_digest_sentinels":true,"rejects_whitespace_prefixed_non_production_native_payload_digest_sentinels":true,"rejects_case_decorated_placeholder_native_payload_digest_sentinels":true,"rejects_whitespace_prefixed_placeholder_native_payload_digest_sentinels":true,"rejects_padded_placeholder_native_payload_digest_sentinels":true,"rejects_delayed_placeholder_native_payload_digest_sentinels":true,"rejects_binary_decorated_placeholder_native_payload_digest_sentinels":true,"binds_native_circuit_fingerprint":true,"binds_registered_profile_digests":true,"binds_canonical_profile_artifact_digests":true,"binds_centered_scale_round_source_chain_digest":true,"requires_distinct_evidence_commitments":true},"#,
        r#""release_audit_signoff":{"version":1,"field_count":4,"payload_version":1,"payload_field_count":18,"reviewer_id_max_bytes":128,"rejects_placeholder_reviewer_ids":true,"binds_release_audit_evidence_digest":true,"binds_generated_circuit_body_digest":true,"binds_centered_scale_round_source_chain_digest":true,"binds_prover_native_payload_digest":true,"binds_verifier_native_payload_digest":true,"binds_external_audit_report_digest":true,"binds_evidence_archive_digest":true,"requires_external_audit_digests_distinct_from_signed_commitments":true,"rejects_header_only_external_audit_digests":true,"rejects_nested_header_external_audit_digests":true,"rejects_whitespace_prefixed_nested_header_external_audit_digests":true,"rejects_zero_body_external_audit_digests":true,"rejects_blank_body_external_audit_digests":true,"rejects_padded_zero_body_external_audit_digests":true,"rejects_padded_blank_body_external_audit_digests":true,"rejects_case_decorated_placeholder_external_audit_digests":true,"rejects_whitespace_prefixed_placeholder_external_audit_digests":true,"rejects_delayed_placeholder_external_audit_digests":true,"rejects_placeholder_external_audit_digests":true,"requires_reviewer_signature":true,"requires_trusted_reviewer_public_key":true,"requires_ed25519_reviewer_public_key":true,"rejects_empty_reviewer_public_key":true,"rejects_all_zero_reviewer_public_key":true},"#,
        r#""release_audit_record":{"version":1,"field_count":4,"digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_release_audit_record_digest.v1","packages_release_audit_evidence":true,"packages_release_audit_signoff":true,"validates_record_against_governed_artifacts":true},"#,
        r#""release_audit_manifest":{"version":1,"field_count":23,"digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_release_audit_manifest_digest.v1","scope":"iroha.crypto.fhe.bfv.full_bootstrap.release_audit.v1","requires_approved_verdict":true,"rejects_placeholder_reviewer_ids":true,"binds_release_audit_record_digest":true,"binds_release_audit_evidence_digest":true,"binds_centered_scale_round_source_chain_digest":true,"binds_artifact_bundle_digest":true,"binds_evaluator_artifact_set_digest":true,"binds_proof_key_pair_commitment":true,"binds_prover_native_payload_digest":true,"binds_verifier_native_payload_digest":true,"binds_native_circuit_fingerprint":true,"binds_generated_circuit_body_digest":true,"binds_external_audit_report_digest":true,"binds_evidence_archive_digest":true,"requires_external_audit_digests_distinct_from_signed_commitments":true,"rejects_header_only_external_audit_digests":true,"rejects_nested_header_external_audit_digests":true,"rejects_whitespace_prefixed_nested_header_external_audit_digests":true,"rejects_zero_body_external_audit_digests":true,"rejects_blank_body_external_audit_digests":true,"rejects_padded_zero_body_external_audit_digests":true,"rejects_padded_blank_body_external_audit_digests":true,"rejects_case_decorated_placeholder_external_audit_digests":true,"rejects_whitespace_prefixed_placeholder_external_audit_digests":true,"rejects_delayed_placeholder_external_audit_digests":true,"rejects_placeholder_external_audit_digests":true,"binds_trusted_reviewer_public_key":true,"binds_ed25519_trusted_reviewer_public_key":true,"rejects_empty_trusted_reviewer_public_key":true,"rejects_all_zero_trusted_reviewer_public_key":true},"#,
        r#""release_audit_package":{"version":1,"field_count":8,"digest_domain":"iroha.crypto.fhe.bfv.full_bootstrap_release_audit_package_digest.v1","audit_report_max_bytes":16777216,"audit_archive_max_bytes":134217728,"audit_report_body_min_bytes":64,"audit_archive_body_min_bytes":64,"packages_release_audit_record":true,"packages_release_audit_manifest":true,"validates_release_audit_manifest_digest":true,"validates_machine_checkable_release_verdict":true,"validates_external_audit_report_bytes":true,"validates_evidence_archive_bytes":true,"requires_audit_report_body_release_evidence_digest":true,"requires_audit_report_body_proof_profile_field_count":true,"requires_audit_report_body_proof_profile_canonical_base_transcript_label_obligation":true,"requires_audit_report_body_proof_profile_rejects_suffixed_transcript_label_aliases":true,"requires_audit_report_body_proof_profile_release_prover_digest_domains":true,"requires_audit_report_body_proof_profile_proof_key_commitment_domains":true,"requires_evidence_archive_body_artifact_bundle_digest":true,"requires_evidence_archive_body_evaluator_artifact_set_digest":true,"requires_evidence_archive_body_centered_scale_round_source_chain_digest":true,"requires_evidence_archive_body_arithmetic_trace_profile_digest":true,"requires_evidence_archive_body_arithmetic_air_constraint_system_digest":true,"requires_evidence_archive_body_proof_profile_field_count":true,"requires_evidence_archive_body_proof_profile_canonical_base_transcript_label_obligation":true,"requires_evidence_archive_body_proof_profile_rejects_suffixed_transcript_label_aliases":true,"requires_evidence_archive_body_proof_profile_release_prover_digest_domains":true,"requires_evidence_archive_body_proof_profile_proof_key_commitment_domains":true,"requires_evidence_archive_body_generated_circuit_body_digest":true,"requires_evidence_archive_body_generated_circuit_body_byte_length":true,"requires_evidence_archive_body_generated_circuit_body_hex":true,"requires_evidence_archive_body_coefficient_to_slot_key_artifact_hex":true,"requires_evidence_archive_body_slot_to_coefficient_key_artifact_hex":true,"requires_evidence_archive_body_blind_rotation_key_artifact_hex":true,"requires_evidence_archive_body_extraction_key_artifact_hex":true,"requires_evidence_archive_body_accumulator_artifact_hex":true,"requires_evidence_archive_body_proof_public_input_schema_artifact_hex":true,"requires_evidence_archive_body_arithmetic_air_constraint_system_artifact_hex":true,"requires_evidence_archive_body_native_circuit_fingerprint":true,"requires_evidence_archive_body_proof_key_pair_commitment":true,"requires_evidence_archive_body_prover_key_digest":true,"requires_evidence_archive_body_verifier_key_digest":true,"requires_evidence_archive_body_native_prover_payload_hex":true,"requires_evidence_archive_body_native_verifier_payload_hex":true,"requires_evidence_archive_body_prover_native_payload_digest":true,"requires_evidence_archive_body_verifier_native_payload_digest":true,"requires_evidence_archive_body_prover_key_artifact_hex":true,"requires_evidence_archive_body_verifier_key_artifact_hex":true,"requires_signed_commitment_standalone_label_tokens":true,"requires_signed_commitment_value_separator":true,"requires_signed_commitment_same_field_values":true,"requires_signed_commitment_standalone_value_tokens":true,"requires_signed_commitment_equals_separator":true,"requires_lowercase_signed_commitment_labels":true,"requires_lowercase_signed_commitment_hex_values":true,"rejects_raw_byte_signed_commitment_values":true,"rejects_colon_separated_signed_commitments":true,"rejects_uppercase_signed_commitment_labels":true,"rejects_uppercase_signed_commitment_values":true,"rejects_relabelled_signed_commitments":true,"rejects_cross_field_signed_commitment_replay":true,"rejects_punctuated_signed_commitment_values":true,"rejects_conflicting_duplicate_signed_commitment_labels":true,"rejects_duplicate_signed_commitment_labels":true,"rejects_same_value_duplicate_signed_commitments":true,"rejects_header_only_external_audit_digests":true,"rejects_nested_header_external_audit_digests":true,"rejects_whitespace_prefixed_nested_header_external_audit_digests":true,"rejects_zero_body_external_audit_digests":true,"rejects_blank_body_external_audit_digests":true,"rejects_padded_zero_body_external_audit_digests":true,"rejects_padded_blank_body_external_audit_digests":true,"rejects_case_decorated_placeholder_external_audit_digests":true,"rejects_whitespace_prefixed_placeholder_external_audit_digests":true,"rejects_delayed_placeholder_external_audit_digests":true,"rejects_placeholder_external_audit_digests":true,"rejects_all_zero_audit_artifacts":true,"requires_canonical_audit_artifact_headers":true,"requires_nonempty_audit_artifact_bodies":true,"requires_minimum_audit_artifact_body_bytes":true,"rejects_blank_audit_artifact_bodies":true,"rejects_zero_body_audit_artifacts":true,"rejects_nested_audit_artifact_bodies":true,"rejects_whitespace_prefixed_nested_audit_artifact_bodies":true,"rejects_delayed_nested_audit_artifact_bodies":true,"rejects_placeholder_audit_artifact_bodies":true,"rejects_delayed_placeholder_audit_artifact_bodies":true,"rejects_delayed_non_production_audit_artifact_bodies":true,"scans_entire_placeholder_audit_artifact_bodies":true,"requires_distinct_audit_artifact_bodies":true,"requires_external_review_report_marker":true,"requires_external_review_archive_marker":true,"requires_byte_leading_external_review_report_marker":true,"requires_byte_leading_external_review_archive_marker":true,"requires_lowercase_external_review_markers":true,"requires_external_review_marker_colon_separator":true,"requires_external_review_marker_statement":true,"requires_external_review_marker_statement_text":true,"requires_printable_ascii_external_review_marker_statement_text":true,"requires_distinct_external_review_marker_statements":true,"requires_external_review_marker_reviewer_id":true,"requires_external_review_marker_reviewer_id_label":true,"requires_single_external_review_marker_reviewer_id_label":true,"rejects_conflicting_external_review_marker_reviewer_id_labels":true,"rejects_duplicate_external_review_marker_reviewer_id_labels":true,"rejects_case_drifted_external_review_marker_reviewer_id_labels":true,"rejects_separator_alias_external_review_marker_reviewer_id_labels":true,"rejects_missing_external_review_marker_colon_separator":true,"rejects_padded_colon_external_review_marker_aliases":true,"rejects_empty_external_review_marker_statement":true,"rejects_generic_external_review_marker_statement":true,"rejects_uppercase_external_review_markers":true,"rejects_whitespace_prefixed_external_review_markers":true,"rejects_machine_generated_audit_artifact_bodies":true,"rejects_separator_obfuscated_machine_generated_audit_artifact_bodies":true,"validates_package_against_governed_artifacts":true,"rejects_placeholder_reviewer_ids":true,"validates_trusted_reviewer_public_key":true,"requires_ed25519_trusted_reviewer_public_key":true,"rejects_empty_trusted_reviewer_public_key":true,"rejects_all_zero_trusted_reviewer_public_key":true,"requires_caller_pinned_package_digest":true,"validates_caller_pinned_package_digest":true,"rejects_placeholder_caller_pinned_package_digest":true,"rejects_binary_framed_placeholder_caller_pinned_package_digest":true,"rejects_placeholder_package_record_digest":true,"rejects_placeholder_package_manifest_digest":true,"rejects_caller_pinned_record_digest_alias":true,"rejects_caller_pinned_manifest_digest_alias":true,"rejects_caller_pinned_signed_commitment_digest_alias":true,"rejects_caller_pinned_centered_scale_round_source_chain_digest_alias":true}}"#
    )
    .as_bytes();
/// Canonical STARK/FRI circuit id for Soracloud BFV full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1: &str =
    BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1;
/// Canonical gas schedule id for Soracloud BFV full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_GAS_SCHEDULE_ID_V1: &str =
    "stark_fri_soracloud_full_bootstrap_execution_v1";
/// Maximum backend-native STARK/FRI envelope bytes for Soracloud FHE input admission.
pub const SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum STARK/FRI public-input wrapper bytes for Soracloud FHE input admission.
pub const SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES: usize =
    SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES + 16 * 1024;
/// Maximum encoded `OpenVerify` envelope bytes for Soracloud FHE input admission.
pub const SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES: usize =
    SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES + 16 * 1024;
/// Maximum backend-native STARK/FRI envelope bytes for Soracloud FHE public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES: usize =
    SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES;
/// Maximum STARK/FRI public-input wrapper bytes for Soracloud FHE public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES: usize =
    SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 16 * 1024;
/// Maximum encoded `OpenVerify` envelope bytes for Soracloud FHE public-key proofs.
pub const SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES: usize =
    SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES + 16 * 1024;
/// Maximum backend-native STARK/FRI envelope bytes for Soracloud FHE bootstrap-key proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES: usize =
    SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES;
/// Maximum STARK/FRI public-input wrapper bytes for Soracloud FHE bootstrap-key proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES: usize =
    SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 16 * 1024;
/// Maximum encoded `OpenVerify` envelope bytes for Soracloud FHE bootstrap-key proofs.
pub const SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES: usize =
    SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES + 16 * 1024;
/// Maximum backend-native STARK/FRI envelope bytes for full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES: usize =
    SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES;
/// Maximum STARK/FRI public-input wrapper bytes for full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES: usize =
    SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 16 * 1024;
/// Maximum encoded `OpenVerify` envelope bytes for full-bootstrap execution proofs.
pub const SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES: usize =
    SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES + 64 * 1024;
/// Schema version for [`SoraServiceStateEntryV1`].
pub const SORA_SERVICE_STATE_ENTRY_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceConfigEntryV1`].
pub const SORA_SERVICE_CONFIG_ENTRY_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceSecretEntryV1`].
pub const SORA_SERVICE_SECRET_ENTRY_VERSION_V1: u16 = 1;
/// Schema version for [`SoraDecryptionRequestRecordV1`].
pub const SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraTrainingJobRecordV1`].
pub const SORA_TRAINING_JOB_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraTrainingJobAuditEventV1`].
pub const SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelRegistryV1`].
pub const SORA_MODEL_REGISTRY_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelWeightVersionRecordV1`].
pub const SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelWeightAuditEventV1`].
pub const SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelArtifactRecordV1`].
pub const SORA_MODEL_ARTIFACT_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelArtifactAuditEventV1`].
pub const SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraUploadedModelBundleV1`].
pub const SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraUploadedModelEncryptionRecipientV1`].
pub const SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraUploadedModelWrappedKeyV1`].
pub const SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1: u16 = 1;
const SORA_UPLOADED_MODEL_X25519_PUBLIC_KEY_BYTES: usize = 32;
/// Schema version for [`SoraPrivateModelArtifactRefV1`].
pub const SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1: u16 = 1;
/// Schema version for [`SoraPrivateUploadedModelExecutionReceiptV1`].
pub const SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraHfSourceRecordV1`].
pub const SORA_HF_SOURCE_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelHostCapabilityRecordV1`].
pub const SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraInrouHostCapabilityRecordV1`].
pub const SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1: u16 = 1;
/// Fixed signature-domain tag for Soracloud runtime provenance preimages.
pub const SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1: &[u8] =
    b"iroha:soracloud:runtime-provenance:v1\x00";
/// Canonical Soracloud runtime provenance preimage version.
pub const SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1: u8 = 1;
/// Schema version for [`SoraHfPlacementRecordV1`].
pub const SORA_HF_PLACEMENT_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraInrouServicePlacementRecordV1`].
pub const SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraHfSharedLeasePoolV1`].
pub const SORA_HF_SHARED_LEASE_POOL_VERSION_V1: u16 = 1;
/// Schema version for [`SoraHfSharedLeaseMemberV1`].
pub const SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1: u16 = 1;
/// Schema version for [`SoraHfSharedLeaseAuditEventV1`].
pub const SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraModelHostViolationEvidenceRecordV1`].
pub const SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAgentApartmentRecordV1`].
pub const SORA_AGENT_APARTMENT_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAgentApartmentAuditEventV1`].
pub const SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceRuntimeStateV1`].
pub const SORA_SERVICE_RUNTIME_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraInrouReplicaRuntimeStateV1`].
pub const SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceMailboxMessageV1`].
pub const SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraRuntimeReceiptV1`].
pub const SORA_RUNTIME_RECEIPT_VERSION_V1: u16 = 1;
/// Schema version for [`CanonicalRequestWitnessV1`].
pub const CANONICAL_REQUEST_WITNESS_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudHostRequestEnvelopeV1`].
pub const SORACLOUD_HOST_REQUEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoracloudHostResponseEnvelopeV1`].
pub const SORACLOUD_HOST_RESPONSE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceRolloutStateV1`].
pub const SORA_SERVICE_ROLLOUT_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceDeploymentStateV1`].
pub const SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceLeaseStateV1`].
pub const SORA_SERVICE_LEASE_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceLeaseVolumeStateV1`].
pub const SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceAuditEventV1`].
pub const SORA_SERVICE_AUDIT_EVENT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppStaticSiteBindingV1`].
pub const SORA_APP_STATIC_SITE_BINDING_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppInfraServiceRefV1`].
pub const SORA_APP_INFRA_SERVICE_REF_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppRouteProjectionV1`].
pub const SORA_APP_ROUTE_PROJECTION_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppInfraManifestV1`].
pub const SORA_APP_INFRA_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppInfraStateV1`].
pub const SORA_APP_INFRA_STATE_VERSION_V1: u16 = 1;
/// Schema version for [`SoraAppInfraAuditEventV1`].
pub const SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1: u16 = 1;
// These are textual includes, rather than nested modules, so the structural
// split does not change public type paths or any path-derived wire identity.
include!("soracloud/schema.rs");
include!("soracloud/fhe.rs");
include!("soracloud/deployment.rs");
include!("soracloud/hosting.rs");
include!("soracloud/host_protocol.rs");
include!("soracloud/prelude.rs");
#[cfg(test)]
mod tests {
    include!("soracloud/tests/fixtures_and_manifests.rs");
    include!("soracloud/tests/proof_schemas.rs");
    include!("soracloud/tests/proof_validation.rs");
    include!("soracloud/tests/provenance.rs");
    include!("soracloud/tests/manifest_validation.rs");
    include!("soracloud/tests/fhe_policy.rs");
    include!("soracloud/tests/decryption_and_records.rs");
}
