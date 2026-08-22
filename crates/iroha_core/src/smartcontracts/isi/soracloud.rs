//! Soracloud lifecycle instruction handlers.
use super::{
    asset::isi::assert_numeric_spec_with,
    staking::{apply_slash_to_validator, max_slash_amount},
    *,
};
use crate::{
    smartcontracts::Execute,
    soracloud_runtime::soracloud_hf_generated_source_binding,
    state::{StateTransaction, public_lane_validator_record_matches_key},
};
#[cfg(all(test, feature = "zk-stark"))]
use iroha_crypto::fhe_bfv::{
    BfvFullBootstrapExecutionProofInputMaterialV1,
    validate_bfv_full_bootstrap_arithmetic_air_evaluation_material_for_trace_v1,
    validate_bfv_full_bootstrap_arithmetic_trace_material_v1,
};
#[cfg(feature = "zk-stark")]
use iroha_crypto::fhe_bfv::{
    BfvFullBootstrapExecutionProverInputMaterialV1,
    bfv_full_bootstrap_arithmetic_air_evaluation_material_v1,
    bfv_full_bootstrap_arithmetic_trace_material_digest_v1,
    bfv_full_bootstrap_arithmetic_trace_material_v1,
    bfv_full_bootstrap_execution_proof_input_material_v1,
    bfv_full_bootstrap_execution_prover_input_material_digest_for_artifacts_v1,
    bfv_full_bootstrap_execution_prover_input_material_v1,
    bfv_full_bootstrap_execution_witness_digest_material_v1,
    validate_bfv_full_bootstrap_execution_prover_input_material_v1,
};
use iroha_crypto::{
    Algorithm, Hash, PublicKey, Signature,
    fhe_bfv::{
        BfvBootstrapKeyMode, BfvCiphertext, BfvEvaluationBudget, BfvEvaluationKeyBundle,
        BfvEvaluationPlan, BfvFullBootstrapCircuitArtifactBundleV1,
        BfvFullBootstrapCircuitArtifactRoleV1, BfvFullBootstrapExecutionProofBoundModeV1,
        BfvFullBootstrapReleaseAuditPackageV1, BfvIdentifierCiphertext, BfvParameters,
        BfvPublicKey, RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT,
        add_ciphertexts_bounded_noise_registered_rns_exact, add_ciphertexts_registered_rns_exact,
        bfv_add_bounded_noise_output_bound, bfv_add_output_residual_multiple_bound,
        bfv_bootstrap_key_refresh_bounded_noise_output_bound,
        bfv_bootstrap_key_refresh_output_residual_multiple_bound,
        bfv_bounded_noise_ciphertext_proof_statement_digest,
        bfv_ciphertext_exact_residual_proof_statement_digest,
        bfv_full_bootstrap_bounded_noise_output_bound_v1,
        bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1,
        bfv_full_bootstrap_execution_proof_statement_digest_with_witness_v1,
        bfv_full_bootstrap_output_residual_multiple_bound_v1,
        bfv_full_bootstrap_with_release_audited_artifacts_bounded_noise_output_bound_v1,
        bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1,
        bfv_multiply_bounded_noise_output_bound, bfv_multiply_output_residual_multiple_bound,
        bfv_packed_rotate_left_bounded_noise_output_bound,
        bfv_packed_rotate_left_output_residual_multiple_bound, bfv_public_key_digest,
        bfv_rotate_slots_left_bounded_noise_registered_rns_basis_extension_output_bounds,
        bfv_rotate_slots_left_output_residual_multiple_bounds,
        bootstrap_ciphertext_bounded_noise_registered_rns_basis_extension_exact_rounds,
        bootstrap_ciphertext_registered_rns_exact_rounds,
        decode_bfv_full_bootstrap_native_proof_key_material_v1,
        decode_bfv_full_bootstrap_proof_key_artifact_v1,
        full_bootstrap_ciphertext_bounded_noise_registered_rns_basis_extension_exact_v1,
        full_bootstrap_ciphertext_registered_rns_exact_v1,
        full_bootstrap_ciphertext_with_release_audited_artifacts_bounded_noise_registered_rns_basis_extension_exact_v1,
        full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1,
        multiply_ciphertexts_bounded_noise_registered_rns_basis_extension_exact,
        multiply_ciphertexts_registered_rns_exact, multiply_plain_scalar,
        ram_lfe_bfv_parameters_v1, registered_bfv_key_switch_decomposition_chain_digest,
        registered_bfv_parameter_digest, registered_bfv_rns_modulus_chain_digest,
        rotate_ciphertext_slots_left_bounded_noise_registered_rns_basis_extension_exact,
        rotate_ciphertext_slots_left_registered_rns_exact,
        rotate_packed_ciphertext_slots_left_with_galois_keys_bounded_noise_registered_rns_basis_extension_exact,
        rotate_packed_ciphertext_slots_left_with_galois_keys_registered_rns_exact,
        validate_bfv_bounded_noise_bound, validate_bfv_exact_residual_multiple_capacity,
        validate_bfv_full_bootstrap_circuit_artifact_bundle_v1,
        validate_bfv_full_bootstrap_execution_artifacts_preflight_v1,
        validate_bfv_full_bootstrap_proof_key_material_envelope_bytes_for_key_v1,
        validate_bfv_full_bootstrap_release_audit_package_for_artifacts_trusted_reviewer_and_digest_v1,
        validate_bfv_full_bootstrap_release_audit_trusted_reviewer_id_v1,
        validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1,
        validate_registered_bfv_parameters,
    },
};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        soracloud as isi,
    },
    name::Name,
    nexus::PublicLaneValidatorStatus,
    proof::ProofAttachment,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        BfvCiphertextBoundModeV1, BfvEvaluationKeyRefreshTranscriptV1, BfvRefreshTranscriptModeV1,
        DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheExecutionPolicyV1, FheJobOperationV1,
        FheJobSpecV1, FheParamSetV1, FheSchemeV1, SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        SORA_AGENT_APARTMENT_RECORD_VERSION_V1, SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        SORA_APP_INFRA_STATE_VERSION_V1, SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
        SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
        SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
        SORA_HF_SOURCE_RECORD_VERSION_V1, SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
        SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
        SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1, SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
        SORA_MODEL_ARTIFACT_RECORD_VERSION_V1, SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
        SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1, SORA_MODEL_REGISTRY_VERSION_V1,
        SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1, SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
        SORA_SERVICE_AUDIT_EVENT_VERSION_V1, SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
        SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_LEASE_STATE_VERSION_V1,
        SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1, SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        SORA_SERVICE_SECRET_ENTRY_VERSION_V1, SORA_SERVICE_STATE_ENTRY_VERSION_V1,
        SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1, SORA_TRAINING_JOB_RECORD_VERSION_V1,
        SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1, SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_GAS_SCHEDULE_ID_V1,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_GAS_SCHEDULE_ID_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1,
        SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_GAS_SCHEDULE_ID_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES,
        SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES,
        SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
        SORACLOUD_FHE_POLICY_RECORD_VERSION_V1, SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_GAS_SCHEDULE_ID_V1,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1, SecretEnvelopeV1, SoraAgentApartmentActionV1,
        SoraAgentApartmentAuditEventV1, SoraAgentApartmentRecordV1, SoraAgentArtifactAllowRuleV1,
        SoraAgentAutonomyRunRecordV1, SoraAgentMailboxMessageV1, SoraAgentPersistentStateV1,
        SoraAgentRuntimeStatusV1, SoraAgentWalletDailySpendEntryV1, SoraAgentWalletSpendRequestV1,
        SoraAppInfraActionV1, SoraAppInfraAuditEventV1, SoraAppInfraManifestV1,
        SoraAppInfraStateV1, SoraDecryptionRequestRecordV1, SoraDeploymentBundleV1,
        SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1,
        SoraHfPlacementRecordV1, SoraHfPlacementStatusV1, SoraHfResourceProfileV1,
        SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1, SoraHfSharedLeaseMemberStatusV1,
        SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSharedLeaseQueuedWindowV1,
        SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1, SoraHfSourceStatusV1, SoraInrouGuestIsaV1,
        SoraInrouHostCapabilityRecordV1, SoraInrouReplicaPlacementV1,
        SoraInrouReplicaRuntimeStateV1, SoraInrouRuntimeBackendV1,
        SoraInrouServicePlacementRecordV1, SoraModelArtifactActionV1,
        SoraModelArtifactAuditEventV1, SoraModelArtifactRecordV1, SoraModelHostCapabilityRecordV1,
        SoraModelHostViolationEvidenceRecordV1, SoraModelHostViolationKindV1,
        SoraModelProvenanceKindV1, SoraModelProvenanceRefV1, SoraModelRegistryV1,
        SoraModelWeightActionV1, SoraModelWeightAuditEventV1, SoraModelWeightVersionRecordV1,
        SoraPrivateUploadedModelExecutionReceiptV1, SoraRolloutStageV1, SoraRuntimeReceiptV1,
        SoraServiceAuditEventV1, SoraServiceConfigEntryV1, SoraServiceDeploymentStateV1,
        SoraServiceExecutionPlaneV1, SoraServiceLeaseStateV1, SoraServiceLeaseStatusV1,
        SoraServiceLeaseVolumeStateV1, SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1,
        SoraServiceRolloutStateV1, SoraServiceRuntimeStateV1, SoraServiceSecretEntryV1,
        SoraServiceStateEntryV1, SoraStateEncryptionV1, SoraStateMutationOperationV1,
        SoraTrainingJobActionV1, SoraTrainingJobAuditEventV1, SoraTrainingJobRecordV1,
        SoraTrainingJobStatusV1, SoraUploadedModelBundleV1, SoracloudFheBootstrapKeyProofV1,
        SoracloudFheFullBootstrapExecutionProofV1, SoracloudFheGovernancePermissionScopeV1,
        SoracloudFheGovernedMaterialV1, SoracloudFheInputAdmissionProofV1,
        SoracloudFhePolicyRecordV1, SoracloudFhePolicyReferenceV1,
        SoracloudFhePolicyVersionLifecycleV1, SoracloudFhePolicyVersionStateV1,
        SoracloudFhePublicKeyProofV1, derive_agent_autonomy_request_commitment,
        derive_soracloud_fhe_input_admission_statement_hash_with_bound_mode,
        encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_app_infra_provenance_payload,
        encode_decryption_request_provenance_payload,
        encode_delete_service_config_provenance_payload,
        encode_delete_service_secret_provenance_payload, encode_fhe_job_run_provenance_payload,
        encode_fhe_policy_register_provenance_payload, encode_fhe_policy_revoke_provenance_payload,
        encode_fhe_policy_rotate_provenance_payload,
        encode_hf_shared_lease_join_provenance_payload,
        encode_hf_shared_lease_leave_provenance_payload,
        encode_hf_shared_lease_renew_provenance_payload,
        encode_inrou_host_advertise_provenance_payload,
        encode_inrou_host_withdraw_provenance_payload,
        encode_model_artifact_register_provenance_payload,
        encode_model_host_advertise_provenance_payload,
        encode_model_host_heartbeat_provenance_payload,
        encode_model_host_withdraw_provenance_payload,
        encode_model_weight_promote_provenance_payload,
        encode_model_weight_register_provenance_payload,
        encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_set_service_config_provenance_payload,
        encode_set_service_secret_provenance_payload, encode_state_mutation_provenance_payload,
        encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
        hf_shared_lease_max_compute_reservation_fee_v1,
        soracloud_fhe_bootstrap_key_proof_open_verify_bounds,
        soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1,
        soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds,
        soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1,
        soracloud_fhe_input_admission_open_verify_bounds,
        soracloud_fhe_input_admission_public_inputs_schema_hash_v1,
        soracloud_fhe_public_key_proof_open_verify_bounds,
        soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1,
    },
    sorafs::pin_registry::{PinStatus, StorageClass},
    zk::{BackendTag, OpenVerifyEnvelope, OpenVerifyEnvelopeBounds, StarkFriOpenProofV1},
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity, RoundingMode},
};
use mv::storage::StorageReadOnly;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::Duration,
};
const CAN_MANAGE_SORACLOUD_PERMISSION: &str = "CanManageSoracloud";
const CAN_GOVERN_SORACLOUD_FHE_PERMISSION: &str = "CanGovernSoracloudFhe";
#[cfg(test)]
const TAIRA_TESTNET_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const TRAINING_MAX_RETRIES: u8 = 16;
const TRAINING_MAX_WORKER_GROUP_SIZE: u16 = 1024;
const TRAINING_MAX_REASON_BYTES: usize = 512;
const TRAINING_MAX_IDENTIFIER_BYTES: usize = 128;
const MODEL_WEIGHT_MAX_DATASET_REF_BYTES: usize = 512;
const MODEL_WEIGHT_MAX_REASON_BYTES: usize = 512;
const MODEL_HOST_VIOLATION_MAX_DETAIL_BYTES: usize = 512;
const HF_REPO_ID_MAX_BYTES: usize = 256;
const HF_REVISION_MAX_BYTES: usize = 160;
const HF_MODEL_NAME_MAX_BYTES: usize = 128;
const AGENT_WALLET_DAY_TICKS: u64 = 10_000;
const AGENT_MAILBOX_MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_LABEL_BYTES: usize = 128;
const AGENT_AUTONOMY_MAX_REQUEST_BYTES: usize = 16 * 1024;
const HF_ADAPTIVE_TARGET_HOST_COUNT_SMALL: u16 = 3;
const HF_ADAPTIVE_TARGET_HOST_COUNT_MEDIUM: u16 = 2;
const HF_ADAPTIVE_TARGET_HOST_COUNT_LARGE: u16 = 2;
#[derive(Clone, Copy, Debug, Default)]
struct HfHostReservationUsage {
    required_model_bytes: u64,
    disk_cache_bytes: u64,
    ram_bytes: u64,
    vram_bytes: u64,
    resident_models: u16,
}
#[derive(Clone, Debug)]
struct HfHostClassPolicy {
    host_class: &'static str,
    min_model_bytes: u64,
    min_disk_cache_bytes: u64,
    min_ram_bytes: u64,
    min_vram_bytes: u64,
    reservation_fee_small: Quantity,
    reservation_fee_medium: Quantity,
    reservation_fee_large: Quantity,
}
fn hf_host_class_policies() -> &'static [HfHostClassPolicy; 3] {
    static POLICIES: OnceLock<[HfHostClassPolicy; 3]> = OnceLock::new();
    POLICIES.get_or_init(|| {
        [
            HfHostClassPolicy {
                host_class: "cpu.small",
                min_model_bytes: 2 * 1024 * 1024 * 1024,
                min_disk_cache_bytes: 8 * 1024 * 1024 * 1024,
                min_ram_bytes: 8 * 1024 * 1024 * 1024,
                min_vram_bytes: 0,
                reservation_fee_small: "0.0000005".parse().expect("small CPU tariff"),
                reservation_fee_medium: "0.00000075".parse().expect("medium CPU tariff"),
                reservation_fee_large: "0.000001".parse().expect("large CPU tariff"),
            },
            HfHostClassPolicy {
                host_class: "cpu.large",
                min_model_bytes: 8 * 1024 * 1024 * 1024,
                min_disk_cache_bytes: 32 * 1024 * 1024 * 1024,
                min_ram_bytes: 32 * 1024 * 1024 * 1024,
                min_vram_bytes: 0,
                reservation_fee_small: "0.000001".parse().expect("small CPU tariff"),
                reservation_fee_medium: "0.0000015".parse().expect("medium CPU tariff"),
                reservation_fee_large: "0.000002".parse().expect("large CPU tariff"),
            },
            HfHostClassPolicy {
                host_class: "gpu.large",
                min_model_bytes: 24 * 1024 * 1024 * 1024,
                min_disk_cache_bytes: 64 * 1024 * 1024 * 1024,
                min_ram_bytes: 64 * 1024 * 1024 * 1024,
                min_vram_bytes: 24 * 1024 * 1024 * 1024,
                reservation_fee_small: "0.0000025".parse().expect("small GPU tariff"),
                reservation_fee_medium: "0.000004".parse().expect("medium GPU tariff"),
                reservation_fee_large: "0.000006".parse().expect("large GPU tariff"),
            },
        ]
    })
}
fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}
fn single_signatory_authority(
    authority: &AccountId,
) -> Result<&PublicKey, InstructionExecutionError> {
    authority.try_signatory().ok_or_else(|| {
        invalid_parameter("Soracloud provenance requires a single-signatory transaction authority")
    })
}
fn invalid_quantity_arithmetic(
    context: &str,
    error: iroha_primitives::numeric::NumericOperationError,
) -> InstructionExecutionError {
    invalid_parameter(format!("{context}: {error}"))
}
fn verify_signature_for_signer(
    signature: &Signature,
    signer: &PublicKey,
    payload: &[u8],
) -> Result<(), iroha_crypto::Error> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())?;
        }
        _ => {}
    }
    signature.verify(signer, payload)
}
fn numeric_to_u128(value: &Numeric) -> Result<u128, InstructionExecutionError> {
    let mantissa = value
        .try_mantissa_u128()
        .ok_or_else(|| invalid_parameter(format!("numeric value `{value}` exceeds u128")))?;
    if value.scale() == 0 {
        return Ok(mantissa);
    }
    let scale = 10u128.checked_pow(value.scale()).ok_or_else(|| {
        invalid_parameter(format!("numeric value `{value}` has unsupported scale"))
    })?;
    if mantissa % scale != 0 {
        return Err(invalid_parameter(format!(
            "numeric value `{value}` must be an integer"
        )));
    }
    mantissa
        .checked_div(scale)
        .ok_or_else(|| invalid_parameter(format!("numeric value `{value}` underflowed")))
}
fn hf_host_class_policy(host_class: &str) -> Option<&'static HfHostClassPolicy> {
    hf_host_class_policies()
        .iter()
        .find(|policy| policy.host_class == host_class)
}
fn hf_adaptive_target_host_count(resource_profile: &SoraHfResourceProfileV1) -> u16 {
    match resource_profile.size_bucket() {
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Small => {
            HF_ADAPTIVE_TARGET_HOST_COUNT_SMALL
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Medium => {
            HF_ADAPTIVE_TARGET_HOST_COUNT_MEDIUM
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Large => {
            HF_ADAPTIVE_TARGET_HOST_COUNT_LARGE
        }
    }
}
fn hf_host_class_reservation_fee(
    host_class: &str,
    resource_profile: &SoraHfResourceProfileV1,
) -> Result<Quantity, InstructionExecutionError> {
    let policy = hf_host_class_policy(host_class)
        .ok_or_else(|| invalid_parameter(format!("unsupported model host class `{host_class}`")))?;
    Ok(match resource_profile.size_bucket() {
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Small => {
            policy.reservation_fee_small.clone()
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Medium => {
            policy.reservation_fee_medium.clone()
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Large => {
            policy.reservation_fee_large.clone()
        }
    })
}
fn ensure_hf_compute_reservation_charge_within_cap(
    charge: &Quantity,
    max_compute_reservation_fee: &Quantity,
) -> Result<(), InstructionExecutionError> {
    if charge > max_compute_reservation_fee {
        return Err(invalid_parameter(format!(
            "HF compute reservation charge `{charge}` exceeds the reviewed maximum `{max_compute_reservation_fee}`"
        )));
    }
    Ok(())
}
fn validate_model_host_capability_against_class(
    capability: &SoraModelHostCapabilityRecordV1,
) -> Result<(), InstructionExecutionError> {
    let policy = hf_host_class_policy(&capability.host_class).ok_or_else(|| {
        invalid_parameter(format!(
            "model host capability uses unsupported host class `{}`",
            capability.host_class
        ))
    })?;
    for (field, actual, minimum) in [
        (
            "max_model_bytes",
            capability.max_model_bytes,
            policy.min_model_bytes,
        ),
        (
            "max_disk_cache_bytes",
            capability.max_disk_cache_bytes,
            policy.min_disk_cache_bytes,
        ),
        (
            "max_ram_bytes",
            capability.max_ram_bytes,
            policy.min_ram_bytes,
        ),
        (
            "max_vram_bytes",
            capability.max_vram_bytes,
            policy.min_vram_bytes,
        ),
    ] {
        if actual < minimum {
            return Err(invalid_parameter(format!(
                "model host capability field `{field}` ({actual}) is below the `{}` class floor ({minimum})",
                capability.host_class
            )));
        }
    }
    Ok(())
}
fn active_hf_assigned_placements_for_validator(
    state_transaction: &StateTransaction<'_, '_>,
    validator_account_id: &AccountId,
    now_ms: u64,
) -> Vec<SoraHfPlacementRecordV1> {
    state_transaction
        .world
        .soracloud_hf_placements
        .iter()
        .filter_map(|(pool_id, placement)| {
            if placement.status == SoraHfPlacementStatusV1::Retired {
                return None;
            }
            let pool = state_transaction
                .world
                .soracloud_hf_shared_lease_pools
                .get(pool_id)?;
            if matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Expired | SoraHfSharedLeaseStatusV1::Retired
            ) {
                return None;
            }
            if pool.window_expires_at_ms <= now_ms && pool.queued_next_window.is_none() {
                return None;
            }
            placement
                .assigned_hosts
                .iter()
                .any(|assignment| {
                    assignment.validator_account_id == *validator_account_id
                        && !matches!(
                            assignment.status,
                            SoraHfPlacementHostStatusV1::Retired
                                | SoraHfPlacementHostStatusV1::Unavailable
                        )
                })
                .then_some(placement.clone())
        })
        .collect()
}
fn model_host_capability_advert_contradiction_detail(
    state_transaction: &StateTransaction<'_, '_>,
    capability: &SoraModelHostCapabilityRecordV1,
    now_ms: u64,
) -> Result<Option<String>, InstructionExecutionError> {
    if let Err(error) = validate_model_host_capability_against_class(capability) {
        return Ok(Some(error.to_string()));
    }
    let placements = active_hf_assigned_placements_for_validator(
        state_transaction,
        &capability.validator_account_id,
        now_ms,
    );
    let mut reserved_usage = HfHostReservationUsage::default();
    for placement in placements {
        if !capability
            .supported_backends
            .contains(&placement.resource_profile.backend_family)
        {
            return Ok(Some(format!(
                "model host capability no longer supports backend family `{:?}` required by placement `{}`",
                placement.resource_profile.backend_family, placement.placement_id
            )));
        }
        if !capability
            .supported_formats
            .contains(&placement.resource_profile.model_format)
        {
            return Ok(Some(format!(
                "model host capability no longer supports model format `{:?}` required by placement `{}`",
                placement.resource_profile.model_format, placement.placement_id
            )));
        }
        accumulate_hf_host_reservation_usage_totals(
            &mut reserved_usage,
            &placement.resource_profile,
        );
    }
    for (field, actual, required) in [
        (
            "max_model_bytes",
            capability.max_model_bytes,
            reserved_usage.required_model_bytes,
        ),
        (
            "max_disk_cache_bytes",
            capability.max_disk_cache_bytes,
            reserved_usage.disk_cache_bytes,
        ),
        (
            "max_ram_bytes",
            capability.max_ram_bytes,
            reserved_usage.ram_bytes,
        ),
        (
            "max_vram_bytes",
            capability.max_vram_bytes,
            reserved_usage.vram_bytes,
        ),
    ] {
        if actual < required {
            return Ok(Some(format!(
                "model host capability field `{field}` ({actual}) is below the active assigned reservation total ({required})"
            )));
        }
    }
    if capability.max_concurrent_resident_models < reserved_usage.resident_models {
        return Ok(Some(format!(
            "model host capability field `max_concurrent_resident_models` ({}) is below the active assigned reservation total ({})",
            capability.max_concurrent_resident_models, reserved_usage.resident_models
        )));
    }
    Ok(None)
}
fn require_soracloud_permission(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let required = Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(()));
    let has_direct = state_transaction
        .world
        .account_permissions_iter(authority)
        .is_ok_and(|permissions| permissions.into_iter().any(|actual| actual == &required));
    let has_role = state_transaction
        .world
        .account_roles_iter(authority)
        .any(|role_id| {
            state_transaction
                .world
                .roles
                .get(role_id)
                .is_some_and(|role| role.permissions().any(|actual| actual == &required))
        });
    if has_direct || has_role {
        Ok(())
    } else {
        Err(InstructionExecutionError::InvariantViolation(
            format!("not permitted: {CAN_MANAGE_SORACLOUD_PERMISSION}").into(),
        ))
    }
}
fn require_soracloud_fhe_governance_permission(
    authority: &AccountId,
    service_name: &Name,
    policy_name: &Name,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let scope = SoracloudFheGovernancePermissionScopeV1 {
        schema_version: SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
        service_name: service_name.clone(),
        policy_name: policy_name.clone(),
    };
    let required = Permission::new(CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(), Json::new(scope));
    let has_direct = state_transaction
        .world
        .account_permissions_iter(authority)
        .is_ok_and(|permissions| permissions.into_iter().any(|actual| actual == &required));
    let has_role = state_transaction
        .world
        .account_roles_iter(authority)
        .any(|role_id| {
            state_transaction
                .world
                .roles
                .get(role_id)
                .is_some_and(|role| role.permissions().any(|actual| actual == &required))
        });
    if has_direct || has_role {
        Ok(())
    } else {
        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "not permitted: {CAN_GOVERN_SORACLOUD_FHE_PERMISSION} for service `{service_name}` policy `{policy_name}`"
            )
            .into(),
        ))
    }
}
fn current_signed_transaction_hash(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Hash, InstructionExecutionError> {
    state_transaction
        .current_tx_hash
        .map(Into::into)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "Soracloud FHE governance requires the canonical signed transaction hash".into(),
            )
        })
}
fn require_active_public_lane_validator(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let is_active_validator =
        state_transaction
            .world
            .public_lane_validators
            .iter()
            .any(|(key, record)| {
                public_lane_validator_record_matches_key(key, record)
                    && key.1 == *authority
                    && record.status == PublicLaneValidatorStatus::Active
                    && state_transaction.is_lane_active_for_authority(key.0)
            });
    if is_active_validator {
        Ok(())
    } else {
        Err(InstructionExecutionError::InvariantViolation(
            format!("account `{authority}` is not an active public-lane validator").into(),
        ))
    }
}
fn require_soracloud_runtime_authority(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    require_soracloud_permission(authority, state_transaction)
        .or_else(|_| require_active_public_lane_validator(authority, state_transaction))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoracloudServiceRuntimeAuthority {
    Manager,
    AssignedValidator,
}
/// Authorize a service-scoped runtime mutation.
///
/// SoraCloud managers may repair or reconcile any service. Runtime validators
/// must be active and assigned to the exact service revision they mutate.
fn require_soracloud_service_runtime_authority(
    authority: &AccountId,
    service_name: &Name,
    service_version: &str,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<SoracloudServiceRuntimeAuthority, InstructionExecutionError> {
    if require_soracloud_permission(authority, state_transaction).is_ok() {
        return Ok(SoracloudServiceRuntimeAuthority::Manager);
    }
    require_active_public_lane_validator(authority, state_transaction)?;
    let deployment = state_transaction
        .world
        .soracloud_service_deployments
        .get(service_name)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("service `{service_name}` is not deployed").into(),
            )
        })?;
    if !active_inrou_service_versions(deployment)
        .iter()
        .any(|active_version| active_version == service_version)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "service `{service_name}` revision `{service_version}` is not an active deployment revision"
            )
            .into(),
        ));
    }
    let key = (service_name.as_ref().to_owned(), service_version.to_owned());
    let placement = state_transaction
        .world
        .soracloud_inrou_service_placements
        .get(&key)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{service_name}` revision `{service_version}` has no active runtime placement"
                )
                .into(),
            )
        })?;
    if placement.service_name != *service_name
        || placement.service_version != service_version
        || placement.schema_version != SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "service `{service_name}` revision `{service_version}` has a malformed runtime placement record"
            )
            .into(),
        ));
    }
    if placement
        .placements
        .iter()
        .any(|assignment| assignment.validator_account_id == *authority)
    {
        Ok(SoracloudServiceRuntimeAuthority::AssignedValidator)
    } else {
        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "validator `{authority}` is not assigned to service `{service_name}` revision `{service_version}`"
            )
            .into(),
        ))
    }
}
fn verify_bundle_provenance(
    authority: &AccountId,
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    initial_service_secrets: &BTreeMap<String, SecretEnvelopeV1>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "bundle provenance signer must match the transaction authority",
        ));
    }
    let payload = iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
        bundle,
        initial_service_configs,
        initial_service_secrets,
    )
    .map_err(|err| invalid_parameter(format!("failed to encode bundle provenance: {err}")))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter("bundle provenance signature verification failed"))?;
    Ok(())
}
fn verify_app_infra_provenance(
    authority: &AccountId,
    manifest: &SoraAppInfraManifestV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "app infra provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_app_infra_provenance_payload(manifest).map_err(|err| {
        invalid_parameter(format!("failed to encode app infra provenance: {err}"))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter("app infra provenance signature verification failed"))?;
    Ok(())
}
fn verify_rollback_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    target_version: Option<&str>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "rollback provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_rollback_provenance_payload(service_name.as_ref(), target_version)
        .map_err(|err| invalid_parameter(format!("failed to encode rollback provenance: {err}")))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter("rollback provenance signature verification failed"))?;
    Ok(())
}
fn service_config_value_hash(value_json: &Json) -> Result<Hash, InstructionExecutionError> {
    norito::json::to_vec(value_json)
        .map(Hash::new)
        .map_err(|err| invalid_parameter(format!("failed to encode service config json: {err}")))
}
fn verify_service_config_set_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    config_name: &str,
    value_json: &Json,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "service config provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_set_service_config_provenance_payload(
        service_name.as_ref(),
        config_name,
        value_json,
    )
    .map_err(|err| {
        invalid_parameter(format!("failed to encode service config provenance: {err}"))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("service config provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_service_config_delete_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    config_name: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "service config delete provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_delete_service_config_provenance_payload(service_name.as_ref(), config_name)
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to encode service config delete provenance: {err}"
                ))
            })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("service config delete provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_service_secret_set_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    secret_name: &str,
    secret: &SecretEnvelopeV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "service secret provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_set_service_secret_provenance_payload(service_name.as_ref(), secret_name, secret)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode service secret provenance: {err}"))
            })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("service secret provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_service_secret_delete_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    secret_name: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "service secret delete provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_delete_service_secret_provenance_payload(service_name.as_ref(), secret_name)
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to encode service secret delete provenance: {err}"
                ))
            })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("service secret delete provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_rollout_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    rollout_handle: &str,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "rollout provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_rollout_provenance_payload(
        service_name.as_ref(),
        rollout_handle,
        healthy,
        promote_to_percent,
        governance_tx_hash,
    )
    .map_err(|err| invalid_parameter(format!("failed to encode rollout provenance: {err}")))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter("rollout provenance signature verification failed"))?;
    Ok(())
}
fn verify_state_mutation_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    state_key: &str,
    operation: SoraStateMutationOperationV1,
    value_size_bytes: Option<u64>,
    payload_commitment: Option<Hash>,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "state mutation provenance signer must match the transaction authority",
        ));
    }
    let operation_label = match operation {
        SoraStateMutationOperationV1::Upsert => "upsert",
        SoraStateMutationOperationV1::Delete => "delete",
    };
    let payload = encode_state_mutation_provenance_payload(
        service_name.as_ref(),
        binding_name.as_ref(),
        state_key,
        operation_label,
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
        fhe_input_admission_proof,
    )
    .map_err(|err| {
        invalid_parameter(format!("failed to encode state mutation provenance: {err}"))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("state mutation provenance signature verification failed"),
    )?;
    Ok(())
}
fn state_mutation_operation_label(operation: SoraStateMutationOperationV1) -> &'static str {
    match operation {
        SoraStateMutationOperationV1::Upsert => "upsert",
        SoraStateMutationOperationV1::Delete => "delete",
    }
}
#[allow(clippy::too_many_arguments)]
fn expected_fhe_input_admission_statement_hash(
    service_name: &Name,
    binding_name: &Name,
    state_key: &str,
    operation: SoraStateMutationOperationV1,
    value_size_bytes: u64,
    payload_commitment: Hash,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    ciphertext_proof_statement_digests: &[Hash],
    residual_multiple_bound: u128,
    bound_mode: BfvCiphertextBoundModeV1,
) -> Result<Hash, InstructionExecutionError> {
    let params = ram_lfe_bfv_parameters_v1();
    validate_registered_bfv_parameters(&params)
        .map_err(|err| invalid_parameter(format!("invalid registered BFV parameters: {err}")))?;
    let parameter_digest = registered_bfv_parameter_digest(&params)
        .map_err(|err| invalid_parameter(format!("failed to digest BFV parameters: {err}")))?;
    let rns_modulus_chain_digest = registered_bfv_rns_modulus_chain_digest(&params)
        .map_err(|err| invalid_parameter(format!("failed to digest BFV RNS chain: {err}")))?;
    let key_switch_decomposition_chain_digest =
        registered_bfv_key_switch_decomposition_chain_digest(&params).map_err(|err| {
            invalid_parameter(format!(
                "failed to digest BFV key-switch decomposition chain: {err}"
            ))
        })?;
    derive_soracloud_fhe_input_admission_statement_hash_with_bound_mode(
        service_name.as_ref(),
        binding_name.as_ref(),
        state_key,
        state_mutation_operation_label(operation),
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
        parameter_digest,
        rns_modulus_chain_digest,
        key_switch_decomposition_chain_digest,
        ciphertext_proof_statement_digests,
        residual_multiple_bound,
        bound_mode,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to derive FHE input admission statement hash: {err}"
        ))
    })
}
fn validate_soracloud_fhe_envelope_shape(
    params: &BfvParameters,
    envelope: &BfvIdentifierCiphertext,
    context: &str,
) -> Result<(), InstructionExecutionError> {
    if envelope.slots.is_empty() {
        return Err(invalid_parameter(
            "fhe ciphertext envelope must contain at least one slot",
        ));
    }
    if envelope.slots.len() > RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT {
        return Err(invalid_parameter(format!(
            "{context} ciphertext envelope slot count {} exceeds registered RAM-LFE BFV identifier slot count {RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT}",
            envelope.slots.len()
        )));
    }
    for (index, slot) in envelope.slots.iter().enumerate() {
        multiply_plain_scalar(params, slot, 1).map_err(|err| {
            invalid_parameter(format!("invalid FHE ciphertext slot[{index}]: {err}"))
        })?;
    }
    Ok(())
}
fn derive_soracloud_fhe_input_ciphertext_statement_digests(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    envelope: &BfvIdentifierCiphertext,
    declared_bound: u128,
    bound_mode: BfvCiphertextBoundModeV1,
    context: &str,
) -> Result<Vec<Hash>, InstructionExecutionError> {
    envelope
        .slots
        .iter()
        .enumerate()
        .map(|(index, ciphertext)| {
            let digest = match bound_mode {
                BfvCiphertextBoundModeV1::ExactResidualMultiple => {
                    bfv_ciphertext_exact_residual_proof_statement_digest(
                        params,
                        public_key,
                        ciphertext,
                        declared_bound,
                    )
                }
                BfvCiphertextBoundModeV1::BoundedNoise => {
                    bfv_bounded_noise_ciphertext_proof_statement_digest(
                        params,
                        public_key,
                        ciphertext,
                        declared_bound,
                    )
                }
            };
            digest.map_err(|err| {
                invalid_parameter(format!(
                    "{context} ciphertext proof statement digest slot[{index}] failed: {err}"
                ))
            })
        })
        .collect()
}
struct SoracloudFheProofAttachmentDecodeContext {
    proof_backend_mismatch: &'static str,
    verifier_backend_mismatch: &'static str,
    verifier_name_empty: &'static str,
    unsupported_backend: &'static str,
    required_backend: Option<&'static str>,
    invalid_attachment_prefix: &'static str,
    open_verify_label: &'static str,
    max_open_verify_bytes: usize,
}
const FHE_INPUT_ADMISSION_ATTACHMENT_CONTEXT: SoracloudFheProofAttachmentDecodeContext =
    SoracloudFheProofAttachmentDecodeContext {
        proof_backend_mismatch: "fhe input admission proof backend mismatch",
        verifier_backend_mismatch: "fhe input admission verifier backend mismatch",
        verifier_name_empty: "fhe input admission verifier name must not be empty",
        unsupported_backend: "Soracloud FHE input admission requires the canonical BFV STARK/FRI backend",
        required_backend: Some(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1),
        invalid_attachment_prefix: "invalid FHE input admission proof attachment",
        open_verify_label: "FHE input admission OpenVerifyEnvelope",
        max_open_verify_bytes: SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES,
    };
const FHE_PUBLIC_KEY_PROOF_ATTACHMENT_CONTEXT: SoracloudFheProofAttachmentDecodeContext =
    SoracloudFheProofAttachmentDecodeContext {
        proof_backend_mismatch: "fhe public-key proof backend mismatch",
        verifier_backend_mismatch: "fhe public-key verifier backend mismatch",
        verifier_name_empty: "fhe public-key verifier name must not be empty",
        unsupported_backend: "Soracloud FHE public-key proof requires the canonical BFV STARK/FRI backend",
        required_backend: Some(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1),
        invalid_attachment_prefix: "invalid FHE public-key proof attachment",
        open_verify_label: "FHE public-key proof OpenVerifyEnvelope",
        max_open_verify_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES,
    };
const FHE_BOOTSTRAP_KEY_PROOF_ATTACHMENT_CONTEXT: SoracloudFheProofAttachmentDecodeContext =
    SoracloudFheProofAttachmentDecodeContext {
        proof_backend_mismatch: "fhe bootstrap-key proof backend mismatch",
        verifier_backend_mismatch: "fhe bootstrap-key verifier backend mismatch",
        verifier_name_empty: "fhe bootstrap-key verifier name must not be empty",
        unsupported_backend: "Soracloud FHE bootstrap-key proof requires the canonical BFV STARK/FRI backend",
        required_backend: Some(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1),
        invalid_attachment_prefix: "invalid FHE bootstrap-key proof attachment",
        open_verify_label: "FHE bootstrap-key proof OpenVerifyEnvelope",
        max_open_verify_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES,
    };
const FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_ATTACHMENT_CONTEXT:
    SoracloudFheProofAttachmentDecodeContext = SoracloudFheProofAttachmentDecodeContext {
    proof_backend_mismatch: "fhe full-bootstrap execution proof backend mismatch",
    verifier_backend_mismatch: "fhe full-bootstrap execution proof verifier backend mismatch",
    verifier_name_empty: "fhe full-bootstrap execution proof verifier name must not be empty",
    unsupported_backend: "Soracloud FHE full-bootstrap execution proof requires the canonical BFV full-bootstrap STARK/FRI backend",
    required_backend: Some(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1),
    invalid_attachment_prefix: "invalid FHE full-bootstrap execution proof attachment",
    open_verify_label: "FHE full-bootstrap execution proof OpenVerifyEnvelope",
    max_open_verify_bytes: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES,
};
const FHE_INPUT_ADMISSION_DEDICATED_WITNESS_AIR_REQUIRED: &str = "FHE input admission requires a dedicated BFV ciphertext witness AIR; binding-only STARK AIR does not prove ciphertext residual or noise bounds";
const FHE_PUBLIC_KEY_DEDICATED_WITNESS_AIR_REQUIRED: &str = "FHE public-key proof requires a dedicated BFV key-generation witness AIR; binding-only STARK AIR does not prove secret-key/public-key consistency or residual/noise bounds";
const FHE_BOOTSTRAP_KEY_DEDICATED_WITNESS_AIR_REQUIRED: &str = "FHE bootstrap-key proof requires a dedicated BFV zero-refresh witness AIR; binding-only STARK AIR does not prove zero-encryption refresh ciphertexts or round consistency";
#[cfg(all(test, feature = "zk-stark"))]
const FHE_FULL_BOOTSTRAP_DEDICATED_PROVER_UNAVAILABLE: &str =
    "dedicated BFV full-bootstrap arithmetic STARK prover is not available";
#[cfg(feature = "zk-stark")]
const FHE_FULL_BOOTSTRAP_GENERIC_BINDING_AIR_REJECTED: &str =
    "generic binding AIR proofs are not accepted for BFV full-bootstrap production proofs";
fn proof_attachment_envelope_with_context(
    attachment: &ProofAttachment,
    context: &SoracloudFheProofAttachmentDecodeContext,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    if attachment.backend != attachment.proof.backend {
        return Err(invalid_parameter(context.proof_backend_mismatch));
    }
    if attachment.vk_ref.backend != attachment.backend {
        return Err(invalid_parameter(context.verifier_backend_mismatch));
    }
    if attachment.vk_ref.name.trim().is_empty() {
        return Err(invalid_parameter(context.verifier_name_empty));
    }
    if !crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str()) {
        return Err(invalid_parameter(context.unsupported_backend));
    }
    if let Some(required_backend) = context.required_backend {
        if attachment.backend != required_backend {
            return Err(invalid_parameter(context.unsupported_backend));
        }
    }
    if let Some((field, reason)) = attachment.structural_error() {
        return Err(invalid_parameter(format!(
            "{}: {field} {reason}",
            context.invalid_attachment_prefix
        )));
    }
    if attachment.proof.bytes.len() > context.max_open_verify_bytes {
        return Err(invalid_parameter(format!(
            "{} length {} exceeds maximum {}",
            context.open_verify_label,
            attachment.proof.bytes.len(),
            context.max_open_verify_bytes
        )));
    }
    norito::decode_canonical::<OpenVerifyEnvelope>(&attachment.proof.bytes)
        .map_err(|err| invalid_parameter(format!("invalid {}: {err}", context.open_verify_label)))
}
fn proof_attachment_envelope(
    attachment: &ProofAttachment,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    proof_attachment_envelope_with_context(attachment, &FHE_INPUT_ADMISSION_ATTACHMENT_CONTEXT)
}
fn public_key_proof_attachment_envelope(
    attachment: &ProofAttachment,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    proof_attachment_envelope_with_context(attachment, &FHE_PUBLIC_KEY_PROOF_ATTACHMENT_CONTEXT)
}
fn bootstrap_key_proof_attachment_envelope(
    attachment: &ProofAttachment,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    proof_attachment_envelope_with_context(attachment, &FHE_BOOTSTRAP_KEY_PROOF_ATTACHMENT_CONTEXT)
}
fn full_bootstrap_execution_proof_attachment_envelope(
    attachment: &ProofAttachment,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    proof_attachment_envelope_with_context(
        attachment,
        &FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_ATTACHMENT_CONTEXT,
    )
}
fn validate_soracloud_fhe_stark_native_envelope_bytes(
    label: &str,
    envelope_bytes: &[u8],
    max_bytes: usize,
) -> Result<(), InstructionExecutionError> {
    if envelope_bytes.is_empty() {
        return Err(invalid_parameter(format!(
            "{label} STARK native envelope bytes must be non-empty"
        )));
    }
    if envelope_bytes.iter().all(|byte| *byte == 0) {
        return Err(invalid_parameter(format!(
            "{label} STARK native envelope bytes must not be all-zero"
        )));
    }
    if envelope_bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(invalid_parameter(format!(
            "{label} STARK native envelope bytes must not be blank"
        )));
    }
    if envelope_bytes.len() > max_bytes {
        return Err(invalid_parameter(format!(
            "{label} STARK native envelope bytes length {} exceeds maximum {}",
            envelope_bytes.len(),
            max_bytes
        )));
    }
    if soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(envelope_bytes) {
        return Err(invalid_parameter(format!(
            "{label} STARK native envelope bytes must not be placeholder or non-production text"
        )));
    }
    Ok(())
}
const SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS: &[&[u8]] = &[
    b"placeholder",
    b"not production ready",
    b"not-production-ready",
    b"not for production",
    b"not-for-production",
    b"replace before production",
    b"replace-before-production",
    b"replace_before_production",
    b"replace-me",
    b"replace me",
    b"replace_me",
    b"changeme",
    b"change-me",
    b"change me",
    b"change_me",
    b"test-only",
    b"test only",
    b"test_only",
    b"your-",
    b"your_",
    b"your-audit",
    b"your audit",
    b"your_audit",
    b"your-proof",
    b"your proof",
    b"your_proof",
    b"todo pending",
    b"todo",
    b"pending native stark",
    b"pending stark",
    b"not_for_production",
    b"not_production_ready",
    b"draft",
    b"draft-only",
    b"draft only",
    b"draft_only",
    b"dummy",
    b"fake",
    b"stub",
    b"mock",
    b"fixture",
    b"sample",
    b"template",
    b"example",
];
fn soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(envelope_bytes: &[u8]) -> bool {
    let is_text_byte = |byte: &u8| byte.is_ascii_graphic() || byte.is_ascii_whitespace();
    if envelope_bytes.iter().all(is_text_byte) {
        return soracloud_fhe_stark_native_envelope_text_span_is_placeholder(
            envelope_bytes,
            SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
        );
    }
    if envelope_bytes
        .split(|byte| !is_text_byte(byte))
        .any(|decorated_text| {
            !decorated_text.is_empty()
                && !decorated_text.iter().all(u8::is_ascii_whitespace)
                && soracloud_fhe_stark_native_envelope_text_span_is_placeholder(
                    decorated_text,
                    SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
                )
        })
    {
        return true;
    }
    soracloud_fhe_stark_native_envelope_fragmented_text_is_placeholder(
        envelope_bytes,
        SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
    )
}
fn soracloud_fhe_stark_native_envelope_text_span_is_placeholder(
    text: &[u8],
    markers: &[&[u8]],
) -> bool {
    let mut lower = Vec::with_capacity(text.len());
    lower.extend(text.iter().map(u8::to_ascii_lowercase));
    soracloud_ascii_text_contains_placeholder_marker(&lower, markers)
}
fn soracloud_fhe_stark_native_envelope_fragmented_text_is_placeholder(
    bytes: &[u8],
    markers: &[&[u8]],
) -> bool {
    let mut collapsed_text = Vec::with_capacity(bytes.len());
    collapsed_text.extend(
        bytes
            .iter()
            .filter(|byte| byte.is_ascii_alphanumeric())
            .map(u8::to_ascii_lowercase),
    );
    !collapsed_text.is_empty()
        && soracloud_collapsed_placeholder_markers(markers)
            .iter()
            .any(|marker| ascii_windows_contains(&collapsed_text, marker))
}
const SORACLOUD_COLLAPSED_PLACEHOLDER_MARKER_MIN_BYTES: usize = 5;
fn soracloud_ascii_text_contains_placeholder_marker(normalized: &[u8], markers: &[&[u8]]) -> bool {
    if markers
        .iter()
        .any(|marker| ascii_windows_contains(normalized, marker))
    {
        return true;
    }
    let collapsed_text = ascii_alnum_collapsed(normalized);
    if collapsed_text.is_empty() {
        return false;
    }
    soracloud_collapsed_placeholder_markers(markers)
        .iter()
        .any(|marker| ascii_windows_contains(&collapsed_text, marker))
}
fn soracloud_collapsed_placeholder_markers(markers: &[&[u8]]) -> &'static [Vec<u8>] {
    debug_assert!(std::ptr::eq(
        markers,
        SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
    ));
    static SORACLOUD_COLLAPSED_PLACEHOLDER_MARKERS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    SORACLOUD_COLLAPSED_PLACEHOLDER_MARKERS.get_or_init(|| {
        markers
            .iter()
            .map(|marker| ascii_alnum_collapsed(marker))
            .filter(|marker| marker.len() >= SORACLOUD_COLLAPSED_PLACEHOLDER_MARKER_MIN_BYTES)
            .collect()
    })
}
fn ascii_alnum_collapsed(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(u8::is_ascii_alphanumeric)
        .collect()
}
fn ascii_windows_contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
fn validate_soracloud_fhe_input_admission_native_envelope_bytes(
    envelope_bytes: &[u8],
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "FHE input admission",
        envelope_bytes,
        SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES,
    )
}
fn validate_soracloud_fhe_input_admission_envelope(
    attachment: &ProofAttachment,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(
            "FHE input admission proof envelope must declare STARK backend",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(
            "FHE input admission proof envelope aux must be empty",
        ));
    }
    envelope
        .validate_with_bounds(soracloud_fhe_input_admission_open_verify_bounds())
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE input admission OpenVerifyEnvelope shape: {err}"
            ))
        })?;
    if envelope.circuit_id != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof circuit id must be canonical v1",
        ));
    }
    if envelope.public_inputs != SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE input admission STARK public-input wrapper: {err}"
            ))
        })?;
    if open.version != 1 {
        return Err(invalid_parameter(
            "FHE input admission STARK public-input wrapper version must be 1",
        ));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(
            "FHE input admission proof public inputs do not match statement hash",
        ));
    }
    validate_soracloud_fhe_input_admission_native_envelope_bytes(&open.envelope_bytes)?;
    let vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires vk_commitment"))?;
    if vk_commitment != envelope.vk_hash {
        return Err(invalid_parameter(
            "FHE input admission proof vk_commitment mismatch",
        ));
    }
    let envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires envelope_hash"))?;
    let expected = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if envelope_hash != expected {
        return Err(invalid_parameter(
            "FHE input admission proof envelope_hash mismatch",
        ));
    }
    Ok(())
}
fn verify_soracloud_fhe_input_admission_backend(
    attachment: &ProofAttachment,
    statement_hash: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let attachment_vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires vk_commitment"))?;
    let attachment_envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires envelope_hash"))?;
    let expected_envelope_hash = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if attachment_envelope_hash != expected_envelope_hash {
        return Err(invalid_parameter(
            "FHE input admission proof envelope_hash mismatch",
        ));
    }
    if attachment.vk_ref.name != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof vk_ref must use the canonical v1 circuit id",
        ));
    }
    let envelope = proof_attachment_envelope(attachment)?;
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(
            "FHE input admission proof envelope must declare STARK backend",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(
            "FHE input admission proof envelope aux must be empty",
        ));
    }
    envelope
        .validate_with_bounds(soracloud_fhe_input_admission_open_verify_bounds())
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE input admission OpenVerifyEnvelope shape: {err}"
            ))
        })?;
    if envelope.circuit_id != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof circuit id must be canonical v1",
        ));
    }
    if envelope.public_inputs != SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE input admission STARK public-input wrapper: {err}"
            ))
        })?;
    if open.version != 1 {
        return Err(invalid_parameter(
            "FHE input admission STARK public-input wrapper version must be 1",
        ));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(
            "FHE input admission proof public inputs do not match statement hash",
        ));
    }
    validate_soracloud_fhe_input_admission_native_envelope_bytes(&open.envelope_bytes)?;
    let record = state_transaction
        .world
        .verifying_keys
        .get(&attachment.vk_ref)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "FHE input admission verifying key not found".into(),
            )
        })?;
    if !record.is_active_at(state_transaction.block_height()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key is not active".into(),
        ));
    }
    if record.namespace != "soracloud" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key must be in the soracloud namespace".into(),
        ));
    }
    if record.backend != BackendTag::Stark {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key must use STARK backend".into(),
        ));
    }
    if record.curve != "goldilocks" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key must use goldilocks STARK field".into(),
        ));
    }
    if record.public_inputs_schema_hash
        != soracloud_fhe_input_admission_public_inputs_schema_hash_v1()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key public-input schema mismatch".into(),
        ));
    }
    if record.circuit_id != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key must use the canonical v1 circuit".into(),
        ));
    }
    if record.version != u32::from(SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key must use the canonical v1 circuit version".into(),
        ));
    }
    if record.gas_schedule_id.as_deref() != Some(SORACLOUD_FHE_INPUT_ADMISSION_GAS_SCHEDULE_ID_V1) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key gas_schedule_id mismatch".into(),
        ));
    }
    let circuit_key = (record.circuit_id.clone(), record.version);
    match state_transaction
        .world
        .verifying_keys_by_circuit
        .get(&circuit_key)
    {
        Some(active_id) if active_id == &attachment.vk_ref => {}
        _ => {
            return Err(InstructionExecutionError::InvariantViolation(
                "FHE input admission verifying key circuit/version not active".into(),
            ));
        }
    }
    if envelope.circuit_id != record.circuit_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission proof circuit mismatch".into(),
        ));
    }
    if envelope.vk_hash != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission proof verifying-key commitment mismatch".into(),
        ));
    }
    if attachment_vk_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission attachment verifying-key commitment mismatch".into(),
        ));
    }
    if record.max_proof_bytes > 0
        && attachment.proof.bytes.len()
            > usize::try_from(record.max_proof_bytes).unwrap_or(usize::MAX)
    {
        return Err(invalid_parameter(
            "FHE input admission proof exceeds verifying key max_proof_bytes",
        ));
    }
    let vk_box = record.key.clone().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key bytes missing".into(),
        )
    })?;
    if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key vk_len mismatch".into(),
        ));
    }
    let actual_commitment = crate::zk::hash_vk(&vk_box);
    if actual_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key commitment mismatch".into(),
        ));
    }
    if vk_box.backend != attachment.backend {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key backend mismatch".into(),
        ));
    }
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_verifier_key_payload(
        "FHE input admission",
        &vk_box,
        SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
    )?;
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_native_air_binding(
        "FHE input admission",
        attachment.backend.as_str(),
        &envelope,
        statement_hash,
    )?;
    // TODO: Implement a dedicated BFV ciphertext witness AIR that proves the declared
    // exact-residual-multiple or bounded-noise relation from private encryption witness
    // material. Even a cryptographically valid binding AIR would authenticate only public
    // statement metadata.
    Err(invalid_parameter(
        FHE_INPUT_ADMISSION_DEDICATED_WITNESS_AIR_REQUIRED,
    ))
}
fn validate_soracloud_fhe_bootstrap_key_proof_native_envelope_bytes(
    envelope_bytes: &[u8],
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "FHE bootstrap-key proof",
        envelope_bytes,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )
}
fn validate_soracloud_fhe_public_key_proof_native_envelope_bytes(
    envelope_bytes: &[u8],
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "FHE public-key proof",
        envelope_bytes,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )
}
fn validate_soracloud_fhe_full_bootstrap_execution_proof_native_envelope_bytes(
    envelope_bytes: &[u8],
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "FHE full-bootstrap execution proof",
        envelope_bytes,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )
}
struct SoracloudFheStatementOpenVerifyContract {
    proof_label: &'static str,
    shape_label: &'static str,
    wrapper_label: &'static str,
    expected_circuit_id: &'static str,
    expected_public_inputs_schema: &'static [u8],
    validate_native_envelope_bytes: fn(&[u8]) -> Result<(), InstructionExecutionError>,
}
const FHE_FULL_BOOTSTRAP_EXECUTION_OPEN_VERIFY_CONTRACT: SoracloudFheStatementOpenVerifyContract =
    SoracloudFheStatementOpenVerifyContract {
        proof_label: "FHE full-bootstrap execution proof",
        shape_label: "FHE full-bootstrap execution OpenVerifyEnvelope",
        wrapper_label: "FHE full-bootstrap execution STARK public-input wrapper",
        expected_circuit_id: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        expected_public_inputs_schema:
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        validate_native_envelope_bytes:
            validate_soracloud_fhe_full_bootstrap_execution_proof_native_envelope_bytes,
    };
fn validate_soracloud_fhe_statement_open_verify_envelope(
    contract: &SoracloudFheStatementOpenVerifyContract,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
    bounds: OpenVerifyEnvelopeBounds,
) -> Result<StarkFriOpenProofV1, InstructionExecutionError> {
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(format!(
            "{} envelope must declare STARK backend",
            contract.proof_label
        )));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(format!(
            "{} envelope aux must be empty",
            contract.proof_label
        )));
    }
    envelope.validate_with_bounds(bounds).map_err(|err| {
        invalid_parameter(format!("invalid {} shape: {err}", contract.shape_label))
    })?;
    if envelope.circuit_id != contract.expected_circuit_id {
        return Err(invalid_parameter(format!(
            "{} circuit id must be canonical v1",
            contract.proof_label
        )));
    }
    if envelope.public_inputs != contract.expected_public_inputs_schema {
        return Err(invalid_parameter(format!(
            "{} public-input schema mismatch",
            contract.proof_label
        )));
    }
    let open = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .map_err(|err| invalid_parameter(format!("invalid {}: {err}", contract.wrapper_label)))?;
    if open.version != 1 {
        return Err(invalid_parameter(format!(
            "{} version must be 1",
            contract.wrapper_label
        )));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(format!(
            "{} public inputs do not match statement hash",
            contract.proof_label
        )));
    }
    (contract.validate_native_envelope_bytes)(&open.envelope_bytes)?;
    Ok(open)
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_native_air_statement_binding_v1(
    label: &str,
    envelope_bytes: &[u8],
    statement_hash: Hash,
    expected_transcript_label: &str,
    expected_circuit_id: &str,
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_prover_statement_hash(label, statement_hash)?;
    let native: crate::zk_stark::StarkVerifyEnvelopeV1 = norito::decode_canonical(envelope_bytes)
        .map_err(|err| {
        invalid_parameter(format!(
            "{label} native AIR envelope must decode as STARK/FRI v1: {err}"
        ))
    })?;
    let transcript_label_matches = if expected_transcript_label
        == iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
    {
        soracloud_fhe_full_bootstrap_native_air_transcript_label_is_allowed_v1(
            &native.transcript_label,
        )
    } else {
        native.transcript_label == expected_transcript_label
    };
    if !transcript_label_matches {
        return Err(invalid_parameter(format!(
            "{label} native AIR transcript label mismatch"
        )));
    }
    let Some(air) = native.proof.air.as_ref() else {
        return Err(invalid_parameter(format!(
            "{label} proof requires native AIR section"
        )));
    };
    if air.circuit_id != expected_circuit_id {
        return Err(invalid_parameter(format!(
            "{label} native AIR circuit id mismatch"
        )));
    }
    if expected_transcript_label
        == iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
        && !soracloud_fhe_full_bootstrap_native_air_domain_tag_matches_v1(
            statement_hash,
            &native.params.domain_tag,
        )
    {
        return Err(invalid_parameter(format!(
            "{label} native AIR domain tag mismatch"
        )));
    }
    if native.proof.commits.comp_root.is_some() || native.proof.comp_values.is_some() {
        return Err(invalid_parameter(format!(
            "{label} native AIR must not carry auxiliary composition value commitments"
        )));
    }
    if air.public_digest != <[u8; Hash::LENGTH]>::from(statement_hash) {
        return Err(invalid_parameter(format!(
            "{label} native AIR public digest mismatch"
        )));
    }
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_stark_verifier_key_payload(
    label: &str,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    expected_circuit_id: &str,
) -> Result<(), InstructionExecutionError> {
    if verifier_key.backend != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Err(invalid_parameter(format!(
            "{label} verifier-key backend mismatch"
        )));
    }
    let payload: crate::zk_stark::StarkFriVerifyingKeyV1 =
        norito::decode_canonical(&verifier_key.bytes).map_err(|err| {
            invalid_parameter(format!(
                "{label} verifier-key has invalid STARK payload: {err}"
            ))
        })?;
    crate::zk_stark::validate_stark_fri_canonical_verifying_key_payload(
        &payload,
        expected_circuit_id,
        label,
    )
    .map_err(invalid_parameter)?;
    if payload.hash_fn != crate::zk_stark::STARK_HASH_SHA256_V1 {
        return Err(invalid_parameter(format!(
            "{label} verifier-key must use SHA-256 STARK/FRI"
        )));
    }
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_stark_native_air_binding(
    label: &str,
    backend: &str,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!("invalid {label} STARK public-input wrapper: {err}"))
        })?;
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(format!(
            "{label} proof public inputs do not match statement hash"
        )));
    }
    let native: crate::zk_stark::StarkVerifyEnvelopeV1 =
        norito::decode_canonical(&open.envelope_bytes).map_err(|err| {
            invalid_parameter(format!(
                "{label} native AIR envelope must decode as STARK/FRI v1: {err}"
            ))
        })?;
    if native.transcript_label != crate::zk::STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1 {
        return Err(invalid_parameter(format!(
            "{label} native AIR transcript label mismatch"
        )));
    }
    let expected_domain_tag = crate::zk::stark_open_verify_domain_tag_current(
        backend,
        &envelope.circuit_id,
        envelope.vk_hash,
        &envelope.public_inputs,
        &open.public_inputs,
    );
    if native.params.domain_tag != expected_domain_tag {
        return Err(invalid_parameter(format!(
            "{label} native AIR domain tag mismatch"
        )));
    }
    let air =
        native.proof.air.as_ref().ok_or_else(|| {
            invalid_parameter(format!("{label} proof requires native AIR section"))
        })?;
    let expected_air_circuit_id =
        crate::zk::normalize_stark_fri_circuit_id_for_backend(backend, &envelope.circuit_id)
            .ok_or_else(|| {
                invalid_parameter(format!("{label} native AIR expected circuit id is invalid"))
            })?;
    let actual_air_circuit_id =
        crate::zk::normalize_stark_fri_circuit_id_for_backend(backend, &air.circuit_id)
            .ok_or_else(|| {
                invalid_parameter(format!("{label} native AIR circuit id is invalid"))
            })?;
    if actual_air_circuit_id != expected_air_circuit_id {
        return Err(invalid_parameter(format!(
            "{label} native AIR circuit id mismatch"
        )));
    }
    if air.trace_width != crate::zk_stark::STARK_BINDING_AIR_TRACE_WIDTH_V1 {
        return Err(invalid_parameter(format!(
            "{label} native AIR trace width mismatch"
        )));
    }
    if air.openings.len() != native.proof.queries.len() {
        return Err(invalid_parameter(format!(
            "{label} native AIR opening count mismatch"
        )));
    }
    if native.proof.commits.roots.first().copied() != Some(air.composition_root) {
        return Err(invalid_parameter(format!(
            "{label} native AIR composition root mismatch"
        )));
    }
    let expected_public_digest = crate::zk::stark_open_verify_air_public_digest_current(
        backend,
        &envelope.circuit_id,
        envelope.vk_hash,
        &envelope.public_inputs,
        &open.public_inputs,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "{label} native AIR public digest reconstruction failed: {err}"
        ))
    })?;
    if air.public_digest != expected_public_digest {
        return Err(invalid_parameter(format!(
            "{label} native AIR public digest mismatch"
        )));
    }
    Ok(())
}
#[derive(Clone)]
struct BfvFullBootstrapNativeAirPublicPaddingContext {
    #[cfg(feature = "zk-stark")]
    slot_index: u32,
    #[cfg(feature = "zk-stark")]
    bound_mode: BfvFullBootstrapExecutionProofBoundModeV1,
    #[cfg(feature = "zk-stark")]
    trace_material_digest: Hash,
    #[cfg(feature = "zk-stark")]
    expected_trace_material_digest: Option<Hash>,
    #[cfg(feature = "zk-stark")]
    expected_trace_rows: Option<Vec<Vec<u64>>>,
    #[cfg(feature = "zk-stark")]
    expected_composition_values: Option<Vec<u64>>,
}
#[cfg(all(test, feature = "zk-stark"))]
const FHE_FULL_BOOTSTRAP_NATIVE_AIR_QUERY_NONCE_LIMIT: u32 = 1_024;
#[cfg(all(test, feature = "zk-stark"))]
fn soracloud_fhe_full_bootstrap_native_air_transcript_label_v1(attempt: u32) -> String {
    if attempt == 0 {
        return iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
            .to_owned();
    }
    format!(
        "{}:{attempt}",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
    )
}
#[cfg(feature = "zk-stark")]
fn soracloud_fhe_full_bootstrap_native_air_transcript_label_is_allowed_v1(label: &str) -> bool {
    label == iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
}
#[cfg(all(test, feature = "zk-stark"))]
fn soracloud_fhe_full_bootstrap_native_air_stark_params_v1(
    statement_hash: Hash,
) -> crate::zk_stark::StarkFriParamsV1 {
    crate::zk_stark::StarkFriParamsV1 {
        version: 1,
        n_log2: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_N_LOG2_V1,
        blowup_log2: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_BLOWUP_LOG2_V1,
        fold_arity: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_FOLD_ARITY_V1,
        queries: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1,
        merkle_arity: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_MERKLE_ARITY_V1,
        hash_fn: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_HASH_SHA256_V1,
        domain_tag: iroha_crypto::fhe_bfv::bfv_full_bootstrap_native_stark_air_domain_tag_v1(
            statement_hash,
        ),
    }
}
#[cfg(feature = "zk-stark")]
fn soracloud_fhe_full_bootstrap_native_air_domain_tag_matches_v1(
    statement_hash: Hash,
    domain_tag: &str,
) -> bool {
    domain_tag
        == iroha_crypto::fhe_bfv::bfv_full_bootstrap_native_stark_air_domain_tag_v1(statement_hash)
}
#[cfg(feature = "zk-stark")]
#[cfg(test)]
fn validate_soracloud_fhe_full_bootstrap_bfv_native_air_boundary(
    label: &str,
    statement_hash: Hash,
    native: &crate::zk_stark::StarkVerifyEnvelopeV1,
    public_padding_context: Option<BfvFullBootstrapNativeAirPublicPaddingContext>,
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_bfv_native_air_boundary_with_limits(
        label,
        statement_hash,
        native,
        public_padding_context,
        &crate::zk_stark::StarkVerifierLimits::default(),
    )
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_bfv_native_air_boundary_with_limits(
    label: &str,
    statement_hash: Hash,
    native: &crate::zk_stark::StarkVerifyEnvelopeV1,
    public_padding_context: Option<BfvFullBootstrapNativeAirPublicPaddingContext>,
    limits: &crate::zk_stark::StarkVerifierLimits,
) -> Result<(), InstructionExecutionError> {
    if statement_hash == Hash::prehashed([0_u8; Hash::LENGTH]) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR statement hash must not be zero"
        )));
    }
    if !soracloud_fhe_full_bootstrap_native_air_transcript_label_is_allowed_v1(
        &native.transcript_label,
    ) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR transcript label mismatch"
        )));
    }
    let Some(air) = native.proof.air.as_ref() else {
        return Err(invalid_parameter(format!(
            "{label} proof requires native BFV AIR section"
        )));
    };
    if !soracloud_fhe_full_bootstrap_native_air_domain_tag_matches_v1(
        statement_hash,
        &native.params.domain_tag,
    ) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR domain tag mismatch"
        )));
    }
    if native.params.version != 1
        || native.proof.version != 1
        || native.proof.commits.version != 1
        || air.version != 1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR must use STARK/FRI v1"
        )));
    }
    if native.params.n_log2 != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_N_LOG2_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR n_log2 mismatch"
        )));
    }
    if native.params.blowup_log2
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_BLOWUP_LOG2_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR blowup_log2 mismatch"
        )));
    }
    if native.params.fold_arity
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_FOLD_ARITY_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR fold arity mismatch"
        )));
    }
    if native.params.queries
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR query count mismatch"
        )));
    }
    if native.params.merkle_arity
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_MERKLE_ARITY_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR Merkle arity mismatch"
        )));
    }
    if native.params.hash_fn
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_HASH_SHA256_V1
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR hash function mismatch"
        )));
    }
    if air.circuit_id != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR circuit id mismatch"
        )));
    }
    if air.trace_width != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1 {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR trace width mismatch"
        )));
    }
    let expected_query_count =
        usize::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1);
    if native.proof.queries.len() != expected_query_count
        || air.openings.len() != expected_query_count
    {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR opening count mismatch"
        )));
    }
    if native.proof.commits.roots.is_empty() {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR commitment root count mismatch"
        )));
    }
    if native.proof.commits.roots.iter().any(is_zero_stark_digest) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR commitment roots must not be all-zero"
        )));
    }
    if is_zero_stark_digest(&air.trace_root) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR trace root must not be all-zero"
        )));
    }
    if is_zero_stark_digest(&air.composition_root) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR composition root must not be all-zero"
        )));
    }
    if native.proof.commits.comp_root.is_some() || native.proof.comp_values.is_some() {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR must not carry auxiliary composition value commitments"
        )));
    }
    if native.proof.commits.roots.first().copied() != Some(air.composition_root) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR composition root mismatch"
        )));
    }
    if air.public_digest != <[u8; Hash::LENGTH]>::from(statement_hash) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR public digest mismatch"
        )));
    }
    let expected_row_width =
        usize::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1);
    let expected_merkle_depth =
        usize::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_N_LOG2_V1);
    let domain_size = 1_usize
        .checked_shl(u32::try_from(expected_merkle_depth).unwrap_or(u32::MAX))
        .ok_or_else(|| invalid_parameter(format!("{label} native BFV AIR domain size overflow")))?;
    let goldilocks_modulus =
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_GOLDILOCKS_MODULUS_V1;
    let public_padding_context = public_padding_context.ok_or_else(|| {
        invalid_parameter(format!(
            "{label} native BFV AIR public padding context is required"
        ))
    })?;
    let expected_trace_material_digest = public_padding_context
        .expected_trace_material_digest
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "{label} native BFV AIR governed trace material digest is required"
            ))
        })?;
    if &public_padding_context.trace_material_digest != expected_trace_material_digest {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR trace material digest mismatch"
        )));
    }
    let expected_trace_rows = public_padding_context
        .expected_trace_rows
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "{label} native BFV AIR governed trace rows are required"
            ))
        })?;
    if expected_trace_rows.len() != domain_size {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR governed arithmetic trace row count mismatch"
        )));
    }
    let expected_trace_root =
        crate::zk_stark::stark_air_trace_root_from_rows_v1(&native.params, expected_trace_rows)
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "{label} native BFV AIR governed arithmetic trace root reconstruction failed"
                ))
            })?;
    if air.trace_root != expected_trace_root {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR trace root does not match governed arithmetic trace"
        )));
    }
    let expected_composition_values = public_padding_context
        .expected_composition_values
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "{label} native BFV AIR governed composition values are required"
            ))
        })?;
    if expected_composition_values.len() != domain_size {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR governed AIR composition value count mismatch"
        )));
    }
    let expected_composition_root = crate::zk_stark::stark_merkle_root_from_field_values_v1(
        &native.params,
        expected_composition_values,
    )
    .ok_or_else(|| {
        invalid_parameter(format!(
            "{label} native BFV AIR governed AIR composition root reconstruction failed"
        ))
    })?;
    if air.composition_root != expected_composition_root {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR composition root does not match governed AIR evaluation"
        )));
    }
    let opening_indices = air
        .openings
        .iter()
        .map(|opening| opening.index)
        .collect::<Vec<_>>();
    let opened_rows = air
        .openings
        .iter()
        .map(|opening| opening.row.clone())
        .collect::<Vec<_>>();
    let opened_next_rows = air
        .openings
        .iter()
        .map(|opening| opening.next_row.clone())
        .collect::<Vec<_>>();
    iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_arithmetic_trace_transcript_public_padding_openings_v1(
        &opening_indices,
        &opened_rows,
        &opened_next_rows,
        statement_hash,
        public_padding_context.trace_material_digest,
        public_padding_context.slot_index,
        public_padding_context.bound_mode,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "{label} native BFV AIR transcript public-padding openings failed validation: {err}"
        ))
    })?;
    let expected_query_indices = opening_indices
        .iter()
        .copied()
        .map(|index| {
            usize::try_from(index).map_err(|_| {
                invalid_parameter(format!(
                    "{label} native BFV AIR opening index exceeds platform usize"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sampled_query_indices =
        crate::zk_stark::validate_stark_fri_query_shape_for_base_indices_with_limits_v1(
            &native.params,
            &native.transcript_label,
            &native.proof.commits.roots,
            &native.proof.queries,
            &expected_query_indices,
            limits,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "{label} native BFV AIR FRI query shape failed validation: {err}"
            ))
        })?;
    let sampled_query_indices_u32 = sampled_query_indices
        .iter()
        .copied()
        .map(u32::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            invalid_parameter(format!(
                "{label} native BFV AIR FRI query index exceeds u32"
            ))
        })?;
    if opening_indices != sampled_query_indices_u32 {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR FRI query/opening index mismatch"
        )));
    }
    for (opening_number, opening) in air.openings.iter().enumerate() {
        let opening_label = format!("{label} native BFV AIR opening {opening_number}");
        validate_soracloud_fhe_full_bootstrap_bfv_native_air_row(
            &opening_label,
            "row",
            &opening.row,
            expected_row_width,
            goldilocks_modulus,
        )?;
        validate_soracloud_fhe_full_bootstrap_bfv_native_air_row(
            &opening_label,
            "next row",
            &opening.next_row,
            expected_row_width,
            goldilocks_modulus,
        )?;
        if opening.composition_value >= goldilocks_modulus {
            return Err(invalid_parameter(format!(
                "{opening_label} composition field element is outside Goldilocks modulus"
            )));
        }
        let opening_index = usize::try_from(opening.index).map_err(|_| {
            invalid_parameter(format!("{opening_label} index exceeds platform usize"))
        })?;
        if opening_index >= domain_size {
            return Err(invalid_parameter(format!(
                "{opening_label} index exceeds native BFV AIR domain"
            )));
        }
        let expected_row = expected_trace_rows.get(opening_index).ok_or_else(|| {
            invalid_parameter(format!(
                "{opening_label} governed arithmetic trace row is missing"
            ))
        })?;
        if opening.row != *expected_row {
            return Err(invalid_parameter(format!(
                "{opening_label} row does not match governed arithmetic trace"
            )));
        }
        let next_index = (opening_index + 1) % domain_size;
        let expected_next_row = expected_trace_rows.get(next_index).ok_or_else(|| {
            invalid_parameter(format!(
                "{opening_label} governed arithmetic next row is missing"
            ))
        })?;
        if opening.next_row != *expected_next_row {
            return Err(invalid_parameter(format!(
                "{opening_label} next row does not match governed arithmetic trace"
            )));
        }
        let expected_composition_value = expected_composition_values
            .get(opening_index)
            .copied()
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "{opening_label} governed AIR composition value is missing"
                ))
            })?;
        if opening.composition_value != expected_composition_value {
            return Err(invalid_parameter(format!(
                "{opening_label} composition value does not match governed AIR evaluation"
            )));
        }
        validate_soracloud_fhe_full_bootstrap_bfv_native_air_merkle_path(
            &opening_label,
            "row",
            &opening.row_path,
            expected_merkle_depth,
            opening_index,
        )?;
        validate_soracloud_fhe_full_bootstrap_bfv_native_air_merkle_path(
            &opening_label,
            "next-row",
            &opening.next_row_path,
            expected_merkle_depth,
            (opening_index + 1) % domain_size,
        )?;
        validate_soracloud_fhe_full_bootstrap_bfv_native_air_merkle_path(
            &opening_label,
            "composition",
            &opening.composition_path,
            expected_merkle_depth,
            opening_index,
        )?;
        iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_arithmetic_trace_public_padding_opening_v1(
                opening.index,
                &opening.row,
                &opening.next_row,
                statement_hash,
                public_padding_context.slot_index,
                public_padding_context.bound_mode,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "{opening_label} public padding rows failed validation: {err}"
                ))
            })?;
        if opening.composition_value != 0 {
            return Err(invalid_parameter(format!(
                "{opening_label} public padding composition value must be zero"
            )));
        }
        crate::zk_stark::validate_stark_air_opening_commitment_roots_with_limits_v1(
            &native.params,
            air,
            opening,
            limits,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "{opening_label} Merkle commitment failed validation: {err}"
            ))
        })?;
        let base_index = sampled_query_indices
            .get(opening_number)
            .copied()
            .ok_or_else(|| {
                invalid_parameter(format!("{opening_label} FRI query sample index is missing"))
            })?;
        let first_decommit = native
            .proof
            .queries
            .get(opening_number)
            .and_then(|chain| chain.first())
            .ok_or_else(|| {
                invalid_parameter(format!("{opening_label} FRI query decommitment is missing"))
            })?;
        crate::zk_stark::validate_stark_air_opening_first_fri_value_v1(
            opening,
            base_index,
            first_decommit,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "{opening_label} FRI/AIR value binding failed validation: {err}"
            ))
        })?;
    }
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn is_zero_stark_digest(digest: &[u8; Hash::LENGTH]) -> bool {
    digest.iter().all(|&byte| byte == 0)
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_bfv_native_air_row(
    opening_label: &str,
    row_label: &str,
    row: &[u64],
    expected_row_width: usize,
    goldilocks_modulus: u64,
) -> Result<(), InstructionExecutionError> {
    if row.len() != expected_row_width {
        return Err(invalid_parameter(format!(
            "{opening_label} {row_label} width mismatch"
        )));
    }
    if row.iter().any(|&value| value >= goldilocks_modulus) {
        return Err(invalid_parameter(format!(
            "{opening_label} {row_label} field element is outside Goldilocks modulus"
        )));
    }
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_bfv_native_air_merkle_path(
    opening_label: &str,
    path_label: &str,
    path: &crate::zk_stark::MerklePath,
    expected_depth: usize,
    expected_index: usize,
) -> Result<(), InstructionExecutionError> {
    if path.siblings.len() != expected_depth {
        return Err(invalid_parameter(format!(
            "{opening_label} {path_label} Merkle path depth mismatch"
        )));
    }
    let expected_dir_bytes = (expected_depth + 7) / 8;
    if path.dirs.len() != expected_dir_bytes {
        return Err(invalid_parameter(format!(
            "{opening_label} {path_label} Merkle path direction byte count mismatch"
        )));
    }
    if expected_depth % 8 != 0 {
        let allowed_mask = (1_u8 << (expected_depth % 8)) - 1;
        if path.dirs.last().copied().unwrap_or_default() & !allowed_mask != 0 {
            return Err(invalid_parameter(format!(
                "{opening_label} {path_label} Merkle path direction padding bits must be zero"
            )));
        }
    }
    let mut actual_index = 0_usize;
    for bit_index in 0..expected_depth {
        let byte_index = bit_index / 8;
        let bit = (path.dirs[byte_index] >> (bit_index % 8)) & 1;
        if bit == 1 {
            actual_index |= 1_usize << bit_index;
        }
    }
    if actual_index != expected_index {
        return Err(invalid_parameter(format!(
            "{opening_label} {path_label} Merkle path index mismatch"
        )));
    }
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn verify_soracloud_fhe_full_bootstrap_arithmetic_stark_air(
    label: &str,
    backend: &str,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
    public_padding_context: Option<BfvFullBootstrapNativeAirPublicPaddingContext>,
    guardrails: crate::zk::ZkVerifyGuardrails,
    outer_proof_bytes_len: usize,
) -> Result<bool, InstructionExecutionError> {
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(format!(
            "{label} proof envelope must declare STARK backend"
        )));
    }
    if !guardrails.stark_enabled {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR STARK verification is disabled by node configuration"
        )));
    }
    if outer_proof_bytes_len > guardrails.stark_max_envelope_bytes {
        return Err(invalid_parameter(format!(
            "{label} proof exceeds node-configured STARK envelope byte cap"
        )));
    }
    if envelope.proof_bytes.len() > guardrails.stark_max_envelope_bytes {
        return Err(invalid_parameter(format!(
            "{label} STARK public-input wrapper exceeds node-configured STARK envelope byte cap"
        )));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!("invalid {label} STARK public-input wrapper: {err}"))
        })?;
    if open.envelope_bytes.len() > guardrails.stark_max_proof_bytes {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR envelope exceeds node-configured STARK proof byte cap"
        )));
    }
    let native: crate::zk_stark::StarkVerifyEnvelopeV1 =
        norito::decode_canonical(&open.envelope_bytes).map_err(|err| {
            invalid_parameter(format!(
                "{label} native AIR envelope must decode as STARK/FRI v1: {err}"
            ))
        })?;
    if native.transcript_label == crate::zk::STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1 {
        validate_soracloud_fhe_stark_native_air_binding(label, backend, envelope, statement_hash)?;
        return Err(invalid_parameter(format!(
            "{label} proof requires a dedicated BFV full-bootstrap arithmetic STARK/AIR proof; {FHE_FULL_BOOTSTRAP_GENERIC_BINDING_AIR_REJECTED}"
        )));
    }
    let mut limits = crate::zk_stark::StarkVerifierLimits::default();
    limits.max_envelope_bytes = guardrails.stark_max_proof_bytes;
    validate_soracloud_fhe_full_bootstrap_bfv_native_air_boundary_with_limits(
        label,
        statement_hash,
        &native,
        public_padding_context.clone(),
        &limits,
    )?;
    let public_padding_context = public_padding_context.as_ref().ok_or_else(|| {
        invalid_parameter(format!(
            "{label} native BFV AIR governed trace context is required"
        ))
    })?;
    if !crate::zk_stark::verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
        &open.envelope_bytes,
        &limits,
        statement_hash,
        public_padding_context.trace_material_digest,
        public_padding_context.slot_index,
        public_padding_context.bound_mode,
    ) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR shared public-padding verifier rejected public openings"
        )));
    }
    let expected_trace_rows = public_padding_context
        .expected_trace_rows
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "{label} native BFV AIR governed trace rows are required"
            ))
        })?;
    let expected_composition_values = public_padding_context
        .expected_composition_values
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "{label} native BFV AIR governed composition values are required"
            ))
        })?;
    let expected_public_digest = <[u8; Hash::LENGTH]>::from(statement_hash);
    let expected_base_indices =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_arithmetic_trace_canonical_opening_indices_from_transcript_v1(
            statement_hash,
            public_padding_context.trace_material_digest,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "{label} native BFV AIR opening schedule derivation failed: {err}"
            ))
        })?
        .into_iter()
        .map(|index| {
            usize::try_from(index).map_err(|_| {
                invalid_parameter(format!(
                    "{label} native BFV AIR opening index exceeds platform usize"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !crate::zk_stark::verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
        &open.envelope_bytes,
        &limits,
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
        &expected_public_digest,
        expected_trace_rows,
        expected_composition_values,
        &expected_base_indices,
    ) {
        return Err(invalid_parameter(format!(
            "{label} native BFV AIR explicit STARK verifier rejected governed trace material"
        )));
    }
    Ok(true)
}
fn validate_soracloud_fhe_public_key_proof_envelope(
    attachment: &ProofAttachment,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(
            "FHE public-key proof envelope must declare STARK backend",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(
            "FHE public-key proof envelope aux must be empty",
        ));
    }
    envelope
        .validate_with_bounds(soracloud_fhe_public_key_proof_open_verify_bounds())
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE public-key OpenVerifyEnvelope shape: {err}"
            ))
        })?;
    if envelope.circuit_id != SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE public-key proof circuit id must be canonical v1",
        ));
    }
    if envelope.public_inputs != SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE public-key proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE public-key STARK public-input wrapper: {err}"
            ))
        })?;
    if open.version != 1 {
        return Err(invalid_parameter(
            "FHE public-key STARK public-input wrapper version must be 1",
        ));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(
            "FHE public-key proof public inputs do not match statement hash",
        ));
    }
    validate_soracloud_fhe_public_key_proof_native_envelope_bytes(&open.envelope_bytes)?;
    let vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE public-key proof requires vk_commitment"))?;
    if vk_commitment != envelope.vk_hash {
        return Err(invalid_parameter(
            "FHE public-key proof vk_commitment mismatch",
        ));
    }
    let envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE public-key proof requires envelope_hash"))?;
    let expected = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if envelope_hash != expected {
        return Err(invalid_parameter(
            "FHE public-key proof envelope_hash mismatch",
        ));
    }
    Ok(())
}
fn validate_soracloud_fhe_bootstrap_key_proof_envelope(
    attachment: &ProofAttachment,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope must declare STARK backend",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope aux must be empty",
        ));
    }
    envelope
        .validate_with_bounds(soracloud_fhe_bootstrap_key_proof_open_verify_bounds())
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE bootstrap-key OpenVerifyEnvelope shape: {err}"
            ))
        })?;
    if envelope.circuit_id != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof circuit id must be canonical v1",
        ));
    }
    if envelope.public_inputs != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE bootstrap-key STARK public-input wrapper: {err}"
            ))
        })?;
    if open.version != 1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key STARK public-input wrapper version must be 1",
        ));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof public inputs do not match statement hash",
        ));
    }
    validate_soracloud_fhe_bootstrap_key_proof_native_envelope_bytes(&open.envelope_bytes)?;
    let vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE bootstrap-key proof requires vk_commitment"))?;
    if vk_commitment != envelope.vk_hash {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof vk_commitment mismatch",
        ));
    }
    let envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE bootstrap-key proof requires envelope_hash"))?;
    let expected = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if envelope_hash != expected {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope_hash mismatch",
        ));
    }
    Ok(())
}
fn validate_soracloud_fhe_full_bootstrap_execution_proof_envelope(
    attachment: &ProofAttachment,
    envelope: &OpenVerifyEnvelope,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    validate_soracloud_fhe_statement_open_verify_envelope(
        &FHE_FULL_BOOTSTRAP_EXECUTION_OPEN_VERIFY_CONTRACT,
        envelope,
        statement_hash,
        soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds(),
    )?;
    let vk_commitment = attachment.vk_commitment.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires vk_commitment")
    })?;
    if vk_commitment != envelope.vk_hash {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof vk_commitment mismatch",
        ));
    }
    let envelope_hash = attachment.envelope_hash.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires envelope_hash")
    })?;
    let expected = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if envelope_hash != expected {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof envelope_hash mismatch",
        ));
    }
    Ok(())
}
fn verify_soracloud_fhe_public_key_proof_backend(
    attachment: &ProofAttachment,
    statement_hash: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let attachment_vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE public-key proof requires vk_commitment"))?;
    let attachment_envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE public-key proof requires envelope_hash"))?;
    let expected_envelope_hash = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if attachment_envelope_hash != expected_envelope_hash {
        return Err(invalid_parameter(
            "FHE public-key proof envelope_hash mismatch",
        ));
    }
    if attachment.vk_ref.name != SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE public-key proof vk_ref must use the canonical v1 circuit id",
        ));
    }
    let envelope = public_key_proof_attachment_envelope(attachment)?;
    validate_soracloud_fhe_public_key_proof_envelope(attachment, &envelope, statement_hash)?;
    let record = state_transaction
        .world
        .verifying_keys
        .get(&attachment.vk_ref)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "FHE public-key verifying key not found".into(),
            )
        })?;
    if !record.is_active_at(state_transaction.block_height()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key is not active".into(),
        ));
    }
    if record.namespace != "soracloud" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key must be in the soracloud namespace".into(),
        ));
    }
    if record.backend != BackendTag::Stark {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key must use STARK backend".into(),
        ));
    }
    if record.curve != "goldilocks" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key must use goldilocks STARK field".into(),
        ));
    }
    if record.public_inputs_schema_hash
        != soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key public-input schema mismatch".into(),
        ));
    }
    if record.circuit_id != SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key must use the canonical v1 circuit".into(),
        ));
    }
    if record.version != u32::from(SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key must use the canonical v1 circuit version".into(),
        ));
    }
    if record.gas_schedule_id.as_deref() != Some(SORACLOUD_FHE_PUBLIC_KEY_PROOF_GAS_SCHEDULE_ID_V1)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key gas_schedule_id mismatch".into(),
        ));
    }
    let circuit_key = (record.circuit_id.clone(), record.version);
    match state_transaction
        .world
        .verifying_keys_by_circuit
        .get(&circuit_key)
    {
        Some(active_id) if active_id == &attachment.vk_ref => {}
        _ => {
            return Err(InstructionExecutionError::InvariantViolation(
                "FHE public-key verifying key circuit/version not active".into(),
            ));
        }
    }
    if envelope.circuit_id != record.circuit_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key proof circuit mismatch".into(),
        ));
    }
    if envelope.vk_hash != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key proof verifying-key commitment mismatch".into(),
        ));
    }
    if attachment_vk_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key attachment verifying-key commitment mismatch".into(),
        ));
    }
    if record.max_proof_bytes > 0
        && attachment.proof.bytes.len()
            > usize::try_from(record.max_proof_bytes).unwrap_or(usize::MAX)
    {
        return Err(invalid_parameter(
            "FHE public-key proof exceeds verifying key max_proof_bytes",
        ));
    }
    let vk_box = record.key.clone().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key bytes missing".into(),
        )
    })?;
    if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key vk_len mismatch".into(),
        ));
    }
    let actual_commitment = crate::zk::hash_vk(&vk_box);
    if actual_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key commitment mismatch".into(),
        ));
    }
    if vk_box.backend != attachment.backend {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE public-key verifying key backend mismatch".into(),
        ));
    }
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_verifier_key_payload(
        "FHE public-key",
        &vk_box,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
    )?;
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_native_air_binding(
        "FHE public-key",
        attachment.backend.as_str(),
        &envelope,
        statement_hash,
    )?;
    // TODO: Implement a dedicated BFV key-generation witness AIR that proves the
    // secret-key/public-key equation and the declared exact-residual or bounded-noise
    // relation. Even a cryptographically valid binding AIR would authenticate only public
    // metadata.
    Err(invalid_parameter(
        FHE_PUBLIC_KEY_DEDICATED_WITNESS_AIR_REQUIRED,
    ))
}
fn verify_soracloud_fhe_bootstrap_key_proof_backend(
    attachment: &ProofAttachment,
    statement_hash: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let attachment_vk_commitment = attachment
        .vk_commitment
        .ok_or_else(|| invalid_parameter("FHE bootstrap-key proof requires vk_commitment"))?;
    let attachment_envelope_hash = attachment
        .envelope_hash
        .ok_or_else(|| invalid_parameter("FHE bootstrap-key proof requires envelope_hash"))?;
    let expected_envelope_hash = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if attachment_envelope_hash != expected_envelope_hash {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope_hash mismatch",
        ));
    }
    if attachment.vk_ref.name != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof vk_ref must use the canonical v1 circuit id",
        ));
    }
    let envelope = bootstrap_key_proof_attachment_envelope(attachment)?;
    if envelope.backend != BackendTag::Stark {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope must declare STARK backend",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof envelope aux must be empty",
        ));
    }
    envelope
        .validate_with_bounds(soracloud_fhe_bootstrap_key_proof_open_verify_bounds())
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE bootstrap-key OpenVerifyEnvelope shape: {err}"
            ))
        })?;
    if envelope.circuit_id != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof circuit id must be canonical v1",
        ));
    }
    if envelope.public_inputs != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE bootstrap-key STARK public-input wrapper: {err}"
            ))
        })?;
    if open.version != 1 {
        return Err(invalid_parameter(
            "FHE bootstrap-key STARK public-input wrapper version must be 1",
        ));
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open.public_inputs != expected_public_inputs {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof public inputs do not match statement hash",
        ));
    }
    validate_soracloud_fhe_bootstrap_key_proof_native_envelope_bytes(&open.envelope_bytes)?;
    let record = state_transaction
        .world
        .verifying_keys
        .get(&attachment.vk_ref)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "FHE bootstrap-key verifying key not found".into(),
            )
        })?;
    if !record.is_active_at(state_transaction.block_height()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key is not active".into(),
        ));
    }
    if record.namespace != "soracloud" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key must be in the soracloud namespace".into(),
        ));
    }
    if record.backend != BackendTag::Stark {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key must use STARK backend".into(),
        ));
    }
    if record.curve != "goldilocks" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key must use goldilocks STARK field".into(),
        ));
    }
    if record.public_inputs_schema_hash
        != soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key public-input schema mismatch".into(),
        ));
    }
    if record.circuit_id != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key must use the canonical v1 circuit".into(),
        ));
    }
    if record.version != u32::from(SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key must use the canonical v1 circuit version".into(),
        ));
    }
    if record.gas_schedule_id.as_deref()
        != Some(SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_GAS_SCHEDULE_ID_V1)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key gas_schedule_id mismatch".into(),
        ));
    }
    let circuit_key = (record.circuit_id.clone(), record.version);
    match state_transaction
        .world
        .verifying_keys_by_circuit
        .get(&circuit_key)
    {
        Some(active_id) if active_id == &attachment.vk_ref => {}
        _ => {
            return Err(InstructionExecutionError::InvariantViolation(
                "FHE bootstrap-key verifying key circuit/version not active".into(),
            ));
        }
    }
    if envelope.circuit_id != record.circuit_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key proof circuit mismatch".into(),
        ));
    }
    if envelope.vk_hash != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key proof verifying-key commitment mismatch".into(),
        ));
    }
    if attachment_vk_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key attachment verifying-key commitment mismatch".into(),
        ));
    }
    if record.max_proof_bytes > 0
        && attachment.proof.bytes.len()
            > usize::try_from(record.max_proof_bytes).unwrap_or(usize::MAX)
    {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof exceeds verifying key max_proof_bytes",
        ));
    }
    let vk_box = record.key.clone().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key bytes missing".into(),
        )
    })?;
    if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key vk_len mismatch".into(),
        ));
    }
    let actual_commitment = crate::zk::hash_vk(&vk_box);
    if actual_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key commitment mismatch".into(),
        ));
    }
    if vk_box.backend != attachment.backend {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE bootstrap-key verifying key backend mismatch".into(),
        ));
    }
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_verifier_key_payload(
        "FHE bootstrap-key",
        &vk_box,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
    )?;
    #[cfg(feature = "zk-stark")]
    validate_soracloud_fhe_stark_native_air_binding(
        "FHE bootstrap-key",
        attachment.backend.as_str(),
        &envelope,
        statement_hash,
    )?;
    // TODO: Implement a dedicated BFV zero-refresh witness AIR that proves every
    // governed refresh ciphertext encrypts zero with the transcript-bound randomness
    // and noise for its declared round. Even a cryptographically valid binding AIR would
    // authenticate only public metadata, not that relation.
    Err(invalid_parameter(
        FHE_BOOTSTRAP_KEY_DEDICATED_WITNESS_AIR_REQUIRED,
    ))
}
fn governed_full_bootstrap_execution_verifier_key(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
) -> Result<iroha_data_model::proof::VerifyingKeyBox, InstructionExecutionError> {
    let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires bootstrap key material")
    })?;
    let material = bootstrap_key
        .full_bootstrap_material
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter("FHE full-bootstrap execution proof requires full-bootstrap material")
        })?;
    validate_governed_full_bootstrap_execution_verifier_key_artifact_canonical_layouts(
        &artifacts.verifier_key,
    )?;
    if let Err(err) =
        validate_bfv_full_bootstrap_circuit_artifact_bundle_v1(params, material, artifacts)
    {
        #[cfg(feature = "zk-stark")]
        if let Some(circuit_err) =
            governed_full_bootstrap_execution_verifier_key_circuit_error_from_artifact(artifacts)
        {
            return Err(circuit_err);
        }
        return Err(invalid_parameter(format!(
            "FHE full-bootstrap execution artifact bundle failed validation: {err}"
        )));
    }
    let verifier_key = decode_bfv_full_bootstrap_proof_key_artifact_v1(
        params,
        material,
        BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
        &artifacts.verifier_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution verifier-key artifact failed validation: {err}"
        ))
    })?;
    let verifier_key_material =
        validate_bfv_full_bootstrap_proof_key_material_envelope_bytes_for_key_v1(
            &verifier_key,
            &verifier_key.key_material,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution verifier-key artifact material envelope failed validation: {err}"
            ))
        })?;
    let native_verifier_key_material = decode_bfv_full_bootstrap_native_proof_key_material_v1(
        &verifier_key_material.native_key_material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution verifier-key native material failed validation: {err}"
        ))
    })?;
    let verifier_key_box = iroha_data_model::proof::VerifyingKeyBox::new(
        verifier_key.backend,
        native_verifier_key_material.native_payload,
    );
    #[cfg(feature = "zk-stark")]
    {
        let mut verifier_key_box = verifier_key_box;
        validate_governed_full_bootstrap_execution_stark_verifier_key_payload(
            &mut verifier_key_box,
        )?;
        return Ok(verifier_key_box);
    }
    #[cfg(not(feature = "zk-stark"))]
    Ok(verifier_key_box)
}
fn validate_governed_full_bootstrap_execution_verifier_key_artifact_canonical_layouts(
    artifact_bytes: &[u8],
) -> Result<(), InstructionExecutionError> {
    if artifact_bytes.len()
        > iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_PROFILE_ARTIFACT_MAX_BYTES
    {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution verifier-key artifact exceeds the V1 byte cap",
        ));
    }
    let artifact: iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitArtifactPayloadV1 =
        norito::decode_canonical(artifact_bytes).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution verifier-key artifact must use canonical V1 bytes: {err}"
            ))
        })?;
    let key: iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyV1 = norito::decode_canonical(
        &artifact.payload,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution verifier-key payload must use canonical V1 bytes: {err}"
        ))
    })?;
    let material_envelope: iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyMaterialEnvelopeV1 =
        norito::decode_canonical(&key.key_material).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution verifier-key material envelope must use canonical V1 bytes: {err}"
            ))
        })?;
    let native_material: iroha_crypto::fhe_bfv::BfvFullBootstrapNativeProofKeyMaterialV1 =
        norito::decode_canonical(&material_envelope.native_key_material).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution native verifier-key material must use canonical V1 bytes: {err}"
            ))
        })?;
    // A governed artifact may carry either the normalized Core STARK key or
    // the richer audited BFV-native descriptor. These are distinct typed
    // artifact formats; each must use its one canonical V1 representation.
    #[cfg(feature = "zk-stark")]
    if let Err(core_err) = norito::decode_canonical::<crate::zk_stark::StarkFriVerifyingKeyV1>(
        &native_material.native_payload,
    ) {
        norito::decode_canonical::<
            iroha_crypto::fhe_bfv::BfvFullBootstrapNativeStarkFriVerifyingKeyPayloadV1,
        >(&native_material.native_payload)
        .map_err(|native_err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution native verifier-key payload must use one canonical governed V1 format: core payload decode failed: {core_err}; native payload decode failed: {native_err}"
            ))
        })?;
    }
    #[cfg(not(feature = "zk-stark"))]
    norito::decode_canonical::<
        iroha_crypto::fhe_bfv::BfvFullBootstrapNativeStarkFriVerifyingKeyPayloadV1,
    >(&native_material.native_payload)
    .map_err(|native_err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution native verifier-key payload must use the canonical BFV-native governed V1 format when Core STARK support is disabled: {native_err}"
        ))
    })?;
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn governed_full_bootstrap_execution_verifier_key_circuit_error_from_artifact(
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
) -> Option<InstructionExecutionError> {
    if artifacts.verifier_key.len()
        > iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_PROFILE_ARTIFACT_MAX_BYTES
    {
        return None;
    }
    let artifact: iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitArtifactPayloadV1 =
        norito::decode_canonical(&artifacts.verifier_key).ok()?;
    if artifact.role != BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey {
        return None;
    }
    let key: iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyV1 =
        norito::decode_canonical(&artifact.payload).ok()?;
    if key.key_role != BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey {
        return None;
    }
    if key.key_material.len()
        > iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_PROFILE_ARTIFACT_MAX_BYTES
    {
        return None;
    }
    let material_envelope: iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyMaterialEnvelopeV1 =
        norito::decode_canonical(&key.key_material).ok()?;
    let material_envelope_preflight =
        governed_full_bootstrap_execution_verifier_key_material_envelope_matches_key_for_circuit_fallback(
        &key,
        &material_envelope,
    );
    if let Err(err) = material_envelope_preflight {
        return Some(invalid_parameter(format!(
            "FHE full-bootstrap execution verifier-key artifact material envelope failed validation before circuit diagnostic: {err}"
        )));
    }
    let native_material: iroha_crypto::fhe_bfv::BfvFullBootstrapNativeProofKeyMaterialV1 =
        norito::decode_canonical(&material_envelope.native_key_material).ok()?;
    if let Ok(native_payload) = norito::decode_canonical::<
        iroha_crypto::fhe_bfv::BfvFullBootstrapNativeStarkFriVerifyingKeyPayloadV1,
    >(&native_material.native_payload)
        && native_payload.circuit_id != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1
    {
        return Some(invalid_parameter(
            "FHE full-bootstrap execution verifier-key artifact native circuit id mismatch",
        ));
    }
    let mut verifier_key_box =
        iroha_data_model::proof::VerifyingKeyBox::new(key.backend, native_material.native_payload);
    let err = validate_governed_full_bootstrap_execution_stark_verifier_key_payload(
        &mut verifier_key_box,
    )
    .err()?;
    if matches!(
        &err,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("circuit id mismatch")
    ) {
        Some(err)
    } else {
        None
    }
}
#[cfg(feature = "zk-stark")]
fn governed_full_bootstrap_execution_verifier_key_material_envelope_matches_key_for_circuit_fallback(
    key: &iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyV1,
    envelope: &iroha_crypto::fhe_bfv::BfvFullBootstrapProofKeyMaterialEnvelopeV1,
) -> Result<(), String> {
    if envelope.version
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_ENVELOPE_VERSION_V1
    {
        return Err(format!(
            "version {} does not match canonical version {}",
            envelope.version,
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_ENVELOPE_VERSION_V1
        ));
    }
    if envelope.field_count
        != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_ENVELOPE_FIELD_COUNT_V1
    {
        return Err(format!(
            "field count {} does not match canonical count {}",
            envelope.field_count,
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_ENVELOPE_FIELD_COUNT_V1
        ));
    }
    let canonical_envelope = norito::encode_canonical(envelope)
        .map_err(|err| format!("canonical encoding failed: {err}"))?;
    if canonical_envelope != key.key_material {
        return Err("canonical bytes do not match proof key material".to_owned());
    }
    macro_rules! expect_envelope_field_match {
        ($field:ident, $label:literal) => {
            if envelope.$field != key.$field {
                return Err(format!(
                    "{label} does not match proof key metadata",
                    label = $label
                ));
            }
        };
    }
    expect_envelope_field_match!(key_role, "role");
    expect_envelope_field_match!(backend, "backend");
    expect_envelope_field_match!(key_format, "key format");
    expect_envelope_field_match!(circuit_id, "circuit id");
    expect_envelope_field_match!(parameter_digest, "parameter digest");
    expect_envelope_field_match!(rns_modulus_chain_digest, "RNS modulus-chain digest");
    expect_envelope_field_match!(
        key_switch_decomposition_chain_digest,
        "key-switch decomposition-chain digest"
    );
    expect_envelope_field_match!(
        centered_scale_round_source_chain_digest,
        "centered scale-round source-chain digest"
    );
    expect_envelope_field_match!(max_bootstrap_depth, "max bootstrap depth");
    expect_envelope_field_match!(public_input_schema_digest, "public-input schema digest");
    expect_envelope_field_match!(
        evaluator_artifact_set_digest,
        "evaluator artifact set digest"
    );
    expect_envelope_field_match!(statement_material_version, "statement material version");
    expect_envelope_field_match!(
        statement_material_field_count,
        "statement material field count"
    );
    expect_envelope_field_match!(claim_version, "claim version");
    expect_envelope_field_match!(claim_field_count, "claim field count");
    expect_envelope_field_match!(witness_digest_domain, "witness digest domain");
    expect_envelope_field_match!(witness_digest_material_version, "witness material version");
    expect_envelope_field_match!(
        witness_digest_material_field_count,
        "witness material field count"
    );
    expect_envelope_field_match!(witness_trace_field_count, "witness trace field count");
    expect_envelope_field_match!(
        witness_trace_bounds_field_count,
        "witness trace bounds field count"
    );
    expect_envelope_field_match!(public_input_hash_count, "public-input hash count");
    expect_envelope_field_match!(public_input_hash_bytes, "public-input hash byte length");
    expect_envelope_field_match!(
        supports_exact_residual_multiple,
        "exact residual-multiple support"
    );
    expect_envelope_field_match!(supports_bounded_noise, "bounded-noise support");
    expect_envelope_field_match!(
        derives_opening_schedule_from_statement_hash,
        "statement-hash opening schedule derivation"
    );
    expect_envelope_field_match!(
        derives_opening_schedule_from_trace_material_digest,
        "trace-material opening schedule derivation"
    );
    expect_envelope_field_match!(
        bounds_opening_schedule_rejection_sampling,
        "bounded opening-schedule rejection sampling"
    );
    expect_envelope_field_match!(
        validates_transcript_public_padding_openings,
        "transcript public-padding opening replay"
    );
    expect_envelope_field_match!(
        validates_transcript_public_opening_material,
        "typed transcript public-opening material validation"
    );
    expect_envelope_field_match!(
        requires_verifier_owned_trace_material_digest,
        "verifier-owned trace material digest"
    );
    expect_envelope_field_match!(validates_merkle_path_shape, "Merkle path shape validation");
    expect_envelope_field_match!(validates_merkle_path_roots, "Merkle path root validation");
    expect_envelope_field_match!(validates_fri_query_chain, "FRI query-chain validation");
    expect_envelope_field_match!(
        binds_first_fri_values_to_opened_air_values,
        "first-FRI/opened-AIR value binding"
    );
    expect_envelope_field_match!(
        binds_fri_queries_to_air_commitment_roots,
        "FRI query AIR-root binding"
    );
    expect_envelope_field_match!(
        requires_canonical_base_transcript_label,
        "canonical base transcript-label enforcement"
    );
    expect_envelope_field_match!(
        rejects_suffixed_transcript_label_aliases,
        "suffixed transcript-label alias rejection"
    );
    expect_envelope_field_match!(
        validates_artifact_bound_prover_input,
        "artifact-bound prover input validation"
    );
    expect_envelope_field_match!(
        rejects_stale_galois_key_set_replay,
        "stale Galois-key set replay rejection"
    );
    expect_envelope_field_match!(
        rejects_stale_proof_key_artifacts,
        "stale proof-key artifact replay rejection"
    );
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn validate_governed_full_bootstrap_execution_stark_verifier_key_payload(
    verifier_key: &mut iroha_data_model::proof::VerifyingKeyBox,
) -> Result<(), InstructionExecutionError> {
    if verifier_key.backend != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution verifier-key artifact backend mismatch",
        ));
    }
    let payload: crate::zk_stark::StarkFriVerifyingKeyV1 = match norito::decode_canonical(
        &verifier_key.bytes,
    ) {
        Ok(payload) => payload,
        Err(core_err) => {
            let native_payload: iroha_crypto::fhe_bfv::BfvFullBootstrapNativeStarkFriVerifyingKeyPayloadV1 =
                    norito::decode_canonical(&verifier_key.bytes).map_err(|native_err| {
                        invalid_parameter(format!(
                            "FHE full-bootstrap execution verifier-key artifact has invalid STARK payload: {core_err}; native payload decode failed: {native_err}"
                        ))
                    })?;
            if native_payload.field_count
                != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_VERIFIER_PAYLOAD_FIELD_COUNT_V1
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native field count mismatch",
                ));
            }
            if native_payload.backend != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native backend mismatch",
                ));
            }
            if native_payload.key_format
                != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_FORMAT_V1
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native key format mismatch",
                ));
            }
            if native_payload.proof_system
                != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_PROOF_SYSTEM_V1
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native proof system mismatch",
                ));
            }
            if native_payload.field
                != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FIELD_V1
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native field mismatch",
                ));
            }
            let expected_trace_profile_digest =
                iroha_crypto::fhe_bfv::bfv_full_bootstrap_arithmetic_trace_profile_digest_v1()
                    .map_err(|err| {
                        invalid_parameter(format!(
                            "FHE full-bootstrap execution verifier-key artifact arithmetic trace profile digest could not be derived: {err}"
                        ))
                    })?;
            if native_payload.arithmetic_trace_profile_digest != expected_trace_profile_digest {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native arithmetic trace profile digest mismatch",
                ));
            }
            let expected_air_constraint_system_digest =
                iroha_crypto::fhe_bfv::bfv_full_bootstrap_arithmetic_air_constraint_system_digest_v1()
                    .map_err(|err| {
                        invalid_parameter(format!(
                            "FHE full-bootstrap execution verifier-key artifact arithmetic AIR constraint-system digest could not be derived: {err}"
                        ))
                    })?;
            if native_payload.arithmetic_air_constraint_system_digest
                != expected_air_constraint_system_digest
            {
                return Err(invalid_parameter(
                    "FHE full-bootstrap execution verifier-key artifact native arithmetic AIR constraint-system digest mismatch",
                ));
            }
            iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_native_stark_fri_verifier_payload_v1(
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
                &verifier_key.bytes,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE full-bootstrap execution verifier-key artifact native generated circuit body validation failed: {err}"
                ))
            })?;
            let payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
                version: native_payload.version,
                circuit_id: native_payload.circuit_id,
                n_log2: native_payload.n_log2,
                blowup_log2: native_payload.blowup_log2,
                fold_arity: native_payload.fold_arity,
                queries: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
                merkle_arity: native_payload.merkle_arity,
                hash_fn: native_payload.hash_fn,
            };
            verifier_key.bytes = norito::encode_canonical(&payload).map_err(|err| {
                    invalid_parameter(format!(
                        "FHE full-bootstrap execution verifier-key artifact canonical STARK payload encoding failed: {err}"
                    ))
                })?;
            payload
        }
    };
    crate::zk_stark::validate_stark_fri_canonical_verifying_key_payload(
        &payload,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        "FHE full-bootstrap execution verifier-key artifact",
    )
    .map_err(invalid_parameter)?;
    if payload.hash_fn != crate::zk_stark::STARK_HASH_SHA256_V1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution verifier-key artifact must use SHA-256 STARK/FRI",
        ));
    }
    Ok(())
}
#[cfg(all(test, feature = "zk-stark"))]
fn validate_soracloud_fhe_full_bootstrap_prover_verifier_key(
    label: &str,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    expected_circuit_id: &str,
) -> Result<(), InstructionExecutionError> {
    canonical_soracloud_fhe_full_bootstrap_prover_verifier_key(
        label,
        verifier_key,
        expected_circuit_id,
    )
    .map(|_| ())
}
#[cfg(feature = "zk-stark")]
fn canonical_soracloud_fhe_full_bootstrap_prover_verifier_key(
    label: &str,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    expected_circuit_id: &str,
) -> Result<iroha_data_model::proof::VerifyingKeyBox, InstructionExecutionError> {
    if verifier_key.backend != iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Err(invalid_parameter(format!(
            "{label} verifier-key backend mismatch"
        )));
    }
    if expected_circuit_id == SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1 {
        let mut verifier_key = verifier_key.clone();
        validate_governed_full_bootstrap_execution_stark_verifier_key_payload(&mut verifier_key)?;
        return Ok(verifier_key);
    }
    let payload: crate::zk_stark::StarkFriVerifyingKeyV1 =
        norito::decode_canonical(&verifier_key.bytes).map_err(|err| {
            invalid_parameter(format!(
                "{label} verifier-key has invalid STARK payload: {err}"
            ))
        })?;
    crate::zk_stark::validate_stark_fri_canonical_verifying_key_payload(
        &payload,
        expected_circuit_id,
        label,
    )
    .map_err(invalid_parameter)?;
    if payload.hash_fn != crate::zk_stark::STARK_HASH_SHA256_V1 {
        return Err(invalid_parameter(format!(
            "{label} verifier-key must use SHA-256 STARK/FRI"
        )));
    }
    Ok(verifier_key.clone())
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_prover_statement_hash(
    label: &str,
    statement_hash: Hash,
) -> Result<(), InstructionExecutionError> {
    if statement_hash == Hash::prehashed([0_u8; Hash::LENGTH]) {
        return Err(invalid_parameter(format!(
            "{label} statement hash must not be zero"
        )));
    }
    Ok(())
}
fn verify_soracloud_fhe_full_bootstrap_execution_proof_backend(
    attachment: &ProofAttachment,
    statement_hash: Hash,
    public_padding_context: BfvFullBootstrapNativeAirPublicPaddingContext,
    governed_verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    #[cfg(not(feature = "zk-stark"))]
    let _ = public_padding_context;
    let attachment_vk_commitment = attachment.vk_commitment.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires vk_commitment")
    })?;
    let attachment_envelope_hash = attachment.envelope_hash.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires envelope_hash")
    })?;
    let expected_envelope_hash = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
    if attachment_envelope_hash != expected_envelope_hash {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof envelope_hash mismatch",
        ));
    }
    if attachment.vk_ref.name != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof vk_ref must use the canonical v1 circuit id",
        ));
    }
    let envelope = full_bootstrap_execution_proof_attachment_envelope(attachment)?;
    validate_soracloud_fhe_full_bootstrap_execution_proof_envelope(
        attachment,
        &envelope,
        statement_hash,
    )?;
    let record = state_transaction
        .world
        .verifying_keys
        .get(&attachment.vk_ref)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "FHE full-bootstrap execution verifying key not found".into(),
            )
        })?;
    if !record.is_active_at(state_transaction.block_height()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key is not active".into(),
        ));
    }
    if record.namespace != "soracloud" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key must be in the soracloud namespace".into(),
        ));
    }
    if record.backend != BackendTag::Stark {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key must use STARK backend".into(),
        ));
    }
    if record.curve != "goldilocks" {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key must use goldilocks STARK field".into(),
        ));
    }
    if record.public_inputs_schema_hash
        != soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key public-input schema mismatch".into(),
        ));
    }
    if record.circuit_id != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1 {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key must use the canonical v1 circuit".into(),
        ));
    }
    if record.version != u32::from(SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key must use the canonical v1 circuit version"
                .into(),
        ));
    }
    if record.gas_schedule_id.as_deref()
        != Some(SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_GAS_SCHEDULE_ID_V1)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key gas_schedule_id mismatch".into(),
        ));
    }
    let circuit_key = (record.circuit_id.clone(), record.version);
    match state_transaction
        .world
        .verifying_keys_by_circuit
        .get(&circuit_key)
    {
        Some(active_id) if active_id == &attachment.vk_ref => {}
        _ => {
            return Err(InstructionExecutionError::InvariantViolation(
                "FHE full-bootstrap execution verifying key circuit/version not active".into(),
            ));
        }
    }
    if envelope.circuit_id != record.circuit_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution proof circuit mismatch".into(),
        ));
    }
    let governed_commitment = crate::zk::hash_vk(governed_verifier_key);
    if record.commitment != governed_commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifier-key commitment must match governed artifact"
                .into(),
        ));
    }
    if envelope.vk_hash != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution proof verifying-key commitment mismatch".into(),
        ));
    }
    if attachment_vk_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution attachment verifying-key commitment mismatch".into(),
        ));
    }
    if record.max_proof_bytes > 0
        && attachment.proof.bytes.len()
            > usize::try_from(record.max_proof_bytes).unwrap_or(usize::MAX)
    {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof exceeds verifying key max_proof_bytes",
        ));
    }
    let vk_box = record.key.clone().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key bytes missing".into(),
        )
    })?;
    if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key vk_len mismatch".into(),
        ));
    }
    if vk_box != *governed_verifier_key {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key bytes must match governed artifact".into(),
        ));
    }
    let actual_commitment = crate::zk::hash_vk(&vk_box);
    if actual_commitment != record.commitment {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key commitment mismatch".into(),
        ));
    }
    if vk_box.backend != attachment.backend {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE full-bootstrap execution verifying key backend mismatch".into(),
        ));
    }
    #[cfg(not(feature = "zk-stark"))]
    {
        let _ = public_padding_context;
        Err(invalid_parameter(
            "FHE full-bootstrap execution proof requires the zk-stark feature for dedicated native AIR verification",
        ))
    }
    #[cfg(feature = "zk-stark")]
    {
        verify_soracloud_fhe_full_bootstrap_arithmetic_stark_air(
            "FHE full-bootstrap execution",
            attachment.backend.as_str(),
            &envelope,
            statement_hash,
            Some(public_padding_context),
            crate::zk::ZkVerifyGuardrails::from_cfg(&state_transaction.zk),
            attachment.proof.bytes.len(),
        )?;
        state_transaction
            .register_confidential_proof(attachment.proof.bytes.len())
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE full-bootstrap execution proof quota accounting failed: {err}"
                ))
            })?;
        Ok(())
    }
}
fn verify_soracloud_fhe_public_key_proof(
    state_transaction: &mut StateTransaction<'_, '_>,
    policy: &FheExecutionPolicyV1,
    expected_statement_hash: Option<Hash>,
    proof: Option<&SoracloudFhePublicKeyProofV1>,
) -> Result<(), InstructionExecutionError> {
    let Some(expected_statement_hash) = expected_statement_hash else {
        if proof.is_some() {
            return Err(invalid_parameter(
                "FHE public-key proof requires public-key proof statement digest",
            ));
        }
        return Ok(());
    };
    let Some(proof) = proof else {
        return Err(invalid_parameter(
            "FHE policy-bound public-key material requires public-key proof",
        ));
    };
    proof
        .validate()
        .map_err(|err| invalid_parameter(format!("invalid FHE public-key proof: {err}")))?;
    if proof.statement_hash != expected_statement_hash {
        return Err(invalid_parameter(
            "FHE public-key proof statement hash mismatch",
        ));
    }
    if policy.public_key_proof_statement_digest != Some(expected_statement_hash) {
        return Err(invalid_parameter(
            "FHE public-key proof statement digest does not match the execution policy",
        ));
    }
    let envelope = public_key_proof_attachment_envelope(&proof.proof)?;
    validate_soracloud_fhe_public_key_proof_envelope(
        &proof.proof,
        &envelope,
        expected_statement_hash,
    )?;
    verify_soracloud_fhe_public_key_proof_backend(
        &proof.proof,
        expected_statement_hash,
        state_transaction,
    )
}
fn verify_soracloud_fhe_bootstrap_key_proof(
    state_transaction: &mut StateTransaction<'_, '_>,
    policy: &FheExecutionPolicyV1,
    expected_statement_hash: Option<Hash>,
    proof: Option<&SoracloudFheBootstrapKeyProofV1>,
    require_proof: bool,
) -> Result<(), InstructionExecutionError> {
    if !require_proof && proof.is_some() {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof is only accepted for bootstrap operations",
        ));
    }
    let Some(expected_statement_hash) = expected_statement_hash else {
        if require_proof {
            return Err(invalid_parameter(
                "FHE bootstrap operation requires bootstrap-key proof statement digest",
            ));
        }
        if proof.is_some() {
            return Err(invalid_parameter(
                "FHE bootstrap-key proof requires bootstrap-capable policy",
            ));
        }
        return Ok(());
    };
    if policy.max_bootstrap_count == 0 {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof digest requires bootstrap-capable policy",
        ));
    }
    let Some(proof) = proof else {
        if require_proof {
            return Err(invalid_parameter(
                "FHE bootstrap operation requires bootstrap-key proof",
            ));
        }
        return Ok(());
    };
    proof
        .validate()
        .map_err(|err| invalid_parameter(format!("invalid FHE bootstrap-key proof: {err}")))?;
    if proof.statement_hash != expected_statement_hash {
        return Err(invalid_parameter(
            "FHE bootstrap-key proof statement hash mismatch",
        ));
    }
    let envelope = bootstrap_key_proof_attachment_envelope(&proof.proof)?;
    validate_soracloud_fhe_bootstrap_key_proof_envelope(
        &proof.proof,
        &envelope,
        expected_statement_hash,
    )?;
    verify_soracloud_fhe_bootstrap_key_proof_backend(
        &proof.proof,
        expected_statement_hash,
        state_transaction,
    )
}
fn full_bootstrap_execution_proof_bound_mode(
    bound_mode: BfvCiphertextBoundModeV1,
) -> BfvFullBootstrapExecutionProofBoundModeV1 {
    match bound_mode {
        BfvCiphertextBoundModeV1::ExactResidualMultiple => {
            BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple
        }
        BfvCiphertextBoundModeV1::BoundedNoise => {
            BfvFullBootstrapExecutionProofBoundModeV1::BoundedNoise
        }
    }
}
#[cfg(feature = "zk-stark")]
fn refresh_transcript_mode_for_ciphertext_bound_mode(
    bound_mode: BfvCiphertextBoundModeV1,
) -> BfvRefreshTranscriptModeV1 {
    match bound_mode {
        BfvCiphertextBoundModeV1::ExactResidualMultiple => BfvRefreshTranscriptModeV1::ExactLift,
        BfvCiphertextBoundModeV1::BoundedNoise => BfvRefreshTranscriptModeV1::BoundedNoise,
    }
}
#[allow(clippy::too_many_arguments)]
fn verify_soracloud_fhe_full_bootstrap_execution_proofs(
    state_transaction: &mut StateTransaction<'_, '_>,
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    evaluation_key_refresh_transcript: &BfvEvaluationKeyRefreshTranscriptV1,
    job: &FheJobSpecV1,
    input_envelopes: &[BfvIdentifierCiphertext],
    input_bounds: &[u128],
    output_envelope: &BfvIdentifierCiphertext,
    output_bound: Option<u128>,
    bound_mode: BfvCiphertextBoundModeV1,
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    proofs: &[SoracloudFheFullBootstrapExecutionProofV1],
) -> Result<(), InstructionExecutionError> {
    let is_bootstrap_job = job.operation == FheJobOperationV1::Bootstrap && job.bootstrap_count > 0;
    let Some(bootstrap_key) = evaluation_keys.bootstrap_key.as_ref() else {
        if full_bootstrap_circuit_artifacts.is_some() || !proofs.is_empty() {
            return Err(invalid_parameter(
                "FHE full-bootstrap execution proof material is only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    };
    if !is_bootstrap_job || bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        if full_bootstrap_circuit_artifacts.is_some() || !proofs.is_empty() {
            return Err(invalid_parameter(
                "FHE full-bootstrap execution proof material is only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    }
    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
    let artifacts = full_bootstrap_circuit_artifacts.ok_or_else(|| {
        invalid_parameter(
            "FHE full-bootstrap execution proof requires full-bootstrap circuit artifacts",
        )
    })?;
    validate_soracloud_fhe_evaluation_budget(job, input_envelopes)?;
    if input_bounds.len() != input_envelopes.len() {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof input bound metadata must match input envelope count",
        ));
    }
    let input_bound = *input_bounds.first().ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires input bound metadata")
    })?;
    let output_bound = output_bound.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires output bound metadata")
    })?;
    let input_slots = first_matching_fhe_slots(input_envelopes)?;
    if output_envelope.slots.len() != input_slots.len() {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof output slot count does not match input slot count",
        ));
    }
    if proofs.len() != output_envelope.slots.len() {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof count must match output slot count",
        ));
    }
    let governed_verifier_key =
        governed_full_bootstrap_execution_verifier_key(params, evaluation_keys, artifacts)?;
    let proof_bound_mode = full_bootstrap_execution_proof_bound_mode(bound_mode);
    for (slot_index, ((input_ciphertext, output_ciphertext), proof)) in input_slots
        .iter()
        .zip(output_envelope.slots.iter())
        .zip(proofs.iter())
        .enumerate()
    {
        let slot_index_u32 = u32::try_from(slot_index).map_err(|_| {
            invalid_parameter("FHE full-bootstrap execution proof slot index overflow")
        })?;
        let claim = bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1(
            params,
            bootstrap_key,
            artifacts,
            &evaluation_keys.galois_keys,
            slot_index_u32,
            input_ciphertext.clone(),
            output_ciphertext.clone(),
            proof_bound_mode,
            input_bound,
            output_bound,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to derive FHE full-bootstrap execution witness for slot {slot_index}: {err}"
            ))
        })?;
        let expected_statement_hash =
            bfv_full_bootstrap_execution_proof_statement_digest_with_witness_v1(
                params,
                &evaluation_key_refresh_transcript.public_key,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                &claim,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution proof statement for slot {slot_index}: {err}"
                ))
            })?;
        #[cfg(feature = "zk-stark")]
        let (trace_material_digest, expected_trace_rows, expected_composition_values) = {
            let witness_material = bfv_full_bootstrap_execution_witness_digest_material_v1(
                params,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                &claim,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution verifier witness material for slot {slot_index}: {err}"
                ))
            })?;
            let proof_input_material = bfv_full_bootstrap_execution_proof_input_material_v1(
                &evaluation_key_refresh_transcript.public_key,
                &witness_material,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution verifier proof input material for slot {slot_index}: {err}"
                ))
            })?;
            let arithmetic_trace_material =
                bfv_full_bootstrap_arithmetic_trace_material_v1(&proof_input_material).map_err(
                    |err| {
                        invalid_parameter(format!(
                            "failed to derive FHE full-bootstrap execution verifier arithmetic trace for slot {slot_index}: {err}"
                        ))
                    },
                )?;
            let arithmetic_air_evaluation_material =
                bfv_full_bootstrap_arithmetic_air_evaluation_material_v1(
                    &arithmetic_trace_material,
                )
                .map_err(|err| {
                    invalid_parameter(format!(
                        "failed to derive FHE full-bootstrap execution verifier AIR evaluation for slot {slot_index}: {err}"
                    ))
                })?;
            let trace_material_digest =
                bfv_full_bootstrap_arithmetic_trace_material_digest_v1(&arithmetic_trace_material)
                    .map_err(|err| {
                        invalid_parameter(format!(
                            "failed to derive FHE full-bootstrap execution verifier arithmetic trace digest for slot {slot_index}: {err}"
                        ))
                    })?;
            (
                trace_material_digest,
                Some(arithmetic_trace_material.rows),
                Some(arithmetic_air_evaluation_material.composition_values),
            )
        };
        proof.validate().map_err(|err| {
            invalid_parameter(format!(
                "invalid FHE full-bootstrap execution proof for slot {slot_index}: {err}"
            ))
        })?;
        if proof.statement_hash != expected_statement_hash {
            return Err(invalid_parameter(format!(
                "FHE full-bootstrap execution proof statement hash mismatch for slot {slot_index}"
            )));
        }
        let envelope = full_bootstrap_execution_proof_attachment_envelope(&proof.proof)?;
        validate_soracloud_fhe_full_bootstrap_execution_proof_envelope(
            &proof.proof,
            &envelope,
            expected_statement_hash,
        )?;
        verify_soracloud_fhe_full_bootstrap_execution_proof_backend(
            &proof.proof,
            expected_statement_hash,
            BfvFullBootstrapNativeAirPublicPaddingContext {
                #[cfg(feature = "zk-stark")]
                slot_index: slot_index_u32,
                #[cfg(feature = "zk-stark")]
                bound_mode: proof_bound_mode,
                #[cfg(feature = "zk-stark")]
                trace_material_digest,
                #[cfg(feature = "zk-stark")]
                expected_trace_material_digest: Some(trace_material_digest),
                #[cfg(feature = "zk-stark")]
                expected_trace_rows,
                #[cfg(feature = "zk-stark")]
                expected_composition_values,
            },
            &governed_verifier_key,
            state_transaction,
        )?;
    }
    Ok(())
}
fn validate_soracloud_fhe_state_payload(
    value_size_bytes: Option<u64>,
    value_payload: Option<&[u8]>,
    payload_commitment: Option<Hash>,
) -> Result<(u64, Hash, BfvIdentifierCiphertext), InstructionExecutionError> {
    let value_size_bytes = value_size_bytes
        .ok_or_else(|| invalid_parameter("FHE upsert requires value_size_bytes"))?;
    let value_payload =
        value_payload.ok_or_else(|| invalid_parameter("FHE upsert requires value_payload"))?;
    let payload_commitment = payload_commitment
        .ok_or_else(|| invalid_parameter("FHE upsert requires payload commitment"))?;
    let actual_value_size = u64::try_from(value_payload.len())
        .map_err(|_| invalid_parameter("FHE value_payload length exceeds u64 range"))?;
    if value_size_bytes != actual_value_size {
        return Err(invalid_parameter(format!(
            "FHE value_size_bytes {value_size_bytes} does not match value_payload length {actual_value_size}"
        )));
    }
    if payload_commitment != Hash::new(value_payload) {
        return Err(invalid_parameter("FHE payload commitment mismatch"));
    }
    let params = ram_lfe_bfv_parameters_v1();
    let envelope = decode_soracloud_fhe_envelope(value_payload)?;
    validate_soracloud_fhe_envelope_shape(&params, &envelope, "fhe state upsert")?;
    Ok((value_size_bytes, payload_commitment, envelope))
}
#[allow(clippy::too_many_arguments)]
fn verify_soracloud_fhe_input_admission_proof(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &Name,
    binding_name: &Name,
    state_key: &str,
    operation: SoraStateMutationOperationV1,
    value_size_bytes: Option<u64>,
    value_payload: Option<&[u8]>,
    payload_commitment: Option<Hash>,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    proof: Option<&SoracloudFheInputAdmissionProofV1>,
) -> Result<Option<(u128, BfvCiphertextBoundModeV1, Hash)>, InstructionExecutionError> {
    let validated_input_payload = if operation == SoraStateMutationOperationV1::Upsert
        && encryption == SoraStateEncryptionV1::FheCiphertext
    {
        Some(validate_soracloud_fhe_state_payload(
            value_size_bytes,
            value_payload,
            payload_commitment,
        )?)
    } else {
        None
    };
    let Some(proof) = proof else {
        return Ok(None);
    };
    if operation != SoraStateMutationOperationV1::Upsert {
        return Err(invalid_parameter(
            "FHE input admission proofs are only valid for upsert mutations",
        ));
    }
    if encryption != SoraStateEncryptionV1::FheCiphertext {
        return Err(invalid_parameter(
            "FHE input admission proofs require FheCiphertext encryption",
        ));
    }
    let (value_size_bytes, payload_commitment, input_envelope) = validated_input_payload
        .ok_or_else(|| {
            invalid_parameter("FHE input admission proof requires an FHE upsert payload")
        })?;
    let params = ram_lfe_bfv_parameters_v1();
    proof
        .validate()
        .map_err(|err| invalid_parameter(format!("invalid FHE input admission proof: {err}")))?;
    let public_key = proof.public_key.as_ref().ok_or_else(|| {
        invalid_parameter("invalid FHE input admission proof: public_key must be present")
    })?;
    let public_key_digest = bfv_public_key_digest(&params, public_key).map_err(|err| {
        invalid_parameter(format!(
            "failed to digest FHE input admission public key: {err}"
        ))
    })?;
    match proof.bound_mode {
        BfvCiphertextBoundModeV1::ExactResidualMultiple => {
            validate_bfv_exact_residual_multiple_capacity(
                &params,
                proof.residual_multiple_bound,
                "Soracloud FHE input admission residual bound",
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE input admission residual metadata exceeds BFV capacity: {err}"
                ))
            })?;
        }
        BfvCiphertextBoundModeV1::BoundedNoise => {
            validate_bfv_bounded_noise_bound(
                &params,
                proof.residual_multiple_bound,
                "Soracloud FHE input admission bounded-noise bound",
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE input admission bounded-noise metadata exceeds BFV capacity: {err}"
                ))
            })?;
        }
    }
    let ciphertext_proof_statement_digests =
        derive_soracloud_fhe_input_ciphertext_statement_digests(
            &params,
            public_key,
            &input_envelope,
            proof.residual_multiple_bound,
            proof.bound_mode,
            "FHE input admission",
        )?;
    if proof.ciphertext_proof_statement_digests != ciphertext_proof_statement_digests {
        return Err(invalid_parameter(
            "FHE input admission ciphertext proof statement digest mismatch",
        ));
    }
    let expected_statement_hash = expected_fhe_input_admission_statement_hash(
        service_name,
        binding_name,
        state_key,
        operation,
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
        &ciphertext_proof_statement_digests,
        proof.residual_multiple_bound,
        proof.bound_mode,
    )?;
    if proof.statement_hash != expected_statement_hash {
        return Err(invalid_parameter(
            "FHE input admission statement hash mismatch",
        ));
    }
    let envelope = proof_attachment_envelope(&proof.proof)?;
    validate_soracloud_fhe_input_admission_envelope(
        &proof.proof,
        &envelope,
        expected_statement_hash,
    )?;
    verify_soracloud_fhe_input_admission_backend(
        &proof.proof,
        expected_statement_hash,
        state_transaction,
    )?;
    Ok(Some((
        proof.residual_multiple_bound,
        proof.bound_mode,
        public_key_digest,
    )))
}
fn verify_fhe_job_run_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    job: FheJobSpecV1,
    policy_reference: SoracloudFhePolicyReferenceV1,
    public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "fhe job provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_fhe_job_run_provenance_payload(
        service_name.as_ref(),
        binding_name.as_ref(),
        job,
        policy_reference,
        public_key_proof,
        bootstrap_key_zero_refresh_proof,
        full_bootstrap_execution_proofs,
    )
    .map_err(|err| invalid_parameter(format!("failed to encode fhe job provenance: {err}")))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter("fhe job provenance signature verification failed"))?;
    Ok(())
}
fn verify_fhe_policy_provenance(
    authority: &AccountId,
    payload: &[u8],
    action: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(format!(
            "fhe policy {action} provenance signer must match the transaction authority"
        )));
    }
    verify_signature_for_signer(&provenance.signature, &provenance.signer, payload).map_err(|_| {
        invalid_parameter(format!(
            "fhe policy {action} provenance signature verification failed"
        ))
    })
}
fn verify_fhe_policy_register_provenance(
    authority: &AccountId,
    service_name: &Name,
    material: &SoracloudFheGovernedMaterialV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_fhe_policy_register_provenance_payload(service_name.as_ref(), material)
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode fhe policy registration provenance: {err}"
            ))
        })?;
    verify_fhe_policy_provenance(authority, &payload, "registration", provenance)
}
fn verify_fhe_policy_rotate_provenance(
    authority: &AccountId,
    service_name: &Name,
    expected_active: &SoracloudFhePolicyReferenceV1,
    material: &SoracloudFheGovernedMaterialV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_fhe_policy_rotate_provenance_payload(
        service_name.as_ref(),
        expected_active,
        material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode fhe policy rotation provenance: {err}"
        ))
    })?;
    verify_fhe_policy_provenance(authority, &payload, "rotation", provenance)
}
fn verify_fhe_policy_revoke_provenance(
    authority: &AccountId,
    service_name: &Name,
    expected_active: &SoracloudFhePolicyReferenceV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload =
        encode_fhe_policy_revoke_provenance_payload(service_name.as_ref(), expected_active)
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to encode fhe policy revocation provenance: {err}"
                ))
            })?;
    verify_fhe_policy_provenance(authority, &payload, "revocation", provenance)
}
fn verify_decryption_request_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    policy: DecryptionAuthorityPolicyV1,
    request: DecryptionRequestV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "decryption request provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_decryption_request_provenance_payload(service_name.as_ref(), policy, request)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode decryption provenance: {err}"))
            })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("decryption request provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_training_job_start_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    model_name: &str,
    job_id: &str,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "training job start provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_training_job_start_provenance_payload(
        service_name.as_ref(),
        model_name,
        job_id,
        worker_group_size,
        target_steps,
        checkpoint_interval_steps,
        max_retries,
        step_compute_units,
        compute_budget_units,
        storage_budget_bytes,
    )
    .map_err(|err| {
        invalid_parameter(format!("failed to encode training start provenance: {err}"))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("training job start provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_training_job_checkpoint_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    job_id: &str,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "training checkpoint provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_training_job_checkpoint_provenance_payload(
        service_name.as_ref(),
        job_id,
        completed_step,
        checkpoint_size_bytes,
        metrics_hash,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode training checkpoint provenance: {err}"
        ))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("training checkpoint provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_training_job_retry_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    job_id: &str,
    reason: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "training retry provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_training_job_retry_provenance_payload(service_name.as_ref(), job_id, reason)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode training retry provenance: {err}"))
            })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("training retry provenance signature verification failed"),
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn verify_model_artifact_register_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    model_name: &str,
    training_job_id: &str,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "model artifact provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_model_artifact_register_provenance_payload(
        service_name.as_ref(),
        model_name,
        training_job_id,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )
    .map_err(|err| {
        invalid_parameter(format!("failed to encode model artifact provenance: {err}"))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("model artifact register provenance signature verification failed"),
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn verify_model_weight_register_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    model_name: &str,
    weight_version: &str,
    training_job_id: &str,
    parent_version: Option<&str>,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "model weight provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_model_weight_register_provenance_payload(
        service_name.as_ref(),
        model_name,
        weight_version,
        training_job_id,
        parent_version,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )
    .map_err(|err| invalid_parameter(format!("failed to encode model weight provenance: {err}")))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("model weight register provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_model_weight_promote_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    model_name: &str,
    weight_version: &str,
    gate_approved: bool,
    gate_report_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "model weight promotion provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_model_weight_promote_provenance_payload(
        service_name.as_ref(),
        model_name,
        weight_version,
        gate_approved,
        gate_report_hash,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode model weight promotion provenance: {err}"
        ))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("model weight promote provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_model_weight_rollback_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    model_name: &str,
    target_version: &str,
    reason: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "model weight rollback provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_model_weight_rollback_provenance_payload(
        service_name.as_ref(),
        model_name,
        target_version,
        reason,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode model weight rollback provenance: {err}"
        ))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("model weight rollback provenance signature verification failed"),
    )?;
    Ok(())
}
fn verify_uploaded_model_bundle_register_provenance(
    authority: &AccountId,
    bundle: &SoraUploadedModelBundleV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "uploaded model bundle provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_uploaded_model_bundle_register_provenance_payload(bundle.clone())
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode uploaded model bundle provenance: {err}"
            ))
        })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("uploaded model bundle provenance signature verification failed"),
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn verify_uploaded_model_finalize_provenance(
    authority: &AccountId,
    service_name: &Name,
    model_name: &str,
    model_id: &str,
    artifact_id: &str,
    weight_version: &str,
    bundle_root: Hash,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(
            "uploaded model finalize provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_uploaded_model_finalize_provenance_payload(
        service_name.as_ref(),
        model_name,
        model_id,
        artifact_id,
        weight_version,
        bundle_root,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode uploaded model finalize provenance: {err}"
        ))
    })?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| invalid_parameter("uploaded model finalize provenance signature verification failed"),
    )?;
    Ok(())
}
pub(crate) fn next_soracloud_audit_sequence(state_transaction: &StateTransaction<'_, '_>) -> u64 {
    [
        state_transaction
            .world
            .soracloud_service_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_app_infra_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_training_job_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_model_weight_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_model_artifact_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_hf_shared_lease_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_model_host_violation_evidence
            .iter()
            .map(|(_evidence_id, record)| record.sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_agent_apartment_audit_events
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        state_transaction
            .world
            .soracloud_runtime_receipts
            .iter()
            .map(|(_receipt_id, receipt)| receipt.emitted_sequence)
            .max()
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
    .saturating_add(1)
}
fn parse_training_model_name(model_name: &str) -> Result<String, InstructionExecutionError> {
    let normalized = model_name.trim();
    let parsed: iroha_data_model::name::Name = normalized
        .parse()
        .map_err(|err| invalid_parameter(format!("invalid model_name: {err}")))?;
    Ok(parsed.to_string())
}
fn parse_training_job_id(job_id: &str) -> Result<String, InstructionExecutionError> {
    let normalized = job_id.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("job_id must not be empty"));
    }
    if normalized.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(invalid_parameter(format!(
            "job_id exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "job_id must not contain control characters",
        ));
    }
    if normalized.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(invalid_parameter("job_id must not contain whitespace"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(invalid_parameter(
            "job_id must use only ASCII letters, digits, or [- _ . : #]",
        ));
    }
    Ok(normalized.to_owned())
}
fn parse_model_weight_version(weight_version: &str) -> Result<String, InstructionExecutionError> {
    let normalized = weight_version.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("weight_version must not be empty"));
    }
    if normalized.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(invalid_parameter(format!(
            "weight_version exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "weight_version must not contain control characters",
        ));
    }
    if normalized.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(invalid_parameter(
            "weight_version must not contain whitespace",
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(invalid_parameter(
            "weight_version must use only ASCII letters, digits, or [- _ . : #]",
        ));
    }
    Ok(normalized.to_owned())
}
fn parse_model_weight_dataset_ref(dataset_ref: &str) -> Result<String, InstructionExecutionError> {
    let normalized = dataset_ref.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("dataset_ref must not be empty"));
    }
    if normalized.len() > MODEL_WEIGHT_MAX_DATASET_REF_BYTES {
        return Err(invalid_parameter(format!(
            "dataset_ref exceeds max bytes ({MODEL_WEIGHT_MAX_DATASET_REF_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "dataset_ref must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}
fn parse_uploaded_model_id(model_id: &str) -> Result<String, InstructionExecutionError> {
    parse_training_job_id(model_id).map_err(|_| invalid_parameter("invalid model_id"))
}
fn parse_uploaded_artifact_id(artifact_id: &str) -> Result<String, InstructionExecutionError> {
    parse_training_job_id(artifact_id).map_err(|_| invalid_parameter("invalid artifact_id"))
}
fn service_allows_uploaded_model_plane(bundle: &SoraDeploymentBundleV1) -> bool {
    bundle.container.capabilities.allow_model_training
        || bundle.container.capabilities.allow_model_inference
}
fn require_active_sorafs_uploaded_model_pin(
    state_transaction: &StateTransaction<'_, '_>,
    bundle: &SoraUploadedModelBundleV1,
) -> Result<(), InstructionExecutionError> {
    let Some(pin) = state_transaction
        .world
        .pin_manifests
        .get(&bundle.sorafs_manifest_digest)
    else {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "SoraFS manifest {:?} for uploaded model `{}` version `{}` is not registered",
                bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
            )
            .into(),
        ));
    };
    match pin.status {
        PinStatus::Approved(_) => Ok(()),
        PinStatus::Pending => Err(InstructionExecutionError::InvariantViolation(
            format!(
                "SoraFS manifest {:?} for uploaded model `{}` version `{}` is not approved",
                bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
            )
            .into(),
        )),
        PinStatus::Retired(epoch) => Err(InstructionExecutionError::InvariantViolation(
            format!(
                "SoraFS manifest {:?} for uploaded model `{}` version `{}` retired at epoch {epoch}",
                bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
            )
            .into(),
        )),
    }?;
    if pin.digest != bundle.sorafs_manifest_digest {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "SoraFS manifest record digest {:?} does not match uploaded model digest {:?}",
                pin.digest, bundle.sorafs_manifest_digest
            )
            .into(),
        ));
    }
    if pin.content_length != bundle.ciphertext_bytes {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "SoraFS manifest {:?} content_length {} does not match uploaded model ciphertext_bytes {}",
                bundle.sorafs_manifest_digest, pin.content_length, bundle.ciphertext_bytes
            )
            .into(),
        ));
    }
    Ok(())
}
fn normalize_training_reason(reason: &str) -> Result<String, InstructionExecutionError> {
    let normalized = reason.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("reason must not be empty"));
    }
    if normalized.len() > TRAINING_MAX_REASON_BYTES {
        return Err(invalid_parameter(format!(
            "reason exceeds max bytes ({TRAINING_MAX_REASON_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "reason must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}
fn normalize_model_weight_reason(reason: &str) -> Result<String, InstructionExecutionError> {
    let normalized = reason.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("reason must not be empty"));
    }
    if normalized.len() > MODEL_WEIGHT_MAX_REASON_BYTES {
        return Err(invalid_parameter(format!(
            "reason exceeds max bytes ({MODEL_WEIGHT_MAX_REASON_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "reason must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}
fn normalize_model_host_violation_detail(
    detail: Option<String>,
) -> Result<Option<String>, InstructionExecutionError> {
    detail
        .map(|detail| {
            let normalized = detail.trim();
            if normalized.is_empty() {
                return Err(invalid_parameter("detail must not be empty"));
            }
            if normalized.len() > MODEL_HOST_VIOLATION_MAX_DETAIL_BYTES {
                return Err(invalid_parameter(format!(
                    "detail exceeds max bytes ({MODEL_HOST_VIOLATION_MAX_DETAIL_BYTES})"
                )));
            }
            if normalized.chars().any(char::is_control) {
                return Err(invalid_parameter(
                    "detail must not contain control characters",
                ));
            }
            Ok(normalized.to_owned())
        })
        .transpose()
}
fn parse_agent_capability_name(capability: &str) -> Result<String, InstructionExecutionError> {
    let normalized: Name = capability
        .trim()
        .parse()
        .map_err(|err| invalid_parameter(format!("invalid capability: {err}")))?;
    Ok(normalized.to_string())
}
fn normalize_hf_token(
    field_name: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<String, InstructionExecutionError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter(format!("{field_name} must not be empty")));
    }
    if normalized.len() > max_bytes {
        return Err(invalid_parameter(format!(
            "{field_name} exceeds max bytes ({max_bytes})"
        )));
    }
    if normalized.chars().any(char::is_control) || normalized.chars().any(char::is_whitespace) {
        return Err(invalid_parameter(format!(
            "{field_name} must not contain control characters or whitespace"
        )));
    }
    Ok(normalized.to_owned())
}
fn parse_hf_repo_id(repo_id: &str) -> Result<String, InstructionExecutionError> {
    normalize_hf_token("repo_id", repo_id, HF_REPO_ID_MAX_BYTES)
}
fn parse_hf_revision(revision: &str) -> Result<String, InstructionExecutionError> {
    normalize_hf_token("resolved_revision", revision, HF_REVISION_MAX_BYTES)
}
fn parse_hf_model_name(model_name: &str) -> Result<String, InstructionExecutionError> {
    normalize_hf_token("model_name", model_name, HF_MODEL_NAME_MAX_BYTES)
}
fn verify_provenance_payload(
    authority: &AccountId,
    provenance: &ManifestProvenance,
    payload: Vec<u8>,
    signer_mismatch: &'static str,
    verification_failed: &'static str,
) -> Result<(), InstructionExecutionError> {
    if single_signatory_authority(authority)? != &provenance.signer {
        return Err(invalid_parameter(signer_mismatch));
    }
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| invalid_parameter(verification_failed))?;
    Ok(())
}
fn normalize_agent_hash_like(
    field_name: &'static str,
    value: &str,
) -> Result<String, InstructionExecutionError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter(format!("{field_name} must not be empty")));
    }
    if normalized.len() > AGENT_AUTONOMY_MAX_HASH_BYTES {
        return Err(invalid_parameter(format!(
            "{field_name} exceeds max bytes ({AGENT_AUTONOMY_MAX_HASH_BYTES})"
        )));
    }
    if normalized.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(invalid_parameter(format!(
            "{field_name} must not contain whitespace"
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '-' | '_' | '.' | '#'))
    {
        return Err(invalid_parameter(format!(
            "{field_name} must use only ASCII letters, digits, or [: - _ . #]"
        )));
    }
    Ok(normalized.to_owned())
}
fn normalize_optional_agent_hash_like(
    field_name: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, InstructionExecutionError> {
    value
        .map(|value| normalize_agent_hash_like(field_name, value))
        .transpose()
}
fn normalize_agent_run_label(run_label: &str) -> Result<String, InstructionExecutionError> {
    let normalized = run_label.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter("run_label must not be empty"));
    }
    if normalized.len() > AGENT_AUTONOMY_MAX_LABEL_BYTES {
        return Err(invalid_parameter(format!(
            "run_label exceeds max bytes ({AGENT_AUTONOMY_MAX_LABEL_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_parameter(
            "run_label must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}
fn normalize_optional_agent_workflow_input_json(
    workflow_input_json: Option<&str>,
) -> Result<Option<String>, InstructionExecutionError> {
    let Some(workflow_input_json) = workflow_input_json else {
        return Ok(None);
    };
    let normalized = workflow_input_json.trim();
    if normalized.is_empty() {
        return Err(invalid_parameter(
            "workflow_input_json must not be empty when provided",
        ));
    }
    if normalized.len() > AGENT_AUTONOMY_MAX_REQUEST_BYTES {
        return Err(invalid_parameter(format!(
            "workflow_input_json exceeds max bytes ({AGENT_AUTONOMY_MAX_REQUEST_BYTES})"
        )));
    }
    let parsed = norito::json::from_str::<norito::json::Value>(normalized).map_err(|error| {
        invalid_parameter(format!("workflow_input_json must be valid JSON: {error}"))
    })?;
    let canonical = norito::json::to_json(&parsed).map_err(|error| {
        invalid_parameter(format!(
            "workflow_input_json canonicalization failed: {error}"
        ))
    })?;
    if canonical.len() > AGENT_AUTONOMY_MAX_REQUEST_BYTES {
        return Err(invalid_parameter(format!(
            "workflow_input_json exceeds max bytes ({AGENT_AUTONOMY_MAX_REQUEST_BYTES}) after canonicalization"
        )));
    }
    Ok(Some(canonical))
}
fn agent_policy_capability_active(record: &SoraAgentApartmentRecordV1, capability: &str) -> bool {
    let declared = record
        .manifest
        .policy_capabilities
        .iter()
        .any(|candidate| candidate.as_ref() == capability);
    declared && !record.revoked_policy_capabilities.contains(capability)
}
fn agent_runtime_status_for_sequence(
    record: &SoraAgentApartmentRecordV1,
    current_sequence: u64,
) -> SoraAgentRuntimeStatusV1 {
    if current_sequence >= record.lease_expires_sequence {
        SoraAgentRuntimeStatusV1::LeaseExpired
    } else {
        record.status
    }
}
fn touch_agent_runtime_activity(record: &mut SoraAgentApartmentRecordV1, sequence: u64) {
    record.last_active_sequence = record.last_active_sequence.max(sequence);
}
fn wallet_day_bucket(sequence: u64) -> u64 {
    sequence / AGENT_WALLET_DAY_TICKS
}
fn wallet_day_spent(
    record: &SoraAgentApartmentRecordV1,
    asset_definition: &str,
    day_bucket: u64,
) -> Quantity {
    let key = format!("{asset_definition}:{day_bucket}");
    record
        .wallet_daily_spend
        .get(&key)
        .map(|entry| entry.spent.clone())
        .unwrap_or_else(Quantity::zero)
}
fn wallet_record_spend(
    record: &mut SoraAgentApartmentRecordV1,
    asset_definition: &str,
    day_bucket: u64,
    spent: Quantity,
) {
    let key = format!("{asset_definition}:{day_bucket}");
    record.wallet_daily_spend.insert(
        key,
        SoraAgentWalletDailySpendEntryV1 {
            asset_definition: asset_definition.to_owned(),
            day_bucket,
            spent,
        },
    );
}
fn projected_agent_persistent_state_total_bytes(
    record: &SoraAgentApartmentRecordV1,
    key: &str,
    value_size_bytes: u64,
) -> Result<u64, InstructionExecutionError> {
    let existing_size = record
        .persistent_state
        .key_sizes
        .get(key)
        .copied()
        .unwrap_or(0);
    record
        .persistent_state
        .total_bytes
        .saturating_sub(existing_size)
        .checked_add(value_size_bytes)
        .ok_or_else(|| {
            invalid_parameter(format!(
                "persistent state accounting overflow for apartment `{}`",
                record.manifest.apartment_name
            ))
        })
}
fn projected_binding_state_total_bytes(
    binding_name: &str,
    current_total_bytes: u64,
    existing_size_bytes: u64,
    value_size_bytes: u64,
) -> Result<u64, InstructionExecutionError> {
    let remaining_total_bytes = current_total_bytes
        .checked_sub(existing_size_bytes)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "binding `{binding_name}` state accounting is inconsistent: \
                     existing item bytes {existing_size_bytes} exceed binding total \
                     {current_total_bytes}"
                )
                .into(),
            )
        })?;
    remaining_total_bytes
        .checked_add(value_size_bytes)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("binding `{binding_name}` state byte accounting overflow").into(),
            )
        })
}
fn autonomy_checkpoint_key(apartment_name: &str, run_id: &str) -> String {
    format!("/{apartment_name}/autonomy/{run_id}")
}
fn autonomy_checkpoint_value_size(
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    run_label: &str,
    budget_units: u64,
    workflow_input_json: Option<&str>,
) -> u64 {
    let mut value_size = u64::try_from(artifact_hash.len()).unwrap_or(u64::MAX);
    value_size = value_size.saturating_add(u64::try_from(run_label.len()).unwrap_or(u64::MAX));
    value_size = value_size
        .saturating_add(u64::try_from(budget_units.to_string().len()).unwrap_or(u64::MAX));
    if let Some(hash) = provenance_hash {
        value_size = value_size.saturating_add(u64::try_from(hash.len()).unwrap_or(u64::MAX));
    }
    if let Some(workflow_input_json) = workflow_input_json {
        value_size =
            value_size.saturating_add(u64::try_from(workflow_input_json.len()).unwrap_or(u64::MAX));
    }
    value_size.saturating_add(32)
}
fn count_revisions_for_service(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
) -> u32 {
    u32::try_from(
        state_transaction
            .world
            .soracloud_service_revisions
            .iter()
            .filter(|((stored_service, _version), _bundle)| stored_service == service_name.as_ref())
            .count(),
    )
    .unwrap_or(u32::MAX)
}
fn rollout_handle(service_name: &str, sequence: u64) -> String {
    format!("{service_name}:rollout:{sequence}")
}
#[cfg(test)]
fn latest_service_audit_event(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
) -> Option<SoraServiceAuditEventV1> {
    state_transaction
        .world
        .soracloud_service_audit_events
        .iter()
        .filter(|(_sequence, event)| &event.service_name == service_name)
        .map(|(_sequence, event)| event.clone())
        .max_by_key(|event| event.sequence)
}
fn previous_service_version(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
    current_version: &str,
) -> Option<String> {
    state_transaction
        .world
        .soracloud_service_audit_events
        .iter()
        .filter(|(_sequence, event)| {
            &event.service_name == service_name && event.to_version != current_version
        })
        .map(|(_sequence, event)| event.clone())
        .max_by_key(|event| event.sequence)
        .map(|event| event.to_version)
}
fn load_admitted_bundle(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
    service_version: &str,
) -> Result<SoraDeploymentBundleV1, InstructionExecutionError> {
    state_transaction
        .world
        .soracloud_service_revisions
        .get(&(service_name.as_ref().to_owned(), service_version.to_owned()))
        .cloned()
        .ok_or_else(|| {
            invalid_parameter(format!(
                "service `{}` revision `{service_version}` has not been admitted",
                service_name
            ))
        })
}
pub(crate) fn load_active_bundle(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
) -> Result<(SoraServiceDeploymentStateV1, SoraDeploymentBundleV1), InstructionExecutionError> {
    let deployment = state_transaction
        .world
        .soracloud_service_deployments
        .get(service_name)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("service `{service_name}` is not deployed").into(),
            )
        })?;
    let bundle = load_admitted_bundle(
        state_transaction,
        service_name,
        &deployment.current_service_version,
    )?;
    Ok((deployment, bundle))
}
fn resolve_active_soracloud_fhe_material(
    deployment: &SoraServiceDeploymentStateV1,
    policy_reference: &SoracloudFhePolicyReferenceV1,
) -> Result<(SoracloudFheGovernedMaterialV1, Hash), InstructionExecutionError> {
    policy_reference
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    let record = deployment
        .fhe_policy_records
        .get(&policy_reference.policy_name)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "FHE policy `{}` is not registered for service `{}`",
                    policy_reference.policy_name, deployment.service_name
                )
                .into(),
            )
        })?;
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if record.active_version != Some(policy_reference.version) {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "FHE policy `{}` version {} is not the exact active version",
                policy_reference.policy_name, policy_reference.version
            )
            .into(),
        ));
    }
    let version_state = record
        .versions
        .get(&policy_reference.version)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "active FHE policy version is missing from its authenticated history".into(),
            )
        })?;
    if version_state.lifecycle != SoracloudFhePolicyVersionLifecycleV1::Active {
        return Err(InstructionExecutionError::InvariantViolation(
            "referenced FHE policy version is revoked or superseded".into(),
        ));
    }
    if version_state.material.material_digest != policy_reference.material_digest {
        return Err(InstructionExecutionError::InvariantViolation(
            "referenced FHE policy material digest does not match authenticated state".into(),
        ));
    }
    Ok((
        version_state.material.clone(),
        version_state.admitted_by_transaction_hash,
    ))
}
pub(crate) fn write_soracloud_runtime_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: SoraServiceRuntimeStateV1,
) -> Result<(), InstructionExecutionError> {
    state
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    let Some(deployment) = state_transaction
        .world
        .soracloud_service_deployments
        .get(&state.service_name)
    else {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("service `{}` is not deployed", state.service_name).into(),
        ));
    };
    if deployment.current_service_version != state.active_service_version {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "service `{}` runtime state version `{}` does not match the active deployment `{}`",
                state.service_name,
                state.active_service_version,
                deployment.current_service_version
            )
            .into(),
        ));
    }
    state_transaction
        .world
        .soracloud_service_runtime
        .insert(state.service_name.clone(), state);
    Ok(())
}
pub(crate) fn write_soracloud_service_lease_usage(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: Name,
    active_service_version: String,
    accounted_egress_bytes: u64,
) -> Result<(), InstructionExecutionError> {
    let mut deployment = state_transaction
        .world
        .soracloud_service_deployments
        .get(&service_name)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("service `{service_name}` is not deployed").into(),
            )
        })?;
    if deployment.current_service_version != active_service_version {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "service `{service_name}` lease usage version `{active_service_version}` does not match the active deployment `{}`",
                deployment.current_service_version
            )
            .into(),
        ));
    }
    let accounted_storage_bytes = deployment.accounted_storage_bytes();
    let current_sequence = next_soracloud_audit_sequence(state_transaction);
    let lease = deployment.service_lease.as_mut().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            format!("service `{service_name}` does not have an active hosted-service lease").into(),
        )
    })?;
    if accounted_egress_bytes < lease.accounted_egress_bytes {
        return Err(invalid_parameter(format!(
            "service `{service_name}` lease usage must not decrease authoritative accounted egress bytes from {} to {accounted_egress_bytes}",
            lease.accounted_egress_bytes
        )));
    }
    lease.accounted_egress_bytes = accounted_egress_bytes;
    match lease
        .status_at(current_sequence, accounted_storage_bytes)
        .map_err(|error| {
            invalid_quantity_arithmetic(
                "failed to calculate hosted-service lease status after usage update",
                error,
            )
        })? {
        SoraServiceLeaseStatusV1::Active => {
            lease.status = SoraServiceLeaseStatusV1::Active;
            lease.last_status_reason = None;
        }
        SoraServiceLeaseStatusV1::Exhausted => {
            lease.status = SoraServiceLeaseStatusV1::Exhausted;
            lease.last_status_reason =
                Some("prepaid runtime balance exhausted by accounted egress usage".to_string());
        }
        SoraServiceLeaseStatusV1::Expired => {
            lease.status = SoraServiceLeaseStatusV1::Expired;
            lease.last_status_reason =
                Some("service lease expired before additional usage could be billed".to_string());
        }
        SoraServiceLeaseStatusV1::Suspended => {}
    }
    record_deployment_state(state_transaction, deployment)
}
pub(crate) fn write_soracloud_mailbox_message(
    state_transaction: &mut StateTransaction<'_, '_>,
    message: SoraServiceMailboxMessageV1,
) -> Result<(), InstructionExecutionError> {
    message
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if state_transaction
        .world
        .soracloud_mailbox_messages
        .get(&message.message_id)
        .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "Soracloud mailbox message `{}` has already been recorded",
                message.message_id
            )
            .into(),
        ));
    }
    if state_transaction
        .world
        .soracloud_service_deployments
        .get(&message.from_service)
        .is_none()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("source service `{}` is not deployed", message.from_service).into(),
        ));
    }
    if state_transaction
        .world
        .soracloud_service_deployments
        .get(&message.to_service)
        .is_none()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "destination service `{}` is not deployed",
                message.to_service
            )
            .into(),
        ));
    }
    state_transaction
        .world
        .soracloud_mailbox_messages
        .insert(message.message_id, message);
    Ok(())
}
pub(crate) fn write_soracloud_runtime_receipt(
    state_transaction: &mut StateTransaction<'_, '_>,
    receipt: SoraRuntimeReceiptV1,
) -> Result<(), InstructionExecutionError> {
    receipt
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if state_transaction
        .world
        .soracloud_runtime_receipts
        .get(&receipt.receipt_id)
        .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "Soracloud runtime receipt `{}` has already been recorded",
                receipt.receipt_id
            )
            .into(),
        ));
    }
    load_admitted_bundle(
        state_transaction,
        &receipt.service_name,
        &receipt.service_version,
    )?;
    if let Some(message_id) = receipt.mailbox_message_id
        && state_transaction
            .world
            .soracloud_mailbox_messages
            .get(&message_id)
            .is_none()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("mailbox message `{message_id}` has not been recorded").into(),
        ));
    }
    if let Some(mut runtime_state) = state_transaction
        .world
        .soracloud_service_runtime
        .get(&receipt.service_name)
        .cloned()
    {
        runtime_state.last_receipt_id = Some(receipt.receipt_id);
        runtime_state
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        state_transaction
            .world
            .soracloud_service_runtime
            .insert(runtime_state.service_name.clone(), runtime_state);
    }
    state_transaction
        .world
        .soracloud_runtime_receipts
        .insert(receipt.receipt_id, receipt);
    Ok(())
}
pub(crate) fn write_soracloud_private_uploaded_model_execution_receipt(
    state_transaction: &mut StateTransaction<'_, '_>,
    receipt: SoraPrivateUploadedModelExecutionReceiptV1,
) -> Result<(), InstructionExecutionError> {
    receipt
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if state_transaction
        .world
        .soracloud_private_uploaded_model_execution_receipts
        .get(&receipt.receipt_id)
        .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "Soracloud private uploaded-model execution receipt `{}` has already been recorded",
                receipt.receipt_id
            )
            .into(),
        ));
    }
    let Some(bundle) = state_transaction
        .world
        .soracloud_uploaded_model_bundles
        .get(&(
            receipt.service_name.as_ref().to_owned(),
            receipt.model_id.clone(),
            receipt.weight_version.clone(),
        ))
    else {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "uploaded model `{}` version `{}` for service `{}` has not been finalized",
                receipt.model_id, receipt.weight_version, receipt.service_name
            )
            .into(),
        ));
    };
    if bundle.runtime_format
        != iroha_data_model::soracloud::SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "private receipt `{}` targets uploaded model `{}` version `{}` with a non-deterministic private runtime format",
                receipt.receipt_id, receipt.model_id, receipt.weight_version
            )
            .into(),
        ));
    }
    if bundle.sorafs_manifest_digest != receipt.model_manifest_digest
        || bundle.bundle_root != receipt.model_bundle_root
        || bundle.decryption_policy_ref != receipt.policy_id
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "private receipt `{}` does not match finalized uploaded model `{}` version `{}`",
                receipt.receipt_id, receipt.model_id, receipt.weight_version
            )
            .into(),
        ));
    }
    state_transaction
        .world
        .soracloud_private_uploaded_model_execution_receipts
        .insert(receipt.receipt_id, receipt);
    Ok(())
}
/// Apply an authoritative Soracloud service-state mutation using the active binding contract.
///
/// The `linkage_hash` is persisted in the service-state row's `governance_tx_hash` field.
/// Ordered runtime execution reuses deterministic receipt identifiers here in v1 so the
/// write-back remains reconstructible from authoritative records without adding a parallel store.
pub(crate) fn apply_soracloud_state_mutation(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    state_key: &str,
    operation: SoraStateMutationOperationV1,
    payload: Option<Vec<u8>>,
    encryption: SoraStateEncryptionV1,
    fhe_public_key_digest: Option<Hash>,
    fhe_residual_multiple_bound: Option<u128>,
    fhe_bound_mode: Option<BfvCiphertextBoundModeV1>,
    linkage_hash: Hash,
    sequence: u64,
) -> Result<(SoraServiceDeploymentStateV1, SoraDeploymentBundleV1), InstructionExecutionError> {
    if state_key.trim().is_empty() {
        return Err(invalid_parameter("state_key must not be empty"));
    }
    if !state_key.starts_with('/') {
        return Err(invalid_parameter("state_key must start with '/'"));
    }
    let (deployment, bundle) = load_active_bundle(state_transaction, service_name)?;
    let binding = bundle
        .service
        .state_bindings
        .iter()
        .find(|binding| binding.binding_name == *binding_name)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("binding `{binding_name}` is not declared for service `{service_name}`")
                    .into(),
            )
        })?;
    if binding.encryption != encryption {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "binding `{binding_name}` requires {:?} encryption",
                binding.encryption
            )
            .into(),
        ));
    }
    if !state_key.starts_with(&binding.key_prefix) {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "state key `{state_key}` is outside binding prefix `{}`",
                binding.key_prefix
            )
            .into(),
        ));
    }
    let state_entry_key = (
        service_name.as_ref().to_owned(),
        binding_name.as_ref().to_owned(),
        state_key.to_owned(),
    );
    let existing_entry = state_transaction
        .world
        .soracloud_service_state_entries
        .get(&state_entry_key)
        .cloned();
    let existing_size = existing_entry
        .as_ref()
        .map_or(0, |entry| entry.payload_bytes.get());
    let (binding_total_bytes, _binding_key_count) =
        binding_state_totals(state_transaction, service_name, binding_name);
    match operation {
        SoraStateMutationOperationV1::Upsert => {
            if binding.mutability == iroha_data_model::soracloud::SoraStateMutabilityV1::ReadOnly {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("binding `{binding_name}` is read-only").into(),
                ));
            }
            let payload = payload.ok_or_else(|| {
                invalid_parameter("value_payload is required for upsert mutations")
            })?;
            let value_size_bytes = u64::try_from(payload.len())
                .map_err(|_| invalid_parameter("value_payload length exceeds u64 range"))?;
            let payload_bytes = std::num::NonZeroU64::new(value_size_bytes).ok_or_else(|| {
                invalid_parameter("value_payload must be non-empty for upsert mutations")
            })?;
            let payload_commitment = Hash::new(&payload);
            if value_size_bytes > binding.max_item_bytes.get() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "payload_bytes {value_size_bytes} exceeds binding max_item_bytes {}",
                        binding.max_item_bytes
                    )
                    .into(),
                ));
            }
            if binding.mutability == iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly
                && existing_size > 0
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "binding `{binding_name}` is append-only; key `{state_key}` already exists"
                    )
                    .into(),
                ));
            }
            let tentative_total = projected_binding_state_total_bytes(
                binding_name.as_ref(),
                binding_total_bytes,
                existing_size,
                value_size_bytes,
            )?;
            if tentative_total > binding.max_total_bytes.get() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "binding `{binding_name}` max_total_bytes {} would be exceeded",
                        binding.max_total_bytes
                    )
                    .into(),
                ));
            }
            record_service_state_entry(
                state_transaction,
                SoraServiceStateEntryV1 {
                    schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: deployment.current_service_version.clone(),
                    binding_name: binding_name.clone(),
                    state_key: state_key.to_owned(),
                    encryption,
                    payload,
                    payload_bytes,
                    payload_commitment,
                    fhe_public_key_digest,
                    fhe_residual_multiple_bound,
                    fhe_bound_mode,
                    last_update_sequence: sequence,
                    governance_tx_hash: linkage_hash,
                    source_action: SoraServiceLifecycleActionV1::StateMutation,
                },
            )?;
        }
        SoraStateMutationOperationV1::Delete => {
            if payload.is_some() {
                return Err(invalid_parameter(
                    "delete mutations must not provide value_payload",
                ));
            }
            if binding.mutability != iroha_data_model::soracloud::SoraStateMutabilityV1::ReadWrite {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("binding `{binding_name}` does not allow deletes").into(),
                ));
            }
            state_transaction
                .world
                .soracloud_service_state_entries
                .remove(state_entry_key);
        }
    }
    Ok((deployment, bundle))
}
fn build_rollout_state(
    bundle: &SoraDeploymentBundleV1,
    sequence: u64,
    baseline_version: Option<String>,
) -> Result<SoraServiceRolloutStateV1, InstructionExecutionError> {
    let canary_percent = bundle.service.rollout.canary_percent.min(100);
    let traffic_percent = if canary_percent == 0 {
        100
    } else {
        canary_percent
    };
    let rollout_state = SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: rollout_handle(bundle.service.service_name.as_ref(), sequence),
        baseline_version,
        candidate_version: bundle.service.service_version.clone(),
        canary_percent,
        traffic_percent,
        stage: if traffic_percent == 100 {
            SoraRolloutStageV1::Promoted
        } else {
            SoraRolloutStageV1::Canary
        },
        health_failures: 0,
        max_health_failures: bundle.service.rollout.automatic_rollback_failures.get(),
        health_window_secs: bundle.service.rollout.health_window_secs.get(),
        created_sequence: sequence,
        updated_sequence: sequence,
    };
    rollout_state
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    Ok(rollout_state)
}
fn record_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraServiceAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_service_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn record_deployment_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: SoraServiceDeploymentStateV1,
) -> Result<(), InstructionExecutionError> {
    state
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_service_deployments
        .insert(state.service_name.clone(), state);
    Ok(())
}
fn record_app_infra_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraAppInfraAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_app_infra_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn record_app_infra_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: SoraAppInfraStateV1,
) -> Result<(), InstructionExecutionError> {
    state
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_app_infra_states
        .insert(state.app_name.clone(), state);
    Ok(())
}
fn build_http_service_lease_state(
    bundle: &SoraDeploymentBundleV1,
    existing: Option<&SoraServiceDeploymentStateV1>,
    sequence: u64,
    extend_terms: bool,
) -> Result<Option<SoraServiceLeaseStateV1>, InstructionExecutionError> {
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService {
        return Ok(None);
    }
    let economics = &bundle.service.economics;
    let existing_lease = existing.and_then(|deployment| deployment.service_lease.as_ref());
    let quota_class = economics.quota_class.clone();
    let deployment_deposit = existing_lease.map_or_else(
        || economics.deployment_deposit.clone(),
        |lease| {
            lease
                .deployment_deposit
                .clone()
                .max(economics.deployment_deposit.clone())
        },
    );
    let prepaid_runtime_balance = match existing_lease {
        Some(lease) if extend_terms => lease
            .prepaid_runtime_balance
            .checked_add(&economics.prepaid_runtime_balance)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "hosted-service prepaid runtime balance overflow while extending lease",
                    error,
                )
            })?,
        Some(lease) => lease.prepaid_runtime_balance.clone(),
        None => economics.prepaid_runtime_balance.clone(),
    };
    let lease_started_sequence =
        existing_lease.map_or(sequence, |lease| lease.lease_started_sequence);
    let lease_expires_sequence = existing_lease.map_or(
        sequence.saturating_add(economics.lease_duration_sequences.get()),
        |lease| {
            if extend_terms {
                lease
                    .lease_expires_sequence
                    .max(sequence)
                    .saturating_add(economics.lease_duration_sequences.get())
            } else {
                lease.lease_expires_sequence
            }
        },
    );
    let existing_status =
        existing_lease.map_or(SoraServiceLeaseStatusV1::Active, |lease| lease.status);
    let status = if existing_status == SoraServiceLeaseStatusV1::Suspended {
        SoraServiceLeaseStatusV1::Suspended
    } else if sequence >= lease_expires_sequence {
        SoraServiceLeaseStatusV1::Expired
    } else {
        SoraServiceLeaseStatusV1::Active
    };
    Ok(Some(SoraServiceLeaseStateV1 {
        schema_version: SORA_SERVICE_LEASE_STATE_VERSION_V1,
        status,
        quota_class,
        deployment_deposit,
        prepaid_runtime_balance,
        runtime_price_per_sequence: economics.runtime_price_per_sequence.clone(),
        storage_price_per_gib_sequence: economics.storage_price_per_gib_sequence.clone(),
        egress_price_per_mib: economics.egress_price_per_mib.clone(),
        lease_started_sequence,
        lease_expires_sequence,
        last_billed_sequence: existing_lease
            .map_or(sequence, |lease| lease.last_billed_sequence)
            .clamp(lease_started_sequence, lease_expires_sequence),
        accounted_egress_bytes: existing_lease.map_or(0, |lease| lease.accounted_egress_bytes),
        last_status_reason: existing_lease.and_then(|lease| lease.last_status_reason.clone()),
    }))
}
fn build_http_service_lease_volume_states(
    bundle: &SoraDeploymentBundleV1,
    lease_state: Option<&SoraServiceLeaseStateV1>,
    existing: Option<&SoraServiceDeploymentStateV1>,
) -> Vec<SoraServiceLeaseVolumeStateV1> {
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService {
        return Vec::new();
    }
    let Some(lease_state) = lease_state else {
        return Vec::new();
    };
    bundle
        .service
        .lease_volumes
        .iter()
        .map(|volume| {
            let existing_state = existing.and_then(|deployment| {
                deployment
                    .lease_volume_states
                    .iter()
                    .find(|state| state.volume_name == volume.volume_name)
            });
            let unchanged = existing_state.is_some_and(|state| {
                state.kind == volume.kind
                    && state.storage_class == volume.storage_class
                    && state.mount_path == volume.mount_path
                    && state.max_total_bytes == volume.max_total_bytes.get()
            });
            SoraServiceLeaseVolumeStateV1 {
                schema_version: SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
                volume_name: volume.volume_name.clone(),
                kind: volume.kind,
                storage_class: volume.storage_class,
                mount_path: volume.mount_path.clone(),
                max_total_bytes: volume.max_total_bytes.get(),
                lease_started_sequence: lease_state.lease_started_sequence,
                lease_expires_sequence: lease_state.lease_expires_sequence,
                authoritative_generation: existing_state.map_or(1, |state| {
                    if unchanged {
                        state.authoritative_generation
                    } else {
                        state.authoritative_generation.saturating_add(1)
                    }
                }),
                last_materialized_sequence: existing_state
                    .and_then(|state| state.last_materialized_sequence),
            }
        })
        .collect()
}
fn apply_service_config_mutation(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &Name,
    config_name: &str,
    value_json: Option<Json>,
    sequence: u64,
) -> Result<(SoraServiceDeploymentStateV1, SoraDeploymentBundleV1), InstructionExecutionError> {
    let (mut deployment, bundle) = load_active_bundle(state_transaction, service_name)?;
    match value_json {
        Some(value_json) => {
            let value_hash = service_config_value_hash(&value_json)?;
            deployment.service_configs.insert(
                config_name.to_owned(),
                SoraServiceConfigEntryV1 {
                    schema_version: SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                    config_name: config_name.to_owned(),
                    value_json,
                    value_hash,
                    last_update_sequence: sequence,
                },
            );
            deployment.config_generation = deployment.config_generation.saturating_add(1);
        }
        None => {
            if deployment.service_configs.remove(config_name).is_none() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("service `{service_name}` config `{config_name}` is not present")
                        .into(),
                ));
            }
            deployment.config_generation = deployment.config_generation.saturating_add(1);
        }
    }
    bundle
        .validate_required_service_materials(
            &deployment.service_configs,
            &deployment.service_secrets,
        )
        .map_err(|err| InstructionExecutionError::InvariantViolation(err.to_string().into()))?;
    record_deployment_state(state_transaction, deployment.clone())?;
    Ok((deployment, bundle))
}
fn apply_service_secret_mutation(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &Name,
    secret_name: &str,
    secret: Option<SecretEnvelopeV1>,
    sequence: u64,
) -> Result<(SoraServiceDeploymentStateV1, SoraDeploymentBundleV1), InstructionExecutionError> {
    let (mut deployment, bundle) = load_active_bundle(state_transaction, service_name)?;
    match secret {
        Some(secret) => {
            deployment.service_secrets.insert(
                secret_name.to_owned(),
                SoraServiceSecretEntryV1 {
                    schema_version: SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                    secret_name: secret_name.to_owned(),
                    envelope: secret,
                    last_update_sequence: sequence,
                },
            );
            deployment.secret_generation = deployment.secret_generation.saturating_add(1);
        }
        None => {
            if deployment.service_secrets.remove(secret_name).is_none() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("service `{service_name}` secret `{secret_name}` is not present")
                        .into(),
                ));
            }
            deployment.secret_generation = deployment.secret_generation.saturating_add(1);
        }
    }
    bundle
        .validate_required_service_materials(
            &deployment.service_configs,
            &deployment.service_secrets,
        )
        .map_err(|err| InstructionExecutionError::InvariantViolation(err.to_string().into()))?;
    record_deployment_state(state_transaction, deployment.clone())?;
    Ok((deployment, bundle))
}
fn record_service_state_entry(
    state_transaction: &mut StateTransaction<'_, '_>,
    entry: SoraServiceStateEntryV1,
) -> Result<(), InstructionExecutionError> {
    entry
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_service_state_entries
        .insert(
            (
                entry.service_name.as_ref().to_owned(),
                entry.binding_name.as_ref().to_owned(),
                entry.state_key.clone(),
            ),
            entry,
        );
    Ok(())
}
fn record_training_job(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraTrainingJobRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction.world.soracloud_training_jobs.insert(
        (
            record.service_name.as_ref().to_owned(),
            record.job_id.clone(),
        ),
        record,
    );
    Ok(())
}
fn record_training_job_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraTrainingJobAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_training_job_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn record_model_registry(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraModelRegistryV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction.world.soracloud_model_registries.insert(
        (
            record.service_name.as_ref().to_owned(),
            record.model_name.clone(),
        ),
        record,
    );
    Ok(())
}
fn record_model_weight_version(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraModelWeightVersionRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_model_weight_versions
        .insert(
            (
                record.service_name.as_ref().to_owned(),
                record.model_name.clone(),
                record.weight_version.clone(),
            ),
            record,
        );
    Ok(())
}
fn record_model_weight_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraModelWeightAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_model_weight_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn record_model_artifact(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraModelArtifactRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction.world.soracloud_model_artifacts.insert(
        (
            record.service_name.as_ref().to_owned(),
            record.artifact_id.clone(),
        ),
        record,
    );
    Ok(())
}
fn record_model_artifact_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraModelArtifactAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_model_artifact_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn record_uploaded_model_bundle(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraUploadedModelBundleV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_uploaded_model_bundles
        .insert(
            (
                record.service_name.as_ref().to_owned(),
                record.model_id.clone(),
                record.weight_version.clone(),
            ),
            record,
        );
    Ok(())
}
fn record_hf_source(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraHfSourceRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_sources
        .insert(record.source_id, record);
    Ok(())
}
fn record_model_host_capability(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraModelHostCapabilityRecordV1,
) -> Result<(), InstructionExecutionError> {
    validate_model_host_capability_against_class(&record)?;
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_model_host_capabilities
        .insert(record.validator_account_id.clone(), record);
    Ok(())
}
fn record_inrou_host_capability(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraInrouHostCapabilityRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_inrou_host_capabilities
        .insert(record.validator_account_id.clone(), record);
    Ok(())
}
fn record_inrou_service_placement(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraInrouServicePlacementRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_inrou_service_placements
        .insert(
            (
                record.service_name.as_ref().to_owned(),
                record.service_version.clone(),
            ),
            record,
        );
    Ok(())
}
fn write_soracloud_inrou_replica_runtime_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: SoraInrouReplicaRuntimeStateV1,
) -> Result<(), InstructionExecutionError> {
    state
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_inrou_replica_runtime
        .insert(
            inrou_replica_runtime_key(
                &state.service_name,
                &state.service_version,
                state.replica_slot,
            ),
            state,
        );
    Ok(())
}
fn inrou_replica_runtime_key(
    service_name: &Name,
    service_version: &str,
    replica_slot: u16,
) -> (String, String, String) {
    (
        service_name.as_ref().to_owned(),
        service_version.to_owned(),
        replica_slot.to_string(),
    )
}
fn clear_soracloud_inrou_replica_runtime_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &Name,
    service_version: &str,
    replica_slot: u16,
) {
    state_transaction
        .world
        .soracloud_inrou_replica_runtime
        .remove(inrou_replica_runtime_key(
            service_name,
            service_version,
            replica_slot,
        ));
}
fn find_inrou_replica_assignment(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &Name,
    service_version: &str,
    replica_slot: u16,
) -> Option<SoraInrouReplicaPlacementV1> {
    state_transaction
        .world
        .soracloud_inrou_service_placements
        .get(&(service_name.as_ref().to_owned(), service_version.to_owned()))
        .and_then(|placement| {
            placement
                .placements
                .iter()
                .find(|placement| placement.replica_slot == replica_slot)
        })
        .cloned()
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct InrouHostReservationUsage {
    hosted_replicas: u16,
    cpu_millis: u64,
    memory_bytes: u64,
    storage_bytes: u64,
}
fn inrou_per_replica_storage_bytes(bundle: &SoraDeploymentBundleV1) -> u64 {
    let per_replica_volume_bytes = bundle
        .service
        .lease_volumes
        .iter()
        .filter(|volume| volume.kind.is_per_replica())
        .fold(0_u64, |total, volume| {
            total.saturating_add(volume.max_total_bytes.get())
        });
    bundle
        .container
        .resources
        .ephemeral_storage_bytes
        .get()
        .saturating_add(per_replica_volume_bytes)
}
fn accumulate_inrou_host_reservation_usage(
    usage_by_validator: &mut BTreeMap<AccountId, InrouHostReservationUsage>,
    validator_account_id: &AccountId,
    bundle: &SoraDeploymentBundleV1,
) {
    let usage = usage_by_validator
        .entry(validator_account_id.clone())
        .or_default();
    usage.hosted_replicas = usage.hosted_replicas.saturating_add(1);
    usage.cpu_millis = usage
        .cpu_millis
        .saturating_add(u64::from(bundle.container.resources.cpu_millis.get()));
    usage.memory_bytes = usage
        .memory_bytes
        .saturating_add(bundle.container.resources.memory_bytes.get());
    usage.storage_bytes = usage
        .storage_bytes
        .saturating_add(inrou_per_replica_storage_bytes(bundle));
}
fn select_inrou_backend_for_host(
    capability: &SoraInrouHostCapabilityRecordV1,
) -> Option<SoraInrouRuntimeBackendV1> {
    if capability
        .supported_backends
        .contains(&SoraInrouRuntimeBackendV1::FirecrackerKvm)
    {
        Some(SoraInrouRuntimeBackendV1::FirecrackerKvm)
    } else if capability
        .supported_backends
        .contains(&SoraInrouRuntimeBackendV1::PortableVm)
    {
        Some(SoraInrouRuntimeBackendV1::PortableVm)
    } else {
        None
    }
}
fn select_inrou_guest_isa_for_host(
    capability: &SoraInrouHostCapabilityRecordV1,
    bundle: &SoraDeploymentBundleV1,
) -> Option<SoraInrouGuestIsaV1> {
    let inrou = bundle.container.inrou.as_ref()?;
    [SoraInrouGuestIsaV1::X8664, SoraInrouGuestIsaV1::Aarch64]
        .into_iter()
        .find(|guest_isa| {
            capability.supported_guest_isas.contains(guest_isa)
                && inrou.guest_images.contains_key(guest_isa)
        })
}
fn inrou_host_supports_bundle(
    capability: &SoraInrouHostCapabilityRecordV1,
    bundle: &SoraDeploymentBundleV1,
    reserved_usage: Option<&InrouHostReservationUsage>,
    now_ms: u64,
) -> Option<(SoraInrouRuntimeBackendV1, SoraInrouGuestIsaV1)> {
    if !capability.can_host_replicas_at(now_ms) {
        return None;
    }
    if bundle.container.runtime != iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou
        || bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
    {
        return None;
    }
    let selected_backend = select_inrou_backend_for_host(capability)?;
    let selected_guest_isa = select_inrou_guest_isa_for_host(capability, bundle)?;
    let usage = reserved_usage.copied().unwrap_or_default();
    let next_hosted_replicas = u32::from(usage.hosted_replicas).saturating_add(1);
    let next_cpu_millis = usage
        .cpu_millis
        .saturating_add(u64::from(bundle.container.resources.cpu_millis.get()));
    let next_memory_bytes = usage
        .memory_bytes
        .saturating_add(bundle.container.resources.memory_bytes.get());
    let next_storage_bytes = usage
        .storage_bytes
        .saturating_add(inrou_per_replica_storage_bytes(bundle));
    if next_hosted_replicas > u32::from(capability.max_hosted_replica_capacity)
        || next_cpu_millis > u64::from(capability.max_cpu_millis)
        || next_memory_bytes > capability.max_memory_bytes
        || next_storage_bytes > capability.max_storage_bytes
    {
        return None;
    }
    Some((selected_backend, selected_guest_isa))
}
fn inrou_replica_selection_digest(
    service_name: &Name,
    service_version: &str,
    replica_slot: u16,
    validator_account_id: &AccountId,
) -> Result<[u8; 32], InstructionExecutionError> {
    let payload = norito::to_bytes(&(
        service_name.clone(),
        service_version.to_owned(),
        replica_slot,
        validator_account_id.clone(),
    ))
    .map_err(|err| invalid_parameter(format!("failed to encode Inrou placement seed: {err}")))?;
    Ok(*Hash::new(payload).as_ref())
}
fn select_inrou_replica_placement(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &Name,
    service_version: &str,
    bundle: &SoraDeploymentBundleV1,
    replica_slot: u16,
    reserved_usage_by_validator: &mut BTreeMap<AccountId, InrouHostReservationUsage>,
    now_ms: u64,
) -> Result<Option<SoraInrouReplicaPlacementV1>, InstructionExecutionError> {
    let mut candidates = state_transaction
        .world
        .soracloud_inrou_host_capabilities
        .iter()
        .filter_map(|(validator_account_id, capability)| {
            let (selected_backend, selected_guest_isa) = inrou_host_supports_bundle(
                capability,
                bundle,
                reserved_usage_by_validator.get(validator_account_id),
                now_ms,
            )?;
            Some((
                validator_account_id.clone(),
                capability.clone(),
                selected_backend,
                selected_guest_isa,
            ))
        })
        .map(
            |(validator_account_id, capability, selected_backend, selected_guest_isa)| {
                inrou_replica_selection_digest(
                    service_name,
                    service_version,
                    replica_slot,
                    &validator_account_id,
                )
                .map(|digest| {
                    (
                        validator_account_id,
                        capability,
                        selected_backend,
                        selected_guest_isa,
                        digest,
                    )
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    candidates.sort_by(
        |(left_account, _, left_backend, left_isa, left_digest),
         (right_account, _, right_backend, right_isa, right_digest)| {
            right_digest
                .cmp(left_digest)
                .then_with(|| left_backend.cmp(right_backend))
                .then_with(|| left_isa.cmp(right_isa))
                .then_with(|| left_account.cmp(right_account))
        },
    );
    let Some((validator_account_id, capability, selected_backend, selected_guest_isa, _digest)) =
        candidates.into_iter().next()
    else {
        return Ok(None);
    };
    accumulate_inrou_host_reservation_usage(
        reserved_usage_by_validator,
        &validator_account_id,
        bundle,
    );
    Ok(Some(SoraInrouReplicaPlacementV1 {
        replica_slot,
        validator_account_id,
        peer_id: capability.peer_id,
        selected_backend,
        selected_guest_isa,
        selected_geography_tag: None,
        selection_latency_ms: None,
    }))
}
fn active_inrou_service_versions(deployment: &SoraServiceDeploymentStateV1) -> Vec<String> {
    let mut versions = vec![deployment.current_service_version.clone()];
    if let Some(rollout) = deployment.active_rollout.as_ref()
        && rollout.traffic_percent > 0
        && rollout.candidate_version != deployment.current_service_version
    {
        versions.push(rollout.candidate_version.clone());
    }
    versions
}
fn reconcile_inrou_service_placements(
    state_transaction: &mut StateTransaction<'_, '_>,
    now_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let current_sequence = next_soracloud_audit_sequence(state_transaction);
    let deployment_keys = state_transaction
        .world
        .soracloud_service_deployments
        .iter()
        .map(|(service_name, _deployment)| service_name.clone())
        .collect::<Vec<_>>();
    let mut desired_records =
        BTreeMap::<(String, String), SoraInrouServicePlacementRecordV1>::new();
    let mut desired_slots =
        BTreeMap::<(String, String, String), SoraInrouReplicaPlacementV1>::new();
    let mut reserved_usage_by_validator = BTreeMap::<AccountId, InrouHostReservationUsage>::new();
    for service_name in deployment_keys {
        let Some(deployment) = state_transaction
            .world
            .soracloud_service_deployments
            .get(&service_name)
            .cloned()
        else {
            continue;
        };
        if !deployment
            .hosted_service_lease_active_at(current_sequence)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "failed to calculate hosted-service lease status during Inrou reconciliation",
                    error,
                )
            })?
        {
            continue;
        }
        for service_version in active_inrou_service_versions(&deployment) {
            let bundle = load_admitted_bundle(state_transaction, &service_name, &service_version)?;
            if bundle.container.runtime
                != iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou
                || bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
            {
                continue;
            }
            let eligible_validator_count = u32::try_from(
                state_transaction
                    .world
                    .soracloud_inrou_host_capabilities
                    .iter()
                    .filter(|(validator_account_id, capability)| {
                        inrou_host_supports_bundle(
                            capability,
                            &bundle,
                            reserved_usage_by_validator.get(validator_account_id),
                            now_ms,
                        )
                        .is_some()
                    })
                    .count(),
            )
            .unwrap_or(u32::MAX);
            let desired_replica_count = bundle.service.replicas.get();
            let mut placements = Vec::with_capacity(usize::from(desired_replica_count));
            for replica_slot in 1..=desired_replica_count {
                let Some(placement) = select_inrou_replica_placement(
                    state_transaction,
                    &service_name,
                    &service_version,
                    &bundle,
                    replica_slot,
                    &mut reserved_usage_by_validator,
                    now_ms,
                )?
                else {
                    break;
                };
                desired_slots.insert(
                    inrou_replica_runtime_key(
                        &service_name,
                        &service_version,
                        placement.replica_slot,
                    ),
                    placement.clone(),
                );
                placements.push(placement);
            }
            let last_error = (placements.len() < usize::from(desired_replica_count)).then(|| {
                format!(
                    "placed {} of {} replicas using {} eligible validators",
                    placements.len(),
                    desired_replica_count,
                    eligible_validator_count
                )
            });
            let record = SoraInrouServicePlacementRecordV1 {
                schema_version: SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                desired_replica_count,
                eligible_validator_count,
                placements,
                reconciled_at_ms: now_ms.max(1),
                last_error,
            };
            desired_records.insert((service_name.as_ref().to_owned(), service_version), record);
        }
    }
    let stale_placement_keys = state_transaction
        .world
        .soracloud_inrou_service_placements
        .iter()
        .filter_map(|(key, _record)| (!desired_records.contains_key(key)).then_some(key.clone()))
        .collect::<Vec<_>>();
    for key in stale_placement_keys {
        state_transaction
            .world
            .soracloud_inrou_service_placements
            .remove(key);
    }
    for record in desired_records.into_values() {
        record_inrou_service_placement(state_transaction, record)?;
    }
    let stale_runtime_keys = state_transaction
        .world
        .soracloud_inrou_replica_runtime
        .iter()
        .filter_map(|(key, state)| {
            let placement = desired_slots.get(key)?;
            (state.validator_account_id != placement.validator_account_id
                || state.peer_id != placement.peer_id
                || state.selected_backend != placement.selected_backend
                || state.selected_guest_isa != placement.selected_guest_isa)
                .then_some(key.clone())
        })
        .chain(
            state_transaction
                .world
                .soracloud_inrou_replica_runtime
                .iter()
                .filter_map(|(key, _state)| {
                    (!desired_slots.contains_key(key)).then_some(key.clone())
                }),
        )
        .collect::<BTreeSet<_>>();
    for key in stale_runtime_keys {
        state_transaction
            .world
            .soracloud_inrou_replica_runtime
            .remove(key);
    }
    Ok(())
}
fn recompute_hf_placement_total_reservation_fee(
    placement: &mut SoraHfPlacementRecordV1,
) -> Result<(), InstructionExecutionError> {
    placement.total_reservation_fee =
        placement
            .assigned_hosts
            .iter()
            .try_fold(Quantity::zero(), |total, host| {
                let fee =
                    hf_host_class_reservation_fee(&host.host_class, &placement.resource_profile)?;
                total.checked_add(&fee).map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF placement reservation fee exceeds the supported decimal domain",
                        error,
                    )
                })
            })?;
    Ok(())
}
fn sync_hf_placements_for_host_capability(
    state_transaction: &mut StateTransaction<'_, '_>,
    capability: &SoraModelHostCapabilityRecordV1,
    now_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let placements = active_hf_assigned_placements_for_validator(
        state_transaction,
        &capability.validator_account_id,
        now_ms,
    );
    for mut placement in placements {
        let mut changed = false;
        for assignment in &mut placement.assigned_hosts {
            if assignment.validator_account_id != capability.validator_account_id
                || matches!(
                    assignment.status,
                    SoraHfPlacementHostStatusV1::Retired | SoraHfPlacementHostStatusV1::Unavailable
                )
            {
                continue;
            }
            if assignment.peer_id != capability.peer_id {
                assignment.peer_id = capability.peer_id.clone();
                changed = true;
            }
            if assignment.host_class != capability.host_class {
                assignment.host_class = capability.host_class.clone();
                changed = true;
            }
        }
        if changed {
            recompute_hf_placement_total_reservation_fee(&mut placement)?;
            placement.last_rebalance_at_ms = now_ms;
            record_hf_placement(state_transaction, placement)?;
        }
    }
    Ok(())
}
fn record_hf_shared_lease_pool(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraHfSharedLeasePoolV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_shared_lease_pools
        .insert(record.pool_id, record);
    Ok(())
}
fn derive_hf_placement_status(record: &SoraHfPlacementRecordV1) -> SoraHfPlacementStatusV1 {
    let warm_primary = record.assigned_hosts.iter().any(|assignment| {
        assignment.status == SoraHfPlacementHostStatusV1::Warm
            && matches!(
                assignment.role,
                iroha_data_model::soracloud::SoraHfPlacementHostRoleV1::Primary
            )
    });
    let warm_count = record.warm_host_count();
    if warm_primary {
        if usize::from(record.adaptive_target_host_count) == record.assigned_hosts.len()
            && usize::try_from(warm_count).unwrap_or(usize::MAX) == record.assigned_hosts.len()
        {
            SoraHfPlacementStatusV1::Ready
        } else {
            SoraHfPlacementStatusV1::Degraded
        }
    } else if warm_count > 0 {
        SoraHfPlacementStatusV1::Degraded
    } else if record
        .assigned_hosts
        .iter()
        .any(|assignment| assignment.status == SoraHfPlacementHostStatusV1::Warming)
    {
        SoraHfPlacementStatusV1::Warming
    } else if record.assigned_hosts.is_empty() {
        SoraHfPlacementStatusV1::Selecting
    } else {
        SoraHfPlacementStatusV1::Unavailable
    }
}
fn record_hf_placement(
    state_transaction: &mut StateTransaction<'_, '_>,
    mut record: SoraHfPlacementRecordV1,
) -> Result<(), InstructionExecutionError> {
    record.status = derive_hf_placement_status(&record);
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_placements
        .insert(record.pool_id, record);
    Ok(())
}
fn load_hf_placement_by_placement_id(
    state_transaction: &StateTransaction<'_, '_>,
    placement_id: &Hash,
) -> Result<SoraHfPlacementRecordV1, InstructionExecutionError> {
    let matches = state_transaction
        .world
        .soracloud_hf_placements
        .iter()
        .filter_map(|(_pool_id, placement)| {
            (placement.placement_id == *placement_id).then_some(placement.clone())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [placement] => Ok(placement.clone()),
        [] => Err(InstructionExecutionError::InvariantViolation(
            format!("hf placement `{placement_id}` not found").into(),
        )),
        _ => Err(InstructionExecutionError::InvariantViolation(
            format!("hf placement `{placement_id}` is duplicated in authoritative state").into(),
        )),
    }
}
fn load_hf_shared_lease_pool_record(
    state_transaction: &StateTransaction<'_, '_>,
    pool_id: &Hash,
) -> Result<SoraHfSharedLeasePoolV1, InstructionExecutionError> {
    state_transaction
        .world
        .soracloud_hf_shared_lease_pools
        .get(pool_id)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("hf shared lease pool `{pool_id}` not found").into(),
            )
        })
}
fn model_host_violation_evidence_id(
    validator_account_id: &AccountId,
    kind: SoraModelHostViolationKindV1,
    placement_id: Option<Hash>,
    sequence: u64,
    observed_at_ms: u64,
) -> Result<Hash, InstructionExecutionError> {
    let payload = norito::to_bytes(&(
        "soracloud-model-host-violation",
        validator_account_id.clone(),
        kind,
        placement_id,
        sequence,
        observed_at_ms,
    ))
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode host-violation evidence id: {err}"
        ))
    })?;
    Ok(Hash::new(payload))
}
fn model_host_violation_slash_id(
    validator_account_id: &AccountId,
    kind: SoraModelHostViolationKindV1,
    placement_id: Option<Hash>,
    sequence: u64,
    observed_at_ms: u64,
) -> Result<Hash, InstructionExecutionError> {
    let payload = norito::to_bytes(&(
        "soracloud-model-host-slash",
        validator_account_id.clone(),
        kind,
        placement_id,
        sequence,
        observed_at_ms,
    ))
    .map_err(|err| invalid_parameter(format!("failed to encode host-violation slash id: {err}")))?;
    Ok(Hash::new(payload))
}
fn record_model_host_violation_evidence(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraModelHostViolationEvidenceRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_model_host_violation_evidence
        .insert(record.evidence_id, record);
    Ok(())
}
fn assigned_heartbeat_miss_history(
    state_transaction: &StateTransaction<'_, '_>,
    validator_account_id: &AccountId,
    placement_id: &Hash,
    window_started_at_ms: u64,
) -> (u32, bool) {
    let mut max_strike_count = 0_u32;
    let mut penalty_already_applied = false;
    for (_evidence_id, record) in state_transaction
        .world
        .soracloud_model_host_violation_evidence
        .iter()
    {
        if record.validator_account_id != *validator_account_id
            || record.kind != SoraModelHostViolationKindV1::AssignedHeartbeatMiss
            || record.placement_id.as_ref() != Some(placement_id)
            || record.window_started_at_ms != Some(window_started_at_ms)
        {
            continue;
        }
        max_strike_count = max_strike_count.max(record.strike_count);
        penalty_already_applied |= record.penalty_applied;
    }
    (max_strike_count, penalty_already_applied)
}
fn slash_validator_for_model_host_violation(
    state_transaction: &mut StateTransaction<'_, '_>,
    validator_account_id: &AccountId,
    slash_bps: u16,
    slash_id: Hash,
) -> Result<Option<Hash>, InstructionExecutionError> {
    if slash_bps == 0 {
        return Ok(None);
    }
    let effective_bps = slash_bps.min(state_transaction.nexus.staking.max_slash_bps);
    if effective_bps == 0 {
        return Ok(None);
    }
    let slashable_records = state_transaction
        .world
        .public_lane_validators
        .iter()
        .filter_map(|((lane_id, candidate_validator), record)| {
            (candidate_validator == validator_account_id
                && !matches!(
                    record.status,
                    PublicLaneValidatorStatus::Exited | PublicLaneValidatorStatus::Slashed(_)
                ))
            .then_some((lane_id.clone(), record.clone()))
        })
        .collect::<Vec<_>>();
    let mut slashed_any = false;
    for (lane_id, record) in slashable_records {
        let amount = max_slash_amount(&record.total_stake, effective_bps).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to compute validator slash amount: {err}").into(),
            )
        })?;
        if amount.is_zero() {
            continue;
        }
        let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
        apply_slash_to_validator(
            state_transaction,
            lane_id,
            validator_account_id,
            slash_id,
            &amount,
            recorded_at_ms,
        )?;
        slashed_any = true;
    }
    Ok(slashed_any.then_some(slash_id))
}
fn model_host_violation_reason(kind: SoraModelHostViolationKindV1, detail: Option<&str>) -> String {
    detail.map(ToOwned::to_owned).unwrap_or_else(|| match kind {
        SoraModelHostViolationKindV1::WarmupNoShow => {
            "assigned host warmup expired before becoming ready".to_string()
        }
        SoraModelHostViolationKindV1::AssignedHeartbeatMiss => {
            "assigned host heartbeat expired".to_string()
        }
        SoraModelHostViolationKindV1::AdvertContradiction => {
            "assigned host advert contradicted authoritative placement requirements".to_string()
        }
    })
}
fn report_model_host_violation(
    state_transaction: &mut StateTransaction<'_, '_>,
    validator_account_id: &AccountId,
    kind: SoraModelHostViolationKindV1,
    placement_id: Option<Hash>,
    detail: Option<String>,
    observed_at_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let detail = normalize_model_host_violation_detail(detail)?;
    let placement = placement_id
        .as_ref()
        .map(|placement_id| load_hf_placement_by_placement_id(state_transaction, placement_id))
        .transpose()?;
    if let Some(placement) = placement.as_ref() {
        if !placement
            .assigned_hosts
            .iter()
            .any(|assignment| assignment.validator_account_id == *validator_account_id)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "validator `{validator_account_id}` is not assigned to placement `{}`",
                    placement.placement_id
                )
                .into(),
            ));
        }
    }
    let pool = placement
        .as_ref()
        .map(|placement| load_hf_shared_lease_pool_record(state_transaction, &placement.pool_id))
        .transpose()?;
    let (strike_count, threshold_reached, penalty_already_applied) =
        if let (SoraModelHostViolationKindV1::AssignedHeartbeatMiss, Some(placement), Some(pool)) =
            (kind, placement.as_ref(), pool.as_ref())
        {
            let (prior_strikes, prior_penalty_applied) = assigned_heartbeat_miss_history(
                state_transaction,
                validator_account_id,
                &placement.placement_id,
                pool.window_started_at_ms,
            );
            let strike_count = prior_strikes.saturating_add(1);
            let threshold_reached = strike_count
                >= state_transaction
                    .nexus
                    .hf_shared_leases
                    .assigned_heartbeat_miss_strike_threshold;
            (strike_count, threshold_reached, prior_penalty_applied)
        } else {
            (1, true, false)
        };
    let should_apply_penalty = match kind {
        SoraModelHostViolationKindV1::WarmupNoShow
        | SoraModelHostViolationKindV1::AdvertContradiction => true,
        SoraModelHostViolationKindV1::AssignedHeartbeatMiss => {
            threshold_reached && !penalty_already_applied
        }
    };
    let slash_bps = match kind {
        SoraModelHostViolationKindV1::WarmupNoShow => {
            state_transaction
                .nexus
                .hf_shared_leases
                .warmup_no_show_slash_bps
        }
        SoraModelHostViolationKindV1::AssignedHeartbeatMiss => {
            state_transaction
                .nexus
                .hf_shared_leases
                .assigned_heartbeat_miss_slash_bps
        }
        SoraModelHostViolationKindV1::AdvertContradiction => {
            state_transaction
                .nexus
                .hf_shared_leases
                .advert_contradiction_slash_bps
        }
    };
    let sequence = next_soracloud_audit_sequence(state_transaction);
    let slash_id = if should_apply_penalty {
        let slash_id = model_host_violation_slash_id(
            validator_account_id,
            kind,
            placement_id.clone(),
            sequence,
            observed_at_ms,
        )?;
        slash_validator_for_model_host_violation(
            state_transaction,
            validator_account_id,
            slash_bps,
            slash_id,
        )?
    } else {
        None
    };
    let host_evicted = should_apply_penalty;
    if host_evicted {
        state_transaction
            .world
            .soracloud_model_host_capabilities
            .remove(validator_account_id.clone());
    }
    let evidence_id = model_host_violation_evidence_id(
        validator_account_id,
        kind,
        placement_id.clone(),
        sequence,
        observed_at_ms,
    )?;
    let record = SoraModelHostViolationEvidenceRecordV1 {
        schema_version: SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1,
        evidence_id,
        sequence,
        validator_account_id: validator_account_id.clone(),
        kind,
        placement_id: placement_id.clone(),
        pool_id: placement.as_ref().map(|placement| placement.pool_id),
        source_id: placement.as_ref().map(|placement| placement.source_id),
        window_started_at_ms: pool.as_ref().map(|pool| pool.window_started_at_ms),
        observed_at_ms,
        detail: detail.clone(),
        strike_count,
        penalty_applied: slash_id.is_some(),
        host_evicted,
        slash_id,
    };
    record_model_host_violation_evidence(state_transaction, record)?;
    if placement.is_some() || host_evicted {
        let reason = model_host_violation_reason(kind, detail.as_deref());
        refresh_hf_placements_for_host_status(
            state_transaction,
            validator_account_id,
            SoraHfPlacementHostStatusV1::Unavailable,
            observed_at_ms,
            Some(&reason),
        )?;
    }
    Ok(())
}
fn reconcile_expired_model_hosts(
    state_transaction: &mut StateTransaction<'_, '_>,
    now_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let expired_validator_account_ids = state_transaction
        .world
        .soracloud_model_host_capabilities
        .iter()
        .filter_map(|(validator_account_id, capability)| {
            (!capability.is_active_at(now_ms)).then_some(validator_account_id.clone())
        })
        .collect::<Vec<_>>();
    for validator_account_id in expired_validator_account_ids {
        let impacted_placements = state_transaction
            .world
            .soracloud_hf_placements
            .iter()
            .filter_map(|(_pool_id, placement)| {
                placement
                    .assigned_hosts
                    .iter()
                    .find(|assignment| assignment.validator_account_id == validator_account_id)
                    .map(|assignment| (placement.placement_id, assignment.status))
            })
            .collect::<Vec<_>>();
        if impacted_placements.is_empty() {
            refresh_hf_placements_for_host_status(
                state_transaction,
                &validator_account_id,
                SoraHfPlacementHostStatusV1::Unavailable,
                now_ms,
                Some("assigned host heartbeat expired"),
            )?;
            continue;
        }
        for (placement_id, host_status) in impacted_placements {
            match host_status {
                SoraHfPlacementHostStatusV1::Warm => report_model_host_violation(
                    state_transaction,
                    &validator_account_id,
                    SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
                    Some(placement_id),
                    Some("assigned host heartbeat expired".to_string()),
                    now_ms,
                )?,
                SoraHfPlacementHostStatusV1::Warming => report_model_host_violation(
                    state_transaction,
                    &validator_account_id,
                    SoraModelHostViolationKindV1::WarmupNoShow,
                    Some(placement_id),
                    Some("assigned host warmup expired before becoming ready".to_string()),
                    now_ms,
                )?,
                _ => refresh_hf_placements_for_host_status(
                    state_transaction,
                    &validator_account_id,
                    SoraHfPlacementHostStatusV1::Unavailable,
                    now_ms,
                    Some("assigned host heartbeat expired"),
                )?,
            }
        }
    }
    Ok(())
}
fn record_hf_shared_lease_member(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraHfSharedLeaseMemberV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_shared_lease_members
        .insert(
            (record.pool_id.to_string(), record.account_id.to_string()),
            record,
        );
    Ok(())
}
fn refresh_hf_placements_for_host_status(
    state_transaction: &mut StateTransaction<'_, '_>,
    validator_account_id: &AccountId,
    next_host_status: SoraHfPlacementHostStatusV1,
    now_ms: u64,
    reason: Option<&str>,
) -> Result<(), InstructionExecutionError> {
    let placements = state_transaction
        .world
        .soracloud_hf_placements
        .iter()
        .filter_map(|(_pool_id, record)| {
            record
                .assigned_hosts
                .iter()
                .any(|assignment| assignment.validator_account_id == *validator_account_id)
                .then_some(record.clone())
        })
        .collect::<Vec<_>>();
    for mut placement in placements {
        let mut changed = false;
        for assignment in &mut placement.assigned_hosts {
            if assignment.validator_account_id == *validator_account_id
                && assignment.status != next_host_status
            {
                assignment.status = next_host_status;
                changed = true;
            }
        }
        if changed {
            if next_host_status == SoraHfPlacementHostStatusV1::Unavailable {
                let ranked_eligible = ranked_hf_eligible_hosts_by_seed(
                    state_transaction,
                    &placement.resource_profile,
                    &placement.selection_seed_hash,
                    now_ms,
                    Some(&placement.pool_id),
                )?
                .into_iter()
                .filter(|(account_id, _capability)| account_id != validator_account_id)
                .collect::<Vec<_>>();
                placement.eligible_validator_count =
                    u32::try_from(ranked_eligible.len()).unwrap_or(u32::MAX);
                let rank_by_validator = ranked_eligible
                    .iter()
                    .enumerate()
                    .map(|(index, (account_id, _capability))| (account_id.clone(), index))
                    .collect::<BTreeMap<_, _>>();
                let target_host_count = placement
                    .adaptive_target_host_count
                    .min(u16::try_from(ranked_eligible.len()).unwrap_or(u16::MAX))
                    .max(1);
                let mut retained_assignments = placement
                    .assigned_hosts
                    .into_iter()
                    .filter(|assignment| {
                        !matches!(
                            assignment.status,
                            SoraHfPlacementHostStatusV1::Unavailable
                                | SoraHfPlacementHostStatusV1::Retired
                        ) && rank_by_validator.contains_key(&assignment.validator_account_id)
                    })
                    .collect::<Vec<_>>();
                retained_assignments.sort_by_key(|assignment| {
                    rank_by_validator
                        .get(&assignment.validator_account_id)
                        .copied()
                        .unwrap_or(usize::MAX)
                });
                let primary_validator = retained_assignments
                    .iter()
                    .find(|assignment| assignment.status == SoraHfPlacementHostStatusV1::Warm)
                    .or_else(|| retained_assignments.first())
                    .map(|assignment| assignment.validator_account_id.clone())
                    .or_else(|| {
                        ranked_eligible
                            .first()
                            .map(|(account_id, _)| account_id.clone())
                    });
                let mut assigned_hosts = Vec::new();
                for mut assignment in retained_assignments {
                    if assigned_hosts.len() >= usize::from(target_host_count) {
                        break;
                    }
                    assignment.role = if primary_validator
                        .as_ref()
                        .is_some_and(|primary| *primary == assignment.validator_account_id)
                    {
                        SoraHfPlacementHostRoleV1::Primary
                    } else {
                        SoraHfPlacementHostRoleV1::Replica
                    };
                    assigned_hosts.push(assignment);
                }
                for (account_id, capability) in ranked_eligible {
                    if assigned_hosts.len() >= usize::from(target_host_count) {
                        break;
                    }
                    if assigned_hosts
                        .iter()
                        .any(|assignment: &SoraHfPlacementHostAssignmentV1| {
                            assignment.validator_account_id == account_id
                        })
                    {
                        continue;
                    }
                    assigned_hosts.push(SoraHfPlacementHostAssignmentV1 {
                        validator_account_id: account_id.clone(),
                        peer_id: capability.peer_id,
                        role: if primary_validator
                            .as_ref()
                            .is_some_and(|primary| *primary == account_id)
                        {
                            SoraHfPlacementHostRoleV1::Primary
                        } else {
                            SoraHfPlacementHostRoleV1::Replica
                        },
                        status: SoraHfPlacementHostStatusV1::Warming,
                        host_class: capability.host_class,
                    });
                }
                if !assigned_hosts.is_empty()
                    && assigned_hosts
                        .iter()
                        .all(|assignment| assignment.role != SoraHfPlacementHostRoleV1::Primary)
                {
                    assigned_hosts[0].role = SoraHfPlacementHostRoleV1::Primary;
                }
                placement.assigned_hosts = assigned_hosts;
            }
            recompute_hf_placement_total_reservation_fee(&mut placement)?;
            placement.last_rebalance_at_ms = now_ms;
            placement.last_error = reason.map(ToOwned::to_owned);
            record_hf_placement(state_transaction, placement)?;
        }
    }
    Ok(())
}
fn record_hf_shared_lease_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraHfSharedLeaseAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_shared_lease_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn hf_source_id(repo_id: &str, resolved_revision: &str) -> Result<Hash, InstructionExecutionError> {
    let payload = norito::to_bytes(&(repo_id, resolved_revision))
        .map_err(|err| invalid_parameter(format!("failed to encode hf source id: {err}")))?;
    Ok(Hash::new(payload))
}
fn hf_shared_lease_pool_id(
    source_id: Hash,
    storage_class: StorageClass,
    lease_term_ms: u64,
) -> Result<Hash, InstructionExecutionError> {
    let payload = norito::to_bytes(&(source_id, storage_class, lease_term_ms)).map_err(|err| {
        invalid_parameter(format!("failed to encode hf shared lease pool id: {err}"))
    })?;
    Ok(Hash::new(payload))
}
fn resolve_hf_resource_profile(
    source_record: &mut SoraHfSourceRecordV1,
    resource_profile: Option<SoraHfResourceProfileV1>,
) -> Result<SoraHfResourceProfileV1, InstructionExecutionError> {
    match (&source_record.resource_profile, resource_profile) {
        (Some(existing), Some(profile)) => {
            profile
                .validate()
                .map_err(|err| invalid_parameter(err.to_string()))?;
            if *existing != profile {
                return Err(invalid_parameter(
                    "hf resource_profile does not match the canonical source profile",
                ));
            }
            Ok(existing.clone())
        }
        (Some(existing), None) => Ok(existing.clone()),
        (None, Some(profile)) => {
            profile
                .validate()
                .map_err(|err| invalid_parameter(err.to_string()))?;
            source_record.resource_profile = Some(profile.clone());
            Ok(profile)
        }
        (None, None) => Err(invalid_parameter(
            "hf resource_profile must be provided when the canonical source profile is unknown",
        )),
    }
}
fn retire_hf_placement_for_pool(
    state_transaction: &mut StateTransaction<'_, '_>,
    pool_id: &Hash,
    now_ms: u64,
    reason: &str,
) -> Result<(), InstructionExecutionError> {
    let Some(mut placement) = state_transaction
        .world
        .soracloud_hf_placements
        .get(pool_id)
        .cloned()
    else {
        return Ok(());
    };
    for assignment in &mut placement.assigned_hosts {
        assignment.status = SoraHfPlacementHostStatusV1::Retired;
    }
    placement.status = SoraHfPlacementStatusV1::Retired;
    placement.last_rebalance_at_ms = now_ms;
    placement.last_error = Some(reason.to_string());
    placement
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_hf_placements
        .insert(*pool_id, placement);
    Ok(())
}
fn hf_active_validator_stakes(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<BTreeMap<AccountId, u128>, InstructionExecutionError> {
    let mut stakes = BTreeMap::new();
    for (key, record) in state_transaction.world.public_lane_validators.iter() {
        if !public_lane_validator_record_matches_key(key, record) {
            continue;
        }
        if record.status != PublicLaneValidatorStatus::Active {
            continue;
        }
        if !state_transaction.is_lane_active_for_authority(key.0) {
            continue;
        }
        let stake = numeric_to_u128(record.total_stake.as_numeric())?;
        let entry = stakes.entry(key.1.clone()).or_insert(0_u128);
        *entry = (*entry).saturating_add(stake.max(1));
    }
    Ok(stakes)
}
fn accumulate_hf_host_reservation_usage(
    usage_by_validator: &mut BTreeMap<AccountId, HfHostReservationUsage>,
    validator_account_id: &AccountId,
    resource_profile: &SoraHfResourceProfileV1,
) {
    let usage = usage_by_validator
        .entry(validator_account_id.clone())
        .or_default();
    accumulate_hf_host_reservation_usage_totals(usage, resource_profile);
}
fn accumulate_hf_host_reservation_usage_totals(
    usage: &mut HfHostReservationUsage,
    resource_profile: &SoraHfResourceProfileV1,
) {
    usage.required_model_bytes = usage
        .required_model_bytes
        .saturating_add(resource_profile.required_model_bytes);
    usage.disk_cache_bytes = usage
        .disk_cache_bytes
        .saturating_add(resource_profile.disk_cache_bytes_floor);
    usage.ram_bytes = usage
        .ram_bytes
        .saturating_add(resource_profile.ram_bytes_floor);
    usage.vram_bytes = usage
        .vram_bytes
        .saturating_add(resource_profile.vram_bytes_floor);
    usage.resident_models = usage.resident_models.saturating_add(1);
}
fn hf_reserved_host_usage(
    state_transaction: &StateTransaction<'_, '_>,
    now_ms: u64,
    exclude_pool_id: Option<&Hash>,
) -> BTreeMap<AccountId, HfHostReservationUsage> {
    let mut usage_by_validator = BTreeMap::new();
    for (pool_id, placement) in state_transaction.world.soracloud_hf_placements.iter() {
        if exclude_pool_id.is_some_and(|excluded| excluded == pool_id) {
            continue;
        }
        if placement.status == SoraHfPlacementStatusV1::Retired {
            continue;
        }
        let Some(pool) = state_transaction
            .world
            .soracloud_hf_shared_lease_pools
            .get(pool_id)
        else {
            continue;
        };
        if matches!(
            pool.status,
            SoraHfSharedLeaseStatusV1::Expired | SoraHfSharedLeaseStatusV1::Retired
        ) {
            continue;
        }
        if pool.window_expires_at_ms <= now_ms && pool.queued_next_window.is_none() {
            continue;
        }
        for assignment in &placement.assigned_hosts {
            if matches!(
                assignment.status,
                SoraHfPlacementHostStatusV1::Retired | SoraHfPlacementHostStatusV1::Unavailable
            ) {
                continue;
            }
            accumulate_hf_host_reservation_usage(
                &mut usage_by_validator,
                &assignment.validator_account_id,
                &placement.resource_profile,
            );
        }
    }
    usage_by_validator
}
fn hf_placement_seed_hash(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: Hash,
    pool_id: Hash,
    window_started_at_ms: u64,
) -> Result<Hash, InstructionExecutionError> {
    let payload = norito::to_bytes(&(
        source_id,
        pool_id,
        window_started_at_ms,
        next_soracloud_audit_sequence(state_transaction),
    ))
    .map_err(|err| invalid_parameter(format!("failed to encode hf placement seed: {err}")))?;
    Ok(Hash::new(payload))
}
fn hf_ranked_validator_score(
    seed_hash: &Hash,
    validator_account_id: &AccountId,
    stake_weight: u128,
) -> Result<(u128, [u8; 32]), InstructionExecutionError> {
    let payload =
        norito::to_bytes(&(seed_hash.clone(), validator_account_id.clone())).map_err(|err| {
            invalid_parameter(format!(
                "failed to encode hf placement rendezvous input for `{validator_account_id}`: {err}"
            ))
        })?;
    let digest = Hash::new(payload);
    let digest_bytes = *digest.as_ref();
    let mut entropy_bytes = [0_u8; 16];
    entropy_bytes.copy_from_slice(&digest_bytes[..16]);
    let entropy = u128::from_be_bytes(entropy_bytes).saturating_add(1);
    Ok((entropy.saturating_mul(stake_weight.max(1)), digest_bytes))
}
fn hf_host_supports_resource_profile(
    capability: &SoraModelHostCapabilityRecordV1,
    resource_profile: &SoraHfResourceProfileV1,
    reserved_usage: Option<&HfHostReservationUsage>,
    now_ms: u64,
) -> bool {
    if !capability.is_active_at(now_ms) {
        return false;
    }
    if !capability
        .supported_backends
        .contains(&resource_profile.backend_family)
        || !capability
            .supported_formats
            .contains(&resource_profile.model_format)
    {
        return false;
    }
    let reserved_usage = reserved_usage.copied().unwrap_or_default();
    capability.max_model_bytes
        >= reserved_usage
            .required_model_bytes
            .saturating_add(resource_profile.required_model_bytes)
        && capability.max_disk_cache_bytes
            >= reserved_usage
                .disk_cache_bytes
                .saturating_add(resource_profile.disk_cache_bytes_floor)
        && capability.max_ram_bytes
            >= reserved_usage
                .ram_bytes
                .saturating_add(resource_profile.ram_bytes_floor)
        && capability.max_vram_bytes
            >= reserved_usage
                .vram_bytes
                .saturating_add(resource_profile.vram_bytes_floor)
        && capability.max_concurrent_resident_models
            >= reserved_usage.resident_models.saturating_add(1)
}
fn ranked_hf_eligible_hosts_by_seed(
    state_transaction: &StateTransaction<'_, '_>,
    resource_profile: &SoraHfResourceProfileV1,
    selection_seed_hash: &Hash,
    now_ms: u64,
    exclude_pool_id: Option<&Hash>,
) -> Result<Vec<(AccountId, SoraModelHostCapabilityRecordV1)>, InstructionExecutionError> {
    let active_validator_stakes = hf_active_validator_stakes(state_transaction)?;
    let reserved_usage_by_validator =
        hf_reserved_host_usage(state_transaction, now_ms, exclude_pool_id);
    let mut eligible = state_transaction
        .world
        .soracloud_model_host_capabilities
        .iter()
        .filter_map(|(validator_account_id, capability)| {
            let stake = active_validator_stakes.get(validator_account_id).copied()?;
            hf_host_supports_resource_profile(
                capability,
                resource_profile,
                reserved_usage_by_validator.get(validator_account_id),
                now_ms,
            )
            .then_some((validator_account_id.clone(), capability.clone(), stake))
        })
        .map(|(validator_account_id, capability, stake)| {
            hf_ranked_validator_score(selection_seed_hash, &validator_account_id, stake)
                .map(|(score, digest)| (validator_account_id, capability, score, digest))
        })
        .collect::<Result<Vec<_>, _>>()?;
    eligible.sort_by(
        |(left_account, _left_capability, left_score, left_digest),
         (right_account, _right_capability, right_score, right_digest)| {
            right_score
                .cmp(left_score)
                .then_with(|| right_digest.cmp(left_digest))
                .then_with(|| left_account.cmp(right_account))
        },
    );
    Ok(eligible
        .into_iter()
        .map(|(validator_account_id, capability, _score, _digest)| {
            (validator_account_id, capability)
        })
        .collect())
}
fn select_hf_placement_for_window(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: Hash,
    pool_id: Hash,
    resource_profile: &SoraHfResourceProfileV1,
    window_started_at_ms: u64,
    now_ms: u64,
    exclude_pool_id: Option<&Hash>,
) -> Result<SoraHfPlacementRecordV1, InstructionExecutionError> {
    let selection_seed_hash =
        hf_placement_seed_hash(state_transaction, source_id, pool_id, window_started_at_ms)?;
    let eligible = ranked_hf_eligible_hosts_by_seed(
        state_transaction,
        resource_profile,
        &selection_seed_hash,
        now_ms,
        exclude_pool_id,
    )?;
    let eligible_validator_count = u32::try_from(eligible.len()).unwrap_or(u32::MAX);
    if eligible.is_empty() {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "no eligible validator host advert can satisfy the canonical HF resource profile for pool `{pool_id}`"
            )
            .into(),
        ));
    }
    let adaptive_target_host_count = hf_adaptive_target_host_count(resource_profile)
        .min(u16::try_from(eligible.len()).unwrap_or(u16::MAX))
        .max(1);
    let assigned_hosts = eligible
        .into_iter()
        .take(usize::from(adaptive_target_host_count))
        .enumerate()
        .map(
            |(index, (validator_account_id, capability))| SoraHfPlacementHostAssignmentV1 {
                validator_account_id,
                peer_id: capability.peer_id,
                role: if index == 0 {
                    SoraHfPlacementHostRoleV1::Primary
                } else {
                    SoraHfPlacementHostRoleV1::Replica
                },
                status: SoraHfPlacementHostStatusV1::Warming,
                host_class: capability.host_class,
            },
        )
        .collect::<Vec<_>>();
    let placement_id_payload =
        norito::to_bytes(&(pool_id, selection_seed_hash)).map_err(|err| {
            invalid_parameter(format!("failed to encode hf placement_id payload: {err}"))
        })?;
    let mut placement = SoraHfPlacementRecordV1 {
        schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
        placement_id: Hash::new(placement_id_payload),
        source_id,
        pool_id,
        status: SoraHfPlacementStatusV1::Selecting,
        selection_seed_hash,
        resource_profile: resource_profile.clone(),
        eligible_validator_count,
        adaptive_target_host_count,
        assigned_hosts,
        total_reservation_fee: Quantity::zero(),
        last_rebalance_at_ms: now_ms.max(1),
        last_error: None,
    };
    recompute_hf_placement_total_reservation_fee(&mut placement)?;
    Ok(placement)
}
fn ensure_hf_placement_for_active_pool(
    state_transaction: &mut StateTransaction<'_, '_>,
    pool: &SoraHfSharedLeasePoolV1,
    resource_profile: &SoraHfResourceProfileV1,
    now_ms: u64,
) -> Result<SoraHfPlacementRecordV1, InstructionExecutionError> {
    reconcile_expired_model_hosts(state_transaction, now_ms)?;
    if let Some(existing) = state_transaction
        .world
        .soracloud_hf_placements
        .get(&pool.pool_id)
        .cloned()
    {
        return Ok(existing);
    }
    let placement = select_hf_placement_for_window(
        state_transaction,
        pool.source_id,
        pool.pool_id,
        resource_profile,
        pool.window_started_at_ms,
        now_ms,
        Some(&pool.pool_id),
    )?;
    record_hf_placement(state_transaction, placement.clone())?;
    Ok(placement)
}
fn prorated_window_fee(
    window_fee: &Quantity,
    remaining_ms: u64,
    lease_term_ms: u64,
) -> Result<Quantity, InstructionExecutionError> {
    if lease_term_ms == 0 {
        return Err(invalid_parameter(
            "cannot prorate a shared-lease fee over a zero lease term",
        ));
    }
    window_fee
        .try_mul_div_decimal_round(
            &Numeric::from(remaining_ms),
            &Numeric::from(lease_term_ms),
            window_fee.scale().max(9),
            RoundingMode::TowardZero,
        )
        .map_err(|error| {
            invalid_quantity_arithmetic("shared-lease prorated fee calculation failed", error)
        })
}
fn divide_quantity_by_member_count(
    amount: &Quantity,
    member_count: usize,
    context: &str,
) -> Result<Quantity, InstructionExecutionError> {
    let member_count = u64::try_from(member_count)
        .map_err(|_| invalid_parameter(format!("{context}: member count exceeds u64")))?;
    if member_count == 0 {
        return Err(invalid_parameter(format!(
            "{context}: member count must be greater than zero"
        )));
    }
    amount
        .try_div_decimal_round(
            &Numeric::from(member_count),
            amount.scale().max(9),
            RoundingMode::TowardZero,
        )
        .map_err(|error| invalid_quantity_arithmetic(context, error))
}
fn distribute_hf_join_refunds(
    authority: &AccountId,
    lease_asset_definition_id: &AssetDefinitionId,
    now_ms: u64,
    join_fee: &Quantity,
    existing_members: &mut [SoraHfSharedLeaseMemberV1],
    state_transaction: &mut StateTransaction<'_, '_>,
    storage_refund: bool,
) -> Result<(), InstructionExecutionError> {
    if existing_members.is_empty() || join_fee.is_zero() {
        return Ok(());
    }
    let member_count = u64::try_from(existing_members.len()).map_err(|_| {
        invalid_parameter("HF shared-lease refund member count exceeds the supported u64 domain")
    })?;
    let base_refund = divide_quantity_by_member_count(
        join_fee,
        existing_members.len(),
        "failed to divide an HF shared-lease join refund",
    )?;
    let distributed_base = base_refund
        .try_mul_decimal(&Numeric::from(member_count))
        .map_err(|error| {
            invalid_quantity_arithmetic(
                "HF shared-lease base refund aggregation exceeded the decimal domain",
                error,
            )
        })?;
    let mut residual = join_fee.checked_sub(&distributed_base).map_err(|error| {
        invalid_quantity_arithmetic(
            "HF shared-lease refund residual calculation underflowed",
            error,
        )
    })?;
    let refund_quantum = Quantity::try_from_numeric(Numeric::new(1_i128, join_fee.scale().max(9)))
        .map_err(|error| {
            invalid_quantity_arithmetic(
                "failed to construct the HF shared-lease refund quantum",
                error,
            )
        })?;
    for existing_member in existing_members.iter_mut() {
        let refund = if residual >= refund_quantum {
            residual = residual.checked_sub(&refund_quantum).map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease residual refund subtraction underflowed",
                    error,
                )
            })?;
            base_refund.checked_add(&refund_quantum).map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease rounded refund exceeded the decimal domain",
                    error,
                )
            })?
        } else {
            base_refund.clone()
        };
        transfer_hf_shared_lease_amount(
            authority,
            lease_asset_definition_id,
            &refund,
            &existing_member.account_id,
            state_transaction,
        )?;
        if storage_refund {
            existing_member.total_refunded = existing_member
                .total_refunded
                .checked_add(&refund)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF shared-lease storage refund total exceeded the decimal domain",
                        error,
                    )
                })?;
        } else {
            existing_member.total_compute_refunded = existing_member
                .total_compute_refunded
                .checked_add(&refund)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF shared-lease compute refund total exceeded the decimal domain",
                        error,
                    )
                })?;
        }
        existing_member.updated_at_ms = now_ms;
        record_hf_shared_lease_member(state_transaction, existing_member.clone())?;
    }
    if !residual.is_zero() {
        return Err(InstructionExecutionError::InvariantViolation(
            "HF shared-lease refund split did not distribute the exact input quantity".into(),
        ));
    }
    Ok(())
}
fn resolve_fee_sink_account(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AccountId, InstructionExecutionError> {
    crate::block::parse_account_literal_with_world(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.fees.fee_sink_account_id,
        state_transaction.block_unix_timestamp_ms(),
    )
    .ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "invalid nexus.fees.fee_sink_account_id; expected canonical I105 account id or on-chain alias"
                .into(),
        )
    })
}
fn transfer_hf_shared_lease_amount(
    authority: &AccountId,
    lease_asset_definition_id: &AssetDefinitionId,
    amount: &Quantity,
    destination: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if amount.is_zero() || authority == destination {
        return Ok(());
    }
    let source_asset_id = AssetId::new(lease_asset_definition_id.clone(), authority.clone());
    iroha_data_model::isi::Transfer::<
        Asset,
        Quantity,
        iroha_data_model::account::Account,
    >::asset_quantity(
        source_asset_id,
        amount.clone(),
        destination.clone(),
    )
    .execute(authority, state_transaction)
}
fn resolve_fee_asset_definition_id(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AssetDefinitionId, InstructionExecutionError> {
    crate::block::parse_asset_definition_literal_with_world(
        &state_transaction.world,
        &state_transaction.nexus.fees.fee_asset_id,
        state_transaction.block_unix_timestamp_ms(),
    )
    .ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "invalid nexus.fees.fee_asset_id; expected canonical Base58 asset definition id or active asset alias"
                .into(),
        )
    })
}
fn resolve_agent_asset_definition_literal(
    state_transaction: &StateTransaction<'_, '_>,
    literal: &str,
) -> Result<AssetDefinitionId, InstructionExecutionError> {
    crate::block::parse_asset_definition_literal_with_world(
        &state_transaction.world,
        literal,
        state_transaction.block_unix_timestamp_ms(),
    )
    .ok_or_else(|| {
        invalid_parameter(
            "asset_definition must be a canonical Base58 asset definition id or active asset alias",
        )
    })
}
fn agent_spend_limit_for_asset_definition<'a>(
    state_transaction: &StateTransaction<'_, '_>,
    record: &'a SoraAgentApartmentRecordV1,
    canonical_asset_definition: &str,
) -> Result<&'a iroha_data_model::soracloud::AgentSpendLimitV1, InstructionExecutionError> {
    record
        .manifest
        .spend_limits
        .iter()
        .find(|limit| {
            resolve_agent_asset_definition_literal(state_transaction, &limit.asset_definition)
                .ok()
                .is_some_and(|definition_id| definition_id.to_string() == canonical_asset_definition)
        })
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{}` has no spend limit configured for asset `{canonical_asset_definition}`",
                    record.manifest.apartment_name
                )
                .into(),
            )
        })
}
fn transfer_uploaded_model_amount(
    authority: &AccountId,
    amount: &Quantity,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if amount.is_zero() {
        return Ok(());
    }
    let fee_asset_definition_id = resolve_fee_asset_definition_id(state_transaction)?;
    let sink_account = resolve_fee_sink_account(state_transaction)?;
    transfer_hf_shared_lease_amount(
        authority,
        &fee_asset_definition_id,
        amount,
        &sink_account,
        state_transaction,
    )
}
fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn canonical_hf_member_order(
    state_transaction: &StateTransaction<'_, '_>,
    pool_id: &Hash,
) -> Vec<SoraHfSharedLeaseMemberV1> {
    let mut members = state_transaction
        .world
        .soracloud_hf_shared_lease_members
        .iter()
        .filter_map(|((_member_pool_id, _account_id), record)| {
            (record.pool_id == *pool_id && record.status == SoraHfSharedLeaseMemberStatusV1::Active)
                .then_some(record.clone())
        })
        .collect::<Vec<_>>();
    members.sort_by(|left, right| {
        left.joined_at_ms
            .cmp(&right.joined_at_ms)
            .then_with(|| left.account_id.cmp(&right.account_id))
    });
    members
}
fn expire_hf_shared_lease_members(
    state_transaction: &mut StateTransaction<'_, '_>,
    pool_id: &Hash,
    updated_at_ms: u64,
) -> Result<(), InstructionExecutionError> {
    expire_hf_shared_lease_members_except(state_transaction, pool_id, None, updated_at_ms)
}
fn expire_hf_shared_lease_members_except(
    state_transaction: &mut StateTransaction<'_, '_>,
    pool_id: &Hash,
    keep_account_id: Option<&AccountId>,
    updated_at_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let members = canonical_hf_member_order(state_transaction, pool_id);
    for mut member in members {
        if keep_account_id.is_some_and(|keep_account_id| member.account_id == *keep_account_id) {
            continue;
        }
        member.status = SoraHfSharedLeaseMemberStatusV1::Left;
        member.updated_at_ms = updated_at_ms;
        member.last_charge = Quantity::zero();
        member.last_compute_charge = Quantity::zero();
        record_hf_shared_lease_member(state_transaction, member)?;
    }
    Ok(())
}
fn promote_hf_shared_lease_queued_window(
    state_transaction: &mut StateTransaction<'_, '_>,
    pool: &mut SoraHfSharedLeasePoolV1,
    source_record: &mut SoraHfSourceRecordV1,
    now_ms: u64,
) -> Result<bool, InstructionExecutionError> {
    let Some(next_window) = pool.queued_next_window.clone() else {
        return Ok(false);
    };
    expire_hf_shared_lease_members_except(
        state_transaction,
        &pool.pool_id,
        Some(&next_window.sponsor_account_id),
        next_window.window_started_at_ms,
    )?;
    let sponsor_key = (
        pool.pool_id.to_string(),
        next_window.sponsor_account_id.to_string(),
    );
    let mut sponsor_member = state_transaction
        .world
        .soracloud_hf_shared_lease_members
        .get(&sponsor_key)
        .cloned()
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "queued next-window sponsor `{}` is missing from hf shared lease pool `{}`",
                    next_window.sponsor_account_id, pool.pool_id
                )
                .into(),
            )
        })?;
    if sponsor_member.status != SoraHfSharedLeaseMemberStatusV1::Active {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "queued next-window sponsor `{}` is not active in hf shared lease pool `{}`",
                next_window.sponsor_account_id, pool.pool_id
            )
            .into(),
        ));
    }
    sponsor_member.joined_at_ms = next_window.window_started_at_ms;
    sponsor_member.updated_at_ms = now_ms;
    sponsor_member.last_charge = Quantity::zero();
    sponsor_member.last_compute_charge = Quantity::zero();
    bind_hf_shared_lease_targets(
        &mut sponsor_member,
        &next_window.service_name,
        next_window.apartment_name.as_ref(),
    );
    record_hf_shared_lease_member(state_transaction, sponsor_member)?;
    source_record.model_name = next_window.model_name.clone();
    source_record.updated_at_ms = now_ms;
    if source_record.status == SoraHfSourceStatusV1::Retired {
        source_record.status = SoraHfSourceStatusV1::PendingImport;
    }
    refresh_hf_source_status_from_generated_service(
        state_transaction,
        source_record,
        &next_window.service_name,
    );
    record_hf_source(state_transaction, source_record.clone())?;
    pool.lease_asset_definition_id = next_window.lease_asset_definition_id;
    pool.base_fee = next_window.base_fee.clone();
    pool.window_started_at_ms = next_window.window_started_at_ms;
    pool.window_expires_at_ms = next_window.window_expires_at_ms;
    pool.active_member_count = 1;
    pool.status = SoraHfSharedLeaseStatusV1::Active;
    pool.queued_next_window = None;
    record_hf_shared_lease_pool(state_transaction, pool.clone())?;
    record_hf_placement(state_transaction, next_window.planned_placement)?;
    reconcile_expired_model_hosts(state_transaction, now_ms)?;
    Ok(true)
}
fn bind_hf_shared_lease_targets(
    member: &mut SoraHfSharedLeaseMemberV1,
    service_name: &Name,
    apartment_name: Option<&Name>,
) {
    member.service_bindings.insert(service_name.to_string());
    if let Some(apartment_name) = apartment_name {
        member.apartment_bindings.insert(apartment_name.to_string());
    }
}
fn refresh_hf_source_status_from_generated_service(
    state_transaction: &StateTransaction<'_, '_>,
    source_record: &mut SoraHfSourceRecordV1,
    service_name: &Name,
) {
    if matches!(
        source_record.status,
        SoraHfSourceStatusV1::Ready | SoraHfSourceStatusV1::Failed | SoraHfSourceStatusV1::Retired
    ) {
        return;
    }
    let Ok((_deployment, bundle)) = load_active_bundle(state_transaction, service_name) else {
        return;
    };
    let Some(binding) = soracloud_hf_generated_source_binding(&bundle) else {
        return;
    };
    if binding.repo_id == source_record.repo_id
        && binding.resolved_revision == source_record.resolved_revision
        && binding.model_name == source_record.model_name
    {
        source_record.status = SoraHfSourceStatusV1::Ready;
        source_record.last_error = None;
    }
}
fn verify_hf_shared_lease_join_provenance(
    authority: &AccountId,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
    service_name: &Name,
    apartment_name: Option<&Name>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: &AssetDefinitionId,
    base_fee: &Quantity,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_hf_shared_lease_join_provenance_payload(
        repo_id,
        resolved_revision,
        model_name,
        service_name.as_ref(),
        apartment_name.map(Name::as_ref),
        storage_class,
        lease_term_ms,
        lease_asset_definition_id,
        base_fee,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode hf shared lease join provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "hf shared lease join provenance signer must match the transaction authority",
        "hf shared lease join provenance signature verification failed",
    )
}
fn verify_hf_shared_lease_leave_provenance(
    authority: &AccountId,
    repo_id: &str,
    resolved_revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    service_name: Option<&Name>,
    apartment_name: Option<&Name>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_hf_shared_lease_leave_provenance_payload(
        repo_id,
        resolved_revision,
        storage_class,
        lease_term_ms,
        service_name.map(Name::as_ref),
        apartment_name.map(Name::as_ref),
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode hf shared lease leave provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "hf shared lease leave provenance signer must match the transaction authority",
        "hf shared lease leave provenance signature verification failed",
    )
}
fn verify_hf_shared_lease_renew_provenance(
    authority: &AccountId,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
    service_name: &Name,
    apartment_name: Option<&Name>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: &AssetDefinitionId,
    base_fee: &Quantity,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_hf_shared_lease_renew_provenance_payload(
        repo_id,
        resolved_revision,
        model_name,
        service_name.as_ref(),
        apartment_name.map(Name::as_ref),
        storage_class,
        lease_term_ms,
        lease_asset_definition_id,
        base_fee,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode hf shared lease renew provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "hf shared lease renew provenance signer must match the transaction authority",
        "hf shared lease renew provenance signature verification failed",
    )
}
fn verify_model_host_advertise_provenance(
    authority: &AccountId,
    capability: &SoraModelHostCapabilityRecordV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_model_host_advertise_provenance_payload(capability).map_err(|err| {
        invalid_parameter(format!(
            "failed to encode model host advertise provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "model host advertise provenance signer must match the transaction authority",
        "model host advertise provenance signature verification failed",
    )
}
fn verify_model_host_heartbeat_provenance(
    authority: &AccountId,
    validator_account_id: &AccountId,
    heartbeat_expires_at_ms: u64,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_model_host_heartbeat_provenance_payload(
        validator_account_id,
        heartbeat_expires_at_ms,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode model host heartbeat provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "model host heartbeat provenance signer must match the transaction authority",
        "model host heartbeat provenance signature verification failed",
    )
}
fn verify_model_host_withdraw_provenance(
    authority: &AccountId,
    validator_account_id: &AccountId,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload =
        encode_model_host_withdraw_provenance_payload(validator_account_id).map_err(|err| {
            invalid_parameter(format!(
                "failed to encode model host withdraw provenance: {err}"
            ))
        })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "model host withdraw provenance signer must match the transaction authority",
        "model host withdraw provenance signature verification failed",
    )
}
fn verify_inrou_host_advertise_provenance(
    authority: &AccountId,
    capability: &SoraInrouHostCapabilityRecordV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload = encode_inrou_host_advertise_provenance_payload(capability).map_err(|err| {
        invalid_parameter(format!(
            "failed to encode Inrou host advertise provenance: {err}"
        ))
    })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "Inrou host advertise provenance signer must match the transaction authority",
        "Inrou host advertise provenance signature verification failed",
    )
}
fn verify_inrou_host_withdraw_provenance(
    authority: &AccountId,
    validator_account_id: &AccountId,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    let payload =
        encode_inrou_host_withdraw_provenance_payload(validator_account_id).map_err(|err| {
            invalid_parameter(format!(
                "failed to encode Inrou host withdraw provenance: {err}"
            ))
        })?;
    verify_provenance_payload(
        authority,
        provenance,
        payload,
        "Inrou host withdraw provenance signer must match the transaction authority",
        "Inrou host withdraw provenance signature verification failed",
    )
}
fn record_agent_apartment(
    state_transaction: &mut StateTransaction<'_, '_>,
    apartment_name: String,
    record: SoraAgentApartmentRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_agent_apartments
        .insert(apartment_name, record);
    Ok(())
}
fn record_agent_apartment_audit_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: SoraAgentApartmentAuditEventV1,
) -> Result<(), InstructionExecutionError> {
    event
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_agent_apartment_audit_events
        .insert(event.sequence, event);
    Ok(())
}
fn binding_state_totals(
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
) -> (u64, u32) {
    let total_bytes = state_transaction
        .world
        .soracloud_service_state_entries
        .iter()
        .filter(|((stored_service, stored_binding, _state_key), _entry)| {
            stored_service == service_name.as_ref() && stored_binding == binding_name.as_ref()
        })
        .map(|(_key, entry)| entry.payload_bytes.get())
        .fold(0u64, u64::saturating_add);
    let key_count = u32::try_from(
        state_transaction
            .world
            .soracloud_service_state_entries
            .iter()
            .filter(|((stored_service, stored_binding, _state_key), _entry)| {
                stored_service == service_name.as_ref() && stored_binding == binding_name.as_ref()
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    (total_bytes, key_count)
}
const REGISTERED_SORACLOUD_BFV_BACKEND: &str = "fhe/bfv-rns/v1";
fn u64_bit_width(value: u64) -> u16 {
    (u64::BITS - value.leading_zeros()) as u16
}
fn validate_registered_soracloud_bfv_descriptor(
    param_set: &FheParamSetV1,
    params: &BfvParameters,
) -> Result<(), InstructionExecutionError> {
    if param_set.backend != REGISTERED_SORACLOUD_BFV_BACKEND {
        return Err(invalid_parameter(
            "fhe parameter-set backend does not match the registered BFV profile",
        ));
    }
    let expected_degree = u32::from(params.polynomial_degree);
    if param_set.polynomial_modulus_degree.get() != expected_degree {
        return Err(invalid_parameter(
            "fhe parameter-set polynomial degree does not match the registered BFV profile",
        ));
    }
    if param_set.slot_count.get() != expected_degree {
        return Err(invalid_parameter(
            "fhe parameter-set slot count does not match the registered BFV profile",
        ));
    }
    let plaintext_bits = u64_bit_width(params.plaintext_modulus);
    if param_set.plaintext_modulus_bits.get() != plaintext_bits {
        return Err(invalid_parameter(
            "fhe parameter-set plaintext modulus bits do not match the registered BFV profile",
        ));
    }
    let ciphertext_bits = u64_bit_width(params.ciphertext_modulus);
    let largest_declared_limb = param_set
        .ciphertext_modulus_bits
        .first()
        .ok_or_else(|| {
            invalid_parameter(
                "fhe parameter-set ciphertext modulus bits must declare at least one limb",
            )
        })?
        .get();
    if largest_declared_limb > ciphertext_bits {
        return Err(invalid_parameter(
            "fhe parameter-set ciphertext modulus bits exceed the registered BFV profile",
        ));
    }
    let declared_chain_bits = param_set
        .ciphertext_modulus_bits
        .iter()
        .map(|bits| u32::from(bits.get()))
        .sum::<u32>();
    if declared_chain_bits < u32::from(ciphertext_bits) {
        return Err(invalid_parameter(
            "fhe parameter-set ciphertext modulus chain under-declares the registered BFV profile",
        ));
    }
    let rns_modulus_chain_digest = registered_bfv_rns_modulus_chain_digest(params)
        .map_err(|err| invalid_parameter(format!("failed to digest BFV RNS chain: {err}")))?;
    if param_set.rns_modulus_chain_digest != rns_modulus_chain_digest {
        return Err(invalid_parameter(
            "fhe parameter-set RNS modulus-chain digest does not match the registered BFV profile",
        ));
    }
    let key_switch_decomposition_chain_digest =
        registered_bfv_key_switch_decomposition_chain_digest(params).map_err(|err| {
            invalid_parameter(format!(
                "failed to digest BFV key-switch decomposition chain: {err}"
            ))
        })?;
    if param_set.key_switch_decomposition_chain_digest != key_switch_decomposition_chain_digest {
        return Err(invalid_parameter(
            "fhe parameter-set key-switch decomposition-chain digest does not match the registered BFV profile",
        ));
    }
    Ok(())
}
fn registered_soracloud_bfv_parameters(
    param_set: &FheParamSetV1,
) -> Result<BfvParameters, InstructionExecutionError> {
    param_set
        .validate()
        .map_err(|err| invalid_parameter(format!("invalid FHE parameter set: {err}")))?;
    if param_set.scheme != FheSchemeV1::Bfv {
        return Err(invalid_parameter(
            "Soracloud FHE jobs currently require the registered BFV parameter profile",
        ));
    }
    let params = ram_lfe_bfv_parameters_v1();
    validate_registered_bfv_parameters(&params)
        .map_err(|err| invalid_parameter(format!("invalid registered BFV parameters: {err}")))?;
    let parameter_digest = registered_bfv_parameter_digest(&params)
        .map_err(|err| invalid_parameter(format!("failed to digest BFV parameters: {err}")))?;
    if param_set.parameter_digest != parameter_digest {
        return Err(invalid_parameter(
            "fhe parameter-set digest does not match the registered BFV profile",
        ));
    }
    validate_registered_soracloud_bfv_descriptor(param_set, &params)?;
    Ok(params)
}
fn decode_soracloud_fhe_envelope(
    payload: &[u8],
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    norito::decode_canonical::<BfvIdentifierCiphertext>(payload)
        .map_err(|err| invalid_parameter(format!("invalid FHE ciphertext envelope: {err}")))
}
fn encode_soracloud_fhe_output_payload(
    output: &BfvIdentifierCiphertext,
) -> Result<Vec<u8>, InstructionExecutionError> {
    norito::encode_canonical(output)
        .map_err(|err| invalid_parameter(format!("failed to encode FHE output: {err}")))
}
struct LoadedSoracloudFheInput {
    envelope: BfvIdentifierCiphertext,
    bound: u128,
}
fn load_soracloud_fhe_inputs(
    params: &BfvParameters,
    state_transaction: &StateTransaction<'_, '_>,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    job: &FheJobSpecV1,
    public_key: &BfvPublicKey,
    public_key_digest: Hash,
    required_bound_mode: BfvCiphertextBoundModeV1,
) -> Result<Vec<LoadedSoracloudFheInput>, InstructionExecutionError> {
    let mut inputs = Vec::with_capacity(job.inputs.len());
    for input in &job.inputs {
        let state_entry_key = (
            service_name.as_ref().to_owned(),
            binding_name.as_ref().to_owned(),
            input.state_key.clone(),
        );
        let entry = state_transaction
            .world
            .soracloud_service_state_entries
            .get(&state_entry_key)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("fhe input state key `{}` is not present", input.state_key).into(),
                )
            })?;
        if entry.encryption != SoraStateEncryptionV1::FheCiphertext {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("fhe input `{}` is not an FHE ciphertext", input.state_key).into(),
            ));
        }
        if entry.payload_bytes != input.payload_bytes {
            return Err(invalid_parameter(format!(
                "fhe input `{}` payload size mismatch",
                input.state_key
            )));
        }
        if entry.payload_commitment != input.commitment {
            return Err(invalid_parameter(format!(
                "fhe input `{}` commitment mismatch",
                input.state_key
            )));
        }
        let bound = entry
            .fhe_residual_multiple_bound
            .ok_or_else(|| match required_bound_mode {
                BfvCiphertextBoundModeV1::ExactResidualMultiple => invalid_parameter(format!(
                    "fhe input `{}` is missing exact BFV residual metadata",
                    input.state_key
                )),
                BfvCiphertextBoundModeV1::BoundedNoise => invalid_parameter(format!(
                    "fhe input `{}` is missing bounded BFV noise metadata",
                    input.state_key
                )),
            })?;
        let bound_mode = entry
            .fhe_bound_mode
            .ok_or_else(|| match required_bound_mode {
                BfvCiphertextBoundModeV1::ExactResidualMultiple => invalid_parameter(format!(
                    "fhe input `{}` is missing exact BFV bound-mode metadata",
                    input.state_key
                )),
                BfvCiphertextBoundModeV1::BoundedNoise => invalid_parameter(format!(
                    "fhe input `{}` is missing bounded BFV bound-mode metadata",
                    input.state_key
                )),
            })?;
        if bound_mode != required_bound_mode {
            return Err(match required_bound_mode {
                BfvCiphertextBoundModeV1::ExactResidualMultiple => invalid_parameter(format!(
                    "fhe input `{}` is not annotated with exact BFV residual metadata",
                    input.state_key
                )),
                BfvCiphertextBoundModeV1::BoundedNoise => invalid_parameter(format!(
                    "fhe input `{}` is not annotated with bounded BFV noise metadata",
                    input.state_key
                )),
            });
        }
        match required_bound_mode {
            BfvCiphertextBoundModeV1::ExactResidualMultiple => {
                validate_bfv_exact_residual_multiple_capacity(
                    params,
                    bound,
                    "Soracloud FHE input residual metadata",
                )
                .map_err(|err| {
                    invalid_parameter(format!(
                        "fhe input `{}` residual metadata exceeds BFV capacity: {err}",
                        input.state_key
                    ))
                })?;
            }
            BfvCiphertextBoundModeV1::BoundedNoise => {
                validate_bfv_bounded_noise_bound(
                    params,
                    bound,
                    "Soracloud FHE input bounded-noise metadata",
                )
                .map_err(|err| {
                    invalid_parameter(format!(
                        "fhe input `{}` bounded-noise metadata exceeds BFV capacity: {err}",
                        input.state_key
                    ))
                })?;
            }
        }
        let entry_public_key_digest = entry.fhe_public_key_digest.ok_or_else(|| {
            invalid_parameter(format!(
                "fhe input `{}` is missing FHE public-key digest metadata",
                input.state_key
            ))
        })?;
        if entry_public_key_digest != public_key_digest {
            return Err(invalid_parameter(format!(
                "fhe input `{}` public-key digest mismatch",
                input.state_key
            )));
        }
        let envelope = decode_soracloud_fhe_envelope(&entry.payload)?;
        let context = format!("fhe input `{}`", input.state_key);
        validate_soracloud_fhe_envelope_shape(params, &envelope, &context)?;
        derive_soracloud_fhe_input_ciphertext_statement_digests(
            params,
            public_key,
            &envelope,
            bound,
            required_bound_mode,
            &context,
        )?;
        inputs.push(LoadedSoracloudFheInput { envelope, bound });
    }
    Ok(inputs)
}
fn first_matching_fhe_slots(
    envelopes: &[BfvIdentifierCiphertext],
) -> Result<&[BfvCiphertext], InstructionExecutionError> {
    let first = envelopes
        .first()
        .ok_or_else(|| invalid_parameter("fhe job requires at least one input envelope"))?;
    let first_slots = first.slots.len();
    if first_slots == 0 {
        return Err(invalid_parameter(
            "fhe ciphertext envelope must contain at least one slot",
        ));
    }
    if envelopes
        .iter()
        .any(|envelope| envelope.slots.len() != first_slots)
    {
        return Err(invalid_parameter(
            "fhe ciphertext envelopes must have matching slot counts",
        ));
    }
    Ok(&first.slots)
}
fn ensure_matching_fhe_slots(
    envelopes: &[BfvIdentifierCiphertext],
) -> Result<usize, InstructionExecutionError> {
    Ok(first_matching_fhe_slots(envelopes)?.len())
}
fn fold_fhe_slots(
    params: &BfvParameters,
    inputs: &[BfvIdentifierCiphertext],
    mut combine: impl FnMut(
        &BfvCiphertext,
        &BfvCiphertext,
    ) -> Result<BfvCiphertext, InstructionExecutionError>,
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    let mut slots = first_matching_fhe_slots(inputs)?.to_vec();
    for envelope in &inputs[1..] {
        for (slot, rhs) in slots.iter_mut().zip(&envelope.slots) {
            *slot = combine(slot, rhs)?;
        }
    }
    for slot in &slots {
        multiply_plain_scalar(params, slot, 1)
            .map_err(|err| invalid_parameter(format!("invalid FHE ciphertext slot: {err}")))?;
    }
    Ok(BfvIdentifierCiphertext { slots })
}
fn fold_fhe_slots_balanced(
    params: &BfvParameters,
    inputs: &[BfvIdentifierCiphertext],
    mut combine: impl FnMut(
        &BfvCiphertext,
        &BfvCiphertext,
    ) -> Result<BfvCiphertext, InstructionExecutionError>,
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    let slot_count = ensure_matching_fhe_slots(inputs)?;
    if inputs.len() < 2 {
        return Err(invalid_parameter(
            "fhe balanced fold requires at least two input envelopes",
        ));
    }
    let mut output_slots = Vec::with_capacity(slot_count);
    for slot_index in 0..slot_count {
        let mut level = inputs
            .iter()
            .map(|envelope| envelope.slots[slot_index].clone())
            .collect::<Vec<_>>();
        while level.len() > 1 {
            let mut next_level = Vec::with_capacity(level.len().div_ceil(2));
            for pair in level.chunks(2) {
                let combined = match pair {
                    [lhs, rhs] => combine(lhs, rhs)?,
                    [single] => single.clone(),
                    _ => unreachable!("chunks(2) never yields an empty slice"),
                };
                next_level.push(combined);
            }
            level = next_level;
        }
        output_slots.push(level.pop().ok_or_else(|| {
            invalid_parameter("fhe balanced fold produced no output for a non-empty slot")
        })?);
    }
    for slot in &output_slots {
        multiply_plain_scalar(params, slot, 1)
            .map_err(|err| invalid_parameter(format!("invalid FHE ciphertext slot: {err}")))?;
    }
    Ok(BfvIdentifierCiphertext {
        slots: output_slots,
    })
}
fn validate_soracloud_fhe_evaluation_budget(
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
) -> Result<(), InstructionExecutionError> {
    let budget = BfvEvaluationBudget::exact_evaluator_v1();
    let plan = match job.operation {
        FheJobOperationV1::Add => {
            if job.requested_multiplication_depth != 0 {
                return Err(invalid_parameter("add operation must use depth 0"));
            }
            if job.rotation_steps != 0 || job.bootstrap_count != 0 {
                return Err(invalid_parameter(
                    "add operation cannot request rotation/bootstrap",
                ));
            }
            BfvEvaluationPlan::add(inputs.len())
                .map_err(|err| invalid_parameter(format!("invalid FHE add plan: {err}")))?
        }
        FheJobOperationV1::Multiply => {
            if job.requested_multiplication_depth == 0 {
                return Err(invalid_parameter(
                    "multiply operation requires non-zero depth",
                ));
            }
            if job.rotation_steps != 0 || job.bootstrap_count != 0 {
                return Err(invalid_parameter(
                    "multiply operation cannot request rotation/bootstrap",
                ));
            }
            let plan = BfvEvaluationPlan::balanced_multiply(inputs.len())
                .map_err(|err| invalid_parameter(format!("invalid FHE multiply plan: {err}")))?;
            if job.requested_multiplication_depth < plan.ciphertext_multiplication_depth {
                return Err(invalid_parameter(format!(
                    "requested_multiplication_depth {} under-declares balanced BFV multiplication depth {}",
                    job.requested_multiplication_depth, plan.ciphertext_multiplication_depth
                )));
            }
            plan
        }
        FheJobOperationV1::RotateLeft => {
            if job.rotation_steps == 0 {
                return Err(invalid_parameter(
                    "rotate operation requires non-zero rotation_steps",
                ));
            }
            if job.requested_multiplication_depth != 0 || job.bootstrap_count != 0 {
                return Err(invalid_parameter(
                    "rotate operation cannot request depth/bootstrap",
                ));
            }
            BfvEvaluationPlan::rotate_left(inputs.len())
                .map_err(|err| invalid_parameter(format!("invalid FHE rotate plan: {err}")))?
        }
        FheJobOperationV1::Bootstrap => {
            if job.bootstrap_count == 0 {
                return Err(invalid_parameter(
                    "bootstrap operation requires non-zero bootstrap_count",
                ));
            }
            if job.requested_multiplication_depth != 0 || job.rotation_steps != 0 {
                return Err(invalid_parameter(
                    "bootstrap operation cannot request depth/rotation",
                ));
            }
            BfvEvaluationPlan::bootstrap_refresh(inputs.len(), job.bootstrap_count)
                .map_err(|err| invalid_parameter(format!("invalid FHE bootstrap plan: {err}")))?
        }
    };
    budget
        .validate_plan(plan)
        .map_err(|err| invalid_parameter(format!("FHE evaluation budget exceeded: {err}")))
}
fn validate_soracloud_fhe_full_bootstrap_single_count(
    job: &FheJobSpecV1,
) -> Result<(), InstructionExecutionError> {
    if job.bootstrap_count != 1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap operation requires bootstrap_count exactly 1",
        ));
    }
    Ok(())
}
fn preflight_soracloud_fhe_full_bootstrap_execution_proofs(
    job: &FheJobSpecV1,
    evaluation_keys: &BfvEvaluationKeyBundle,
    input_envelopes: &[BfvIdentifierCiphertext],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    proofs: &[SoracloudFheFullBootstrapExecutionProofV1],
) -> Result<(), InstructionExecutionError> {
    let is_bootstrap_job = job.operation == FheJobOperationV1::Bootstrap && job.bootstrap_count > 0;
    let Some(bootstrap_key) = evaluation_keys.bootstrap_key.as_ref() else {
        if full_bootstrap_circuit_artifacts.is_some() || !proofs.is_empty() {
            return Err(invalid_parameter(
                "FHE full-bootstrap execution proof material is only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    };
    if !is_bootstrap_job || bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        if full_bootstrap_circuit_artifacts.is_some() || !proofs.is_empty() {
            return Err(invalid_parameter(
                "FHE full-bootstrap execution proof material is only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    }
    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
    full_bootstrap_circuit_artifacts.ok_or_else(|| {
        invalid_parameter(
            "FHE full-bootstrap execution proof requires full-bootstrap circuit artifacts",
        )
    })?;
    validate_soracloud_fhe_evaluation_budget(job, input_envelopes)?;
    let input_slot_count = first_matching_fhe_slots(input_envelopes)?.len();
    if proofs.len() != input_slot_count {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof count must match input/output slot count",
        ));
    }
    Ok(())
}
#[derive(Clone, Copy, Debug)]
struct BfvFullBootstrapReleaseAuditRuntimeContext<'a> {
    package: &'a BfvFullBootstrapReleaseAuditPackageV1,
    expected_package_digest: Hash,
    trusted_reviewer_id: &'a str,
    trusted_reviewer_public_key: &'a PublicKey,
}
const FHE_FULL_BOOTSTRAP_RELEASE_AUDIT_CONTEXT_REQUIRED: &str =
    "FHE full-bootstrap artifact execution requires release audit package context";
fn full_bootstrap_release_audit_context_required_error() -> iroha_crypto::fhe_bfv::BfvError {
    iroha_crypto::fhe_bfv::BfvError::InvalidParameters(
        FHE_FULL_BOOTSTRAP_RELEASE_AUDIT_CONTEXT_REQUIRED.to_owned(),
    )
}
fn soracloud_fhe_full_bootstrap_release_audit_runtime_context<'a>(
    params: &BfvParameters,
    evaluation_keys: &'a BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    full_bootstrap_circuit_artifacts: Option<&'a BfvFullBootstrapCircuitArtifactBundleV1>,
    policy: &'a FheExecutionPolicyV1,
) -> Result<Option<BfvFullBootstrapReleaseAuditRuntimeContext<'a>>, InstructionExecutionError> {
    let is_bootstrap_job = job.operation == FheJobOperationV1::Bootstrap && job.bootstrap_count > 0;
    let Some(bootstrap_key) = evaluation_keys.bootstrap_key.as_ref() else {
        return Ok(None);
    };
    if !is_bootstrap_job || bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        return Ok(None);
    }
    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
    let material = bootstrap_key
        .full_bootstrap_material
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter("FHE full-bootstrap release audit requires governed material")
        })?;
    let artifacts = full_bootstrap_circuit_artifacts.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap release audit requires circuit artifacts")
    })?;
    let package = policy
        .full_bootstrap_release_audit_package
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter("FHE full-bootstrap execution policy requires release audit package")
        })?;
    let expected_package_digest = policy
        .full_bootstrap_release_audit_package_digest
        .ok_or_else(|| {
            invalid_parameter(
                "FHE full-bootstrap execution policy requires release audit package digest",
            )
        })?;
    let trusted_reviewer_id = policy
        .full_bootstrap_release_audit_trusted_reviewer_id
        .as_deref()
        .ok_or_else(|| {
            invalid_parameter(
                "FHE full-bootstrap execution policy requires trusted release audit reviewer id",
            )
        })?;
    let trusted_reviewer_public_key = policy
        .full_bootstrap_release_audit_trusted_reviewer_public_key
        .as_ref()
        .ok_or_else(|| {
            invalid_parameter(
                "FHE full-bootstrap execution policy requires trusted release audit reviewer public key",
            )
        })?;
    validate_bfv_full_bootstrap_release_audit_trusted_reviewer_id_v1(trusted_reviewer_id)
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution policy trusted release audit reviewer id failed validation: {err}"
            ))
        })?;
    validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1(
        trusted_reviewer_public_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution policy trusted release audit reviewer public key failed validation: {err}"
        ))
    })?;
    validate_bfv_full_bootstrap_release_audit_package_for_artifacts_trusted_reviewer_and_digest_v1(
        params,
        material,
        artifacts,
        package,
        expected_package_digest,
        trusted_reviewer_id,
        trusted_reviewer_public_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap release audit package failed validation: {err}"
        ))
    })?;
    Ok(Some(BfvFullBootstrapReleaseAuditRuntimeContext {
        package,
        expected_package_digest,
        trusted_reviewer_id,
        trusted_reviewer_public_key,
    }))
}
fn soracloud_fhe_job_output_residual_multiple_bound(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_residual_bounds: &[u128],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<Option<u128>, InstructionExecutionError> {
    if inputs.len() != input_residual_bounds.len() {
        return Err(invalid_parameter(
            "fhe residual metadata must match input envelope count",
        ));
    }
    match job.operation {
        FheJobOperationV1::Add => {
            bfv_add_output_residual_multiple_bound(params, input_residual_bounds)
                .map(Some)
                .map_err(|err| invalid_parameter(format!("FHE add residual bound exceeded: {err}")))
        }
        FheJobOperationV1::Multiply => {
            for (index, &input_bound) in input_residual_bounds.iter().enumerate() {
                validate_bfv_exact_residual_multiple_capacity(
                    params,
                    input_bound,
                    &format!("FHE multiply input[{index}] residual bound"),
                )
                .map_err(|err| {
                    invalid_parameter(format!("FHE multiply residual bound exceeded: {err}"))
                })?;
            }
            if input_residual_bounds.len() < 2 {
                return Err(invalid_parameter(
                    "fhe multiply residual metadata requires at least two input bounds",
                ));
            }
            let mut level = input_residual_bounds.to_vec();
            while level.len() > 1 {
                let mut next_level = Vec::with_capacity(level.len().div_ceil(2));
                for pair in level.chunks(2) {
                    let combined = match pair {
                        [lhs_bound, rhs_bound] => bfv_multiply_output_residual_multiple_bound(
                            params,
                            &evaluation_keys.relinearization_key,
                            *lhs_bound,
                            *rhs_bound,
                        )
                        .map_err(|err| {
                            invalid_parameter(format!(
                                "FHE multiply residual bound exceeded: {err}"
                            ))
                        })?,
                        [single] => *single,
                        _ => unreachable!("chunks(2) never yields an empty slice"),
                    };
                    next_level.push(combined);
                }
                level = next_level;
            }
            Ok(level.pop())
        }
        FheJobOperationV1::RotateLeft => {
            let input_bound = *input_residual_bounds
                .first()
                .ok_or_else(|| invalid_parameter("fhe rotate requires residual metadata"))?;
            let slots = first_matching_fhe_slots(inputs)?;
            if slots.len() == 1 {
                return bfv_packed_rotate_left_output_residual_multiple_bound(
                    params,
                    &evaluation_keys.galois_keys,
                    input_bound,
                    job.rotation_steps,
                )
                .map(Some)
                .map_err(|err| {
                    invalid_parameter(format!("FHE packed rotate residual bound exceeded: {err}"))
                });
            }
            let rotation_key = evaluation_keys
                .rotation_keys
                .iter()
                .find(|key| key.rotation_steps == job.rotation_steps)
                .ok_or_else(|| {
                    invalid_parameter(format!(
                        "missing BFV rotation key for {} steps",
                        job.rotation_steps
                    ))
                })?;
            let slot_bounds = vec![input_bound; slots.len()];
            let output_bounds = bfv_rotate_slots_left_output_residual_multiple_bounds(
                params,
                rotation_key,
                &slot_bounds,
            )
            .map_err(|err| {
                invalid_parameter(format!("FHE rotate residual bound exceeded: {err}"))
            })?;
            Ok(output_bounds.into_iter().max())
        }
        FheJobOperationV1::Bootstrap => {
            let input_bound = *input_residual_bounds
                .first()
                .ok_or_else(|| invalid_parameter("fhe bootstrap requires residual metadata"))?;
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bootstrap residual bound")
            })?;
            let result = match bootstrap_key.mode {
                BfvBootstrapKeyMode::RefreshOnlyV1 => {
                    bfv_bootstrap_key_refresh_output_residual_multiple_bound(
                        params,
                        bootstrap_key,
                        input_bound,
                        job.bootstrap_count,
                    )
                }
                BfvBootstrapKeyMode::FullBootstrapV1 => {
                    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
                    if let Some(artifacts) = full_bootstrap_circuit_artifacts {
                        if let Some(audit) = full_bootstrap_release_audit {
                            bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1(
                                params,
                                bootstrap_key,
                                artifacts,
                                &evaluation_keys.galois_keys,
                                input_bound,
                                audit.package,
                                audit.expected_package_digest,
                                audit.trusted_reviewer_id,
                                audit.trusted_reviewer_public_key,
                            )
                        } else {
                            Err(full_bootstrap_release_audit_context_required_error())
                        }
                    } else {
                        bfv_full_bootstrap_output_residual_multiple_bound_v1(
                            params,
                            bootstrap_key,
                            input_bound,
                        )
                    }
                }
            };
            result.map(Some).map_err(|err| {
                invalid_parameter(format!("FHE bootstrap residual bound exceeded: {err}"))
            })
        }
    }
}
fn soracloud_fhe_job_output_bounded_noise_bound(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_noise_bounds: &[u128],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<Option<u128>, InstructionExecutionError> {
    if inputs.len() != input_noise_bounds.len() {
        return Err(invalid_parameter(
            "fhe bounded-noise metadata must match input envelope count",
        ));
    }
    match job.operation {
        FheJobOperationV1::Add => bfv_add_bounded_noise_output_bound(params, input_noise_bounds)
            .map(Some)
            .map_err(|err| {
                invalid_parameter(format!("FHE add bounded-noise bound exceeded: {err}"))
            }),
        FheJobOperationV1::Multiply => {
            for (index, &input_bound) in input_noise_bounds.iter().enumerate() {
                validate_bfv_bounded_noise_bound(
                    params,
                    input_bound,
                    &format!("FHE multiply input[{index}] bounded-noise bound"),
                )
                .map_err(|err| {
                    invalid_parameter(format!("FHE multiply bounded-noise bound exceeded: {err}"))
                })?;
            }
            if input_noise_bounds.len() < 2 {
                return Err(invalid_parameter(
                    "fhe multiply bounded-noise metadata requires at least two input bounds",
                ));
            }
            let mut level = input_noise_bounds.to_vec();
            while level.len() > 1 {
                let mut next_level = Vec::with_capacity(level.len().div_ceil(2));
                for pair in level.chunks(2) {
                    let combined = match pair {
                        [lhs_bound, rhs_bound] => bfv_multiply_bounded_noise_output_bound(
                            params,
                            &evaluation_keys.relinearization_key,
                            *lhs_bound,
                            *rhs_bound,
                        )
                        .map_err(|err| {
                            invalid_parameter(format!(
                                "FHE multiply bounded-noise bound exceeded: {err}"
                            ))
                        })?,
                        [single] => *single,
                        _ => unreachable!("chunks(2) never yields an empty slice"),
                    };
                    next_level.push(combined);
                }
                level = next_level;
            }
            Ok(level.pop())
        }
        FheJobOperationV1::RotateLeft => {
            let input_bound = *input_noise_bounds
                .first()
                .ok_or_else(|| invalid_parameter("fhe rotate requires bounded-noise metadata"))?;
            let slots = first_matching_fhe_slots(inputs)?;
            if slots.len() == 1 {
                return bfv_packed_rotate_left_bounded_noise_output_bound(
                    params,
                    &evaluation_keys.galois_keys,
                    input_bound,
                    job.rotation_steps,
                )
                .map(Some)
                .map_err(|err| {
                    invalid_parameter(format!(
                        "FHE packed rotate bounded-noise bound exceeded: {err}"
                    ))
                });
            }
            let rotation_key = evaluation_keys
                .rotation_keys
                .iter()
                .find(|key| key.rotation_steps == job.rotation_steps)
                .ok_or_else(|| {
                    invalid_parameter(format!(
                        "missing BFV rotation key for {} steps",
                        job.rotation_steps
                    ))
                })?;
            let slot_bounds = vec![input_bound; slots.len()];
            let output_bounds =
                bfv_rotate_slots_left_bounded_noise_registered_rns_basis_extension_output_bounds(
                    params,
                    rotation_key,
                    &slot_bounds,
                )
                .map_err(|err| {
                    invalid_parameter(format!("FHE rotate bounded-noise bound exceeded: {err}"))
                })?;
            Ok(output_bounds.into_iter().max())
        }
        FheJobOperationV1::Bootstrap => {
            let input_bound = *input_noise_bounds.first().ok_or_else(|| {
                invalid_parameter("fhe bootstrap requires bounded-noise metadata")
            })?;
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bootstrap bounded-noise bound")
            })?;
            let result = match bootstrap_key.mode {
                BfvBootstrapKeyMode::RefreshOnlyV1 => {
                    bfv_bootstrap_key_refresh_bounded_noise_output_bound(
                        params,
                        bootstrap_key,
                        input_bound,
                        job.bootstrap_count,
                    )
                }
                BfvBootstrapKeyMode::FullBootstrapV1 => {
                    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
                    if let Some(artifacts) = full_bootstrap_circuit_artifacts {
                        if let Some(audit) = full_bootstrap_release_audit {
                            bfv_full_bootstrap_with_release_audited_artifacts_bounded_noise_output_bound_v1(
                                params,
                                bootstrap_key,
                                artifacts,
                                &evaluation_keys.galois_keys,
                                input_bound,
                                audit.package,
                                audit.expected_package_digest,
                                audit.trusted_reviewer_id,
                                audit.trusted_reviewer_public_key,
                            )
                        } else {
                            Err(full_bootstrap_release_audit_context_required_error())
                        }
                    } else {
                        bfv_full_bootstrap_bounded_noise_output_bound_v1(
                            params,
                            bootstrap_key,
                            input_bound,
                        )
                    }
                }
            };
            result.map(Some).map_err(|err| {
                invalid_parameter(format!("FHE bootstrap bounded-noise bound exceeded: {err}"))
            })
        }
    }
}
#[cfg(test)]
fn execute_soracloud_fhe_job_with_residual_bounds(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_residual_bounds: &[u128],
) -> Result<(BfvIdentifierCiphertext, Option<u128>), InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    let output_residual_bound = soracloud_fhe_job_output_residual_multiple_bound(
        params,
        evaluation_keys,
        job,
        inputs,
        input_residual_bounds,
        None,
        None,
    )?;
    let output = execute_soracloud_fhe_job(params, evaluation_keys, job, inputs)?;
    Ok((output, output_residual_bound))
}
fn validate_soracloud_fhe_full_bootstrap_circuit_artifacts_for_job(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<(), InstructionExecutionError> {
    let is_bootstrap_job = job.operation == FheJobOperationV1::Bootstrap && job.bootstrap_count > 0;
    let Some(bootstrap_key) = evaluation_keys.bootstrap_key.as_ref() else {
        if full_bootstrap_circuit_artifacts.is_some() {
            return Err(invalid_parameter(
                "FHE full-bootstrap circuit artifacts are only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    };
    if !is_bootstrap_job || bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        if full_bootstrap_circuit_artifacts.is_some() {
            return Err(invalid_parameter(
                "FHE full-bootstrap circuit artifacts are only accepted for full-bootstrap operations",
            ));
        }
        return Ok(());
    }
    validate_soracloud_fhe_full_bootstrap_single_count(job)?;
    let artifacts = full_bootstrap_circuit_artifacts.ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap operation requires full-bootstrap circuit artifacts")
    })?;
    for slot in first_matching_fhe_slots(inputs)? {
        validate_bfv_full_bootstrap_execution_artifacts_preflight_v1(
            params,
            bootstrap_key,
            slot,
            artifacts,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap circuit artifact preflight failed: {err}"
            ))
        })?;
    }
    if full_bootstrap_release_audit.is_none() {
        return Err(invalid_parameter(
            FHE_FULL_BOOTSTRAP_RELEASE_AUDIT_CONTEXT_REQUIRED,
        ));
    }
    Ok(())
}
fn execute_soracloud_fhe_job_with_residual_bounds_and_full_bootstrap_artifacts(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_residual_bounds: &[u128],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<(BfvIdentifierCiphertext, Option<u128>), InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    validate_soracloud_fhe_full_bootstrap_circuit_artifacts_for_job(
        params,
        evaluation_keys,
        job,
        inputs,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    let output_residual_bound = soracloud_fhe_job_output_residual_multiple_bound(
        params,
        evaluation_keys,
        job,
        inputs,
        input_residual_bounds,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    let output = execute_soracloud_fhe_job_with_full_bootstrap_artifacts(
        params,
        evaluation_keys,
        job,
        inputs,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    Ok((output, output_residual_bound))
}
#[cfg(test)]
fn execute_soracloud_fhe_job_with_bounded_noise_bounds(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_noise_bounds: &[u128],
) -> Result<(BfvIdentifierCiphertext, Option<u128>), InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    let output_noise_bound = soracloud_fhe_job_output_bounded_noise_bound(
        params,
        evaluation_keys,
        job,
        inputs,
        input_noise_bounds,
        None,
        None,
    )?;
    let output = execute_soracloud_fhe_job_bounded_noise(params, evaluation_keys, job, inputs)?;
    Ok((output, output_noise_bound))
}
fn execute_soracloud_fhe_job_with_bounded_noise_bounds_and_full_bootstrap_artifacts(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_noise_bounds: &[u128],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<(BfvIdentifierCiphertext, Option<u128>), InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    validate_soracloud_fhe_full_bootstrap_circuit_artifacts_for_job(
        params,
        evaluation_keys,
        job,
        inputs,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    let output_noise_bound = soracloud_fhe_job_output_bounded_noise_bound(
        params,
        evaluation_keys,
        job,
        inputs,
        input_noise_bounds,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    let output = execute_soracloud_fhe_job_bounded_noise_with_full_bootstrap_artifacts(
        params,
        evaluation_keys,
        job,
        inputs,
        full_bootstrap_circuit_artifacts,
        full_bootstrap_release_audit,
    )?;
    Ok((output, output_noise_bound))
}
#[cfg(test)]
fn execute_soracloud_fhe_job(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    execute_soracloud_fhe_job_with_full_bootstrap_artifacts(
        params,
        evaluation_keys,
        job,
        inputs,
        None,
        None,
    )
}
fn execute_soracloud_fhe_job_with_full_bootstrap_artifacts(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    match job.operation {
        FheJobOperationV1::Add => fold_fhe_slots(params, inputs, |lhs, rhs| {
            add_ciphertexts_registered_rns_exact(params, lhs, rhs)
                .map_err(|err| invalid_parameter(format!("FHE add failed: {err}")))
        }),
        FheJobOperationV1::Multiply => fold_fhe_slots_balanced(params, inputs, |lhs, rhs| {
            multiply_ciphertexts_registered_rns_exact(
                params,
                &evaluation_keys.relinearization_key,
                lhs,
                rhs,
            )
            .map_err(|err| invalid_parameter(format!("FHE multiply failed: {err}")))
        }),
        FheJobOperationV1::RotateLeft => {
            let slots = first_matching_fhe_slots(inputs)?;
            if slots.len() == 1 {
                let rotated =
                    rotate_packed_ciphertext_slots_left_with_galois_keys_registered_rns_exact(
                        params,
                        &evaluation_keys.galois_keys,
                        &slots[0],
                        job.rotation_steps,
                    )
                    .map_err(|err| invalid_parameter(format!("FHE packed rotate failed: {err}")))?;
                return Ok(BfvIdentifierCiphertext {
                    slots: vec![rotated],
                });
            }
            let rotation_key = evaluation_keys
                .rotation_keys
                .iter()
                .find(|key| key.rotation_steps == job.rotation_steps)
                .ok_or_else(|| {
                    invalid_parameter(format!(
                        "missing BFV rotation key for {} steps",
                        job.rotation_steps
                    ))
                })?;
            let slots =
                rotate_ciphertext_slots_left_registered_rns_exact(params, rotation_key, slots)
                    .map_err(|err| invalid_parameter(format!("FHE rotate failed: {err}")))?;
            Ok(BfvIdentifierCiphertext { slots })
        }
        FheJobOperationV1::Bootstrap => {
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bootstrap operation")
            })?;
            let slots = first_matching_fhe_slots(inputs)?
                .iter()
                .map(|slot| {
                    let result = match bootstrap_key.mode {
                        BfvBootstrapKeyMode::RefreshOnlyV1 => {
                            bootstrap_ciphertext_registered_rns_exact_rounds(
                                params,
                                bootstrap_key,
                                slot,
                                job.bootstrap_count,
                            )
                        }
                        BfvBootstrapKeyMode::FullBootstrapV1 => {
                            validate_soracloud_fhe_full_bootstrap_single_count(job)?;
                            if let Some(artifacts) = full_bootstrap_circuit_artifacts {
                                if let Some(audit) = full_bootstrap_release_audit {
                                    full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1(
                                        params,
                                        bootstrap_key,
                                        artifacts,
                                        &evaluation_keys.galois_keys,
                                        slot,
                                        audit.package,
                                        audit.expected_package_digest,
                                        audit.trusted_reviewer_id,
                                        audit.trusted_reviewer_public_key,
                                    )
                                } else {
                                    Err(full_bootstrap_release_audit_context_required_error())
                                }
                            } else {
                                full_bootstrap_ciphertext_registered_rns_exact_v1(
                                    params,
                                    bootstrap_key,
                                    slot,
                                )
                            }
                        }
                    };
                    result.map_err(|err| invalid_parameter(format!("FHE bootstrap failed: {err}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(BfvIdentifierCiphertext { slots })
        }
    }
}
#[cfg(test)]
fn execute_soracloud_fhe_job_bounded_noise(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    execute_soracloud_fhe_job_bounded_noise_with_full_bootstrap_artifacts(
        params,
        evaluation_keys,
        job,
        inputs,
        None,
        None,
    )
}
fn execute_soracloud_fhe_job_bounded_noise_with_full_bootstrap_artifacts(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    full_bootstrap_circuit_artifacts: Option<&BfvFullBootstrapCircuitArtifactBundleV1>,
    full_bootstrap_release_audit: Option<&BfvFullBootstrapReleaseAuditRuntimeContext<'_>>,
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    match job.operation {
        FheJobOperationV1::Add => fold_fhe_slots(params, inputs, |lhs, rhs| {
            add_ciphertexts_bounded_noise_registered_rns_exact(params, lhs, rhs)
                .map_err(|err| invalid_parameter(format!("FHE bounded-noise add failed: {err}")))
        }),
        FheJobOperationV1::Multiply => fold_fhe_slots_balanced(params, inputs, |lhs, rhs| {
            multiply_ciphertexts_bounded_noise_registered_rns_basis_extension_exact(
                params,
                &evaluation_keys.relinearization_key,
                lhs,
                rhs,
            )
            .map_err(|err| invalid_parameter(format!("FHE bounded-noise multiply failed: {err}")))
        }),
        FheJobOperationV1::RotateLeft => {
            let slots = first_matching_fhe_slots(inputs)?;
            if slots.len() == 1 {
                let rotated =
                    rotate_packed_ciphertext_slots_left_with_galois_keys_bounded_noise_registered_rns_basis_extension_exact(
                        params,
                        &evaluation_keys.galois_keys,
                        &slots[0],
                        job.rotation_steps,
                    )
                    .map_err(|err| {
                        invalid_parameter(format!("FHE bounded-noise packed rotate failed: {err}"))
                    })?;
                return Ok(BfvIdentifierCiphertext {
                    slots: vec![rotated],
                });
            }
            let rotation_key = evaluation_keys
                .rotation_keys
                .iter()
                .find(|key| key.rotation_steps == job.rotation_steps)
                .ok_or_else(|| {
                    invalid_parameter(format!(
                        "missing BFV rotation key for {} steps",
                        job.rotation_steps
                    ))
                })?;
            let slots =
                rotate_ciphertext_slots_left_bounded_noise_registered_rns_basis_extension_exact(
                    params,
                    rotation_key,
                    slots,
                )
                .map_err(|err| {
                    invalid_parameter(format!("FHE bounded-noise rotate failed: {err}"))
                })?;
            Ok(BfvIdentifierCiphertext { slots })
        }
        FheJobOperationV1::Bootstrap => {
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bounded-noise bootstrap operation")
            })?;
            let slots = first_matching_fhe_slots(inputs)?
                .iter()
                .map(|slot| {
                    let result = match bootstrap_key.mode {
                        BfvBootstrapKeyMode::RefreshOnlyV1 => {
                            bootstrap_ciphertext_bounded_noise_registered_rns_basis_extension_exact_rounds(
                                params,
                                bootstrap_key,
                                slot,
                                job.bootstrap_count,
                            )
                        }
                        BfvBootstrapKeyMode::FullBootstrapV1 => {
                            validate_soracloud_fhe_full_bootstrap_single_count(job)?;
                            if let Some(artifacts) = full_bootstrap_circuit_artifacts {
                                if let Some(audit) = full_bootstrap_release_audit {
                                    full_bootstrap_ciphertext_with_release_audited_artifacts_bounded_noise_registered_rns_basis_extension_exact_v1(
                                        params,
                                        bootstrap_key,
                                        artifacts,
                                        &evaluation_keys.galois_keys,
                                        slot,
                                        audit.package,
                                        audit.expected_package_digest,
                                        audit.trusted_reviewer_id,
                                        audit.trusted_reviewer_public_key,
                                    )
                                } else {
                                    Err(full_bootstrap_release_audit_context_required_error())
                                }
                            } else {
                                full_bootstrap_ciphertext_bounded_noise_registered_rns_basis_extension_exact_v1(
                                    params,
                                    bootstrap_key,
                                    slot,
                                )
                            }
                        }
                    };
                    result
                    .map_err(|err| {
                        invalid_parameter(format!("FHE bounded-noise bootstrap failed: {err}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(BfvIdentifierCiphertext { slots })
        }
    }
}
fn verify_soracloud_fhe_evaluation_key_digest(
    params: &BfvParameters,
    policy: &FheExecutionPolicyV1,
    evaluation_keys: &BfvEvaluationKeyBundle,
) -> Result<(), InstructionExecutionError> {
    let actual = evaluation_keys
        .digest(params)
        .map_err(|err| invalid_parameter(format!("invalid BFV evaluation keys: {err}")))?;
    if actual != policy.evaluation_key_digest {
        return Err(invalid_parameter(
            "fhe evaluation-key digest does not match the execution policy",
        ));
    }
    Ok(())
}
fn verify_soracloud_fhe_refresh_transcript_digest(
    params: &BfvParameters,
    policy: &FheExecutionPolicyV1,
    evaluation_keys: &BfvEvaluationKeyBundle,
    transcript: &BfvEvaluationKeyRefreshTranscriptV1,
) -> Result<(), InstructionExecutionError> {
    let actual = transcript
        .digest_for_evaluation_keys_with_mode(
            params,
            evaluation_keys,
            policy.refresh_transcript_mode,
        )
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if actual != policy.evaluation_key_refresh_transcript_digest {
        return Err(invalid_parameter(
            "fhe evaluation-key refresh transcript digest does not match the execution policy",
        ));
    }
    let expected_public_key_statement =
        policy.public_key_proof_statement_digest.ok_or_else(|| {
            invalid_parameter("fhe policy must bind public-key proof statement digest")
        })?;
    let actual_public_key_statement = transcript
        .public_key_proof_statement_digest_with_mode(params, policy.refresh_transcript_mode)
        .map_err(|err| invalid_parameter(err.to_string()))?;
    if actual_public_key_statement != expected_public_key_statement {
        return Err(invalid_parameter(
            "fhe public-key proof statement digest does not match the execution policy",
        ));
    }
    match evaluation_keys.bootstrap_key.as_ref() {
        Some(bootstrap_key) if bootstrap_key.mode == BfvBootstrapKeyMode::FullBootstrapV1 => {
            if policy.max_bootstrap_count == 0 {
                return Err(invalid_parameter(
                    "fhe full-bootstrap governed material requires bootstrap-capable policy",
                ));
            }
            if policy
                .bootstrap_key_zero_refresh_proof_statement_digest
                .is_some()
            {
                return Err(invalid_parameter(
                    "fhe full-bootstrap policy must not bind bootstrap-key zero-refresh proof statement digest",
                ));
            }
            if policy.full_bootstrap_release_audit_package.is_none() {
                return Err(invalid_parameter(
                    "fhe full-bootstrap policy must bind governed release-audited material",
                ));
            }
        }
        _ => {
            if let Some(expected) = policy.bootstrap_key_zero_refresh_proof_statement_digest {
                if policy.max_bootstrap_count == 0 {
                    return Err(invalid_parameter(
                        "fhe bootstrap-key proof statement digest requires bootstrap-capable policy",
                    ));
                }
                let actual = transcript
                    .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                        params,
                        evaluation_keys,
                        policy.refresh_transcript_mode,
                    )
                    .map_err(|err| invalid_parameter(err.to_string()))?
                    .ok_or_else(|| {
                        invalid_parameter(
                            "fhe bootstrap-key proof statement digest requires bootstrap key material",
                        )
                    })?;
                if actual != expected {
                    return Err(invalid_parameter(
                        "fhe bootstrap-key proof statement digest does not match the execution policy",
                    ));
                }
            } else if policy.max_bootstrap_count > 0 {
                return Err(invalid_parameter(
                    "fhe bootstrap-capable policy must bind bootstrap-key proof statement digest",
                ));
            }
        }
    }
    Ok(())
}
fn soracloud_fhe_ciphertext_bound_mode(policy: &FheExecutionPolicyV1) -> BfvCiphertextBoundModeV1 {
    match policy.refresh_transcript_mode {
        BfvRefreshTranscriptModeV1::ExactLift => BfvCiphertextBoundModeV1::ExactResidualMultiple,
        BfvRefreshTranscriptModeV1::BoundedNoise => BfvCiphertextBoundModeV1::BoundedNoise,
    }
}
fn insert_admitted_bundle(
    state_transaction: &mut StateTransaction<'_, '_>,
    bundle: SoraDeploymentBundleV1,
) {
    state_transaction.world.soracloud_service_revisions.insert(
        (
            bundle.service.service_name.as_ref().to_owned(),
            bundle.service.service_version.clone(),
        ),
        bundle,
    );
}
fn admit_bundle(
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    provenance: ManifestProvenance,
    action: SoraServiceLifecycleActionV1,
) -> Result<(), InstructionExecutionError> {
    require_soracloud_permission(authority, state_transaction)?;
    verify_bundle_provenance(
        authority,
        &bundle,
        &initial_service_configs,
        &initial_service_secrets,
        &provenance,
    )?;
    bundle
        .validate_for_admission()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    let service_name = bundle.service.service_name.clone();
    let service_version = bundle.service.service_version.clone();
    let revision_key = (service_name.as_ref().to_owned(), service_version.clone());
    if state_transaction
        .world
        .soracloud_service_revisions
        .get(&revision_key)
        .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "service `{service_name}` revision `{service_version}` has already been admitted"
            )
            .into(),
        ));
    }
    let existing = state_transaction
        .world
        .soracloud_service_deployments
        .get(&service_name)
        .cloned();
    match (action, existing.as_ref()) {
        (SoraServiceLifecycleActionV1::Deploy, Some(_)) => {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("service `{service_name}` is already deployed").into(),
            ));
        }
        (SoraServiceLifecycleActionV1::Upgrade, None) => {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("service `{service_name}` must be deployed before it can be upgraded")
                    .into(),
            ));
        }
        _ => {}
    }
    if existing
        .as_ref()
        .is_some_and(|state| state.current_service_version == service_version)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("service `{service_name}` is already at version `{service_version}`").into(),
        ));
    }
    let sequence = next_soracloud_audit_sequence(state_transaction);
    let previous_version = existing
        .as_ref()
        .map(|deployment| deployment.current_service_version.clone());
    let current_service_manifest_hash = bundle.service_manifest_hash();
    let current_container_manifest_hash = bundle.container_manifest_hash();
    let mut service_configs = existing.as_ref().map_or_else(BTreeMap::new, |deployment| {
        deployment.service_configs.clone()
    });
    let mut service_secrets = existing.as_ref().map_or_else(BTreeMap::new, |deployment| {
        deployment.service_secrets.clone()
    });
    let mut config_generation = existing
        .as_ref()
        .map_or(0, |deployment| deployment.config_generation);
    let mut secret_generation = existing
        .as_ref()
        .map_or(0, |deployment| deployment.secret_generation);
    if !initial_service_configs.is_empty() {
        config_generation = config_generation.saturating_add(1);
        for (config_name, value_json) in initial_service_configs {
            let value_hash = service_config_value_hash(&value_json)?;
            service_configs.insert(
                config_name.clone(),
                SoraServiceConfigEntryV1 {
                    schema_version: SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                    config_name,
                    value_json,
                    value_hash,
                    last_update_sequence: sequence,
                },
            );
        }
    }
    if !initial_service_secrets.is_empty() {
        secret_generation = secret_generation.saturating_add(1);
        for (secret_name, envelope) in initial_service_secrets {
            service_secrets.insert(
                secret_name.clone(),
                SoraServiceSecretEntryV1 {
                    schema_version: SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                    secret_name,
                    envelope,
                    last_update_sequence: sequence,
                },
            );
        }
    }
    bundle
        .validate_required_service_materials(&service_configs, &service_secrets)
        .map_err(|err| invalid_parameter(err.to_string()))?;
    insert_admitted_bundle(state_transaction, bundle.clone());
    let revision_count = count_revisions_for_service(state_transaction, &service_name);
    let process_generation = existing.as_ref().map_or(1, |deployment| {
        deployment.process_generation.saturating_add(1)
    });
    let last_rollout = if action == SoraServiceLifecycleActionV1::Upgrade {
        Some(build_rollout_state(
            &bundle,
            sequence,
            previous_version.clone(),
        )?)
    } else {
        None
    };
    let active_rollout = last_rollout
        .clone()
        .filter(|rollout| rollout.stage == SoraRolloutStageV1::Canary);
    let service_lease = build_http_service_lease_state(
        &bundle,
        existing.as_ref(),
        sequence,
        action == SoraServiceLifecycleActionV1::Upgrade,
    )?;
    let lease_volume_states =
        build_http_service_lease_volume_states(&bundle, service_lease.as_ref(), existing.as_ref());
    record_deployment_state(
        state_transaction,
        SoraServiceDeploymentStateV1 {
            schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            service_name: service_name.clone(),
            current_service_version: service_version.clone(),
            current_service_manifest_hash,
            current_container_manifest_hash,
            revision_count,
            process_generation,
            process_started_sequence: sequence,
            config_generation,
            secret_generation,
            service_configs,
            service_secrets,
            fhe_policy_records: existing
                .as_ref()
                .map(|deployment| deployment.fhe_policy_records.clone())
                .unwrap_or_default(),
            active_rollout,
            last_rollout: last_rollout.clone(),
            service_lease,
            lease_volume_states,
        },
    )?;
    record_audit_event(
        state_transaction,
        SoraServiceAuditEventV1 {
            schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
            sequence,
            action,
            service_name,
            from_version: previous_version,
            to_version: service_version,
            service_manifest_hash: current_service_manifest_hash,
            container_manifest_hash: current_container_manifest_hash,
            governance_tx_hash: None,
            binding_name: None,
            state_key: None,
            config_name: None,
            secret_name: None,
            rollout_handle: last_rollout.map(|rollout| rollout.rollout_handle),
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signer: provenance.signer,
        },
    )
}
impl Execute for isi::DeploySoracloudService {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        admit_bundle(
            authority,
            state_transaction,
            self.bundle,
            self.initial_service_configs,
            self.initial_service_secrets,
            self.provenance,
            SoraServiceLifecycleActionV1::Deploy,
        )
    }
}
impl Execute for isi::UpgradeSoracloudService {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        admit_bundle(
            authority,
            state_transaction,
            self.bundle,
            self.initial_service_configs,
            self.initial_service_secrets,
            self.provenance,
            SoraServiceLifecycleActionV1::Upgrade,
        )
    }
}
fn admit_app_infra(
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    manifest: SoraAppInfraManifestV1,
    provenance: ManifestProvenance,
    action: SoraAppInfraActionV1,
) -> Result<(), InstructionExecutionError> {
    require_soracloud_permission(authority, state_transaction)?;
    verify_app_infra_provenance(authority, &manifest, &provenance)?;
    manifest
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    let app_name = manifest.app_name.clone();
    let existing = state_transaction
        .world
        .soracloud_app_infra_states
        .get(&app_name)
        .cloned();
    match (action, existing.as_ref()) {
        (SoraAppInfraActionV1::Deploy, Some(_)) => {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("app `{app_name}` infrastructure is already deployed").into(),
            ));
        }
        (SoraAppInfraActionV1::Upgrade, None) => {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` infrastructure must be deployed before it can be upgraded"
                )
                .into(),
            ));
        }
        _ => {}
    }
    for service_ref in &manifest.services {
        let deployment = state_transaction
            .world
            .soracloud_service_deployments
            .get(&service_ref.service_name)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "app `{app_name}` references missing service `{}`",
                        service_ref.service_name
                    )
                    .into(),
                )
            })?;
        if deployment.current_service_version != service_ref.service_version {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` references service `{}` version `{}`, but active version is `{}`",
                    service_ref.service_name,
                    service_ref.service_version,
                    deployment.current_service_version
                )
                .into(),
            ));
        }
        if deployment.current_service_manifest_hash != service_ref.service_manifest_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` service `{}` manifest hash does not match active deployment",
                    service_ref.service_name
                )
                .into(),
            ));
        }
        if deployment.current_container_manifest_hash != service_ref.container_manifest_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` service `{}` container hash does not match active deployment",
                    service_ref.service_name
                )
                .into(),
            ));
        }
        let bundle = load_admitted_bundle(
            state_transaction,
            &service_ref.service_name,
            &service_ref.service_version,
        )?;
        if bundle.service.execution_plane != service_ref.execution_plane {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` service `{}` execution plane does not match admitted revision",
                    service_ref.service_name
                )
                .into(),
            ));
        }
        if bundle.container.runtime != service_ref.runtime {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "app `{app_name}` service `{}` runtime does not match admitted revision",
                    service_ref.service_name
                )
                .into(),
            ));
        }
    }
    let sequence = next_soracloud_audit_sequence(state_transaction);
    let previous_version = existing
        .as_ref()
        .map(|state| state.current_app_version.clone());
    let manifest_hash = manifest.manifest_hash();
    let revision_count = existing
        .as_ref()
        .map_or(1, |state| state.revision_count.saturating_add(1));
    let deployed_sequence = existing
        .as_ref()
        .map_or(sequence, |state| state.deployed_sequence);
    record_app_infra_state(
        state_transaction,
        SoraAppInfraStateV1 {
            schema_version: SORA_APP_INFRA_STATE_VERSION_V1,
            app_name: app_name.clone(),
            current_app_version: manifest.app_version.clone(),
            current_manifest_hash: manifest_hash,
            revision_count,
            deployed_sequence,
            updated_sequence: sequence,
            manifest: manifest.clone(),
        },
    )?;
    record_app_infra_audit_event(
        state_transaction,
        SoraAppInfraAuditEventV1 {
            schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
            sequence,
            action,
            app_name,
            from_version: previous_version,
            to_version: manifest.app_version,
            app_manifest_hash: manifest_hash,
            service_count: u32::try_from(manifest.services.len()).unwrap_or(u32::MAX),
            signer: provenance.signer,
        },
    )
}
impl Execute for isi::DeploySoracloudAppInfra {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        admit_app_infra(
            authority,
            state_transaction,
            self.manifest,
            self.provenance,
            SoraAppInfraActionV1::Deploy,
        )
    }
}
impl Execute for isi::UpgradeSoracloudAppInfra {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        admit_app_infra(
            authority,
            state_transaction,
            self.manifest,
            self.provenance,
            SoraAppInfraActionV1::Upgrade,
        )
    }
}
impl Execute for isi::RollbackSoracloudService {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_rollback_provenance(
            authority,
            &self.service_name,
            self.target_version.as_deref(),
            &self.provenance,
        )?;
        let Some(existing) = state_transaction
            .world
            .soracloud_service_deployments
            .get(&self.service_name)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("service `{}` is not deployed", self.service_name).into(),
            ));
        };
        let target_version = match self.target_version {
            Some(target_version) => {
                if target_version.trim().is_empty() {
                    return Err(invalid_parameter("target_version must not be empty"));
                }
                if target_version == existing.current_service_version {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "service `{}` is already at version `{target_version}`",
                            self.service_name
                        )
                        .into(),
                    ));
                }
                target_version
            }
            None => previous_service_version(
                state_transaction,
                &self.service_name,
                &existing.current_service_version,
            )
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "service `{}` has no previously admitted revision to roll back to",
                        self.service_name
                    )
                    .into(),
                )
            })?,
        };
        let bundle = load_admitted_bundle(state_transaction, &self.service_name, &target_version)?;
        bundle
            .validate_required_service_materials(
                &existing.service_configs,
                &existing.service_secrets,
            )
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let service_lease =
            build_http_service_lease_state(&bundle, Some(&existing), sequence, false)?;
        let lease_volume_states = build_http_service_lease_volume_states(
            &bundle,
            service_lease.as_ref(),
            Some(&existing),
        );
        record_deployment_state(
            state_transaction,
            SoraServiceDeploymentStateV1 {
                schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: self.service_name.clone(),
                current_service_version: target_version.clone(),
                current_service_manifest_hash: bundle.service_manifest_hash(),
                current_container_manifest_hash: bundle.container_manifest_hash(),
                revision_count: existing.revision_count,
                process_generation: existing.process_generation.saturating_add(1),
                process_started_sequence: sequence,
                config_generation: existing.config_generation,
                secret_generation: existing.secret_generation,
                service_configs: existing.service_configs,
                service_secrets: existing.service_secrets,
                fhe_policy_records: existing.fhe_policy_records,
                active_rollout: None,
                last_rollout: None,
                service_lease,
                lease_volume_states,
            },
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::Rollback,
                service_name: self.service_name,
                from_version: Some(existing.current_service_version),
                to_version: target_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::SetSoracloudServiceConfig {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_service_config_set_provenance(
            authority,
            &self.service_name,
            &self.config_name,
            &self.value_json,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_service_config_mutation(
            state_transaction,
            &self.service_name,
            &self.config_name,
            Some(self.value_json),
            sequence,
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::ConfigMutation,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: Some(self.config_name),
                secret_name: None,
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::DeleteSoracloudServiceConfig {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_service_config_delete_provenance(
            authority,
            &self.service_name,
            &self.config_name,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_service_config_mutation(
            state_transaction,
            &self.service_name,
            &self.config_name,
            None,
            sequence,
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::ConfigMutation,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: Some(self.config_name),
                secret_name: None,
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::SetSoracloudServiceSecret {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_service_secret_set_provenance(
            authority,
            &self.service_name,
            &self.secret_name,
            &self.secret,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_service_secret_mutation(
            state_transaction,
            &self.service_name,
            &self.secret_name,
            Some(self.secret),
            sequence,
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::SecretMutation,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: Some(self.secret_name),
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::DeleteSoracloudServiceSecret {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_service_secret_delete_provenance(
            authority,
            &self.service_name,
            &self.secret_name,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_service_secret_mutation(
            state_transaction,
            &self.service_name,
            &self.secret_name,
            None,
            sequence,
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::SecretMutation,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: Some(self.secret_name),
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::MutateSoracloudState {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::MutateSoracloudState {
            service_name,
            binding_name,
            state_key,
            operation,
            value_size_bytes,
            value_payload,
            encryption,
            governance_tx_hash,
            fhe_input_admission_proof,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let (signed_value_size_bytes, signed_payload_commitment) = match operation {
            SoraStateMutationOperationV1::Upsert => {
                let payload = value_payload.as_ref().ok_or_else(|| {
                    invalid_parameter("value_payload is required for upsert mutations")
                })?;
                let actual_size = u64::try_from(payload.len())
                    .map_err(|_| invalid_parameter("value_payload length exceeds u64 range"))?;
                if let Some(declared_size) = value_size_bytes
                    && declared_size != actual_size
                {
                    return Err(invalid_parameter(format!(
                        "value_size_bytes {declared_size} does not match value_payload length {actual_size}",
                    )));
                }
                (Some(actual_size), Some(Hash::new(payload)))
            }
            SoraStateMutationOperationV1::Delete => {
                if value_size_bytes.is_some() || value_payload.is_some() {
                    return Err(invalid_parameter(
                        "delete mutations must not provide value_size_bytes or value_payload",
                    ));
                }
                (None, None)
            }
        };
        verify_state_mutation_provenance(
            authority,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            signed_value_size_bytes,
            signed_payload_commitment,
            encryption,
            governance_tx_hash,
            fhe_input_admission_proof.clone(),
            &provenance,
        )?;
        let admitted_fhe_bound = verify_soracloud_fhe_input_admission_proof(
            state_transaction,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            signed_value_size_bytes,
            value_payload.as_deref(),
            signed_payload_commitment,
            encryption,
            governance_tx_hash,
            fhe_input_admission_proof.as_ref(),
        )?;
        let (admitted_fhe_residual_bound, admitted_fhe_bound_mode, admitted_fhe_public_key_digest) =
            admitted_fhe_bound.map_or((None, None, None), |(bound, mode, public_key_digest)| {
                (Some(bound), Some(mode), Some(public_key_digest))
            });
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_soracloud_state_mutation(
            state_transaction,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            value_payload,
            encryption,
            admitted_fhe_public_key_digest,
            admitted_fhe_residual_bound,
            admitted_fhe_bound_mode,
            governance_tx_hash,
            sequence,
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::StateMutation,
                service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(governance_tx_hash),
                binding_name: Some(binding_name),
                state_key: Some(state_key),
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: provenance.signer,
            },
        )
    }
}
impl Execute for isi::RegisterSoracloudFhePolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.service_name != self.material.service_name {
            return Err(invalid_parameter(
                "FHE policy registration service_name must match material.service_name",
            ));
        }
        require_soracloud_fhe_governance_permission(
            authority,
            &self.service_name,
            &self.material.policy_name,
            state_transaction,
        )?;
        verify_fhe_policy_register_provenance(
            authority,
            &self.service_name,
            &self.material,
            &self.provenance,
        )?;
        self.material
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        registered_soracloud_bfv_parameters(&self.material.governance_bundle.param_set)?;
        if self.material.version.get() != 1 {
            return Err(invalid_parameter(
                "first FHE policy material version must be one",
            ));
        }
        let transaction_hash = current_signed_transaction_hash(state_transaction)?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (mut deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if deployment
            .fhe_policy_records
            .contains_key(&self.material.policy_name)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "FHE policy `{}` is already registered for service `{}`",
                    self.material.policy_name, self.service_name
                )
                .into(),
            ));
        }
        let policy_name = self.material.policy_name.clone();
        let material_digest = self.material.material_digest;
        let version = self.material.version;
        let record = SoracloudFhePolicyRecordV1 {
            schema_version: SORACLOUD_FHE_POLICY_RECORD_VERSION_V1,
            service_name: self.service_name.clone(),
            policy_name: policy_name.clone(),
            active_version: Some(version),
            versions: BTreeMap::from([(
                version,
                SoracloudFhePolicyVersionStateV1 {
                    material: self.material,
                    admitted_by_transaction_hash: transaction_hash,
                    lifecycle: SoracloudFhePolicyVersionLifecycleV1::Active,
                    deactivated_by_transaction_hash: None,
                },
            )]),
        };
        record
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        deployment
            .fhe_policy_records
            .insert(policy_name.clone(), record);
        record_deployment_state(state_transaction, deployment.clone())?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::FhePolicyRegister,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(transaction_hash),
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: Some(policy_name),
                policy_snapshot_hash: Some(material_digest),
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RotateSoracloudFhePolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        self.expected_active
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        if self.service_name != self.material.service_name
            || self.expected_active.policy_name != self.material.policy_name
        {
            return Err(invalid_parameter(
                "FHE policy rotation service and policy identities must match",
            ));
        }
        require_soracloud_fhe_governance_permission(
            authority,
            &self.service_name,
            &self.expected_active.policy_name,
            state_transaction,
        )?;
        verify_fhe_policy_rotate_provenance(
            authority,
            &self.service_name,
            &self.expected_active,
            &self.material,
            &self.provenance,
        )?;
        self.material
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        registered_soracloud_bfv_parameters(&self.material.governance_bundle.param_set)?;
        let next_version = self
            .expected_active
            .version
            .get()
            .checked_add(1)
            .and_then(std::num::NonZeroU32::new)
            .ok_or_else(|| invalid_parameter("FHE policy version exceeds u32"))?;
        if self.material.version != next_version {
            return Err(invalid_parameter(
                "rotated FHE policy material must use the exact next version",
            ));
        }
        let transaction_hash = current_signed_transaction_hash(state_transaction)?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (mut deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        resolve_active_soracloud_fhe_material(&deployment, &self.expected_active)?;
        let record = deployment
            .fhe_policy_records
            .get_mut(&self.expected_active.policy_name)
            .expect("active material resolution established the policy record");
        let old_state = record
            .versions
            .get_mut(&self.expected_active.version)
            .expect("active material resolution established the active version");
        old_state.lifecycle = SoracloudFhePolicyVersionLifecycleV1::Superseded;
        old_state.deactivated_by_transaction_hash = Some(transaction_hash);
        let material_digest = self.material.material_digest;
        record.versions.insert(
            next_version,
            SoracloudFhePolicyVersionStateV1 {
                material: self.material,
                admitted_by_transaction_hash: transaction_hash,
                lifecycle: SoracloudFhePolicyVersionLifecycleV1::Active,
                deactivated_by_transaction_hash: None,
            },
        );
        record.active_version = Some(next_version);
        record
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let policy_name = record.policy_name.clone();
        record_deployment_state(state_transaction, deployment.clone())?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::FhePolicyRotate,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(transaction_hash),
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: Some(policy_name),
                policy_snapshot_hash: Some(material_digest),
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RevokeSoracloudFhePolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        self.expected_active
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        require_soracloud_fhe_governance_permission(
            authority,
            &self.service_name,
            &self.expected_active.policy_name,
            state_transaction,
        )?;
        verify_fhe_policy_revoke_provenance(
            authority,
            &self.service_name,
            &self.expected_active,
            &self.provenance,
        )?;
        let transaction_hash = current_signed_transaction_hash(state_transaction)?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (mut deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        resolve_active_soracloud_fhe_material(&deployment, &self.expected_active)?;
        let record = deployment
            .fhe_policy_records
            .get_mut(&self.expected_active.policy_name)
            .expect("active material resolution established the policy record");
        let old_state = record
            .versions
            .get_mut(&self.expected_active.version)
            .expect("active material resolution established the active version");
        old_state.lifecycle = SoracloudFhePolicyVersionLifecycleV1::Revoked;
        old_state.deactivated_by_transaction_hash = Some(transaction_hash);
        record.active_version = None;
        record
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let policy_name = record.policy_name.clone();
        record_deployment_state(state_transaction, deployment.clone())?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::FhePolicyRevoke,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(transaction_hash),
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: Some(policy_name),
                policy_snapshot_hash: Some(self.expected_active.material_digest),
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RunSoracloudFheJob {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_fhe_job_run_provenance(
            authority,
            &self.service_name,
            &self.binding_name,
            self.job.clone(),
            self.policy_reference.clone(),
            self.public_key_proof.clone(),
            self.bootstrap_key_zero_refresh_proof.clone(),
            self.full_bootstrap_execution_proofs.clone(),
            &self.provenance,
        )?;
        // Resolve the exact authenticated version before deriving any proof
        // statement or beginning deterministic execution. A transaction signed
        // against a superseded or revoked version must never fall through to a
        // newer policy with different key material.
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        let (material, governance_tx_hash) =
            resolve_active_soracloud_fhe_material(&deployment, &self.policy_reference)?;
        material
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let material_digest = material.material_digest;
        let policy_name = material.policy_name.clone();
        let policy = material.governance_bundle.execution_policy;
        let param_set = material.governance_bundle.param_set;
        let evaluation_keys = material.evaluation_keys;
        let evaluation_key_refresh_transcript = material.evaluation_key_refresh_transcript;
        let full_bootstrap_circuit_artifacts = material.full_bootstrap_circuit_artifacts;
        param_set
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        policy
            .validate_for_param_set(&param_set)
            .map_err(|err| invalid_parameter(err.to_string()))?;
        self.job
            .validate_for_execution(&policy, &param_set)
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let ciphertext_bound_mode = soracloud_fhe_ciphertext_bound_mode(&policy);
        let bfv_params = registered_soracloud_bfv_parameters(&param_set)?;
        evaluation_keys
            .validate(&bfv_params)
            .map_err(|err| invalid_parameter(format!("invalid BFV evaluation keys: {err}")))?;
        verify_soracloud_fhe_evaluation_key_digest(&bfv_params, &policy, &evaluation_keys)?;
        verify_soracloud_fhe_refresh_transcript_digest(
            &bfv_params,
            &policy,
            &evaluation_keys,
            &evaluation_key_refresh_transcript,
        )?;
        let public_key_digest =
            bfv_public_key_digest(&bfv_params, &evaluation_key_refresh_transcript.public_key)
                .map_err(|err| {
                    invalid_parameter(format!("failed to digest FHE public key: {err}"))
                })?;
        verify_soracloud_fhe_bootstrap_key_proof(
            state_transaction,
            &policy,
            policy.bootstrap_key_zero_refresh_proof_statement_digest,
            self.bootstrap_key_zero_refresh_proof.as_ref(),
            self.job.bootstrap_count > 0
                && policy
                    .bootstrap_key_zero_refresh_proof_statement_digest
                    .is_some(),
        )?;
        let loaded_inputs = load_soracloud_fhe_inputs(
            &bfv_params,
            state_transaction,
            &self.service_name,
            &self.binding_name,
            &self.job,
            &evaluation_key_refresh_transcript.public_key,
            public_key_digest,
            ciphertext_bound_mode,
        )?;
        let (input_envelopes, input_bounds): (Vec<_>, Vec<_>) = loaded_inputs
            .into_iter()
            .map(|input| (input.envelope, input.bound))
            .unzip();
        preflight_soracloud_fhe_full_bootstrap_execution_proofs(
            &self.job,
            &evaluation_keys,
            &input_envelopes,
            full_bootstrap_circuit_artifacts.as_ref(),
            &self.full_bootstrap_execution_proofs,
        )?;
        verify_soracloud_fhe_public_key_proof(
            state_transaction,
            &policy,
            policy.public_key_proof_statement_digest,
            self.public_key_proof.as_ref(),
        )?;
        let full_bootstrap_release_audit =
            soracloud_fhe_full_bootstrap_release_audit_runtime_context(
                &bfv_params,
                &evaluation_keys,
                &self.job,
                full_bootstrap_circuit_artifacts.as_ref(),
                &policy,
            )?;
        let (output_envelope, output_bound) = match ciphertext_bound_mode {
            BfvCiphertextBoundModeV1::ExactResidualMultiple => {
                execute_soracloud_fhe_job_with_residual_bounds_and_full_bootstrap_artifacts(
                    &bfv_params,
                    &evaluation_keys,
                    &self.job,
                    &input_envelopes,
                    &input_bounds,
                    full_bootstrap_circuit_artifacts.as_ref(),
                    full_bootstrap_release_audit.as_ref(),
                )?
            }
            BfvCiphertextBoundModeV1::BoundedNoise => {
                execute_soracloud_fhe_job_with_bounded_noise_bounds_and_full_bootstrap_artifacts(
                    &bfv_params,
                    &evaluation_keys,
                    &self.job,
                    &input_envelopes,
                    &input_bounds,
                    full_bootstrap_circuit_artifacts.as_ref(),
                    full_bootstrap_release_audit.as_ref(),
                )?
            }
        };
        verify_soracloud_fhe_full_bootstrap_execution_proofs(
            state_transaction,
            &bfv_params,
            &evaluation_keys,
            &evaluation_key_refresh_transcript,
            &self.job,
            &input_envelopes,
            &input_bounds,
            &output_envelope,
            output_bound,
            ciphertext_bound_mode,
            full_bootstrap_circuit_artifacts.as_ref(),
            &self.full_bootstrap_execution_proofs,
        )?;
        let output_payload = encode_soracloud_fhe_output_payload(&output_envelope)?;
        let output_payload_bytes = u64::try_from(output_payload.len())
            .map_err(|_| invalid_parameter("FHE output payload length exceeds u64 range"))?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let binding = bundle
            .service
            .state_bindings
            .iter()
            .find(|binding| binding.binding_name == self.binding_name)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "binding `{}` is not declared for service `{}`",
                        self.binding_name, self.service_name
                    )
                    .into(),
                )
            })?;
        if binding.encryption != SoraStateEncryptionV1::FheCiphertext {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "binding `{}` is not configured for FHE ciphertexts",
                    self.binding_name
                )
                .into(),
            ));
        }
        if binding.mutability == iroha_data_model::soracloud::SoraStateMutabilityV1::ReadOnly {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("binding `{}` is read-only", self.binding_name).into(),
            ));
        }
        if !self.job.output_state_key.starts_with(&binding.key_prefix) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "fhe output key `{}` is outside binding prefix `{}`",
                    self.job.output_state_key, binding.key_prefix
                )
                .into(),
            ));
        }
        let payload_bytes = std::num::NonZeroU64::new(output_payload_bytes).ok_or_else(|| {
            invalid_parameter("fhe output payload size must be greater than zero")
        })?;
        if output_payload_bytes > policy.max_ciphertext_bytes.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "fhe output size {output_payload_bytes} exceeds policy max_ciphertext_bytes {}",
                    policy.max_ciphertext_bytes
                )
                .into(),
            ));
        }
        if output_payload_bytes > binding.max_item_bytes.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "fhe output size {output_payload_bytes} exceeds binding max_item_bytes {}",
                    binding.max_item_bytes
                )
                .into(),
            ));
        }
        let state_entry_key = (
            self.service_name.as_ref().to_owned(),
            self.binding_name.as_ref().to_owned(),
            self.job.output_state_key.clone(),
        );
        let existing_size = state_transaction
            .world
            .soracloud_service_state_entries
            .get(&state_entry_key)
            .map_or(0, |entry| entry.payload_bytes.get());
        let (binding_total_bytes, _binding_key_count) =
            binding_state_totals(state_transaction, &self.service_name, &self.binding_name);
        if binding.mutability == iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly
            && existing_size > 0
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "binding `{}` is append-only; key `{}` already exists",
                    self.binding_name, self.job.output_state_key
                )
                .into(),
            ));
        }
        let tentative_total = projected_binding_state_total_bytes(
            self.binding_name.as_ref(),
            binding_total_bytes,
            existing_size,
            output_payload_bytes,
        )?;
        if tentative_total > binding.max_total_bytes.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "binding `{}` max_total_bytes {} would be exceeded",
                    self.binding_name, binding.max_total_bytes
                )
                .into(),
            ));
        }
        let output_state_key = self.job.output_state_key.clone();
        let output_commitment = Hash::new(&output_payload);
        record_service_state_entry(
            state_transaction,
            SoraServiceStateEntryV1 {
                schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                service_name: self.service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                binding_name: self.binding_name.clone(),
                state_key: output_state_key.clone(),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                payload: output_payload,
                payload_bytes,
                payload_commitment: output_commitment,
                fhe_public_key_digest: Some(public_key_digest),
                fhe_residual_multiple_bound: output_bound,
                fhe_bound_mode: Some(ciphertext_bound_mode),
                last_update_sequence: sequence,
                governance_tx_hash,
                source_action: SoraServiceLifecycleActionV1::FheJobRun,
            },
        )?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::FheJobRun,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(governance_tx_hash),
                binding_name: Some(self.binding_name),
                state_key: Some(output_state_key),
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: Some(policy_name),
                policy_snapshot_hash: Some(material_digest),
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RecordSoracloudDecryptionRequest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_decryption_request_provenance(
            authority,
            &self.service_name,
            self.policy.clone(),
            self.request.clone(),
            &self.provenance,
        )?;
        self.policy
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        self.request
            .validate_for_policy(&self.policy)
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        let binding = bundle
            .service
            .state_bindings
            .iter()
            .find(|binding| binding.binding_name == self.request.binding_name)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "binding `{}` is not declared for service `{}`",
                        self.request.binding_name, self.service_name
                    )
                    .into(),
                )
            })?;
        if binding.encryption == SoraStateEncryptionV1::Plaintext {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "binding `{}` is plaintext; decryption authority policy is not applicable",
                    self.request.binding_name
                )
                .into(),
            ));
        }
        if !self.request.state_key.starts_with(&binding.key_prefix) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "decryption request key `{}` is outside binding prefix `{}`",
                    self.request.state_key, binding.key_prefix
                )
                .into(),
            ));
        }
        let record_key = (
            self.service_name.as_ref().to_owned(),
            self.request.request_id.clone(),
        );
        if state_transaction
            .world
            .soracloud_decryption_request_records
            .get(&record_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "decryption request `{}` has already been recorded for service `{}`",
                    self.request.request_id, self.service_name
                )
                .into(),
            ));
        }
        let record = SoraDecryptionRequestRecordV1 {
            schema_version: SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
            service_name: self.service_name.clone(),
            service_version: deployment.current_service_version.clone(),
            policy: self.policy.clone(),
            request: self.request.clone(),
            sequence,
            signer: self.provenance.signer.clone(),
        };
        record
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let policy_snapshot_hash = record.policy_snapshot_hash();
        state_transaction
            .world
            .soracloud_decryption_request_records
            .insert(record_key, record);
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraServiceLifecycleActionV1::DecryptionRequest,
                service_name: self.service_name,
                from_version: None,
                to_version: deployment.current_service_version,
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                governance_tx_hash: Some(self.request.governance_tx_hash),
                binding_name: Some(self.request.binding_name.clone()),
                state_key: Some(self.request.state_key.clone()),
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: Some(self.request.policy_name.clone()),
                policy_snapshot_hash: Some(policy_snapshot_hash),
                jurisdiction_tag: Some(self.request.jurisdiction_tag.clone()),
                consent_evidence_hash: self.request.consent_evidence_hash,
                break_glass: Some(self.request.break_glass),
                break_glass_reason: self.request.break_glass_reason,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::JoinSoracloudHfSharedLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::JoinSoracloudHfSharedLease {
            repo_id,
            resolved_revision,
            model_name,
            service_name,
            apartment_name,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee,
            resource_profile,
            max_compute_reservation_fee,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let repo_id = parse_hf_repo_id(&repo_id)?;
        let resolved_revision = parse_hf_revision(&resolved_revision)?;
        let model_name = parse_hf_model_name(&model_name)?;
        if lease_term_ms == 0 {
            return Err(invalid_parameter("lease_term_ms must be greater than zero"));
        }
        if base_fee.is_zero() {
            return Err(invalid_parameter("base_fee must be greater than zero"));
        }
        verify_hf_shared_lease_join_provenance(
            authority,
            &repo_id,
            &resolved_revision,
            &model_name,
            &service_name,
            apartment_name.as_ref(),
            storage_class,
            lease_term_ms,
            &lease_asset_definition_id,
            &base_fee,
            &provenance,
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        let source_id = hf_source_id(&repo_id, &resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let mut source_record = state_transaction
            .world
            .soracloud_hf_sources
            .get(&source_id)
            .cloned()
            .unwrap_or(SoraHfSourceRecordV1 {
                schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                source_id,
                repo_id: repo_id.clone(),
                resolved_revision: resolved_revision.clone(),
                model_name: model_name.clone(),
                adapter_id: "hf.shared.v1".to_string(),
                normalized_runtime_hash: Hash::new(
                    format!("{repo_id}:{resolved_revision}:{model_name}").as_bytes(),
                ),
                resource_profile: None,
                status: SoraHfSourceStatusV1::PendingImport,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
                last_error: None,
            });
        if source_record.status == SoraHfSourceStatusV1::Failed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "hf source `{repo_id}@{resolved_revision}` is in failed state; fix import failure before joining"
                )
                .into(),
            ));
        }
        source_record.repo_id = repo_id.clone();
        source_record.resolved_revision = resolved_revision.clone();
        let resource_profile = resolve_hf_resource_profile(&mut source_record, resource_profile)?;
        let canonical_compute_cap =
            hf_shared_lease_max_compute_reservation_fee_v1(&resource_profile, lease_term_ms)
                .map_err(|err| invalid_parameter(err.to_string()))?;
        if max_compute_reservation_fee != canonical_compute_cap {
            return Err(invalid_parameter(format!(
                "max_compute_reservation_fee must equal the canonical V1 cap `{canonical_compute_cap}` for the reviewed resource profile and lease term"
            )));
        }
        source_record.updated_at_ms = now_ms;
        if source_record.status == SoraHfSourceStatusV1::Retired {
            source_record.status = SoraHfSourceStatusV1::PendingImport;
        }
        refresh_hf_source_status_from_generated_service(
            state_transaction,
            &mut source_record,
            &service_name,
        );
        record_hf_source(state_transaction, source_record.clone())?;
        let member_key = (pool_id.to_string(), authority.to_string());
        let mut pool_record = state_transaction
            .world
            .soracloud_hf_shared_lease_pools
            .get(&pool_id)
            .cloned();
        if let Some(pool) = pool_record.as_mut()
            && pool.window_expires_at_ms <= now_ms
            && matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
            )
        {
            if !promote_hf_shared_lease_queued_window(
                state_transaction,
                pool,
                &mut source_record,
                now_ms,
            )? {
                expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
                pool.active_member_count = 0;
                pool.status = SoraHfSharedLeaseStatusV1::Expired;
                record_hf_shared_lease_pool(state_transaction, pool.clone())?;
                retire_hf_placement_for_pool(
                    state_transaction,
                    &pool_id,
                    now_ms,
                    "lease window expired without a queued next-window sponsor",
                )?;
            }
            pool_record = state_transaction
                .world
                .soracloud_hf_shared_lease_pools
                .get(&pool_id)
                .cloned();
        }
        let existing_member = state_transaction
            .world
            .soracloud_hf_shared_lease_members
            .get(&member_key)
            .cloned();
        if let (Some(mut member), Some(pool)) = (existing_member.clone(), pool_record.as_ref())
            && member.status == SoraHfSharedLeaseMemberStatusV1::Active
            && pool.status == SoraHfSharedLeaseStatusV1::Active
            && pool.window_expires_at_ms > now_ms
        {
            ensure_hf_compute_reservation_charge_within_cap(
                &Quantity::zero(),
                &max_compute_reservation_fee,
            )?;
            if pool.lease_asset_definition_id != lease_asset_definition_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different settlement asset".into(),
                ));
            }
            if pool.base_fee != base_fee {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different base_fee".into(),
                ));
            }
            bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
            member.updated_at_ms = now_ms;
            member.last_charge = Quantity::zero();
            member.last_compute_charge = Quantity::zero();
            record_hf_shared_lease_member(state_transaction, member)?;
            return record_hf_shared_lease_audit_event(
                state_transaction,
                SoraHfSharedLeaseAuditEventV1 {
                    schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                    sequence: next_soracloud_audit_sequence(state_transaction),
                    action: SoraHfSharedLeaseActionV1::Join,
                    pool_id,
                    source_id,
                    account_id: authority.clone(),
                    occurred_at_ms: now_ms,
                    active_member_count: pool.active_member_count,
                    charged: Quantity::zero(),
                    refunded: Quantity::zero(),
                    lease_expires_at_ms: pool.window_expires_at_ms,
                    service_name: Some(service_name.to_string()),
                    apartment_name: apartment_name.map(|name| name.to_string()),
                },
            );
        }
        if let Some(mut pool) = pool_record.clone()
            && pool.status == SoraHfSharedLeaseStatusV1::Active
            && pool.window_expires_at_ms > now_ms
        {
            if pool.lease_asset_definition_id != lease_asset_definition_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different settlement asset".into(),
                ));
            }
            if pool.base_fee != base_fee {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different base_fee".into(),
                ));
            }
            let mut existing_members = canonical_hf_member_order(state_transaction, &pool_id);
            pool.active_member_count = u32::try_from(existing_members.len()).unwrap_or(u32::MAX);
            let remaining_ms = pool.window_expires_at_ms.saturating_sub(now_ms);
            let remaining_fee = prorated_window_fee(&base_fee, remaining_ms, lease_term_ms)?;
            let join_fee = divide_quantity_by_member_count(
                &remaining_fee,
                existing_members.len().saturating_add(1),
                "failed to calculate the per-member HF shared-lease storage join fee",
            )?;
            let placement = ensure_hf_placement_for_active_pool(
                state_transaction,
                &pool,
                &resource_profile,
                now_ms,
            )?;
            let remaining_compute_fee = prorated_window_fee(
                &placement.total_reservation_fee,
                remaining_ms,
                lease_term_ms,
            )?;
            let join_compute_fee = divide_quantity_by_member_count(
                &remaining_compute_fee,
                existing_members.len().saturating_add(1),
                "failed to calculate the per-member HF shared-lease compute join fee",
            )?;
            ensure_hf_compute_reservation_charge_within_cap(
                &join_compute_fee,
                &max_compute_reservation_fee,
            )?;
            distribute_hf_join_refunds(
                authority,
                &lease_asset_definition_id,
                now_ms,
                &join_fee,
                &mut existing_members,
                state_transaction,
                true,
            )?;
            distribute_hf_join_refunds(
                authority,
                &lease_asset_definition_id,
                now_ms,
                &join_compute_fee,
                &mut existing_members,
                state_transaction,
                false,
            )?;
            let mut member = existing_member.unwrap_or(SoraHfSharedLeaseMemberV1 {
                schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                pool_id,
                source_id,
                account_id: authority.clone(),
                status: SoraHfSharedLeaseMemberStatusV1::Left,
                joined_at_ms: now_ms,
                updated_at_ms: now_ms,
                total_paid: Quantity::zero(),
                total_refunded: Quantity::zero(),
                last_charge: Quantity::zero(),
                total_compute_paid: Quantity::zero(),
                total_compute_refunded: Quantity::zero(),
                last_compute_charge: Quantity::zero(),
                service_bindings: std::collections::BTreeSet::new(),
                apartment_bindings: std::collections::BTreeSet::new(),
            });
            member.status = SoraHfSharedLeaseMemberStatusV1::Active;
            member.joined_at_ms = now_ms;
            member.updated_at_ms = now_ms;
            member.total_paid = member.total_paid.checked_add(&join_fee).map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease member storage payment total exceeded the decimal domain",
                    error,
                )
            })?;
            member.last_charge = join_fee.clone();
            member.total_compute_paid = member
                .total_compute_paid
                .checked_add(&join_compute_fee)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF shared-lease member compute payment total exceeded the decimal domain",
                        error,
                    )
                })?;
            member.last_compute_charge = join_compute_fee.clone();
            bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
            record_hf_shared_lease_member(state_transaction, member)?;
            pool.active_member_count =
                u32::try_from(existing_members.len().saturating_add(1)).unwrap_or(u32::MAX);
            record_hf_shared_lease_pool(state_transaction, pool.clone())?;
            let total_join_charge = join_fee.checked_add(&join_compute_fee).map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease total join charge exceeded the decimal domain",
                    error,
                )
            })?;
            return record_hf_shared_lease_audit_event(
                state_transaction,
                SoraHfSharedLeaseAuditEventV1 {
                    schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                    sequence: next_soracloud_audit_sequence(state_transaction),
                    action: SoraHfSharedLeaseActionV1::Join,
                    pool_id,
                    source_id,
                    account_id: authority.clone(),
                    occurred_at_ms: now_ms,
                    active_member_count: pool.active_member_count,
                    charged: total_join_charge,
                    refunded: Quantity::zero(),
                    lease_expires_at_ms: pool.window_expires_at_ms,
                    service_name: Some(service_name.to_string()),
                    apartment_name: apartment_name.map(|name| name.to_string()),
                },
            );
        }
        let pool = SoraHfSharedLeasePoolV1 {
            schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            pool_id,
            source_id,
            storage_class,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee: base_fee.clone(),
            lease_term_ms,
            window_started_at_ms: now_ms,
            window_expires_at_ms: now_ms.saturating_add(lease_term_ms),
            active_member_count: 1,
            status: SoraHfSharedLeaseStatusV1::Active,
            queued_next_window: None,
        };
        reconcile_expired_model_hosts(state_transaction, now_ms)?;
        let placement = select_hf_placement_for_window(
            state_transaction,
            source_id,
            pool_id,
            &resource_profile,
            now_ms,
            now_ms,
            Some(&pool_id),
        )?;
        ensure_hf_compute_reservation_charge_within_cap(
            &placement.total_reservation_fee,
            &max_compute_reservation_fee,
        )?;
        let sink_account = resolve_fee_sink_account(state_transaction)?;
        let initial_total_charge = base_fee
            .checked_add(&placement.total_reservation_fee)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease initial total charge exceeded the decimal domain",
                    error,
                )
            })?;
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            &initial_total_charge,
            &sink_account,
            state_transaction,
        )?;
        let previous_paid = existing_member
            .as_ref()
            .map(|member| member.total_paid.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_refunded = existing_member
            .as_ref()
            .map(|member| member.total_refunded.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_compute_paid = existing_member
            .as_ref()
            .map(|member| member.total_compute_paid.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_compute_refunded = existing_member
            .as_ref()
            .map(|member| member.total_compute_refunded.clone())
            .unwrap_or_else(Quantity::zero);
        let total_paid = previous_paid.checked_add(&base_fee).map_err(|error| {
            invalid_quantity_arithmetic(
                "HF shared-lease member storage payment total exceeded the decimal domain",
                error,
            )
        })?;
        let total_compute_paid = previous_compute_paid
            .checked_add(&placement.total_reservation_fee)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease member compute payment total exceeded the decimal domain",
                    error,
                )
            })?;
        let mut member = SoraHfSharedLeaseMemberV1 {
            schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            pool_id,
            source_id,
            account_id: authority.clone(),
            status: SoraHfSharedLeaseMemberStatusV1::Active,
            joined_at_ms: now_ms,
            updated_at_ms: now_ms,
            total_paid,
            total_refunded: previous_refunded,
            last_charge: base_fee.clone(),
            total_compute_paid,
            total_compute_refunded: previous_compute_refunded,
            last_compute_charge: placement.total_reservation_fee.clone(),
            service_bindings: std::collections::BTreeSet::new(),
            apartment_bindings: std::collections::BTreeSet::new(),
        };
        bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
        record_hf_shared_lease_pool(state_transaction, pool.clone())?;
        record_hf_placement(state_transaction, placement)?;
        record_hf_shared_lease_member(state_transaction, member)?;
        record_hf_shared_lease_audit_event(
            state_transaction,
            SoraHfSharedLeaseAuditEventV1 {
                schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                sequence: next_soracloud_audit_sequence(state_transaction),
                action: SoraHfSharedLeaseActionV1::CreateWindow,
                pool_id,
                source_id,
                account_id: authority.clone(),
                occurred_at_ms: now_ms,
                active_member_count: 1,
                charged: initial_total_charge,
                refunded: Quantity::zero(),
                lease_expires_at_ms: pool.window_expires_at_ms,
                service_name: Some(service_name.to_string()),
                apartment_name: apartment_name.map(|name| name.to_string()),
            },
        )
    }
}
impl Execute for isi::LeaveSoracloudHfSharedLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::LeaveSoracloudHfSharedLease {
            repo_id,
            resolved_revision,
            storage_class,
            lease_term_ms,
            service_name,
            apartment_name,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let repo_id = parse_hf_repo_id(&repo_id)?;
        let resolved_revision = parse_hf_revision(&resolved_revision)?;
        if lease_term_ms == 0 {
            return Err(invalid_parameter("lease_term_ms must be greater than zero"));
        }
        verify_hf_shared_lease_leave_provenance(
            authority,
            &repo_id,
            &resolved_revision,
            storage_class,
            lease_term_ms,
            service_name.as_ref(),
            apartment_name.as_ref(),
            &provenance,
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        let source_id = hf_source_id(&repo_id, &resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let mut source_record = state_transaction
            .world
            .soracloud_hf_sources
            .get(&source_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "hf source `{repo_id}@{resolved_revision}` is not registered; join first"
                    )
                    .into(),
                )
            })?;
        let mut pool = state_transaction
            .world
            .soracloud_hf_shared_lease_pools
            .get(&pool_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "hf shared lease pool for `{repo_id}@{resolved_revision}` is not active"
                    )
                    .into(),
                )
            })?;
        if pool.window_expires_at_ms <= now_ms
            && matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
            )
        {
            if !promote_hf_shared_lease_queued_window(
                state_transaction,
                &mut pool,
                &mut source_record,
                now_ms,
            )? {
                expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
                pool.active_member_count = 0;
                pool.status = SoraHfSharedLeaseStatusV1::Expired;
                record_hf_shared_lease_pool(state_transaction, pool.clone())?;
                retire_hf_placement_for_pool(
                    state_transaction,
                    &pool_id,
                    now_ms,
                    "lease window expired without a queued next-window sponsor",
                )?;
            }
            pool = state_transaction
                .world
                .soracloud_hf_shared_lease_pools
                .get(&pool_id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        format!(
                            "hf shared lease pool for `{repo_id}@{resolved_revision}` is not active"
                        )
                        .into(),
                    )
                })?;
        }
        let member_key = (pool_id.to_string(), authority.to_string());
        let mut member = state_transaction
            .world
            .soracloud_hf_shared_lease_members
            .get(&member_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "account `{authority}` is not a member of hf shared lease pool `{pool_id}`"
                    )
                    .into(),
                )
            })?;
        if member.status != SoraHfSharedLeaseMemberStatusV1::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("account `{authority}` already left hf shared lease pool `{pool_id}`")
                    .into(),
            ));
        }
        if pool
            .queued_next_window
            .as_ref()
            .is_some_and(|next_window| next_window.sponsor_account_id == *authority)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "account `{authority}` is sponsoring the queued next window for hf shared lease pool `{pool_id}` and cannot leave before it activates"
                )
                .into(),
            ));
        }
        member.status = SoraHfSharedLeaseMemberStatusV1::Left;
        member.updated_at_ms = now_ms;
        member.last_charge = Quantity::zero();
        member.last_compute_charge = Quantity::zero();
        member.service_bindings.clear();
        member.apartment_bindings.clear();
        record_hf_shared_lease_member(state_transaction, member)?;
        let remaining_members = canonical_hf_member_order(state_transaction, &pool_id);
        pool.active_member_count = u32::try_from(remaining_members.len()).unwrap_or(u32::MAX);
        if remaining_members.is_empty() {
            pool.status = SoraHfSharedLeaseStatusV1::Draining;
            // Keep the drain window strictly in the future even when configured to zero so
            // the persisted pool record remains self-consistent if leave happens in the
            // same block as the window creation.
            let drain_grace_ms =
                duration_millis(state_transaction.nexus.hf_shared_leases.drain_grace).max(1);
            pool.window_expires_at_ms = now_ms.saturating_add(drain_grace_ms);
        } else if pool.window_expires_at_ms <= now_ms {
            pool.status = SoraHfSharedLeaseStatusV1::Expired;
        } else {
            pool.status = SoraHfSharedLeaseStatusV1::Active;
        }
        record_hf_shared_lease_pool(state_transaction, pool.clone())?;
        record_hf_shared_lease_audit_event(
            state_transaction,
            SoraHfSharedLeaseAuditEventV1 {
                schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                sequence: next_soracloud_audit_sequence(state_transaction),
                action: SoraHfSharedLeaseActionV1::Leave,
                pool_id,
                source_id,
                account_id: authority.clone(),
                occurred_at_ms: now_ms,
                active_member_count: pool.active_member_count,
                charged: Quantity::zero(),
                refunded: Quantity::zero(),
                lease_expires_at_ms: pool.window_expires_at_ms,
                service_name: service_name.map(|name| name.to_string()),
                apartment_name: apartment_name.map(|name| name.to_string()),
            },
        )
    }
}
impl Execute for isi::RenewSoracloudHfSharedLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RenewSoracloudHfSharedLease {
            repo_id,
            resolved_revision,
            model_name,
            service_name,
            apartment_name,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee,
            resource_profile,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let repo_id = parse_hf_repo_id(&repo_id)?;
        let resolved_revision = parse_hf_revision(&resolved_revision)?;
        let model_name = parse_hf_model_name(&model_name)?;
        if lease_term_ms == 0 {
            return Err(invalid_parameter("lease_term_ms must be greater than zero"));
        }
        if base_fee.is_zero() {
            return Err(invalid_parameter("base_fee must be greater than zero"));
        }
        verify_hf_shared_lease_renew_provenance(
            authority,
            &repo_id,
            &resolved_revision,
            &model_name,
            &service_name,
            apartment_name.as_ref(),
            storage_class,
            lease_term_ms,
            &lease_asset_definition_id,
            &base_fee,
            &provenance,
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        let source_id = hf_source_id(&repo_id, &resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let mut source_record = state_transaction
            .world
            .soracloud_hf_sources
            .get(&source_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "hf source `{repo_id}@{resolved_revision}` is not registered; join first"
                    )
                    .into(),
                )
            })?;
        if source_record.status == SoraHfSourceStatusV1::Failed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "hf source `{repo_id}@{resolved_revision}` is in failed state; fix import failure before renewal"
                )
                .into(),
            ));
        }
        source_record.repo_id = repo_id.clone();
        source_record.resolved_revision = resolved_revision.clone();
        source_record.model_name = model_name.clone();
        let resource_profile = resolve_hf_resource_profile(&mut source_record, resource_profile)?;
        let mut pool = state_transaction
            .world
            .soracloud_hf_shared_lease_pools
            .get(&pool_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "hf shared lease pool for `{repo_id}@{resolved_revision}` does not exist; use join to create it"
                    )
                    .into(),
                )
            })?;
        if pool.window_expires_at_ms <= now_ms
            && matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
            )
        {
            if !promote_hf_shared_lease_queued_window(
                state_transaction,
                &mut pool,
                &mut source_record,
                now_ms,
            )? {
                expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
                pool.active_member_count = 0;
                pool.status = SoraHfSharedLeaseStatusV1::Expired;
                record_hf_shared_lease_pool(state_transaction, pool.clone())?;
                retire_hf_placement_for_pool(
                    state_transaction,
                    &pool_id,
                    now_ms,
                    "lease window expired without a queued next-window sponsor",
                )?;
            }
            pool = state_transaction
                .world
                .soracloud_hf_shared_lease_pools
                .get(&pool_id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        format!(
                            "hf shared lease pool for `{repo_id}@{resolved_revision}` does not exist; use join to create it"
                        )
                        .into(),
                    )
                })?;
        }
        let member_key = (pool_id.to_string(), authority.to_string());
        let existing_member = state_transaction
            .world
            .soracloud_hf_shared_lease_members
            .get(&member_key)
            .cloned();
        if pool.window_expires_at_ms > now_ms && pool.status == SoraHfSharedLeaseStatusV1::Active {
            if pool.queued_next_window.is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "hf shared lease pool `{pool_id}` already has a queued next-window sponsor"
                    )
                    .into(),
                ));
            }
            let mut member = existing_member.clone().ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "account `{authority}` must already be an active member of hf shared lease pool `{pool_id}` to sponsor the next window before expiry"
                    )
                    .into(),
                )
            })?;
            if member.status != SoraHfSharedLeaseMemberStatusV1::Active {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account `{authority}` must already be an active member of hf shared lease pool `{pool_id}` to sponsor the next window before expiry"
                    )
                    .into(),
                ));
            }
            let next_window_started_at_ms = pool.window_expires_at_ms;
            let next_window_expires_at_ms = pool.window_expires_at_ms.saturating_add(lease_term_ms);
            reconcile_expired_model_hosts(state_transaction, now_ms)?;
            let planned_placement = select_hf_placement_for_window(
                state_transaction,
                source_id,
                pool_id,
                &resource_profile,
                next_window_started_at_ms,
                now_ms,
                Some(&pool_id),
            )?;
            source_record.updated_at_ms = now_ms;
            record_hf_source(state_transaction, source_record.clone())?;
            let sink_account = resolve_fee_sink_account(state_transaction)?;
            let queued_total_charge = base_fee
                .checked_add(&planned_placement.total_reservation_fee)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF shared-lease queued renewal charge exceeded the decimal domain",
                        error,
                    )
                })?;
            transfer_hf_shared_lease_amount(
                authority,
                &lease_asset_definition_id,
                &queued_total_charge,
                &sink_account,
                state_transaction,
            )?;
            member.updated_at_ms = now_ms;
            member.total_paid = member.total_paid.checked_add(&base_fee).map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease member storage payment total exceeded the decimal domain",
                    error,
                )
            })?;
            member.last_charge = base_fee.clone();
            member.total_compute_paid = member
                .total_compute_paid
                .checked_add(&planned_placement.total_reservation_fee)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        "HF shared-lease member compute payment total exceeded the decimal domain",
                        error,
                    )
                })?;
            member.last_compute_charge = planned_placement.total_reservation_fee.clone();
            record_hf_shared_lease_member(state_transaction, member)?;
            let next_window = SoraHfSharedLeaseQueuedWindowV1 {
                sponsor_account_id: authority.clone(),
                model_name: model_name.clone(),
                lease_asset_definition_id,
                base_fee: base_fee.clone(),
                compute_reservation_fee: planned_placement.total_reservation_fee.clone(),
                planned_placement,
                sponsored_at_ms: now_ms,
                window_started_at_ms: next_window_started_at_ms,
                window_expires_at_ms: next_window_expires_at_ms,
                service_name: service_name.clone(),
                apartment_name,
            };
            pool.queued_next_window = Some(next_window.clone());
            record_hf_shared_lease_pool(state_transaction, pool.clone())?;
            return record_hf_shared_lease_audit_event(
                state_transaction,
                SoraHfSharedLeaseAuditEventV1 {
                    schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                    sequence: next_soracloud_audit_sequence(state_transaction),
                    action: SoraHfSharedLeaseActionV1::Renew,
                    pool_id,
                    source_id,
                    account_id: authority.clone(),
                    occurred_at_ms: now_ms,
                    active_member_count: pool.active_member_count,
                    charged: queued_total_charge,
                    refunded: Quantity::zero(),
                    lease_expires_at_ms: next_window.window_expires_at_ms,
                    service_name: Some(next_window.service_name.to_string()),
                    apartment_name: next_window.apartment_name.map(|name| name.to_string()),
                },
            );
        }
        expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
        source_record.model_name = model_name;
        source_record.updated_at_ms = now_ms;
        if source_record.status == SoraHfSourceStatusV1::Retired {
            source_record.status = SoraHfSourceStatusV1::PendingImport;
        }
        refresh_hf_source_status_from_generated_service(
            state_transaction,
            &mut source_record,
            &service_name,
        );
        record_hf_source(state_transaction, source_record)?;
        pool.lease_asset_definition_id = lease_asset_definition_id.clone();
        pool.base_fee = base_fee.clone();
        pool.window_started_at_ms = now_ms;
        pool.window_expires_at_ms = now_ms.saturating_add(lease_term_ms);
        pool.active_member_count = 1;
        pool.status = SoraHfSharedLeaseStatusV1::Active;
        pool.queued_next_window = None;
        reconcile_expired_model_hosts(state_transaction, now_ms)?;
        let placement = select_hf_placement_for_window(
            state_transaction,
            source_id,
            pool_id,
            &resource_profile,
            pool.window_started_at_ms,
            now_ms,
            Some(&pool_id),
        )?;
        let sink_account = resolve_fee_sink_account(state_transaction)?;
        let renewed_total_charge = base_fee
            .checked_add(&placement.total_reservation_fee)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease renewal charge exceeded the decimal domain",
                    error,
                )
            })?;
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            &renewed_total_charge,
            &sink_account,
            state_transaction,
        )?;
        record_hf_shared_lease_pool(state_transaction, pool.clone())?;
        let previous_paid = existing_member
            .as_ref()
            .map(|member| member.total_paid.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_refunded = existing_member
            .as_ref()
            .map(|member| member.total_refunded.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_compute_paid = existing_member
            .as_ref()
            .map(|member| member.total_compute_paid.clone())
            .unwrap_or_else(Quantity::zero);
        let previous_compute_refunded = existing_member
            .as_ref()
            .map(|member| member.total_compute_refunded.clone())
            .unwrap_or_else(Quantity::zero);
        let total_paid = previous_paid.checked_add(&base_fee).map_err(|error| {
            invalid_quantity_arithmetic(
                "HF shared-lease member storage payment total exceeded the decimal domain",
                error,
            )
        })?;
        let total_compute_paid = previous_compute_paid
            .checked_add(&placement.total_reservation_fee)
            .map_err(|error| {
                invalid_quantity_arithmetic(
                    "HF shared-lease member compute payment total exceeded the decimal domain",
                    error,
                )
            })?;
        let mut member = SoraHfSharedLeaseMemberV1 {
            schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            pool_id,
            source_id,
            account_id: authority.clone(),
            status: SoraHfSharedLeaseMemberStatusV1::Active,
            joined_at_ms: now_ms,
            updated_at_ms: now_ms,
            total_paid,
            total_refunded: previous_refunded,
            last_charge: base_fee.clone(),
            total_compute_paid,
            total_compute_refunded: previous_compute_refunded,
            last_compute_charge: placement.total_reservation_fee.clone(),
            service_bindings: std::collections::BTreeSet::new(),
            apartment_bindings: std::collections::BTreeSet::new(),
        };
        bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
        record_hf_placement(state_transaction, placement)?;
        record_hf_shared_lease_member(state_transaction, member)?;
        record_hf_shared_lease_audit_event(
            state_transaction,
            SoraHfSharedLeaseAuditEventV1 {
                schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                sequence: next_soracloud_audit_sequence(state_transaction),
                action: SoraHfSharedLeaseActionV1::Renew,
                pool_id,
                source_id,
                account_id: authority.clone(),
                occurred_at_ms: now_ms,
                active_member_count: 1,
                charged: renewed_total_charge,
                refunded: Quantity::zero(),
                lease_expires_at_ms: pool.window_expires_at_ms,
                service_name: Some(service_name.to_string()),
                apartment_name: apartment_name.map(|name| name.to_string()),
            },
        )
    }
}
impl Execute for isi::AdvertiseSoracloudModelHost {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::AdvertiseSoracloudModelHost {
            mut capability,
            provenance,
        } = self;
        require_soracloud_runtime_authority(authority, state_transaction)?;
        require_active_public_lane_validator(authority, state_transaction)?;
        if capability.validator_account_id != *authority {
            return Err(invalid_parameter(
                "model host capability validator_account_id must match the transaction authority",
            ));
        }
        verify_model_host_advertise_provenance(authority, &capability, &provenance)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        if capability.advertised_at_ms == 0 {
            capability.advertised_at_ms = now_ms;
        }
        if capability.schema_version == 0 {
            capability.schema_version = SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1;
        }
        if capability.heartbeat_expires_at_ms <= capability.advertised_at_ms {
            return Err(invalid_parameter(
                "model host capability heartbeat_expires_at_ms must be greater than advertised_at_ms",
            ));
        }
        reconcile_expired_model_hosts(state_transaction, now_ms)?;
        capability
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        if let Some(detail) = model_host_capability_advert_contradiction_detail(
            state_transaction,
            &capability,
            now_ms,
        )? {
            report_model_host_violation(
                state_transaction,
                &capability.validator_account_id,
                SoraModelHostViolationKindV1::AdvertContradiction,
                None,
                Some(detail),
                now_ms,
            )?;
            return Ok(());
        }
        record_model_host_capability(state_transaction, capability.clone())?;
        sync_hf_placements_for_host_capability(state_transaction, &capability, now_ms)
    }
}
impl Execute for isi::HeartbeatSoracloudModelHost {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::HeartbeatSoracloudModelHost {
            validator_account_id,
            heartbeat_expires_at_ms,
            provenance,
        } = self;
        require_soracloud_runtime_authority(authority, state_transaction)?;
        require_active_public_lane_validator(authority, state_transaction)?;
        if validator_account_id != *authority {
            return Err(invalid_parameter(
                "model host heartbeat validator_account_id must match the transaction authority",
            ));
        }
        verify_model_host_heartbeat_provenance(
            authority,
            &validator_account_id,
            heartbeat_expires_at_ms,
            &provenance,
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        if heartbeat_expires_at_ms <= now_ms {
            return Err(invalid_parameter(
                "model host heartbeat_expires_at_ms must be in the future",
            ));
        }
        reconcile_expired_model_hosts(state_transaction, now_ms)?;
        let mut capability = state_transaction
            .world
            .soracloud_model_host_capabilities
            .get(&validator_account_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "model host capability for validator `{validator_account_id}` does not exist"
                    )
                    .into(),
                )
            })?;
        capability.advertised_at_ms = now_ms;
        capability.heartbeat_expires_at_ms = heartbeat_expires_at_ms;
        record_model_host_capability(state_transaction, capability)?;
        refresh_hf_placements_for_host_status(
            state_transaction,
            &validator_account_id,
            SoraHfPlacementHostStatusV1::Warm,
            now_ms,
            None,
        )
    }
}
impl Execute for isi::WithdrawSoracloudModelHost {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::WithdrawSoracloudModelHost {
            validator_account_id,
            provenance,
        } = self;
        require_soracloud_runtime_authority(authority, state_transaction)?;
        require_active_public_lane_validator(authority, state_transaction)?;
        if validator_account_id != *authority {
            return Err(invalid_parameter(
                "model host withdraw validator_account_id must match the transaction authority",
            ));
        }
        verify_model_host_withdraw_provenance(authority, &validator_account_id, &provenance)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        state_transaction
            .world
            .soracloud_model_host_capabilities
            .remove(validator_account_id.clone());
        reconcile_expired_model_hosts(state_transaction, now_ms)?;
        refresh_hf_placements_for_host_status(
            state_transaction,
            &validator_account_id,
            SoraHfPlacementHostStatusV1::Unavailable,
            now_ms,
            Some("assigned host withdrew capability advert"),
        )
    }
}
impl Execute for isi::ReconcileSoracloudModelHosts {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        reconcile_expired_model_hosts(state_transaction, now_ms)
    }
}
impl Execute for isi::AdvertiseSoracloudInrouHost {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::AdvertiseSoracloudInrouHost {
            mut capability,
            provenance,
        } = self;
        require_soracloud_runtime_authority(authority, state_transaction)?;
        require_active_public_lane_validator(authority, state_transaction)?;
        if capability.validator_account_id != *authority {
            return Err(invalid_parameter(
                "Inrou host capability validator_account_id must match the transaction authority",
            ));
        }
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        if capability.schema_version == 0 {
            capability.schema_version = SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1;
        }
        if capability.advertised_at_ms == 0 {
            capability.advertised_at_ms = now_ms;
        }
        if capability.heartbeat_expires_at_ms <= capability.advertised_at_ms {
            return Err(invalid_parameter(
                "Inrou host capability heartbeat_expires_at_ms must be greater than advertised_at_ms",
            ));
        }
        verify_inrou_host_advertise_provenance(authority, &capability, &provenance)?;
        record_inrou_host_capability(state_transaction, capability)?;
        reconcile_inrou_service_placements(state_transaction, now_ms)
    }
}
impl Execute for isi::WithdrawSoracloudInrouHost {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::WithdrawSoracloudInrouHost {
            validator_account_id,
            provenance,
        } = self;
        require_soracloud_runtime_authority(authority, state_transaction)?;
        require_active_public_lane_validator(authority, state_transaction)?;
        if validator_account_id != *authority {
            return Err(invalid_parameter(
                "Inrou host withdraw validator_account_id must match the transaction authority",
            ));
        }
        verify_inrou_host_withdraw_provenance(authority, &validator_account_id, &provenance)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        state_transaction
            .world
            .soracloud_inrou_host_capabilities
            .remove(validator_account_id);
        reconcile_inrou_service_placements(state_transaction, now_ms)
    }
}
impl Execute for isi::ReconcileSoracloudInrouPlacements {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_runtime_authority(authority, state_transaction)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        reconcile_inrou_service_placements(state_transaction, now_ms)
    }
}
impl Execute for isi::ReportSoracloudModelHostViolation {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::ReportSoracloudModelHostViolation {
            validator_account_id,
            kind,
            placement_id,
            detail,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        report_model_host_violation(
            state_transaction,
            &validator_account_id,
            kind,
            placement_id,
            detail,
            now_ms,
        )
    }
}
impl Execute for isi::DeploySoracloudAgentApartment {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::DeploySoracloudAgentApartment {
            manifest,
            lease_ticks,
            autonomy_budget_units,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let payload = encode_agent_deploy_provenance_payload(
            manifest.clone(),
            lease_ticks,
            Some(autonomy_budget_units),
        )
        .map_err(|err| {
            invalid_parameter(format!("failed to encode agent deploy provenance: {err}"))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent deploy provenance signer must match the transaction authority",
            "agent deploy provenance signature verification failed",
        )?;
        manifest
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        if lease_ticks == 0 {
            return Err(invalid_parameter("lease_ticks must be greater than zero"));
        }
        if autonomy_budget_units == 0 {
            return Err(invalid_parameter(
                "autonomy_budget_units must be greater than zero",
            ));
        }
        let apartment_name = manifest.apartment_name.clone();
        let apartment_key = apartment_name.to_string();
        if state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("apartment `{apartment_name}` is already deployed").into(),
            ));
        }
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let manifest_hash = Hash::new(Encode::encode(&manifest));
        let record = SoraAgentApartmentRecordV1 {
            schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest,
            manifest_hash,
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: sequence,
            lease_started_sequence: sequence,
            lease_expires_sequence: sequence.saturating_add(lease_ticks),
            last_renewed_sequence: sequence,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 1,
            process_started_sequence: sequence,
            last_active_sequence: sequence,
            last_checkpoint_sequence: None,
            checkpoint_count: 0,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 0,
                key_sizes: std::collections::BTreeMap::new(),
            },
            revoked_policy_capabilities: std::collections::BTreeSet::new(),
            pending_wallet_requests: std::collections::BTreeMap::new(),
            wallet_daily_spend: std::collections::BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: autonomy_budget_units,
            autonomy_budget_remaining_units: autonomy_budget_units,
            artifact_allowlist: std::collections::BTreeMap::new(),
            autonomy_run_history: Vec::new(),
        };
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::Deploy,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash,
                restart_count: 0,
                signer: provenance.signer,
                request_id: None,
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RenewSoracloudAgentLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RenewSoracloudAgentLease {
            apartment_name,
            lease_ticks,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let payload =
            encode_agent_lease_renew_provenance_payload(apartment_name.as_ref(), lease_ticks)
                .map_err(|err| {
                    invalid_parameter(format!(
                        "failed to encode agent lease renew provenance: {err}"
                    ))
                })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent lease renew provenance signer must match the transaction authority",
            "agent lease renew provenance signature verification failed",
        )?;
        if lease_ticks == 0 {
            return Err(invalid_parameter("lease_ticks must be greater than zero"));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        let base = record.lease_expires_sequence.max(sequence);
        record.lease_expires_sequence = base.saturating_add(lease_ticks);
        record.last_renewed_sequence = sequence;
        record.status = SoraAgentRuntimeStatusV1::Running;
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::LeaseRenew,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: None,
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RestartSoracloudAgentApartment {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RestartSoracloudAgentApartment {
            apartment_name,
            reason,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_reason = reason.trim().to_owned();
        let payload = encode_agent_restart_provenance_payload(
            apartment_name.as_ref(),
            normalized_reason.as_str(),
        )
        .map_err(|err| {
            invalid_parameter(format!("failed to encode agent restart provenance: {err}"))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent restart provenance signer must match the transaction authority",
            "agent restart provenance signature verification failed",
        )?;
        if normalized_reason.is_empty() {
            return Err(invalid_parameter("reason must not be empty"));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before restart",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        record.status = SoraAgentRuntimeStatusV1::Running;
        record.restart_count = record.restart_count.saturating_add(1);
        record.last_restart_sequence = Some(sequence);
        record.last_restart_reason = Some(normalized_reason.clone());
        record.process_generation = record.process_generation.saturating_add(1).max(1);
        record.process_started_sequence = sequence;
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::Restart,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: None,
                asset_definition: None,
                amount: None,
                capability: None,
                reason: Some(normalized_reason),
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RevokeSoracloudAgentPolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RevokeSoracloudAgentPolicy {
            apartment_name,
            capability,
            reason,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_capability = parse_agent_capability_name(&capability)?;
        let normalized_reason = reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .map(ToOwned::to_owned);
        let payload = encode_agent_policy_revoke_provenance_payload(
            apartment_name.as_ref(),
            normalized_capability.as_str(),
            normalized_reason.as_deref(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent policy revoke provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent policy revoke provenance signer must match the transaction authority",
            "agent policy revoke provenance signature verification failed",
        )?;
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        let declared = record
            .manifest
            .policy_capabilities
            .iter()
            .any(|candidate| candidate.as_ref() == normalized_capability.as_str());
        if !declared {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not declare policy capability `{normalized_capability}`"
                )
                .into(),
            ));
        }
        if record
            .revoked_policy_capabilities
            .contains(normalized_capability.as_str())
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "policy capability `{normalized_capability}` is already revoked for apartment `{apartment_name}`"
                )
                .into(),
            ));
        }
        record
            .revoked_policy_capabilities
            .insert(normalized_capability.clone());
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::PolicyRevoked,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: None,
                asset_definition: None,
                amount: None,
                capability: Some(normalized_capability),
                reason: normalized_reason,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RequestSoracloudAgentWalletSpend {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RequestSoracloudAgentWalletSpend {
            apartment_name,
            asset_definition,
            amount,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_asset_definition = asset_definition.trim().to_owned();
        let payload = encode_agent_wallet_spend_provenance_payload(
            apartment_name.as_ref(),
            normalized_asset_definition.as_str(),
            &amount,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent wallet spend provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent wallet spend provenance signer must match the transaction authority",
            "agent wallet spend provenance signature verification failed",
        )?;
        if normalized_asset_definition.is_empty() {
            return Err(invalid_parameter("asset_definition must not be empty"));
        }
        if amount.is_zero() {
            return Err(invalid_parameter("amount must be greater than zero"));
        }
        let canonical_asset_definition_id = resolve_agent_asset_definition_literal(
            state_transaction,
            &normalized_asset_definition,
        )?;
        let asset_numeric_spec = state_transaction
            .numeric_spec_for(&canonical_asset_definition_id)
            .map_err(InstructionExecutionError::from)?;
        assert_numeric_spec_with(amount.as_numeric(), asset_numeric_spec)?;
        let canonical_asset_definition = canonical_asset_definition_id.to_string();
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&record, "wallet.sign") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `wallet.sign` capability"
                )
                .into(),
            ));
        }
        let spend_limit = agent_spend_limit_for_asset_definition(
            state_transaction,
            &record,
            &canonical_asset_definition,
        )?;
        assert_numeric_spec_with(spend_limit.max_per_tx.as_numeric(), asset_numeric_spec)?;
        assert_numeric_spec_with(spend_limit.max_per_day.as_numeric(), asset_numeric_spec)?;
        if amount > spend_limit.max_per_tx {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "requested amount {amount} exceeds max_per_tx {} for asset `{canonical_asset_definition}`",
                    spend_limit.max_per_tx
                )
                .into(),
            ));
        }
        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(&record, &canonical_asset_definition, day_bucket);
        let projected_day_spent = current_day_spent.checked_add(&amount).map_err(|error| {
            invalid_quantity_arithmetic(
                &format!("wallet daily spend overflow for apartment `{apartment_name}`"),
                error,
            )
        })?;
        if projected_day_spent > spend_limit.max_per_day {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "projected daily spend {projected_day_spent} exceeds max_per_day {} for asset `{canonical_asset_definition}`",
                    spend_limit.max_per_day
                )
                .into(),
            ));
        }
        let request_id = format!("{apartment_key}:wallet:{sequence}");
        let action = if agent_policy_capability_active(&record, "wallet.auto_approve") {
            wallet_record_spend(
                &mut record,
                &canonical_asset_definition,
                day_bucket,
                projected_day_spent,
            );
            SoraAgentApartmentActionV1::WalletSpendApproved
        } else {
            record.pending_wallet_requests.insert(
                request_id.clone(),
                SoraAgentWalletSpendRequestV1 {
                    request_id: request_id.clone(),
                    asset_definition: canonical_asset_definition.clone(),
                    amount: amount.clone(),
                    created_sequence: sequence,
                },
            );
            SoraAgentApartmentActionV1::WalletSpendRequested
        };
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: Some(request_id),
                asset_definition: Some(canonical_asset_definition),
                amount: Some(amount),
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::ApproveSoracloudAgentWalletSpend {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::ApproveSoracloudAgentWalletSpend {
            apartment_name,
            request_id,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_request_id = request_id.trim().to_owned();
        let payload = encode_agent_wallet_approve_provenance_payload(
            apartment_name.as_ref(),
            normalized_request_id.as_str(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent wallet approve provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent wallet approve provenance signer must match the transaction authority",
            "agent wallet approve provenance signature verification failed",
        )?;
        if normalized_request_id.is_empty() {
            return Err(invalid_parameter("request_id must not be empty"));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&record, "wallet.sign") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `wallet.sign` capability"
                )
                .into(),
            ));
        }
        let pending = record
            .pending_wallet_requests
            .remove(normalized_request_id.as_str())
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "wallet request `{normalized_request_id}` is not pending for apartment `{apartment_name}`"
                    )
                    .into(),
                )
            })?;
        let canonical_asset_definition_id =
            resolve_agent_asset_definition_literal(state_transaction, &pending.asset_definition)?;
        let asset_numeric_spec = state_transaction
            .numeric_spec_for(&canonical_asset_definition_id)
            .map_err(InstructionExecutionError::from)?;
        assert_numeric_spec_with(pending.amount.as_numeric(), asset_numeric_spec)?;
        let spend_limit = agent_spend_limit_for_asset_definition(
            state_transaction,
            &record,
            &pending.asset_definition,
        )?;
        assert_numeric_spec_with(spend_limit.max_per_tx.as_numeric(), asset_numeric_spec)?;
        assert_numeric_spec_with(spend_limit.max_per_day.as_numeric(), asset_numeric_spec)?;
        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(&record, &pending.asset_definition, day_bucket);
        let projected_day_spent =
            current_day_spent
                .checked_add(&pending.amount)
                .map_err(|error| {
                    invalid_quantity_arithmetic(
                        &format!("wallet daily spend overflow for apartment `{apartment_name}`"),
                        error,
                    )
                })?;
        if projected_day_spent > spend_limit.max_per_day {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "projected daily spend {projected_day_spent} exceeds max_per_day {} for asset `{}`",
                    spend_limit.max_per_day,
                    pending.asset_definition
                )
                .into(),
            ));
        }
        wallet_record_spend(
            &mut record,
            &pending.asset_definition,
            day_bucket,
            projected_day_spent,
        );
        touch_agent_runtime_activity(&mut record, sequence);
        let event_request_id = pending.request_id.clone();
        let event_asset_definition = pending.asset_definition.clone();
        let event_amount = pending.amount.clone();
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::WalletSpendApproved,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: Some(event_request_id),
                asset_definition: Some(event_asset_definition),
                amount: Some(event_amount),
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::EnqueueSoracloudAgentMessage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::EnqueueSoracloudAgentMessage {
            from_apartment,
            to_apartment,
            channel,
            payload,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_channel = channel.trim().to_owned();
        let normalized_payload = payload.trim().to_owned();
        let encoded = encode_agent_message_send_provenance_payload(
            from_apartment.as_ref(),
            to_apartment.as_ref(),
            normalized_channel.as_str(),
            normalized_payload.as_str(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent message send provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            encoded,
            "agent message send provenance signer must match the transaction authority",
            "agent message send provenance signature verification failed",
        )?;
        if normalized_channel.is_empty() {
            return Err(invalid_parameter("channel must not be empty"));
        }
        if normalized_payload.is_empty() {
            return Err(invalid_parameter("payload must not be empty"));
        }
        if normalized_payload.len() > AGENT_MAILBOX_MAX_PAYLOAD_BYTES {
            return Err(invalid_parameter(format!(
                "payload exceeds max mailbox payload bytes ({AGENT_MAILBOX_MAX_PAYLOAD_BYTES})"
            )));
        }
        let from_key = from_apartment.to_string();
        let to_key = to_apartment.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let message_id = format!("{to_key}:mail:{sequence}");
        let payload_hash = Hash::new(normalized_payload.as_bytes());
        let mut sender = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&from_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{from_apartment}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&sender, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "sender apartment `{from_apartment}` lease expired at sequence {}; renew before messaging",
                    sender.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&sender, "agent.mailbox.send") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{from_apartment}` does not have active `agent.mailbox.send` capability"
                )
                .into(),
            ));
        }
        if from_key == to_key {
            if !agent_policy_capability_active(&sender, "agent.mailbox.receive") {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "apartment `{to_apartment}` does not have active `agent.mailbox.receive` capability"
                    )
                    .into(),
                ));
            }
            sender.mailbox_queue.push(SoraAgentMailboxMessageV1 {
                message_id: message_id.clone(),
                from_apartment: from_key.clone(),
                channel: normalized_channel.clone(),
                payload: normalized_payload,
                payload_hash,
                enqueued_sequence: sequence,
            });
            touch_agent_runtime_activity(&mut sender, sequence);
            let event_status =
                agent_runtime_status_for_sequence(&sender, sequence.saturating_add(1));
            let lease_expires_sequence = sender.lease_expires_sequence;
            let manifest_hash = sender.manifest_hash;
            let restart_count = sender.restart_count;
            record_agent_apartment(state_transaction, from_key.clone(), sender)?;
            return record_agent_apartment_audit_event(
                state_transaction,
                SoraAgentApartmentAuditEventV1 {
                    schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                    sequence,
                    action: SoraAgentApartmentActionV1::MessageEnqueued,
                    apartment_name: to_apartment,
                    status: event_status,
                    lease_expires_sequence,
                    manifest_hash,
                    restart_count,
                    signer: provenance.signer,
                    request_id: Some(message_id),
                    asset_definition: None,
                    amount: None,
                    capability: None,
                    reason: None,
                    from_apartment: Some(from_key.clone()),
                    to_apartment: Some(from_key),
                    channel: Some(normalized_channel),
                    payload_hash: Some(payload_hash),
                    artifact_hash: None,
                    provenance_hash: None,
                    run_id: None,
                    run_label: None,
                    budget_units: None,
                    service_name: None,
                    service_version: None,
                    handler_name: None,
                    result_commitment: None,
                    runtime_receipt_id: None,
                    journal_artifact_hash: None,
                    checkpoint_artifact_hash: None,
                    succeeded: None,
                },
            );
        }
        let mut recipient = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&to_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{to_apartment}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&recipient, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "recipient apartment `{to_apartment}` lease expired at sequence {}; renew before messaging",
                    recipient.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&recipient, "agent.mailbox.receive") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{to_apartment}` does not have active `agent.mailbox.receive` capability"
                )
                .into(),
            ));
        }
        recipient.mailbox_queue.push(SoraAgentMailboxMessageV1 {
            message_id: message_id.clone(),
            from_apartment: from_key.clone(),
            channel: normalized_channel.clone(),
            payload: normalized_payload,
            payload_hash,
            enqueued_sequence: sequence,
        });
        touch_agent_runtime_activity(&mut sender, sequence);
        touch_agent_runtime_activity(&mut recipient, sequence);
        let event_status =
            agent_runtime_status_for_sequence(&recipient, sequence.saturating_add(1));
        let lease_expires_sequence = recipient.lease_expires_sequence;
        let manifest_hash = recipient.manifest_hash;
        let restart_count = recipient.restart_count;
        record_agent_apartment(state_transaction, from_key.clone(), sender)?;
        record_agent_apartment(state_transaction, to_key.clone(), recipient)?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::MessageEnqueued,
                apartment_name: to_apartment,
                status: event_status,
                lease_expires_sequence,
                manifest_hash,
                restart_count,
                signer: provenance.signer,
                request_id: Some(message_id),
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: Some(from_key),
                to_apartment: Some(to_key),
                channel: Some(normalized_channel),
                payload_hash: Some(payload_hash),
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::AcknowledgeSoracloudAgentMessage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::AcknowledgeSoracloudAgentMessage {
            apartment_name,
            message_id,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_message_id = message_id.trim().to_owned();
        let payload = encode_agent_message_ack_provenance_payload(
            apartment_name.as_ref(),
            normalized_message_id.as_str(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent message ack provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent message ack provenance signer must match the transaction authority",
            "agent message ack provenance signature verification failed",
        )?;
        if normalized_message_id.is_empty() {
            return Err(invalid_parameter("message_id must not be empty"));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before mailbox actions",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&record, "agent.mailbox.receive") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `agent.mailbox.receive` capability"
                )
                .into(),
            ));
        }
        let message_index = record
            .mailbox_queue
            .iter()
            .position(|message| message.message_id == normalized_message_id)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "mailbox message `{normalized_message_id}` is not queued for apartment `{apartment_name}`"
                    )
                    .into(),
                )
            })?;
        let message = record.mailbox_queue.remove(message_index);
        touch_agent_runtime_activity(&mut record, sequence);
        let from_apartment = message.from_apartment.clone();
        let channel = message.channel.clone();
        let payload_hash = message.payload_hash;
        let event_message_id = message.message_id.clone();
        record_agent_apartment(state_transaction, apartment_key.clone(), record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::MessageAcknowledged,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: Some(event_message_id),
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: Some(from_apartment),
                to_apartment: Some(apartment_key),
                channel: Some(channel),
                payload_hash: Some(payload_hash),
                artifact_hash: None,
                provenance_hash: None,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::AllowSoracloudAgentAutonomyArtifact {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::AllowSoracloudAgentAutonomyArtifact {
            apartment_name,
            artifact_hash,
            provenance_hash,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_artifact_hash = normalize_agent_hash_like("artifact_hash", &artifact_hash)?;
        let normalized_provenance_hash =
            normalize_optional_agent_hash_like("provenance_hash", provenance_hash.as_deref())?;
        let payload = encode_agent_artifact_allow_provenance_payload(
            apartment_name.as_ref(),
            normalized_artifact_hash.as_str(),
            normalized_provenance_hash.as_deref(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent artifact allow provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent artifact allow provenance signer must match the transaction authority",
            "agent artifact allow provenance signature verification failed",
        )?;
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !(agent_policy_capability_active(&record, "governance.audit")
            || agent_policy_capability_active(&record, "agent.autonomy.allow"))
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `governance.audit` or `agent.autonomy.allow` capability"
                )
                .into(),
            ));
        }
        if record
            .artifact_allowlist
            .get(&normalized_artifact_hash)
            .is_some_and(|rule| rule.provenance_hash == normalized_provenance_hash)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact `{normalized_artifact_hash}` is already allowlisted for apartment `{apartment_name}` with the same provenance rule"
                )
                .into(),
            ));
        }
        record.artifact_allowlist.insert(
            normalized_artifact_hash.clone(),
            SoraAgentArtifactAllowRuleV1 {
                artifact_hash: normalized_artifact_hash.clone(),
                provenance_hash: normalized_provenance_hash.clone(),
                added_sequence: sequence,
            },
        );
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::ArtifactAllowed,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: None,
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: Some(normalized_artifact_hash),
                provenance_hash: normalized_provenance_hash,
                run_id: None,
                run_label: None,
                budget_units: None,
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RunSoracloudAgentAutonomy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RunSoracloudAgentAutonomy {
            apartment_name,
            artifact_hash,
            provenance_hash,
            budget_units,
            run_label,
            workflow_input_json,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_artifact_hash = normalize_agent_hash_like("artifact_hash", &artifact_hash)?;
        let normalized_provenance_hash =
            normalize_optional_agent_hash_like("provenance_hash", provenance_hash.as_deref())?;
        let normalized_run_label = normalize_agent_run_label(&run_label)?;
        let normalized_workflow_input_json =
            normalize_optional_agent_workflow_input_json(workflow_input_json.as_deref())?;
        let payload = encode_agent_autonomy_run_provenance_payload(
            apartment_name.as_ref(),
            normalized_artifact_hash.as_str(),
            normalized_provenance_hash.as_deref(),
            budget_units,
            normalized_run_label.as_str(),
            normalized_workflow_input_json.as_deref(),
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "failed to encode agent autonomy run provenance: {err}"
            ))
        })?;
        verify_provenance_payload(
            authority,
            &provenance,
            payload,
            "agent autonomy run provenance signer must match the transaction authority",
            "agent autonomy run provenance signature verification failed",
        )?;
        if budget_units == 0 {
            return Err(invalid_parameter("budget_units must be greater than zero"));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        if agent_runtime_status_for_sequence(&record, sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                    record.lease_expires_sequence
                )
                .into(),
            ));
        }
        if !agent_policy_capability_active(&record, "agent.autonomy.run") {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `agent.autonomy.run` capability"
                )
                .into(),
            ));
        }
        let allow_rule = record
            .artifact_allowlist
            .get(&normalized_artifact_hash)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "artifact `{normalized_artifact_hash}` is not allowlisted for apartment `{apartment_name}`"
                    )
                    .into(),
                )
            })?;
        if let Some(expected_provenance) = allow_rule.provenance_hash.as_deref() {
            let provided_provenance = normalized_provenance_hash.as_deref().ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "artifact `{normalized_artifact_hash}` requires provenance_hash `{expected_provenance}`"
                    )
                    .into(),
                )
            })?;
            if provided_provenance != expected_provenance {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "artifact `{normalized_artifact_hash}` provenance mismatch: expected `{expected_provenance}`, got `{provided_provenance}`"
                    )
                    .into(),
                ));
            }
        }
        if budget_units > record.autonomy_budget_remaining_units {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "requested budget {budget_units} exceeds remaining autonomy budget {} for apartment `{apartment_name}`",
                    record.autonomy_budget_remaining_units
                )
                .into(),
            ));
        }
        let run_id = format!("{apartment_key}:autonomy:{sequence}");
        let request_commitment = derive_agent_autonomy_request_commitment(
            apartment_name.as_ref(),
            normalized_artifact_hash.as_str(),
            normalized_provenance_hash.as_deref(),
            budget_units,
            &run_id,
            normalized_run_label.as_str(),
            normalized_workflow_input_json.as_deref(),
            record.process_generation,
        );
        let checkpoint_key = autonomy_checkpoint_key(&apartment_key, &run_id);
        let checkpoint_value_size = autonomy_checkpoint_value_size(
            &normalized_artifact_hash,
            normalized_provenance_hash.as_deref(),
            &normalized_run_label,
            budget_units,
            normalized_workflow_input_json.as_deref(),
        );
        let projected_total = projected_agent_persistent_state_total_bytes(
            &record,
            &checkpoint_key,
            checkpoint_value_size,
        )?;
        if projected_total > record.manifest.state_quota_bytes.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "autonomy checkpoint would exceed apartment `{apartment_name}` state_quota_bytes {}",
                    record.manifest.state_quota_bytes
                )
                .into(),
            ));
        }
        record.autonomy_budget_remaining_units = record
            .autonomy_budget_remaining_units
            .saturating_sub(budget_units);
        record
            .autonomy_run_history
            .push(SoraAgentAutonomyRunRecordV1 {
                run_id: run_id.clone(),
                artifact_hash: normalized_artifact_hash.clone(),
                provenance_hash: normalized_provenance_hash.clone(),
                budget_units,
                run_label: normalized_run_label.clone(),
                workflow_input_json: normalized_workflow_input_json.clone(),
                approved_process_generation: record.process_generation,
                request_commitment,
                approved_sequence: sequence,
            });
        record.persistent_state.total_bytes = projected_total;
        record
            .persistent_state
            .key_sizes
            .insert(checkpoint_key, checkpoint_value_size);
        record.last_checkpoint_sequence = Some(sequence);
        record.checkpoint_count = record.checkpoint_count.saturating_add(1);
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::AutonomyRunApproved,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: provenance.signer,
                request_id: Some(run_id.clone()),
                asset_definition: None,
                amount: None,
                capability: None,
                reason: None,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: normalized_workflow_input_json
                    .as_ref()
                    .map(|payload| Hash::new(payload.as_bytes())),
                artifact_hash: Some(normalized_artifact_hash),
                provenance_hash: normalized_provenance_hash,
                run_id: Some(run_id),
                run_label: Some(normalized_run_label),
                budget_units: Some(budget_units),
                service_name: None,
                service_version: None,
                handler_name: None,
                result_commitment: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                succeeded: None,
            },
        )
    }
}
impl Execute for isi::RecordSoracloudAgentAutonomyExecution {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::RecordSoracloudAgentAutonomyExecution {
            apartment_name,
            run_id,
            process_generation,
            succeeded,
            result_commitment,
            service_name,
            service_version,
            handler_name,
            runtime_receipt_id,
            journal_artifact_hash,
            checkpoint_artifact_hash,
            error,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        if process_generation == 0 {
            return Err(invalid_parameter(
                "process_generation must be greater than zero",
            ));
        }
        let normalized_run_id = run_id.trim();
        if normalized_run_id.is_empty() {
            return Err(invalid_parameter("run_id must not be empty"));
        }
        if let Some(service_version) = service_version.as_deref()
            && service_version.trim().is_empty()
        {
            return Err(invalid_parameter(
                "service_version must not be empty when provided",
            ));
        }
        if let Some(error) = error.as_deref()
            && error.trim().is_empty()
        {
            return Err(invalid_parameter("error must not be empty when provided"));
        }
        if succeeded && error.is_some() {
            return Err(invalid_parameter(
                "successful autonomy execution must not include an error",
            ));
        }
        if !succeeded && error.is_none() {
            return Err(invalid_parameter(
                "failed autonomy execution must include an error",
            ));
        }
        let apartment_key = apartment_name.to_string();
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let mut record = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("apartment `{apartment_name}` is not deployed").into(),
                )
            })?;
        let run = record
            .autonomy_run_history
            .iter()
            .find(|run| run.run_id == normalized_run_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "apartment `{apartment_name}` does not contain approved run `{normalized_run_id}`"
                    )
                    .into(),
                )
            })?;
        if record.process_generation != process_generation {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` process generation {} does not match execution generation {process_generation}",
                    record.process_generation
                )
                .into(),
            ));
        }
        if run.approved_process_generation != process_generation {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "run `{normalized_run_id}` for apartment `{apartment_name}` was approved for generation {}, not {process_generation}",
                    run.approved_process_generation
                )
                .into(),
            ));
        }
        touch_agent_runtime_activity(&mut record, sequence);
        record_agent_apartment(state_transaction, apartment_key, record.clone())?;
        record_agent_apartment_audit_event(
            state_transaction,
            SoraAgentApartmentAuditEventV1 {
                schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraAgentApartmentActionV1::AutonomyRunExecuted,
                apartment_name,
                status: agent_runtime_status_for_sequence(&record, sequence.saturating_add(1)),
                lease_expires_sequence: record.lease_expires_sequence,
                manifest_hash: record.manifest_hash,
                restart_count: record.restart_count,
                signer: single_signatory_authority(authority)?.clone(),
                request_id: Some(normalized_run_id.to_owned()),
                asset_definition: None,
                amount: None,
                capability: None,
                reason: error,
                from_apartment: None,
                to_apartment: None,
                channel: None,
                payload_hash: None,
                artifact_hash: Some(run.artifact_hash),
                provenance_hash: run.provenance_hash,
                run_id: Some(normalized_run_id.to_owned()),
                run_label: Some(run.run_label),
                budget_units: Some(run.budget_units),
                service_name: service_name.map(|value| value.to_string()),
                service_version,
                handler_name: handler_name.map(|value| value.to_string()),
                result_commitment: Some(result_commitment),
                runtime_receipt_id,
                journal_artifact_hash,
                checkpoint_artifact_hash,
                succeeded: Some(succeeded),
            },
        )
    }
}
impl Execute for isi::StartSoracloudTrainingJob {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&self.model_name)?;
        let job_id = parse_training_job_id(&self.job_id)?;
        verify_training_job_start_provenance(
            authority,
            &self.service_name,
            &model_name,
            &job_id,
            self.worker_group_size,
            self.target_steps,
            self.checkpoint_interval_steps,
            self.max_retries,
            self.step_compute_units,
            self.compute_budget_units,
            self.storage_budget_bytes,
            &self.provenance,
        )?;
        if self.worker_group_size == 0 || self.worker_group_size > TRAINING_MAX_WORKER_GROUP_SIZE {
            return Err(invalid_parameter(format!(
                "worker_group_size must be within 1..={TRAINING_MAX_WORKER_GROUP_SIZE}"
            )));
        }
        if self.target_steps == 0 {
            return Err(invalid_parameter("target_steps must be greater than zero"));
        }
        if self.checkpoint_interval_steps == 0 {
            return Err(invalid_parameter(
                "checkpoint_interval_steps must be greater than zero",
            ));
        }
        if self.checkpoint_interval_steps > self.target_steps {
            return Err(invalid_parameter(
                "checkpoint_interval_steps must not exceed target_steps",
            ));
        }
        if self.max_retries > TRAINING_MAX_RETRIES {
            return Err(invalid_parameter(format!(
                "max_retries must be within 0..={TRAINING_MAX_RETRIES}"
            )));
        }
        if self.step_compute_units == 0 {
            return Err(invalid_parameter(
                "step_compute_units must be greater than zero",
            ));
        }
        if self.compute_budget_units == 0 {
            return Err(invalid_parameter(
                "compute_budget_units must be greater than zero",
            ));
        }
        if self.storage_budget_bytes == 0 {
            return Err(invalid_parameter(
                "storage_budget_bytes must be greater than zero",
            ));
        }
        let minimum_step_units = self
            .step_compute_units
            .checked_mul(u64::from(self.worker_group_size))
            .ok_or_else(|| {
                invalid_parameter("step_compute_units * worker_group_size overflows u64")
            })?;
        if self.compute_budget_units < minimum_step_units {
            return Err(invalid_parameter(format!(
                "compute_budget_units must cover at least one worker-group step ({minimum_step_units})"
            )));
        }
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_key = (self.service_name.as_ref().to_owned(), job_id.clone());
        if state_transaction
            .world
            .soracloud_training_jobs
            .get(&job_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{job_id}` already exists for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_record = SoraTrainingJobRecordV1 {
            schema_version: SORA_TRAINING_JOB_RECORD_VERSION_V1,
            service_name: self.service_name.clone(),
            service_version: deployment.current_service_version.clone(),
            model_name: model_name.clone(),
            job_id: job_id.clone(),
            status: SoraTrainingJobStatusV1::Running,
            worker_group_size: self.worker_group_size,
            target_steps: self.target_steps,
            completed_steps: 0,
            checkpoint_interval_steps: self.checkpoint_interval_steps,
            last_checkpoint_step: None,
            checkpoint_count: 0,
            retry_count: 0,
            max_retries: self.max_retries,
            step_compute_units: self.step_compute_units,
            compute_budget_units: self.compute_budget_units,
            compute_consumed_units: 0,
            storage_budget_bytes: self.storage_budget_bytes,
            storage_consumed_bytes: 0,
            latest_metrics_hash: None,
            last_failure_reason: None,
            created_sequence: sequence,
            updated_sequence: sequence,
        };
        record_training_job(state_transaction, job_record.clone())?;
        record_training_job_audit_event(
            state_transaction,
            SoraTrainingJobAuditEventV1 {
                schema_version: SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraTrainingJobActionV1::Start,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name,
                job_id,
                status: job_record.status,
                completed_steps: job_record.completed_steps,
                checkpoint_count: job_record.checkpoint_count,
                retry_count: job_record.retry_count,
                compute_consumed_units: job_record.compute_consumed_units,
                storage_consumed_bytes: job_record.storage_consumed_bytes,
                last_checkpoint_step: job_record.last_checkpoint_step,
                latest_metrics_hash: job_record.latest_metrics_hash,
                last_failure_reason: job_record.last_failure_reason,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::CheckpointSoracloudTrainingJob {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let job_id = parse_training_job_id(&self.job_id)?;
        verify_training_job_checkpoint_provenance(
            authority,
            &self.service_name,
            &job_id,
            self.completed_step,
            self.checkpoint_size_bytes,
            self.metrics_hash,
            &self.provenance,
        )?;
        if self.completed_step == 0 {
            return Err(invalid_parameter(
                "completed_step must be greater than zero",
            ));
        }
        if self.checkpoint_size_bytes == 0 {
            return Err(invalid_parameter(
                "checkpoint_size_bytes must be greater than zero",
            ));
        }
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_key = (self.service_name.as_ref().to_owned(), job_id.clone());
        let Some(mut job_record) = state_transaction
            .world
            .soracloud_training_jobs
            .get(&job_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{job_id}` not found for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if matches!(
            job_record.status,
            SoraTrainingJobStatusV1::Completed | SoraTrainingJobStatusV1::Exhausted
        ) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{job_id}` is not accepting checkpoints in {:?} status",
                    job_record.status
                )
                .into(),
            ));
        }
        if self.completed_step <= job_record.completed_steps {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "completed_step {} must be greater than current completed_steps {}",
                    self.completed_step, job_record.completed_steps
                )
                .into(),
            ));
        }
        if self.completed_step > job_record.target_steps {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "completed_step {} exceeds target_steps {}",
                    self.completed_step, job_record.target_steps
                )
                .into(),
            ));
        }
        if self.completed_step != job_record.target_steps
            && self.completed_step % job_record.checkpoint_interval_steps != 0
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "completed_step {} must align with checkpoint_interval_steps {} (or equal target_steps {})",
                    self.completed_step,
                    job_record.checkpoint_interval_steps,
                    job_record.target_steps
                )
                .into(),
            ));
        }
        let delta_steps = self.completed_step - job_record.completed_steps;
        let checkpoint_compute_units = u64::from(delta_steps)
            .checked_mul(job_record.step_compute_units)
            .and_then(|value| value.checked_mul(u64::from(job_record.worker_group_size)))
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "training checkpoint compute-cost calculation overflowed u64".into(),
                )
            })?;
        let next_compute_total = job_record
            .compute_consumed_units
            .checked_add(checkpoint_compute_units)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "training compute consumption overflowed u64".into(),
                )
            })?;
        if next_compute_total > job_record.compute_budget_units {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training checkpoint would exceed compute budget {}",
                    job_record.compute_budget_units
                )
                .into(),
            ));
        }
        let next_storage_total = job_record
            .storage_consumed_bytes
            .checked_add(self.checkpoint_size_bytes)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "training storage consumption overflowed u64".into(),
                )
            })?;
        if next_storage_total > job_record.storage_budget_bytes {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training checkpoint would exceed storage budget {}",
                    job_record.storage_budget_bytes
                )
                .into(),
            ));
        }
        job_record.service_version = deployment.current_service_version.clone();
        job_record.compute_consumed_units = next_compute_total;
        job_record.storage_consumed_bytes = next_storage_total;
        job_record.completed_steps = self.completed_step;
        job_record.checkpoint_count = job_record.checkpoint_count.saturating_add(1);
        job_record.last_checkpoint_step = Some(self.completed_step);
        job_record.latest_metrics_hash = Some(self.metrics_hash);
        job_record.last_failure_reason = None;
        job_record.status = if job_record.completed_steps >= job_record.target_steps {
            SoraTrainingJobStatusV1::Completed
        } else {
            SoraTrainingJobStatusV1::Running
        };
        job_record.updated_sequence = sequence;
        record_training_job(state_transaction, job_record.clone())?;
        record_training_job_audit_event(
            state_transaction,
            SoraTrainingJobAuditEventV1 {
                schema_version: SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraTrainingJobActionV1::Checkpoint,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name: job_record.model_name,
                job_id,
                status: job_record.status,
                completed_steps: job_record.completed_steps,
                checkpoint_count: job_record.checkpoint_count,
                retry_count: job_record.retry_count,
                compute_consumed_units: job_record.compute_consumed_units,
                storage_consumed_bytes: job_record.storage_consumed_bytes,
                last_checkpoint_step: job_record.last_checkpoint_step,
                latest_metrics_hash: job_record.latest_metrics_hash,
                last_failure_reason: job_record.last_failure_reason,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RetrySoracloudTrainingJob {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let job_id = parse_training_job_id(&self.job_id)?;
        let reason = normalize_training_reason(&self.reason)?;
        verify_training_job_retry_provenance(
            authority,
            &self.service_name,
            &job_id,
            &reason,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_key = (self.service_name.as_ref().to_owned(), job_id.clone());
        let Some(mut job_record) = state_transaction
            .world
            .soracloud_training_jobs
            .get(&job_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{job_id}` not found for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if job_record.status == SoraTrainingJobStatusV1::Completed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("training job `{job_id}` is already completed").into(),
            ));
        }
        if job_record.status == SoraTrainingJobStatusV1::Exhausted {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("training job `{job_id}` retry budget is exhausted").into(),
            ));
        }
        if job_record.retry_count >= job_record.max_retries {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{job_id}` cannot retry because retry_count {} reached max_retries {}",
                    job_record.retry_count, job_record.max_retries
                )
                .into(),
            ));
        }
        job_record.service_version = deployment.current_service_version.clone();
        job_record.retry_count = job_record.retry_count.saturating_add(1);
        job_record.status = SoraTrainingJobStatusV1::RetryPending;
        job_record.last_failure_reason = Some(reason.clone());
        job_record.updated_sequence = sequence;
        record_training_job(state_transaction, job_record.clone())?;
        record_training_job_audit_event(
            state_transaction,
            SoraTrainingJobAuditEventV1 {
                schema_version: SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraTrainingJobActionV1::Retry,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name: job_record.model_name,
                job_id,
                status: job_record.status,
                completed_steps: job_record.completed_steps,
                checkpoint_count: job_record.checkpoint_count,
                retry_count: job_record.retry_count,
                compute_consumed_units: job_record.compute_consumed_units,
                storage_consumed_bytes: job_record.storage_consumed_bytes,
                last_checkpoint_step: job_record.last_checkpoint_step,
                latest_metrics_hash: job_record.latest_metrics_hash,
                last_failure_reason: job_record.last_failure_reason,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RegisterSoracloudModelArtifact {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&self.model_name)?;
        let training_job_id = parse_training_job_id(&self.training_job_id)?;
        let dataset_ref = parse_model_weight_dataset_ref(&self.dataset_ref)?;
        verify_model_artifact_register_provenance(
            authority,
            &self.service_name,
            &model_name,
            &training_job_id,
            self.weight_artifact_hash,
            &dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_key = (
            self.service_name.as_ref().to_owned(),
            training_job_id.clone(),
        );
        let Some(job_record) = state_transaction
            .world
            .soracloud_training_jobs
            .get(&job_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{training_job_id}` not found for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if job_record.model_name != model_name {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    job_record.model_name
                )
                .into(),
            ));
        }
        if job_record.status != SoraTrainingJobStatusV1::Completed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("training job `{training_job_id}` is not completed").into(),
            ));
        }
        let artifact_key = (
            self.service_name.as_ref().to_owned(),
            training_job_id.clone(),
        );
        if state_transaction
            .world
            .soracloud_model_artifacts
            .get(&artifact_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact metadata for training job `{training_job_id}` already registered for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        }
        record_model_artifact(
            state_transaction,
            SoraModelArtifactRecordV1 {
                schema_version: SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                service_name: self.service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                artifact_id: training_job_id.clone(),
                training_job_id: training_job_id.clone(),
                weight_version: None,
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::TrainingJob,
                    id: training_job_id.clone(),
                }),
                weight_artifact_hash: self.weight_artifact_hash,
                dataset_ref,
                training_config_hash: self.training_config_hash,
                reproducibility_hash: self.reproducibility_hash,
                provenance_attestation_hash: self.provenance_attestation_hash,
                registered_sequence: sequence,
                consumed_by_version: None,
                chunk_manifest_root: None,
            },
        )?;
        record_model_artifact_audit_event(
            state_transaction,
            SoraModelArtifactAuditEventV1 {
                schema_version: SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelArtifactActionV1::Register,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name,
                training_job_id,
                consumed_by_version: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RegisterSoracloudModelWeight {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&self.model_name)?;
        let weight_version = parse_model_weight_version(&self.weight_version)?;
        let training_job_id = parse_training_job_id(&self.training_job_id)?;
        let parent_version = self
            .parent_version
            .as_deref()
            .map(parse_model_weight_version)
            .transpose()?;
        let dataset_ref = parse_model_weight_dataset_ref(&self.dataset_ref)?;
        verify_model_weight_register_provenance(
            authority,
            &self.service_name,
            &model_name,
            &weight_version,
            &training_job_id,
            parent_version.as_deref(),
            self.weight_artifact_hash,
            &dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let job_key = (
            self.service_name.as_ref().to_owned(),
            training_job_id.clone(),
        );
        let Some(job_record) = state_transaction
            .world
            .soracloud_training_jobs
            .get(&job_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{training_job_id}` not found for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if job_record.model_name != model_name {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    job_record.model_name
                )
                .into(),
            ));
        }
        if job_record.status != SoraTrainingJobStatusV1::Completed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("training job `{training_job_id}` is not completed").into(),
            ));
        }
        let artifact_key = (
            self.service_name.as_ref().to_owned(),
            training_job_id.clone(),
        );
        let Some(mut artifact_record) = state_transaction
            .world
            .soracloud_model_artifacts
            .get(&artifact_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact metadata for training job `{training_job_id}` not found for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if artifact_record.model_name != model_name {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact metadata for training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    artifact_record.model_name
                )
                .into(),
            ));
        }
        if artifact_record.consumed_by_version.is_some() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact metadata for training job `{training_job_id}` was already consumed by another model weight version"
                )
                .into(),
            ));
        }
        if artifact_record.weight_artifact_hash != self.weight_artifact_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("weight_artifact_hash mismatch for training job `{training_job_id}`")
                    .into(),
            ));
        }
        if artifact_record.dataset_ref != dataset_ref {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("dataset_ref mismatch for training job `{training_job_id}`").into(),
            ));
        }
        if artifact_record.training_config_hash != self.training_config_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("training_config_hash mismatch for training job `{training_job_id}`")
                    .into(),
            ));
        }
        if artifact_record.reproducibility_hash != self.reproducibility_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("reproducibility_hash mismatch for training job `{training_job_id}`")
                    .into(),
            ));
        }
        if artifact_record.provenance_attestation_hash != self.provenance_attestation_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "provenance_attestation_hash mismatch for training job `{training_job_id}`"
                )
                .into(),
            ));
        }
        let weight_key = (
            self.service_name.as_ref().to_owned(),
            model_name.clone(),
            weight_version.clone(),
        );
        if state_transaction
            .world
            .soracloud_model_weight_versions
            .get(&weight_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` weight version `{weight_version}` already exists for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        }
        let existing_version_count = state_transaction
            .world
            .soracloud_model_weight_versions
            .iter()
            .filter(|((service, model, _version), _record)| {
                service == self.service_name.as_ref() && model == model_name.as_str()
            })
            .count();
        let lineage_parent = match (existing_version_count == 0, parent_version.clone()) {
            (true, None) => None,
            (true, Some(_)) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    "parent_version must be omitted for the first model weight version".into(),
                ));
            }
            (false, None) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    "parent_version is required when registering subsequent weight versions".into(),
                ));
            }
            (false, Some(parent)) => {
                let parent_key = (
                    self.service_name.as_ref().to_owned(),
                    model_name.clone(),
                    parent.clone(),
                );
                if state_transaction
                    .world
                    .soracloud_model_weight_versions
                    .get(&parent_key)
                    .is_none()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!("parent_version `{parent}` not found for model `{model_name}`")
                            .into(),
                    ));
                }
                Some(parent)
            }
        };
        let registry_key = (self.service_name.as_ref().to_owned(), model_name.clone());
        let mut registry_record = state_transaction
            .world
            .soracloud_model_registries
            .get(&registry_key)
            .cloned()
            .unwrap_or(SoraModelRegistryV1 {
                schema_version: SORA_MODEL_REGISTRY_VERSION_V1,
                service_name: self.service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                current_version: None,
                updated_sequence: sequence,
            });
        registry_record.service_version = deployment.current_service_version.clone();
        registry_record.updated_sequence = sequence;
        artifact_record.service_version = deployment.current_service_version.clone();
        artifact_record.weight_version = Some(weight_version.clone());
        artifact_record.consumed_by_version = Some(weight_version.clone());
        record_model_weight_version(
            state_transaction,
            SoraModelWeightVersionRecordV1 {
                schema_version: SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                service_name: self.service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                weight_version: weight_version.clone(),
                parent_version: lineage_parent.clone(),
                training_job_id: training_job_id.clone(),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::TrainingJob,
                    id: training_job_id.clone(),
                }),
                weight_artifact_hash: self.weight_artifact_hash,
                dataset_ref,
                training_config_hash: self.training_config_hash,
                reproducibility_hash: self.reproducibility_hash,
                provenance_attestation_hash: self.provenance_attestation_hash,
                registered_sequence: sequence,
                promoted_sequence: None,
                gate_report_hash: None,
                promoted_by: None,
            },
        )?;
        record_model_registry(state_transaction, registry_record.clone())?;
        record_model_artifact(state_transaction, artifact_record)?;
        record_model_weight_audit_event(
            state_transaction,
            SoraModelWeightAuditEventV1 {
                schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelWeightActionV1::Register,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name,
                target_version: weight_version,
                current_version: registry_record.current_version,
                parent_version: lineage_parent,
                gate_approved: None,
                rollback_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::PromoteSoracloudModelWeight {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&self.model_name)?;
        let weight_version = parse_model_weight_version(&self.weight_version)?;
        verify_model_weight_promote_provenance(
            authority,
            &self.service_name,
            &model_name,
            &weight_version,
            self.gate_approved,
            self.gate_report_hash,
            &self.provenance,
        )?;
        if !self.gate_approved {
            return Err(InstructionExecutionError::InvariantViolation(
                "model promotion gate is not approved".into(),
            ));
        }
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let registry_key = (self.service_name.as_ref().to_owned(), model_name.clone());
        let Some(mut registry_record) = state_transaction
            .world
            .soracloud_model_registries
            .get(&registry_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` is not registered for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if registry_record.current_version.as_deref() == Some(weight_version.as_str()) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` weight version `{weight_version}` is already promoted"
                )
                .into(),
            ));
        }
        let weight_key = (
            self.service_name.as_ref().to_owned(),
            model_name.clone(),
            weight_version.clone(),
        );
        let Some(mut weight_record) = state_transaction
            .world
            .soracloud_model_weight_versions
            .get(&weight_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("weight version `{weight_version}` not found for model `{model_name}`")
                    .into(),
            ));
        };
        weight_record.service_version = deployment.current_service_version.clone();
        weight_record.promoted_sequence = Some(sequence);
        weight_record.gate_report_hash = Some(self.gate_report_hash);
        weight_record.promoted_by = Some(self.provenance.signer.clone());
        registry_record.service_version = deployment.current_service_version.clone();
        registry_record.current_version = Some(weight_version.clone());
        registry_record.updated_sequence = sequence;
        let parent_version = weight_record.parent_version.clone();
        record_model_weight_version(state_transaction, weight_record)?;
        record_model_registry(state_transaction, registry_record.clone())?;
        record_model_weight_audit_event(
            state_transaction,
            SoraModelWeightAuditEventV1 {
                schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelWeightActionV1::Promote,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name,
                target_version: weight_version,
                current_version: registry_record.current_version,
                parent_version,
                gate_approved: Some(true),
                rollback_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RollbackSoracloudModelWeight {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&self.model_name)?;
        let target_version = parse_model_weight_version(&self.target_version)?;
        let reason = normalize_model_weight_reason(&self.reason)?;
        verify_model_weight_rollback_provenance(
            authority,
            &self.service_name,
            &model_name,
            &target_version,
            &reason,
            &self.provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
        if !bundle.container.capabilities.allow_model_training {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow model training",
                    self.service_name
                )
                .into(),
            ));
        }
        let registry_key = (self.service_name.as_ref().to_owned(), model_name.clone());
        let Some(mut registry_record) = state_transaction
            .world
            .soracloud_model_registries
            .get(&registry_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` is not registered for service `{}`",
                    self.service_name
                )
                .into(),
            ));
        };
        if registry_record.current_version.as_deref() == Some(target_version.as_str()) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("model `{model_name}` is already at weight version `{target_version}`")
                    .into(),
            ));
        }
        let weight_key = (
            self.service_name.as_ref().to_owned(),
            model_name.clone(),
            target_version.clone(),
        );
        let Some(weight_record) = state_transaction
            .world
            .soracloud_model_weight_versions
            .get(&weight_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("weight version `{target_version}` not found for model `{model_name}`")
                    .into(),
            ));
        };
        registry_record.service_version = deployment.current_service_version.clone();
        registry_record.current_version = Some(target_version.clone());
        registry_record.updated_sequence = sequence;
        let parent_version = weight_record.parent_version.clone();
        record_model_registry(state_transaction, registry_record.clone())?;
        record_model_weight_audit_event(
            state_transaction,
            SoraModelWeightAuditEventV1 {
                schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelWeightActionV1::Rollback,
                service_name: self.service_name,
                service_version: deployment.current_service_version,
                model_name,
                target_version,
                current_version: registry_record.current_version,
                parent_version,
                gate_approved: None,
                rollback_reason: Some(reason),
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::RegisterSoracloudUploadedModelBundle {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::RegisterSoracloudUploadedModelBundle { bundle, provenance } = self;
        verify_uploaded_model_bundle_register_provenance(authority, &bundle, &provenance)?;
        bundle
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let model_id = parse_uploaded_model_id(&bundle.model_id)?;
        let weight_version = parse_model_weight_version(&bundle.weight_version)?;
        let (_deployment, service_bundle) =
            load_active_bundle(state_transaction, &bundle.service_name)?;
        if !service_allows_uploaded_model_plane(&service_bundle) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active revision does not allow uploaded model admission",
                    bundle.service_name
                )
                .into(),
            ));
        }
        if bundle.plaintext_bytes
            > state_transaction
                .nexus
                .uploaded_models
                .max_plaintext_bytes_per_model
        {
            return Err(invalid_parameter(format!(
                "plaintext_bytes exceeds nexus.uploaded_models.max_plaintext_bytes_per_model ({})",
                state_transaction
                    .nexus
                    .uploaded_models
                    .max_plaintext_bytes_per_model
            )));
        }
        if bundle.chunk_count
            > state_transaction
                .nexus
                .uploaded_models
                .max_chunk_count_per_model
        {
            return Err(invalid_parameter(format!(
                "chunk_count exceeds nexus.uploaded_models.max_chunk_count_per_model ({})",
                state_transaction
                    .nexus
                    .uploaded_models
                    .max_chunk_count_per_model
            )));
        }
        let bundle_key = (
            bundle.service_name.as_ref().to_owned(),
            model_id.clone(),
            weight_version.clone(),
        );
        if state_transaction
            .world
            .soracloud_uploaded_model_bundles
            .get(&bundle_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model bundle `{model_id}` version `{weight_version}` already registered for service `{}`",
                    bundle.service_name
                )
                .into(),
            ));
        }
        require_active_sorafs_uploaded_model_pin(state_transaction, &bundle)?;
        let record = SoraUploadedModelBundleV1 {
            schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
            model_id,
            weight_version,
            ..bundle
        };
        transfer_uploaded_model_amount(
            authority,
            &record.pricing_policy.storage_price,
            state_transaction,
        )?;
        record_uploaded_model_bundle(state_transaction, record)
    }
}
impl Execute for isi::FinalizeSoracloudUploadedModelBundle {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::FinalizeSoracloudUploadedModelBundle {
            service_name,
            model_name,
            model_id,
            artifact_id,
            weight_version,
            bundle_root,
            weight_artifact_hash,
            dataset_ref,
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance,
        } = self;
        let model_name = parse_training_model_name(&model_name)?;
        let model_id = parse_uploaded_model_id(&model_id)?;
        let artifact_id = parse_uploaded_artifact_id(&artifact_id)?;
        let weight_version = parse_model_weight_version(&weight_version)?;
        let dataset_ref = parse_model_weight_dataset_ref(&dataset_ref)?;
        let signer = provenance.signer.clone();
        verify_uploaded_model_finalize_provenance(
            authority,
            &service_name,
            &model_name,
            &model_id,
            &artifact_id,
            &weight_version,
            bundle_root,
            weight_artifact_hash,
            &dataset_ref,
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            &provenance,
        )?;
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, service_bundle) = load_active_bundle(state_transaction, &service_name)?;
        if !service_allows_uploaded_model_plane(&service_bundle) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{service_name}` active revision does not allow uploaded model admission"
                )
                .into(),
            ));
        }
        let bundle_key = (
            service_name.as_ref().to_owned(),
            model_id.clone(),
            weight_version.clone(),
        );
        let Some(bundle_record) = state_transaction
            .world
            .soracloud_uploaded_model_bundles
            .get(&bundle_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model bundle `{model_id}` version `{weight_version}` not found for service `{service_name}`"
                )
                .into(),
            ));
        };
        if bundle_record.bundle_root != bundle_root {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("bundle_root mismatch for uploaded model `{model_id}`").into(),
            ));
        }
        require_active_sorafs_uploaded_model_pin(state_transaction, &bundle_record)?;
        let artifact_key = (service_name.as_ref().to_owned(), artifact_id.clone());
        if state_transaction
            .world
            .soracloud_model_artifacts
            .get(&artifact_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("artifact `{artifact_id}` already registered for service `{service_name}`")
                    .into(),
            ));
        }
        let registry_key = (service_name.as_ref().to_owned(), model_name.clone());
        let mut registry_record = state_transaction
            .world
            .soracloud_model_registries
            .get(&registry_key)
            .cloned()
            .unwrap_or(SoraModelRegistryV1 {
                schema_version: SORA_MODEL_REGISTRY_VERSION_V1,
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                current_version: None,
                updated_sequence: sequence,
            });
        let existing_versions = state_transaction
            .world
            .soracloud_model_weight_versions
            .iter()
            .filter_map(|((stored_service, stored_model, _version), record)| {
                (stored_service == service_name.as_ref() && stored_model == model_name.as_str())
                    .then(|| record.clone())
            })
            .collect::<Vec<_>>();
        if existing_versions
            .iter()
            .any(|record| record.weight_version == weight_version)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` version `{weight_version}` already exists for service `{service_name}`"
                )
                .into(),
            ));
        }
        let lineage_parent = if existing_versions.is_empty() {
            None
        } else {
            registry_record.current_version.clone().or_else(|| {
                existing_versions
                    .iter()
                    .max_by(|left, right| {
                        left.registered_sequence
                            .cmp(&right.registered_sequence)
                            .then_with(|| left.weight_version.cmp(&right.weight_version))
                    })
                    .map(|record| record.weight_version.clone())
            })
        };
        registry_record.service_version = deployment.current_service_version.clone();
        registry_record.current_version = Some(weight_version.clone());
        registry_record.updated_sequence = sequence;
        record_model_weight_version(
            state_transaction,
            SoraModelWeightVersionRecordV1 {
                schema_version: SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                weight_version: weight_version.clone(),
                parent_version: lineage_parent.clone(),
                training_job_id: String::new(),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::UserUpload,
                    id: model_id.clone(),
                }),
                weight_artifact_hash,
                dataset_ref: dataset_ref.clone(),
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
                registered_sequence: sequence,
                promoted_sequence: None,
                gate_report_hash: None,
                promoted_by: None,
            },
        )?;
        record_model_registry(state_transaction, registry_record.clone())?;
        record_model_artifact(
            state_transaction,
            SoraModelArtifactRecordV1 {
                schema_version: SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                artifact_id: artifact_id.clone(),
                training_job_id: artifact_id.clone(),
                weight_version: Some(weight_version.clone()),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::UserUpload,
                    id: model_id.clone(),
                }),
                weight_artifact_hash,
                dataset_ref,
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
                registered_sequence: sequence,
                consumed_by_version: Some(weight_version.clone()),
                chunk_manifest_root: Some(bundle_record.chunk_manifest_root),
            },
        )?;
        record_model_weight_audit_event(
            state_transaction,
            SoraModelWeightAuditEventV1 {
                schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelWeightActionV1::Register,
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                model_name: model_name.clone(),
                target_version: weight_version.clone(),
                current_version: registry_record.current_version.clone(),
                parent_version: lineage_parent,
                gate_approved: None,
                rollback_reason: None,
                signer: signer.clone(),
            },
        )?;
        record_model_artifact_audit_event(
            state_transaction,
            SoraModelArtifactAuditEventV1 {
                schema_version: SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
                sequence,
                action: SoraModelArtifactActionV1::Register,
                service_name,
                service_version: deployment.current_service_version,
                model_name,
                training_job_id: artifact_id,
                consumed_by_version: Some(weight_version),
                signer,
            },
        )
    }
}
impl Execute for isi::AdvanceSoracloudRollout {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        verify_rollout_provenance(
            authority,
            &self.service_name,
            &self.rollout_handle,
            self.healthy,
            self.promote_to_percent,
            self.governance_tx_hash,
            &self.provenance,
        )?;
        if self.rollout_handle.trim().is_empty() {
            return Err(invalid_parameter("rollout_handle must not be empty"));
        }
        if self.promote_to_percent.is_some_and(|value| value > 100) {
            return Err(invalid_parameter(
                "promote_to_percent must be within 0..=100",
            ));
        }
        let Some(mut deployment) = state_transaction
            .world
            .soracloud_service_deployments
            .get(&self.service_name)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("service `{}` is not deployed", self.service_name).into(),
            ));
        };
        let Some(mut rollout) = deployment.active_rollout.clone() else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("service `{}` has no active rollout", self.service_name).into(),
            ));
        };
        if rollout.rollout_handle != self.rollout_handle {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{}` active rollout handle mismatch (expected `{}`)",
                    self.service_name, rollout.rollout_handle
                )
                .into(),
            ));
        }
        let sequence = next_soracloud_audit_sequence(state_transaction);
        let current_version = deployment.current_service_version.clone();
        let mut action = SoraServiceLifecycleActionV1::Rollout;
        let mut to_version = current_version.clone();
        let mut service_manifest_hash = deployment.current_service_manifest_hash;
        let mut container_manifest_hash = deployment.current_container_manifest_hash;
        if self.healthy {
            let promote_to = self.promote_to_percent.unwrap_or(100);
            if promote_to < rollout.traffic_percent {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "rollout traffic cannot decrease from {} to {promote_to}",
                        rollout.traffic_percent
                    )
                    .into(),
                ));
            }
            if promote_to < rollout.canary_percent {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "rollout traffic cannot be below canary_percent {}",
                        rollout.canary_percent
                    )
                    .into(),
                ));
            }
            rollout.traffic_percent = promote_to;
            rollout.stage = if promote_to == 100 {
                SoraRolloutStageV1::Promoted
            } else {
                SoraRolloutStageV1::Canary
            };
            rollout.health_failures = 0;
            rollout.updated_sequence = sequence;
            deployment.active_rollout = if rollout.stage == SoraRolloutStageV1::Canary {
                Some(rollout.clone())
            } else {
                None
            };
            deployment.last_rollout = Some(rollout.clone());
        } else {
            rollout.health_failures = rollout.health_failures.saturating_add(1);
            rollout.updated_sequence = sequence;
            if rollout.health_failures >= rollout.max_health_failures {
                let Some(baseline_version) = rollout.baseline_version.clone() else {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "service `{}` rollout `{}` has no baseline version for automatic rollback",
                            self.service_name, rollout.rollout_handle
                        )
                        .into(),
                    ));
                };
                let bundle =
                    load_admitted_bundle(state_transaction, &self.service_name, &baseline_version)?;
                rollout.stage = SoraRolloutStageV1::RolledBack;
                rollout.traffic_percent = 0;
                deployment.current_service_version = baseline_version.clone();
                deployment.current_service_manifest_hash = bundle.service_manifest_hash();
                deployment.current_container_manifest_hash = bundle.container_manifest_hash();
                deployment.process_generation = deployment.process_generation.saturating_add(1);
                deployment.process_started_sequence = sequence;
                deployment.active_rollout = None;
                deployment.last_rollout = Some(rollout.clone());
                action = SoraServiceLifecycleActionV1::Rollback;
                to_version = baseline_version;
                service_manifest_hash = bundle.service_manifest_hash();
                container_manifest_hash = bundle.container_manifest_hash();
            } else {
                deployment.active_rollout = Some(rollout.clone());
                deployment.last_rollout = Some(rollout.clone());
            }
        }
        record_deployment_state(state_transaction, deployment)?;
        record_audit_event(
            state_transaction,
            SoraServiceAuditEventV1 {
                schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence,
                action,
                service_name: self.service_name,
                from_version: Some(current_version),
                to_version,
                service_manifest_hash,
                container_manifest_hash,
                governance_tx_hash: Some(self.governance_tx_hash),
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: Some(self.rollout_handle),
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: self.provenance.signer,
            },
        )
    }
}
impl Execute for isi::SetSoracloudRuntimeState {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_service_runtime_authority(
            authority,
            &self.state.service_name,
            &self.state.active_service_version,
            state_transaction,
        )?;
        write_soracloud_runtime_state(state_transaction, self.state)
    }
}
impl Execute for isi::SetSoracloudInrouReplicaRuntimeState {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_runtime_authority(authority, state_transaction)?;
        let mut state = self.state;
        let now_ms = state_transaction.block_unix_timestamp_ms().max(1);
        if state.schema_version == 0 {
            state.schema_version = SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1;
        }
        if state.updated_at_ms == 0 {
            state.updated_at_ms = now_ms;
        }
        let Some(assignment) = find_inrou_replica_assignment(
            state_transaction,
            &state.service_name,
            &state.service_version,
            state.replica_slot,
        ) else {
            clear_soracloud_inrou_replica_runtime_state(
                state_transaction,
                &state.service_name,
                &state.service_version,
                state.replica_slot,
            );
            return Ok(());
        };
        if assignment.validator_account_id != *authority {
            return Ok(());
        }
        if state.validator_account_id != assignment.validator_account_id
            || state.peer_id != assignment.peer_id
            || state.selected_backend != assignment.selected_backend
            || state.selected_guest_isa != assignment.selected_guest_isa
        {
            return Ok(());
        }
        write_soracloud_inrou_replica_runtime_state(state_transaction, state)
    }
}
impl Execute for isi::ClearSoracloudInrouReplicaRuntimeState {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_runtime_authority(authority, state_transaction)?;
        if self.service_version.trim().is_empty() {
            return Err(invalid_parameter("service_version must not be empty"));
        }
        if self.replica_slot == 0 {
            return Err(invalid_parameter("replica_slot must be greater than zero"));
        }
        let Some(assignment) = find_inrou_replica_assignment(
            state_transaction,
            &self.service_name,
            &self.service_version,
            self.replica_slot,
        ) else {
            clear_soracloud_inrou_replica_runtime_state(
                state_transaction,
                &self.service_name,
                &self.service_version,
                self.replica_slot,
            );
            return Ok(());
        };
        if assignment.validator_account_id != *authority {
            return Ok(());
        }
        clear_soracloud_inrou_replica_runtime_state(
            state_transaction,
            &self.service_name,
            &self.service_version,
            self.replica_slot,
        );
        Ok(())
    }
}
impl Execute for isi::ReportSoracloudServiceLeaseUsage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.active_service_version.trim().is_empty() {
            return Err(invalid_parameter(
                "active_service_version must not be empty".to_string(),
            ));
        }
        require_soracloud_service_runtime_authority(
            authority,
            &self.service_name,
            &self.active_service_version,
            state_transaction,
        )?;
        write_soracloud_service_lease_usage(
            state_transaction,
            self.service_name,
            self.active_service_version,
            self.accounted_egress_bytes,
        )
    }
}
impl Execute for isi::RecordSoracloudMailboxMessage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let source_service_version = state_transaction
            .world
            .soracloud_service_deployments
            .get(&self.message.from_service)
            .map(|deployment| deployment.current_service_version.clone())
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "source service `{}` is not deployed",
                        self.message.from_service
                    )
                    .into(),
                )
            })?;
        require_soracloud_service_runtime_authority(
            authority,
            &self.message.from_service,
            &source_service_version,
            state_transaction,
        )?;
        write_soracloud_mailbox_message(state_transaction, self.message)
    }
}
impl Execute for isi::RecordSoracloudRuntimeReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let runtime_authority = require_soracloud_service_runtime_authority(
            authority,
            &self.receipt.service_name,
            &self.receipt.service_version,
            state_transaction,
        )?;
        if runtime_authority == SoracloudServiceRuntimeAuthority::AssignedValidator
            && self.receipt.selected_validator_account_id.as_ref() != Some(authority)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "runtime receipt `{}` must identify submitting validator `{authority}` as selected_validator_account_id",
                    self.receipt.receipt_id
                )
                .into(),
            ));
        }
        write_soracloud_runtime_receipt(state_transaction, self.receipt)
    }
}
impl Execute for isi::RecordSoracloudPrivateUploadedModelExecutionReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        write_soracloud_private_uploaded_model_execution_receipt(state_transaction, self.receipt)
    }
}
#[cfg(all(test, feature = "zk-stark"))]
fn finalize_soracloud_fhe_full_bootstrap_stark_proof_attachment_v1(
    proof_box: iroha_data_model::proof::ProofBox,
    circuit_id: &str,
    context: &SoracloudFheProofAttachmentDecodeContext,
) -> Result<ProofAttachment, InstructionExecutionError> {
    let backend = proof_box.backend.clone();
    let mut attachment = ProofAttachment::new_ref(
        backend.clone(),
        proof_box,
        iroha_data_model::proof::VerifyingKeyId::new(backend, circuit_id),
    );
    attachment.envelope_hash = Some(<[u8; Hash::LENGTH]>::from(Hash::new(
        &attachment.proof.bytes,
    )));
    let envelope = proof_attachment_envelope_with_context(&attachment, context)?;
    if envelope.circuit_id != circuit_id {
        return Err(invalid_parameter(format!(
            "generated Soracloud FHE full-bootstrap proof circuit mismatch: expected `{circuit_id}`, found `{}`",
            envelope.circuit_id
        )));
    }
    attachment.vk_commitment = Some(envelope.vk_hash);
    Ok(attachment)
}
#[cfg(feature = "zk-stark")]
fn soracloud_fhe_full_bootstrap_execution_proof_from_native_air_envelope_v1(
    statement_hash: Hash,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    envelope_bytes: Vec<u8>,
) -> Result<SoracloudFheFullBootstrapExecutionProofV1, InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_execution_proof_native_envelope_bytes(&envelope_bytes)?;
    validate_soracloud_fhe_full_bootstrap_native_air_statement_binding_v1(
        "FHE full-bootstrap execution proof",
        &envelope_bytes,
        statement_hash,
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1,
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
    )?;
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        envelope_bytes,
    };
    let proof_envelope = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        crate::zk::hash_vk(verifier_key),
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        norito::encode_canonical(&open).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution STARK public-input wrapper encoding failed: {err}"
            ))
        })?,
    );
    let proof_box = iroha_data_model::proof::ProofBox::new(
        verifier_key.backend.clone(),
        norito::encode_canonical(&proof_envelope).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution OpenVerifyEnvelope encoding failed: {err}"
            ))
        })?,
    );
    let mut attachment = ProofAttachment::new_ref(
        verifier_key.backend.clone(),
        proof_box,
        iroha_data_model::proof::VerifyingKeyId::new(
            verifier_key.backend.clone(),
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        ),
    );
    attachment.envelope_hash = Some(<[u8; Hash::LENGTH]>::from(Hash::new(
        &attachment.proof.bytes,
    )));
    attachment.vk_commitment = Some(proof_envelope.vk_hash);
    let proof = SoracloudFheFullBootstrapExecutionProofV1 {
        schema_version: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1,
        statement_hash,
        proof: attachment,
    };
    proof.validate().map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution native STARK proof failed validation: {err}"
        ))
    })?;
    Ok(proof)
}
#[cfg(all(test, feature = "zk-stark"))]
pub(crate) fn prove_soracloud_fhe_full_bootstrap_execution_proof_v1(
    statement_hash: Hash,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
) -> Result<SoracloudFheFullBootstrapExecutionProofV1, InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_prover_verifier_key(
        "FHE full-bootstrap execution proof",
        verifier_key,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    )?;
    validate_soracloud_fhe_full_bootstrap_prover_statement_hash(
        "FHE full-bootstrap execution proof",
        statement_hash,
    )?;
    Err(invalid_parameter(format!(
        "FHE full-bootstrap execution proof generation requires {FHE_FULL_BOOTSTRAP_DEDICATED_PROVER_UNAVAILABLE}"
    )))
}
/// Build native BFV AIR STARK/FRI envelope bytes from release-prover material.
///
/// The returned envelope commits the governed row-major arithmetic trace and
/// the typed AIR evaluation composition vector bound into
/// [`BfvFullBootstrapExecutionProverInputMaterialV1`]. This is the
/// release-prover handoff immediately before the proof wrapper is emitted.
///
/// # Errors
/// Returns an execution error when prover input material validation fails, the
/// AIR evaluation material does not bind the trace, the shared BFV STARK
/// wrapper rejects the generated envelope, or the envelope cannot be built
/// within production byte limits.
#[cfg(feature = "zk-stark")]
fn build_soracloud_fhe_full_bootstrap_execution_native_air_envelope_bytes_from_prover_input_material_v1(
    prover_input_material: &BfvFullBootstrapExecutionProverInputMaterialV1,
) -> Result<Vec<u8>, InstructionExecutionError> {
    validate_bfv_full_bootstrap_execution_prover_input_material_v1(prover_input_material).map_err(
        |err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution prover input material failed validation: {err}"
            ))
        },
    )?;
    let envelope_bytes = crate::zk_stark::prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(
        prover_input_material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution native AIR envelope generation failed: {err}"
        ))
    })?;
    let mut limits = crate::zk_stark::StarkVerifierLimits::default();
    limits.max_envelope_bytes =
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES;
    if !crate::zk_stark::verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
        &envelope_bytes,
        &limits,
        prover_input_material,
    ) {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution native AIR envelope shared BFV verifier rejected release-prover material",
        ));
    }
    Ok(envelope_bytes)
}
#[cfg(all(test, feature = "zk-stark"))]
fn validate_soracloud_fhe_full_bootstrap_execution_native_air_envelope_bytes_for_trace_material_v1(
    envelope_bytes: &[u8],
    arithmetic_trace_material: &iroha_crypto::fhe_bfv::BfvFullBootstrapArithmeticTraceMaterialV1,
    arithmetic_air_evaluation_material: &iroha_crypto::fhe_bfv::BfvFullBootstrapArithmeticAirEvaluationMaterialV1,
) -> Result<crate::zk_stark::StarkVerifyEnvelopeV1, InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_execution_proof_native_envelope_bytes(envelope_bytes)?;
    validate_bfv_full_bootstrap_arithmetic_trace_material_v1(arithmetic_trace_material).map_err(
        |err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution arithmetic trace material failed validation: {err}"
            ))
        },
    )?;
    validate_bfv_full_bootstrap_arithmetic_air_evaluation_material_for_trace_v1(
        arithmetic_trace_material,
        arithmetic_air_evaluation_material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution arithmetic AIR evaluation material failed validation: {err}"
        ))
    })?;
    let statement_hash = arithmetic_trace_material
        .proof_input_material
        .statement_hash;
    let native_envelope: crate::zk_stark::StarkVerifyEnvelopeV1 =
        norito::decode_canonical(envelope_bytes).map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution native AIR envelope decode failed: {err}"
            ))
        })?;
    let slot_index = arithmetic_trace_material
        .proof_input_material
        .witness_material
        .slot_index;
    let bound_mode = arithmetic_trace_material
        .proof_input_material
        .witness_material
        .bound_mode;
    let trace_material_digest =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_arithmetic_trace_material_digest_v1(
            arithmetic_trace_material,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution arithmetic trace material digest failed validation: {err}"
            ))
        })?;
    let public_padding_context = BfvFullBootstrapNativeAirPublicPaddingContext {
        slot_index,
        bound_mode,
        trace_material_digest,
        expected_trace_material_digest: Some(trace_material_digest),
        expected_trace_rows: Some(arithmetic_trace_material.rows.clone()),
        expected_composition_values: Some(
            arithmetic_air_evaluation_material
                .composition_values
                .clone(),
        ),
    };
    let mut limits = crate::zk_stark::StarkVerifierLimits::default();
    limits.max_envelope_bytes =
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES;
    validate_soracloud_fhe_full_bootstrap_bfv_native_air_boundary_with_limits(
        "FHE full-bootstrap execution proof",
        statement_hash,
        &native_envelope,
        Some(public_padding_context.clone()),
        &limits,
    )?;
    if !crate::zk_stark::verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
        envelope_bytes,
        &limits,
        statement_hash,
        public_padding_context.trace_material_digest,
        slot_index,
        bound_mode,
    ) {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution native AIR envelope shared public-padding verifier rejected release-prover public openings",
        ));
    }
    let expected_base_indices =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_arithmetic_trace_canonical_opening_indices_from_transcript_v1(
            statement_hash,
            trace_material_digest,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution native AIR opening schedule derivation failed: {err}"
            ))
        })?
        .into_iter()
        .map(|index| {
            usize::try_from(index).map_err(|_| {
                invalid_parameter(
                    "FHE full-bootstrap execution native AIR opening index exceeds platform usize",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !crate::zk_stark::verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
        envelope_bytes,
        &limits,
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
        &<[u8; Hash::LENGTH]>::from(statement_hash),
        &arithmetic_trace_material.rows,
        &arithmetic_air_evaluation_material.composition_values,
        &expected_base_indices,
    ) {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution native AIR envelope replay rejected release-prover trace material",
        ));
    }
    Ok(native_envelope)
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_execution_prover_input_material_for_artifacts_v1(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    prover_input_material: &BfvFullBootstrapExecutionProverInputMaterialV1,
) -> Result<(), InstructionExecutionError> {
    let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires bootstrap key material")
    })?;
    if bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof requires FullBootstrapV1 bootstrap key material",
        ));
    }
    bfv_full_bootstrap_execution_prover_input_material_digest_for_artifacts_v1(
        params,
        bootstrap_key,
        artifacts,
        &evaluation_keys.galois_keys,
        prover_input_material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution artifact-bound prover input material digest failed validation: {err}"
        ))
    })?;
    Ok(())
}
/// Prove a governed Soracloud FHE full-bootstrap execution statement from release-prover material.
///
/// The release-prover material is validated before native STARK/FRI envelope
/// construction against the concrete governed artifacts, so stale prefix
/// traces, stale arithmetic/AIR material, or unrelated generated
/// prover/verifier proof-key pairs are rejected before a proof attachment is
/// emitted.
///
/// # Errors
/// Returns an execution error when the verifier key is not the governed STARK/FRI key for the
/// execution circuit, when the prover input material is stale or malformed, or
/// when the native BFV STARK/FRI envelope cannot be built within production
/// limits.
#[cfg(feature = "zk-stark")]
fn prove_soracloud_fhe_full_bootstrap_execution_proof_from_prover_input_material_v1(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    prover_input_material: &BfvFullBootstrapExecutionProverInputMaterialV1,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
) -> Result<SoracloudFheFullBootstrapExecutionProofV1, InstructionExecutionError> {
    let canonical_verifier_key = canonical_soracloud_fhe_full_bootstrap_prover_verifier_key(
        "FHE full-bootstrap execution proof",
        verifier_key,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    )?;
    validate_soracloud_fhe_full_bootstrap_execution_prover_input_material_for_artifacts_v1(
        params,
        evaluation_keys,
        artifacts,
        prover_input_material,
    )?;
    let prover_input_verifier_key =
        full_bootstrap_execution_prover_input_material_verifier_key(prover_input_material)?;
    if prover_input_verifier_key != canonical_verifier_key {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof verifier key must match prover input material",
        ));
    }
    validate_soracloud_fhe_full_bootstrap_prover_statement_hash(
        "FHE full-bootstrap execution proof",
        prover_input_material.proof_input_material.statement_hash,
    )?;
    let native_air_envelope_bytes =
        build_soracloud_fhe_full_bootstrap_execution_native_air_envelope_bytes_from_prover_input_material_v1(
            prover_input_material,
        )?;
    soracloud_fhe_full_bootstrap_execution_proof_from_native_air_envelope_v1(
        prover_input_material.proof_input_material.statement_hash,
        &canonical_verifier_key,
        native_air_envelope_bytes,
    )
}
#[cfg(feature = "zk-stark")]
fn full_bootstrap_execution_prover_input_material_verifier_key(
    prover_input_material: &BfvFullBootstrapExecutionProverInputMaterialV1,
) -> Result<iroha_data_model::proof::VerifyingKeyBox, InstructionExecutionError> {
    let material_envelope =
        validate_bfv_full_bootstrap_proof_key_material_envelope_bytes_for_key_v1(
            &prover_input_material.verifier_key,
            &prover_input_material.verifier_key.key_material,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE full-bootstrap execution prover input verifier key material envelope failed validation: {err}"
            ))
        })?;
    let native_verifier_key_material = decode_bfv_full_bootstrap_native_proof_key_material_v1(
        &material_envelope.native_key_material,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution prover input native verifier key material failed validation: {err}"
        ))
    })?;
    let mut verifier_key_box = iroha_data_model::proof::VerifyingKeyBox::new(
        prover_input_material.verifier_key.backend.clone(),
        native_verifier_key_material.native_payload,
    );
    validate_governed_full_bootstrap_execution_stark_verifier_key_payload(&mut verifier_key_box)?;
    Ok(verifier_key_box)
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_release_audit_package_for_evaluation_keys_v1(
    context: &str,
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    transcript: &BfvEvaluationKeyRefreshTranscriptV1,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    release_audit_package: &BfvFullBootstrapReleaseAuditPackageV1,
    expected_release_audit_package_digest: Hash,
    trusted_reviewer_id: &str,
    trusted_reviewer_public_key: &PublicKey,
    required_refresh_mode: Option<BfvRefreshTranscriptModeV1>,
) -> Result<(), InstructionExecutionError> {
    let bootstrap_key = evaluation_keys
        .bootstrap_key
        .as_ref()
        .ok_or_else(|| invalid_parameter(format!("{context} requires bootstrap key material")))?;
    if bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        return Err(invalid_parameter(format!(
            "{context} requires FullBootstrapV1 bootstrap key material"
        )));
    }
    let full_bootstrap_material =
        bootstrap_key
            .full_bootstrap_material
            .as_ref()
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "{context} requires governed full-bootstrap material"
                ))
            })?;
    let expected_public_key_digest = bootstrap_key.public_key_digest.as_ref().ok_or_else(|| {
        invalid_parameter(format!(
            "{context} requires governed bootstrap public-key digest"
        ))
    })?;
    let transcript_public_key_digest = bfv_public_key_digest(params, &transcript.public_key)
        .map_err(|err| {
            invalid_parameter(format!(
                "{context} refresh transcript public key failed validation: {err}"
            ))
        })?;
    if &transcript_public_key_digest != expected_public_key_digest {
        return Err(invalid_parameter(format!(
            "{context} refresh transcript public-key digest does not match governed bootstrap key"
        )));
    }
    validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
        context,
        params,
        evaluation_keys,
        transcript,
        required_refresh_mode,
    )?;
    validate_bfv_full_bootstrap_release_audit_package_for_artifacts_trusted_reviewer_and_digest_v1(
        params,
        full_bootstrap_material,
        artifacts,
        release_audit_package,
        expected_release_audit_package_digest,
        trusted_reviewer_id,
        trusted_reviewer_public_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "{context} release audit package failed validation: {err}"
        ))
    })?;
    Ok(())
}
#[cfg(feature = "zk-stark")]
fn validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
    context: &str,
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    transcript: &BfvEvaluationKeyRefreshTranscriptV1,
    required_refresh_mode: Option<BfvRefreshTranscriptModeV1>,
) -> Result<(), InstructionExecutionError> {
    let validate_mode = |mode| {
        let mode_label = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => "exact-lift",
            BfvRefreshTranscriptModeV1::BoundedNoise => "bounded-noise",
        };
        transcript
            .digest_for_evaluation_keys_with_mode(params, evaluation_keys, mode)
            .map(|_| ())
            .map_err(|err| {
                format!("{context} refresh transcript failed {mode_label} validation: {err}")
            })
    };
    if let Some(mode) = required_refresh_mode {
        return validate_mode(mode).map_err(invalid_parameter);
    }
    match validate_mode(BfvRefreshTranscriptModeV1::ExactLift) {
        Ok(()) => Ok(()),
        Err(exact_err) => match validate_mode(BfvRefreshTranscriptModeV1::BoundedNoise) {
            Ok(()) => Ok(()),
            Err(bounded_err) => Err(invalid_parameter(format!(
                "{context} refresh transcript must validate in exact-lift or bounded-noise mode: exact-lift: {exact_err}; bounded-noise: {bounded_err}"
            ))),
        },
    }
}
/// Derive and prove the Soracloud FHE full-bootstrap material statement for evaluation keys.
///
/// This helper mirrors runtime policy admission: the statement hash is derived from the BFV
/// parameters, refresh transcript public key, governed evaluation-key bundle, and concrete
/// full-bootstrap artifact bundle before the STARK/FRI material proof would be constructed.
/// It stays internal so production callers use the release-audit-gated wrapper.
///
/// # Errors
/// Returns an execution error when the evaluation keys do not carry governed `FullBootstrapV1`
/// material, the artifact bundle does not match that material, statement derivation fails, or the
/// dedicated full-bootstrap prover is unavailable.
#[cfg(feature = "zk-stark")]
fn prove_soracloud_fhe_full_bootstrap_execution_proofs_for_claims_v1(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    transcript: &BfvEvaluationKeyRefreshTranscriptV1,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    input: &BfvIdentifierCiphertext,
    output: &BfvIdentifierCiphertext,
    bound_mode: BfvCiphertextBoundModeV1,
    input_bound: u128,
    output_bound: u128,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
) -> Result<Vec<SoracloudFheFullBootstrapExecutionProofV1>, InstructionExecutionError> {
    let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
        invalid_parameter("FHE full-bootstrap execution proof requires bootstrap key material")
    })?;
    if bootstrap_key.mode != BfvBootstrapKeyMode::FullBootstrapV1 {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof requires FullBootstrapV1 bootstrap key material",
        ));
    }
    let full_bootstrap_material =
        bootstrap_key
            .full_bootstrap_material
            .as_ref()
            .ok_or_else(|| {
                invalid_parameter(
                    "FHE full-bootstrap execution proof requires governed full-bootstrap material",
                )
            })?;
    let governed_verifier_key =
        governed_full_bootstrap_execution_verifier_key(params, evaluation_keys, artifacts)?;
    let verifier_key_matches_governed_artifact = if &governed_verifier_key == verifier_key {
        true
    } else {
        match canonical_soracloud_fhe_full_bootstrap_prover_verifier_key(
            "FHE full-bootstrap execution proof",
            verifier_key,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        ) {
            Ok(canonical_verifier_key) => canonical_verifier_key == governed_verifier_key,
            Err(_) => false,
        }
    };
    if !verifier_key_matches_governed_artifact {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof verifier key must match governed artifact",
        ));
    }
    let prover_proof_key = decode_bfv_full_bootstrap_proof_key_artifact_v1(
        params,
        full_bootstrap_material,
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        &artifacts.prover_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution proof prover key failed validation: {err}"
        ))
    })?;
    let verifier_proof_key = decode_bfv_full_bootstrap_proof_key_artifact_v1(
        params,
        full_bootstrap_material,
        BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
        &artifacts.verifier_key,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "FHE full-bootstrap execution proof verifier key failed validation: {err}"
        ))
    })?;
    let input_slots = first_matching_fhe_slots(std::slice::from_ref(input))?;
    if input_slots.len() != output.slots.len() {
        return Err(invalid_parameter(
            "FHE full-bootstrap execution proof output slot count does not match input slot count",
        ));
    }
    let proof_bound_mode = full_bootstrap_execution_proof_bound_mode(bound_mode);
    input_slots
        .iter()
        .zip(output.slots.iter())
        .enumerate()
        .map(|(slot_index, (input_ciphertext, output_ciphertext))| {
            let claim = bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1(
                params,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                u32::try_from(slot_index).map_err(|_| {
                    invalid_parameter("FHE full-bootstrap execution proof slot index overflow")
                })?,
                input_ciphertext.clone(),
                output_ciphertext.clone(),
                proof_bound_mode,
                input_bound,
                output_bound,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution witness for slot {slot_index}: {err}"
                ))
            })?;
            let witness_material = bfv_full_bootstrap_execution_witness_digest_material_v1(
                params,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                &claim,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution proof input material for slot {slot_index}: {err}"
                ))
            })?;
            let input_material = bfv_full_bootstrap_execution_proof_input_material_v1(
                &transcript.public_key,
                &witness_material,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution proof input material for slot {slot_index}: {err}"
                ))
            })?;
            let prover_input_material = bfv_full_bootstrap_execution_prover_input_material_v1(
                &input_material,
                &prover_proof_key,
                &verifier_proof_key,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "failed to derive FHE full-bootstrap execution prover input material for slot {slot_index}: {err}"
                ))
            })?;
            prove_soracloud_fhe_full_bootstrap_execution_proof_from_prover_input_material_v1(
                params,
                evaluation_keys,
                artifacts,
                &prover_input_material,
                verifier_key,
            )
        })
        .collect()
}
#[cfg(feature = "zk-stark")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn validate_soracloud_fhe_full_bootstrap_release_audited_execution_output_v1(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    input: &BfvIdentifierCiphertext,
    output: &BfvIdentifierCiphertext,
    bound_mode: BfvCiphertextBoundModeV1,
    input_bound: u128,
    output_bound: u128,
    release_audit_package: &BfvFullBootstrapReleaseAuditPackageV1,
    expected_release_audit_package_digest: Hash,
    trusted_reviewer_id: &str,
    trusted_reviewer_public_key: &PublicKey,
) -> Result<(), InstructionExecutionError> {
    let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
        invalid_parameter(
            "FHE full-bootstrap release-audited execution requires bootstrap key material",
        )
    })?;
    if input.slots.len() != output.slots.len() {
        return Err(invalid_parameter(
            "FHE full-bootstrap release-audited execution output slot count does not match input slot count",
        ));
    }
    let expected_output_bound = match bound_mode {
        BfvCiphertextBoundModeV1::ExactResidualMultiple => {
            for (slot_index, (input_ciphertext, output_ciphertext)) in
                input.slots.iter().zip(output.slots.iter()).enumerate()
            {
                let expected_output =
                    full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1(
                        params,
                        bootstrap_key,
                        artifacts,
                        &evaluation_keys.galois_keys,
                        input_ciphertext,
                        release_audit_package,
                        expected_release_audit_package_digest,
                        trusted_reviewer_id,
                        trusted_reviewer_public_key,
                    )
                    .map_err(|err| {
                        invalid_parameter(format!(
                            "FHE full-bootstrap release-audited exact execution failed for slot {slot_index}: {err}"
                        ))
                    })?;
                if &expected_output != output_ciphertext {
                    return Err(invalid_parameter(format!(
                        "FHE full-bootstrap release-audited exact output mismatch for slot {slot_index}"
                    )));
                }
            }
            bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1(
                params,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                input_bound,
                release_audit_package,
                expected_release_audit_package_digest,
                trusted_reviewer_id,
                trusted_reviewer_public_key,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE full-bootstrap release-audited exact output bound failed: {err}"
                ))
            })?
        }
        BfvCiphertextBoundModeV1::BoundedNoise => {
            for (slot_index, (input_ciphertext, output_ciphertext)) in
                input.slots.iter().zip(output.slots.iter()).enumerate()
            {
                let expected_output =
                    full_bootstrap_ciphertext_with_release_audited_artifacts_bounded_noise_registered_rns_basis_extension_exact_v1(
                        params,
                        bootstrap_key,
                        artifacts,
                        &evaluation_keys.galois_keys,
                        input_ciphertext,
                        release_audit_package,
                        expected_release_audit_package_digest,
                        trusted_reviewer_id,
                        trusted_reviewer_public_key,
                    )
                    .map_err(|err| {
                        invalid_parameter(format!(
                            "FHE full-bootstrap release-audited bounded-noise execution failed for slot {slot_index}: {err}"
                        ))
                    })?;
                if &expected_output != output_ciphertext {
                    return Err(invalid_parameter(format!(
                        "FHE full-bootstrap release-audited bounded-noise output mismatch for slot {slot_index}"
                    )));
                }
            }
            bfv_full_bootstrap_with_release_audited_artifacts_bounded_noise_output_bound_v1(
                params,
                bootstrap_key,
                artifacts,
                &evaluation_keys.galois_keys,
                input_bound,
                release_audit_package,
                expected_release_audit_package_digest,
                trusted_reviewer_id,
                trusted_reviewer_public_key,
            )
            .map_err(|err| {
                invalid_parameter(format!(
                    "FHE full-bootstrap release-audited bounded-noise output bound failed: {err}"
                ))
            })?
        }
    };
    if expected_output_bound != output_bound {
        return Err(invalid_parameter(format!(
            "FHE full-bootstrap release-audited output bound {output_bound} does not match deterministic governed bound {expected_output_bound}"
        )));
    }
    Ok(())
}
/// Derive and prove full-bootstrap execution statements after release-audit validation.
///
/// This is the production release-prover entry point: the signed release audit
/// package must validate against the governed full-bootstrap material,
/// concrete artifacts, caller-trusted reviewer id/key, and caller-pinned package
/// digest before any native proof attachment is emitted. The lower-level claim
/// helper stays internal for fixture construction and unit-level prover-boundary
/// tests.
///
/// # Errors
/// Returns an execution error when the evaluation keys are not governed
/// `FullBootstrapV1` keys, the release audit package does not match the
/// governed artifacts, trusted reviewer, or caller-pinned package digest, or
/// the underlying native proof generation fails.
#[cfg(feature = "zk-stark")]
#[allow(clippy::too_many_arguments)]
pub fn prove_soracloud_fhe_full_bootstrap_execution_proofs_for_claims_with_release_audit_v1(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    transcript: &BfvEvaluationKeyRefreshTranscriptV1,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    input: &BfvIdentifierCiphertext,
    output: &BfvIdentifierCiphertext,
    bound_mode: BfvCiphertextBoundModeV1,
    input_bound: u128,
    output_bound: u128,
    verifier_key: &iroha_data_model::proof::VerifyingKeyBox,
    release_audit_package: &BfvFullBootstrapReleaseAuditPackageV1,
    expected_release_audit_package_digest: Hash,
    trusted_reviewer_id: &str,
    trusted_reviewer_public_key: &PublicKey,
) -> Result<Vec<SoracloudFheFullBootstrapExecutionProofV1>, InstructionExecutionError> {
    validate_soracloud_fhe_full_bootstrap_release_audit_package_for_evaluation_keys_v1(
        "FHE full-bootstrap execution proof",
        params,
        evaluation_keys,
        transcript,
        artifacts,
        release_audit_package,
        expected_release_audit_package_digest,
        trusted_reviewer_id,
        trusted_reviewer_public_key,
        Some(refresh_transcript_mode_for_ciphertext_bound_mode(
            bound_mode,
        )),
    )?;
    validate_soracloud_fhe_full_bootstrap_release_audited_execution_output_v1(
        params,
        evaluation_keys,
        artifacts,
        input,
        output,
        bound_mode,
        input_bound,
        output_bound,
        release_audit_package,
        expected_release_audit_package_digest,
        trusted_reviewer_id,
        trusted_reviewer_public_key,
    )?;
    prove_soracloud_fhe_full_bootstrap_execution_proofs_for_claims_v1(
        params,
        evaluation_keys,
        transcript,
        artifacts,
        input,
        output,
        bound_mode,
        input_bound,
        output_bound,
        verifier_key,
    )
}
#[cfg(test)]
mod tests {
    include!("soracloud_tests.rs");
    mod agent_apartment;
}
