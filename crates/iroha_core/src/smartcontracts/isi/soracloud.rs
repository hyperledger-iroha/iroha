//! Soracloud lifecycle instruction handlers.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use iroha_crypto::{
    Hash,
    fhe_bfv::{
        BfvCiphertext, BfvEvaluationBudget, BfvEvaluationKeyBundle, BfvEvaluationPlan,
        BfvIdentifierCiphertext, BfvParameters, RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT,
        add_ciphertexts_rns_exact, bfv_add_bounded_noise_output_bound,
        bfv_add_output_residual_multiple_bound,
        bfv_bootstrap_key_refresh_bounded_noise_output_bound,
        bfv_bootstrap_key_refresh_output_residual_multiple_bound,
        bfv_multiply_bounded_noise_output_bound, bfv_multiply_output_residual_multiple_bound,
        bfv_packed_rotate_left_bounded_noise_output_bound,
        bfv_packed_rotate_left_output_residual_multiple_bound,
        bfv_rotate_slots_left_bounded_noise_output_bounds,
        bfv_rotate_slots_left_output_residual_multiple_bounds,
        bootstrap_ciphertext_bounded_noise_rns_exact_round, bootstrap_ciphertext_rns_exact_round,
        multiply_ciphertexts_bounded_noise_rns_exact, multiply_ciphertexts_rns_exact,
        multiply_plain_scalar, ram_lfe_bfv_parameters_v1, registered_bfv_parameter_digest,
        registered_bfv_rns_modulus_chain, registered_bfv_rns_modulus_chain_digest,
        rotate_ciphertext_slots_left_bounded_noise_rns_exact,
        rotate_ciphertext_slots_left_rns_exact,
        rotate_packed_ciphertext_slots_left_with_galois_keys_bounded_noise_rns_exact,
        rotate_packed_ciphertext_slots_left_with_galois_keys_rns_exact,
        validate_bfv_bounded_noise_bound, validate_bfv_exact_residual_multiple_capacity,
        validate_registered_bfv_parameters,
    },
};
use iroha_data_model::{
    account::AccountId,
    confidential::ConfidentialStatus,
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
        SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1, SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1, SecretEnvelopeV1,
        SoraAgentApartmentActionV1, SoraAgentApartmentAuditEventV1, SoraAgentApartmentRecordV1,
        SoraAgentArtifactAllowRuleV1, SoraAgentAutonomyRunRecordV1, SoraAgentMailboxMessageV1,
        SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1, SoraAgentWalletDailySpendEntryV1,
        SoraAgentWalletSpendRequestV1, SoraAppInfraActionV1, SoraAppInfraAuditEventV1,
        SoraAppInfraManifestV1, SoraAppInfraStateV1, SoraDecryptionRequestRecordV1,
        SoraDeploymentBundleV1, SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1,
        SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1, SoraHfPlacementStatusV1,
        SoraHfResourceProfileV1, SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1,
        SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1,
        SoraHfSharedLeaseQueuedWindowV1, SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1,
        SoraHfSourceStatusV1, SoraInrouGuestIsaV1, SoraInrouHostCapabilityRecordV1,
        SoraInrouReplicaPlacementV1, SoraInrouReplicaRuntimeStateV1, SoraInrouRuntimeBackendV1,
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
        SoraTrainingJobStatusV1, SoraUploadedModelBundleV1, SoracloudFheInputAdmissionProofV1,
        derive_agent_autonomy_request_commitment,
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
        soracloud_fhe_input_admission_public_inputs_schema_hash_v1,
    },
    sorafs::pin_registry::{PinStatus, StorageClass},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
use iroha_primitives::{json::Json, numeric::Numeric};
use mv::storage::StorageReadOnly;

use super::{
    staking::{apply_slash_to_validator, max_slash_amount},
    *,
};
use crate::{
    smartcontracts::Execute, soracloud_runtime::soracloud_hf_generated_source_binding,
    state::StateTransaction,
};

const CAN_MANAGE_SORACLOUD_PERMISSION: &str = "CanManageSoracloud";
const TAIRA_TESTNET_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
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

#[derive(Clone, Copy, Debug)]
struct HfHostClassPolicy {
    host_class: &'static str,
    min_model_bytes: u64,
    min_disk_cache_bytes: u64,
    min_ram_bytes: u64,
    min_vram_bytes: u64,
    reservation_fee_small_nanos: u128,
    reservation_fee_medium_nanos: u128,
    reservation_fee_large_nanos: u128,
}

const HF_HOST_CLASS_POLICIES: [HfHostClassPolicy; 3] = [
    HfHostClassPolicy {
        host_class: "cpu.small",
        min_model_bytes: 2 * 1024 * 1024 * 1024,
        min_disk_cache_bytes: 8 * 1024 * 1024 * 1024,
        min_ram_bytes: 8 * 1024 * 1024 * 1024,
        min_vram_bytes: 0,
        reservation_fee_small_nanos: 500,
        reservation_fee_medium_nanos: 750,
        reservation_fee_large_nanos: 1_000,
    },
    HfHostClassPolicy {
        host_class: "cpu.large",
        min_model_bytes: 8 * 1024 * 1024 * 1024,
        min_disk_cache_bytes: 32 * 1024 * 1024 * 1024,
        min_ram_bytes: 32 * 1024 * 1024 * 1024,
        min_vram_bytes: 0,
        reservation_fee_small_nanos: 1_000,
        reservation_fee_medium_nanos: 1_500,
        reservation_fee_large_nanos: 2_000,
    },
    HfHostClassPolicy {
        host_class: "gpu.large",
        min_model_bytes: 24 * 1024 * 1024 * 1024,
        min_disk_cache_bytes: 64 * 1024 * 1024 * 1024,
        min_ram_bytes: 64 * 1024 * 1024 * 1024,
        min_vram_bytes: 24 * 1024 * 1024 * 1024,
        reservation_fee_small_nanos: 2_500,
        reservation_fee_medium_nanos: 4_000,
        reservation_fee_large_nanos: 6_000,
    },
];

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
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
    HF_HOST_CLASS_POLICIES
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

fn hf_host_class_reservation_fee_nanos(
    host_class: &str,
    resource_profile: &SoraHfResourceProfileV1,
) -> Result<u128, InstructionExecutionError> {
    let policy = hf_host_class_policy(host_class)
        .ok_or_else(|| invalid_parameter(format!("unsupported model host class `{host_class}`")))?;
    Ok(match resource_profile.size_bucket() {
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Small => {
            policy.reservation_fee_small_nanos
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Medium => {
            policy.reservation_fee_medium_nanos
        }
        iroha_data_model::soracloud::SoraHfModelSizeBucketV1::Large => {
            policy.reservation_fee_large_nanos
        }
    })
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
    if state_transaction.chain_id.as_str() == TAIRA_TESTNET_CHAIN_ID {
        return Ok(());
    }

    let has_permission = state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|permission| permission.name() == CAN_MANAGE_SORACLOUD_PERMISSION)
        });
    if has_permission {
        Ok(())
    } else {
        Err(InstructionExecutionError::InvariantViolation(
            format!("not permitted: {CAN_MANAGE_SORACLOUD_PERMISSION}").into(),
        ))
    }
}

fn require_active_public_lane_validator(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let is_active_validator = state_transaction.world.public_lane_validators.iter().any(
        |((_lane_id, account_id), record)| {
            account_id == authority && record.status == PublicLaneValidatorStatus::Active
        },
    );
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

fn verify_bundle_provenance(
    authority: &AccountId,
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    initial_service_secrets: &BTreeMap<String, SecretEnvelopeV1>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| invalid_parameter("bundle provenance signature verification failed"))?;
    Ok(())
}

fn verify_app_infra_provenance(
    authority: &AccountId,
    manifest: &SoraAppInfraManifestV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "app infra provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_app_infra_provenance_payload(manifest).map_err(|err| {
        invalid_parameter(format!("failed to encode app infra provenance: {err}"))
    })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| invalid_parameter("app infra provenance signature verification failed"))?;
    Ok(())
}

fn verify_rollback_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    target_version: Option<&str>,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "rollback provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_rollback_provenance_payload(service_name.as_ref(), target_version)
        .map_err(|err| invalid_parameter(format!("failed to encode rollback provenance: {err}")))?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("service config provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_service_config_delete_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    config_name: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("service config delete provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_service_secret_set_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    secret_name: &str,
    secret: &SecretEnvelopeV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "service secret provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_set_service_secret_provenance_payload(service_name.as_ref(), secret_name, secret)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode service secret provenance: {err}"))
            })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("service secret provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_service_secret_delete_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    secret_name: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("service secret delete provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("state mutation provenance signature verification failed")
        })?;
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
        residual_multiple_bound,
        bound_mode,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to derive FHE input admission statement hash: {err}"
        ))
    })
}

fn validate_soracloud_fhe_input_envelope_shape(
    params: &BfvParameters,
    payload: &[u8],
) -> Result<(), InstructionExecutionError> {
    let envelope = decode_soracloud_fhe_envelope(payload)?;
    validate_soracloud_fhe_envelope_shape(params, &envelope, "fhe input admission")
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

fn proof_attachment_envelope(
    attachment: &ProofAttachment,
) -> Result<OpenVerifyEnvelope, InstructionExecutionError> {
    if attachment.backend != attachment.proof.backend {
        return Err(invalid_parameter(
            "fhe input admission proof backend mismatch",
        ));
    }
    if attachment.vk_ref.backend != attachment.backend {
        return Err(invalid_parameter(
            "fhe input admission verifier backend mismatch",
        ));
    }
    if attachment.vk_ref.name.trim().is_empty() {
        return Err(invalid_parameter(
            "fhe input admission verifier name must not be empty",
        ));
    }
    if !crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str()) {
        return Err(invalid_parameter(
            "Soracloud FHE input admission requires a STARK/FRI proof backend",
        ));
    }
    norito::decode_from_bytes::<OpenVerifyEnvelope>(&attachment.proof.bytes).map_err(|err| {
        invalid_parameter(format!(
            "invalid FHE input admission OpenVerifyEnvelope: {err}"
        ))
    })
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
    if envelope.public_inputs != SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(invalid_parameter(
            "FHE input admission proof public-input schema mismatch",
        ));
    }
    let open =
        norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes).map_err(|err| {
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
    if let Some(envelope_hash) = attachment.envelope_hash {
        let expected = <[u8; Hash::LENGTH]>::from(Hash::new(&attachment.proof.bytes));
        if envelope_hash != expected {
            return Err(invalid_parameter(
                "FHE input admission proof envelope_hash mismatch",
            ));
        }
    }
    Ok(())
}

fn verify_soracloud_fhe_input_admission_backend(
    attachment: &ProofAttachment,
    envelope: &OpenVerifyEnvelope,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
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
    if record.status != ConfidentialStatus::Active {
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
    if record.gas_schedule_id.is_none() {
        return Err(InstructionExecutionError::InvariantViolation(
            "FHE input admission verifying key missing gas_schedule_id".into(),
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
    if let Some(vk_commitment) = attachment.vk_commitment
        && vk_commitment != record.commitment
    {
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

    state_transaction
        .register_confidential_proof(attachment.proof.bytes.len())
        .map_err(|err| {
            invalid_parameter(format!(
                "FHE input admission proof quota accounting failed: {err}"
            ))
        })?;
    let ok = state_transaction
        .lookup_preverified_proof(&attachment.proof, &attachment.vk_ref, record.commitment)
        .unwrap_or_else(|| {
            crate::zk::verify_backend_with_timing_checked(
                attachment.backend.as_str(),
                &attachment.proof,
                Some(&vk_box),
                &state_transaction.zk,
            )
            .ok
        });
    if !ok {
        return Err(invalid_parameter(
            "FHE input admission proof verification failed",
        ));
    }
    Ok(())
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
) -> Result<Option<(u128, BfvCiphertextBoundModeV1)>, InstructionExecutionError> {
    let Some(proof) = proof else {
        return Ok(None);
    };
    proof
        .validate()
        .map_err(|err| invalid_parameter(format!("invalid FHE input admission proof: {err}")))?;
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
    let value_size_bytes = value_size_bytes
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires value_size_bytes"))?;
    let value_payload = value_payload
        .ok_or_else(|| invalid_parameter("FHE input admission proof requires value_payload"))?;
    let payload_commitment = payload_commitment.ok_or_else(|| {
        invalid_parameter("FHE input admission proof requires payload commitment")
    })?;

    let params = ram_lfe_bfv_parameters_v1();
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
            return Err(invalid_parameter(
                "bounded-noise FHE input admission proofs are not yet supported by the runtime verifier",
            ));
        }
    }
    validate_soracloud_fhe_input_envelope_shape(&params, value_payload)?;

    let expected_statement_hash = expected_fhe_input_admission_statement_hash(
        service_name,
        binding_name,
        state_key,
        operation,
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
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
    verify_soracloud_fhe_input_admission_backend(&proof.proof, &envelope, state_transaction)?;
    Ok(Some((proof.residual_multiple_bound, proof.bound_mode)))
}

fn verify_fhe_job_run_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    job: FheJobSpecV1,
    policy: FheExecutionPolicyV1,
    param_set: FheParamSetV1,
    evaluation_keys: BfvEvaluationKeyBundle,
    evaluation_key_refresh_transcript: BfvEvaluationKeyRefreshTranscriptV1,
    governance_tx_hash: Hash,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "fhe job provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_fhe_job_run_provenance_payload(
        service_name.as_ref(),
        binding_name.as_ref(),
        job,
        policy,
        param_set,
        evaluation_keys,
        evaluation_key_refresh_transcript,
        governance_tx_hash,
    )
    .map_err(|err| invalid_parameter(format!("failed to encode fhe job provenance: {err}")))?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| invalid_parameter("fhe job provenance signature verification failed"))?;
    Ok(())
}

fn verify_decryption_request_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    policy: DecryptionAuthorityPolicyV1,
    request: DecryptionRequestV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "decryption request provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_decryption_request_provenance_payload(service_name.as_ref(), policy, request)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode decryption provenance: {err}"))
            })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("decryption request provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("training job start provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("training checkpoint provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_training_job_retry_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    job_id: &str,
    reason: &str,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "training retry provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_training_job_retry_provenance_payload(service_name.as_ref(), job_id, reason)
            .map_err(|err| {
                invalid_parameter(format!("failed to encode training retry provenance: {err}"))
            })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("training retry provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("model artifact register provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("model weight register provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("model weight promote provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("model weight rollback provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_uploaded_model_bundle_register_provenance(
    authority: &AccountId,
    bundle: &SoraUploadedModelBundleV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("uploaded model bundle provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
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
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("uploaded model finalize provenance signature verification failed")
        })?;
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
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(signer_mismatch));
    }
    provenance
        .signature
        .verify(&provenance.signer, &payload)
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
) -> u64 {
    let key = format!("{asset_definition}:{day_bucket}");
    record
        .wallet_daily_spend
        .get(&key)
        .map(|entry| entry.spent_nanos)
        .unwrap_or(0)
}

fn wallet_record_spend(
    record: &mut SoraAgentApartmentRecordV1,
    asset_definition: &str,
    day_bucket: u64,
    spent_nanos: u64,
) {
    let key = format!("{asset_definition}:{day_bucket}");
    record.wallet_daily_spend.insert(
        key,
        SoraAgentWalletDailySpendEntryV1 {
            asset_definition: asset_definition.to_owned(),
            day_bucket,
            spent_nanos,
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
    match lease.status_at(current_sequence, accounted_storage_bytes) {
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
            let tentative_total = binding_total_bytes
                .saturating_sub(existing_size)
                .saturating_add(value_size_bytes);
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
) -> Option<SoraServiceLeaseStateV1> {
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService {
        return None;
    }

    let economics = &bundle.service.economics;
    let existing_lease = existing.and_then(|deployment| deployment.service_lease.as_ref());
    let quota_class = economics.quota_class.clone();
    let deployment_deposit_nanos =
        existing_lease.map_or(economics.deployment_deposit_nanos.get(), |lease| {
            lease
                .deployment_deposit_nanos
                .max(economics.deployment_deposit_nanos.get())
        });
    let prepaid_runtime_balance_nanos =
        existing_lease.map_or(economics.prepaid_runtime_balance_nanos.get(), |lease| {
            if extend_terms {
                lease
                    .prepaid_runtime_balance_nanos
                    .saturating_add(economics.prepaid_runtime_balance_nanos.get())
            } else {
                lease.prepaid_runtime_balance_nanos
            }
        });
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

    Some(SoraServiceLeaseStateV1 {
        schema_version: SORA_SERVICE_LEASE_STATE_VERSION_V1,
        status,
        quota_class,
        deployment_deposit_nanos,
        prepaid_runtime_balance_nanos,
        runtime_nanos_per_sequence: economics.runtime_nanos_per_sequence.get(),
        storage_nanos_per_gib_sequence: economics.storage_nanos_per_gib_sequence.get(),
        egress_nanos_per_mib: economics.egress_nanos_per_mib.get(),
        lease_started_sequence,
        lease_expires_sequence,
        last_billed_sequence: existing_lease
            .map_or(sequence, |lease| lease.last_billed_sequence)
            .clamp(lease_started_sequence, lease_expires_sequence),
        accounted_egress_bytes: existing_lease.map_or(0, |lease| lease.accounted_egress_bytes),
        last_status_reason: existing_lease.and_then(|lease| lease.last_status_reason.clone()),
    })
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
        if !deployment.hosted_service_lease_active_at(current_sequence) {
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

fn recompute_hf_placement_total_reservation_fee_nanos(
    placement: &mut SoraHfPlacementRecordV1,
) -> Result<(), InstructionExecutionError> {
    placement.total_reservation_fee_nanos =
        placement
            .assigned_hosts
            .iter()
            .try_fold(0_u128, |total, host| {
                hf_host_class_reservation_fee_nanos(&host.host_class, &placement.resource_profile)
                    .map(|fee| total.saturating_add(fee))
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
            recompute_hf_placement_total_reservation_fee_nanos(&mut placement)?;
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
    let mut matches = state_transaction
        .world
        .soracloud_hf_placements
        .iter()
        .filter_map(|(_pool_id, placement)| {
            (placement.placement_id == *placement_id).then_some(placement.clone())
        })
        .collect::<Vec<_>>();
    match matches.len() {
        1 => Ok(matches.pop().expect("one placement match")),
        0 => Err(InstructionExecutionError::InvariantViolation(
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
            &mut state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.staking,
            lane_id,
            validator_account_id,
            slash_id,
            &amount,
            recorded_at_ms,
            #[cfg(feature = "telemetry")]
            Some(state_transaction.telemetry),
            #[cfg(not(feature = "telemetry"))]
            None,
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
            recompute_hf_placement_total_reservation_fee_nanos(&mut placement)?;
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
    for ((_lane_id, validator_account_id), record) in
        state_transaction.world.public_lane_validators.iter()
    {
        if record.status != PublicLaneValidatorStatus::Active {
            continue;
        }
        let stake = numeric_to_u128(&record.total_stake)?;
        let entry = stakes.entry(validator_account_id.clone()).or_insert(0_u128);
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
    let entropy = u128::from_be_bytes(
        digest_bytes[..16]
            .try_into()
            .expect("digest slice length is fixed"),
    )
    .saturating_add(1);
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
        total_reservation_fee_nanos: 0,
        last_rebalance_at_ms: now_ms.max(1),
        last_error: None,
    };
    recompute_hf_placement_total_reservation_fee_nanos(&mut placement)?;
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

fn prorated_window_fee_nanos(
    window_fee_nanos: u128,
    remaining_ms: u64,
    lease_term_ms: u64,
) -> u128 {
    if lease_term_ms == 0 {
        return 0;
    }
    window_fee_nanos.saturating_mul(u128::from(remaining_ms)) / u128::from(lease_term_ms)
}

fn distribute_hf_join_refunds(
    authority: &AccountId,
    lease_asset_definition_id: &AssetDefinitionId,
    now_ms: u64,
    join_fee_nanos: u128,
    existing_members: &mut [SoraHfSharedLeaseMemberV1],
    state_transaction: &mut StateTransaction<'_, '_>,
    storage_refund: bool,
) -> Result<(), InstructionExecutionError> {
    if existing_members.is_empty() || join_fee_nanos == 0 {
        return Ok(());
    }

    let member_count_u128 = u128::try_from(existing_members.len()).unwrap_or(u128::MAX);
    let base_refund_nanos = join_fee_nanos / member_count_u128;
    let remainder = usize::try_from(join_fee_nanos % member_count_u128).unwrap_or(0);
    for (index, existing_member) in existing_members.iter_mut().enumerate() {
        let refund_nanos = base_refund_nanos + u128::from((index < remainder) as u8);
        if storage_refund {
            transfer_hf_shared_lease_amount(
                authority,
                lease_asset_definition_id,
                refund_nanos,
                &existing_member.account_id,
                state_transaction,
            )?;
            existing_member.total_refunded_nanos = existing_member
                .total_refunded_nanos
                .saturating_add(refund_nanos);
        } else {
            existing_member.total_compute_refunded_nanos = existing_member
                .total_compute_refunded_nanos
                .saturating_add(refund_nanos);
        }
        existing_member.updated_at_ms = now_ms;
        record_hf_shared_lease_member(state_transaction, existing_member.clone())?;
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
    amount_nanos: u128,
    destination: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if amount_nanos == 0 || authority == destination {
        return Ok(());
    }
    let source_asset_id = AssetId::new(lease_asset_definition_id.clone(), authority.clone());
    iroha_data_model::isi::Transfer::<Asset, Numeric, iroha_data_model::account::Account>::asset_numeric(
        source_asset_id,
        Numeric::new(amount_nanos, 0),
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
    amount_nanos: u128,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if amount_nanos == 0 {
        return Ok(());
    }
    let fee_asset_definition_id = resolve_fee_asset_definition_id(state_transaction)?;
    let sink_account = resolve_fee_sink_account(state_transaction)?;
    transfer_hf_shared_lease_amount(
        authority,
        &fee_asset_definition_id,
        amount_nanos,
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
        member.last_charge_nanos = 0;
        member.last_compute_charge_nanos = 0;
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
    sponsor_member.last_charge_nanos = 0;
    sponsor_member.last_compute_charge_nanos = 0;
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
    pool.base_fee_nanos = next_window.base_fee_nanos;
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
    base_fee_nanos: u128,
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
        base_fee_nanos,
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
    base_fee_nanos: u128,
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
        base_fee_nanos,
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
    u16::try_from(u64::BITS - value.leading_zeros()).expect("u64 bit width fits in u16")
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
        .expect("parameter-set validation requires a non-empty modulus chain")
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
    let archived = norito::from_bytes::<BfvIdentifierCiphertext>(payload)
        .map_err(|err| invalid_parameter(format!("invalid FHE ciphertext envelope: {err}")))?;
    Ok(norito::core::NoritoDeserialize::deserialize(archived))
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
            .unwrap_or(BfvCiphertextBoundModeV1::ExactResidualMultiple);
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
        let envelope = decode_soracloud_fhe_envelope(&entry.payload)?;
        let context = format!("fhe input `{}`", input.state_key);
        validate_soracloud_fhe_envelope_shape(params, &envelope, &context)?;
        inputs.push(LoadedSoracloudFheInput { envelope, bound });
    }
    Ok(inputs)
}

fn ensure_matching_fhe_slots(
    envelopes: &[BfvIdentifierCiphertext],
) -> Result<usize, InstructionExecutionError> {
    let first_slots = envelopes
        .first()
        .map(|envelope| envelope.slots.len())
        .ok_or_else(|| invalid_parameter("fhe job requires at least one input envelope"))?;
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
    Ok(first_slots)
}

fn fold_fhe_slots(
    params: &BfvParameters,
    inputs: &[BfvIdentifierCiphertext],
    mut combine: impl FnMut(
        &BfvCiphertext,
        &BfvCiphertext,
    ) -> Result<BfvCiphertext, InstructionExecutionError>,
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    ensure_matching_fhe_slots(inputs)?;
    let mut slots = inputs
        .first()
        .expect("input presence checked above")
        .slots
        .clone();
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
        output_slots.push(level.pop().expect("non-empty level is maintained"));
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
        FheJobOperationV1::Add => BfvEvaluationPlan::add(inputs.len())
            .map_err(|err| invalid_parameter(format!("invalid FHE add plan: {err}")))?,
        FheJobOperationV1::Multiply => {
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
        FheJobOperationV1::RotateLeft => BfvEvaluationPlan::rotate_left(inputs.len())
            .map_err(|err| invalid_parameter(format!("invalid FHE rotate plan: {err}")))?,
        FheJobOperationV1::Bootstrap => {
            BfvEvaluationPlan::bootstrap_refresh(inputs.len(), job.bootstrap_count)
                .map_err(|err| invalid_parameter(format!("invalid FHE bootstrap plan: {err}")))?
        }
    };
    budget
        .validate_plan(plan)
        .map_err(|err| invalid_parameter(format!("FHE evaluation budget exceeded: {err}")))
}

fn soracloud_fhe_job_output_residual_multiple_bound(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
    input_residual_bounds: &[u128],
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
            ensure_matching_fhe_slots(inputs)?;
            let input_bound = *input_residual_bounds
                .first()
                .ok_or_else(|| invalid_parameter("fhe rotate requires residual metadata"))?;
            let slots = &inputs.first().expect("input presence checked above").slots;
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
            bfv_bootstrap_key_refresh_output_residual_multiple_bound(
                params,
                bootstrap_key,
                input_bound,
                job.bootstrap_count,
            )
            .map(Some)
            .map_err(|err| {
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
            ensure_matching_fhe_slots(inputs)?;
            let input_bound = *input_noise_bounds
                .first()
                .ok_or_else(|| invalid_parameter("fhe rotate requires bounded-noise metadata"))?;
            let slots = &inputs.first().expect("input presence checked above").slots;
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
            let output_bounds = bfv_rotate_slots_left_bounded_noise_output_bounds(
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
            bfv_bootstrap_key_refresh_bounded_noise_output_bound(
                params,
                bootstrap_key,
                input_bound,
                job.bootstrap_count,
            )
            .map(Some)
            .map_err(|err| {
                invalid_parameter(format!("FHE bootstrap bounded-noise bound exceeded: {err}"))
            })
        }
    }
}

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
    )?;
    let output = execute_soracloud_fhe_job(params, evaluation_keys, job, inputs)?;
    Ok((output, output_residual_bound))
}

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
    )?;
    let output = execute_soracloud_fhe_job_bounded_noise(params, evaluation_keys, job, inputs)?;
    Ok((output, output_noise_bound))
}

fn execute_soracloud_fhe_job(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    let rns_chain = registered_bfv_rns_modulus_chain(params)
        .map_err(|err| invalid_parameter(format!("invalid registered BFV RNS chain: {err}")))?;
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    match job.operation {
        FheJobOperationV1::Add => fold_fhe_slots(params, inputs, |lhs, rhs| {
            add_ciphertexts_rns_exact(params, &rns_chain, lhs, rhs)
                .map_err(|err| invalid_parameter(format!("FHE add failed: {err}")))
        }),
        FheJobOperationV1::Multiply => fold_fhe_slots_balanced(params, inputs, |lhs, rhs| {
            multiply_ciphertexts_rns_exact(
                params,
                &rns_chain,
                &evaluation_keys.relinearization_key,
                lhs,
                rhs,
            )
            .map_err(|err| invalid_parameter(format!("FHE multiply failed: {err}")))
        }),
        FheJobOperationV1::RotateLeft => {
            ensure_matching_fhe_slots(inputs)?;
            let slots = &inputs.first().expect("input presence checked above").slots;
            if slots.len() == 1 {
                let rotated = rotate_packed_ciphertext_slots_left_with_galois_keys_rns_exact(
                    params,
                    &rns_chain,
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
                rotate_ciphertext_slots_left_rns_exact(params, &rns_chain, rotation_key, slots)
                    .map_err(|err| invalid_parameter(format!("FHE rotate failed: {err}")))?;
            Ok(BfvIdentifierCiphertext { slots })
        }
        FheJobOperationV1::Bootstrap => {
            ensure_matching_fhe_slots(inputs)?;
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bootstrap operation")
            })?;
            if job.bootstrap_count > bootstrap_key.max_refresh_rounds {
                return Err(invalid_parameter(format!(
                    "bootstrap_count {} exceeds BFV bootstrap key max_refresh_rounds {}",
                    job.bootstrap_count, bootstrap_key.max_refresh_rounds
                )));
            }
            let slots = inputs
                .first()
                .expect("input presence checked above")
                .slots
                .iter()
                .map(|slot| {
                    let mut refreshed = slot.clone();
                    for round_index in 0..job.bootstrap_count {
                        refreshed = bootstrap_ciphertext_rns_exact_round(
                            params,
                            &rns_chain,
                            bootstrap_key,
                            &refreshed,
                            round_index,
                        )
                        .map_err(|err| invalid_parameter(format!("FHE bootstrap failed: {err}")))?;
                    }
                    Ok::<BfvCiphertext, InstructionExecutionError>(refreshed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(BfvIdentifierCiphertext { slots })
        }
    }
}

fn execute_soracloud_fhe_job_bounded_noise(
    params: &BfvParameters,
    evaluation_keys: &BfvEvaluationKeyBundle,
    job: &FheJobSpecV1,
    inputs: &[BfvIdentifierCiphertext],
) -> Result<BfvIdentifierCiphertext, InstructionExecutionError> {
    let rns_chain = registered_bfv_rns_modulus_chain(params)
        .map_err(|err| invalid_parameter(format!("invalid registered BFV RNS chain: {err}")))?;
    ensure_matching_fhe_slots(inputs)?;
    validate_soracloud_fhe_evaluation_budget(job, inputs)?;
    match job.operation {
        FheJobOperationV1::Add => fold_fhe_slots(params, inputs, |lhs, rhs| {
            add_ciphertexts_rns_exact(params, &rns_chain, lhs, rhs)
                .map_err(|err| invalid_parameter(format!("FHE bounded-noise add failed: {err}")))
        }),
        FheJobOperationV1::Multiply => fold_fhe_slots_balanced(params, inputs, |lhs, rhs| {
            multiply_ciphertexts_bounded_noise_rns_exact(
                params,
                &rns_chain,
                &evaluation_keys.relinearization_key,
                lhs,
                rhs,
            )
            .map_err(|err| invalid_parameter(format!("FHE bounded-noise multiply failed: {err}")))
        }),
        FheJobOperationV1::RotateLeft => {
            ensure_matching_fhe_slots(inputs)?;
            let slots = &inputs.first().expect("input presence checked above").slots;
            if slots.len() == 1 {
                let rotated =
                    rotate_packed_ciphertext_slots_left_with_galois_keys_bounded_noise_rns_exact(
                        params,
                        &rns_chain,
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
            let slots = rotate_ciphertext_slots_left_bounded_noise_rns_exact(
                params,
                &rns_chain,
                rotation_key,
                slots,
            )
            .map_err(|err| invalid_parameter(format!("FHE bounded-noise rotate failed: {err}")))?;
            Ok(BfvIdentifierCiphertext { slots })
        }
        FheJobOperationV1::Bootstrap => {
            ensure_matching_fhe_slots(inputs)?;
            let bootstrap_key = evaluation_keys.bootstrap_key.as_ref().ok_or_else(|| {
                invalid_parameter("missing BFV bootstrap key for bounded-noise bootstrap operation")
            })?;
            if job.bootstrap_count > bootstrap_key.max_refresh_rounds {
                return Err(invalid_parameter(format!(
                    "bootstrap_count {} exceeds BFV bootstrap key max_refresh_rounds {}",
                    job.bootstrap_count, bootstrap_key.max_refresh_rounds
                )));
            }
            let slots = inputs
                .first()
                .expect("input presence checked above")
                .slots
                .iter()
                .map(|slot| {
                    let mut refreshed = slot.clone();
                    for round_index in 0..job.bootstrap_count {
                        refreshed = bootstrap_ciphertext_bounded_noise_rns_exact_round(
                            params,
                            &rns_chain,
                            bootstrap_key,
                            &refreshed,
                            round_index,
                        )
                        .map_err(|err| {
                            invalid_parameter(format!("FHE bounded-noise bootstrap failed: {err}"))
                        })?;
                    }
                    Ok::<BfvCiphertext, InstructionExecutionError>(refreshed)
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
    );
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
            build_http_service_lease_state(&bundle, Some(&existing), sequence, false);
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
        let (admitted_fhe_residual_bound, admitted_fhe_bound_mode) =
            admitted_fhe_bound.map_or((None, None), |(bound, mode)| (Some(bound), Some(mode)));

        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = apply_soracloud_state_mutation(
            state_transaction,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            value_payload,
            encryption,
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
            self.policy.clone(),
            self.param_set.clone(),
            self.evaluation_keys.clone(),
            self.evaluation_key_refresh_transcript.clone(),
            self.governance_tx_hash,
            &self.provenance,
        )?;
        self.param_set
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        self.policy
            .validate_for_param_set(&self.param_set)
            .map_err(|err| invalid_parameter(err.to_string()))?;
        self.job
            .validate_for_execution(&self.policy, &self.param_set)
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let ciphertext_bound_mode = soracloud_fhe_ciphertext_bound_mode(&self.policy);
        let bfv_params = registered_soracloud_bfv_parameters(&self.param_set)?;
        self.evaluation_keys
            .validate(&bfv_params)
            .map_err(|err| invalid_parameter(format!("invalid BFV evaluation keys: {err}")))?;
        verify_soracloud_fhe_evaluation_key_digest(
            &bfv_params,
            &self.policy,
            &self.evaluation_keys,
        )?;
        verify_soracloud_fhe_refresh_transcript_digest(
            &bfv_params,
            &self.policy,
            &self.evaluation_keys,
            &self.evaluation_key_refresh_transcript,
        )?;
        let loaded_inputs = load_soracloud_fhe_inputs(
            &bfv_params,
            state_transaction,
            &self.service_name,
            &self.binding_name,
            &self.job,
            ciphertext_bound_mode,
        )?;
        let (input_envelopes, input_bounds): (Vec<_>, Vec<_>) = loaded_inputs
            .into_iter()
            .map(|input| (input.envelope, input.bound))
            .unzip();
        let (output_envelope, output_bound) = match ciphertext_bound_mode {
            BfvCiphertextBoundModeV1::ExactResidualMultiple => {
                execute_soracloud_fhe_job_with_residual_bounds(
                    &bfv_params,
                    &self.evaluation_keys,
                    &self.job,
                    &input_envelopes,
                    &input_bounds,
                )?
            }
            BfvCiphertextBoundModeV1::BoundedNoise => {
                execute_soracloud_fhe_job_with_bounded_noise_bounds(
                    &bfv_params,
                    &self.evaluation_keys,
                    &self.job,
                    &input_envelopes,
                    &input_bounds,
                )?
            }
        };
        let output_payload = norito::to_bytes(&output_envelope)
            .map_err(|err| invalid_parameter(format!("failed to encode FHE output: {err}")))?;
        let output_payload_bytes = u64::try_from(output_payload.len())
            .map_err(|_| invalid_parameter("FHE output payload length exceeds u64 range"))?;

        let sequence = next_soracloud_audit_sequence(state_transaction);
        let (deployment, bundle) = load_active_bundle(state_transaction, &self.service_name)?;
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
        if output_payload_bytes > self.policy.max_ciphertext_bytes.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "fhe output size {output_payload_bytes} exceeds policy max_ciphertext_bytes {}",
                    self.policy.max_ciphertext_bytes
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
        let tentative_total = binding_total_bytes
            .saturating_sub(existing_size)
            .saturating_add(output_payload_bytes);
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
                fhe_residual_multiple_bound: output_bound,
                fhe_bound_mode: Some(ciphertext_bound_mode),
                last_update_sequence: sequence,
                governance_tx_hash: self.governance_tx_hash,
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
                governance_tx_hash: Some(self.governance_tx_hash),
                binding_name: Some(self.binding_name),
                state_key: Some(output_state_key),
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
            base_fee_nanos,
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
        if base_fee_nanos == 0 {
            return Err(invalid_parameter(
                "base_fee_nanos must be greater than zero",
            ));
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
            base_fee_nanos,
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
            if pool.lease_asset_definition_id != lease_asset_definition_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different settlement asset".into(),
                ));
            }
            if pool.base_fee_nanos != base_fee_nanos {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different base_fee_nanos".into(),
                ));
            }
            bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
            member.updated_at_ms = now_ms;
            member.last_charge_nanos = 0;
            member.last_compute_charge_nanos = 0;
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
                    charged_nanos: 0,
                    refunded_nanos: 0,
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
            if pool.base_fee_nanos != base_fee_nanos {
                return Err(InstructionExecutionError::InvariantViolation(
                    "existing shared lease pool uses a different base_fee_nanos".into(),
                ));
            }

            let mut existing_members = canonical_hf_member_order(state_transaction, &pool_id);
            pool.active_member_count = u32::try_from(existing_members.len()).unwrap_or(u32::MAX);
            let remaining_ms = pool.window_expires_at_ms.saturating_sub(now_ms);
            let remaining_fee_nanos =
                prorated_window_fee_nanos(base_fee_nanos, remaining_ms, lease_term_ms);
            let join_fee_nanos = remaining_fee_nanos
                / u128::try_from(existing_members.len().saturating_add(1)).unwrap_or(u128::MAX);
            let placement = ensure_hf_placement_for_active_pool(
                state_transaction,
                &pool,
                &resource_profile,
                now_ms,
            )?;
            let remaining_compute_fee_nanos = prorated_window_fee_nanos(
                placement.total_reservation_fee_nanos,
                remaining_ms,
                lease_term_ms,
            );
            let join_compute_fee_nanos = remaining_compute_fee_nanos
                / u128::try_from(existing_members.len().saturating_add(1)).unwrap_or(u128::MAX);

            distribute_hf_join_refunds(
                authority,
                &lease_asset_definition_id,
                now_ms,
                join_fee_nanos,
                &mut existing_members,
                state_transaction,
                true,
            )?;
            distribute_hf_join_refunds(
                authority,
                &lease_asset_definition_id,
                now_ms,
                join_compute_fee_nanos,
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
                total_paid_nanos: 0,
                total_refunded_nanos: 0,
                last_charge_nanos: 0,
                total_compute_paid_nanos: 0,
                total_compute_refunded_nanos: 0,
                last_compute_charge_nanos: 0,
                service_bindings: std::collections::BTreeSet::new(),
                apartment_bindings: std::collections::BTreeSet::new(),
            });
            member.status = SoraHfSharedLeaseMemberStatusV1::Active;
            member.joined_at_ms = now_ms;
            member.updated_at_ms = now_ms;
            member.total_paid_nanos = member.total_paid_nanos.saturating_add(join_fee_nanos);
            member.last_charge_nanos = join_fee_nanos;
            member.total_compute_paid_nanos = member
                .total_compute_paid_nanos
                .saturating_add(join_compute_fee_nanos);
            member.last_compute_charge_nanos = join_compute_fee_nanos;
            bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
            record_hf_shared_lease_member(state_transaction, member)?;

            pool.active_member_count =
                u32::try_from(existing_members.len().saturating_add(1)).unwrap_or(u32::MAX);
            record_hf_shared_lease_pool(state_transaction, pool.clone())?;
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
                    charged_nanos: join_fee_nanos,
                    refunded_nanos: 0,
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
            base_fee_nanos,
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
        let sink_account = resolve_fee_sink_account(state_transaction)?;
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            base_fee_nanos.saturating_add(placement.total_reservation_fee_nanos),
            &sink_account,
            state_transaction,
        )?;
        let previous_paid_nanos = existing_member
            .as_ref()
            .map(|member| member.total_paid_nanos)
            .unwrap_or(0);
        let previous_refunded_nanos = existing_member
            .as_ref()
            .map(|member| member.total_refunded_nanos)
            .unwrap_or(0);
        let previous_compute_paid_nanos = existing_member
            .as_ref()
            .map(|member| member.total_compute_paid_nanos)
            .unwrap_or(0);
        let previous_compute_refunded_nanos = existing_member
            .as_ref()
            .map(|member| member.total_compute_refunded_nanos)
            .unwrap_or(0);
        let mut member = SoraHfSharedLeaseMemberV1 {
            schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            pool_id,
            source_id,
            account_id: authority.clone(),
            status: SoraHfSharedLeaseMemberStatusV1::Active,
            joined_at_ms: now_ms,
            updated_at_ms: now_ms,
            total_paid_nanos: previous_paid_nanos.saturating_add(base_fee_nanos),
            total_refunded_nanos: previous_refunded_nanos,
            last_charge_nanos: base_fee_nanos,
            total_compute_paid_nanos: previous_compute_paid_nanos
                .saturating_add(placement.total_reservation_fee_nanos),
            total_compute_refunded_nanos: previous_compute_refunded_nanos,
            last_compute_charge_nanos: placement.total_reservation_fee_nanos,
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
                charged_nanos: base_fee_nanos,
                refunded_nanos: 0,
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
        member.last_charge_nanos = 0;
        member.last_compute_charge_nanos = 0;
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
                charged_nanos: 0,
                refunded_nanos: 0,
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
            base_fee_nanos,
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
        if base_fee_nanos == 0 {
            return Err(invalid_parameter(
                "base_fee_nanos must be greater than zero",
            ));
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
            base_fee_nanos,
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
            let mut member = existing_member.ok_or_else(|| {
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
            transfer_hf_shared_lease_amount(
                authority,
                &lease_asset_definition_id,
                base_fee_nanos.saturating_add(planned_placement.total_reservation_fee_nanos),
                &sink_account,
                state_transaction,
            )?;

            member.updated_at_ms = now_ms;
            member.total_paid_nanos = member.total_paid_nanos.saturating_add(base_fee_nanos);
            member.last_charge_nanos = base_fee_nanos;
            member.total_compute_paid_nanos = member
                .total_compute_paid_nanos
                .saturating_add(planned_placement.total_reservation_fee_nanos);
            member.last_compute_charge_nanos = planned_placement.total_reservation_fee_nanos;
            record_hf_shared_lease_member(state_transaction, member)?;

            let next_window = SoraHfSharedLeaseQueuedWindowV1 {
                sponsor_account_id: authority.clone(),
                model_name: model_name.clone(),
                lease_asset_definition_id,
                base_fee_nanos,
                compute_reservation_fee_nanos: planned_placement.total_reservation_fee_nanos,
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
                    charged_nanos: base_fee_nanos,
                    refunded_nanos: 0,
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
        pool.base_fee_nanos = base_fee_nanos;
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
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            base_fee_nanos.saturating_add(placement.total_reservation_fee_nanos),
            &sink_account,
            state_transaction,
        )?;
        record_hf_shared_lease_pool(state_transaction, pool.clone())?;

        let previous_paid_nanos = existing_member
            .as_ref()
            .map(|member| member.total_paid_nanos)
            .unwrap_or(0);
        let previous_refunded_nanos = existing_member
            .as_ref()
            .map(|member| member.total_refunded_nanos)
            .unwrap_or(0);
        let previous_compute_paid_nanos = existing_member
            .as_ref()
            .map(|member| member.total_compute_paid_nanos)
            .unwrap_or(0);
        let previous_compute_refunded_nanos = existing_member
            .as_ref()
            .map(|member| member.total_compute_refunded_nanos)
            .unwrap_or(0);
        let mut member = SoraHfSharedLeaseMemberV1 {
            schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            pool_id,
            source_id,
            account_id: authority.clone(),
            status: SoraHfSharedLeaseMemberStatusV1::Active,
            joined_at_ms: now_ms,
            updated_at_ms: now_ms,
            total_paid_nanos: previous_paid_nanos.saturating_add(base_fee_nanos),
            total_refunded_nanos: previous_refunded_nanos,
            last_charge_nanos: base_fee_nanos,
            total_compute_paid_nanos: previous_compute_paid_nanos
                .saturating_add(placement.total_reservation_fee_nanos),
            total_compute_refunded_nanos: previous_compute_refunded_nanos,
            last_compute_charge_nanos: placement.total_reservation_fee_nanos,
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
                charged_nanos: base_fee_nanos,
                refunded_nanos: 0,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
            amount_nanos,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_asset_definition = asset_definition.trim().to_owned();
        let payload = encode_agent_wallet_spend_provenance_payload(
            apartment_name.as_ref(),
            normalized_asset_definition.as_str(),
            amount_nanos,
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
        if amount_nanos == 0 {
            return Err(invalid_parameter("amount_nanos must be greater than zero"));
        }
        let canonical_asset_definition = resolve_agent_asset_definition_literal(
            state_transaction,
            &normalized_asset_definition,
        )?
        .to_string();

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
        if amount_nanos > spend_limit.max_per_tx_nanos.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "requested amount {amount_nanos} exceeds max_per_tx_nanos {} for asset `{canonical_asset_definition}`",
                    spend_limit.max_per_tx_nanos.get()
                )
                .into(),
            ));
        }

        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(&record, &canonical_asset_definition, day_bucket);
        let projected_day_spent = current_day_spent.checked_add(amount_nanos).ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("wallet daily spend overflow for apartment `{apartment_name}`").into(),
            )
        })?;
        if projected_day_spent > spend_limit.max_per_day_nanos.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "projected daily spend {projected_day_spent} exceeds max_per_day_nanos {} for asset `{canonical_asset_definition}`",
                    spend_limit.max_per_day_nanos.get()
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
                    amount_nanos,
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
                amount_nanos: Some(amount_nanos),
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
        let spend_limit = agent_spend_limit_for_asset_definition(
            state_transaction,
            &record,
            &pending.asset_definition,
        )?;
        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(&record, &pending.asset_definition, day_bucket);
        let projected_day_spent = current_day_spent
            .checked_add(pending.amount_nanos)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("wallet daily spend overflow for apartment `{apartment_name}`").into(),
                )
            })?;
        if projected_day_spent > spend_limit.max_per_day_nanos.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "projected daily spend {projected_day_spent} exceeds max_per_day_nanos {} for asset `{}`",
                    spend_limit.max_per_day_nanos.get(),
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
        let event_amount_nanos = pending.amount_nanos;
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
                amount_nanos: Some(event_amount_nanos),
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
                    amount_nanos: None,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
                amount_nanos: None,
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
                signer: authority.signatory().clone(),
                request_id: Some(normalized_run_id.to_owned()),
                asset_definition: None,
                amount_nanos: None,
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
            record.pricing_policy.storage_xor_nanos,
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
        require_soracloud_runtime_authority(authority, state_transaction)?;
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
        require_soracloud_runtime_authority(authority, state_transaction)?;
        if self.active_service_version.trim().is_empty() {
            return Err(invalid_parameter(
                "active_service_version must not be empty".to_string(),
            ));
        }
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
        require_soracloud_runtime_authority(authority, state_transaction)?;
        write_soracloud_mailbox_message(state_transaction, self.message)
    }
}

impl Execute for isi::RecordSoracloudRuntimeReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_runtime_authority(authority, state_transaction)?;
        write_soracloud_runtime_receipt(state_transaction, self.receipt)
    }
}

impl Execute for isi::RecordSoracloudPrivateUploadedModelExecutionReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_runtime_authority(authority, state_transaction)?;
        write_soracloud_private_uploaded_model_execution_receipt(state_transaction, self.receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        num::{NonZeroU16, NonZeroU32, NonZeroU64},
        sync::Arc,
        time::Duration,
    };

    use iroha_crypto::{
        Hash, KeyPair,
        fhe_bfv::{
            BfvCiphertext, BfvEvaluationKeyBundle, BfvIdentifierCiphertext,
            BfvIdentifierPublicParameters, BfvParameters, BfvPublicKey, BfvSecretKey,
            apply_galois_automorphism_ciphertext, bfv_add_bounded_noise_output_bound,
            bfv_balanced_multiplication_depth, bfv_encrypted_zero_refresh_residual_multiple_bound,
            bfv_fresh_bounded_noise_ciphertext_bound, bootstrap_ciphertext_rns_exact_round,
            bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed, bootstrap_key_from_seed,
            bootstrap_key_with_max_refresh_rounds_from_seed, decode_packed_plaintext_slots,
            decrypt, decrypt_bounded_noise, decrypt_identifier, encode_packed_plaintext_slots,
            encrypt_bounded_noise_from_seed, encrypt_from_seed, encrypt_identifier_from_seed,
            galois_key_from_seed, keygen_bounded_noise_with_relinearization_from_seed,
            keygen_from_seed, packed_galois_slot_permutation,
            packed_left_rotation_galois_automorphism_power,
            packed_left_rotation_galois_automorphism_powers, registered_bfv_rns_modulus_chain,
            rotation_key_bounded_noise_from_seed, rotation_key_from_seed,
        },
    };
    use iroha_data_model::{
        Encode,
        account::Account,
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        domain::Domain,
        isi::{Grant, Mint},
        metadata::Metadata,
        nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        permission::Permission,
        prelude::Register,
        soracloud::{
            AgentApartmentManifestV1, BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES,
            BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES, BfvBootstrapRefreshTranscriptV1,
            BfvRefreshTranscriptModeV1, BfvRotationRefreshTranscriptV1, DecryptionAuthorityModeV1,
            DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheDeterministicRoundingModeV1,
            FheExecutionPolicyV1, FheGovernanceBundleV1, FheJobInputRefV1, FheJobOperationV1,
            FheJobSpecV1, FheParamLifecycleV1, FheParamSetV1, FheSchemeV1,
            SECRET_ENVELOPE_VERSION_V1, SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
            SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1, SecretEnvelopeEncryptionV1,
            SecretEnvelopeV1, SoraArtifactKindV1, SoraArtifactRefV1, SoraCapabilityPolicyV1,
            SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1, SoraContainerManifestV1,
            SoraContainerRuntimeV1, SoraHfBackendFamilyV1, SoraHfModelFormatV1,
            SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1,
            SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1, SoraHfPlacementStatusV1,
            SoraHfResourceProfileV1, SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1,
            SoraHttpServiceEconomicsV1, SoraInrouGuestOsV1, SoraInrouManifestV1,
            SoraLeaseVolumeBindingV1, SoraLeaseVolumeKindV1, SoraLifecycleHooksV1,
            SoraModelHostCapabilityRecordV1, SoraNetworkAllowlistEntryV1, SoraNetworkPolicyV1,
            SoraPrivateModelArtifactRefV1, SoraPrivateUploadedModelExecutionReceiptV1,
            SoraResourceLimitsV1, SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1,
            SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceManifestV1,
            SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
            SoraStateMutationOperationV1, SoraStateScopeV1, SoraTlsModeV1,
        },
        sorafs::pin_registry::ManifestDigest,
    };
    use iroha_primitives::json::Json;
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID,
    };
    use sha2::{Digest, Sha256};

    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, WorldReadOnly},
    };

    fn seed_test_call_hash(state_transaction: &mut StateTransaction<'_, '_>, byte: u8) {
        state_transaction.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
    }

    fn seed_domain_name_lease_tx(
        state_transaction: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        domain_id: &DomainId,
    ) {
        let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        state_transaction.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    fn state_with_soracloud_permission(kura: &Arc<Kura>) -> Result<State, eyre::Report> {
        state_with_soracloud_permission_on_chain(
            kura,
            iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
        )
    }

    fn state_with_soracloud_permission_on_chain(
        kura: &Arc<Kura>,
        chain_id: iroha_data_model::ChainId,
    ) -> Result<State, eyre::Report> {
        let world = World::with([], [], []);
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura.clone(), query_handle, chain_id);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let wonderland: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal")?;
        seed_domain_name_lease_tx(
            &mut state_transaction,
            &SAMPLE_GENESIS_ACCOUNT_ID,
            &wonderland,
        );
        Register::domain(Domain::new(wonderland.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Grant::account_permission(
            Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(())),
            ALICE_ID.clone(),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        state_transaction.world.public_lane_validators.insert(
            (LaneId::SINGLE, ALICE_ID.clone()),
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: ALICE_ID.clone(),
                peer_id: PeerId::from(ALICE_ID.signatory().clone()),
                stake_account: ALICE_ID.clone(),
                total_stake: Numeric::new(1_000, 0),
                self_stake: Numeric::new(1_000, 0),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            },
        );
        state_transaction.apply();
        state_block.commit()?;
        Ok(state)
    }

    #[test]
    fn soracloud_permission_allows_granted_authority() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let state_transaction = state_block.transaction();

        require_soracloud_permission(&ALICE_ID, &state_transaction)?;
        Ok(())
    }

    #[test]
    fn soracloud_permission_allows_taira_testnet_authority() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission_on_chain(
            &kura,
            iroha_data_model::ChainId::from(TAIRA_TESTNET_CHAIN_ID),
        )?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;

        require_soracloud_permission(&BOB_ID, &state_transaction)?;
        Ok(())
    }

    #[test]
    fn soracloud_permission_rejects_ungranted_authority() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;

        let err = require_soracloud_permission(&BOB_ID, &state_transaction)
            .expect_err("authority without Soracloud permission must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(message)
                if message.as_ref() == "not permitted: CanManageSoracloud"
        ));
        Ok(())
    }

    fn insert_active_public_lane_validator(
        state_transaction: &mut StateTransaction<'_, '_>,
        validator: AccountId,
        total_stake: u64,
    ) {
        state_transaction.world.public_lane_validators.insert(
            (LaneId::SINGLE, validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: validator.clone(),
                peer_id: PeerId::from(validator.signatory().clone()),
                stake_account: validator,
                total_stake: Numeric::new(total_stake, 0),
                self_stake: Numeric::new(total_stake, 0),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            },
        );
    }

    fn configure_staking_assets_for_validator_slash_test(
        state_transaction: &mut StateTransaction<'_, '_>,
        validator: &AccountId,
        escrow_balance: u64,
    ) -> Result<AssetDefinitionId, eyre::Report> {
        Register::account(Account::new(validator.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, state_transaction)?;
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "stake".parse().expect("stake"),
        );
        Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, state_transaction)?;
        Mint::asset_numeric(
            Numeric::new(escrow_balance, 0),
            AssetId::new(asset_definition_id.clone(), ALICE_ID.clone()),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, state_transaction)?;
        state_transaction.nexus.staking.stake_asset_id = asset_definition_id.to_string();
        state_transaction.nexus.staking.stake_escrow_account_id = ALICE_ID.to_string();
        state_transaction.nexus.staking.slash_sink_account_id = ALICE_ID.to_string();
        Ok(asset_definition_id)
    }

    fn sample_hf_shared_lease_pool_record(
        pool_id: Hash,
        source_id: Hash,
        window_started_at_ms: u64,
    ) -> SoraHfSharedLeasePoolV1 {
        SoraHfSharedLeasePoolV1 {
            schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            pool_id,
            source_id,
            storage_class: StorageClass::Warm,
            lease_asset_definition_id: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("domain"),
                "xor".parse().expect("asset"),
            ),
            base_fee_nanos: 10_000,
            lease_term_ms: 60_000,
            window_started_at_ms,
            window_expires_at_ms: window_started_at_ms + 60_000,
            active_member_count: 1,
            status: SoraHfSharedLeaseStatusV1::Active,
            queued_next_window: None,
        }
    }

    fn sample_bundle(
        service_name: &str,
        service_version: &str,
        canary_percent: u8,
    ) -> SoraDeploymentBundleV1 {
        let container = SoraContainerManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            runtime: SoraContainerRuntimeV1::Ivm,
            bundle_hash: Hash::new(format!("bundle:{service_name}:{service_version}").as_bytes()),
            bundle_path: "/bundles/service.ivm".to_string(),
            entrypoint: "main".to_string(),
            args: Vec::new(),
            env: std::collections::BTreeMap::new(),
            inrou: None,
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            capabilities: SoraCapabilityPolicyV1 {
                network: SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                    "api.example.test",
                    [443],
                )]),
                allow_wallet_signing: false,
                allow_state_writes: false,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(500).expect("nonzero"),
                memory_bytes: NonZeroU64::new(64 * 1024 * 1024).expect("nonzero"),
                ephemeral_storage_bytes: NonZeroU64::new(64 * 1024 * 1024).expect("nonzero"),
                max_open_files: NonZeroU32::new(1024).expect("nonzero"),
                max_tasks: NonZeroU16::new(32).expect("nonzero"),
            },
            lifecycle: SoraLifecycleHooksV1 {
                start_grace_secs: NonZeroU32::new(10).expect("nonzero"),
                stop_grace_secs: NonZeroU32::new(10).expect("nonzero"),
                healthcheck_path: Some("/health".to_string()),
            },
        };
        let container_manifest_hash = Hash::new(Encode::encode(&container));

        SoraDeploymentBundleV1 {
            schema_version: iroha_data_model::soracloud::SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service: SoraServiceManifestV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_MANIFEST_VERSION_V1,
                service_name: service_name.parse().expect("valid name"),
                service_version: service_version.to_string(),
                execution_plane:
                    iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
                container: SoraContainerManifestRefV1 {
                    manifest_hash: container_manifest_hash,
                    expected_schema_version:
                        iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
                },
                replicas: NonZeroU16::new(2).expect("nonzero"),
                route: Some(SoraRouteTargetV1 {
                    host: format!("{service_name}.example.test"),
                    path_prefix: "/".to_string(),
                    service_port: NonZeroU16::new(8080).expect("nonzero"),
                    visibility: SoraRouteVisibilityV1::Public,
                    tls_mode: SoraTlsModeV1::Required,
                }),
                rollout: SoraRolloutPolicyV1 {
                    canary_percent,
                    max_unavailable_replicas: 0,
                    health_window_secs: NonZeroU32::new(30).expect("nonzero"),
                    automatic_rollback_failures: NonZeroU32::new(2).expect("nonzero"),
                },
                economics: Default::default(),
                state_bindings: vec![SoraStateBindingV1 {
                    schema_version: iroha_data_model::soracloud::SORA_STATE_BINDING_VERSION_V1,
                    binding_name: "session".parse().expect("valid name"),
                    key_prefix: "/state/session".to_string(),
                    scope: SoraStateScopeV1::ServiceState,
                    encryption: SoraStateEncryptionV1::Plaintext,
                    mutability: SoraStateMutabilityV1::ReadOnly,
                    max_item_bytes: NonZeroU64::new(1024).expect("nonzero"),
                    max_total_bytes: NonZeroU64::new(2048).expect("nonzero"),
                }],
                lease_volumes: Vec::new(),
                handlers: vec![SoraServiceHandlerV1 {
                    handler_name: "query".parse().expect("valid name"),
                    class: SoraServiceHandlerClassV1::Query,
                    entrypoint: "serve_query".to_string(),
                    route_path: Some("/query".to_string()),
                    certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    mailbox: None,
                }],
                artifacts: vec![SoraArtifactRefV1 {
                    kind: SoraArtifactKindV1::StaticAsset,
                    artifact_hash: Hash::new(
                        format!("asset:{service_name}:{service_version}").as_bytes(),
                    ),
                    artifact_path: "/public/index.html".to_string(),
                    handler_name: Some("query".parse().expect("valid name")),
                }],
            },
        }
    }

    fn sample_inrou_manifest() -> SoraInrouManifestV1 {
        SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: SoraInrouGuestOsV1::DebianSlim,
            guest_images: std::collections::BTreeMap::from([
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_string(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_string(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_string(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_string(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: None,
            ssh_authorized_keys: vec!["ssh-ed25519 test-key soracloud-tests".to_string()],
        }
    }

    fn sample_inrou_lease_volumes() -> Vec<SoraLeaseVolumeBindingV1> {
        vec![
            SoraLeaseVolumeBindingV1 {
                volume_name: "root_disk".parse().expect("valid name"),
                kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
                storage_class: StorageClass::Warm,
                mount_path: "/".to_string(),
                max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero"),
            },
            SoraLeaseVolumeBindingV1 {
                volume_name: "service_state".parse().expect("valid name"),
                kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: StorageClass::Warm,
                mount_path: "/var/lib/sora".to_string(),
                max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
            },
        ]
    }

    fn sample_inrou_replica_runtime_state_for(
        service_name: iroha_data_model::name::Name,
        service_version: &str,
        replica_slot: u16,
        validator_account_id: AccountId,
    ) -> SoraInrouReplicaRuntimeStateV1 {
        SoraInrouReplicaRuntimeStateV1 {
            schema_version: SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
            service_name,
            service_version: service_version.to_string(),
            replica_slot,
            validator_account_id,
            peer_id: "12D3KooWInrouRuntimePeer".to_string(),
            selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
            selected_guest_isa: SoraInrouGuestIsaV1::Aarch64,
            health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 250,
            materialized_bundle_hash: Hash::new(b"inrou-runtime-state-test-bundle"),
            accounted_egress_bytes: 0,
            pending_mailbox_message_count: 0,
            last_receipt_id: None,
            updated_at_ms: 1_000,
            last_error: None,
        }
    }

    fn sample_inrou_service_placement_record_for(
        service_name: iroha_data_model::name::Name,
        service_version: &str,
        runtime_state: &SoraInrouReplicaRuntimeStateV1,
    ) -> SoraInrouServicePlacementRecordV1 {
        SoraInrouServicePlacementRecordV1 {
            schema_version: SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
            service_name,
            service_version: service_version.to_string(),
            desired_replica_count: runtime_state.replica_slot,
            eligible_validator_count: 1,
            placements: vec![SoraInrouReplicaPlacementV1 {
                replica_slot: runtime_state.replica_slot,
                validator_account_id: runtime_state.validator_account_id.clone(),
                peer_id: runtime_state.peer_id.clone(),
                selected_backend: runtime_state.selected_backend,
                selected_guest_isa: runtime_state.selected_guest_isa,
                selected_geography_tag: None,
                selection_latency_ms: None,
            }],
            reconciled_at_ms: 1_000,
            last_error: None,
        }
    }

    fn bundle_provenance(bundle: &SoraDeploymentBundleV1) -> ManifestProvenance {
        let payload = iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
            bundle,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("bundle payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn rollback_provenance(
        service_name: &iroha_data_model::name::Name,
        target_version: Option<&str>,
    ) -> ManifestProvenance {
        let payload = encode_rollback_provenance_payload(service_name.as_ref(), target_version)
            .expect("rollback payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn rollout_provenance(
        service_name: &iroha_data_model::name::Name,
        rollout_handle: &str,
        healthy: bool,
        promote_to_percent: Option<u8>,
        governance_tx_hash: Hash,
    ) -> ManifestProvenance {
        let payload = encode_rollout_provenance_payload(
            service_name.as_ref(),
            rollout_handle,
            healthy,
            promote_to_percent,
            governance_tx_hash,
        )
        .expect("rollout payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn service_config_delete_manifest_provenance(
        service_name: &iroha_data_model::name::Name,
        config_name: &str,
    ) -> ManifestProvenance {
        let payload =
            encode_delete_service_config_provenance_payload(service_name.as_ref(), config_name)
                .expect("service config delete payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_bundle_with_state_binding(
        service_name: &str,
        service_version: &str,
        canary_percent: u8,
        binding_name: &str,
        key_prefix: &str,
        encryption: SoraStateEncryptionV1,
        mutability: SoraStateMutabilityV1,
        max_item_bytes: u64,
        max_total_bytes: u64,
    ) -> SoraDeploymentBundleV1 {
        let mut bundle = sample_bundle(service_name, service_version, canary_percent);
        bundle.container.capabilities.allow_state_writes = true;
        bundle.service.state_bindings = vec![SoraStateBindingV1 {
            schema_version: iroha_data_model::soracloud::SORA_STATE_BINDING_VERSION_V1,
            binding_name: binding_name.parse().expect("valid name"),
            key_prefix: key_prefix.to_string(),
            scope: SoraStateScopeV1::ServiceState,
            encryption,
            mutability,
            max_item_bytes: NonZeroU64::new(max_item_bytes).expect("nonzero"),
            max_total_bytes: NonZeroU64::new(max_total_bytes).expect("nonzero"),
        }];
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        bundle
    }

    fn state_mutation_provenance(
        service_name: &iroha_data_model::name::Name,
        binding_name: &iroha_data_model::name::Name,
        state_key: &str,
        operation: SoraStateMutationOperationV1,
        value_size_bytes: Option<u64>,
        payload_commitment: Option<Hash>,
        encryption: SoraStateEncryptionV1,
        governance_tx_hash: Hash,
        fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
    ) -> ManifestProvenance {
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
        .expect("state mutation payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_fhe_param_set() -> FheParamSetV1 {
        let registered_params = ram_lfe_bfv_parameters_v1();
        let parameter_digest = registered_bfv_parameter_digest(&registered_params)
            .expect("registered BFV parameter digest");
        let rns_modulus_chain_digest = registered_bfv_rns_modulus_chain_digest(&registered_params)
            .expect("registered BFV RNS modulus-chain digest");
        FheParamSetV1 {
            schema_version: iroha_data_model::soracloud::FHE_PARAM_SET_VERSION_V1,
            param_set: "bfv-default".parse().expect("valid name"),
            version: NonZeroU32::new(1).expect("nonzero"),
            backend: "fhe/bfv-rns/v1".to_string(),
            scheme: FheSchemeV1::Bfv,
            ciphertext_modulus_bits: vec![
                NonZeroU16::new(53).expect("nonzero"),
                NonZeroU16::new(52).expect("nonzero"),
            ],
            plaintext_modulus_bits: NonZeroU16::new(9).expect("nonzero"),
            polynomial_modulus_degree: NonZeroU32::new(u32::from(
                registered_params.polynomial_degree,
            ))
            .expect("nonzero"),
            slot_count: NonZeroU32::new(u32::from(registered_params.polynomial_degree))
                .expect("nonzero"),
            security_level_bits: NonZeroU16::new(128).expect("nonzero"),
            max_multiplicative_depth: NonZeroU16::new(1).expect("nonzero"),
            lifecycle: FheParamLifecycleV1::Active,
            activation_height: Some(1),
            deprecation_height: None,
            withdraw_height: None,
            parameter_digest,
            rns_modulus_chain_digest,
        }
    }

    fn sample_bfv_evaluation_key_bundle() -> BfvEvaluationKeyBundle {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        let packed_half_rotation = u32::from(params.polynomial_degree) / 2;
        let packed_half_rotation_power =
            packed_left_rotation_galois_automorphism_power(&params, packed_half_rotation)
                .expect("registered packed half-rotation must be one Galois automorphism");
        BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![
                rotation_key_from_seed(&params, &public_key, 1, b"soracloud-fhe-rotation-key")
                    .expect("rotation key"),
            ],
            galois_keys: vec![
                galois_key_from_seed(&params, &secret_key, 3, b"soracloud-fhe-galois-key")
                    .expect("Galois key"),
                galois_key_from_seed(
                    &params,
                    &secret_key,
                    packed_half_rotation_power,
                    b"soracloud-fhe-packed-rotate-galois-key",
                )
                .expect("packed rotation Galois key"),
            ],
            bootstrap_key: Some(
                bootstrap_key_with_max_refresh_rounds_from_seed(
                    &params,
                    &public_key,
                    "bootstrap-test-key",
                    2,
                    b"soracloud-fhe-bootstrap-key",
                )
                .expect("bootstrap key"),
            ),
        }
    }

    fn sample_bfv_evaluation_key_digest() -> Hash {
        let params = ram_lfe_bfv_parameters_v1();
        sample_bfv_evaluation_key_bundle()
            .digest(&params)
            .expect("sample evaluation-key digest")
    }

    fn sample_bfv_refresh_transcript() -> BfvEvaluationKeyRefreshTranscriptV1 {
        let params = ram_lfe_bfv_parameters_v1();
        let (_secret_key, public_key, _relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        BfvEvaluationKeyRefreshTranscriptV1 {
            public_key,
            rotation_transcripts: vec![BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: b"soracloud-fhe-rotation-key".to_vec(),
            }],
            bootstrap_transcript: Some(BfvBootstrapRefreshTranscriptV1 {
                key_id: "bootstrap-test-key".to_string(),
                max_refresh_rounds: 2,
                seed: b"soracloud-fhe-bootstrap-key".to_vec(),
            }),
        }
    }

    fn sample_bfv_refresh_transcript_digest() -> Hash {
        let params = ram_lfe_bfv_parameters_v1();
        sample_bfv_refresh_transcript()
            .digest_for_evaluation_keys(&params, &sample_bfv_evaluation_key_bundle())
            .expect("sample refresh transcript digest")
    }

    fn sample_bounded_noise_bfv_refresh_material() -> (
        BfvParameters,
        BfvEvaluationKeyBundle,
        BfvEvaluationKeyRefreshTranscriptV1,
        Hash,
    ) {
        let params = BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 4_294_967_296,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let (_secret_key, public_key, relinearization_key) =
            keygen_bounded_noise_with_relinearization_from_seed(
                &params,
                b"soracloud-core-bounded-refresh-keygen",
            )
            .expect("bounded-noise keygen");
        let rotation_seed = b"soracloud-core-bounded-refresh-rotation";
        let rotation_key =
            rotation_key_bounded_noise_from_seed(&params, &public_key, 1, rotation_seed)
                .expect("bounded-noise rotation key");
        let bootstrap_seed = b"soracloud-core-bounded-refresh-bootstrap";
        let bootstrap_key = bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed(
            &params,
            &public_key,
            "soracloud-core-bounded-bootstrap",
            2,
            bootstrap_seed,
        )
        .expect("bounded-noise bootstrap key");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![rotation_key],
            galois_keys: Vec::new(),
            bootstrap_key: Some(bootstrap_key),
        };
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key,
            rotation_transcripts: vec![BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: rotation_seed.to_vec(),
            }],
            bootstrap_transcript: Some(BfvBootstrapRefreshTranscriptV1 {
                key_id: "soracloud-core-bounded-bootstrap".to_string(),
                max_refresh_rounds: 2,
                seed: bootstrap_seed.to_vec(),
            }),
        };
        let digest = transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("bounded-noise refresh transcript digest");
        (params, evaluation_keys, transcript, digest)
    }

    fn sample_registered_bounded_noise_bfv_material() -> (
        BfvSecretKey,
        BfvPublicKey,
        BfvEvaluationKeyBundle,
        BfvEvaluationKeyRefreshTranscriptV1,
        Hash,
    ) {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_bounded_noise_with_relinearization_from_seed(
                &params,
                b"soracloud-core-registered-bounded-keygen",
            )
            .expect("registered bounded-noise keygen");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: public_key.clone(),
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };
        let digest = transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("registered bounded-noise refresh transcript digest");
        (secret_key, public_key, evaluation_keys, transcript, digest)
    }

    fn sample_bounded_noise_fhe_payload(
        public_key: &BfvPublicKey,
        slot_values: &[u64],
        seed_prefix: &str,
    ) -> Vec<u8> {
        let params = ram_lfe_bfv_parameters_v1();
        let slots = slot_values
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                encrypt_bounded_noise_from_seed(
                    &params,
                    public_key,
                    &[value],
                    format!("{seed_prefix}-{index}").as_bytes(),
                )
                .expect("encrypt bounded-noise slot")
            })
            .collect::<Vec<_>>();
        norito::to_bytes(&BfvIdentifierCiphertext { slots })
            .expect("encode bounded-noise FHE payload")
    }

    fn sample_fhe_payload(input: &[u8], seed: &[u8]) -> Vec<u8> {
        let params = ram_lfe_bfv_parameters_v1();
        let (_secret_key, public_key, _relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key,
            max_input_bytes: 8,
        };
        let ciphertext =
            encrypt_identifier_from_seed(&public_parameters, input, seed).expect("encrypt");
        norito::to_bytes(&ciphertext).expect("encode ciphertext")
    }

    fn sample_fhe_envelope(input: &[u8], seed: &[u8]) -> BfvIdentifierCiphertext {
        decode_soracloud_fhe_envelope(&sample_fhe_payload(input, seed))
            .expect("sample FHE payload decodes")
    }

    fn sample_oversized_fhe_payload(input: &[u8], seed: &[u8]) -> Vec<u8> {
        let mut envelope = sample_fhe_envelope(input, seed);
        let slot = envelope.slots.first().cloned().expect("sample FHE slot");
        envelope
            .slots
            .resize(RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT + 1, slot);
        norito::to_bytes(&envelope).expect("encode oversized FHE payload")
    }

    const FHE_INPUT_ADMISSION_BACKEND: &str = "stark/fri/sha256-goldilocks";
    const FHE_INPUT_ADMISSION_CIRCUIT_ID: &str = SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1;

    fn sample_fhe_input_admission_proof(
        service_name: &Name,
        binding_name: &Name,
        state_key: &str,
        payload: &[u8],
        governance_tx_hash: Hash,
        residual_multiple_bound: u128,
    ) -> SoracloudFheInputAdmissionProofV1 {
        let statement_hash = expected_fhe_input_admission_statement_hash(
            service_name,
            binding_name,
            state_key,
            SoraStateMutationOperationV1::Upsert,
            u64::try_from(payload.len()).expect("payload len"),
            Hash::new(payload),
            SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            residual_multiple_bound,
            BfvCiphertextBoundModeV1::ExactResidualMultiple,
        )
        .expect("statement hash");
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
            envelope_bytes: vec![0xA5; 32],
        };
        let envelope = OpenVerifyEnvelope::new(
            BackendTag::Stark,
            FHE_INPUT_ADMISSION_CIRCUIT_ID,
            [0x77; 32],
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
            norito::to_bytes(&open).expect("encode STARK wrapper"),
        );
        let proof_box = iroha_data_model::proof::ProofBox::new(
            FHE_INPUT_ADMISSION_BACKEND.into(),
            norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
        );
        SoracloudFheInputAdmissionProofV1 {
            schema_version:
                iroha_data_model::soracloud::SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1,
            residual_multiple_bound,
            bound_mode: BfvCiphertextBoundModeV1::ExactResidualMultiple,
            statement_hash,
            proof: iroha_data_model::proof::ProofAttachment::new_ref(
                FHE_INPUT_ADMISSION_BACKEND.into(),
                proof_box,
                iroha_data_model::proof::VerifyingKeyId::new(
                    FHE_INPUT_ADMISSION_BACKEND,
                    FHE_INPUT_ADMISSION_CIRCUIT_ID,
                ),
            ),
        }
    }

    #[cfg(feature = "zk-stark")]
    fn sample_fhe_input_admission_vk_box() -> iroha_data_model::proof::VerifyingKeyBox {
        sample_fhe_input_admission_vk_box_for_circuit(FHE_INPUT_ADMISSION_CIRCUIT_ID)
    }

    #[cfg(feature = "zk-stark")]
    fn sample_fhe_input_admission_vk_box_for_circuit(
        circuit_id: &str,
    ) -> iroha_data_model::proof::VerifyingKeyBox {
        let vk_payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_string(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: crate::zk_stark::STARK_HASH_SHA256_V1,
        };
        iroha_data_model::proof::VerifyingKeyBox::new(
            FHE_INPUT_ADMISSION_BACKEND.into(),
            norito::to_bytes(&vk_payload).expect("encode FHE input admission STARK VK"),
        )
    }

    #[cfg(feature = "zk-stark")]
    fn sample_verified_fhe_input_admission_proof(
        service_name: &Name,
        binding_name: &Name,
        state_key: &str,
        payload: &[u8],
        governance_tx_hash: Hash,
        residual_multiple_bound: u128,
        vk_box: &iroha_data_model::proof::VerifyingKeyBox,
    ) -> SoracloudFheInputAdmissionProofV1 {
        let statement_hash = expected_fhe_input_admission_statement_hash(
            service_name,
            binding_name,
            state_key,
            SoraStateMutationOperationV1::Upsert,
            u64::try_from(payload.len()).expect("payload len"),
            Hash::new(payload),
            SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            residual_multiple_bound,
            BfvCiphertextBoundModeV1::ExactResidualMultiple,
        )
        .expect("statement hash");
        let proof_box = crate::zk::prove_stark_fri_open_verify_envelope(
            FHE_INPUT_ADMISSION_BACKEND,
            FHE_INPUT_ADMISSION_CIRCUIT_ID,
            vk_box,
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
            vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        )
        .expect("prove FHE input admission STARK envelope");
        SoracloudFheInputAdmissionProofV1 {
            schema_version:
                iroha_data_model::soracloud::SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1,
            residual_multiple_bound,
            bound_mode: BfvCiphertextBoundModeV1::ExactResidualMultiple,
            statement_hash,
            proof: iroha_data_model::proof::ProofAttachment::new_ref(
                FHE_INPUT_ADMISSION_BACKEND.into(),
                proof_box,
                iroha_data_model::proof::VerifyingKeyId::new(
                    FHE_INPUT_ADMISSION_BACKEND,
                    FHE_INPUT_ADMISSION_CIRCUIT_ID,
                ),
            ),
        }
    }

    #[cfg(feature = "zk-stark")]
    fn register_fhe_input_admission_verifier(
        state_transaction: &mut StateTransaction<'_, '_>,
        vk_box: iroha_data_model::proof::VerifyingKeyBox,
    ) -> Result<iroha_data_model::proof::VerifyingKeyId, InstructionExecutionError> {
        register_fhe_input_admission_verifier_for_circuit(
            state_transaction,
            vk_box,
            FHE_INPUT_ADMISSION_CIRCUIT_ID,
        )
    }

    #[cfg(feature = "zk-stark")]
    fn register_fhe_input_admission_verifier_for_circuit(
        state_transaction: &mut StateTransaction<'_, '_>,
        vk_box: iroha_data_model::proof::VerifyingKeyBox,
        record_circuit_id: &str,
    ) -> Result<iroha_data_model::proof::VerifyingKeyId, InstructionExecutionError> {
        Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".to_string(), Json::new(())),
            ALICE_ID.clone(),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, state_transaction)?;
        let vk_id = iroha_data_model::proof::VerifyingKeyId::new(
            FHE_INPUT_ADMISSION_BACKEND,
            FHE_INPUT_ADMISSION_CIRCUIT_ID,
        );
        let commitment = crate::zk::hash_vk(&vk_box);
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new_with_owner(
            1,
            record_circuit_id,
            None,
            "soracloud",
            BackendTag::Stark,
            "goldilocks",
            soracloud_fhe_input_admission_public_inputs_schema_hash_v1(),
            commitment,
        );
        record.vk_len = u32::try_from(vk_box.bytes.len()).expect("VK length fits u32");
        record.status = ConfidentialStatus::Active;
        record.key = Some(vk_box);
        record.gas_schedule_id = Some("stark_fri_soracloud_input_admission_v1".to_string());
        iroha_data_model::isi::InstructionBox::from(
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record,
            },
        )
        .execute(&ALICE_ID, state_transaction)?;
        Ok(vk_id)
    }

    fn assert_invalid_parameter_contains(err: InstructionExecutionError, expected: &str) {
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(ref message)
                ) if message.contains(expected)
            ),
            "unexpected error: {err:?}"
        );
    }

    fn assert_invariant_contains(err: InstructionExecutionError, expected: &str) {
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvariantViolation(ref message) if message.contains(expected)
            ),
            "unexpected error: {err:?}"
        );
    }

    fn sample_fhe_input_ref(state_key: &str, payload: &[u8]) -> FheJobInputRefV1 {
        FheJobInputRefV1 {
            state_key: state_key.to_string(),
            payload_bytes: NonZeroU64::new(u64::try_from(payload.len()).expect("payload len"))
                .expect("nonzero"),
            commitment: Hash::new(payload),
        }
    }

    fn sample_fhe_policy() -> FheExecutionPolicyV1 {
        FheExecutionPolicyV1 {
            schema_version: iroha_data_model::soracloud::FHE_EXECUTION_POLICY_VERSION_V1,
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            evaluation_key_digest: sample_bfv_evaluation_key_digest(),
            evaluation_key_refresh_transcript_digest: sample_bfv_refresh_transcript_digest(),
            refresh_transcript_mode: BfvRefreshTranscriptModeV1::ExactLift,
            max_ciphertext_bytes: NonZeroU64::new(131_072).expect("nonzero"),
            max_plaintext_bytes: NonZeroU64::new(512).expect("nonzero"),
            max_input_ciphertexts: NonZeroU16::new(4).expect("nonzero"),
            max_output_ciphertexts: NonZeroU16::new(1).expect("nonzero"),
            max_multiplication_depth: NonZeroU16::new(1).expect("nonzero"),
            max_rotation_count: NonZeroU32::new(16).expect("nonzero"),
            max_bootstrap_count: 1,
            rounding_mode: FheDeterministicRoundingModeV1::NearestTiesToEven,
        }
    }

    fn sample_fhe_job(inputs: Vec<FheJobInputRefV1>) -> FheJobSpecV1 {
        FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "job-1".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::Add,
            inputs,
            output_state_key: "/state/private/output-1".to_string(),
            requested_multiplication_depth: 0,
            rotation_steps: 0,
            bootstrap_count: 0,
        }
    }

    const SORACLOUD_BFV_OPERATION_VECTOR_SET: &str = "soracloud-bfv-operation-v1";

    fn shared_bfv_fixture() -> norito::json::Value {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/bfv_identifier_vectors_v1.json");
        let fixture = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        norito::json::from_str(&fixture)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", fixture_path.display()))
    }

    fn shared_fhe_governance_bundle_fixture() -> FheGovernanceBundleV1 {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/fhe_governance_bundle_v1.json");
        let fixture = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        norito::json::from_str(&fixture)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", fixture_path.display()))
    }

    fn fixture_get<'a>(value: &'a norito::json::Value, field: &str) -> &'a norito::json::Value {
        value
            .get(field)
            .unwrap_or_else(|| panic!("fixture field `{field}` is missing"))
    }

    fn fixture_str<'a>(value: &'a norito::json::Value, field: &str) -> &'a str {
        fixture_get(value, field)
            .as_str()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be a string"))
    }

    fn fixture_u64_value(value: &norito::json::Value, field: &str) -> u64 {
        if let Some(value) = value.as_u64() {
            return value;
        }
        value
            .as_str()
            .unwrap_or_else(|| {
                panic!("fixture field `{field}` must be an unsigned integer or decimal string")
            })
            .parse::<u64>()
            .unwrap_or_else(|err| panic!("fixture field `{field}` must fit u64: {err}"))
    }

    fn fixture_u64(value: &norito::json::Value, field: &str) -> u64 {
        fixture_u64_value(fixture_get(value, field), field)
    }

    fn fixture_u64_array(value: &norito::json::Value, field: &str) -> Vec<u64> {
        fixture_array(value, field)
            .iter()
            .enumerate()
            .map(|(index, value)| fixture_u64_value(value, &format!("{field}[{index}]")))
            .collect()
    }

    fn fixture_str_array<'a>(value: &'a norito::json::Value, field: &str) -> Vec<&'a str> {
        fixture_array(value, field)
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("fixture field `{field}[{index}]` must be a string"))
            })
            .collect()
    }

    fn fixture_array<'a>(value: &'a norito::json::Value, field: &str) -> &'a [norito::json::Value] {
        fixture_get(value, field)
            .as_array()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an array"))
    }

    fn fixture_operation_vectors(root: &norito::json::Value) -> &norito::json::Value {
        let operation_vectors = fixture_get(root, "operation_vectors");
        assert_eq!(
            fixture_str(operation_vectors, "vector_set"),
            SORACLOUD_BFV_OPERATION_VECTOR_SET
        );
        operation_vectors
    }

    fn assert_bfv_public_parameters_fixture(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
        public_parameters: &BfvIdentifierPublicParameters,
    ) {
        let public_key_fixture = fixture_get(operation_vectors, "public_key");
        let encoded_public_key =
            norito::to_bytes(&public_parameters.public_key).expect("encode public key");
        assert_eq!(
            fixture_u64(public_key_fixture, "expected_norito_bytes"),
            u64::try_from(encoded_public_key.len()).expect("public-key length fits u64"),
            "public-key Norito byte length"
        );
        assert_eq!(
            fixture_str(public_key_fixture, "expected_sha256"),
            sha256_hex(&encoded_public_key),
            "public-key SHA-256"
        );

        let public_parameters_fixture = fixture_get(operation_vectors, "public_parameters");
        let encoded_public_parameters =
            norito::to_bytes(public_parameters).expect("encode public parameters");
        assert_eq!(
            fixture_u64(public_parameters_fixture, "expected_norito_bytes"),
            u64::try_from(encoded_public_parameters.len())
                .expect("public-parameter length fits u64"),
            "public-parameter Norito byte length"
        );
        assert_eq!(
            fixture_str(public_parameters_fixture, "expected_sha256"),
            sha256_hex(&encoded_public_parameters),
            "public-parameter SHA-256"
        );
        assert_eq!(
            fixture_u64(public_parameters_fixture, "polynomial_degree"),
            u64::from(params.polynomial_degree),
            "public-parameter polynomial degree"
        );
        assert_eq!(
            fixture_u64(public_parameters_fixture, "plaintext_modulus"),
            params.plaintext_modulus,
            "public-parameter plaintext modulus"
        );
        assert_eq!(
            fixture_u64(public_parameters_fixture, "ciphertext_modulus"),
            params.ciphertext_modulus,
            "public-parameter ciphertext modulus"
        );
        assert_eq!(
            fixture_u64(public_parameters_fixture, "decomposition_base_log"),
            u64::from(params.decomposition_base_log),
            "public-parameter decomposition base log"
        );
        assert_eq!(
            fixture_u64(public_parameters_fixture, "max_input_bytes"),
            u64::from(public_parameters.max_input_bytes),
            "public-parameter max input bytes"
        );

        let decoded_fixture = fixture_get(operation_vectors, "public_parameters_decoded");
        let decoded_parameters = fixture_get(decoded_fixture, "parameters");
        assert_eq!(
            fixture_u64(decoded_parameters, "polynomial_degree"),
            u64::from(params.polynomial_degree),
            "decoded public-parameter polynomial degree"
        );
        assert_eq!(
            fixture_u64(decoded_parameters, "plaintext_modulus"),
            params.plaintext_modulus,
            "decoded public-parameter plaintext modulus"
        );
        assert_eq!(
            fixture_u64(decoded_parameters, "ciphertext_modulus"),
            params.ciphertext_modulus,
            "decoded public-parameter ciphertext modulus"
        );
        assert_eq!(
            fixture_u64(decoded_parameters, "decomposition_base_log"),
            u64::from(params.decomposition_base_log),
            "decoded public-parameter decomposition base log"
        );
        let decoded_public_key = fixture_get(decoded_fixture, "public_key");
        assert_eq!(
            fixture_u64_array(decoded_public_key, "b"),
            public_parameters.public_key.b,
            "decoded public-key b polynomial"
        );
        assert_eq!(
            fixture_u64_array(decoded_public_key, "a"),
            public_parameters.public_key.a,
            "decoded public-key a polynomial"
        );
        assert_eq!(
            fixture_u64(decoded_fixture, "max_input_bytes"),
            u64::from(public_parameters.max_input_bytes),
            "decoded public-parameter max input bytes"
        );
        assert_eq!(
            fixture_str(decoded_fixture, "norito_length_encoding"),
            "compact-v1",
            "decoded public-parameter Norito length encoding"
        );
    }

    fn assert_bfv_rns_modulus_chain_fixture(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
    ) {
        let chain_fixture = fixture_get(operation_vectors, "rns_modulus_chain");
        let chain = registered_bfv_rns_modulus_chain(params).expect("registered BFV RNS chain");
        assert_eq!(
            fixture_u64_array(chain_fixture, "moduli"),
            chain.moduli,
            "RNS modulus-chain limbs"
        );
        assert_eq!(
            fixture_str(chain_fixture, "product"),
            chain.product().expect("RNS product").to_string(),
            "RNS modulus-chain product"
        );
        assert_eq!(
            fixture_str(chain_fixture, "expected_digest_hex"),
            registered_bfv_rns_modulus_chain_digest(params)
                .expect("registered RNS digest")
                .to_string(),
            "RNS modulus-chain digest"
        );

        let polynomial_fixture = fixture_get(chain_fixture, "sample_polynomials");
        let lhs_coefficients = fixture_u64_array(polynomial_fixture, "lhs_coefficients");
        let rhs_coefficients = fixture_u64_array(polynomial_fixture, "rhs_coefficients");
        assert_eq!(
            lhs_coefficients,
            rns_sample_lhs_coefficients(params),
            "RNS fixture lhs sample coefficients"
        );
        assert_eq!(
            rhs_coefficients,
            rns_sample_rhs_coefficients(params),
            "RNS fixture rhs sample coefficients"
        );
        let lhs = chain
            .decompose_polynomial(params, &lhs_coefficients)
            .expect("decompose fixture lhs polynomial");
        let rhs = chain
            .decompose_polynomial(params, &rhs_coefficients)
            .expect("decompose fixture rhs polynomial");
        let lhs_reconstructed = u64_coefficients_to_u128(&lhs_coefficients);
        let rhs_reconstructed = u64_coefficients_to_u128(&rhs_coefficients);
        assert_rns_polynomial_fixture(
            fixture_get(polynomial_fixture, "lhs"),
            "lhs",
            params,
            &chain,
            &lhs,
            &lhs_reconstructed,
        );
        assert_rns_polynomial_fixture(
            fixture_get(polynomial_fixture, "rhs"),
            "rhs",
            params,
            &chain,
            &rhs,
            &rhs_reconstructed,
        );

        let added = chain
            .add_rns_polynomials(params, &lhs, &rhs)
            .expect("add fixture RNS polynomials");
        let reconstructed_add = chain
            .reconstruct_polynomial(params, &added)
            .expect("reconstruct fixture RNS sum");
        assert_rns_polynomial_fixture(
            fixture_get(polynomial_fixture, "sum"),
            "sum",
            params,
            &chain,
            &added,
            &reconstructed_add,
        );

        let multiplied = chain
            .multiply_rns_polynomials_negacyclic(params, &lhs, &rhs)
            .expect("multiply fixture RNS polynomials");
        let reconstructed_product = chain
            .reconstruct_polynomial(params, &multiplied)
            .expect("reconstruct fixture RNS product");
        assert_rns_polynomial_fixture(
            fixture_get(polynomial_fixture, "negacyclic_product"),
            "negacyclic product",
            params,
            &chain,
            &multiplied,
            &reconstructed_product,
        );
    }

    fn assert_rns_polynomial_fixture(
        fixture: &norito::json::Value,
        label: &str,
        params: &BfvParameters,
        chain: &iroha_crypto::fhe_bfv::BfvRnsModulusChain,
        polynomial: &iroha_crypto::fhe_bfv::BfvRnsPolynomial,
        reconstructed: &[u128],
    ) {
        assert_eq!(
            fixture_u64(fixture, "coefficient_count"),
            u64::from(params.polynomial_degree),
            "{label} coefficient count"
        );
        assert_eq!(
            fixture_str(fixture, "reconstructed_sha256"),
            coefficient_u128_vector_sha256_hex(reconstructed),
            "{label} reconstructed coefficient SHA-256"
        );
        let limb_hashes = fixture_str_array(fixture, "residue_limb_sha256");
        assert_eq!(
            limb_hashes.len(),
            chain.moduli.len(),
            "{label} residue limb count"
        );
        for (index, (expected_hash, residues)) in limb_hashes
            .iter()
            .zip(&polynomial.residues_by_limb)
            .enumerate()
        {
            assert_eq!(
                *expected_hash,
                coefficient_vector_sha256_hex(residues),
                "{label} residue limb {index} SHA-256"
            );
        }
    }

    fn assert_bfv_evaluation_key_fixture(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
        evaluation_keys: &BfvEvaluationKeyBundle,
    ) {
        let key_fixture = fixture_get(operation_vectors, "evaluation_key_bundle");
        let encoded_keys = norito::to_bytes(evaluation_keys).expect("encode evaluation keys");
        assert_eq!(
            fixture_u64(key_fixture, "expected_norito_bytes"),
            u64::try_from(encoded_keys.len()).expect("evaluation-key length fits u64"),
            "evaluation-key Norito byte length"
        );
        assert_eq!(
            fixture_str(key_fixture, "expected_sha256"),
            sha256_hex(&encoded_keys),
            "evaluation-key Norito SHA-256"
        );
        assert_eq!(
            fixture_str(key_fixture, "expected_digest_hex"),
            evaluation_keys
                .digest(params)
                .expect("evaluation-key digest")
                .to_string(),
            "evaluation-key domain-separated digest"
        );
        assert_eq!(
            fixture_u64(key_fixture, "decomposition_base_log"),
            u64::from(params.decomposition_base_log),
            "evaluation-key decomposition base log"
        );
        assert_eq!(
            fixture_u64(key_fixture, "decomposition_digit_count"),
            u64::try_from(evaluation_keys.relinearization_key.entries.len())
                .expect("entry count fits u64"),
            "evaluation-key decomposition digit count"
        );
        assert_eq!(
            fixture_u64(key_fixture, "relinearization_entry_count"),
            u64::try_from(evaluation_keys.relinearization_key.entries.len())
                .expect("entry count fits u64"),
            "relinearization entry count"
        );
        assert_eq!(
            fixture_u64(key_fixture, "rotation_key_count"),
            u64::try_from(evaluation_keys.rotation_keys.len()).expect("rotation count fits u64"),
            "rotation key count"
        );
        assert_eq!(
            fixture_u64(key_fixture, "galois_key_count"),
            u64::try_from(evaluation_keys.galois_keys.len()).expect("Galois count fits u64"),
            "Galois key count"
        );
        assert_eq!(
            fixture_str(key_fixture, "bootstrap_key_id"),
            evaluation_keys
                .bootstrap_key
                .as_ref()
                .expect("fixture bootstrap key")
                .key_id,
            "bootstrap key id"
        );

        let relinearization_fixtures = fixture_array(key_fixture, "relinearization_entries");
        assert_eq!(
            relinearization_fixtures.len(),
            evaluation_keys.relinearization_key.entries.len(),
            "relinearization entry fixture count"
        );
        for (index, (fixture, entry)) in relinearization_fixtures
            .iter()
            .zip(&evaluation_keys.relinearization_key.entries)
            .enumerate()
        {
            assert_eq!(
                fixture_u64(fixture, "index"),
                u64::try_from(index).expect("entry index fits u64"),
                "relinearization entry index"
            );
            assert_eq!(
                fixture_u64(fixture, "coefficient_count"),
                u64::from(params.polynomial_degree),
                "relinearization entry coefficient count"
            );
            assert_eq!(
                fixture_str(fixture, "b_sha256"),
                coefficient_vector_sha256_hex(&entry.b),
                "relinearization entry b SHA-256"
            );
            assert_eq!(
                fixture_str(fixture, "a_sha256"),
                coefficient_vector_sha256_hex(&entry.a),
                "relinearization entry a SHA-256"
            );
        }

        let galois_fixtures = fixture_array(operation_vectors, "galois_keys");
        assert_eq!(
            galois_fixtures.len(),
            evaluation_keys.galois_keys.len(),
            "Galois key fixture count"
        );
        for (fixture, key) in galois_fixtures.iter().zip(&evaluation_keys.galois_keys) {
            assert_eq!(
                fixture_u64(fixture, "automorphism_power"),
                u64::from(key.automorphism_power),
                "Galois automorphism power"
            );
            assert_eq!(
                fixture_u64(fixture, "entry_count"),
                u64::try_from(key.entries.len()).expect("Galois entry count fits u64"),
                "Galois entry count"
            );
            let entry_fixtures = fixture_array(fixture, "entries");
            assert_eq!(
                entry_fixtures.len(),
                key.entries.len(),
                "Galois entry fixture count"
            );
            for (index, (entry_fixture, entry)) in
                entry_fixtures.iter().zip(&key.entries).enumerate()
            {
                assert_eq!(
                    fixture_u64(entry_fixture, "index"),
                    u64::try_from(index).expect("entry index fits u64"),
                    "Galois entry index"
                );
                assert_eq!(
                    fixture_u64(entry_fixture, "coefficient_count"),
                    u64::from(params.polynomial_degree),
                    "Galois entry coefficient count"
                );
                assert_eq!(
                    fixture_str(entry_fixture, "b_sha256"),
                    coefficient_vector_sha256_hex(&entry.b),
                    "Galois entry b SHA-256"
                );
                assert_eq!(
                    fixture_str(entry_fixture, "a_sha256"),
                    coefficient_vector_sha256_hex(&entry.a),
                    "Galois entry a SHA-256"
                );
            }
        }

        let rotation_fixtures = fixture_array(operation_vectors, "rotation_keys");
        assert_eq!(
            rotation_fixtures.len(),
            evaluation_keys.rotation_keys.len(),
            "rotation key fixture count"
        );
        for (fixture, key) in rotation_fixtures.iter().zip(&evaluation_keys.rotation_keys) {
            assert_eq!(
                fixture_u64(fixture, "rotation_steps"),
                u64::from(key.rotation_steps),
                "rotation steps"
            );
            let encoded_refresh =
                norito::to_bytes(&key.zero_refresh).expect("encode rotation refresh");
            assert_eq!(
                fixture_u64(fixture, "expected_zero_refresh_bytes"),
                u64::try_from(encoded_refresh.len()).expect("refresh length fits u64"),
                "rotation zero-refresh byte length"
            );
            assert_eq!(
                fixture_str(fixture, "expected_zero_refresh_sha256"),
                sha256_hex(&encoded_refresh),
                "rotation zero-refresh SHA-256"
            );
            assert_ciphertext_component_fixture(
                fixture_get(fixture, "zero_refresh_components"),
                "rotation zero-refresh",
                params,
                &key.zero_refresh,
            );
        }

        let bootstrap_fixture = fixture_get(operation_vectors, "bootstrap_key");
        let bootstrap_key = evaluation_keys
            .bootstrap_key
            .as_ref()
            .expect("fixture bootstrap key");
        assert_eq!(
            fixture_str(bootstrap_fixture, "key_id"),
            bootstrap_key.key_id,
            "bootstrap key id"
        );
        assert_eq!(
            fixture_u64(bootstrap_fixture, "max_refresh_rounds"),
            u64::from(bootstrap_key.max_refresh_rounds),
            "bootstrap key max refresh rounds"
        );
        let encoded_refresh =
            norito::to_bytes(&bootstrap_key.zero_refresh).expect("encode bootstrap refresh");
        assert_eq!(
            fixture_u64(bootstrap_fixture, "expected_zero_refresh_bytes"),
            u64::try_from(encoded_refresh.len()).expect("refresh length fits u64"),
            "bootstrap zero-refresh byte length"
        );
        assert_eq!(
            fixture_str(bootstrap_fixture, "expected_zero_refresh_sha256"),
            sha256_hex(&encoded_refresh),
            "bootstrap zero-refresh SHA-256"
        );
        assert_ciphertext_component_fixture(
            fixture_get(bootstrap_fixture, "zero_refresh_components"),
            "bootstrap zero-refresh",
            params,
            &bootstrap_key.zero_refresh,
        );
        let round_refresh_fixtures = fixture_array(bootstrap_fixture, "round_refreshes");
        assert_eq!(
            round_refresh_fixtures.len(),
            usize::from(bootstrap_key.max_refresh_rounds),
            "bootstrap round-refresh fixture count"
        );
        assert_eq!(
            round_refresh_fixtures.len(),
            bootstrap_key.round_refreshes.len(),
            "bootstrap round-refresh key count"
        );
        for (round_index, (fixture, refresh)) in round_refresh_fixtures
            .iter()
            .zip(&bootstrap_key.round_refreshes)
            .enumerate()
        {
            assert_eq!(
                fixture_u64(fixture, "round_index"),
                u64::try_from(round_index).expect("round index fits u64"),
                "bootstrap round-refresh index"
            );
            let encoded_refresh =
                norito::to_bytes(refresh).expect("encode bootstrap round refresh");
            assert_eq!(
                fixture_u64(fixture, "expected_refresh_bytes"),
                u64::try_from(encoded_refresh.len()).expect("refresh length fits u64"),
                "bootstrap round-refresh byte length"
            );
            assert_eq!(
                fixture_str(fixture, "expected_refresh_sha256"),
                sha256_hex(&encoded_refresh),
                "bootstrap round-refresh SHA-256"
            );
            assert_ciphertext_component_fixture(
                fixture_get(fixture, "components"),
                "bootstrap round-refresh",
                params,
                refresh,
            );
        }
    }

    fn assert_bfv_galois_switch_vectors(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
        public_parameters: &BfvIdentifierPublicParameters,
        secret_key: &iroha_crypto::fhe_bfv::BfvSecretKey,
        evaluation_keys: &BfvEvaluationKeyBundle,
    ) {
        let vectors = fixture_array(operation_vectors, "galois_switch_vectors");
        assert!(
            !vectors.is_empty(),
            "Galois switch vector fixture must not be empty"
        );
        for vector in vectors {
            let power = u32::try_from(fixture_u64(vector, "automorphism_power"))
                .expect("fixture Galois automorphism power must fit u32");
            let key = evaluation_keys
                .galois_keys
                .iter()
                .find(|key| key.automorphism_power == power)
                .unwrap_or_else(|| panic!("fixture Galois key for power {power} is missing"));
            let input_plaintext = fixture_u64_array(vector, "input_plaintext_slots");
            let input = encrypt_from_seed(
                params,
                &public_parameters.public_key,
                &input_plaintext,
                fixture_str(vector, "seed_utf8").as_bytes(),
            )
            .expect("fixture Galois input must encrypt");
            let encoded_input = norito::to_bytes(&input).expect("encode Galois input");
            assert_eq!(
                fixture_u64(vector, "expected_input_ciphertext_bytes"),
                u64::try_from(encoded_input.len()).expect("Galois input length fits u64"),
                "Galois input byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_input_ciphertext_sha256"),
                sha256_hex(&encoded_input),
                "Galois input SHA-256"
            );

            let transformed = apply_galois_automorphism_ciphertext(params, key, &input)
                .expect("fixture Galois switch must apply");
            let encoded_output = norito::to_bytes(&transformed).expect("encode Galois output");
            assert_eq!(
                fixture_u64(vector, "expected_output_ciphertext_bytes"),
                u64::try_from(encoded_output.len()).expect("Galois output length fits u64"),
                "Galois output byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_output_ciphertext_sha256"),
                sha256_hex(&encoded_output),
                "Galois output SHA-256"
            );
            assert_ciphertext_component_fixture(
                fixture_get(vector, "output_components"),
                "Galois output",
                params,
                &transformed,
            );

            let plaintext =
                decrypt(params, secret_key, &transformed).expect("decrypt Galois output");
            assert_eq!(
                fixture_str(vector, "expected_plaintext_sha256"),
                coefficient_vector_sha256_hex(&plaintext),
                "Galois output plaintext SHA-256"
            );
        }
    }

    fn assert_bfv_packed_galois_switch_vectors(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
        public_parameters: &BfvIdentifierPublicParameters,
        secret_key: &iroha_crypto::fhe_bfv::BfvSecretKey,
        evaluation_keys: &BfvEvaluationKeyBundle,
    ) {
        let vectors = fixture_array(operation_vectors, "packed_galois_switch_vectors");
        assert!(
            !vectors.is_empty(),
            "packed Galois switch vector fixture must not be empty"
        );
        for vector in vectors {
            let power = u32::try_from(fixture_u64(vector, "automorphism_power"))
                .expect("fixture Galois automorphism power must fit u32");
            let key = evaluation_keys
                .galois_keys
                .iter()
                .find(|key| key.automorphism_power == power)
                .unwrap_or_else(|| panic!("fixture Galois key for power {power} is missing"));
            let input_slots = fixture_u64_array(vector, "input_packed_slots");
            let packed_plaintext =
                encode_packed_plaintext_slots(params, &input_slots).expect("pack fixture slots");
            assert_eq!(
                fixture_str(vector, "expected_packed_plaintext_sha256"),
                coefficient_vector_sha256_hex(&packed_plaintext),
                "packed Galois plaintext coefficient SHA-256"
            );

            let expected_permutation = fixture_u64_array(vector, "expected_slot_permutation")
                .into_iter()
                .map(|slot| usize::try_from(slot).expect("fixture slot index fits usize"))
                .collect::<Vec<_>>();
            assert_eq!(
                expected_permutation,
                packed_galois_slot_permutation(params, power).expect("packed slot permutation"),
                "packed Galois slot permutation"
            );
            let expected_slots = expected_permutation
                .iter()
                .map(|&input_index| input_slots[input_index])
                .collect::<Vec<_>>();
            assert_eq!(
                fixture_u64_array(vector, "expected_packed_slots"),
                expected_slots,
                "packed Galois expected slots"
            );

            let input = encrypt_from_seed(
                params,
                &public_parameters.public_key,
                &packed_plaintext,
                fixture_str(vector, "seed_utf8").as_bytes(),
            )
            .expect("fixture packed Galois input must encrypt");
            let encoded_input = norito::to_bytes(&input).expect("encode packed Galois input");
            assert_eq!(
                fixture_u64(vector, "expected_input_ciphertext_bytes"),
                u64::try_from(encoded_input.len()).expect("packed Galois input length fits u64"),
                "packed Galois input byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_input_ciphertext_sha256"),
                sha256_hex(&encoded_input),
                "packed Galois input SHA-256"
            );

            let transformed = apply_galois_automorphism_ciphertext(params, key, &input)
                .expect("fixture packed Galois switch must apply");
            let encoded_output =
                norito::to_bytes(&transformed).expect("encode packed Galois output");
            assert_eq!(
                fixture_u64(vector, "expected_output_ciphertext_bytes"),
                u64::try_from(encoded_output.len()).expect("packed Galois output length fits u64"),
                "packed Galois output byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_output_ciphertext_sha256"),
                sha256_hex(&encoded_output),
                "packed Galois output SHA-256"
            );
            assert_ciphertext_component_fixture(
                fixture_get(vector, "output_components"),
                "packed Galois output",
                params,
                &transformed,
            );

            let plaintext =
                decrypt(params, secret_key, &transformed).expect("decrypt packed Galois output");
            assert_eq!(
                fixture_str(vector, "expected_plaintext_coefficients_sha256"),
                coefficient_vector_sha256_hex(&plaintext),
                "packed Galois output plaintext coefficient SHA-256"
            );
            assert_eq!(
                decode_packed_plaintext_slots(params, &plaintext).expect("decode packed output"),
                expected_slots,
                "packed Galois output slots"
            );
        }
    }

    fn assert_bfv_bootstrap_refresh_vectors(
        operation_vectors: &norito::json::Value,
        params: &BfvParameters,
        public_parameters: &BfvIdentifierPublicParameters,
        secret_key: &iroha_crypto::fhe_bfv::BfvSecretKey,
        evaluation_keys: &BfvEvaluationKeyBundle,
    ) {
        let vectors = fixture_array(operation_vectors, "bootstrap_refresh_vectors");
        assert!(
            !vectors.is_empty(),
            "bootstrap refresh vector fixture must not be empty"
        );
        let bootstrap_key = evaluation_keys
            .bootstrap_key
            .as_ref()
            .expect("fixture bootstrap key");
        let rns_chain = registered_bfv_rns_modulus_chain(params).expect("registered RNS chain");
        for vector in vectors {
            assert_eq!(
                fixture_str(vector, "key_id"),
                bootstrap_key.key_id,
                "bootstrap refresh vector key id"
            );
            let refresh_rounds = u16::try_from(fixture_u64(vector, "refresh_rounds"))
                .expect("fixture bootstrap refresh_rounds must fit u16");
            assert!(
                refresh_rounds > 0,
                "bootstrap refresh vector rounds must be non-zero"
            );
            assert!(
                refresh_rounds <= bootstrap_key.max_refresh_rounds,
                "bootstrap refresh vector rounds exceed key capacity"
            );
            let input_plaintext = fixture_u64_array(vector, "input_plaintext_slots");
            let input = encrypt_from_seed(
                params,
                &public_parameters.public_key,
                &input_plaintext,
                fixture_str(vector, "seed_utf8").as_bytes(),
            )
            .expect("fixture bootstrap input must encrypt");
            let encoded_input = norito::to_bytes(&input).expect("encode bootstrap input");
            assert_eq!(
                fixture_u64(vector, "expected_input_ciphertext_bytes"),
                u64::try_from(encoded_input.len()).expect("bootstrap input length fits u64"),
                "bootstrap input byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_input_ciphertext_sha256"),
                sha256_hex(&encoded_input),
                "bootstrap input SHA-256"
            );

            let mut refreshed = input;
            for round_index in 0..refresh_rounds {
                refreshed = bootstrap_ciphertext_rns_exact_round(
                    params,
                    &rns_chain,
                    bootstrap_key,
                    &refreshed,
                    round_index,
                )
                .expect("fixture bootstrap refresh must apply");
            }
            let encoded_output = norito::to_bytes(&refreshed).expect("encode bootstrap output");
            assert_eq!(
                fixture_u64(vector, "expected_output_ciphertext_bytes"),
                u64::try_from(encoded_output.len()).expect("bootstrap output length fits u64"),
                "bootstrap output byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_output_ciphertext_sha256"),
                sha256_hex(&encoded_output),
                "bootstrap output SHA-256"
            );
            assert_ciphertext_component_fixture(
                fixture_get(vector, "output_components"),
                "bootstrap output",
                params,
                &refreshed,
            );

            let plaintext =
                decrypt(params, secret_key, &refreshed).expect("decrypt bootstrap output");
            assert_eq!(
                fixture_str(vector, "expected_plaintext_sha256"),
                coefficient_vector_sha256_hex(&plaintext),
                "bootstrap output plaintext SHA-256"
            );
        }
    }

    fn bfv_operation_material(
        operation_vectors: &norito::json::Value,
    ) -> (
        BfvParameters,
        BfvIdentifierPublicParameters,
        iroha_crypto::fhe_bfv::BfvSecretKey,
        BfvEvaluationKeyBundle,
    ) {
        let params = ram_lfe_bfv_parameters_v1();
        let max_input_bytes = u16::try_from(fixture_u64(operation_vectors, "max_input_bytes"))
            .expect("fixture max_input_bytes must fit u16");
        let (secret_key, public_key, relinearization_key) = keygen_from_seed(
            &params,
            fixture_str(operation_vectors, "keygen_seed_utf8").as_bytes(),
        )
        .expect("fixture keygen seed must produce BFV keys");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes,
        };
        public_parameters
            .validate()
            .expect("fixture public parameters must validate");
        assert_bfv_public_parameters_fixture(operation_vectors, &params, &public_parameters);
        assert_bfv_rns_modulus_chain_fixture(operation_vectors, &params);

        let rotation_keys = fixture_array(operation_vectors, "rotation_keys")
            .iter()
            .map(|key| {
                let steps = u32::try_from(fixture_u64(key, "rotation_steps"))
                    .expect("fixture rotation steps must fit u32");
                rotation_key_from_seed(
                    &params,
                    &public_key,
                    steps,
                    fixture_str(key, "seed_utf8").as_bytes(),
                )
                .expect("fixture rotation key must derive")
            })
            .collect();
        let galois_keys = fixture_array(operation_vectors, "galois_keys")
            .iter()
            .map(|key| {
                let power = u32::try_from(fixture_u64(key, "automorphism_power"))
                    .expect("fixture Galois automorphism power must fit u32");
                galois_key_from_seed(
                    &params,
                    &secret_key,
                    power,
                    fixture_str(key, "seed_utf8").as_bytes(),
                )
                .expect("fixture Galois key must derive")
            })
            .collect();
        let bootstrap = fixture_get(operation_vectors, "bootstrap_key");
        let bootstrap_key = Some(
            bootstrap_key_with_max_refresh_rounds_from_seed(
                &params,
                &public_key,
                fixture_str(bootstrap, "key_id"),
                u16::try_from(fixture_u64(bootstrap, "max_refresh_rounds"))
                    .expect("fixture bootstrap max_refresh_rounds must fit u16"),
                fixture_str(bootstrap, "seed_utf8").as_bytes(),
            )
            .expect("fixture bootstrap key must derive"),
        );
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys,
            galois_keys,
            bootstrap_key,
        };
        evaluation_keys
            .validate(&params)
            .expect("fixture evaluation keys must validate");
        assert_bfv_evaluation_key_fixture(operation_vectors, &params, &evaluation_keys);
        assert_bfv_galois_switch_vectors(
            operation_vectors,
            &params,
            &public_parameters,
            &secret_key,
            &evaluation_keys,
        );
        assert_bfv_packed_galois_switch_vectors(
            operation_vectors,
            &params,
            &public_parameters,
            &secret_key,
            &evaluation_keys,
        );
        assert_bfv_bootstrap_refresh_vectors(
            operation_vectors,
            &params,
            &public_parameters,
            &secret_key,
            &evaluation_keys,
        );

        (params, public_parameters, secret_key, evaluation_keys)
    }

    fn operation_vector_inputs(
        public_parameters: &BfvIdentifierPublicParameters,
        vector: &norito::json::Value,
    ) -> Vec<BfvIdentifierCiphertext> {
        fixture_array(vector, "inputs")
            .iter()
            .map(|input| {
                if input.get("packed_slots").is_some() {
                    let packed_slots = fixture_u64_array(input, "packed_slots");
                    let packed_plaintext =
                        encode_packed_plaintext_slots(&public_parameters.parameters, &packed_slots)
                            .expect("fixture packed input must encode");
                    assert_eq!(
                        fixture_str(input, "expected_packed_plaintext_sha256"),
                        coefficient_vector_sha256_hex(&packed_plaintext),
                        "{} input {} packed plaintext digest",
                        fixture_str(vector, "name"),
                        fixture_str(input, "seed_utf8")
                    );
                    let ciphertext = encrypt_from_seed(
                        &public_parameters.parameters,
                        &public_parameters.public_key,
                        &packed_plaintext,
                        fixture_str(input, "seed_utf8").as_bytes(),
                    )
                    .expect("fixture packed input must encrypt");
                    let envelope = BfvIdentifierCiphertext {
                        slots: vec![ciphertext],
                    };
                    let encoded = norito::to_bytes(&envelope).expect("encode packed fixture input");
                    assert_eq!(
                        fixture_u64(input, "expected_ciphertext_bytes"),
                        u64::try_from(encoded.len()).expect("encoded input length fits u64"),
                        "{} input {} byte length",
                        fixture_str(vector, "name"),
                        fixture_str(input, "seed_utf8")
                    );
                    assert_eq!(
                        fixture_str(input, "expected_ciphertext_sha256"),
                        sha256_hex(&encoded),
                        "{} input {} digest",
                        fixture_str(vector, "name"),
                        fixture_str(input, "seed_utf8")
                    );
                    return envelope;
                }
                let input_bytes =
                    hex::decode(fixture_str(input, "input_hex")).expect("fixture input_hex");
                let ciphertext = encrypt_identifier_from_seed(
                    public_parameters,
                    &input_bytes,
                    fixture_str(input, "seed_utf8").as_bytes(),
                )
                .expect("fixture input must encrypt");
                let encoded = norito::to_bytes(&ciphertext).expect("encode fixture input");
                assert_eq!(
                    fixture_u64(input, "expected_ciphertext_bytes"),
                    u64::try_from(encoded.len()).expect("encoded input length fits u64"),
                    "{} input {} byte length",
                    fixture_str(vector, "name"),
                    fixture_str(input, "seed_utf8")
                );
                assert_eq!(
                    fixture_str(input, "expected_ciphertext_sha256"),
                    sha256_hex(&encoded),
                    "{} input {} digest",
                    fixture_str(vector, "name"),
                    fixture_str(input, "seed_utf8")
                );
                ciphertext
            })
            .collect()
    }

    fn operation_vector_job(vector: &norito::json::Value) -> FheJobSpecV1 {
        let mut job = sample_fhe_job(Vec::new());
        match fixture_str(vector, "operation") {
            "Add" => {
                job.operation = FheJobOperationV1::Add;
            }
            "Multiply" => {
                job.operation = FheJobOperationV1::Multiply;
                job.requested_multiplication_depth =
                    u16::try_from(fixture_u64(vector, "requested_multiplication_depth"))
                        .expect("fixture requested_multiplication_depth must fit u16");
            }
            "RotateLeft" => {
                job.operation = FheJobOperationV1::RotateLeft;
                job.rotation_steps = u32::try_from(fixture_u64(vector, "rotation_steps"))
                    .expect("fixture rotation_steps must fit u32");
            }
            "Bootstrap" => {
                job.operation = FheJobOperationV1::Bootstrap;
                job.bootstrap_count = u16::try_from(fixture_u64(vector, "bootstrap_count"))
                    .expect("fixture bootstrap_count must fit u16");
            }
            operation => panic!("unsupported fixture operation `{operation}`"),
        }
        job
    }

    fn output_plaintext_slots(
        params: &BfvParameters,
        secret_key: &iroha_crypto::fhe_bfv::BfvSecretKey,
        output: &BfvIdentifierCiphertext,
    ) -> Vec<u64> {
        output
            .slots
            .iter()
            .map(|slot| decrypt(params, secret_key, slot).expect("decrypt output slot")[0])
            .collect()
    }

    fn expected_plaintext_slots(vector: &norito::json::Value) -> Vec<u64> {
        fixture_array(vector, "expected_plaintext_slots")
            .iter()
            .map(|slot| slot.as_u64().expect("expected plaintext slot must be u64"))
            .collect()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode_upper(Sha256::digest(bytes))
    }

    fn coefficient_vector_sha256_hex(values: &[u64]) -> String {
        let encoded = norito::to_bytes(&values.to_vec()).expect("encode coefficient vector");
        sha256_hex(&encoded)
    }

    fn coefficient_u128_vector_sha256_hex(values: &[u128]) -> String {
        let mut encoded = Vec::with_capacity(values.len() * 16);
        for value in values {
            encoded.extend_from_slice(&value.to_le_bytes());
        }
        sha256_hex(&encoded)
    }

    fn u64_coefficients_to_u128(values: &[u64]) -> Vec<u128> {
        values.iter().copied().map(u128::from).collect()
    }

    fn assert_ciphertext_component_fixture(
        fixture: &norito::json::Value,
        label: &str,
        params: &BfvParameters,
        ciphertext: &BfvCiphertext,
    ) {
        assert_eq!(
            fixture_u64(fixture, "coefficient_count"),
            u64::from(params.polynomial_degree),
            "{label} coefficient count"
        );
        assert_eq!(
            fixture_str(fixture, "c0_sha256"),
            coefficient_vector_sha256_hex(&ciphertext.c0),
            "{label} c0 SHA-256"
        );
        assert_eq!(
            fixture_str(fixture, "c1_sha256"),
            coefficient_vector_sha256_hex(&ciphertext.c1),
            "{label} c1 SHA-256"
        );
    }

    fn decimal_json_array(values: &[u64]) -> String {
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn u64_json_array(values: &[u64]) -> String {
        values
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn string_json_array(values: &[String]) -> String {
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn rns_sample_lhs_coefficients(params: &BfvParameters) -> Vec<u64> {
        rns_sample_coefficients(params, 3, params.plaintext_modulus + 11)
    }

    fn rns_sample_rhs_coefficients(params: &BfvParameters) -> Vec<u64> {
        rns_sample_coefficients(params, 5, params.plaintext_modulus + 29)
    }

    fn rns_sample_coefficients(
        params: &BfvParameters,
        index_offset: u64,
        multiplier: u64,
    ) -> Vec<u64> {
        (0..u64::from(params.polynomial_degree))
            .map(|index| {
                let coefficient = (u128::from(index + index_offset) * u128::from(multiplier))
                    % u128::from(params.ciphertext_modulus);
                u64::try_from(coefficient).expect("RNS sample coefficient fits u64")
            })
            .collect()
    }

    fn reconstruct_rns_polynomial(
        params: &BfvParameters,
        chain: &iroha_crypto::fhe_bfv::BfvRnsModulusChain,
        polynomial: &iroha_crypto::fhe_bfv::BfvRnsPolynomial,
    ) -> Vec<u128> {
        chain
            .reconstruct_polynomial(params, polynomial)
            .expect("reconstruct RNS fixture polynomial")
    }

    fn rns_polynomial_fixture_json(
        params: &BfvParameters,
        polynomial: &iroha_crypto::fhe_bfv::BfvRnsPolynomial,
        reconstructed: &[u128],
    ) -> String {
        let limb_hashes = polynomial
            .residues_by_limb
            .iter()
            .map(|residues| coefficient_vector_sha256_hex(residues))
            .collect::<Vec<_>>();
        format!(
            "{{\"coefficient_count\":{},\"residue_limb_sha256\":[{}],\"reconstructed_sha256\":\"{}\"}}",
            params.polynomial_degree,
            string_json_array(&limb_hashes),
            coefficient_u128_vector_sha256_hex(reconstructed)
        )
    }

    fn rns_modulus_chain_fixture_json(params: &BfvParameters) -> String {
        let chain = registered_bfv_rns_modulus_chain(params).expect("registered BFV RNS chain");
        let lhs_coefficients = rns_sample_lhs_coefficients(params);
        let rhs_coefficients = rns_sample_rhs_coefficients(params);
        let lhs = chain
            .decompose_polynomial(params, &lhs_coefficients)
            .expect("decompose RNS fixture lhs");
        let rhs = chain
            .decompose_polynomial(params, &rhs_coefficients)
            .expect("decompose RNS fixture rhs");
        let sum = chain
            .add_rns_polynomials(params, &lhs, &rhs)
            .expect("add RNS fixture polynomials");
        let product = chain
            .multiply_rns_polynomials_negacyclic(params, &lhs, &rhs)
            .expect("multiply RNS fixture polynomials");
        let reconstructed_lhs = u64_coefficients_to_u128(&lhs_coefficients);
        let reconstructed_rhs = u64_coefficients_to_u128(&rhs_coefficients);
        let reconstructed_sum = reconstruct_rns_polynomial(params, &chain, &sum);
        let reconstructed_product = reconstruct_rns_polynomial(params, &chain, &product);

        format!(
            "{{\"moduli\":[{}],\"product\":\"{}\",\"expected_digest_hex\":\"{}\",\"sample_polynomials\":{{\"lhs_coefficients\":[{}],\"rhs_coefficients\":[{}],\"lhs\":{},\"rhs\":{},\"sum\":{},\"negacyclic_product\":{}}}}}",
            u64_json_array(&chain.moduli),
            chain.product().expect("RNS modulus-chain product"),
            registered_bfv_rns_modulus_chain_digest(params)
                .expect("registered BFV RNS chain digest"),
            u64_json_array(&lhs_coefficients),
            u64_json_array(&rhs_coefficients),
            rns_polynomial_fixture_json(params, &lhs, &reconstructed_lhs),
            rns_polynomial_fixture_json(params, &rhs, &reconstructed_rhs),
            rns_polynomial_fixture_json(params, &sum, &reconstructed_sum),
            rns_polynomial_fixture_json(params, &product, &reconstructed_product)
        )
    }

    fn execute_operation_vector(
        params: &BfvParameters,
        public_parameters: &BfvIdentifierPublicParameters,
        evaluation_keys: &BfvEvaluationKeyBundle,
        vector: &norito::json::Value,
    ) -> BfvIdentifierCiphertext {
        let inputs = operation_vector_inputs(public_parameters, vector);
        let job = operation_vector_job(vector);
        execute_soracloud_fhe_job(params, evaluation_keys, &job, &inputs)
            .expect("fixture FHE operation must execute")
    }

    #[test]
    fn soracloud_bfv_operation_vectors_match_shared_fixture() {
        let root = shared_bfv_fixture();
        let operation_vectors = fixture_operation_vectors(&root);
        let (params, public_parameters, secret_key, evaluation_keys) =
            bfv_operation_material(operation_vectors);
        let mut seen_digests = BTreeSet::new();
        let galois_key_powers = evaluation_keys
            .galois_keys
            .iter()
            .map(|key| key.automorphism_power)
            .collect::<BTreeSet<_>>();

        for vector in fixture_array(operation_vectors, "vectors") {
            let expected_depth = if fixture_str(vector, "operation") == "Multiply" {
                bfv_balanced_multiplication_depth(fixture_array(vector, "inputs").len())
                    .expect("fixture multiply input count must produce a BFV depth plan")
            } else {
                0
            };
            assert_eq!(
                fixture_u64(vector, "requested_multiplication_depth"),
                u64::from(expected_depth),
                "{} requested multiplication depth",
                fixture_str(vector, "name")
            );
            if vector.get("automorphism_powers").is_some() {
                let rotation_steps = u32::try_from(fixture_u64(vector, "rotation_steps"))
                    .expect("fixture rotation_steps must fit u32");
                let automorphism_powers = fixture_u64_array(vector, "automorphism_powers")
                    .into_iter()
                    .map(|power| {
                        u32::try_from(power)
                            .expect("fixture Galois automorphism power must fit u32")
                    })
                    .collect::<Vec<_>>();
                assert!(
                    automorphism_powers.len() > 1,
                    "{} Galois schedule must use multiple powers",
                    fixture_str(vector, "name")
                );
                assert_eq!(
                    automorphism_powers,
                    packed_left_rotation_galois_automorphism_powers(&params, rotation_steps)
                        .expect("fixture rotation schedule must derive"),
                    "{} Galois schedule",
                    fixture_str(vector, "name")
                );
                for power in &automorphism_powers {
                    assert!(
                        galois_key_powers.contains(power),
                        "{} Galois schedule power {power} is missing from evaluation keys",
                        fixture_str(vector, "name")
                    );
                }
            }
            let output =
                execute_operation_vector(&params, &public_parameters, &evaluation_keys, vector);
            let encoded_output = norito::to_bytes(&output).expect("encode fixture output");
            assert_eq!(
                fixture_u64(vector, "expected_output_ciphertext_bytes"),
                u64::try_from(encoded_output.len()).expect("encoded output length fits u64"),
                "{} output byte length",
                fixture_str(vector, "name")
            );
            let digest = sha256_hex(&encoded_output);
            assert_eq!(
                fixture_str(vector, "expected_output_ciphertext_sha256"),
                digest,
                "{} output digest",
                fixture_str(vector, "name")
            );
            assert!(
                seen_digests.insert(digest.clone()),
                "operation fixture output digests must be unique: {digest}"
            );
            if vector.get("expected_packed_slots").is_some() {
                assert_eq!(
                    output.slots.len(),
                    1,
                    "{} packed output must contain one ciphertext",
                    fixture_str(vector, "name")
                );
                let plaintext =
                    decrypt(&params, &secret_key, &output.slots[0]).expect("decrypt packed output");
                assert_eq!(
                    fixture_str(vector, "expected_plaintext_coefficients_sha256"),
                    coefficient_vector_sha256_hex(&plaintext),
                    "{} packed plaintext coefficient digest",
                    fixture_str(vector, "name")
                );
                assert_eq!(
                    fixture_u64_array(vector, "expected_packed_slots"),
                    decode_packed_plaintext_slots(&params, &plaintext)
                        .expect("decode packed output slots"),
                    "{} packed output slots",
                    fixture_str(vector, "name")
                );
                assert_ciphertext_component_fixture(
                    fixture_get(vector, "output_components"),
                    fixture_str(vector, "name"),
                    &params,
                    &output.slots[0],
                );
            } else {
                assert_eq!(
                    expected_plaintext_slots(vector),
                    output_plaintext_slots(&params, &secret_key, &output),
                    "{} plaintext slots",
                    fixture_str(vector, "name")
                );
            }
            if let Some(expected_utf8) = vector
                .get("expected_output_utf8")
                .and_then(norito::json::Value::as_str)
            {
                let plaintext = decrypt_identifier(&public_parameters, &secret_key, &output)
                    .expect("fixture output must decrypt as identifier");
                assert_eq!(expected_utf8.as_bytes(), plaintext);
            }
        }
    }

    #[test]
    fn soracloud_bfv_key_fixture_rejects_valid_wrong_key_material() {
        let root = shared_bfv_fixture();
        let operation_vectors = fixture_operation_vectors(&root);
        let params = ram_lfe_bfv_parameters_v1();
        let (_secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-wrong-keygen").expect("wrong keygen");
        let wrong_public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes: u16::try_from(fixture_u64(operation_vectors, "max_input_bytes"))
                .expect("fixture max_input_bytes must fit u16"),
        };
        wrong_public_parameters
            .validate()
            .expect("wrong but well-formed public parameters must validate structurally");
        let wrong_rotation_key = rotation_key_from_seed(
            &params,
            &public_key,
            1,
            fixture_str(
                &fixture_array(operation_vectors, "rotation_keys")[0],
                "seed_utf8",
            )
            .as_bytes(),
        )
        .expect("wrong rotation key");
        let wrong_bootstrap_key = bootstrap_key_from_seed(
            &params,
            &public_key,
            fixture_str(fixture_get(operation_vectors, "bootstrap_key"), "key_id"),
            fixture_str(fixture_get(operation_vectors, "bootstrap_key"), "seed_utf8").as_bytes(),
        )
        .expect("wrong bootstrap key");
        let wrong_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![wrong_rotation_key],
            galois_keys: Vec::new(),
            bootstrap_key: Some(wrong_bootstrap_key),
        };
        wrong_keys
            .validate(&params)
            .expect("wrong but well-formed key material must validate structurally");

        let public_key_fixture = fixture_get(operation_vectors, "public_key");
        let encoded_public_key =
            norito::to_bytes(&wrong_public_parameters.public_key).expect("encode wrong public key");
        assert_ne!(
            fixture_str(public_key_fixture, "expected_sha256"),
            sha256_hex(&encoded_public_key),
            "fixture must reject a structurally valid but different public key"
        );
        let public_parameters_fixture = fixture_get(operation_vectors, "public_parameters");
        let encoded_public_parameters =
            norito::to_bytes(&wrong_public_parameters).expect("encode wrong public parameters");
        assert_ne!(
            fixture_str(public_parameters_fixture, "expected_sha256"),
            sha256_hex(&encoded_public_parameters),
            "fixture must reject structurally valid public parameters with a different key"
        );

        let key_fixture = fixture_get(operation_vectors, "evaluation_key_bundle");
        let encoded_keys = norito::to_bytes(&wrong_keys).expect("encode wrong keys");
        assert_ne!(
            fixture_str(key_fixture, "expected_sha256"),
            sha256_hex(&encoded_keys),
            "fixture must reject a structurally valid but different evaluation-key bundle"
        );
        assert_ne!(
            fixture_str(key_fixture, "expected_digest_hex"),
            wrong_keys
                .digest(&params)
                .expect("wrong key digest")
                .to_string(),
            "fixture must reject a different domain-separated evaluation-key digest"
        );
        let wrong_relinearization_entry = &wrong_keys.relinearization_key.entries[0];
        let relinearization_fixture = &fixture_array(key_fixture, "relinearization_entries")[0];
        assert_ne!(
            fixture_str(relinearization_fixture, "b_sha256"),
            coefficient_vector_sha256_hex(&wrong_relinearization_entry.b),
            "fixture must reject wrong relinearization b component material"
        );
        assert_ne!(
            fixture_str(relinearization_fixture, "a_sha256"),
            coefficient_vector_sha256_hex(&wrong_relinearization_entry.a),
            "fixture must reject wrong relinearization a component material"
        );

        let rotation_fixture = &fixture_array(operation_vectors, "rotation_keys")[0];
        let encoded_rotation_refresh = norito::to_bytes(&wrong_keys.rotation_keys[0].zero_refresh)
            .expect("encode wrong rotation refresh");
        assert_ne!(
            fixture_str(rotation_fixture, "expected_zero_refresh_sha256"),
            sha256_hex(&encoded_rotation_refresh),
            "fixture must reject wrong rotation refresh material"
        );
        let rotation_components = fixture_get(rotation_fixture, "zero_refresh_components");
        assert_ne!(
            fixture_str(rotation_components, "c0_sha256"),
            coefficient_vector_sha256_hex(&wrong_keys.rotation_keys[0].zero_refresh.c0),
            "fixture must reject wrong rotation refresh c0 component material"
        );
        assert_ne!(
            fixture_str(rotation_components, "c1_sha256"),
            coefficient_vector_sha256_hex(&wrong_keys.rotation_keys[0].zero_refresh.c1),
            "fixture must reject wrong rotation refresh c1 component material"
        );
        let bootstrap_fixture = fixture_get(operation_vectors, "bootstrap_key");
        let encoded_bootstrap_refresh = norito::to_bytes(
            &wrong_keys
                .bootstrap_key
                .as_ref()
                .expect("wrong bootstrap key")
                .zero_refresh,
        )
        .expect("encode wrong bootstrap refresh");
        assert_ne!(
            fixture_str(bootstrap_fixture, "expected_zero_refresh_sha256"),
            sha256_hex(&encoded_bootstrap_refresh),
            "fixture must reject wrong bootstrap refresh material"
        );
        let wrong_bootstrap_refresh = &wrong_keys
            .bootstrap_key
            .as_ref()
            .expect("wrong bootstrap key")
            .zero_refresh;
        let bootstrap_components = fixture_get(bootstrap_fixture, "zero_refresh_components");
        assert_ne!(
            fixture_str(bootstrap_components, "c0_sha256"),
            coefficient_vector_sha256_hex(&wrong_bootstrap_refresh.c0),
            "fixture must reject wrong bootstrap refresh c0 component material"
        );
        assert_ne!(
            fixture_str(bootstrap_components, "c1_sha256"),
            coefficient_vector_sha256_hex(&wrong_bootstrap_refresh.c1),
            "fixture must reject wrong bootstrap refresh c1 component material"
        );
    }

    #[test]
    fn soracloud_fhe_policy_rejects_wrong_evaluation_key_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let policy = sample_fhe_policy();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        verify_soracloud_fhe_evaluation_key_digest(&params, &policy, &evaluation_keys)
            .expect("sample policy must pin the sample evaluation-key bundle");

        let mut wrong_but_well_formed_keys = evaluation_keys;
        wrong_but_well_formed_keys.relinearization_key.entries[0].a[0] ^= 0x01;
        wrong_but_well_formed_keys
            .validate(&params)
            .expect("tampered key material remains structurally valid");
        let err = verify_soracloud_fhe_evaluation_key_digest(
            &params,
            &policy,
            &wrong_but_well_formed_keys,
        )
        .expect_err("wrong key material must not satisfy the policy digest");
        assert_invalid_parameter_contains(err, "evaluation-key digest");

        let mut wrong_policy = policy;
        wrong_policy.evaluation_key_digest = Hash::new(b"wrong-soracloud-fhe-evaluation-keys");
        let err = verify_soracloud_fhe_evaluation_key_digest(
            &params,
            &wrong_policy,
            &sample_bfv_evaluation_key_bundle(),
        )
        .expect_err("wrong policy digest must reject the correct key material");
        assert_invalid_parameter_contains(err, "evaluation-key digest");
    }

    #[test]
    fn soracloud_fhe_policy_rejects_wrong_refresh_transcript_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let policy = sample_fhe_policy();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let transcript = sample_bfv_refresh_transcript();
        verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &transcript,
        )
        .expect("sample policy must pin the sample refresh transcript");

        let mut wrong_transcript = transcript.clone();
        wrong_transcript.rotation_transcripts[0]
            .seed
            .extend_from_slice(b"-wrong");
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &wrong_transcript,
        )
        .expect_err("wrong transcript material must not satisfy the policy digest");
        assert_invalid_parameter_contains(err, "refresh transcript");

        let mut oversized_seed_transcript = transcript.clone();
        oversized_seed_transcript.rotation_transcripts[0].seed =
            vec![0xA5; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES + 1];
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &oversized_seed_transcript,
        )
        .expect_err("unbounded transcript seeds must fail runtime admission preflight");
        assert_invalid_parameter_contains(err, "rotation_transcripts.seed");

        let mut duplicate_rotation_transcript = transcript.clone();
        duplicate_rotation_transcript
            .rotation_transcripts
            .push(BfvRotationRefreshTranscriptV1 {
                rotation_steps: duplicate_rotation_transcript.rotation_transcripts[0]
                    .rotation_steps,
                seed: b"soracloud-fhe-duplicate-rotation".to_vec(),
            });
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &duplicate_rotation_transcript,
        )
        .expect_err("duplicate rotation transcript steps must fail runtime admission preflight");
        assert_invalid_parameter_contains(err, "rotation_transcripts.rotation_steps");

        let mut oversized_key_id_transcript = transcript.clone();
        oversized_key_id_transcript
            .bootstrap_transcript
            .as_mut()
            .expect("sample bootstrap transcript")
            .key_id = "k".repeat(BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES + 1);
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &oversized_key_id_transcript,
        )
        .expect_err("unbounded bootstrap transcript key ids must fail runtime admission preflight");
        assert_invalid_parameter_contains(err, "bootstrap_transcript.key_id");

        let mut oversized_rounds_transcript = transcript.clone();
        oversized_rounds_transcript
            .bootstrap_transcript
            .as_mut()
            .expect("sample bootstrap transcript")
            .max_refresh_rounds = u16::MAX;
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &policy,
            &evaluation_keys,
            &oversized_rounds_transcript,
        )
        .expect_err("unbounded bootstrap transcript rounds must fail runtime admission preflight");
        assert_invalid_parameter_contains(err, "bootstrap_transcript.max_refresh_rounds");

        let mut wrong_policy = policy;
        wrong_policy.evaluation_key_refresh_transcript_digest =
            Hash::new(b"wrong-soracloud-fhe-refresh-transcript");
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &wrong_policy,
            &evaluation_keys,
            &sample_bfv_refresh_transcript(),
        )
        .expect_err("wrong policy digest must reject the correct refresh transcript");
        assert_invalid_parameter_contains(err, "refresh transcript digest");
    }

    #[test]
    fn soracloud_fhe_policy_binds_refresh_transcript_mode() {
        let (params, evaluation_keys, transcript, bounded_digest) =
            sample_bounded_noise_bfv_refresh_material();
        let mut bounded_policy = sample_fhe_policy();
        bounded_policy.evaluation_key_digest = evaluation_keys
            .digest(&params)
            .expect("bounded-noise evaluation-key digest");
        bounded_policy.evaluation_key_refresh_transcript_digest = bounded_digest;
        bounded_policy.refresh_transcript_mode = BfvRefreshTranscriptModeV1::BoundedNoise;
        verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &bounded_policy,
            &evaluation_keys,
            &transcript,
        )
        .expect("bounded policy must pin bounded refresh transcript digest");
        assert_eq!(
            soracloud_fhe_ciphertext_bound_mode(&bounded_policy),
            BfvCiphertextBoundModeV1::BoundedNoise,
            "bounded policy must require bounded-noise ciphertext metadata"
        );

        let mut exact_mode_policy = bounded_policy.clone();
        exact_mode_policy.refresh_transcript_mode = BfvRefreshTranscriptModeV1::ExactLift;
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &params,
            &exact_mode_policy,
            &evaluation_keys,
            &transcript,
        )
        .expect_err("exact-mode policy must reject bounded refresh transcript material");
        assert_invalid_parameter_contains(err, "refresh transcript");

        let mut bounded_mode_exact_policy = sample_fhe_policy();
        bounded_mode_exact_policy.refresh_transcript_mode =
            BfvRefreshTranscriptModeV1::BoundedNoise;
        let err = verify_soracloud_fhe_refresh_transcript_digest(
            &ram_lfe_bfv_parameters_v1(),
            &bounded_mode_exact_policy,
            &sample_bfv_evaluation_key_bundle(),
            &sample_bfv_refresh_transcript(),
        )
        .expect_err("bounded-mode policy must reject exact-lift refresh transcript material");
        assert_invalid_parameter_contains(err, "refresh transcript");
    }

    #[test]
    fn soracloud_bfv_operation_vectors_reject_tampered_refresh_material() {
        let root = shared_bfv_fixture();
        let operation_vectors = fixture_operation_vectors(&root);
        let (params, public_parameters, _secret_key, evaluation_keys) =
            bfv_operation_material(operation_vectors);

        let rotate = fixture_array(operation_vectors, "vectors")
            .iter()
            .find(|vector| fixture_str(vector, "operation") == "RotateLeft")
            .expect("fixture must include RotateLeft");
        let rotate_inputs = operation_vector_inputs(&public_parameters, rotate);
        let rotate_job = operation_vector_job(rotate);
        let mut tampered_rotation_keys = evaluation_keys.clone();
        tampered_rotation_keys.rotation_keys[0].zero_refresh.c0[0] = params.ciphertext_modulus;
        let err = execute_soracloud_fhe_job(
            &params,
            &tampered_rotation_keys,
            &rotate_job,
            &rotate_inputs,
        )
        .expect_err("tampered rotation refresh material must reject");
        assert_invalid_parameter_contains(err, "FHE rotate failed");

        let bootstrap = fixture_array(operation_vectors, "vectors")
            .iter()
            .find(|vector| fixture_str(vector, "operation") == "Bootstrap")
            .expect("fixture must include Bootstrap");
        let bootstrap_inputs = operation_vector_inputs(&public_parameters, bootstrap);
        let bootstrap_job = operation_vector_job(bootstrap);
        let mut tampered_bootstrap_keys = evaluation_keys;
        tampered_bootstrap_keys
            .bootstrap_key
            .as_mut()
            .expect("fixture bootstrap key")
            .zero_refresh
            .c1
            .pop();
        let err = execute_soracloud_fhe_job(
            &params,
            &tampered_bootstrap_keys,
            &bootstrap_job,
            &bootstrap_inputs,
        )
        .expect_err("tampered bootstrap refresh material must reject");
        assert_invalid_parameter_contains(err, "FHE bootstrap failed");
    }

    #[test]
    #[ignore = "prints refreshed Soracloud BFV operation-vector fixture rows"]
    fn print_soracloud_bfv_operation_vectors() {
        let params = ram_lfe_bfv_parameters_v1();
        let rns_chain = registered_bfv_rns_modulus_chain(&params).expect("registered RNS chain");
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes: 8,
        };
        let encoded_public_key = norito::to_bytes(&public_key).expect("encode public key");
        let encoded_public_parameters =
            norito::to_bytes(&public_parameters).expect("encode public parameters");
        println!(
            "public-key: bytes={} sha256={} polynomial_degree={} plaintext_modulus={} ciphertext_modulus={} decomposition_base_log={}",
            encoded_public_key.len(),
            sha256_hex(&encoded_public_key),
            params.polynomial_degree,
            params.plaintext_modulus,
            params.ciphertext_modulus,
            params.decomposition_base_log
        );
        println!(
            "public-parameters: bytes={} sha256={} max_input_bytes={}",
            encoded_public_parameters.len(),
            sha256_hex(&encoded_public_parameters),
            public_parameters.max_input_bytes
        );
        println!(
            "public-parameters-decoded: {{\"parameters\":{{\"polynomial_degree\":{},\"plaintext_modulus\":{},\"ciphertext_modulus\":\"{}\",\"decomposition_base_log\":{}}},\"public_key\":{{\"b\":[{}],\"a\":[{}]}},\"max_input_bytes\":{},\"norito_length_encoding\":\"compact-v1\"}}",
            params.polynomial_degree,
            params.plaintext_modulus,
            params.ciphertext_modulus,
            params.decomposition_base_log,
            decimal_json_array(&public_key.b),
            decimal_json_array(&public_key.a),
            public_parameters.max_input_bytes
        );
        println!(
            "rns-modulus-chain-json: {}",
            rns_modulus_chain_fixture_json(&params)
        );
        let packed_half_rotation = u32::from(params.polynomial_degree) / 2;
        let packed_half_rotation_power =
            packed_left_rotation_galois_automorphism_power(&params, packed_half_rotation)
                .expect("registered packed half-rotation must be one Galois automorphism");
        let packed_schedule_rotation = 1_u32;
        let packed_schedule_powers =
            packed_left_rotation_galois_automorphism_powers(&params, packed_schedule_rotation)
                .expect("registered packed one-step rotation must have a Galois schedule");
        let mut seen_galois_powers = BTreeSet::new();
        let mut galois_key_specs = Vec::<(u32, String)>::new();
        for (power, seed) in [
            (3, "soracloud-fhe-galois-key".to_string()),
            (
                packed_half_rotation_power,
                "soracloud-fhe-packed-rotate-galois-key".to_string(),
            ),
        ] {
            seen_galois_powers.insert(power);
            galois_key_specs.push((power, seed));
        }
        for power in &packed_schedule_powers {
            if seen_galois_powers.insert(*power) {
                galois_key_specs.push((
                    *power,
                    format!("soracloud-fhe-packed-rotate-schedule-galois-key-{power}"),
                ));
            }
        }
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![
                rotation_key_from_seed(&params, &public_key, 1, b"soracloud-fhe-rotation-key")
                    .expect("rotation key"),
            ],
            galois_keys: galois_key_specs
                .iter()
                .map(|(power, seed)| {
                    galois_key_from_seed(&params, &secret_key, *power, seed.as_bytes())
                        .expect("Galois key")
                })
                .collect(),
            bootstrap_key: Some(
                bootstrap_key_with_max_refresh_rounds_from_seed(
                    &params,
                    &public_key,
                    "bootstrap-test-key",
                    2,
                    b"soracloud-fhe-bootstrap-key",
                )
                .expect("bootstrap key"),
            ),
        };
        let encoded_evaluation_keys =
            norito::to_bytes(&evaluation_keys).expect("encode evaluation keys");
        println!(
            "evaluation-key-bundle: bytes={} sha256={} digest={} relinearization_entries={} rotation_key_count={} galois_key_count={} bootstrap_key_id={} bootstrap_max_refresh_rounds={}",
            encoded_evaluation_keys.len(),
            sha256_hex(&encoded_evaluation_keys),
            evaluation_keys
                .digest(&params)
                .expect("evaluation-key digest"),
            evaluation_keys.relinearization_key.entries.len(),
            evaluation_keys.rotation_keys.len(),
            evaluation_keys.galois_keys.len(),
            evaluation_keys
                .bootstrap_key
                .as_ref()
                .expect("bootstrap key")
                .key_id,
            evaluation_keys
                .bootstrap_key
                .as_ref()
                .expect("bootstrap key")
                .max_refresh_rounds
        );
        for (index, entry) in evaluation_keys
            .relinearization_key
            .entries
            .iter()
            .enumerate()
        {
            println!(
                "relinearization-entry: index={} coeffs={} b_sha256={} a_sha256={}",
                index,
                entry.b.len(),
                coefficient_vector_sha256_hex(&entry.b),
                coefficient_vector_sha256_hex(&entry.a)
            );
        }
        for (key, (_power, seed)) in evaluation_keys.galois_keys.iter().zip(&galois_key_specs) {
            println!(
                "galois-key: power={} seed={} entry_count={}",
                key.automorphism_power,
                seed,
                key.entries.len(),
            );
            for (index, entry) in key.entries.iter().enumerate() {
                println!(
                    "galois-key-entry: power={} index={} coeffs={} b_sha256={} a_sha256={}",
                    key.automorphism_power,
                    index,
                    entry.b.len(),
                    coefficient_vector_sha256_hex(&entry.b),
                    coefficient_vector_sha256_hex(&entry.a)
                );
            }
        }
        let galois_input_plaintext = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let galois_input = encrypt_from_seed(
            &params,
            &public_key,
            &galois_input_plaintext,
            b"soracloud-fhe-galois-switch-input",
        )
        .expect("encrypt Galois switch input");
        let encoded_galois_input =
            norito::to_bytes(&galois_input).expect("encode Galois switch input");
        let galois_output = apply_galois_automorphism_ciphertext(
            &params,
            &evaluation_keys.galois_keys[0],
            &galois_input,
        )
        .expect("apply Galois switch");
        let encoded_galois_output =
            norito::to_bytes(&galois_output).expect("encode Galois switch output");
        let galois_plaintext =
            decrypt(&params, &secret_key, &galois_output).expect("decrypt Galois switch output");
        println!(
            "galois-switch-vector: {{\"name\":\"soracloud-galois-power-3-output\",\"purpose\":\"BFV packed-polynomial Galois key-switch output over one scalar ciphertext\",\"automorphism_power\":{},\"seed_utf8\":\"soracloud-fhe-galois-switch-input\",\"input_plaintext_slots\":[{}],\"expected_input_ciphertext_bytes\":{},\"expected_input_ciphertext_sha256\":\"{}\",\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_sha256\":\"{}\",\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            evaluation_keys.galois_keys[0].automorphism_power,
            u64_json_array(&galois_input_plaintext),
            encoded_galois_input.len(),
            sha256_hex(&encoded_galois_input),
            encoded_galois_output.len(),
            sha256_hex(&encoded_galois_output),
            coefficient_vector_sha256_hex(&galois_plaintext),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&galois_output.c0),
            coefficient_vector_sha256_hex(&galois_output.c1)
        );
        let packed_galois_input_slots = (0..usize::from(params.polynomial_degree))
            .map(|index| u64::try_from(index + 1).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        let packed_galois_plaintext =
            encode_packed_plaintext_slots(&params, &packed_galois_input_slots)
                .expect("encode packed Galois input");
        let packed_galois_permutation = packed_galois_slot_permutation(
            &params,
            evaluation_keys.galois_keys[0].automorphism_power,
        )
        .expect("packed Galois slot permutation");
        let packed_galois_expected_slots = packed_galois_permutation
            .iter()
            .map(|&input_index| packed_galois_input_slots[input_index])
            .collect::<Vec<_>>();
        let packed_galois_input = encrypt_from_seed(
            &params,
            &public_key,
            &packed_galois_plaintext,
            b"soracloud-fhe-packed-galois-switch-input",
        )
        .expect("encrypt packed Galois switch input");
        let encoded_packed_galois_input =
            norito::to_bytes(&packed_galois_input).expect("encode packed Galois switch input");
        let packed_galois_output = apply_galois_automorphism_ciphertext(
            &params,
            &evaluation_keys.galois_keys[0],
            &packed_galois_input,
        )
        .expect("apply packed Galois switch");
        let encoded_packed_galois_output =
            norito::to_bytes(&packed_galois_output).expect("encode packed Galois switch output");
        let packed_galois_plaintext_output = decrypt(&params, &secret_key, &packed_galois_output)
            .expect("decrypt packed Galois switch output");
        assert_eq!(
            decode_packed_plaintext_slots(&params, &packed_galois_plaintext_output)
                .expect("decode packed Galois output"),
            packed_galois_expected_slots
        );
        let packed_galois_permutation_u64 = packed_galois_permutation
            .iter()
            .map(|&slot| u64::try_from(slot).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        println!(
            "packed-galois-switch-vector: {{\"name\":\"soracloud-packed-galois-power-3-slots\",\"purpose\":\"BFV packed-slot Galois key-switch execution vector\",\"automorphism_power\":{},\"seed_utf8\":\"soracloud-fhe-packed-galois-switch-input\",\"input_packed_slots\":[{}],\"expected_slot_permutation\":[{}],\"expected_packed_slots\":[{}],\"expected_packed_plaintext_sha256\":\"{}\",\"expected_input_ciphertext_bytes\":{},\"expected_input_ciphertext_sha256\":\"{}\",\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_coefficients_sha256\":\"{}\",\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            evaluation_keys.galois_keys[0].automorphism_power,
            u64_json_array(&packed_galois_input_slots),
            u64_json_array(&packed_galois_permutation_u64),
            u64_json_array(&packed_galois_expected_slots),
            coefficient_vector_sha256_hex(&packed_galois_plaintext),
            encoded_packed_galois_input.len(),
            sha256_hex(&encoded_packed_galois_input),
            encoded_packed_galois_output.len(),
            sha256_hex(&encoded_packed_galois_output),
            coefficient_vector_sha256_hex(&packed_galois_plaintext_output),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&packed_galois_output.c0),
            coefficient_vector_sha256_hex(&packed_galois_output.c1)
        );
        for key in &evaluation_keys.rotation_keys {
            let encoded_refresh =
                norito::to_bytes(&key.zero_refresh).expect("encode rotation refresh");
            println!(
                "rotation-key: steps={} zero_refresh_bytes={} zero_refresh_sha256={}",
                key.rotation_steps,
                encoded_refresh.len(),
                sha256_hex(&encoded_refresh)
            );
            println!(
                "rotation-key-components: steps={} coeffs={} c0_sha256={} c1_sha256={}",
                key.rotation_steps,
                key.zero_refresh.c0.len(),
                coefficient_vector_sha256_hex(&key.zero_refresh.c0),
                coefficient_vector_sha256_hex(&key.zero_refresh.c1)
            );
        }
        let bootstrap_key = evaluation_keys
            .bootstrap_key
            .as_ref()
            .expect("bootstrap key");
        let encoded_bootstrap_refresh =
            norito::to_bytes(&bootstrap_key.zero_refresh).expect("encode bootstrap refresh");
        println!(
            "bootstrap-key: key_id={} max_refresh_rounds={} zero_refresh_bytes={} zero_refresh_sha256={}",
            bootstrap_key.key_id,
            bootstrap_key.max_refresh_rounds,
            encoded_bootstrap_refresh.len(),
            sha256_hex(&encoded_bootstrap_refresh)
        );
        println!(
            "bootstrap-key-components: key_id={} coeffs={} c0_sha256={} c1_sha256={}",
            bootstrap_key.key_id,
            bootstrap_key.zero_refresh.c0.len(),
            coefficient_vector_sha256_hex(&bootstrap_key.zero_refresh.c0),
            coefficient_vector_sha256_hex(&bootstrap_key.zero_refresh.c1)
        );
        for (round_index, refresh) in bootstrap_key.round_refreshes.iter().enumerate() {
            let encoded_refresh =
                norito::to_bytes(refresh).expect("encode bootstrap round refresh");
            println!(
                "bootstrap-key-round-json: {{\"round_index\":{},\"expected_refresh_bytes\":{},\"expected_refresh_sha256\":\"{}\",\"components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
                round_index,
                encoded_refresh.len(),
                sha256_hex(&encoded_refresh),
                refresh.c0.len(),
                coefficient_vector_sha256_hex(&refresh.c0),
                coefficient_vector_sha256_hex(&refresh.c1)
            );
        }
        let bootstrap_input_plaintext = vec![9, 8, 7, 6, 5, 4, 3, 2];
        let bootstrap_input = encrypt_from_seed(
            &params,
            &public_key,
            &bootstrap_input_plaintext,
            b"soracloud-fhe-bootstrap-refresh-input",
        )
        .expect("encrypt bootstrap refresh input");
        let encoded_bootstrap_input =
            norito::to_bytes(&bootstrap_input).expect("encode bootstrap refresh input");
        let bootstrap_output = bootstrap_ciphertext_rns_exact_round(
            &params,
            &rns_chain,
            bootstrap_key,
            &bootstrap_input,
            0,
        )
        .expect("apply bootstrap refresh");
        let encoded_bootstrap_output =
            norito::to_bytes(&bootstrap_output).expect("encode bootstrap refresh output");
        let bootstrap_plaintext = decrypt(&params, &secret_key, &bootstrap_output)
            .expect("decrypt bootstrap refresh output");
        println!(
            "bootstrap-refresh-vector: {{\"name\":\"soracloud-bootstrap-refresh-output\",\"purpose\":\"BFV bootstrap encrypted-zero refresh output over one scalar ciphertext\",\"key_id\":\"{}\",\"refresh_rounds\":1,\"seed_utf8\":\"soracloud-fhe-bootstrap-refresh-input\",\"input_plaintext_slots\":[{}],\"expected_input_ciphertext_bytes\":{},\"expected_input_ciphertext_sha256\":\"{}\",\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_sha256\":\"{}\",\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            bootstrap_key.key_id,
            u64_json_array(&bootstrap_input_plaintext),
            encoded_bootstrap_input.len(),
            sha256_hex(&encoded_bootstrap_input),
            encoded_bootstrap_output.len(),
            sha256_hex(&encoded_bootstrap_output),
            coefficient_vector_sha256_hex(&bootstrap_plaintext),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&bootstrap_output.c0),
            coefficient_vector_sha256_hex(&bootstrap_output.c1)
        );
        let second_bootstrap_output = bootstrap_ciphertext_rns_exact_round(
            &params,
            &rns_chain,
            bootstrap_key,
            &bootstrap_output,
            1,
        )
        .expect("apply second bootstrap refresh");
        let encoded_second_bootstrap_output =
            norito::to_bytes(&second_bootstrap_output).expect("encode second bootstrap output");
        let second_bootstrap_plaintext = decrypt(&params, &secret_key, &second_bootstrap_output)
            .expect("decrypt second bootstrap refresh output");
        println!(
            "bootstrap-refresh-vector: {{\"name\":\"soracloud-bootstrap-refresh-two-round-output\",\"purpose\":\"BFV bounded two-round bootstrap encrypted-zero refresh output over one scalar ciphertext\",\"key_id\":\"{}\",\"refresh_rounds\":2,\"seed_utf8\":\"soracloud-fhe-bootstrap-refresh-input\",\"input_plaintext_slots\":[{}],\"expected_input_ciphertext_bytes\":{},\"expected_input_ciphertext_sha256\":\"{}\",\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_sha256\":\"{}\",\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            bootstrap_key.key_id,
            u64_json_array(&bootstrap_input_plaintext),
            encoded_bootstrap_input.len(),
            sha256_hex(&encoded_bootstrap_input),
            encoded_second_bootstrap_output.len(),
            sha256_hex(&encoded_second_bootstrap_output),
            coefficient_vector_sha256_hex(&second_bootstrap_plaintext),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&second_bootstrap_output.c0),
            coefficient_vector_sha256_hex(&second_bootstrap_output.c1)
        );
        let specs = [
            (
                "soracloud-add-output",
                "Add",
                vec![
                    ("0102", "soracloud-fhe-add-input-1"),
                    ("0304", "soracloud-fhe-add-input-2"),
                    ("0506", "soracloud-fhe-add-input-3"),
                ],
                0,
                0,
                None,
            ),
            (
                "soracloud-multiply-output",
                "Multiply",
                vec![
                    ("0203", "soracloud-fhe-mul-input-1"),
                    ("0405", "soracloud-fhe-mul-input-2"),
                    ("0607", "soracloud-fhe-mul-input-3"),
                ],
                0,
                0,
                None,
            ),
            (
                "soracloud-rotate-left-output",
                "RotateLeft",
                vec![("6162", "soracloud-rotate-input")],
                1,
                0,
                None,
            ),
            (
                "soracloud-bootstrap-output",
                "Bootstrap",
                vec![("616263", "soracloud-bootstrap-input")],
                0,
                1,
                Some("abc"),
            ),
        ];
        for (name, operation, inputs, rotation_steps, bootstrap_count, expected_utf8) in specs {
            let encrypted_inputs = inputs
                .iter()
                .map(|(input_hex, seed_utf8)| {
                    let input_bytes = hex::decode(input_hex).expect("fixture input hex");
                    let ciphertext = encrypt_identifier_from_seed(
                        &public_parameters,
                        &input_bytes,
                        seed_utf8.as_bytes(),
                    )
                    .expect("encrypt fixture input");
                    let encoded_input =
                        norito::to_bytes(&ciphertext).expect("encode fixture input");
                    println!(
                        "{name} input {seed_utf8}: bytes={} sha256={}",
                        encoded_input.len(),
                        sha256_hex(&encoded_input)
                    );
                    ciphertext
                })
                .collect::<Vec<_>>();
            let input_values = inputs
                .iter()
                .map(|(input_hex, seed_utf8)| {
                    let mut input = norito::json::Map::new();
                    input.insert(
                        "input_hex".to_string(),
                        norito::json::Value::from(*input_hex),
                    );
                    input.insert(
                        "seed_utf8".to_string(),
                        norito::json::Value::from(*seed_utf8),
                    );
                    norito::json::Value::Object(input)
                })
                .collect::<Vec<_>>();
            let mut vector = norito::json::Map::new();
            vector.insert("name".to_string(), norito::json::Value::from(name));
            vector.insert(
                "operation".to_string(),
                norito::json::Value::from(operation),
            );
            vector.insert(
                "inputs".to_string(),
                norito::json::Value::Array(input_values),
            );
            vector.insert(
                "rotation_steps".to_string(),
                norito::json::Value::from(rotation_steps),
            );
            let requested_multiplication_depth = if operation == "Multiply" {
                bfv_balanced_multiplication_depth(inputs.len())
                    .expect("fixture multiply input count must produce a BFV depth plan")
            } else {
                0
            };
            vector.insert(
                "requested_multiplication_depth".to_string(),
                norito::json::Value::from(u64::from(requested_multiplication_depth)),
            );
            vector.insert(
                "bootstrap_count".to_string(),
                norito::json::Value::from(bootstrap_count),
            );
            let vector = norito::json::Value::Object(vector);
            let job = operation_vector_job(&vector);
            let output =
                execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &encrypted_inputs)
                    .expect("fixture FHE operation must execute");
            let encoded_output = norito::to_bytes(&output).expect("encode fixture output");
            let expected_slots = output_plaintext_slots(&params, &secret_key, &output);
            println!(
                "{name}: bytes={} sha256={} slots={expected_slots:?} expected_utf8={expected_utf8:?}",
                encoded_output.len(),
                sha256_hex(&encoded_output)
            );
        }
        let packed_rotate_input_slots = (0..usize::from(params.polynomial_degree))
            .map(|index| u64::try_from(index + 1).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        let packed_rotate_plaintext =
            encode_packed_plaintext_slots(&params, &packed_rotate_input_slots)
                .expect("encode packed RotateLeft input");
        let packed_rotate_input_ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &packed_rotate_plaintext,
            b"soracloud-fhe-packed-rotate-input",
        )
        .expect("encrypt packed RotateLeft input");
        let packed_rotate_input = BfvIdentifierCiphertext {
            slots: vec![packed_rotate_input_ciphertext],
        };
        let encoded_packed_rotate_input =
            norito::to_bytes(&packed_rotate_input).expect("encode packed RotateLeft input");
        let packed_rotate_job = FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "packed-rotate-job".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::RotateLeft,
            inputs: vec![sample_fhe_input_ref(
                "/state/private/packed-rotate-input",
                &encoded_packed_rotate_input,
            )],
            output_state_key: "/state/private/packed-rotate-output".to_string(),
            requested_multiplication_depth: 0,
            rotation_steps: packed_half_rotation,
            bootstrap_count: 0,
        };
        let packed_rotate_output = execute_soracloud_fhe_job(
            &params,
            &evaluation_keys,
            &packed_rotate_job,
            &[packed_rotate_input],
        )
        .expect("execute packed RotateLeft fixture");
        let encoded_packed_rotate_output =
            norito::to_bytes(&packed_rotate_output).expect("encode packed RotateLeft output");
        let packed_rotate_plaintext_output =
            decrypt(&params, &secret_key, &packed_rotate_output.slots[0])
                .expect("decrypt packed RotateLeft output");
        let mut packed_rotate_expected_slots = packed_rotate_input_slots.clone();
        packed_rotate_expected_slots
            .rotate_left(usize::try_from(packed_half_rotation).expect("rotation fits usize"));
        println!(
            "packed-rotate-vector: {{\"name\":\"soracloud-packed-rotate-left-output\",\"purpose\":\"Soracloud runtime packed-slot RotateLeft output backed by BFV Galois key-switching\",\"operation\":\"RotateLeft\",\"inputs\":[{{\"packed_slots\":[{}],\"seed_utf8\":\"soracloud-fhe-packed-rotate-input\",\"expected_packed_plaintext_sha256\":\"{}\",\"expected_ciphertext_bytes\":{},\"expected_ciphertext_sha256\":\"{}\"}}],\"rotation_steps\":{},\"requested_multiplication_depth\":0,\"automorphism_power\":{},\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_coefficients_sha256\":\"{}\",\"expected_packed_slots\":[{}],\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            u64_json_array(&packed_rotate_input_slots),
            coefficient_vector_sha256_hex(&packed_rotate_plaintext),
            encoded_packed_rotate_input.len(),
            sha256_hex(&encoded_packed_rotate_input),
            packed_half_rotation,
            packed_half_rotation_power,
            encoded_packed_rotate_output.len(),
            sha256_hex(&encoded_packed_rotate_output),
            coefficient_vector_sha256_hex(&packed_rotate_plaintext_output),
            u64_json_array(&packed_rotate_expected_slots),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&packed_rotate_output.slots[0].c0),
            coefficient_vector_sha256_hex(&packed_rotate_output.slots[0].c1)
        );
        let packed_schedule_input_ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &packed_rotate_plaintext,
            b"soracloud-fhe-packed-rotate-schedule-input",
        )
        .expect("encrypt packed RotateLeft schedule input");
        let packed_schedule_input = BfvIdentifierCiphertext {
            slots: vec![packed_schedule_input_ciphertext],
        };
        let encoded_packed_schedule_input =
            norito::to_bytes(&packed_schedule_input).expect("encode packed RotateLeft input");
        let packed_schedule_job = FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "packed-rotate-schedule-job".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::RotateLeft,
            inputs: vec![sample_fhe_input_ref(
                "/state/private/packed-rotate-schedule-input",
                &encoded_packed_schedule_input,
            )],
            output_state_key: "/state/private/packed-rotate-schedule-output".to_string(),
            requested_multiplication_depth: 0,
            rotation_steps: packed_schedule_rotation,
            bootstrap_count: 0,
        };
        let packed_schedule_output = execute_soracloud_fhe_job(
            &params,
            &evaluation_keys,
            &packed_schedule_job,
            &[packed_schedule_input],
        )
        .expect("execute packed RotateLeft schedule fixture");
        let encoded_packed_schedule_output =
            norito::to_bytes(&packed_schedule_output).expect("encode packed RotateLeft output");
        let packed_schedule_plaintext_output =
            decrypt(&params, &secret_key, &packed_schedule_output.slots[0])
                .expect("decrypt packed RotateLeft output");
        let mut packed_schedule_expected_slots = packed_rotate_input_slots.clone();
        packed_schedule_expected_slots
            .rotate_left(usize::try_from(packed_schedule_rotation).expect("rotation fits usize"));
        let packed_schedule_powers_u64 = packed_schedule_powers
            .iter()
            .map(|power| u64::from(*power))
            .collect::<Vec<_>>();
        println!(
            "packed-rotate-vector: {{\"name\":\"soracloud-packed-rotate-left-schedule-output\",\"purpose\":\"Soracloud runtime packed-slot RotateLeft output backed by a BFV Galois mask-and-sum key schedule\",\"operation\":\"RotateLeft\",\"inputs\":[{{\"packed_slots\":[{}],\"seed_utf8\":\"soracloud-fhe-packed-rotate-schedule-input\",\"expected_packed_plaintext_sha256\":\"{}\",\"expected_ciphertext_bytes\":{},\"expected_ciphertext_sha256\":\"{}\"}}],\"rotation_steps\":{},\"requested_multiplication_depth\":0,\"automorphism_powers\":[{}],\"expected_output_ciphertext_bytes\":{},\"expected_output_ciphertext_sha256\":\"{}\",\"expected_plaintext_coefficients_sha256\":\"{}\",\"expected_packed_slots\":[{}],\"output_components\":{{\"coefficient_count\":{},\"c0_sha256\":\"{}\",\"c1_sha256\":\"{}\"}}}}",
            u64_json_array(&packed_rotate_input_slots),
            coefficient_vector_sha256_hex(&packed_rotate_plaintext),
            encoded_packed_schedule_input.len(),
            sha256_hex(&encoded_packed_schedule_input),
            packed_schedule_rotation,
            u64_json_array(&packed_schedule_powers_u64),
            encoded_packed_schedule_output.len(),
            sha256_hex(&encoded_packed_schedule_output),
            coefficient_vector_sha256_hex(&packed_schedule_plaintext_output),
            u64_json_array(&packed_schedule_expected_slots),
            params.polynomial_degree,
            coefficient_vector_sha256_hex(&packed_schedule_output.slots[0].c0),
            coefficient_vector_sha256_hex(&packed_schedule_output.slots[0].c1)
        );
    }

    #[test]
    fn soracloud_multi_input_add_matches_plaintext_slots() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, _relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key,
            max_input_bytes: 8,
        };
        let inputs = [
            encrypt_identifier_from_seed(&public_parameters, &[1, 2], b"soracloud-fhe-add-input-1")
                .expect("encrypt input 1"),
            encrypt_identifier_from_seed(&public_parameters, &[3, 4], b"soracloud-fhe-add-input-2")
                .expect("encrypt input 2"),
            encrypt_identifier_from_seed(&public_parameters, &[5, 6], b"soracloud-fhe-add-input-3")
                .expect("encrypt input 3"),
        ];
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let job = sample_fhe_job(Vec::new());

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &inputs)
            .expect("execute three-input FHE add job");
        let plaintext_slots = output
            .slots
            .iter()
            .map(|slot| decrypt(&params, &secret_key, slot).expect("decrypt slot")[0])
            .collect::<Vec<_>>();

        assert_eq!(plaintext_slots[0], 6, "length slots add homomorphically");
        assert_eq!(
            plaintext_slots[1], 9,
            "first byte slots add homomorphically"
        );
        assert_eq!(
            plaintext_slots[2], 12,
            "second byte slots add homomorphically"
        );
        assert!(
            plaintext_slots[3..].iter().all(|slot| *slot == 0),
            "unused slots remain zero after three-input add: {plaintext_slots:?}"
        );
    }

    #[test]
    fn soracloud_multi_input_multiply_matches_plaintext_slots() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, _relinearization_key) =
            keygen_from_seed(&params, b"soracloud-fhe-test-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key,
            max_input_bytes: 8,
        };
        let inputs = [
            encrypt_identifier_from_seed(&public_parameters, &[2, 3], b"soracloud-fhe-mul-input-1")
                .expect("encrypt input 1"),
            encrypt_identifier_from_seed(&public_parameters, &[4, 5], b"soracloud-fhe-mul-input-2")
                .expect("encrypt input 2"),
            encrypt_identifier_from_seed(&public_parameters, &[6, 7], b"soracloud-fhe-mul-input-3")
                .expect("encrypt input 3"),
        ];
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Multiply;
        job.requested_multiplication_depth =
            bfv_balanced_multiplication_depth(inputs.len()).expect("three-input depth plan");

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &inputs)
            .expect("execute three-input FHE multiply job");
        let plaintext_slots = output
            .slots
            .iter()
            .map(|slot| decrypt(&params, &secret_key, slot).expect("decrypt slot")[0])
            .collect::<Vec<_>>();

        assert_eq!(
            plaintext_slots[0], 8,
            "length slots multiply through the relinerized fold"
        );
        assert_eq!(
            plaintext_slots[1], 48,
            "first byte slots multiply through the relinerized fold"
        );
        assert_eq!(
            plaintext_slots[2], 105,
            "second byte slots multiply through the relinerized fold"
        );
        assert!(
            plaintext_slots[3..].iter().all(|slot| *slot == 0),
            "unused slots remain zero after three-input multiply: {plaintext_slots:?}"
        );
    }

    #[test]
    fn soracloud_multi_input_multiply_rejects_underdeclared_depth() {
        let params = ram_lfe_bfv_parameters_v1();
        let inputs = [
            sample_fhe_envelope(b"\x02\x03", b"soracloud-fhe-depth-left"),
            sample_fhe_envelope(b"\x04\x05", b"soracloud-fhe-depth-middle"),
            sample_fhe_envelope(b"\x06\x07", b"soracloud-fhe-depth-right"),
        ];
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Multiply;
        job.requested_multiplication_depth = 1;

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &inputs)
            .expect_err("underdeclared multiply depth must fail before evaluation");
        assert_invalid_parameter_contains(err, "under-declares balanced BFV multiplication depth");
    }

    #[test]
    fn soracloud_fhe_job_residual_metadata_tracks_non_multiply_operations() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let input_bound = bfv_encrypted_zero_refresh_residual_multiple_bound(&params)
            .expect("fresh input residual bound");

        let lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-bound-left");
        let rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-bound-right");
        let add_job = sample_fhe_job(Vec::new());
        let (_output, add_bound) = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &add_job,
            &[lhs, rhs],
            &[input_bound, input_bound],
        )
        .expect("bounded add job");
        assert_eq!(
            add_bound,
            Some(
                bfv_add_output_residual_multiple_bound(&params, &[input_bound, input_bound])
                    .expect("add output bound")
            )
        );

        let rotate_input = sample_fhe_envelope(b"ab", b"soracloud-fhe-bound-rotate");
        let mut rotate_job = sample_fhe_job(Vec::new());
        rotate_job.operation = FheJobOperationV1::RotateLeft;
        rotate_job.rotation_steps = 1;
        let (_output, rotate_bound) = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &rotate_job,
            std::slice::from_ref(&rotate_input),
            &[input_bound],
        )
        .expect("bounded rotate job");
        let rotation_key = evaluation_keys
            .rotation_keys
            .iter()
            .find(|key| key.rotation_steps == 1)
            .expect("sample rotation key");
        let rotate_slot_bounds = vec![input_bound; rotate_input.slots.len()];
        let expected_rotate_bound = bfv_rotate_slots_left_output_residual_multiple_bounds(
            &params,
            rotation_key,
            &rotate_slot_bounds,
        )
        .expect("rotate output bounds")
        .into_iter()
        .max();
        assert_eq!(rotate_bound, expected_rotate_bound);

        let mut bootstrap_job = sample_fhe_job(Vec::new());
        bootstrap_job.operation = FheJobOperationV1::Bootstrap;
        bootstrap_job.bootstrap_count = 1;
        let bootstrap_input = sample_fhe_envelope(b"abc", b"soracloud-fhe-bound-bootstrap");
        let (_output, bootstrap_bound) = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &bootstrap_job,
            &[bootstrap_input],
            &[input_bound],
        )
        .expect("bounded bootstrap job");
        assert_eq!(
            bootstrap_bound,
            Some(
                bfv_bootstrap_key_refresh_output_residual_multiple_bound(
                    &params,
                    evaluation_keys
                        .bootstrap_key
                        .as_ref()
                        .expect("sample bootstrap key"),
                    input_bound,
                    1
                )
                .expect("bootstrap output bound")
            )
        );
    }

    #[test]
    fn soracloud_fhe_job_residual_metadata_rejects_over_capacity_add() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let capacity = u128::from(params.ciphertext_modulus / params.plaintext_modulus / 2);
        let lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-bound-cap-left");
        let rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-bound-cap-right");
        let job = sample_fhe_job(Vec::new());

        let err = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &job,
            &[lhs, rhs],
            &[capacity, 1],
        )
        .expect_err("add output above residual capacity must fail before evaluation");
        assert_invalid_parameter_contains(err, "FHE add residual bound exceeded");
    }

    #[test]
    fn soracloud_fhe_job_residual_metadata_rejects_bootstrap_count_above_key_capacity() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let input_bound = bfv_encrypted_zero_refresh_residual_multiple_bound(&params)
            .expect("fresh input residual bound");
        let input = sample_fhe_envelope(b"abc", b"soracloud-fhe-bound-bootstrap-capacity");
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = evaluation_keys
            .bootstrap_key
            .as_ref()
            .expect("sample bootstrap key")
            .max_refresh_rounds
            .saturating_add(1);

        let err = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &job,
            &[input],
            &[input_bound],
        )
        .expect_err("bootstrap residual admission must reject counts above key capacity");
        assert_invalid_parameter_contains(err, "max_refresh_rounds");
    }

    #[test]
    fn soracloud_fhe_job_residual_metadata_tracks_multiply_output() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let input_bound = bfv_encrypted_zero_refresh_residual_multiple_bound(&params)
            .expect("fresh input residual bound");
        let lhs = sample_fhe_envelope(b"\x02\x03", b"soracloud-fhe-bound-mul-left");
        let rhs = sample_fhe_envelope(b"\x04\x05", b"soracloud-fhe-bound-mul-right");
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Multiply;
        job.requested_multiplication_depth = 1;

        let (_output, output_bound) = execute_soracloud_fhe_job_with_residual_bounds(
            &params,
            &evaluation_keys,
            &job,
            &[lhs, rhs],
            &[input_bound, input_bound],
        )
        .expect("bounded multiply job");
        assert_eq!(
            output_bound,
            Some(
                bfv_multiply_output_residual_multiple_bound(
                    &params,
                    &evaluation_keys.relinearization_key,
                    input_bound,
                    input_bound,
                )
                .expect("multiply output bound")
            )
        );
    }

    #[test]
    fn soracloud_multi_input_fold_rejects_malformed_late_operand() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let lhs = sample_fhe_envelope(&[1, 2], b"soracloud-fhe-late-bad-left");
        let rhs = sample_fhe_envelope(&[3, 4], b"soracloud-fhe-late-bad-middle");
        let mut malicious = sample_fhe_envelope(&[5, 6], b"soracloud-fhe-late-bad-right");
        malicious.slots[1].c1[0] = params.ciphertext_modulus;
        let job = sample_fhe_job(Vec::new());

        let err =
            execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[lhs, rhs, malicious])
                .expect_err("malformed late operands must reject before output emission");
        assert_invalid_parameter_contains(err, "FHE add failed");
    }

    #[test]
    fn soracloud_bootstrap_uses_refresh_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-bootstrap-refresh-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes: 8,
        };
        let input =
            encrypt_identifier_from_seed(&public_parameters, b"abc", b"soracloud-bootstrap-input")
                .expect("encrypt input");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(
                bootstrap_key_from_seed(
                    &params,
                    &public_key,
                    "bootstrap-refresh-key",
                    b"soracloud-bootstrap-refresh-zero",
                )
                .expect("bootstrap key"),
            ),
        };
        let job = FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "bootstrap-job".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::Bootstrap,
            inputs: vec![sample_fhe_input_ref(
                "/state/private/input",
                &norito::to_bytes(&input).expect("encode input"),
            )],
            output_state_key: "/state/private/bootstrap-output".to_string(),
            requested_multiplication_depth: 0,
            rotation_steps: 0,
            bootstrap_count: 1,
        };

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input.clone()])
            .expect("execute bootstrap job");
        assert_ne!(output, input);
        let plaintext =
            decrypt_identifier(&public_parameters, &secret_key, &output).expect("decrypt output");
        assert_eq!(plaintext, b"abc");
    }

    #[test]
    fn soracloud_bootstrap_rejects_missing_refresh_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let mut evaluation_keys = sample_bfv_evaluation_key_bundle();
        evaluation_keys.bootstrap_key = None;
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = 1;
        let input = sample_fhe_envelope(b"abc", b"soracloud-bootstrap-missing-key");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("bootstrap must require a bootstrap refresh key");
        assert_invalid_parameter_contains(err, "missing BFV bootstrap key");
    }

    #[test]
    fn soracloud_bootstrap_rejects_refresh_count_above_key_capacity() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = evaluation_keys
            .bootstrap_key
            .as_ref()
            .expect("sample bootstrap key")
            .max_refresh_rounds
            .saturating_add(1);
        let input = sample_fhe_envelope(b"abc", b"soracloud-bootstrap-over-capacity");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("bootstrap must reject counts above the key capacity");
        assert_invalid_parameter_contains(err, "max_refresh_rounds");
    }

    #[test]
    fn soracloud_rotate_left_uses_rotation_key_refresh() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-rotate-refresh-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes: 4,
        };
        let input =
            encrypt_identifier_from_seed(&public_parameters, b"ab", b"soracloud-rotate-input")
                .expect("encrypt input");
        let mut plain_rotated = input.slots.clone();
        plain_rotated.rotate_left(1);
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![
                rotation_key_from_seed(&params, &public_key, 1, b"soracloud-rotate-refresh-zero")
                    .expect("rotation key"),
            ],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let job = FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "rotate-job".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::RotateLeft,
            inputs: vec![sample_fhe_input_ref(
                "/state/private/input",
                &norito::to_bytes(&input).expect("encode input"),
            )],
            output_state_key: "/state/private/rotate-output".to_string(),
            requested_multiplication_depth: 0,
            rotation_steps: 1,
            bootstrap_count: 0,
        };

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect("execute rotate job");
        assert_ne!(output.slots, plain_rotated);
        let plaintext_slots = output
            .slots
            .iter()
            .map(|slot| decrypt(&params, &secret_key, slot).expect("decrypt")[0])
            .collect::<Vec<_>>();
        assert_eq!(plaintext_slots, vec![97, 98, 0, 0, 2]);
    }

    #[test]
    fn soracloud_rotate_left_rejects_outer_slot_full_cycle_noop() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-rotate-full-cycle-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key: public_key.clone(),
            max_input_bytes: 4,
        };
        let input = encrypt_identifier_from_seed(
            &public_parameters,
            b"ab",
            b"soracloud-rotate-full-cycle-input",
        )
        .expect("encrypt input");
        let full_cycle_steps = u32::try_from(input.slots.len()).expect("slot count fits u32");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![
                rotation_key_from_seed(
                    &params,
                    &public_key,
                    full_cycle_steps,
                    b"soracloud-rotate-full-cycle-refresh",
                )
                .expect("rotation key"),
            ],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(
            "/state/private/input",
            &norito::to_bytes(&input).expect("encode input"),
        )]);
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = full_cycle_steps;

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("full-cycle outer-slot RotateLeft must fail before output emission");
        assert_invalid_parameter_contains(err, "full slot cycle");
    }

    #[test]
    fn soracloud_packed_rotate_left_uses_galois_key_switch() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-packed-rotate-keygen").expect("keygen");
        let half_rotation = u32::from(params.polynomial_degree) / 2;
        let degree = usize::from(params.polynomial_degree);
        let automorphism_power =
            packed_left_rotation_galois_automorphism_power(&params, half_rotation)
                .expect("fixture packed rotation must be representable");
        let input_slots = (0..degree)
            .map(|index| u64::try_from(index + 1).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        let packed_plaintext =
            encode_packed_plaintext_slots(&params, &input_slots).expect("encode packed slots");
        let packed_ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &packed_plaintext,
            b"soracloud-packed-rotate-input",
        )
        .expect("encrypt packed input");
        let input = BfvIdentifierCiphertext {
            slots: vec![packed_ciphertext],
        };
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: vec![
                galois_key_from_seed(
                    &params,
                    &secret_key,
                    automorphism_power,
                    b"soracloud-packed-rotate-galois-key",
                )
                .expect("Galois key"),
            ],
            bootstrap_key: None,
        };
        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(
            "/state/private/packed-input",
            &norito::to_bytes(&input).expect("encode packed input"),
        )]);
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = half_rotation;

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect("execute packed rotate job");
        assert_eq!(output.slots.len(), 1);
        let plaintext =
            decrypt(&params, &secret_key, &output.slots[0]).expect("decrypt packed output");
        let output_slots =
            decode_packed_plaintext_slots(&params, &plaintext).expect("decode packed output");
        let mut expected_slots = input_slots;
        expected_slots.rotate_left(usize::try_from(half_rotation).expect("rotation fits usize"));
        assert_eq!(output_slots, expected_slots);
    }

    #[test]
    fn soracloud_packed_rotate_left_supports_galois_mask_schedule() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-packed-rotate-schedule-keygen").expect("keygen");
        let degree = usize::from(params.polynomial_degree);
        let input_slots = (0..degree)
            .map(|index| u64::try_from(index + 1).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        let packed_plaintext =
            encode_packed_plaintext_slots(&params, &input_slots).expect("encode packed slots");
        let input = BfvIdentifierCiphertext {
            slots: vec![
                encrypt_from_seed(
                    &params,
                    &public_key,
                    &packed_plaintext,
                    b"soracloud-packed-rotate-schedule-input",
                )
                .expect("encrypt packed input"),
            ],
        };
        let powers = packed_left_rotation_galois_automorphism_powers(&params, 1)
            .expect("one-step packed rotation schedule");
        assert!(powers.len() > 1);
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: powers
                .into_iter()
                .map(|power| {
                    galois_key_from_seed(
                        &params,
                        &secret_key,
                        power,
                        b"soracloud-packed-rotate-schedule-galois-key",
                    )
                    .expect("Galois key")
                })
                .collect(),
            bootstrap_key: None,
        };
        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(
            "/state/private/packed-input",
            &norito::to_bytes(&input).expect("encode packed input"),
        )]);
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = 1;

        let output = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect("execute packed schedule rotate job");
        assert_eq!(output.slots.len(), 1);
        let plaintext =
            decrypt(&params, &secret_key, &output.slots[0]).expect("decrypt packed output");
        let output_slots =
            decode_packed_plaintext_slots(&params, &plaintext).expect("decode packed output");
        let mut expected_slots = input_slots;
        expected_slots.rotate_left(1);
        assert_eq!(output_slots, expected_slots);
    }

    #[test]
    fn soracloud_packed_rotate_left_rejects_missing_galois_key_without_outer_fallback() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"soracloud-packed-rotate-missing-keygen").expect("keygen");
        let half_rotation = u32::from(params.polynomial_degree) / 2;
        let degree = usize::from(params.polynomial_degree);
        let packed_plaintext =
            encode_packed_plaintext_slots(&params, &vec![0; degree]).expect("encode packed slots");
        let input = BfvIdentifierCiphertext {
            slots: vec![
                encrypt_from_seed(
                    &params,
                    &public_key,
                    &packed_plaintext,
                    b"soracloud-packed-rotate-missing-input",
                )
                .expect("encrypt packed input"),
            ],
        };
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![
                rotation_key_from_seed(
                    &params,
                    &public_key,
                    half_rotation,
                    b"soracloud-packed-rotate-outer-fallback-key",
                )
                .expect("outer rotation key"),
            ],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(
            "/state/private/packed-input",
            &norito::to_bytes(&input).expect("encode packed input"),
        )]);
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = half_rotation;

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("packed rotation must not fall back to outer-slot rotation keys");
        assert_invalid_parameter_contains(err, "missing BFV Galois key");
    }

    #[test]
    fn soracloud_rotate_left_rejects_missing_rotation_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = 2;
        let input = sample_fhe_envelope(b"ab", b"soracloud-rotate-missing-key");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("rotation must require a matching public rotation key");
        assert_invalid_parameter_contains(err, "missing BFV rotation key for 2 steps");
    }

    #[test]
    fn soracloud_fhe_job_rejects_mismatched_ciphertext_slot_counts() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-slot-count-left");
        let mut rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-slot-count-right");
        rhs.slots.pop().expect("sample envelope has slots");
        let job = sample_fhe_job(Vec::new());

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[lhs, rhs])
            .expect_err("FHE jobs must reject incompatible ciphertext envelopes");
        assert_invalid_parameter_contains(err, "matching slot counts");
    }

    #[test]
    fn soracloud_fhe_job_rejects_empty_ciphertext_envelope() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let job = sample_fhe_job(Vec::new());
        let input = BfvIdentifierCiphertext { slots: Vec::new() };

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("empty FHE ciphertext envelopes must be rejected");
        assert_invalid_parameter_contains(err, "at least one slot");
    }

    #[test]
    fn soracloud_fhe_job_rejects_missing_input_envelopes() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let job = sample_fhe_job(Vec::new());

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[])
            .expect_err("FHE jobs must reject missing input envelopes");
        assert_invalid_parameter_contains(err, "at least one input envelope");
    }

    #[test]
    fn soracloud_fhe_job_rejects_operation_shape_bypasses_before_evaluation() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-shape-left");
        let rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-shape-right");

        let add_job = sample_fhe_job(Vec::new());
        let err = execute_soracloud_fhe_job(
            &params,
            &evaluation_keys,
            &add_job,
            std::slice::from_ref(&lhs),
        )
        .expect_err("single-input add plans must fail before evaluation");
        assert_invalid_parameter_contains(err, "invalid FHE add plan");

        let mut rotate_job = sample_fhe_job(Vec::new());
        rotate_job.operation = FheJobOperationV1::RotateLeft;
        rotate_job.rotation_steps = 1;
        let err = execute_soracloud_fhe_job(
            &params,
            &evaluation_keys,
            &rotate_job,
            &[lhs.clone(), rhs.clone()],
        )
        .expect_err("multi-input rotate plans must fail before evaluation");
        assert_invalid_parameter_contains(err, "invalid FHE rotate plan");

        let mut multi_input_bootstrap_job = sample_fhe_job(Vec::new());
        multi_input_bootstrap_job.operation = FheJobOperationV1::Bootstrap;
        multi_input_bootstrap_job.bootstrap_count = 1;
        let err = execute_soracloud_fhe_job(
            &params,
            &evaluation_keys,
            &multi_input_bootstrap_job,
            &[lhs.clone(), rhs],
        )
        .expect_err("multi-input bootstrap plans must fail before evaluation");
        assert_invalid_parameter_contains(err, "invalid FHE bootstrap plan");

        let mut bootstrap_job = sample_fhe_job(Vec::new());
        bootstrap_job.operation = FheJobOperationV1::Bootstrap;
        bootstrap_job.bootstrap_count = 0;
        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &bootstrap_job, &[lhs])
            .expect_err("zero-round bootstrap plans must fail before evaluation");
        assert_invalid_parameter_contains(err, "invalid FHE bootstrap plan");
    }

    #[test]
    fn soracloud_fhe_job_rejects_malformed_ciphertext_slot_coefficients() {
        let params = ram_lfe_bfv_parameters_v1();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let mut lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-malformed-left");
        let rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-malformed-right");
        lhs.slots[0].c0[0] = params.ciphertext_modulus;
        let job = sample_fhe_job(Vec::new());

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[lhs, rhs])
            .expect_err("malformed ciphertext coefficients must be rejected");
        assert_invalid_parameter_contains(err, "FHE add failed");
    }

    #[test]
    fn soracloud_multiply_rejects_malformed_relinearization_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let mut evaluation_keys = sample_bfv_evaluation_key_bundle();
        evaluation_keys
            .relinearization_key
            .entries
            .pop()
            .expect("sample relin key has entries");
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Multiply;
        job.requested_multiplication_depth = 1;
        let lhs = sample_fhe_envelope(b"alice", b"soracloud-fhe-bad-relin-left");
        let rhs = sample_fhe_envelope(b"bob", b"soracloud-fhe-bad-relin-right");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[lhs, rhs])
            .expect_err("malformed relinearization keys must be rejected");
        assert_invalid_parameter_contains(err, "FHE multiply failed");
    }

    #[test]
    fn soracloud_rotate_left_rejects_malformed_rotation_refresh_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let mut evaluation_keys = sample_bfv_evaluation_key_bundle();
        evaluation_keys.rotation_keys[0].zero_refresh.c1[0] = params.ciphertext_modulus;
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::RotateLeft;
        job.rotation_steps = 1;
        let input = sample_fhe_envelope(b"ab", b"soracloud-rotate-malformed-key");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("malformed rotation refresh keys must be rejected");
        assert_invalid_parameter_contains(err, "FHE rotate failed");
    }

    #[test]
    fn soracloud_bootstrap_rejects_malformed_refresh_key() {
        let params = ram_lfe_bfv_parameters_v1();
        let mut evaluation_keys = sample_bfv_evaluation_key_bundle();
        evaluation_keys
            .bootstrap_key
            .as_mut()
            .expect("sample bundle has bootstrap key")
            .zero_refresh
            .c0
            .pop();
        let mut job = sample_fhe_job(Vec::new());
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = 1;
        let input = sample_fhe_envelope(b"abc", b"soracloud-bootstrap-malformed-key");

        let err = execute_soracloud_fhe_job(&params, &evaluation_keys, &job, &[input])
            .expect_err("malformed bootstrap refresh keys must be rejected");
        assert_invalid_parameter_contains(err, "FHE bootstrap failed");
    }

    #[test]
    fn soracloud_registered_bfv_parameters_reject_digest_mismatch() {
        let mut param_set = sample_fhe_param_set();
        param_set.parameter_digest = Hash::new(b"tampered-bfv-parameters");

        let err = registered_soracloud_bfv_parameters(&param_set)
            .expect_err("tampered parameter-set digest must be rejected");
        assert_invalid_parameter_contains(err, "registered BFV profile");
    }

    #[test]
    fn soracloud_registered_bfv_parameters_accept_shared_governance_fixture() {
        let bundle = shared_fhe_governance_bundle_fixture();
        bundle
            .validate_for_admission()
            .expect("shared FHE governance fixture must validate");

        let params = registered_soracloud_bfv_parameters(&bundle.param_set)
            .expect("shared FHE governance fixture must match the registered runtime profile");

        assert_eq!(params, ram_lfe_bfv_parameters_v1());
    }

    #[test]
    fn soracloud_registered_bfv_parameters_reject_descriptor_drift() {
        let bundle = shared_fhe_governance_bundle_fixture();

        let mut wrong_backend = bundle.param_set.clone();
        wrong_backend.backend = "fhe/bfv-rns/v2".to_string();
        let err = registered_soracloud_bfv_parameters(&wrong_backend)
            .expect_err("wrong backend must be rejected");
        assert_invalid_parameter_contains(err, "backend");

        let mut wrong_degree = bundle.param_set.clone();
        wrong_degree.polynomial_modulus_degree = NonZeroU32::new(8_192).expect("nonzero");
        let err = registered_soracloud_bfv_parameters(&wrong_degree)
            .expect_err("wrong polynomial degree must be rejected");
        assert_invalid_parameter_contains(err, "polynomial degree");

        let mut wrong_plaintext = bundle.param_set.clone();
        wrong_plaintext.plaintext_modulus_bits = NonZeroU16::new(20).expect("nonzero");
        let err = registered_soracloud_bfv_parameters(&wrong_plaintext)
            .expect_err("wrong plaintext modulus width must be rejected");
        assert_invalid_parameter_contains(err, "plaintext modulus bits");

        let mut wrong_rns_digest = bundle.param_set.clone();
        wrong_rns_digest.rns_modulus_chain_digest = Hash::new(b"wrong-bfv-rns-chain");
        let err = registered_soracloud_bfv_parameters(&wrong_rns_digest)
            .expect_err("wrong RNS modulus-chain digest must be rejected");
        assert_invalid_parameter_contains(err, "RNS modulus-chain digest");

        let mut excessive_ciphertext_limb = bundle.param_set.clone();
        excessive_ciphertext_limb.ciphertext_modulus_bits = vec![
            NonZeroU16::new(60).expect("nonzero"),
            NonZeroU16::new(50).expect("nonzero"),
        ];
        let err = registered_soracloud_bfv_parameters(&excessive_ciphertext_limb)
            .expect_err("oversized ciphertext modulus limb must be rejected");
        assert_invalid_parameter_contains(err, "ciphertext modulus bits");

        let mut underdeclared_ciphertext_chain = bundle.param_set;
        underdeclared_ciphertext_chain.ciphertext_modulus_bits = vec![
            NonZeroU16::new(30).expect("nonzero"),
            NonZeroU16::new(26).expect("nonzero"),
        ];
        let err = registered_soracloud_bfv_parameters(&underdeclared_ciphertext_chain)
            .expect_err("under-declared ciphertext modulus chain must be rejected");
        assert_invalid_parameter_contains(err, "under-declares");
    }

    fn fhe_job_provenance(
        service_name: &iroha_data_model::name::Name,
        binding_name: &iroha_data_model::name::Name,
        job: FheJobSpecV1,
        policy: FheExecutionPolicyV1,
        param_set: FheParamSetV1,
        evaluation_keys: BfvEvaluationKeyBundle,
        evaluation_key_refresh_transcript: BfvEvaluationKeyRefreshTranscriptV1,
        governance_tx_hash: Hash,
    ) -> ManifestProvenance {
        let payload = encode_fhe_job_run_provenance_payload(
            service_name.as_ref(),
            binding_name.as_ref(),
            job,
            policy,
            param_set,
            evaluation_keys,
            evaluation_key_refresh_transcript,
            governance_tx_hash,
        )
        .expect("fhe job payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_decryption_policy() -> DecryptionAuthorityPolicyV1 {
        DecryptionAuthorityPolicyV1 {
            schema_version: iroha_data_model::soracloud::DECRYPTION_AUTHORITY_POLICY_VERSION_V1,
            policy_name: "phi_policy".parse().expect("valid name"),
            mode: DecryptionAuthorityModeV1::ThresholdService,
            approver_quorum: NonZeroU16::new(2).expect("nonzero"),
            approver_ids: vec![
                "approver-a".parse().expect("valid name"),
                "approver-b".parse().expect("valid name"),
            ],
            allow_break_glass: true,
            jurisdiction_tag: "us_hipaa".to_string(),
            require_consent_evidence: false,
            max_ttl_blocks: NonZeroU32::new(128).expect("nonzero"),
            audit_tag: "phi.access".to_string(),
        }
    }

    fn sample_decryption_request() -> DecryptionRequestV1 {
        DecryptionRequestV1 {
            schema_version: iroha_data_model::soracloud::DECRYPTION_REQUEST_VERSION_V1,
            request_id: "decrypt-1".to_string(),
            policy_name: "phi_policy".parse().expect("valid name"),
            binding_name: "vault".parse().expect("valid name"),
            state_key: "/state/private/patient-1".to_string(),
            ciphertext_commitment: Hash::new(b"ciphertext"),
            justification: "care review".to_string(),
            jurisdiction_tag: "us_hipaa".to_string(),
            consent_evidence_hash: None,
            requested_ttl_blocks: NonZeroU32::new(64).expect("nonzero"),
            break_glass: false,
            break_glass_reason: None,
            governance_tx_hash: Hash::new(b"gov"),
        }
    }

    fn sample_service_secret_envelope() -> SecretEnvelopeV1 {
        SecretEnvelopeV1 {
            schema_version: SECRET_ENVELOPE_VERSION_V1,
            encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
            key_id: "kms://soracloud/db".to_string(),
            key_version: NonZeroU32::new(1).expect("nonzero"),
            nonce: vec![1, 2, 3, 4],
            ciphertext: vec![5, 6, 7, 8],
            commitment: Hash::new(b"soracloud-secret"),
            aad_digest: None,
        }
    }

    fn decryption_request_provenance(
        service_name: &iroha_data_model::name::Name,
        policy: DecryptionAuthorityPolicyV1,
        request: DecryptionRequestV1,
    ) -> ManifestProvenance {
        let payload =
            encode_decryption_request_provenance_payload(service_name.as_ref(), policy, request)
                .expect("decryption request payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_training_bundle(service_name: &str, service_version: &str) -> SoraDeploymentBundleV1 {
        let mut bundle = sample_bundle(service_name, service_version, 0);
        bundle.container.capabilities.allow_model_training = true;
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        bundle
    }

    fn sample_uploaded_model_bundle(
        service_name: &str,
        digest: ManifestDigest,
    ) -> SoraUploadedModelBundleV1 {
        SoraUploadedModelBundleV1 {
            schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
            service_name: service_name.parse().expect("valid service name"),
            model_id: "vision_model".to_string(),
            weight_version: "v1".to_string(),
            family: "decoder-only".to_string(),
            modalities: vec!["text".to_string()],
            plaintext_root: Hash::new(b"plaintext-root"),
            runtime_format: iroha_data_model::soracloud::SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors,
            bundle_root: Hash::new(b"bundle-root"),
            sorafs_manifest_digest: digest,
            chunk_count: 1,
            plaintext_bytes: 4_096,
            ciphertext_bytes: 4_352,
            chunk_manifest_root: Hash::new(b"chunk-manifest-root"),
            upload_recipient: iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                key_id: "soracloud-upload".to_string(),
                key_version: NonZeroU32::new(1).expect("non-zero key version"),
                kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                public_key_bytes: vec![3u8; 32],
                public_key_fingerprint: Hash::new([3u8; 32]),
            },
            wrapped_bundle_key: iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                recipient_key_id: "soracloud-upload".to_string(),
                recipient_key_version: NonZeroU32::new(1).expect("non-zero key version"),
                kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                ephemeral_public_key: vec![4u8; 32],
                nonce: vec![5u8; 12],
                wrapped_key_ciphertext: vec![6u8; 48],
                ciphertext_hash: Hash::new([6u8; 48]),
                aad_digest: Hash::new(b"wrapped-aad"),
            },
            pricing_policy: iroha_data_model::soracloud::SoraUploadedModelPricingPolicyV1 {
                storage_xor_nanos: 0,
            },
            decryption_policy_ref: "policy/private-release".to_string(),
        }
    }

    fn uploaded_model_bundle_provenance(bundle: &SoraUploadedModelBundleV1) -> ManifestProvenance {
        uploaded_model_bundle_provenance_for(bundle, &ALICE_KEYPAIR)
    }

    fn uploaded_model_bundle_provenance_for(
        bundle: &SoraUploadedModelBundleV1,
        key_pair: &KeyPair,
    ) -> ManifestProvenance {
        let payload = encode_uploaded_model_bundle_register_provenance_payload(bundle.clone())
            .expect("uploaded model bundle payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: iroha_crypto::Signature::new(key_pair.private_key(), &payload),
        }
    }

    fn uploaded_model_finalize_provenance(
        service_name: &iroha_data_model::name::Name,
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
    ) -> ManifestProvenance {
        uploaded_model_finalize_provenance_for(
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
            &ALICE_KEYPAIR,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn uploaded_model_finalize_provenance_for(
        service_name: &iroha_data_model::name::Name,
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
        key_pair: &KeyPair,
    ) -> ManifestProvenance {
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
        .expect("uploaded model finalize payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: iroha_crypto::Signature::new(key_pair.private_key(), &payload),
        }
    }

    fn insert_uploaded_model_pin(
        state_transaction: &mut StateTransaction<'_, '_>,
        digest: ManifestDigest,
        status: PinStatus,
    ) {
        insert_uploaded_model_pin_record(state_transaction, digest, digest, 4_352, status);
    }

    fn insert_uploaded_model_pin_with_content_length(
        state_transaction: &mut StateTransaction<'_, '_>,
        digest: ManifestDigest,
        content_length: u64,
        status: PinStatus,
    ) {
        insert_uploaded_model_pin_record(state_transaction, digest, digest, content_length, status);
    }

    fn insert_uploaded_model_pin_record(
        state_transaction: &mut StateTransaction<'_, '_>,
        storage_key_digest: ManifestDigest,
        record_digest: ManifestDigest,
        content_length: u64,
        status: PinStatus,
    ) {
        let chunker = iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_string(),
            name: "sf1".to_string(),
            semver: "1.0.0".to_string(),
            multihash_code: 0x1e,
        };
        let policy = iroha_data_model::sorafs::pin_registry::PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Warm,
            retention_epoch: u64::MAX,
        };
        let mut record = iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
            record_digest,
            chunker,
            [0xA7; 32],
            policy,
            ALICE_ID.clone(),
            1,
            None,
            None,
            Metadata::default(),
        )
        .with_content_length(content_length);
        match status {
            PinStatus::Pending => {}
            PinStatus::Approved(epoch) => {
                let amount_nano = state_transaction
                    .world
                    .sorafs_pricing
                    .get()
                    .public_pin_fee_nano(
                        policy.storage_class,
                        content_length,
                        policy.min_replicas,
                        1,
                        policy.retention_epoch,
                    );
                record.record_pin_fee_payment(
                    iroha_data_model::sorafs::pin_registry::PinFeePayment {
                        paid_by: ALICE_ID.clone(),
                        fee_asset_id: state_transaction.gov.sorafs_pin_fee_asset_id.clone(),
                        treasury_account_id: state_transaction
                            .gov
                            .sorafs_pin_fee_treasury_account
                            .clone(),
                        amount_nano,
                    },
                );
                record.approve(epoch, None);
            }
            PinStatus::Retired(epoch) => record.retire(epoch, None),
        }
        state_transaction
            .world
            .pin_manifests
            .insert(storage_key_digest, record);
    }

    fn deploy_uploaded_model_service(
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let service_bundle = sample_training_bundle("portal", "1.0.0");
        isi::DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&service_bundle),
        }
        .execute(&ALICE_ID, state_transaction)
    }

    fn sample_uploaded_model_finalize_instruction(
        bundle: &SoraUploadedModelBundleV1,
        artifact_id: &str,
        bundle_root: Hash,
    ) -> isi::FinalizeSoracloudUploadedModelBundle {
        let service_name = bundle.service_name.clone();
        let weight_artifact_hash = Hash::new(b"weights");
        let training_config_hash = Hash::new(b"training-config");
        let reproducibility_hash = Hash::new(b"reproducibility");
        let provenance_attestation_hash = Hash::new(b"provenance-attestation");
        isi::FinalizeSoracloudUploadedModelBundle {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            model_id: bundle.model_id.clone(),
            artifact_id: artifact_id.to_string(),
            weight_version: bundle.weight_version.clone(),
            bundle_root,
            weight_artifact_hash,
            dataset_ref: "dataset://upload".to_string(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: uploaded_model_finalize_provenance(
                &service_name,
                "vision_model",
                &bundle.model_id,
                artifact_id,
                &bundle.weight_version,
                bundle_root,
                weight_artifact_hash,
                "dataset://upload",
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            ),
        }
    }

    fn sample_agent_manifest_with_capabilities(
        apartment_name: &str,
        extra_capabilities: &[&str],
    ) -> AgentApartmentManifestV1 {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/agent_apartment_manifest_v1.json");
        let fixture = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        let mut manifest: AgentApartmentManifestV1 =
            norito::json::from_str(&fixture).expect("agent manifest fixture should decode");
        manifest.apartment_name = apartment_name.parse().expect("valid apartment name");
        for capability in extra_capabilities {
            let capability = capability.parse().expect("valid capability");
            if !manifest.policy_capabilities.contains(&capability) {
                manifest.policy_capabilities.push(capability);
            }
        }
        manifest.validate().expect("agent manifest should validate");
        manifest
    }

    fn agent_deploy_provenance(
        manifest: AgentApartmentManifestV1,
        lease_ticks: u64,
        autonomy_budget_units: u64,
    ) -> ManifestProvenance {
        let payload = encode_agent_deploy_provenance_payload(
            manifest,
            lease_ticks,
            Some(autonomy_budget_units),
        )
        .expect("agent deploy payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn training_start_provenance(
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
    ) -> ManifestProvenance {
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
        .expect("training start payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn training_checkpoint_provenance(
        service_name: &iroha_data_model::name::Name,
        job_id: &str,
        completed_step: u32,
        checkpoint_size_bytes: u64,
        metrics_hash: Hash,
    ) -> ManifestProvenance {
        let payload = encode_training_job_checkpoint_provenance_payload(
            service_name.as_ref(),
            job_id,
            completed_step,
            checkpoint_size_bytes,
            metrics_hash,
        )
        .expect("training checkpoint payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn training_retry_provenance(
        service_name: &iroha_data_model::name::Name,
        job_id: &str,
        reason: &str,
    ) -> ManifestProvenance {
        let payload =
            encode_training_job_retry_provenance_payload(service_name.as_ref(), job_id, reason)
                .expect("training retry payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn model_artifact_provenance(
        service_name: &iroha_data_model::name::Name,
        model_name: &str,
        training_job_id: &str,
        weight_artifact_hash: Hash,
        dataset_ref: &str,
        training_config_hash: Hash,
        reproducibility_hash: Hash,
        provenance_attestation_hash: Hash,
    ) -> ManifestProvenance {
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
        .expect("model artifact payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn model_weight_register_provenance(
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
    ) -> ManifestProvenance {
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
        .expect("model weight register payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn model_weight_promote_provenance(
        service_name: &iroha_data_model::name::Name,
        model_name: &str,
        weight_version: &str,
        gate_approved: bool,
        gate_report_hash: Hash,
    ) -> ManifestProvenance {
        let payload = encode_model_weight_promote_provenance_payload(
            service_name.as_ref(),
            model_name,
            weight_version,
            gate_approved,
            gate_report_hash,
        )
        .expect("model weight promote payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn model_weight_rollback_provenance(
        service_name: &iroha_data_model::name::Name,
        model_name: &str,
        target_version: &str,
        reason: &str,
    ) -> ManifestProvenance {
        let payload = encode_model_weight_rollback_provenance_payload(
            service_name.as_ref(),
            model_name,
            target_version,
            reason,
        )
        .expect("model weight rollback payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn hf_shared_lease_join_provenance(
        repo_id: &str,
        resolved_revision: &str,
        model_name: &str,
        service_name: &iroha_data_model::name::Name,
        apartment_name: Option<&iroha_data_model::name::Name>,
        storage_class: StorageClass,
        lease_term_ms: u64,
        lease_asset_definition_id: &AssetDefinitionId,
        base_fee_nanos: u128,
    ) -> ManifestProvenance {
        hf_shared_lease_join_provenance_for(
            &ALICE_KEYPAIR,
            repo_id,
            resolved_revision,
            model_name,
            service_name,
            apartment_name,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee_nanos,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn hf_shared_lease_join_provenance_for(
        key_pair: &KeyPair,
        repo_id: &str,
        resolved_revision: &str,
        model_name: &str,
        service_name: &iroha_data_model::name::Name,
        apartment_name: Option<&iroha_data_model::name::Name>,
        storage_class: StorageClass,
        lease_term_ms: u64,
        lease_asset_definition_id: &AssetDefinitionId,
        base_fee_nanos: u128,
    ) -> ManifestProvenance {
        let payload = encode_hf_shared_lease_join_provenance_payload(
            repo_id,
            resolved_revision,
            model_name,
            service_name.as_ref(),
            apartment_name.map(iroha_data_model::name::Name::as_ref),
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee_nanos,
        )
        .expect("hf shared lease join payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: iroha_crypto::Signature::new(key_pair.private_key(), &payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn hf_shared_lease_renew_provenance(
        repo_id: &str,
        resolved_revision: &str,
        model_name: &str,
        service_name: &iroha_data_model::name::Name,
        apartment_name: Option<&iroha_data_model::name::Name>,
        storage_class: StorageClass,
        lease_term_ms: u64,
        lease_asset_definition_id: &AssetDefinitionId,
        base_fee_nanos: u128,
    ) -> ManifestProvenance {
        let payload = encode_hf_shared_lease_renew_provenance_payload(
            repo_id,
            resolved_revision,
            model_name,
            service_name.as_ref(),
            apartment_name.map(iroha_data_model::name::Name::as_ref),
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee_nanos,
        )
        .expect("hf shared lease renew payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn hf_shared_lease_leave_provenance(
        repo_id: &str,
        resolved_revision: &str,
        storage_class: StorageClass,
        lease_term_ms: u64,
        service_name: Option<&iroha_data_model::name::Name>,
        apartment_name: Option<&iroha_data_model::name::Name>,
    ) -> ManifestProvenance {
        let payload = encode_hf_shared_lease_leave_provenance_payload(
            repo_id,
            resolved_revision,
            storage_class,
            lease_term_ms,
            service_name.map(iroha_data_model::name::Name::as_ref),
            apartment_name.map(iroha_data_model::name::Name::as_ref),
        )
        .expect("hf shared lease leave payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_hf_resource_profile() -> SoraHfResourceProfileV1 {
        SoraHfResourceProfileV1 {
            required_model_bytes: 1024 * 1024 * 1024,
            backend_family: SoraHfBackendFamilyV1::Transformers,
            model_format: SoraHfModelFormatV1::Safetensors,
            disk_cache_bytes_floor: 1024 * 1024 * 1024,
            ram_bytes_floor: 2 * 1024 * 1024 * 1024,
            vram_bytes_floor: 0,
        }
    }

    fn sample_model_host_capability(
        validator_account_id: AccountId,
        advertised_at_ms: u64,
        heartbeat_expires_at_ms: u64,
    ) -> SoraModelHostCapabilityRecordV1 {
        SoraModelHostCapabilityRecordV1 {
            schema_version: SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
            validator_account_id,
            peer_id: "12D3KooWCoreTestPeer".to_string(),
            supported_backends: std::collections::BTreeSet::from([
                SoraHfBackendFamilyV1::Transformers,
            ]),
            supported_formats: std::collections::BTreeSet::from([SoraHfModelFormatV1::Safetensors]),
            max_model_bytes: 8 * 1024 * 1024 * 1024,
            max_disk_cache_bytes: 32 * 1024 * 1024 * 1024,
            max_ram_bytes: 32 * 1024 * 1024 * 1024,
            max_vram_bytes: 0,
            max_concurrent_resident_models: 2,
            host_class: "cpu.large".to_string(),
            advertised_at_ms,
            heartbeat_expires_at_ms,
        }
    }

    fn model_host_advertise_provenance(
        capability: &SoraModelHostCapabilityRecordV1,
    ) -> ManifestProvenance {
        model_host_advertise_provenance_for(&ALICE_KEYPAIR, capability)
    }

    fn model_host_advertise_provenance_for(
        key_pair: &KeyPair,
        capability: &SoraModelHostCapabilityRecordV1,
    ) -> ManifestProvenance {
        let payload = encode_model_host_advertise_provenance_payload(capability)
            .expect("model host advertise payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: iroha_crypto::Signature::new(key_pair.private_key(), &payload),
        }
    }

    fn model_host_heartbeat_provenance(
        validator_account_id: &AccountId,
        heartbeat_expires_at_ms: u64,
    ) -> ManifestProvenance {
        let payload = encode_model_host_heartbeat_provenance_payload(
            validator_account_id,
            heartbeat_expires_at_ms,
        )
        .expect("model host heartbeat payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn model_host_withdraw_provenance(validator_account_id: &AccountId) -> ManifestProvenance {
        let payload = encode_model_host_withdraw_provenance_payload(validator_account_id)
            .expect("model host withdraw payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    #[test]
    fn next_soracloud_audit_sequence_includes_hf_shared_lease_events() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        stx.world.soracloud_hf_shared_lease_audit_events.insert(
            9,
            SoraHfSharedLeaseAuditEventV1 {
                schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                sequence: 9,
                action: SoraHfSharedLeaseActionV1::CreateWindow,
                pool_id: Hash::new(b"hf-pool"),
                source_id: Hash::new(b"hf-source"),
                account_id: ALICE_ID.clone(),
                occurred_at_ms: 10,
                active_member_count: 1,
                charged_nanos: 10_000,
                refunded_nanos: 0,
                lease_expires_at_ms: 20,
                service_name: Some("vision_portal".to_owned()),
                apartment_name: Some("ops_agent".to_owned()),
            },
        );

        assert_eq!(next_soracloud_audit_sequence(&stx), 10);
        Ok(())
    }

    #[test]
    fn model_host_advertise_and_withdraw_updates_authoritative_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;

        let advertise_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut advertise_block = state.block(advertise_header);
        let mut advertise_tx = advertise_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 10, 110);
        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut advertise_tx)?;
        advertise_tx.apply();
        advertise_block.commit()?;

        let view = state.view();
        let advertised = view
            .world()
            .soracloud_model_host_capabilities()
            .get(&ALICE_ID)
            .expect("advertised capability");
        assert_eq!(advertised.peer_id, capability.peer_id);

        let withdraw_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut withdraw_block = state.block(withdraw_header);
        let mut withdraw_tx = withdraw_block.transaction();
        isi::WithdrawSoracloudModelHost {
            validator_account_id: ALICE_ID.clone(),
            provenance: model_host_withdraw_provenance(&ALICE_ID),
        }
        .execute(&ALICE_ID, &mut withdraw_tx)?;
        withdraw_tx.apply();
        withdraw_block.commit()?;

        let view = state.view();
        assert!(
            view.world()
                .soracloud_model_host_capabilities()
                .get(&ALICE_ID)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn model_host_heartbeat_marks_assigned_placement_warm() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;

        let advertise_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut advertise_block = state.block(advertise_header);
        let mut advertise_tx = advertise_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 10, 110);
        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut advertise_tx)?;
        let placement = SoraHfPlacementRecordV1 {
            schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            placement_id: Hash::new(b"placement"),
            source_id: Hash::new(b"source"),
            pool_id: Hash::new(b"pool"),
            status: SoraHfPlacementStatusV1::Warming,
            selection_seed_hash: Hash::new(b"seed"),
            resource_profile: SoraHfResourceProfileV1 {
                required_model_bytes: 1_024,
                backend_family: SoraHfBackendFamilyV1::Transformers,
                model_format: SoraHfModelFormatV1::Safetensors,
                disk_cache_bytes_floor: 2_048,
                ram_bytes_floor: 2_048,
                vram_bytes_floor: 0,
            },
            eligible_validator_count: 1,
            adaptive_target_host_count: 1,
            assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                validator_account_id: ALICE_ID.clone(),
                peer_id: capability.peer_id.clone(),
                role: SoraHfPlacementHostRoleV1::Primary,
                status: SoraHfPlacementHostStatusV1::Warming,
                host_class: capability.host_class.clone(),
            }],
            total_reservation_fee_nanos: 1_000,
            last_rebalance_at_ms: 10,
            last_error: None,
        };
        record_hf_placement(&mut advertise_tx, placement)?;
        advertise_tx.apply();
        advertise_block.commit()?;

        let heartbeat_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut heartbeat_block = state.block(heartbeat_header);
        let mut heartbeat_tx = heartbeat_block.transaction();
        isi::HeartbeatSoracloudModelHost {
            validator_account_id: ALICE_ID.clone(),
            heartbeat_expires_at_ms: 510,
            provenance: model_host_heartbeat_provenance(&ALICE_ID, 510),
        }
        .execute(&ALICE_ID, &mut heartbeat_tx)?;
        heartbeat_tx.apply();
        heartbeat_block.commit()?;

        let view = state.view();
        let placement = view
            .world()
            .soracloud_hf_placements()
            .get(&Hash::new(b"pool"))
            .expect("updated placement");
        assert_eq!(placement.status, SoraHfPlacementStatusV1::Ready);
        assert_eq!(
            placement.assigned_hosts[0].status,
            SoraHfPlacementHostStatusV1::Warm
        );
        Ok(())
    }

    #[test]
    fn model_host_readvertise_updates_assigned_placement_metadata() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let pool_id = Hash::new(b"placement-pool");

        let initial_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut initial_block = state.block(initial_header);
        let mut initial_tx = initial_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 100, 1_000);
        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut initial_tx)?;
        record_hf_shared_lease_pool(
            &mut initial_tx,
            sample_hf_shared_lease_pool_record(pool_id, Hash::new(b"metadata-source"), 100),
        )?;
        record_hf_placement(
            &mut initial_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: Hash::new(b"metadata-placement"),
                source_id: Hash::new(b"metadata-source"),
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"metadata-seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: ALICE_ID.clone(),
                    peer_id: capability.peer_id.clone(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: capability.host_class.clone(),
                }],
                total_reservation_fee_nanos: 1_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        initial_tx.apply();
        initial_block.commit()?;

        let updated_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 200;
            })
            .as_ref()
            .header();
        let mut updated_block = state.block(updated_header);
        let mut updated_tx = updated_block.transaction();
        let mut updated_capability = capability.clone();
        updated_capability.advertised_at_ms = 200;
        updated_capability.heartbeat_expires_at_ms = 1_200;
        updated_capability.peer_id = "12D3KooWUpdatedMetadataPeer".to_string();
        updated_capability.host_class = "cpu.small".to_string();
        isi::AdvertiseSoracloudModelHost {
            capability: updated_capability.clone(),
            provenance: model_host_advertise_provenance(&updated_capability),
        }
        .execute(&ALICE_ID, &mut updated_tx)?;
        updated_tx.apply();
        updated_block.commit()?;

        let view = state.view();
        let advertised = view
            .world()
            .soracloud_model_host_capabilities()
            .get(&ALICE_ID)
            .expect("updated capability");
        assert_eq!(advertised.peer_id, updated_capability.peer_id);
        assert_eq!(advertised.host_class, updated_capability.host_class);
        let placement = view
            .world()
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("updated placement");
        assert_eq!(placement.last_rebalance_at_ms, 200);
        assert_eq!(placement.total_reservation_fee_nanos, 500);
        assert_eq!(placement.assigned_hosts.len(), 1);
        assert_eq!(
            placement.assigned_hosts[0].peer_id,
            updated_capability.peer_id
        );
        assert_eq!(
            placement.assigned_hosts[0].host_class,
            updated_capability.host_class
        );
        Ok(())
    }

    #[test]
    fn reconcile_soracloud_model_hosts_promotes_warm_replica_after_primary_expiry()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let charlie_id = AccountId::new(KeyPair::random().public_key().clone());
        let pool_id = Hash::new(b"pool");

        let initial_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut initial_block = state.block(initial_header);
        let mut initial_tx = initial_block.transaction();
        insert_active_public_lane_validator(&mut initial_tx, BOB_ID.clone(), 900);
        insert_active_public_lane_validator(&mut initial_tx, charlie_id.clone(), 800);

        let alice_capability = sample_model_host_capability(ALICE_ID.clone(), 10, 110);
        let mut bob_capability = sample_model_host_capability(BOB_ID.clone(), 10, 500);
        bob_capability.peer_id = "12D3KooWBobHostTestPeer".to_string();
        let mut charlie_capability = sample_model_host_capability(charlie_id.clone(), 10, 500);
        charlie_capability.peer_id = "12D3KooWCharlieHostTestPeer".to_string();
        record_model_host_capability(&mut initial_tx, alice_capability.clone())?;
        record_model_host_capability(&mut initial_tx, bob_capability.clone())?;
        record_model_host_capability(&mut initial_tx, charlie_capability.clone())?;
        record_hf_shared_lease_pool(
            &mut initial_tx,
            sample_hf_shared_lease_pool_record(pool_id, Hash::new(b"source"), 100),
        )?;

        record_hf_placement(
            &mut initial_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: Hash::new(b"placement"),
                source_id: Hash::new(b"source"),
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 3,
                adaptive_target_host_count: 2,
                assigned_hosts: vec![
                    SoraHfPlacementHostAssignmentV1 {
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: alice_capability.peer_id.clone(),
                        role: SoraHfPlacementHostRoleV1::Primary,
                        status: SoraHfPlacementHostStatusV1::Warm,
                        host_class: alice_capability.host_class.clone(),
                    },
                    SoraHfPlacementHostAssignmentV1 {
                        validator_account_id: BOB_ID.clone(),
                        peer_id: bob_capability.peer_id.clone(),
                        role: SoraHfPlacementHostRoleV1::Replica,
                        status: SoraHfPlacementHostStatusV1::Warm,
                        host_class: bob_capability.host_class.clone(),
                    },
                ],
                total_reservation_fee_nanos: 3_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        initial_tx.apply();
        initial_block.commit()?;

        let reconcile_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 111;
            })
            .as_ref()
            .header();
        let mut reconcile_block = state.block(reconcile_header);
        let mut reconcile_tx = reconcile_block.transaction();
        isi::ReconcileSoracloudModelHosts.execute(&ALICE_ID, &mut reconcile_tx)?;
        reconcile_tx.apply();
        reconcile_block.commit()?;

        let view = state.view();
        let host_violation_evidence = view
            .world()
            .soracloud_model_host_violation_evidence()
            .iter()
            .map(|(_evidence_id, record)| record.clone())
            .collect::<Vec<_>>();
        let placement = view
            .world()
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("reconciled placement");
        assert_eq!(placement.status, SoraHfPlacementStatusV1::Degraded);
        assert_eq!(placement.eligible_validator_count, 2);
        assert_eq!(placement.last_rebalance_at_ms, 111);
        assert_eq!(
            placement.last_error.as_deref(),
            Some("assigned host heartbeat expired")
        );
        assert_eq!(placement.assigned_hosts.len(), 2);
        assert_eq!(
            placement.assigned_hosts[0].validator_account_id,
            BOB_ID.clone()
        );
        assert_eq!(
            placement.assigned_hosts[0].role,
            SoraHfPlacementHostRoleV1::Primary
        );
        assert_eq!(
            placement.assigned_hosts[0].status,
            SoraHfPlacementHostStatusV1::Warm
        );
        assert_eq!(placement.assigned_hosts[1].validator_account_id, charlie_id);
        assert_eq!(
            placement.assigned_hosts[1].role,
            SoraHfPlacementHostRoleV1::Replica
        );
        assert_eq!(
            placement.assigned_hosts[1].status,
            SoraHfPlacementHostStatusV1::Warming
        );
        assert!(
            placement
                .assigned_hosts
                .iter()
                .all(|assignment| assignment.validator_account_id != ALICE_ID.clone())
        );
        assert_eq!(host_violation_evidence.len(), 1);
        assert_eq!(
            host_violation_evidence[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(host_violation_evidence[0].strike_count, 1);
        assert!(!host_violation_evidence[0].penalty_applied);
        Ok(())
    }

    #[test]
    fn reconcile_soracloud_model_hosts_is_idempotent_after_primary_eviction()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let charlie_id = AccountId::new(KeyPair::random().public_key().clone());
        let pool_id = Hash::new(b"pool");

        let initial_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut initial_block = state.block(initial_header);
        let mut initial_tx = initial_block.transaction();
        insert_active_public_lane_validator(&mut initial_tx, BOB_ID.clone(), 900);
        insert_active_public_lane_validator(&mut initial_tx, charlie_id.clone(), 800);

        let alice_capability = sample_model_host_capability(ALICE_ID.clone(), 10, 110);
        let mut bob_capability = sample_model_host_capability(BOB_ID.clone(), 10, 500);
        bob_capability.peer_id = "12D3KooWBobHostTestPeer".to_string();
        let mut charlie_capability = sample_model_host_capability(charlie_id, 10, 500);
        charlie_capability.peer_id = "12D3KooWCharlieHostTestPeer".to_string();
        record_model_host_capability(&mut initial_tx, alice_capability.clone())?;
        record_model_host_capability(&mut initial_tx, bob_capability.clone())?;
        record_model_host_capability(&mut initial_tx, charlie_capability.clone())?;
        record_hf_shared_lease_pool(
            &mut initial_tx,
            sample_hf_shared_lease_pool_record(pool_id, Hash::new(b"source"), 100),
        )?;

        record_hf_placement(
            &mut initial_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: Hash::new(b"placement"),
                source_id: Hash::new(b"source"),
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 3,
                adaptive_target_host_count: 2,
                assigned_hosts: vec![
                    SoraHfPlacementHostAssignmentV1 {
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: alice_capability.peer_id.clone(),
                        role: SoraHfPlacementHostRoleV1::Primary,
                        status: SoraHfPlacementHostStatusV1::Warm,
                        host_class: alice_capability.host_class.clone(),
                    },
                    SoraHfPlacementHostAssignmentV1 {
                        validator_account_id: BOB_ID.clone(),
                        peer_id: bob_capability.peer_id.clone(),
                        role: SoraHfPlacementHostRoleV1::Replica,
                        status: SoraHfPlacementHostStatusV1::Warm,
                        host_class: bob_capability.host_class.clone(),
                    },
                ],
                total_reservation_fee_nanos: 3_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        initial_tx.apply();
        initial_block.commit()?;

        let first_reconcile_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 111;
            })
            .as_ref()
            .header();
        let mut first_reconcile_block = state.block(first_reconcile_header);
        let mut first_reconcile_tx = first_reconcile_block.transaction();
        isi::ReconcileSoracloudModelHosts.execute(&ALICE_ID, &mut first_reconcile_tx)?;
        first_reconcile_tx.apply();
        first_reconcile_block.commit()?;

        let placement_after_first = state
            .view()
            .world()
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("placement after first reconcile")
            .clone();

        let second_reconcile_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 222;
            })
            .as_ref()
            .header();
        let mut second_reconcile_block = state.block(second_reconcile_header);
        let mut second_reconcile_tx = second_reconcile_block.transaction();
        isi::ReconcileSoracloudModelHosts.execute(&ALICE_ID, &mut second_reconcile_tx)?;
        second_reconcile_tx.apply();
        second_reconcile_block.commit()?;

        let view = state.view();
        let placement_after_second = view
            .world()
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("placement after second reconcile");
        assert_eq!(*placement_after_second, placement_after_first);
        Ok(())
    }

    #[test]
    fn report_model_host_violation_slashes_and_evicts_warmup_no_show() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        let placement_id = Hash::new(b"warmup-placement");
        let pool_id = Hash::new(b"warmup-pool");
        let source_id = Hash::new(b"warmup-source");
        let stake_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "stake".parse().expect("stake"),
        );
        state.nexus.get_mut().staking.stake_asset_id = stake_asset_definition_id.to_string();
        state.nexus.get_mut().staking.stake_escrow_account_id = ALICE_ID.to_string();
        state.nexus.get_mut().staking.slash_sink_account_id = ALICE_ID.to_string();

        let setup_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut setup_block = state.block(setup_header);
        let mut setup_tx = setup_block.transaction();
        configure_staking_assets_for_validator_slash_test(&mut setup_tx, &BOB_ID, 1_000)?;
        insert_active_public_lane_validator(&mut setup_tx, BOB_ID.clone(), 1_000);
        let mut bob_capability = sample_model_host_capability(BOB_ID.clone(), 10, 1_000);
        bob_capability.peer_id = "12D3KooWBobWarmupHost".to_string();
        record_model_host_capability(&mut setup_tx, bob_capability.clone())?;
        record_hf_shared_lease_pool(
            &mut setup_tx,
            sample_hf_shared_lease_pool_record(pool_id, source_id, 100),
        )?;
        record_hf_placement(
            &mut setup_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id,
                source_id,
                pool_id,
                status: SoraHfPlacementStatusV1::Warming,
                selection_seed_hash: Hash::new(b"warmup-seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: BOB_ID.clone(),
                    peer_id: bob_capability.peer_id.clone(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warming,
                    host_class: bob_capability.host_class.clone(),
                }],
                total_reservation_fee_nanos: 1_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        setup_tx.apply();
        setup_block.commit()?;

        let report_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 200;
            })
            .as_ref()
            .header();
        let mut report_block = state.block(report_header);
        let mut report_tx = report_block.transaction();
        isi::ReportSoracloudModelHostViolation {
            validator_account_id: BOB_ID.clone(),
            kind: SoraModelHostViolationKindV1::WarmupNoShow,
            placement_id: Some(placement_id),
            detail: Some("warmup deadline exceeded".to_string()),
        }
        .execute(&ALICE_ID, &mut report_tx)?;
        report_tx.apply();
        report_block.commit()?;

        let view = state.view();
        let evidence = view
            .world()
            .soracloud_model_host_violation_evidence()
            .iter()
            .map(|(_evidence_id, record)| record.clone())
            .collect::<Vec<_>>();
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].kind, SoraModelHostViolationKindV1::WarmupNoShow);
        assert_eq!(evidence[0].strike_count, 1);
        assert!(evidence[0].penalty_applied);
        assert!(evidence[0].host_evicted);
        assert!(evidence[0].slash_id.is_some());
        assert_eq!(evidence[0].pool_id, Some(pool_id));
        assert_eq!(evidence[0].source_id, Some(source_id));
        assert_eq!(evidence[0].window_started_at_ms, Some(100));
        assert!(
            view.world()
                .soracloud_model_host_capabilities()
                .get(&BOB_ID)
                .is_none()
        );
        let bob_validator = view
            .world()
            .public_lane_validators()
            .get(&(LaneId::SINGLE, BOB_ID.clone()))
            .expect("bob validator after slash");
        assert_eq!(bob_validator.total_stake, Numeric::new(950, 0));
        assert_eq!(bob_validator.self_stake, Numeric::new(950, 0));
        assert!(matches!(
            bob_validator.status,
            PublicLaneValidatorStatus::Slashed(_)
        ));
        Ok(())
    }

    #[test]
    fn report_model_host_violation_applies_slash_when_heartbeat_miss_reaches_threshold()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        let placement_id = Hash::new(b"heartbeat-placement");
        let pool_id = Hash::new(b"heartbeat-pool");
        let source_id = Hash::new(b"heartbeat-source");
        let stake_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "stake".parse().expect("stake"),
        );
        state.nexus.get_mut().staking.stake_asset_id = stake_asset_definition_id.to_string();
        state.nexus.get_mut().staking.stake_escrow_account_id = ALICE_ID.to_string();
        state.nexus.get_mut().staking.slash_sink_account_id = ALICE_ID.to_string();
        state
            .nexus
            .get_mut()
            .hf_shared_leases
            .assigned_heartbeat_miss_strike_threshold = 2;

        let setup_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut setup_block = state.block(setup_header);
        let mut setup_tx = setup_block.transaction();
        configure_staking_assets_for_validator_slash_test(&mut setup_tx, &BOB_ID, 1_000)?;
        insert_active_public_lane_validator(&mut setup_tx, BOB_ID.clone(), 1_000);
        let mut bob_capability = sample_model_host_capability(BOB_ID.clone(), 10, 1_000);
        bob_capability.peer_id = "12D3KooWBobHeartbeatHost".to_string();
        record_model_host_capability(&mut setup_tx, bob_capability.clone())?;
        record_hf_shared_lease_pool(
            &mut setup_tx,
            sample_hf_shared_lease_pool_record(pool_id, source_id, 100),
        )?;
        record_hf_placement(
            &mut setup_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id,
                source_id,
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"heartbeat-seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: BOB_ID.clone(),
                    peer_id: bob_capability.peer_id.clone(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: bob_capability.host_class.clone(),
                }],
                total_reservation_fee_nanos: 1_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        record_model_host_violation_evidence(
            &mut setup_tx,
            SoraModelHostViolationEvidenceRecordV1 {
                schema_version: SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1,
                evidence_id: Hash::new(b"prior-heartbeat-evidence"),
                sequence: 1,
                validator_account_id: BOB_ID.clone(),
                kind: SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
                placement_id: Some(placement_id),
                pool_id: Some(pool_id),
                source_id: Some(source_id),
                window_started_at_ms: Some(100),
                observed_at_ms: 150,
                detail: Some("first missed heartbeat".to_string()),
                strike_count: 1,
                penalty_applied: false,
                host_evicted: false,
                slash_id: None,
            },
        )?;
        setup_tx.apply();
        setup_block.commit()?;

        let report_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 200;
            })
            .as_ref()
            .header();
        let mut report_block = state.block(report_header);
        let mut report_tx = report_block.transaction();
        isi::ReportSoracloudModelHostViolation {
            validator_account_id: BOB_ID.clone(),
            kind: SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
            placement_id: Some(placement_id),
            detail: Some("second missed heartbeat".to_string()),
        }
        .execute(&ALICE_ID, &mut report_tx)?;
        report_tx.apply();
        report_block.commit()?;

        let view = state.view();
        let evidence = view
            .world()
            .soracloud_model_host_violation_evidence()
            .iter()
            .map(|(_evidence_id, record)| record.clone())
            .collect::<Vec<_>>();
        assert_eq!(evidence.len(), 2);
        let latest = evidence
            .into_iter()
            .max_by_key(|record| record.sequence)
            .expect("latest heartbeat evidence");
        assert_eq!(
            latest.kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(latest.strike_count, 2);
        assert!(latest.penalty_applied);
        assert!(latest.host_evicted);
        assert!(latest.slash_id.is_some());
        assert!(
            view.world()
                .soracloud_model_host_capabilities()
                .get(&BOB_ID)
                .is_none()
        );
        let bob_validator = view
            .world()
            .public_lane_validators()
            .get(&(LaneId::SINGLE, BOB_ID.clone()))
            .expect("bob validator after heartbeat slash");
        assert_eq!(bob_validator.total_stake, Numeric::new(975, 0));
        assert_eq!(bob_validator.self_stake, Numeric::new(975, 0));
        assert!(matches!(
            bob_validator.status,
            PublicLaneValidatorStatus::Slashed(_)
        ));
        Ok(())
    }

    #[test]
    fn model_host_advertise_contradiction_emits_evidence_and_slashes_validator()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        let pool_id = Hash::new(b"advert-contradiction-pool");
        let source_id = Hash::new(b"advert-contradiction-source");
        let stake_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "stake".parse().expect("stake"),
        );
        state.nexus.get_mut().staking.stake_asset_id = stake_asset_definition_id.to_string();
        state.nexus.get_mut().staking.stake_escrow_account_id = ALICE_ID.to_string();
        state.nexus.get_mut().staking.slash_sink_account_id = ALICE_ID.to_string();

        let setup_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = 100;
            })
            .as_ref()
            .header();
        let mut setup_block = state.block(setup_header);
        let mut setup_tx = setup_block.transaction();
        configure_staking_assets_for_validator_slash_test(&mut setup_tx, &BOB_ID, 1_000)?;
        Grant::account_permission(
            Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(())),
            BOB_ID.clone(),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut setup_tx)?;
        insert_active_public_lane_validator(&mut setup_tx, BOB_ID.clone(), 1_000);
        let mut bob_capability = sample_model_host_capability(BOB_ID.clone(), 10, 1_000);
        bob_capability.peer_id = "12D3KooWBobContradictionPeer".to_string();
        record_model_host_capability(&mut setup_tx, bob_capability.clone())?;
        record_hf_shared_lease_pool(
            &mut setup_tx,
            sample_hf_shared_lease_pool_record(pool_id, source_id, 100),
        )?;
        record_hf_placement(
            &mut setup_tx,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: Hash::new(b"advert-contradiction-placement"),
                source_id,
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"advert-contradiction-seed"),
                resource_profile: sample_hf_resource_profile(),
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: BOB_ID.clone(),
                    peer_id: bob_capability.peer_id.clone(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: bob_capability.host_class.clone(),
                }],
                total_reservation_fee_nanos: 1_000,
                last_rebalance_at_ms: 100,
                last_error: None,
            },
        )?;
        setup_tx.apply();
        setup_block.commit()?;

        let advertise_header = ValidBlock::new_dummy_and_modify_header(
            &BOB_KEYPAIR.clone().into_parts().1,
            |header| {
                header.creation_time_ms = 200;
            },
        )
        .as_ref()
        .header();
        let mut advertise_block = state.block(advertise_header);
        let mut advertise_tx = advertise_block.transaction();
        let mut contradictory_capability = bob_capability.clone();
        contradictory_capability.advertised_at_ms = 200;
        contradictory_capability.heartbeat_expires_at_ms = 1_200;
        contradictory_capability.supported_formats =
            std::collections::BTreeSet::from([SoraHfModelFormatV1::Gguf]);
        isi::AdvertiseSoracloudModelHost {
            capability: contradictory_capability.clone(),
            provenance: model_host_advertise_provenance_for(
                &BOB_KEYPAIR,
                &contradictory_capability,
            ),
        }
        .execute(&BOB_ID, &mut advertise_tx)?;
        advertise_tx.apply();
        advertise_block.commit()?;

        let view = state.view();
        assert!(
            view.world()
                .soracloud_model_host_capabilities()
                .get(&BOB_ID)
                .is_none()
        );
        let evidence = view
            .world()
            .soracloud_model_host_violation_evidence()
            .iter()
            .map(|(_evidence_id, record)| record.clone())
            .collect::<Vec<_>>();
        assert_eq!(evidence.len(), 1);
        assert_eq!(
            evidence[0].kind,
            SoraModelHostViolationKindV1::AdvertContradiction
        );
        assert!(evidence[0].penalty_applied);
        assert!(evidence[0].host_evicted);
        assert!(evidence[0].placement_id.is_none());
        assert!(
            evidence[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("model format"))
        );
        let placement = view
            .world()
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("updated placement after contradiction");
        assert!(
            placement
                .assigned_hosts
                .iter()
                .all(|assignment| assignment.validator_account_id != BOB_ID.clone())
        );
        assert_eq!(
            placement.last_error.as_deref(),
            evidence[0].detail.as_deref()
        );
        let bob_validator = view
            .world()
            .public_lane_validators()
            .get(&(LaneId::SINGLE, BOB_ID.clone()))
            .expect("bob validator after contradiction slash");
        assert_eq!(bob_validator.total_stake, Numeric::new(900, 0));
        assert_eq!(bob_validator.self_stake, Numeric::new(900, 0));
        assert!(matches!(
            bob_validator.status,
            PublicLaneValidatorStatus::Slashed(_)
        ));
        Ok(())
    }

    #[test]
    fn leave_hf_shared_lease_last_member_uses_configured_drain_grace() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();
        state.nexus.get_mut().hf_shared_leases.drain_grace = Duration::from_secs(30);

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);

        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                model_name,
                &service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::LeaveSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            storage_class,
            lease_term_ms,
            service_name: Some(service_name.clone()),
            apartment_name: None,
            provenance: hf_shared_lease_leave_provenance(
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                Some(&service_name),
                None,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let source_id = hf_source_id(repo_id, resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let member_key = (pool_id.to_string(), ALICE_ID.to_string());
        let view = state.view();
        let world = view.world();
        let pool = world
            .soracloud_hf_shared_lease_pools()
            .get(&pool_id)
            .expect("shared lease pool");
        let member = world
            .soracloud_hf_shared_lease_members()
            .get(&member_key)
            .expect("shared lease member");
        let latest_audit_event = world
            .soracloud_hf_shared_lease_audit_events()
            .iter()
            .max_by_key(|(sequence, _event)| *sequence)
            .map(|(_sequence, event)| event)
            .expect("latest audit event");

        assert_eq!(pool.status, SoraHfSharedLeaseStatusV1::Draining);
        assert_eq!(pool.active_member_count, 0);
        assert_eq!(member.status, SoraHfSharedLeaseMemberStatusV1::Left);
        assert_eq!(pool.window_expires_at_ms, member.updated_at_ms + 30_000);
        assert_eq!(latest_audit_event.action, SoraHfSharedLeaseActionV1::Leave);
        assert_eq!(
            latest_audit_event.lease_expires_at_ms,
            pool.window_expires_at_ms
        );
        Ok(())
    }

    #[test]
    fn join_hf_shared_lease_marks_source_ready_when_generated_service_is_already_deployed()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );
        let source_id = hf_source_id(repo_id, resolved_revision)?;
        let bundle = crate::soracloud_runtime::build_soracloud_hf_generated_service_bundle(
            service_name.clone(),
            &source_id.to_string(),
            repo_id,
            resolved_revision,
            model_name,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);

        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                model_name,
                &service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let source = view
            .world()
            .soracloud_hf_sources()
            .get(&source_id)
            .expect("hf source");
        assert_eq!(source.status, SoraHfSourceStatusV1::Ready);
        Ok(())
    }

    #[test]
    fn renew_hf_shared_lease_active_window_queues_next_window() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let renewed_model_name = "gpt-oss-renewed";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let renewed_service_name: iroha_data_model::name::Name =
            "vision_portal_v2".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let renewed_fee_nanos = 12_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);

        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                model_name,
                &service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let current_pool_expiry = stx
            .world
            .soracloud_hf_shared_lease_pools
            .iter()
            .next()
            .map(|(_pool_id, pool)| pool.window_expires_at_ms)
            .expect("pool");

        isi::RenewSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: renewed_model_name.to_string(),
            service_name: renewed_service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos: renewed_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_renew_provenance(
                repo_id,
                resolved_revision,
                renewed_model_name,
                &renewed_service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                renewed_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let source_id = hf_source_id(repo_id, resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let member_key = (pool_id.to_string(), ALICE_ID.to_string());
        let view = state.view();
        let world = view.world();
        let pool = world
            .soracloud_hf_shared_lease_pools()
            .get(&pool_id)
            .expect("shared lease pool");
        let member = world
            .soracloud_hf_shared_lease_members()
            .get(&member_key)
            .expect("shared lease member");
        let audit_event = world
            .soracloud_hf_shared_lease_audit_events()
            .iter()
            .max_by_key(|(sequence, _event)| *sequence)
            .map(|(_sequence, event)| event)
            .expect("latest audit event");
        let queued_next_window = pool
            .queued_next_window
            .as_ref()
            .expect("queued next window");
        let active_placement = world
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("active placement");

        assert_eq!(pool.status, SoraHfSharedLeaseStatusV1::Active);
        assert_eq!(pool.active_member_count, 1);
        assert_eq!(pool.base_fee_nanos, base_fee_nanos);
        assert_eq!(pool.window_expires_at_ms, current_pool_expiry);
        assert_eq!(queued_next_window.window_started_at_ms, current_pool_expiry);
        assert_eq!(
            queued_next_window.window_expires_at_ms,
            current_pool_expiry + lease_term_ms
        );
        assert_eq!(queued_next_window.base_fee_nanos, renewed_fee_nanos);
        assert_eq!(queued_next_window.model_name, renewed_model_name);
        assert_eq!(queued_next_window.service_name, renewed_service_name);
        assert_eq!(
            queued_next_window.compute_reservation_fee_nanos,
            queued_next_window
                .planned_placement
                .total_reservation_fee_nanos
        );
        assert!(queued_next_window.compute_reservation_fee_nanos > 0);
        assert_eq!(member.status, SoraHfSharedLeaseMemberStatusV1::Active);
        assert_eq!(
            member.total_paid_nanos,
            base_fee_nanos.saturating_add(renewed_fee_nanos)
        );
        assert_eq!(member.last_charge_nanos, renewed_fee_nanos);
        assert_eq!(
            member.total_compute_paid_nanos,
            active_placement
                .total_reservation_fee_nanos
                .saturating_add(queued_next_window.compute_reservation_fee_nanos)
        );
        assert_eq!(
            member.last_compute_charge_nanos,
            queued_next_window.compute_reservation_fee_nanos
        );
        assert_eq!(audit_event.action, SoraHfSharedLeaseActionV1::Renew);
        assert_eq!(
            audit_event.lease_expires_at_ms,
            queued_next_window.window_expires_at_ms
        );
        Ok(())
    }

    #[test]
    fn join_hf_shared_lease_after_queued_sponsorship_promotes_next_window()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let renewed_model_name = "gpt-oss-renewed";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let renewed_service_name: iroha_data_model::name::Name =
            "vision_portal_v2".parse().expect("valid");
        let rebound_service_name: iroha_data_model::name::Name =
            "vision_portal_v3".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let renewed_fee_nanos = 12_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );

        {
            let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
                .as_ref()
                .header();
            let mut state_block = state.block(block_header);
            let mut stx = state_block.transaction();
            seed_test_call_hash(&mut stx, 0xE1);
            let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);

            isi::AdvertiseSoracloudModelHost {
                capability: capability.clone(),
                provenance: model_host_advertise_provenance(&capability),
            }
            .execute(&ALICE_ID, &mut stx)?;

            isi::JoinSoracloudHfSharedLease {
                repo_id: repo_id.to_string(),
                resolved_revision: resolved_revision.to_string(),
                model_name: model_name.to_string(),
                service_name: service_name.clone(),
                apartment_name: None,
                storage_class,
                lease_term_ms,
                lease_asset_definition_id: lease_asset_definition_id.clone(),
                base_fee_nanos,
                resource_profile: Some(sample_hf_resource_profile()),
                provenance: hf_shared_lease_join_provenance(
                    repo_id,
                    resolved_revision,
                    model_name,
                    &service_name,
                    None,
                    storage_class,
                    lease_term_ms,
                    &lease_asset_definition_id,
                    base_fee_nanos,
                ),
            }
            .execute(&ALICE_ID, &mut stx)?;

            isi::RenewSoracloudHfSharedLease {
                repo_id: repo_id.to_string(),
                resolved_revision: resolved_revision.to_string(),
                model_name: renewed_model_name.to_string(),
                service_name: renewed_service_name.clone(),
                apartment_name: None,
                storage_class,
                lease_term_ms,
                lease_asset_definition_id: lease_asset_definition_id.clone(),
                base_fee_nanos: renewed_fee_nanos,
                resource_profile: Some(sample_hf_resource_profile()),
                provenance: hf_shared_lease_renew_provenance(
                    repo_id,
                    resolved_revision,
                    renewed_model_name,
                    &renewed_service_name,
                    None,
                    storage_class,
                    lease_term_ms,
                    &lease_asset_definition_id,
                    renewed_fee_nanos,
                ),
            }
            .execute(&ALICE_ID, &mut stx)?;

            stx.apply();
            state_block.commit()?;
        }

        let first_pool_expires_at_ms = {
            let view = state.view();
            let world = view.world();
            world
                .soracloud_hf_shared_lease_pools()
                .iter()
                .next()
                .map(|(_pool_id, pool)| pool.window_expires_at_ms)
                .expect("pool")
        };
        let queued_placement_id = {
            let view = state.view();
            let world = view.world();
            let source_id = hf_source_id(repo_id, resolved_revision)?;
            let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
            world
                .soracloud_hf_shared_lease_pools()
                .get(&pool_id)
                .and_then(|pool| pool.queued_next_window.as_ref())
                .map(|next_window| next_window.planned_placement.placement_id)
                .expect("queued next-window placement")
        };

        let second_block_header =
            ValidBlock::new_dummy_and_modify_header(&KeyPair::random().into_parts().1, |header| {
                header.creation_time_ms = first_pool_expires_at_ms.saturating_add(1);
            })
            .as_ref()
            .header();
        let mut second_state_block = state.block(second_block_header);
        let mut second_stx = second_state_block.transaction();

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: renewed_model_name.to_string(),
            service_name: rebound_service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos: renewed_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                renewed_model_name,
                &rebound_service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                renewed_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut second_stx)?;

        second_stx.apply();
        second_state_block.commit()?;

        let source_id = hf_source_id(repo_id, resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let member_key = (pool_id.to_string(), ALICE_ID.to_string());
        let view = state.view();
        let world = view.world();
        let pool = world
            .soracloud_hf_shared_lease_pools()
            .get(&pool_id)
            .expect("shared lease pool");
        let member = world
            .soracloud_hf_shared_lease_members()
            .get(&member_key)
            .expect("shared lease member");
        let source = world
            .soracloud_hf_sources()
            .get(&source_id)
            .expect("hf source");
        let placement = world
            .soracloud_hf_placements()
            .get(&pool_id)
            .expect("active placement");
        let audit_event = world
            .soracloud_hf_shared_lease_audit_events()
            .iter()
            .max_by_key(|(sequence, _event)| *sequence)
            .map(|(_sequence, event)| event)
            .expect("latest audit event");

        assert_eq!(pool.status, SoraHfSharedLeaseStatusV1::Active);
        assert_eq!(pool.active_member_count, 1);
        assert!(pool.queued_next_window.is_none());
        assert_eq!(pool.window_started_at_ms, first_pool_expires_at_ms);
        assert_eq!(
            pool.window_expires_at_ms,
            first_pool_expires_at_ms.saturating_add(lease_term_ms)
        );
        assert_eq!(pool.base_fee_nanos, renewed_fee_nanos);
        assert_eq!(source.model_name, renewed_model_name);
        assert_eq!(placement.placement_id, queued_placement_id);
        assert_eq!(member.status, SoraHfSharedLeaseMemberStatusV1::Active);
        assert_eq!(member.joined_at_ms, first_pool_expires_at_ms);
        assert_eq!(member.last_charge_nanos, 0);
        assert_eq!(member.last_compute_charge_nanos, 0);
        assert!(member.service_bindings.contains(service_name.as_ref()));
        assert!(
            member
                .service_bindings
                .contains(renewed_service_name.as_ref())
        );
        assert!(
            member
                .service_bindings
                .contains(rebound_service_name.as_ref())
        );
        assert_eq!(audit_event.action, SoraHfSharedLeaseActionV1::Join);
        assert_eq!(audit_event.charged_nanos, 0);
        assert_eq!(audit_event.lease_expires_at_ms, pool.window_expires_at_ms);
        Ok(())
    }

    #[test]
    fn join_hf_shared_lease_rejects_when_no_model_host_can_run_profile() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let error = isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                model_name,
                &service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("join should fail without an eligible host advert");

        assert!(
            error
                .to_string()
                .contains("no eligible validator host advert"),
            "unexpected error: {error}"
        );
        Ok(())
    }

    #[test]
    fn join_hf_shared_lease_late_join_prorates_compute_and_refunds_existing_members()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let bob_service_name: iroha_data_model::name::Name =
            "vision_portal_bob".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );

        {
            let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
                .as_ref()
                .header();
            let mut state_block = state.block(block_header);
            let mut stx = state_block.transaction();
            let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);
            Register::account(Account::new(BOB_ID.clone()))
                .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Grant::account_permission(
                Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(())),
                BOB_ID.clone(),
            )
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Register::asset_definition(
                AssetDefinition::numeric(lease_asset_definition_id.clone())
                    .with_name(lease_asset_definition_id.name().to_string()),
            )
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Mint::asset_numeric(
                Numeric::new(100_000, 0),
                AssetId::new(lease_asset_definition_id.clone(), ALICE_ID.clone()),
            )
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Mint::asset_numeric(
                Numeric::new(100_000, 0),
                AssetId::new(lease_asset_definition_id.clone(), BOB_ID.clone()),
            )
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            isi::AdvertiseSoracloudModelHost {
                capability: capability.clone(),
                provenance: model_host_advertise_provenance(&capability),
            }
            .execute(&ALICE_ID, &mut stx)?;

            isi::JoinSoracloudHfSharedLease {
                repo_id: repo_id.to_string(),
                resolved_revision: resolved_revision.to_string(),
                model_name: model_name.to_string(),
                service_name: service_name.clone(),
                apartment_name: None,
                storage_class,
                lease_term_ms,
                lease_asset_definition_id: lease_asset_definition_id.clone(),
                base_fee_nanos,
                resource_profile: Some(sample_hf_resource_profile()),
                provenance: hf_shared_lease_join_provenance(
                    repo_id,
                    resolved_revision,
                    model_name,
                    &service_name,
                    None,
                    storage_class,
                    lease_term_ms,
                    &lease_asset_definition_id,
                    base_fee_nanos,
                ),
            }
            .execute(&ALICE_ID, &mut stx)?;

            stx.apply();
            state_block.commit()?;
        }

        let source_id = hf_source_id(repo_id, resolved_revision)?;
        let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
        let (first_window_started_at_ms, first_window_expires_at_ms, placement_fee_nanos) = {
            let view = state.view();
            let world = view.world();
            let pool = world
                .soracloud_hf_shared_lease_pools()
                .get(&pool_id)
                .expect("shared lease pool");
            let placement = world
                .soracloud_hf_placements()
                .get(&pool_id)
                .expect("placement");
            (
                pool.window_started_at_ms,
                pool.window_expires_at_ms,
                placement.total_reservation_fee_nanos,
            )
        };
        let second_join_time_ms =
            first_window_started_at_ms.saturating_add(lease_term_ms.saturating_div(2));

        let second_block_header = ValidBlock::new_dummy_and_modify_header(
            &BOB_KEYPAIR.clone().into_parts().1,
            |header| {
                header.creation_time_ms = second_join_time_ms;
            },
        )
        .as_ref()
        .header();
        let mut second_state_block = state.block(second_block_header);
        let mut second_stx = second_state_block.transaction();
        seed_test_call_hash(&mut second_stx, 0xE2);

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: bob_service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance_for(
                &BOB_KEYPAIR,
                repo_id,
                resolved_revision,
                model_name,
                &bob_service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&BOB_ID, &mut second_stx)?;

        second_stx.apply();
        second_state_block.commit()?;

        let remaining_ms = first_window_expires_at_ms.saturating_sub(second_join_time_ms);
        let expected_storage_join_fee =
            prorated_window_fee_nanos(base_fee_nanos, remaining_ms, lease_term_ms) / 2;
        let expected_compute_join_fee =
            prorated_window_fee_nanos(placement_fee_nanos, remaining_ms, lease_term_ms) / 2;
        let alice_member_key = (pool_id.to_string(), ALICE_ID.to_string());
        let bob_member_key = (pool_id.to_string(), BOB_ID.to_string());
        let view = state.view();
        let world = view.world();
        let pool = world
            .soracloud_hf_shared_lease_pools()
            .get(&pool_id)
            .expect("shared lease pool");
        let alice_member = world
            .soracloud_hf_shared_lease_members()
            .get(&alice_member_key)
            .expect("alice shared lease member");
        let bob_member = world
            .soracloud_hf_shared_lease_members()
            .get(&bob_member_key)
            .expect("bob shared lease member");

        assert_eq!(pool.active_member_count, 2);
        assert_eq!(alice_member.total_refunded_nanos, expected_storage_join_fee);
        assert_eq!(
            alice_member.total_compute_refunded_nanos,
            expected_compute_join_fee
        );
        assert_eq!(bob_member.total_paid_nanos, expected_storage_join_fee);
        assert_eq!(
            bob_member.total_compute_paid_nanos,
            expected_compute_join_fee
        );
        assert_eq!(bob_member.last_charge_nanos, expected_storage_join_fee);
        assert_eq!(
            bob_member.last_compute_charge_nanos,
            expected_compute_join_fee
        );
        Ok(())
    }

    #[test]
    fn leave_hf_shared_lease_rejects_queued_next_window_sponsor() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let repo_id = "openai/gpt-oss";
        let resolved_revision = "main";
        let model_name = "gpt-oss";
        let renewed_model_name = "gpt-oss-renewed";
        let service_name: iroha_data_model::name::Name = "vision_portal".parse().expect("valid");
        let renewed_service_name: iroha_data_model::name::Name =
            "vision_portal_v2".parse().expect("valid");
        let storage_class = StorageClass::Warm;
        let lease_term_ms = 60_000_u64;
        let base_fee_nanos = 10_000_u128;
        let renewed_fee_nanos = 12_000_u128;
        let lease_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").expect("domain"),
            "xor".parse().expect("xor"),
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);

        isi::AdvertiseSoracloudModelHost {
            capability: capability.clone(),
            provenance: model_host_advertise_provenance(&capability),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::JoinSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: model_name.to_string(),
            service_name: service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_join_provenance(
                repo_id,
                resolved_revision,
                model_name,
                &service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                base_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::RenewSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            model_name: renewed_model_name.to_string(),
            service_name: renewed_service_name.clone(),
            apartment_name: None,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id: lease_asset_definition_id.clone(),
            base_fee_nanos: renewed_fee_nanos,
            resource_profile: Some(sample_hf_resource_profile()),
            provenance: hf_shared_lease_renew_provenance(
                repo_id,
                resolved_revision,
                renewed_model_name,
                &renewed_service_name,
                None,
                storage_class,
                lease_term_ms,
                &lease_asset_definition_id,
                renewed_fee_nanos,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let error = isi::LeaveSoracloudHfSharedLease {
            repo_id: repo_id.to_string(),
            resolved_revision: resolved_revision.to_string(),
            storage_class,
            lease_term_ms,
            service_name: Some(service_name.clone()),
            apartment_name: None,
            provenance: hf_shared_lease_leave_provenance(
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                Some(&service_name),
                None,
            ),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("queued sponsor should not be able to leave");

        assert!(
            error
                .to_string()
                .contains("cannot leave before it activates"),
            "unexpected error: {error}"
        );
        Ok(())
    }

    #[test]
    fn set_inrou_replica_runtime_state_ignores_missing_placement() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let key = inrou_replica_runtime_key(
            &runtime_state.service_name,
            service_version,
            runtime_state.replica_slot,
        );
        stx.world
            .soracloud_inrou_replica_runtime
            .insert(key.clone(), runtime_state.clone());

        isi::SetSoracloudInrouReplicaRuntimeState {
            state: runtime_state,
        }
        .execute(&ALICE_ID, &mut stx)?;

        assert!(
            stx.world
                .soracloud_inrou_replica_runtime
                .get(&key)
                .is_none(),
            "stale runtime update without an active placement should be cleared instead of failing"
        );
        Ok(())
    }

    #[test]
    fn clear_inrou_replica_runtime_state_ignores_missing_placement() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let key = inrou_replica_runtime_key(
            &runtime_state.service_name,
            service_version,
            runtime_state.replica_slot,
        );
        stx.world
            .soracloud_inrou_replica_runtime
            .insert(key.clone(), runtime_state);

        isi::ClearSoracloudInrouReplicaRuntimeState {
            service_name,
            service_version: service_version.to_string(),
            replica_slot: 1,
        }
        .execute(&ALICE_ID, &mut stx)?;

        assert!(
            stx.world
                .soracloud_inrou_replica_runtime
                .get(&key)
                .is_none(),
            "stale clear without an active placement should remove local runtime state"
        );
        Ok(())
    }

    #[test]
    fn set_inrou_replica_runtime_state_records_matching_placement() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let placement = sample_inrou_service_placement_record_for(
            service_name,
            service_version,
            &runtime_state,
        );
        stx.world.soracloud_inrou_service_placements.insert(
            (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            ),
            placement,
        );

        isi::SetSoracloudInrouReplicaRuntimeState {
            state: runtime_state.clone(),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let key = inrou_replica_runtime_key(
            &runtime_state.service_name,
            service_version,
            runtime_state.replica_slot,
        );
        assert_eq!(
            stx.world.soracloud_inrou_replica_runtime.get(&key),
            Some(&runtime_state)
        );
        Ok(())
    }

    #[test]
    fn set_inrou_replica_runtime_state_ignores_non_assigned_validator() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission_on_chain(
            &kura,
            iroha_data_model::ChainId::from(TAIRA_TESTNET_CHAIN_ID),
        )?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let assigned_runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let placement = sample_inrou_service_placement_record_for(
            service_name.clone(),
            service_version,
            &assigned_runtime_state,
        );
        stx.world.soracloud_inrou_service_placements.insert(
            (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            ),
            placement,
        );
        let key = inrou_replica_runtime_key(&service_name, service_version, 1);
        stx.world
            .soracloud_inrou_replica_runtime
            .insert(key.clone(), assigned_runtime_state.clone());
        let stale_runtime_state = sample_inrou_replica_runtime_state_for(
            service_name,
            service_version,
            1,
            BOB_ID.clone(),
        );

        isi::SetSoracloudInrouReplicaRuntimeState {
            state: stale_runtime_state,
        }
        .execute(&BOB_ID, &mut stx)?;

        assert_eq!(
            stx.world.soracloud_inrou_replica_runtime.get(&key),
            Some(&assigned_runtime_state),
            "runtime telemetry from a non-assigned validator should not invalidate or overwrite state"
        );
        Ok(())
    }

    #[test]
    fn set_inrou_replica_runtime_state_ignores_mismatched_placement_fields()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let assigned_runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let placement = sample_inrou_service_placement_record_for(
            service_name.clone(),
            service_version,
            &assigned_runtime_state,
        );
        stx.world.soracloud_inrou_service_placements.insert(
            (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            ),
            placement,
        );
        let key = inrou_replica_runtime_key(&service_name, service_version, 1);
        stx.world
            .soracloud_inrou_replica_runtime
            .insert(key.clone(), assigned_runtime_state.clone());
        let mut mismatched_runtime_state = assigned_runtime_state.clone();
        mismatched_runtime_state.peer_id = "12D3KooWMismatchedRuntimePeer".to_string();

        isi::SetSoracloudInrouReplicaRuntimeState {
            state: mismatched_runtime_state,
        }
        .execute(&ALICE_ID, &mut stx)?;

        assert_eq!(
            stx.world.soracloud_inrou_replica_runtime.get(&key),
            Some(&assigned_runtime_state),
            "runtime telemetry that no longer matches placement should not overwrite valid state"
        );
        Ok(())
    }

    #[test]
    fn clear_inrou_replica_runtime_state_ignores_non_assigned_validator() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission_on_chain(
            &kura,
            iroha_data_model::ChainId::from(TAIRA_TESTNET_CHAIN_ID),
        )?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
        let service_name: iroha_data_model::name::Name = "hayahi_live".parse().expect("valid");
        let service_version = "2026.04.28.075015";
        let assigned_runtime_state = sample_inrou_replica_runtime_state_for(
            service_name.clone(),
            service_version,
            1,
            ALICE_ID.clone(),
        );
        let placement = sample_inrou_service_placement_record_for(
            service_name.clone(),
            service_version,
            &assigned_runtime_state,
        );
        stx.world.soracloud_inrou_service_placements.insert(
            (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            ),
            placement,
        );
        let key = inrou_replica_runtime_key(&service_name, service_version, 1);
        stx.world
            .soracloud_inrou_replica_runtime
            .insert(key.clone(), assigned_runtime_state.clone());

        isi::ClearSoracloudInrouReplicaRuntimeState {
            service_name,
            service_version: service_version.to_string(),
            replica_slot: 1,
        }
        .execute(&BOB_ID, &mut stx)?;

        assert_eq!(
            stx.world.soracloud_inrou_replica_runtime.get(&key),
            Some(&assigned_runtime_state),
            "a non-assigned validator should not clear another replica's runtime state"
        );
        Ok(())
    }

    #[test]
    fn deploy_soracloud_service_records_bundle_and_audit_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle("portal", "1.0.0", 0);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        assert!(
            world
                .soracloud_service_revisions()
                .get(&(service_name.as_ref().to_owned(), "1.0.0".to_string()))
                .is_some()
        );
        let deployment = world
            .soracloud_service_deployments()
            .get(&service_name)
            .expect("deployment state");
        assert_eq!(deployment.current_service_version, "1.0.0");
        assert_eq!(deployment.revision_count, 1);
        assert_eq!(deployment.process_generation, 1);
        assert!(deployment.active_rollout.is_none());
        assert_eq!(world.soracloud_service_audit_events().iter().count(), 1);
        Ok(())
    }

    #[test]
    fn deploy_soracloud_service_rejects_missing_shared_http_service_volume()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let mut bundle = sample_bundle("portal", "1.0.0", 0);
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.container.inrou = Some(SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: SoraInrouGuestOsV1::DebianSlim,
            guest_images: std::collections::BTreeMap::from([
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_string(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_string(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_string(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_string(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: None,
            ssh_authorized_keys: vec!["ssh-ed25519 test-key soracloud-tests".to_string()],
        });
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        bundle.service.lease_volumes = vec![SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_string(),
            max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero"),
        }];
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("hosted HTTP deployments must declare shared replica-safe storage");
        Ok(())
    }

    #[test]
    fn report_soracloud_service_lease_usage_updates_authoritative_lease_state()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let mut bundle = sample_bundle("portal", "1.0.0", 0);
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.container.inrou = Some(sample_inrou_manifest());
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Open;
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.economics = SoraHttpServiceEconomicsV1 {
            schema_version: iroha_data_model::soracloud::SORA_HTTP_SERVICE_ECONOMICS_VERSION_V1,
            quota_class: "taira-open".to_string(),
            deployment_deposit_nanos: NonZeroU64::new(1_000_000_000).expect("nonzero"),
            prepaid_runtime_balance_nanos: NonZeroU64::new(5_000).expect("nonzero"),
            runtime_nanos_per_sequence: NonZeroU64::new(1).expect("nonzero"),
            storage_nanos_per_gib_sequence: NonZeroU64::new(1).expect("nonzero"),
            egress_nanos_per_mib: NonZeroU64::new(5_000).expect("nonzero"),
            lease_duration_sequences: NonZeroU64::new(100).expect("nonzero"),
        };
        bundle.service.lease_volumes = sample_inrou_lease_volumes();
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        bundle.service.artifacts[0].handler_name = None;
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        isi::ReportSoracloudServiceLeaseUsage {
            service_name: bundle.service.service_name.clone(),
            active_service_version: bundle.service.service_version.clone(),
            accounted_egress_bytes: 1024 * 1024,
        }
        .execute(&ALICE_ID, &mut stx)?;

        let deployment = stx
            .world
            .soracloud_service_deployments
            .get(&bundle.service.service_name)
            .expect("deployment");
        let lease = deployment.service_lease.as_ref().expect("lease");
        assert_eq!(lease.accounted_egress_bytes, 1024 * 1024);
        assert_eq!(lease.status, SoraServiceLeaseStatusV1::Exhausted);
        assert!(
            lease
                .last_status_reason
                .as_deref()
                .is_some_and(|reason| { reason.contains("prepaid runtime balance exhausted") })
        );
        Ok(())
    }

    #[test]
    fn deploy_soracloud_service_accepts_required_inline_materials() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let mut bundle = sample_bundle("portal", "1.0.0", 0);
        bundle.container.required_config_names = vec!["runtime/feature_flag".to_string()];
        bundle.container.required_secret_names = vec!["db/password".to_string()];
        bundle.service.container.manifest_hash = Hash::new(Encode::encode(&bundle.container));
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::from([(
                "runtime/feature_flag".to_string(),
                Json::from(norito::json!(true)),
            )]),
            initial_service_secrets: BTreeMap::from([(
                "db/password".to_string(),
                sample_service_secret_envelope(),
            )]),
            provenance: {
                let payload =
                    iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
                        &bundle,
                        &BTreeMap::from([(
                            "runtime/feature_flag".to_string(),
                            Json::from(norito::json!(true)),
                        )]),
                        &BTreeMap::from([(
                            "db/password".to_string(),
                            sample_service_secret_envelope(),
                        )]),
                    )
                    .expect("bundle payload");
                ManifestProvenance {
                    signer: ALICE_KEYPAIR.public_key().clone(),
                    signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
                }
            },
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let service_name: iroha_data_model::name::Name =
            "portal".parse().expect("valid service name");
        let view = state.view();
        let deployment = view
            .world()
            .soracloud_service_deployments()
            .get(&service_name)
            .expect("deployment state");
        assert_eq!(deployment.config_generation, 1);
        assert_eq!(deployment.secret_generation, 1);
        assert!(
            deployment
                .service_configs
                .contains_key("runtime/feature_flag")
        );
        assert!(deployment.service_secrets.contains_key("db/password"));
        Ok(())
    }

    #[test]
    fn delete_soracloud_service_config_rejects_required_active_material() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let mut bundle = sample_bundle("portal", "1.0.0", 0);
        bundle.container.required_config_names = vec!["runtime/feature_flag".to_string()];
        bundle.service.container.manifest_hash = Hash::new(Encode::encode(&bundle.container));
        let initial_service_configs = BTreeMap::from([(
            "runtime/feature_flag".to_string(),
            Json::from(norito::json!(true)),
        )]);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let deploy_payload =
            iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
                &bundle,
                &initial_service_configs,
                &BTreeMap::new(),
            )
            .expect("bundle payload");
        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs,
            initial_service_secrets: BTreeMap::new(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &deploy_payload,
                ),
            },
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let error = isi::DeleteSoracloudServiceConfig {
            service_name: service_name.clone(),
            config_name: "runtime/feature_flag".to_string(),
            provenance: service_config_delete_manifest_provenance(
                &service_name,
                "runtime/feature_flag",
            ),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("required config deletion must fail");
        assert!(
            error
                .to_string()
                .contains("required service config `runtime/feature_flag` is missing"),
            "unexpected error: {error}"
        );
        Ok(())
    }

    #[test]
    fn upgrade_soracloud_service_starts_canary_rollout() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let deploy_bundle = sample_bundle("portal", "1.0.0", 0);
        let upgrade_bundle = sample_bundle("portal", "1.1.0", 25);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: deploy_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&upgrade_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let deployment = world
            .soracloud_service_deployments()
            .get(&service_name)
            .expect("deployment state");
        let active_rollout = deployment.active_rollout.as_ref().expect("active rollout");
        assert_eq!(deployment.current_service_version, "1.1.0");
        assert_eq!(deployment.revision_count, 2);
        assert_eq!(active_rollout.canary_percent, 25);
        assert_eq!(active_rollout.stage, SoraRolloutStageV1::Canary);
        assert_eq!(world.soracloud_service_audit_events().iter().count(), 2);
        Ok(())
    }

    #[test]
    fn unhealthy_rollout_auto_rolls_back_to_baseline() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let deploy_bundle = sample_bundle("portal", "1.0.0", 0);
        let upgrade_bundle = sample_bundle("portal", "1.1.0", 25);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: deploy_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&upgrade_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let rollout_handle = latest_service_audit_event(&stx, &service_name)
            .and_then(|event| event.rollout_handle)
            .expect("rollout handle");
        let governance_tx_hash = Hash::new(b"gov-hash");
        isi::AdvanceSoracloudRollout {
            service_name: service_name.clone(),
            rollout_handle: rollout_handle.clone(),
            healthy: false,
            promote_to_percent: None,
            governance_tx_hash,
            provenance: rollout_provenance(
                &service_name,
                &rollout_handle,
                false,
                None,
                governance_tx_hash,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::AdvanceSoracloudRollout {
            service_name: service_name.clone(),
            rollout_handle: rollout_handle.clone(),
            healthy: false,
            promote_to_percent: None,
            governance_tx_hash,
            provenance: rollout_provenance(
                &service_name,
                &rollout_handle,
                false,
                None,
                governance_tx_hash,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let deployment = world
            .soracloud_service_deployments()
            .get(&service_name)
            .expect("deployment state");
        let last_rollout = deployment.last_rollout.as_ref().expect("last rollout");
        assert_eq!(deployment.current_service_version, "1.0.0");
        assert!(deployment.active_rollout.is_none());
        assert_eq!(last_rollout.stage, SoraRolloutStageV1::RolledBack);
        assert_eq!(last_rollout.traffic_percent, 0);
        assert_eq!(deployment.process_generation, 3);
        assert_eq!(world.soracloud_service_audit_events().iter().count(), 4);
        Ok(())
    }

    #[test]
    fn rollback_soracloud_service_reuses_admitted_revision() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let deploy_bundle = sample_bundle("portal", "1.0.0", 0);
        let upgrade_bundle = sample_bundle("portal", "1.1.0", 100);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: deploy_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&upgrade_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        isi::RollbackSoracloudService {
            service_name: service_name.clone(),
            target_version: Some("1.0.0".to_string()),
            provenance: rollback_provenance(&service_name, Some("1.0.0")),
        }
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let deployment = world
            .soracloud_service_deployments()
            .get(&service_name)
            .expect("deployment state");
        assert_eq!(deployment.current_service_version, "1.0.0");
        assert_eq!(deployment.revision_count, 2);
        assert_eq!(deployment.process_generation, 3);
        assert_eq!(world.soracloud_service_audit_events().iter().count(), 3);
        Ok(())
    }

    #[test]
    fn mutate_soracloud_state_records_authoritative_service_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::Plaintext,
            SoraStateMutabilityV1::ReadWrite,
            512,
            2_048,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let binding_name: iroha_data_model::name::Name = "vault".parse().expect("valid");
        let governance_tx_hash = Hash::new(b"gov-state");
        let value_payload = vec![0xAB; 256];
        let value_payload_commitment = Hash::new(&value_payload);
        iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: "/state/private/patient-1".to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(256),
            value_payload: Some(value_payload),
            encryption: SoraStateEncryptionV1::Plaintext,
            governance_tx_hash,
            fhe_input_admission_proof: None,
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                "/state/private/patient-1",
                SoraStateMutationOperationV1::Upsert,
                Some(256),
                Some(value_payload_commitment),
                SoraStateEncryptionV1::Plaintext,
                governance_tx_hash,
                None,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let entry = world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                "/state/private/patient-1".to_string(),
            ))
            .expect("service state entry");
        assert_eq!(entry.encryption, SoraStateEncryptionV1::Plaintext);
        assert_eq!(entry.payload_bytes.get(), 256);
        assert_eq!(
            entry.source_action,
            SoraServiceLifecycleActionV1::StateMutation
        );
        let recorded_audit = world
            .soracloud_service_audit_events()
            .get(&2)
            .expect("audit event");
        assert_eq!(
            recorded_audit.action,
            SoraServiceLifecycleActionV1::StateMutation
        );
        assert_eq!(recorded_audit.binding_name.as_ref(), Some(&binding_name));
        assert_eq!(
            recorded_audit.state_key.as_deref(),
            Some("/state/private/patient-1")
        );
        Ok(())
    }

    #[test]
    fn run_soracloud_fhe_job_records_ciphertext_output_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let binding_name: iroha_data_model::name::Name = "vault".parse().expect("valid");
        let input_1_payload = sample_fhe_payload(b"alice", b"seed-1");
        let input_2_payload = sample_fhe_payload(b"bob", b"seed-2");
        let input_residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        for (state_key, payload) in [
            ("/state/private/input-1", input_1_payload.clone()),
            ("/state/private/input-2", input_2_payload.clone()),
        ] {
            record_service_state_entry(
                &mut stx,
                SoraServiceStateEntryV1 {
                    schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: "1.0.0".to_string(),
                    binding_name: binding_name.clone(),
                    state_key: state_key.to_string(),
                    encryption: SoraStateEncryptionV1::FheCiphertext,
                    payload_bytes: NonZeroU64::new(
                        u64::try_from(payload.len()).expect("payload len"),
                    )
                    .expect("nonzero"),
                    payload_commitment: Hash::new(&payload),
                    payload,
                    fhe_residual_multiple_bound: Some(input_residual_bound),
                    fhe_bound_mode: Some(BfvCiphertextBoundModeV1::ExactResidualMultiple),
                    last_update_sequence: 1,
                    governance_tx_hash: Hash::new(b"input-state"),
                    source_action: SoraServiceLifecycleActionV1::StateMutation,
                },
            )?;
        }
        let job = sample_fhe_job(vec![
            sample_fhe_input_ref("/state/private/input-1", &input_1_payload),
            sample_fhe_input_ref("/state/private/input-2", &input_2_payload),
        ]);
        let policy = sample_fhe_policy();
        let param_set = sample_fhe_param_set();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let evaluation_key_refresh_transcript = sample_bfv_refresh_transcript();
        let governance_tx_hash = Hash::new(b"gov-fhe");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            evaluation_keys: evaluation_keys.clone(),
            evaluation_key_refresh_transcript: evaluation_key_refresh_transcript.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job.clone(),
                policy.clone(),
                param_set.clone(),
                evaluation_keys,
                evaluation_key_refresh_transcript,
                governance_tx_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let entry = world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                job.output_state_key.clone(),
            ))
            .expect("fhe output entry");
        assert_eq!(entry.encryption, SoraStateEncryptionV1::FheCiphertext);
        assert_eq!(entry.payload_bytes.get(), entry.payload.len() as u64);
        assert_eq!(entry.payload_commitment, Hash::new(&entry.payload));
        assert_eq!(
            entry.fhe_residual_multiple_bound,
            Some(
                bfv_add_output_residual_multiple_bound(
                    &ram_lfe_bfv_parameters_v1(),
                    &[input_residual_bound, input_residual_bound],
                )
                .expect("add output residual bound")
            )
        );
        assert_eq!(
            entry.fhe_bound_mode,
            Some(BfvCiphertextBoundModeV1::ExactResidualMultiple),
            "exact evaluator outputs must persist their bound semantics"
        );
        assert!(!entry.payload.is_empty());
        assert_eq!(entry.source_action, SoraServiceLifecycleActionV1::FheJobRun);
        assert_eq!(
            world
                .soracloud_service_audit_events()
                .get(&2)
                .expect("audit")
                .action,
            SoraServiceLifecycleActionV1::FheJobRun
        );
        Ok(())
    }

    #[test]
    fn run_soracloud_fhe_job_records_bounded_noise_add_output_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let params = ram_lfe_bfv_parameters_v1();
        let (
            secret_key,
            public_key,
            evaluation_keys,
            evaluation_key_refresh_transcript,
            refresh_digest,
        ) = sample_registered_bounded_noise_bfv_material();
        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let input_1_payload =
            sample_bounded_noise_fhe_payload(&public_key, &[5, 7], "bounded-add-input-1");
        let input_2_payload =
            sample_bounded_noise_fhe_payload(&public_key, &[11, 13], "bounded-add-input-2");
        let fresh_bound =
            bfv_fresh_bounded_noise_ciphertext_bound(&params).expect("fresh bounded-noise bound");
        for (state_key, payload) in [
            ("/state/private/bounded-input-1", input_1_payload.clone()),
            ("/state/private/bounded-input-2", input_2_payload.clone()),
        ] {
            record_service_state_entry(
                &mut stx,
                SoraServiceStateEntryV1 {
                    schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: "1.0.0".to_string(),
                    binding_name: binding_name.clone(),
                    state_key: state_key.to_string(),
                    encryption: SoraStateEncryptionV1::FheCiphertext,
                    payload_bytes: NonZeroU64::new(
                        u64::try_from(payload.len()).expect("payload len"),
                    )
                    .expect("nonzero"),
                    payload_commitment: Hash::new(&payload),
                    payload,
                    fhe_residual_multiple_bound: Some(fresh_bound),
                    fhe_bound_mode: Some(BfvCiphertextBoundModeV1::BoundedNoise),
                    last_update_sequence: 1,
                    governance_tx_hash: Hash::new(b"bounded-input-state"),
                    source_action: SoraServiceLifecycleActionV1::StateMutation,
                },
            )?;
        }

        let job = sample_fhe_job(vec![
            sample_fhe_input_ref("/state/private/bounded-input-1", &input_1_payload),
            sample_fhe_input_ref("/state/private/bounded-input-2", &input_2_payload),
        ]);
        let mut policy = sample_fhe_policy();
        policy.evaluation_key_digest = evaluation_keys
            .digest(&params)
            .expect("bounded-noise evaluation-key digest");
        policy.evaluation_key_refresh_transcript_digest = refresh_digest;
        policy.refresh_transcript_mode = BfvRefreshTranscriptModeV1::BoundedNoise;
        let param_set = sample_fhe_param_set();
        let governance_tx_hash = Hash::new(b"gov-fhe-bounded-add");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            evaluation_keys: evaluation_keys.clone(),
            evaluation_key_refresh_transcript: evaluation_key_refresh_transcript.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job.clone(),
                policy,
                param_set,
                evaluation_keys,
                evaluation_key_refresh_transcript,
                governance_tx_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let entry = world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                job.output_state_key.clone(),
            ))
            .expect("bounded FHE output entry");
        assert_eq!(entry.encryption, SoraStateEncryptionV1::FheCiphertext);
        assert_eq!(
            entry.fhe_residual_multiple_bound,
            Some(
                bfv_add_bounded_noise_output_bound(&params, &[fresh_bound, fresh_bound])
                    .expect("bounded add output noise bound")
            )
        );
        assert_eq!(
            entry.fhe_bound_mode,
            Some(BfvCiphertextBoundModeV1::BoundedNoise),
            "bounded evaluator outputs must persist bounded-noise semantics"
        );

        let output = decode_soracloud_fhe_envelope(&entry.payload)?;
        let plaintext = output
            .slots
            .iter()
            .map(|slot| {
                decrypt_bounded_noise(&params, &secret_key, slot)
                    .expect("decrypt bounded output slot")[0]
            })
            .collect::<Vec<_>>();
        assert_eq!(plaintext, vec![16, 20]);
        Ok(())
    }

    #[test]
    fn run_soracloud_fhe_job_rejects_client_mutated_fhe_input_without_residual_metadata()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let binding_name: iroha_data_model::name::Name = "vault".parse().expect("valid");
        let input_1_payload = sample_fhe_payload(b"alice", b"seed-missing-bound-1");
        let input_2_payload = sample_fhe_payload(b"bob", b"seed-missing-bound-2");
        for (state_key, payload, governance_seed) in [
            (
                "/state/private/input-1",
                input_1_payload.clone(),
                b"gov-fhe-input-1".as_slice(),
            ),
            (
                "/state/private/input-2",
                input_2_payload.clone(),
                b"gov-fhe-input-2".as_slice(),
            ),
        ] {
            let governance_tx_hash = Hash::new(governance_seed);
            iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
                service_name: service_name.clone(),
                binding_name: binding_name.clone(),
                state_key: state_key.to_string(),
                operation: SoraStateMutationOperationV1::Upsert,
                value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
                value_payload: Some(payload.clone()),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                fhe_input_admission_proof: None,
                provenance: state_mutation_provenance(
                    &service_name,
                    &binding_name,
                    state_key,
                    SoraStateMutationOperationV1::Upsert,
                    Some(u64::try_from(payload.len()).expect("payload len")),
                    Some(Hash::new(&payload)),
                    SoraStateEncryptionV1::FheCiphertext,
                    governance_tx_hash,
                    None,
                ),
            })
            .execute(&ALICE_ID, &mut stx)?;
        }

        let job = sample_fhe_job(vec![
            sample_fhe_input_ref("/state/private/input-1", &input_1_payload),
            sample_fhe_input_ref("/state/private/input-2", &input_2_payload),
        ]);
        let policy = sample_fhe_policy();
        let param_set = sample_fhe_param_set();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let evaluation_key_refresh_transcript = sample_bfv_refresh_transcript();
        let governance_tx_hash = Hash::new(b"gov-fhe-missing-bound");
        let err = iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            evaluation_keys: evaluation_keys.clone(),
            evaluation_key_refresh_transcript: evaluation_key_refresh_transcript.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job,
                policy,
                param_set,
                evaluation_keys,
                evaluation_key_refresh_transcript,
                governance_tx_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("client-mutated FHE inputs without residual metadata must fail closed");
        assert_invalid_parameter_contains(err, "missing exact BFV residual metadata");
        Ok(())
    }

    #[test]
    fn run_soracloud_fhe_job_rejects_bounded_noise_persisted_fhe_input() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let input_key = "/state/private/bounded-input";
        let input_payload = sample_fhe_payload(b"alice", b"seed-bounded-noise-job-input");
        let input_residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        record_service_state_entry(
            &mut stx,
            SoraServiceStateEntryV1 {
                schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                binding_name: binding_name.clone(),
                state_key: input_key.to_string(),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                payload_bytes: NonZeroU64::new(
                    u64::try_from(input_payload.len()).expect("payload len"),
                )
                .expect("nonzero"),
                payload_commitment: Hash::new(&input_payload),
                payload: input_payload.clone(),
                fhe_residual_multiple_bound: Some(input_residual_bound),
                fhe_bound_mode: Some(BfvCiphertextBoundModeV1::BoundedNoise),
                last_update_sequence: 1,
                governance_tx_hash: Hash::new(b"bounded-noise-input-state"),
                source_action: SoraServiceLifecycleActionV1::StateMutation,
            },
        )?;

        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(input_key, &input_payload)]);
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = 1;
        let policy = sample_fhe_policy();
        let param_set = sample_fhe_param_set();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let evaluation_key_refresh_transcript = sample_bfv_refresh_transcript();
        let governance_tx_hash = Hash::new(b"gov-fhe-bounded-input");
        let err = iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            evaluation_keys: evaluation_keys.clone(),
            evaluation_key_refresh_transcript: evaluation_key_refresh_transcript.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job,
                policy,
                param_set,
                evaluation_keys,
                evaluation_key_refresh_transcript,
                governance_tx_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("bounded-noise FHE inputs must fail closed for the exact evaluator");
        assert_invalid_parameter_contains(err, "not annotated with exact BFV residual metadata");
        Ok(())
    }

    #[test]
    fn run_soracloud_fhe_job_rejects_oversized_persisted_fhe_input_envelope()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let input_key = "/state/private/oversized-input";
        let input_payload = sample_oversized_fhe_payload(b"alice", b"seed-oversized-job-input");
        let input_residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        record_service_state_entry(
            &mut stx,
            SoraServiceStateEntryV1 {
                schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                binding_name: binding_name.clone(),
                state_key: input_key.to_string(),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                payload_bytes: NonZeroU64::new(
                    u64::try_from(input_payload.len()).expect("payload len"),
                )
                .expect("nonzero"),
                payload_commitment: Hash::new(&input_payload),
                payload: input_payload.clone(),
                fhe_residual_multiple_bound: Some(input_residual_bound),
                fhe_bound_mode: Some(BfvCiphertextBoundModeV1::ExactResidualMultiple),
                last_update_sequence: 1,
                governance_tx_hash: Hash::new(b"oversized-input-state"),
                source_action: SoraServiceLifecycleActionV1::StateMutation,
            },
        )?;

        let mut job = sample_fhe_job(vec![sample_fhe_input_ref(input_key, &input_payload)]);
        job.operation = FheJobOperationV1::Bootstrap;
        job.bootstrap_count = 1;
        let policy = sample_fhe_policy();
        let param_set = sample_fhe_param_set();
        let evaluation_keys = sample_bfv_evaluation_key_bundle();
        let evaluation_key_refresh_transcript = sample_bfv_refresh_transcript();
        let governance_tx_hash = Hash::new(b"gov-fhe-oversized-input");
        let err = iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            evaluation_keys: evaluation_keys.clone(),
            evaluation_key_refresh_transcript: evaluation_key_refresh_transcript.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job,
                policy,
                param_set,
                evaluation_keys,
                evaluation_key_refresh_transcript,
                governance_tx_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("oversized persisted FHE input envelopes must fail before execution");
        assert_invalid_parameter_contains(err, "slot count");
        Ok(())
    }

    #[test]
    fn mutate_soracloud_state_rejects_bounded_noise_fhe_input_admission_proof()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let state_key = "/state/private/input-bounded-proof";
        let payload = sample_fhe_payload(b"alice", b"seed-proof-bounded-noise");
        let governance_tx_hash = Hash::new(b"gov-fhe-input-proof-bounded-noise");
        let residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        let mut admission_proof = sample_fhe_input_admission_proof(
            &service_name,
            &binding_name,
            state_key,
            &payload,
            governance_tx_hash,
            residual_bound,
        );
        admission_proof.bound_mode = BfvCiphertextBoundModeV1::BoundedNoise;

        let err = iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: state_key.to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
            value_payload: Some(payload.clone()),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: Some(admission_proof.clone()),
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                state_key,
                SoraStateMutationOperationV1::Upsert,
                Some(u64::try_from(payload.len()).expect("payload len")),
                Some(Hash::new(&payload)),
                SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                Some(admission_proof),
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("bounded-noise FHE input admission must fail closed");

        assert_invalid_parameter_contains(err, "bounded-noise FHE input admission proofs");
        assert!(
            stx.world
                .soracloud_service_state_entries
                .get(&(
                    service_name.as_ref().to_owned(),
                    binding_name.as_ref().to_owned(),
                    state_key.to_string(),
                ))
                .is_none(),
            "bounded-noise FHE input admission must not persist state"
        );
        Ok(())
    }

    #[test]
    fn mutate_soracloud_state_rejects_fhe_input_admission_proof_without_registered_verifier()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let state_key = "/state/private/input-1";
        let payload = sample_fhe_payload(b"alice", b"seed-proof-missing-vk");
        let governance_tx_hash = Hash::new(b"gov-fhe-input-proof");
        let residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        let admission_proof = sample_fhe_input_admission_proof(
            &service_name,
            &binding_name,
            state_key,
            &payload,
            governance_tx_hash,
            residual_bound,
        );

        let err = iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: state_key.to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
            value_payload: Some(payload.clone()),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: Some(admission_proof.clone()),
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                state_key,
                SoraStateMutationOperationV1::Upsert,
                Some(u64::try_from(payload.len()).expect("payload len")),
                Some(Hash::new(&payload)),
                SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                Some(admission_proof),
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("unregistered proof verifier must reject FHE input admission");

        assert_invariant_contains(err, "FHE input admission verifying key not found");
        assert!(
            stx.world
                .soracloud_service_state_entries
                .get(&(
                    service_name.as_ref().to_owned(),
                    binding_name.as_ref().to_owned(),
                    state_key.to_string(),
                ))
                .is_none(),
            "failed admission must not persist FHE input state"
        );
        Ok(())
    }

    #[test]
    fn mutate_soracloud_state_rejects_oversized_fhe_input_admission_envelope()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let state_key = "/state/private/input-oversized";
        let payload = sample_oversized_fhe_payload(b"alice", b"seed-proof-oversized-envelope");
        let governance_tx_hash = Hash::new(b"gov-fhe-input-proof-oversized-envelope");
        let residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        let admission_proof = sample_fhe_input_admission_proof(
            &service_name,
            &binding_name,
            state_key,
            &payload,
            governance_tx_hash,
            residual_bound,
        );

        let err = iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: state_key.to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
            value_payload: Some(payload.clone()),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: Some(admission_proof.clone()),
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                state_key,
                SoraStateMutationOperationV1::Upsert,
                Some(u64::try_from(payload.len()).expect("payload len")),
                Some(Hash::new(&payload)),
                SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                Some(admission_proof),
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("oversized FHE input envelopes must fail before verifier lookup");

        assert_invalid_parameter_contains(err, "slot count");
        assert!(
            stx.world
                .soracloud_service_state_entries
                .get(&(
                    service_name.as_ref().to_owned(),
                    binding_name.as_ref().to_owned(),
                    state_key.to_string(),
                ))
                .is_none(),
            "oversized FHE input admission must not persist state"
        );
        Ok(())
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn mutate_soracloud_state_accepts_registered_fhe_input_admission_proof()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        stx.zk.stark.enabled = true;

        let vk_box = sample_fhe_input_admission_vk_box();
        let vk_id = register_fhe_input_admission_verifier(&mut stx, vk_box.clone())?;
        assert_eq!(
            vk_id,
            iroha_data_model::proof::VerifyingKeyId::new(
                FHE_INPUT_ADMISSION_BACKEND,
                FHE_INPUT_ADMISSION_CIRCUIT_ID,
            )
        );

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let state_key = "/state/private/input-verified";
        let payload = sample_fhe_payload(b"alice", b"seed-proof-registered-vk");
        let governance_tx_hash = Hash::new(b"gov-fhe-input-proof-registered");
        let residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        let admission_proof = sample_verified_fhe_input_admission_proof(
            &service_name,
            &binding_name,
            state_key,
            &payload,
            governance_tx_hash,
            residual_bound,
            &vk_box,
        );

        iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: state_key.to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
            value_payload: Some(payload.clone()),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: Some(admission_proof.clone()),
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                state_key,
                SoraStateMutationOperationV1::Upsert,
                Some(u64::try_from(payload.len()).expect("payload len")),
                Some(Hash::new(&payload)),
                SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                Some(admission_proof),
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let entry = world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                state_key.to_string(),
            ))
            .expect("admitted FHE state entry");
        assert_eq!(entry.encryption, SoraStateEncryptionV1::FheCiphertext);
        assert_eq!(entry.payload_commitment, Hash::new(&payload));
        assert_eq!(
            entry.fhe_residual_multiple_bound,
            Some(residual_bound),
            "verified FHE input admission must persist the proven residual bound"
        );
        assert_eq!(
            entry.fhe_bound_mode,
            Some(BfvCiphertextBoundModeV1::ExactResidualMultiple),
            "verified FHE input admission must persist exact bound semantics"
        );
        assert_eq!(
            world
                .verifying_keys()
                .get(&vk_id)
                .expect("registered input-admission verifier")
                .namespace,
            "soracloud"
        );
        Ok(())
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_circuit()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::FheCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            131_072,
            262_144,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();
        stx.zk.stark.enabled = true;

        let wrong_circuit_id = "soracloud_fhe_input_admission_shadow_v1";
        let vk_box = sample_fhe_input_admission_vk_box_for_circuit(wrong_circuit_id);
        let vk_id =
            register_fhe_input_admission_verifier_for_circuit(&mut stx, vk_box, wrong_circuit_id)?;
        assert_eq!(
            vk_id,
            iroha_data_model::proof::VerifyingKeyId::new(
                FHE_INPUT_ADMISSION_BACKEND,
                FHE_INPUT_ADMISSION_CIRCUIT_ID,
            )
        );

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: Name = "portal".parse().expect("valid");
        let binding_name: Name = "vault".parse().expect("valid");
        let state_key = "/state/private/input-wrong-circuit";
        let payload = sample_fhe_payload(b"alice", b"seed-proof-wrong-circuit");
        let governance_tx_hash = Hash::new(b"gov-fhe-input-proof-wrong-circuit");
        let residual_bound =
            bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                .expect("fresh input residual bound");
        let admission_proof = sample_fhe_input_admission_proof(
            &service_name,
            &binding_name,
            state_key,
            &payload,
            governance_tx_hash,
            residual_bound,
        );

        let err = iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: state_key.to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
            value_payload: Some(payload.clone()),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: Some(admission_proof.clone()),
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                state_key,
                SoraStateMutationOperationV1::Upsert,
                Some(u64::try_from(payload.len()).expect("payload len")),
                Some(Hash::new(&payload)),
                SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                Some(admission_proof),
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("wrong input-admission circuit must fail closed");

        assert_invariant_contains(err, "canonical v1 circuit");
        assert!(
            stx.world
                .soracloud_service_state_entries
                .get(&(
                    service_name.as_ref().to_owned(),
                    binding_name.as_ref().to_owned(),
                    state_key.to_string(),
                ))
                .is_none(),
            "wrong-circuit admission must not persist FHE input state"
        );
        Ok(())
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn mutate_soracloud_state_rejects_restored_fhe_input_verifier_metadata_drift()
    -> Result<(), eyre::Report> {
        enum VerifierTamper {
            Curve,
            VkLen,
        }

        for (tamper, expected_error, state_key, seed) in [
            (
                VerifierTamper::Curve,
                "goldilocks STARK field",
                "/state/private/input-wrong-field",
                b"seed-proof-wrong-field".as_slice(),
            ),
            (
                VerifierTamper::VkLen,
                "vk_len mismatch",
                "/state/private/input-wrong-vk-len",
                b"seed-proof-wrong-vk-len".as_slice(),
            ),
        ] {
            let kura = Kura::blank_kura_for_testing();
            let state = state_with_soracloud_permission(&kura)?;
            let bundle = sample_bundle_with_state_binding(
                "portal",
                "1.0.0",
                0,
                "vault",
                "/state/private",
                SoraStateEncryptionV1::FheCiphertext,
                SoraStateMutabilityV1::ReadWrite,
                131_072,
                262_144,
            );
            let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
                .as_ref()
                .header();
            let mut state_block = state.block(block_header);
            let mut stx = state_block.transaction();
            stx.zk.stark.enabled = true;

            let vk_box = sample_fhe_input_admission_vk_box();
            let vk_id = register_fhe_input_admission_verifier(&mut stx, vk_box.clone())?;
            match tamper {
                VerifierTamper::Curve => {
                    stx.world
                        .verifying_keys
                        .get_mut(&vk_id)
                        .expect("registered verifier")
                        .curve = "bn254".to_string();
                }
                VerifierTamper::VkLen => {
                    stx.world
                        .verifying_keys
                        .get_mut(&vk_id)
                        .expect("registered verifier")
                        .vk_len = u32::try_from(vk_box.bytes.len())
                        .expect("VK length fits")
                        .saturating_add(1);
                }
            }

            isi::DeploySoracloudService {
                bundle: bundle.clone(),
                initial_service_configs: BTreeMap::new(),
                initial_service_secrets: BTreeMap::new(),
                provenance: bundle_provenance(&bundle),
            }
            .execute(&ALICE_ID, &mut stx)?;

            let service_name: Name = "portal".parse().expect("valid");
            let binding_name: Name = "vault".parse().expect("valid");
            let payload = sample_fhe_payload(b"alice", seed);
            let governance_tx_hash = Hash::new(state_key.as_bytes());
            let residual_bound =
                bfv_encrypted_zero_refresh_residual_multiple_bound(&ram_lfe_bfv_parameters_v1())
                    .expect("fresh input residual bound");
            let admission_proof = sample_verified_fhe_input_admission_proof(
                &service_name,
                &binding_name,
                state_key,
                &payload,
                governance_tx_hash,
                residual_bound,
                &vk_box,
            );

            let err = iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
                service_name: service_name.clone(),
                binding_name: binding_name.clone(),
                state_key: state_key.to_string(),
                operation: SoraStateMutationOperationV1::Upsert,
                value_size_bytes: Some(u64::try_from(payload.len()).expect("payload len")),
                value_payload: Some(payload.clone()),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash,
                fhe_input_admission_proof: Some(admission_proof.clone()),
                provenance: state_mutation_provenance(
                    &service_name,
                    &binding_name,
                    state_key,
                    SoraStateMutationOperationV1::Upsert,
                    Some(u64::try_from(payload.len()).expect("payload len")),
                    Some(Hash::new(&payload)),
                    SoraStateEncryptionV1::FheCiphertext,
                    governance_tx_hash,
                    Some(admission_proof),
                ),
            })
            .execute(&ALICE_ID, &mut stx)
            .expect_err("restored verifier metadata drift must fail closed");

            assert_invariant_contains(err, expected_error);
            assert!(
                stx.world
                    .soracloud_service_state_entries
                    .get(&(
                        service_name.as_ref().to_owned(),
                        binding_name.as_ref().to_owned(),
                        state_key.to_string(),
                    ))
                    .is_none(),
                "metadata-drifted verifier must not persist FHE input state"
            );
        }
        Ok(())
    }

    #[test]
    fn record_soracloud_decryption_request_persists_policy_snapshot() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_bundle_with_state_binding(
            "portal",
            "1.0.0",
            0,
            "vault",
            "/state/private",
            SoraStateEncryptionV1::ClientCiphertext,
            SoraStateMutabilityV1::ReadWrite,
            4_096,
            16_384,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let policy = sample_decryption_policy();
        let request = sample_decryption_request();
        iroha_data_model::isi::InstructionBox::from(isi::RecordSoracloudDecryptionRequest {
            service_name: service_name.clone(),
            policy: policy.clone(),
            request: request.clone(),
            provenance: decryption_request_provenance(
                &service_name,
                policy.clone(),
                request.clone(),
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_decryption_request_records()
            .get(&(service_name.as_ref().to_owned(), request.request_id.clone()))
            .expect("decryption request record");
        assert_eq!(record.service_version, "1.0.0");
        assert_eq!(record.policy.policy_name, policy.policy_name);
        assert_eq!(
            record.policy_snapshot_hash(),
            Hash::new(Encode::encode(&policy))
        );
        let audit = world
            .soracloud_service_audit_events()
            .get(&2)
            .expect("audit event");
        assert_eq!(
            audit.action,
            SoraServiceLifecycleActionV1::DecryptionRequest
        );
        assert_eq!(audit.policy_name.as_ref(), Some(&policy.policy_name));
        assert_eq!(
            audit.policy_snapshot_hash,
            Some(record.policy_snapshot_hash())
        );
        Ok(())
    }

    #[test]
    fn start_soracloud_training_job_records_authoritative_job_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        iroha_data_model::isi::InstructionBox::from(isi::StartSoracloudTrainingJob {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            storage_budget_bytes: 8_192,
            provenance: training_start_provenance(
                &service_name,
                "vision_model",
                "job-1",
                4,
                100,
                20,
                3,
                50,
                40_000,
                8_192,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_training_jobs()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .expect("training job");
        assert_eq!(record.model_name, "vision_model");
        assert_eq!(record.service_version, "1.0.0");
        assert_eq!(record.status, SoraTrainingJobStatusV1::Running);
        assert_eq!(record.created_sequence, 2);
        assert_eq!(
            world
                .soracloud_training_job_audit_events()
                .get(&2)
                .expect("training audit")
                .action,
            SoraTrainingJobActionV1::Start
        );
        Ok(())
    }

    #[test]
    fn checkpoint_soracloud_training_job_updates_authoritative_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        iroha_data_model::isi::InstructionBox::from(isi::StartSoracloudTrainingJob {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            storage_budget_bytes: 8_192,
            provenance: training_start_provenance(
                &service_name,
                "vision_model",
                "job-1",
                4,
                100,
                20,
                3,
                50,
                40_000,
                8_192,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        let metrics_hash = Hash::new(b"metrics");
        iroha_data_model::isi::InstructionBox::from(isi::CheckpointSoracloudTrainingJob {
            service_name: service_name.clone(),
            job_id: "job-1".to_string(),
            completed_step: 100,
            checkpoint_size_bytes: 1_024,
            metrics_hash,
            provenance: training_checkpoint_provenance(
                &service_name,
                "job-1",
                100,
                1_024,
                metrics_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_training_jobs()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .expect("training job");
        assert_eq!(record.status, SoraTrainingJobStatusV1::Completed);
        assert_eq!(record.completed_steps, 100);
        assert_eq!(record.checkpoint_count, 1);
        assert_eq!(record.compute_consumed_units, 20_000);
        assert_eq!(record.storage_consumed_bytes, 1_024);
        assert_eq!(record.latest_metrics_hash, Some(metrics_hash));
        Ok(())
    }

    #[test]
    fn retry_soracloud_training_job_records_retry_pending_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        iroha_data_model::isi::InstructionBox::from(isi::StartSoracloudTrainingJob {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            storage_budget_bytes: 8_192,
            provenance: training_start_provenance(
                &service_name,
                "vision_model",
                "job-1",
                4,
                100,
                20,
                3,
                50,
                40_000,
                8_192,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        iroha_data_model::isi::InstructionBox::from(isi::RetrySoracloudTrainingJob {
            service_name: service_name.clone(),
            job_id: "job-1".to_string(),
            reason: "worker unavailable".to_string(),
            provenance: training_retry_provenance(&service_name, "job-1", "worker unavailable"),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_training_jobs()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .expect("training job");
        assert_eq!(record.status, SoraTrainingJobStatusV1::RetryPending);
        assert_eq!(record.retry_count, 1);
        assert_eq!(
            record.last_failure_reason.as_deref(),
            Some("worker unavailable")
        );
        assert_eq!(
            world
                .soracloud_training_job_audit_events()
                .get(&3)
                .expect("training audit")
                .action,
            SoraTrainingJobActionV1::Retry
        );
        Ok(())
    }

    #[test]
    fn register_soracloud_model_artifact_records_authoritative_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        iroha_data_model::isi::InstructionBox::from(isi::StartSoracloudTrainingJob {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            storage_budget_bytes: 8_192,
            provenance: training_start_provenance(
                &service_name,
                "vision_model",
                "job-1",
                4,
                100,
                20,
                3,
                50,
                40_000,
                8_192,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        let metrics_hash = Hash::new(b"metrics");
        iroha_data_model::isi::InstructionBox::from(isi::CheckpointSoracloudTrainingJob {
            service_name: service_name.clone(),
            job_id: "job-1".to_string(),
            completed_step: 100,
            checkpoint_size_bytes: 1_024,
            metrics_hash,
            provenance: training_checkpoint_provenance(
                &service_name,
                "job-1",
                100,
                1_024,
                metrics_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let weight_artifact_hash = Hash::new(b"weights");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"prov");
        iroha_data_model::isi::InstructionBox::from(isi::RegisterSoracloudModelArtifact {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            training_job_id: "job-1".to_string(),
            weight_artifact_hash,
            dataset_ref: "dataset://train".to_string(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: model_artifact_provenance(
                &service_name,
                "vision_model",
                "job-1",
                weight_artifact_hash,
                "dataset://train",
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let artifact = world
            .soracloud_model_artifacts()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .expect("artifact record");
        assert_eq!(artifact.model_name, "vision_model");
        assert_eq!(artifact.dataset_ref, "dataset://train");
        assert!(artifact.consumed_by_version.is_none());
        assert_eq!(
            world
                .soracloud_model_artifact_audit_events()
                .get(&4)
                .expect("artifact audit")
                .action,
            SoraModelArtifactActionV1::Register
        );
        Ok(())
    }

    #[test]
    fn model_weight_lifecycle_updates_authoritative_registry_state() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        iroha_data_model::isi::InstructionBox::from(isi::StartSoracloudTrainingJob {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            storage_budget_bytes: 8_192,
            provenance: training_start_provenance(
                &service_name,
                "vision_model",
                "job-1",
                4,
                100,
                20,
                3,
                50,
                40_000,
                8_192,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        let metrics_hash = Hash::new(b"metrics");
        iroha_data_model::isi::InstructionBox::from(isi::CheckpointSoracloudTrainingJob {
            service_name: service_name.clone(),
            job_id: "job-1".to_string(),
            completed_step: 100,
            checkpoint_size_bytes: 1_024,
            metrics_hash,
            provenance: training_checkpoint_provenance(
                &service_name,
                "job-1",
                100,
                1_024,
                metrics_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let weight_artifact_hash = Hash::new(b"weights");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"prov");
        iroha_data_model::isi::InstructionBox::from(isi::RegisterSoracloudModelArtifact {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            training_job_id: "job-1".to_string(),
            weight_artifact_hash,
            dataset_ref: "dataset://train".to_string(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: model_artifact_provenance(
                &service_name,
                "vision_model",
                "job-1",
                weight_artifact_hash,
                "dataset://train",
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        iroha_data_model::isi::InstructionBox::from(isi::RegisterSoracloudModelWeight {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            weight_version: "v2".to_string(),
            training_job_id: "job-1".to_string(),
            parent_version: None,
            weight_artifact_hash,
            dataset_ref: "dataset://train".to_string(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: model_weight_register_provenance(
                &service_name,
                "vision_model",
                "v2",
                "job-1",
                None,
                weight_artifact_hash,
                "dataset://train",
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let gate_report_hash = Hash::new(b"gate");
        iroha_data_model::isi::InstructionBox::from(isi::PromoteSoracloudModelWeight {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            weight_version: "v2".to_string(),
            gate_approved: true,
            gate_report_hash,
            provenance: model_weight_promote_provenance(
                &service_name,
                "vision_model",
                "v2",
                true,
                gate_report_hash,
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;
        iroha_data_model::isi::InstructionBox::from(isi::RollbackSoracloudModelWeight {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            target_version: "v2".to_string(),
            reason: "reaffirm baseline".to_string(),
            provenance: model_weight_rollback_provenance(
                &service_name,
                "vision_model",
                "v2",
                "reaffirm baseline",
            ),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("rollback to current version should fail");

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let registry = world
            .soracloud_model_registries()
            .get(&(service_name.as_ref().to_owned(), "vision_model".to_string()))
            .expect("model registry");
        let version = world
            .soracloud_model_weight_versions()
            .get(&(
                service_name.as_ref().to_owned(),
                "vision_model".to_string(),
                "v2".to_string(),
            ))
            .expect("model version");
        let artifact = world
            .soracloud_model_artifacts()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .expect("artifact record");
        assert_eq!(registry.current_version.as_deref(), Some("v2"));
        assert_eq!(version.training_job_id, "job-1");
        assert_eq!(version.promoted_sequence, Some(6));
        assert_eq!(version.gate_report_hash, Some(gate_report_hash));
        assert_eq!(artifact.consumed_by_version.as_deref(), Some("v2"));
        assert_eq!(
            world
                .soracloud_model_weight_audit_events()
                .get(&5)
                .expect("weight register audit")
                .action,
            SoraModelWeightActionV1::Register
        );
        assert_eq!(
            world
                .soracloud_model_weight_audit_events()
                .get(&6)
                .expect("weight promote audit")
                .action,
            SoraModelWeightActionV1::Promote
        );
        Ok(())
    }

    #[test]
    fn rollback_soracloud_model_weight_updates_authoritative_registry_state()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let bundle = sample_training_bundle("portal", "1.0.0");
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        record_model_registry(
            &mut stx,
            SoraModelRegistryV1 {
                schema_version: SORA_MODEL_REGISTRY_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                model_name: "vision_model".to_string(),
                current_version: Some("v2".to_string()),
                updated_sequence: 4,
            },
        )?;
        record_model_weight_version(
            &mut stx,
            SoraModelWeightVersionRecordV1 {
                schema_version: SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                model_name: "vision_model".to_string(),
                weight_version: "v1".to_string(),
                parent_version: None,
                training_job_id: "job-1".to_string(),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::TrainingJob,
                    id: "job-1".to_string(),
                }),
                weight_artifact_hash: Hash::new(b"weights-v1"),
                dataset_ref: "dataset://train".to_string(),
                training_config_hash: Hash::new(b"train-config-v1"),
                reproducibility_hash: Hash::new(b"repro-v1"),
                provenance_attestation_hash: Hash::new(b"prov-v1"),
                registered_sequence: 2,
                promoted_sequence: Some(2),
                gate_report_hash: Some(Hash::new(b"gate-v1")),
                promoted_by: Some(ALICE_KEYPAIR.public_key().clone()),
            },
        )?;
        record_model_weight_version(
            &mut stx,
            SoraModelWeightVersionRecordV1 {
                schema_version: SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                model_name: "vision_model".to_string(),
                weight_version: "v2".to_string(),
                parent_version: Some("v1".to_string()),
                training_job_id: "job-2".to_string(),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::TrainingJob,
                    id: "job-2".to_string(),
                }),
                weight_artifact_hash: Hash::new(b"weights-v2"),
                dataset_ref: "dataset://train".to_string(),
                training_config_hash: Hash::new(b"train-config-v2"),
                reproducibility_hash: Hash::new(b"repro-v2"),
                provenance_attestation_hash: Hash::new(b"prov-v2"),
                registered_sequence: 3,
                promoted_sequence: Some(4),
                gate_report_hash: Some(Hash::new(b"gate-v2")),
                promoted_by: Some(ALICE_KEYPAIR.public_key().clone()),
            },
        )?;
        record_model_weight_audit_event(
            &mut stx,
            SoraModelWeightAuditEventV1 {
                schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence: 4,
                action: SoraModelWeightActionV1::Promote,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                model_name: "vision_model".to_string(),
                target_version: "v2".to_string(),
                current_version: Some("v2".to_string()),
                parent_version: Some("v1".to_string()),
                gate_approved: Some(true),
                rollback_reason: None,
                signer: ALICE_KEYPAIR.public_key().clone(),
            },
        )?;

        iroha_data_model::isi::InstructionBox::from(isi::RollbackSoracloudModelWeight {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            target_version: "v1".to_string(),
            reason: "revert".to_string(),
            provenance: model_weight_rollback_provenance(
                &service_name,
                "vision_model",
                "v1",
                "revert",
            ),
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let registry = world
            .soracloud_model_registries()
            .get(&(service_name.as_ref().to_owned(), "vision_model".to_string()))
            .expect("model registry");
        assert_eq!(registry.current_version.as_deref(), Some("v1"));
        assert_eq!(registry.updated_sequence, 5);
        assert_eq!(
            world
                .soracloud_model_weight_audit_events()
                .get(&5)
                .expect("rollback audit")
                .action,
            SoraModelWeightActionV1::Rollback
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_uses_approved_sorafs_pin_without_storing_chunks()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let service_bundle = sample_training_bundle("portal", "1.0.0");
        isi::DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&service_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let digest = ManifestDigest::new([0xA5; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let bundle_key = (
            "portal".to_string(),
            "vision_model".to_string(),
            "v1".to_string(),
        );
        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&bundle_key)
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_missing_pending_or_retired_sorafs_pin()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let service_bundle = sample_training_bundle("portal", "1.0.0");
        isi::DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&service_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let missing_digest = ManifestDigest::new([0xB5; 32]);
        let missing_bundle = sample_uploaded_model_bundle("portal", missing_digest);
        let missing_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: missing_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&missing_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(missing_result.is_err());

        let pending_digest = ManifestDigest::new([0xC4; 32]);
        insert_uploaded_model_pin(&mut stx, pending_digest, PinStatus::Pending);
        let pending_bundle = sample_uploaded_model_bundle("portal", pending_digest);
        let pending_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: pending_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&pending_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(pending_result.is_err());

        let retired_digest = ManifestDigest::new([0xC5; 32]);
        insert_uploaded_model_pin(&mut stx, retired_digest, PinStatus::Retired(9));
        let retired_bundle = sample_uploaded_model_bundle("portal", retired_digest);
        let retired_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: retired_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&retired_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(retired_result.is_err());
        Ok(())
    }

    #[test]
    fn private_uploaded_model_execution_receipt_persists_only_for_deterministic_runtime()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let model_digest = ManifestDigest::new([0xD1; 32]);
        let mut bundle = sample_uploaded_model_bundle("portal", model_digest);
        bundle.runtime_format =
            iroha_data_model::soracloud::SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1;
        stx.world.soracloud_uploaded_model_bundles.insert(
            (
                bundle.service_name.as_ref().to_owned(),
                bundle.model_id.clone(),
                bundle.weight_version.clone(),
            ),
            bundle.clone(),
        );

        let artifact = |role: &str, byte: u8| SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([byte; 32]),
            artifact_hash: Hash::new([byte; 32]),
            ciphertext_bytes: 64,
            artifact_role: role.to_string(),
        };
        let receipt = SoraPrivateUploadedModelExecutionReceiptV1 {
            schema_version:
                iroha_data_model::soracloud::SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
            receipt_id: Hash::new(b"private-receipt-ok"),
            service_name: bundle.service_name.clone(),
            model_id: bundle.model_id.clone(),
            weight_version: bundle.weight_version.clone(),
            runtime_version:
                crate::soracloud_runtime::SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_string(),
            model_manifest_digest: bundle.sorafs_manifest_digest,
            model_bundle_root: bundle.bundle_root,
            policy_id: bundle.decryption_policy_ref.clone(),
            input_artifact: artifact("input", 0xD2),
            output_artifact: artifact("output", 0xD3),
            input_commitment: Hash::new(b"input-commitment"),
            output_commitment: Hash::new(b"output-commitment"),
            request_commitment: Hash::new(b"request-commitment"),
            result_commitment: Hash::new(b"result-commitment"),
            emitted_sequence: 1,
        };

        write_soracloud_private_uploaded_model_execution_receipt(&mut stx, receipt.clone())?;
        assert!(
            stx.world
                .soracloud_private_uploaded_model_execution_receipts
                .get(&receipt.receipt_id)
                .is_some()
        );

        let mut nondeterministic_bundle = bundle.clone();
        nondeterministic_bundle.model_id = "vision_model_hf".to_string();
        nondeterministic_bundle.runtime_format =
            iroha_data_model::soracloud::SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors;
        stx.world.soracloud_uploaded_model_bundles.insert(
            (
                nondeterministic_bundle.service_name.as_ref().to_owned(),
                nondeterministic_bundle.model_id.clone(),
                nondeterministic_bundle.weight_version.clone(),
            ),
            nondeterministic_bundle.clone(),
        );
        let mut rejected = receipt;
        rejected.receipt_id = Hash::new(b"private-receipt-rejected");
        rejected.model_id = nondeterministic_bundle.model_id;
        let err = write_soracloud_private_uploaded_model_execution_receipt(&mut stx, rejected)
            .expect_err("non-deterministic private runtime bundle must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(_)
        ));
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_adversarial_sorafs_pin_metadata()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let digest_mismatch = ManifestDigest::new([0xCA; 32]);
        let digest_mismatch_bundle = sample_uploaded_model_bundle("portal", digest_mismatch);
        insert_uploaded_model_pin_record(
            &mut stx,
            digest_mismatch,
            ManifestDigest::new([0xCB; 32]),
            digest_mismatch_bundle.ciphertext_bytes,
            PinStatus::Approved(1),
        );
        let digest_mismatch_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: digest_mismatch_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&digest_mismatch_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(digest_mismatch_result.is_err());

        let length_mismatch = ManifestDigest::new([0xCC; 32]);
        let length_mismatch_bundle = sample_uploaded_model_bundle("portal", length_mismatch);
        insert_uploaded_model_pin_with_content_length(
            &mut stx,
            length_mismatch,
            length_mismatch_bundle.ciphertext_bytes.saturating_sub(1),
            PinStatus::Approved(1),
        );
        let length_mismatch_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: length_mismatch_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&length_mismatch_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(length_mismatch_result.is_err());

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_malformed_identifiers() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let invalid_model_digest = ManifestDigest::new([0xE0; 32]);
        insert_uploaded_model_pin(&mut stx, invalid_model_digest, PinStatus::Approved(1));
        let mut invalid_model_bundle = sample_uploaded_model_bundle("portal", invalid_model_digest);
        invalid_model_bundle.model_id = "vision model".to_string();
        let invalid_model_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: invalid_model_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&invalid_model_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(invalid_model_result.is_err());

        let invalid_version_digest = ManifestDigest::new([0xE1; 32]);
        insert_uploaded_model_pin(&mut stx, invalid_version_digest, PinStatus::Approved(1));
        let mut invalid_version_bundle =
            sample_uploaded_model_bundle("portal", invalid_version_digest);
        invalid_version_bundle.weight_version = "v1\nreplay".to_string();
        let invalid_version_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: invalid_version_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&invalid_version_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(invalid_version_result.is_err());

        assert_eq!(
            stx.world
                .soracloud_uploaded_model_bundles
                .iter()
                .filter(|((service, _model, _version), _)| service == "portal")
                .count(),
            0
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_zero_storage_metadata() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let adversarial_mutations: [(u8, fn(&mut SoraUploadedModelBundleV1)); 3] = [
            (0xE3, |bundle: &mut SoraUploadedModelBundleV1| {
                bundle.chunk_count = 0;
            }),
            (0xE4, |bundle: &mut SoraUploadedModelBundleV1| {
                bundle.plaintext_bytes = 0;
            }),
            (0xE5, |bundle: &mut SoraUploadedModelBundleV1| {
                bundle.ciphertext_bytes = 0;
            }),
        ];
        for (digest_byte, mutate) in adversarial_mutations {
            let digest = ManifestDigest::new([digest_byte; 32]);
            let mut bundle = sample_uploaded_model_bundle("portal", digest);
            mutate(&mut bundle);
            insert_uploaded_model_pin_with_content_length(
                &mut stx,
                digest,
                bundle.ciphertext_bytes,
                PinStatus::Approved(1),
            );
            let result = isi::RegisterSoracloudUploadedModelBundle {
                bundle: bundle.clone(),
                provenance: uploaded_model_bundle_provenance(&bundle),
            }
            .execute(&ALICE_ID, &mut stx);
            assert!(result.is_err());
        }

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_malformed_bundle_manifest_fields()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let adversarial_mutations: [fn(&mut SoraUploadedModelBundleV1); 8] = [
            |bundle| {
                bundle.schema_version = SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1 + 1;
            },
            |bundle| {
                bundle.family.clear();
            },
            |bundle| {
                bundle.modalities.clear();
            },
            |bundle| {
                bundle.modalities = vec![" ".to_string()];
            },
            |bundle| {
                bundle.modalities = vec!["text\nimage".to_string()];
            },
            |bundle| {
                bundle.modalities = vec!["text".to_string(), "text".to_string()];
            },
            |bundle| {
                bundle.decryption_policy_ref.clear();
            },
            |bundle| {
                bundle.wrapped_bundle_key.recipient_key_id = "other-recipient".to_string();
            },
        ];
        for (index, mutate) in adversarial_mutations.into_iter().enumerate() {
            let digest = ManifestDigest::new([(0xE6 + index) as u8; 32]);
            let mut bundle = sample_uploaded_model_bundle("portal", digest);
            mutate(&mut bundle);
            insert_uploaded_model_pin_with_content_length(
                &mut stx,
                digest,
                bundle.ciphertext_bytes,
                PinStatus::Approved(1),
            );
            let result = isi::RegisterSoracloudUploadedModelBundle {
                bundle: bundle.clone(),
                provenance: uploaded_model_bundle_provenance(&bundle),
            }
            .execute(&ALICE_ID, &mut stx);
            assert!(result.is_err());
        }

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_malformed_key_material() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let adversarial_mutations: [fn(&mut SoraUploadedModelBundleV1); 12] = [
            |bundle| {
                bundle.upload_recipient.schema_version =
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1
                        + 1;
            },
            |bundle| {
                bundle.upload_recipient.key_id.clear();
            },
            |bundle| {
                bundle.upload_recipient.public_key_bytes.clear();
            },
            |bundle| {
                bundle.upload_recipient.public_key_bytes = vec![0u8; 32];
                bundle.upload_recipient.public_key_fingerprint =
                    Hash::new(bundle.upload_recipient.public_key_bytes.as_slice());
            },
            |bundle| {
                bundle.upload_recipient.public_key_fingerprint = Hash::new(b"wrong-recipient-key");
            },
            |bundle| {
                bundle.wrapped_bundle_key.schema_version =
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1 + 1;
            },
            |bundle| {
                bundle.wrapped_bundle_key.ephemeral_public_key.clear();
            },
            |bundle| {
                bundle.wrapped_bundle_key.ephemeral_public_key = vec![0u8; 32];
            },
            |bundle| {
                bundle.wrapped_bundle_key.nonce.clear();
            },
            |bundle| {
                bundle.wrapped_bundle_key.wrapped_key_ciphertext.clear();
            },
            |bundle| {
                bundle.wrapped_bundle_key.ciphertext_hash = Hash::new(b"wrong-wrapped-key");
            },
            |bundle| {
                bundle.wrapped_bundle_key.recipient_key_version =
                    NonZeroU32::new(2).expect("non-zero key version");
            },
        ];
        for (index, mutate) in adversarial_mutations.into_iter().enumerate() {
            let digest = ManifestDigest::new([(0xEC + index) as u8; 32]);
            let mut bundle = sample_uploaded_model_bundle("portal", digest);
            mutate(&mut bundle);
            insert_uploaded_model_pin_with_content_length(
                &mut stx,
                digest,
                bundle.ciphertext_bytes,
                PinStatus::Approved(1),
            );
            let result = isi::RegisterSoracloudUploadedModelBundle {
                bundle: bundle.clone(),
                provenance: uploaded_model_bundle_provenance(&bundle),
            }
            .execute(&ALICE_ID, &mut stx);
            assert!(result.is_err());
        }

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_oversized_key_material() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let adversarial_mutations: [fn(&mut SoraUploadedModelBundleV1); 3] = [
            |bundle| {
                bundle.upload_recipient.public_key_bytes = vec![7u8; 257];
                bundle.upload_recipient.public_key_fingerprint =
                    Hash::new(bundle.upload_recipient.public_key_bytes.as_slice());
            },
            |bundle| {
                bundle.wrapped_bundle_key.ephemeral_public_key = vec![8u8; 257];
            },
            |bundle| {
                bundle.wrapped_bundle_key.wrapped_key_ciphertext = vec![9u8; 4_097];
                bundle.wrapped_bundle_key.ciphertext_hash =
                    Hash::new(bundle.wrapped_bundle_key.wrapped_key_ciphertext.as_slice());
            },
        ];
        for (index, mutate) in adversarial_mutations.into_iter().enumerate() {
            let digest = ManifestDigest::new([(0xF5 + index) as u8; 32]);
            let mut bundle = sample_uploaded_model_bundle("portal", digest);
            mutate(&mut bundle);
            insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
            let result = isi::RegisterSoracloudUploadedModelBundle {
                bundle: bundle.clone(),
                provenance: uploaded_model_bundle_provenance(&bundle),
            }
            .execute(&ALICE_ID, &mut stx);
            assert!(result.is_err());
        }

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_disallowed_service_plane()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let mut service_bundle = sample_bundle("portal", "1.0.0", 0);
        service_bundle.container.capabilities.allow_model_training = false;
        service_bundle.container.capabilities.allow_model_inference = false;
        service_bundle.service.container.manifest_hash = service_bundle.container_manifest_hash();
        isi::DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&service_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let digest = ManifestDigest::new([0xCD; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        let result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_nexus_limit_overrides() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;

        let plaintext_digest = ManifestDigest::new([0xCE; 32]);
        insert_uploaded_model_pin(&mut stx, plaintext_digest, PinStatus::Approved(1));
        stx.nexus.uploaded_models.max_plaintext_bytes_per_model = 1;
        let plaintext_bundle = sample_uploaded_model_bundle("portal", plaintext_digest);
        let plaintext_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: plaintext_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&plaintext_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(plaintext_result.is_err());

        let chunk_digest = ManifestDigest::new([0xCF; 32]);
        insert_uploaded_model_pin(&mut stx, chunk_digest, PinStatus::Approved(1));
        stx.nexus.uploaded_models.max_plaintext_bytes_per_model = u64::MAX;
        stx.nexus.uploaded_models.max_chunk_count_per_model = 0;
        let chunk_bundle = sample_uploaded_model_bundle("portal", chunk_digest);
        let chunk_result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: chunk_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&chunk_bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(chunk_result.is_err());

        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_tampered_signed_bundle() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xC6; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let mut bundle = sample_uploaded_model_bundle("portal", digest);
        let provenance = uploaded_model_bundle_provenance(&bundle);
        bundle.model_id = "vision_model_replayed".to_string();

        let result = isi::RegisterSoracloudUploadedModelBundle { bundle, provenance }
            .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model_replayed".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_tampered_signed_storage_reference()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let signed_digest = ManifestDigest::new([0xF8; 32]);
        let replay_digest = ManifestDigest::new([0xF9; 32]);
        insert_uploaded_model_pin(&mut stx, signed_digest, PinStatus::Approved(1));
        insert_uploaded_model_pin(&mut stx, replay_digest, PinStatus::Approved(1));
        let mut bundle = sample_uploaded_model_bundle("portal", signed_digest);
        let provenance = uploaded_model_bundle_provenance(&bundle);
        bundle.sorafs_manifest_digest = replay_digest;

        let result = isi::RegisterSoracloudUploadedModelBundle { bundle, provenance }
            .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_provenance_signer_mismatch()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xD0; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        let result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance_for(&bundle, &BOB_KEYPAIR),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_uploaded_model_bundles
                .get(&(
                    "portal".to_string(),
                    "vision_model".to_string(),
                    "v1".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_register_rejects_duplicate_model_version()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xC7; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let result = isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert_eq!(
            stx.world
                .soracloud_uploaded_model_bundles
                .iter()
                .filter(|((service, model, version), _)| {
                    service == "portal" && model == "vision_model" && version == "v1"
                })
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_uses_sorafs_pin_metadata_without_chunks()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let service_bundle = sample_training_bundle("portal", "1.0.0");
        isi::DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&service_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let digest = ManifestDigest::new([0xD5; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let service_name = bundle.service_name.clone();
        let weight_artifact_hash = Hash::new(b"weights");
        let training_config_hash = Hash::new(b"training-config");
        let reproducibility_hash = Hash::new(b"reproducibility");
        let provenance_attestation_hash = Hash::new(b"provenance-attestation");
        isi::FinalizeSoracloudUploadedModelBundle {
            service_name: service_name.clone(),
            model_name: "vision_model".to_string(),
            model_id: bundle.model_id.clone(),
            artifact_id: "uploaded-artifact".to_string(),
            weight_version: bundle.weight_version.clone(),
            bundle_root: bundle.bundle_root,
            weight_artifact_hash,
            dataset_ref: "dataset://upload".to_string(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: uploaded_model_finalize_provenance(
                &service_name,
                "vision_model",
                &bundle.model_id,
                "uploaded-artifact",
                &bundle.weight_version,
                bundle.bundle_root,
                weight_artifact_hash,
                "dataset://upload",
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            ),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let registry = stx
            .world
            .soracloud_model_registries
            .get(&(service_name.as_ref().to_owned(), "vision_model".to_string()))
            .expect("model registry");
        assert_eq!(registry.current_version.as_deref(), Some("v1"));
        let artifact = stx
            .world
            .soracloud_model_artifacts
            .get(&(
                service_name.as_ref().to_owned(),
                "uploaded-artifact".to_string(),
            ))
            .expect("model artifact");
        assert_eq!(
            artifact.chunk_manifest_root,
            Some(bundle.chunk_manifest_root)
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_unregistered_bundle() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xFA; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-unregistered",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_registries
                .get(&("portal".to_string(), "vision_model".to_string()))
                .is_none()
        );
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-unregistered".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_tampered_bundle_root() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xD6; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-tampered",
            Hash::new(b"tampered-bundle-root"),
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_registries
                .get(&("portal".to_string(), "vision_model".to_string()))
                .is_none()
        );
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-tampered".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_tampered_signed_payload()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xD8; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let mut instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-replayed",
            bundle.bundle_root,
        );
        instruction.model_id = "vision_model_replayed".to_string();
        let result = instruction.execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-replayed".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_malformed_identifiers() -> Result<(), eyre::Report>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xE2; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let invalid_artifact_result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded artifact",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(invalid_artifact_result.is_err());

        let mut invalid_model_instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-invalid-model",
            bundle.bundle_root,
        );
        invalid_model_instruction.model_id = "vision model".to_string();
        let invalid_model_result = invalid_model_instruction.execute(&ALICE_ID, &mut stx);
        assert!(invalid_model_result.is_err());

        let mut invalid_version_instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-invalid-version",
            bundle.bundle_root,
        );
        invalid_version_instruction.weight_version = "v1\tshadow".to_string();
        let invalid_version_result = invalid_version_instruction.execute(&ALICE_ID, &mut stx);
        assert!(invalid_version_result.is_err());

        let mut invalid_model_name_instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-invalid-model-name",
            bundle.bundle_root,
        );
        invalid_model_name_instruction.model_name = "vision model".to_string();
        let invalid_model_name_result = invalid_model_name_instruction.execute(&ALICE_ID, &mut stx);
        assert!(invalid_model_name_result.is_err());

        let mut invalid_dataset_instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-invalid-dataset",
            bundle.bundle_root,
        );
        invalid_dataset_instruction.dataset_ref = "dataset://upload\nshadow".to_string();
        let invalid_dataset_result = invalid_dataset_instruction.execute(&ALICE_ID, &mut stx);
        assert!(invalid_dataset_result.is_err());

        assert_eq!(
            stx.world
                .soracloud_model_artifacts
                .iter()
                .filter(|((service, artifact), _)| {
                    service == "portal" && artifact.starts_with("uploaded-artifact-invalid")
                })
                .count(),
            0
        );
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&("portal".to_string(), "uploaded artifact".to_string()))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_duplicate_weight_version()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xD9; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-original",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx)?;

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-replayed-version",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-replayed-version".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_pin_metadata_changed_after_register()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xDA; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        insert_uploaded_model_pin_with_content_length(
            &mut stx,
            digest,
            bundle.ciphertext_bytes + 1,
            PinStatus::Approved(2),
        );

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-mutated-pin",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-mutated-pin".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_pin_digest_changed_after_register()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xF1; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        insert_uploaded_model_pin_record(
            &mut stx,
            digest,
            ManifestDigest::new([0xF2; 32]),
            bundle.ciphertext_bytes,
            PinStatus::Approved(2),
        );

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-mutated-pin-digest",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-mutated-pin-digest".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_provenance_signer_mismatch()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xDB; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;

        let mut instruction = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-signer-mismatch",
            bundle.bundle_root,
        );
        instruction.provenance = uploaded_model_finalize_provenance_for(
            &bundle.service_name,
            "vision_model",
            &bundle.model_id,
            "uploaded-artifact-signer-mismatch",
            &bundle.weight_version,
            bundle.bundle_root,
            instruction.weight_artifact_hash,
            &instruction.dataset_ref,
            instruction.training_config_hash,
            instruction.reproducibility_hash,
            instruction.provenance_attestation_hash,
            &BOB_KEYPAIR,
        );
        let result = instruction.execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-signer-mismatch".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn soracloud_uploaded_model_finalize_rejects_retired_pin_after_register()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        deploy_uploaded_model_service(&mut stx)?;
        let digest = ManifestDigest::new([0xD7; 32]);
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
        let bundle = sample_uploaded_model_bundle("portal", digest);
        isi::RegisterSoracloudUploadedModelBundle {
            bundle: bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        insert_uploaded_model_pin(&mut stx, digest, PinStatus::Retired(12));

        let result = sample_uploaded_model_finalize_instruction(
            &bundle,
            "uploaded-artifact-retired",
            bundle.bundle_root,
        )
        .execute(&ALICE_ID, &mut stx);
        assert!(result.is_err());
        assert!(
            stx.world
                .soracloud_model_artifacts
                .get(&(
                    "portal".to_string(),
                    "uploaded-artifact-retired".to_string(),
                ))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn agent_apartment_lifecycle_instructions_record_authoritative_state()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let manifest =
            sample_agent_manifest_with_capabilities("ops_agent", &["agent.autonomy.run"]);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
            manifest: manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: 500,
            provenance: agent_deploy_provenance(manifest, 120, 500),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let apartment_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
        let renew_payload =
            encode_agent_lease_renew_provenance_payload(apartment_name.as_ref(), 60)
                .expect("renew payload");
        iroha_data_model::isi::InstructionBox::from(isi::RenewSoracloudAgentLease {
            apartment_name: apartment_name.clone(),
            lease_ticks: 60,
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &renew_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let restart_payload =
            encode_agent_restart_provenance_payload(apartment_name.as_ref(), "manual-restart")
                .expect("restart payload");
        iroha_data_model::isi::InstructionBox::from(isi::RestartSoracloudAgentApartment {
            apartment_name: apartment_name.clone(),
            reason: "manual-restart".to_string(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &restart_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let revoke_payload = encode_agent_policy_revoke_provenance_payload(
            apartment_name.as_ref(),
            "agent.autonomy.run",
            Some("manual-review"),
        )
        .expect("revoke payload");
        iroha_data_model::isi::InstructionBox::from(isi::RevokeSoracloudAgentPolicy {
            apartment_name: apartment_name.clone(),
            capability: "agent.autonomy.run".to_string(),
            reason: Some("manual-review".to_string()),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &revoke_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_agent_apartments()
            .get("ops_agent")
            .expect("apartment record");
        assert_eq!(record.restart_count, 1);
        assert_eq!(record.process_generation, 2);
        assert_eq!(
            record.last_restart_reason.as_deref(),
            Some("manual-restart")
        );
        assert!(
            record
                .revoked_policy_capabilities
                .contains("agent.autonomy.run"),
            "policy capability should be revoked"
        );
        let audit_actions = world
            .soracloud_agent_apartment_audit_events()
            .iter()
            .map(|(_sequence, event)| event.action)
            .collect::<Vec<_>>();
        assert_eq!(
            audit_actions,
            vec![
                SoraAgentApartmentActionV1::Deploy,
                SoraAgentApartmentActionV1::LeaseRenew,
                SoraAgentApartmentActionV1::Restart,
                SoraAgentApartmentActionV1::PolicyRevoked,
            ]
        );
        Ok(())
    }

    #[test]
    fn agent_wallet_mailbox_and_autonomy_instructions_record_authoritative_state()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let ops_manifest = sample_agent_manifest_with_capabilities(
            "ops_agent",
            &[
                "wallet.sign",
                "agent.mailbox.send",
                "agent.autonomy.allow",
                "agent.autonomy.run",
            ],
        );
        let worker_manifest =
            sample_agent_manifest_with_capabilities("worker_agent", &["agent.mailbox.receive"]);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
            manifest: ops_manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: 500,
            provenance: agent_deploy_provenance(ops_manifest, 120, 500),
        })
        .execute(&ALICE_ID, &mut stx)?;
        iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
            manifest: worker_manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: 250,
            provenance: agent_deploy_provenance(worker_manifest, 120, 250),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let ops_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
        let worker_name: iroha_data_model::name::Name = "worker_agent".parse().expect("valid");

        let wallet_spend_payload = encode_agent_wallet_spend_provenance_payload(
            ops_name.as_ref(),
            "61CtjvNd9T3THAR65GsMVHr82Bjc",
            1_000_000,
        )
        .expect("wallet spend payload");
        iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
            apartment_name: ops_name.clone(),
            asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
            amount_nanos: 1_000_000,
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &wallet_spend_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let wallet_approve_payload =
            encode_agent_wallet_approve_provenance_payload(ops_name.as_ref(), "ops_agent:wallet:3")
                .expect("wallet approve payload");
        iroha_data_model::isi::InstructionBox::from(isi::ApproveSoracloudAgentWalletSpend {
            apartment_name: ops_name.clone(),
            request_id: "ops_agent:wallet:3".to_string(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &wallet_approve_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let message_send_payload = encode_agent_message_send_provenance_payload(
            ops_name.as_ref(),
            worker_name.as_ref(),
            "ops.sync",
            "rotate-key-42",
        )
        .expect("message send payload");
        iroha_data_model::isi::InstructionBox::from(isi::EnqueueSoracloudAgentMessage {
            from_apartment: ops_name.clone(),
            to_apartment: worker_name.clone(),
            channel: "ops.sync".to_string(),
            payload: "rotate-key-42".to_string(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &message_send_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let message_ack_payload = encode_agent_message_ack_provenance_payload(
            worker_name.as_ref(),
            "worker_agent:mail:5",
        )
        .expect("message ack payload");
        iroha_data_model::isi::InstructionBox::from(isi::AcknowledgeSoracloudAgentMessage {
            apartment_name: worker_name.clone(),
            message_id: "worker_agent:mail:5".to_string(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &message_ack_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let artifact_allow_payload = encode_agent_artifact_allow_provenance_payload(
            ops_name.as_ref(),
            "hash:artifact#1",
            Some("hash:prov#1"),
        )
        .expect("artifact allow payload");
        iroha_data_model::isi::InstructionBox::from(isi::AllowSoracloudAgentAutonomyArtifact {
            apartment_name: ops_name.clone(),
            artifact_hash: "hash:artifact#1".to_string(),
            provenance_hash: Some("hash:prov#1".to_string()),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &artifact_allow_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let autonomy_run_payload = encode_agent_autonomy_run_provenance_payload(
            ops_name.as_ref(),
            "hash:artifact#1",
            Some("hash:prov#1"),
            120,
            "nightly-batch-1",
            Some(
                "{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}",
            ),
        )
        .expect("autonomy run payload");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
            apartment_name: ops_name,
            artifact_hash: "hash:artifact#1".to_string(),
            provenance_hash: Some("hash:prov#1".to_string()),
            budget_units: 120,
            run_label: "nightly-batch-1".to_string(),
            workflow_input_json: Some(
                "{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}"
                    .to_string(),
            ),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &autonomy_run_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let ops_record = world
            .soracloud_agent_apartments()
            .get("ops_agent")
            .expect("ops apartment");
        assert!(ops_record.pending_wallet_requests.is_empty());
        assert_eq!(
            ops_record
                .wallet_daily_spend
                .get("61CtjvNd9T3THAR65GsMVHr82Bjc:0")
                .expect("wallet day aggregate")
                .spent_nanos,
            1_000_000
        );
        assert_eq!(ops_record.autonomy_budget_remaining_units, 380);
        assert_eq!(ops_record.autonomy_run_history.len(), 1);
        let canonical_workflow_input_json =
            "{\"inputs\":{\"messages\":[{\"content\":\"nightly-batch-1\",\"role\":\"user\"}]}}";
        assert_eq!(
            ops_record.autonomy_run_history[0]
                .workflow_input_json
                .as_deref(),
            Some(canonical_workflow_input_json)
        );
        let autonomy_event = world
            .soracloud_agent_apartment_audit_events()
            .get(&ops_record.autonomy_run_history[0].approved_sequence)
            .expect("autonomy audit event");
        assert_eq!(
            autonomy_event.payload_hash,
            Some(Hash::new(canonical_workflow_input_json.as_bytes()))
        );
        assert_eq!(ops_record.checkpoint_count, 1);
        assert_eq!(ops_record.last_checkpoint_sequence, Some(8));
        assert_eq!(ops_record.artifact_allowlist.len(), 1);

        let worker_record = world
            .soracloud_agent_apartments()
            .get("worker_agent")
            .expect("worker apartment");
        assert!(worker_record.mailbox_queue.is_empty());
        assert_eq!(
            world
                .soracloud_agent_apartment_audit_events()
                .iter()
                .count(),
            8
        );
        Ok(())
    }

    #[test]
    fn record_agent_autonomy_execution_records_authoritative_audit_state()
    -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let ops_manifest = sample_agent_manifest_with_capabilities(
            "ops_agent",
            &["agent.autonomy.allow", "agent.autonomy.run"],
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
            manifest: ops_manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: 500,
            provenance: agent_deploy_provenance(ops_manifest, 120, 500),
        })
        .execute(&ALICE_ID, &mut stx)?;

        let apartment_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
        let artifact_allow_payload = encode_agent_artifact_allow_provenance_payload(
            apartment_name.as_ref(),
            "hash:artifact#1",
            Some("hash:prov#1"),
        )
        .expect("artifact allow payload");
        iroha_data_model::isi::InstructionBox::from(isi::AllowSoracloudAgentAutonomyArtifact {
            apartment_name: apartment_name.clone(),
            artifact_hash: "hash:artifact#1".to_string(),
            provenance_hash: Some("hash:prov#1".to_string()),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &artifact_allow_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let workflow_input_json = "{\"inputs\":\"nightly\"}";
        let autonomy_run_payload = encode_agent_autonomy_run_provenance_payload(
            apartment_name.as_ref(),
            "hash:artifact#1",
            Some("hash:prov#1"),
            120,
            "nightly",
            Some(workflow_input_json),
        )
        .expect("autonomy run payload");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
            apartment_name: apartment_name.clone(),
            artifact_hash: "hash:artifact#1".to_string(),
            provenance_hash: Some("hash:prov#1".to_string()),
            budget_units: 120,
            run_label: "nightly".to_string(),
            workflow_input_json: Some(workflow_input_json.to_string()),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: iroha_crypto::Signature::new(
                    ALICE_KEYPAIR.private_key(),
                    &autonomy_run_payload,
                ),
            },
        })
        .execute(&ALICE_ID, &mut stx)?;

        let approved_run = stx
            .world
            .soracloud_agent_apartments
            .get("ops_agent")
            .expect("ops apartment in transaction")
            .autonomy_run_history
            .last()
            .cloned()
            .expect("approved run");
        let result_commitment = Hash::new(b"ops-agent-runtime-result");
        let runtime_receipt_id = Hash::new(b"ops-agent-runtime-receipt");
        let journal_artifact_hash = Hash::new(b"ops-agent-runtime-journal");
        let checkpoint_artifact_hash = Hash::new(b"ops-agent-runtime-checkpoint");
        let service_name: iroha_data_model::name::Name =
            "hf_agent_service".parse().expect("valid service name");
        let handler_name: iroha_data_model::name::Name = "infer".parse().expect("valid handler");
        iroha_data_model::isi::InstructionBox::from(isi::RecordSoracloudAgentAutonomyExecution {
            apartment_name,
            run_id: approved_run.run_id.clone(),
            process_generation: approved_run.approved_process_generation,
            succeeded: true,
            result_commitment,
            service_name: Some(service_name),
            service_version: Some("hf.generated.v1".to_string()),
            handler_name: Some(handler_name),
            runtime_receipt_id: Some(runtime_receipt_id),
            journal_artifact_hash: Some(journal_artifact_hash),
            checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
            error: None,
        })
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let record = world
            .soracloud_agent_apartments()
            .get("ops_agent")
            .expect("ops apartment");
        let event = world
            .soracloud_agent_apartment_audit_events()
            .get(&record.last_active_sequence)
            .expect("execution audit event");
        assert_eq!(
            event.schema_version,
            iroha_data_model::soracloud::SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1
        );
        assert_eq!(
            event.action,
            iroha_data_model::soracloud::SoraAgentApartmentActionV1::AutonomyRunExecuted
        );
        assert_eq!(event.run_id.as_deref(), Some(approved_run.run_id.as_str()));
        assert_eq!(
            event.request_id.as_deref(),
            Some(approved_run.run_id.as_str())
        );
        assert_eq!(event.result_commitment, Some(result_commitment));
        assert_eq!(event.runtime_receipt_id, Some(runtime_receipt_id));
        assert_eq!(event.journal_artifact_hash, Some(journal_artifact_hash));
        assert_eq!(
            event.checkpoint_artifact_hash,
            Some(checkpoint_artifact_hash)
        );
        assert_eq!(event.succeeded, Some(true));
        assert_eq!(event.service_name.as_deref(), Some("hf_agent_service"));
        assert_eq!(event.service_version.as_deref(), Some("hf.generated.v1"));
        assert_eq!(event.handler_name.as_deref(), Some("infer"));
        assert_eq!(
            world
                .soracloud_agent_apartment_audit_events()
                .iter()
                .count(),
            4
        );
        Ok(())
    }
}
