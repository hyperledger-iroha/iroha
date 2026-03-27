//! Soracloud lifecycle and private-runtime instruction handlers.

use std::{collections::BTreeMap, time::Duration};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        soracloud as isi,
    },
    name::Name,
    nexus::PublicLaneValidatorStatus,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheExecutionPolicyV1, FheJobSpecV1,
        FheParamSetV1, SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        SORA_AGENT_APARTMENT_RECORD_VERSION_V1, SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
        SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
        SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
        SORA_HF_SOURCE_RECORD_VERSION_V1, SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
        SORA_MODEL_ARTIFACT_RECORD_VERSION_V1, SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
        SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1, SORA_MODEL_REGISTRY_VERSION_V1,
        SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1, SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
        SORA_PRIVATE_COMPILE_PROFILE_VERSION_V1, SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
        SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1, SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        SORA_SERVICE_CONFIG_ENTRY_VERSION_V1, SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        SORA_SERVICE_ROLLOUT_STATE_VERSION_V1, SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
        SORA_SERVICE_STATE_ENTRY_VERSION_V1, SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
        SORA_TRAINING_JOB_RECORD_VERSION_V1, SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        SORA_UPLOADED_MODEL_CHUNK_VERSION_V1, SecretEnvelopeV1, SoraAgentApartmentActionV1,
        SoraAgentApartmentAuditEventV1, SoraAgentApartmentRecordV1, SoraAgentArtifactAllowRuleV1,
        SoraAgentAutonomyRunRecordV1, SoraAgentMailboxMessageV1, SoraAgentPersistentStateV1,
        SoraAgentRuntimeStatusV1, SoraAgentWalletDailySpendEntryV1, SoraAgentWalletSpendRequestV1,
        SoraDecryptionRequestRecordV1, SoraDeploymentBundleV1, SoraHfPlacementHostAssignmentV1,
        SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
        SoraHfPlacementStatusV1, SoraHfResourceProfileV1, SoraHfSharedLeaseActionV1,
        SoraHfSharedLeaseAuditEventV1, SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1,
        SoraHfSharedLeasePoolV1, SoraHfSharedLeaseQueuedWindowV1, SoraHfSharedLeaseStatusV1,
        SoraHfSourceRecordV1, SoraHfSourceStatusV1, SoraModelArtifactActionV1,
        SoraModelArtifactAuditEventV1, SoraModelArtifactRecordV1, SoraModelHostCapabilityRecordV1,
        SoraModelHostViolationEvidenceRecordV1, SoraModelHostViolationKindV1,
        SoraModelPrivacyModeV1, SoraModelProvenanceKindV1, SoraModelProvenanceRefV1,
        SoraModelRegistryV1, SoraModelWeightActionV1, SoraModelWeightAuditEventV1,
        SoraModelWeightVersionRecordV1, SoraPrivateCompileProfileV1,
        SoraPrivateInferenceCheckpointV1, SoraPrivateInferenceSessionStatusV1,
        SoraPrivateInferenceSessionV1, SoraRolloutStageV1, SoraRuntimeReceiptV1,
        SoraServiceAuditEventV1, SoraServiceConfigEntryV1, SoraServiceDeploymentStateV1,
        SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1,
        SoraServiceRuntimeStateV1, SoraServiceSecretEntryV1, SoraServiceStateEntryV1,
        SoraStateEncryptionV1, SoraStateMutationOperationV1, SoraTrainingJobActionV1,
        SoraTrainingJobAuditEventV1, SoraTrainingJobRecordV1, SoraTrainingJobStatusV1,
        SoraUploadedModelBindingStatusV1, SoraUploadedModelBindingV1, SoraUploadedModelBundleV1,
        SoraUploadedModelChunkV1, derive_agent_autonomy_request_commitment,
        encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_decryption_request_provenance_payload,
        encode_delete_service_config_provenance_payload,
        encode_delete_service_secret_provenance_payload, encode_fhe_job_run_provenance_payload,
        encode_hf_shared_lease_join_provenance_payload,
        encode_hf_shared_lease_leave_provenance_payload,
        encode_hf_shared_lease_renew_provenance_payload,
        encode_model_artifact_register_provenance_payload,
        encode_model_host_advertise_provenance_payload,
        encode_model_host_heartbeat_provenance_payload,
        encode_model_host_withdraw_provenance_payload,
        encode_model_weight_promote_provenance_payload,
        encode_model_weight_register_provenance_payload,
        encode_model_weight_rollback_provenance_payload,
        encode_private_compile_profile_provenance_payload,
        encode_private_inference_start_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_set_service_config_provenance_payload,
        encode_set_service_secret_provenance_payload, encode_state_mutation_provenance_payload,
        encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
        encode_uploaded_model_allow_provenance_payload,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_chunk_append_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
    },
    sorafs::pin_registry::StorageClass,
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
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
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
        encryption,
        governance_tx_hash,
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

fn verify_fhe_job_run_provenance(
    authority: &AccountId,
    service_name: &iroha_data_model::name::Name,
    binding_name: &iroha_data_model::name::Name,
    job: FheJobSpecV1,
    policy: FheExecutionPolicyV1,
    param_set: FheParamSetV1,
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

fn verify_uploaded_model_chunk_append_provenance(
    authority: &AccountId,
    chunk: &SoraUploadedModelChunkV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "uploaded model chunk provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_uploaded_model_chunk_append_provenance_payload(chunk.clone()).map_err(|err| {
            invalid_parameter(format!(
                "failed to encode uploaded model chunk provenance: {err}"
            ))
        })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("uploaded model chunk provenance signature verification failed")
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
    privacy_mode: SoraModelPrivacyModeV1,
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
        privacy_mode,
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

fn verify_private_compile_profile_provenance(
    authority: &AccountId,
    service_name: &Name,
    model_id: &str,
    weight_version: &str,
    bundle_root: Hash,
    compile_profile: &SoraPrivateCompileProfileV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "private compile provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_private_compile_profile_provenance_payload(
        service_name.as_ref(),
        model_id,
        weight_version,
        bundle_root,
        compile_profile.clone(),
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode private compile provenance: {err}"
        ))
    })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("private compile provenance signature verification failed")
        })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_uploaded_model_allow_provenance(
    authority: &AccountId,
    apartment_name: &Name,
    service_name: &Name,
    model_name: &str,
    model_id: &str,
    artifact_id: &str,
    weight_version: &str,
    bundle_root: Hash,
    compile_profile_hash: Hash,
    privacy_mode: SoraModelPrivacyModeV1,
    require_model_inference: bool,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "uploaded model allow provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_uploaded_model_allow_provenance_payload(
        apartment_name.as_ref(),
        service_name.as_ref(),
        model_name,
        model_id,
        artifact_id,
        weight_version,
        bundle_root,
        compile_profile_hash,
        privacy_mode,
        require_model_inference,
    )
    .map_err(|err| {
        invalid_parameter(format!(
            "failed to encode uploaded model allow provenance: {err}"
        ))
    })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("uploaded model allow provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_private_inference_start_provenance(
    authority: &AccountId,
    session: &SoraPrivateInferenceSessionV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "private inference start provenance signer must match the transaction authority",
        ));
    }
    let payload =
        encode_private_inference_start_provenance_payload(session.clone()).map_err(|err| {
            invalid_parameter(format!(
                "failed to encode private inference start provenance: {err}"
            ))
        })?;
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| {
            invalid_parameter("private inference start provenance signature verification failed")
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
        state_transaction
            .world
            .soracloud_agent_apartments
            .iter()
            .filter_map(|(_key, record)| {
                record
                    .uploaded_model_binding
                    .as_ref()
                    .map(|binding| binding.bound_sequence)
            })
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

fn parse_private_session_id(session_id: &str) -> Result<String, InstructionExecutionError> {
    parse_training_job_id(session_id).map_err(|_| invalid_parameter("invalid session_id"))
}

fn parse_private_decrypt_request_id(
    decrypt_request_id: &str,
) -> Result<String, InstructionExecutionError> {
    parse_training_job_id(decrypt_request_id)
        .map_err(|_| invalid_parameter("invalid decrypt_request_id"))
}

fn service_allows_uploaded_model_plane(bundle: &SoraDeploymentBundleV1) -> bool {
    bundle.container.capabilities.allow_model_training
        || bundle.container.capabilities.allow_model_inference
}

fn uploaded_model_chunk_storage_key(
    service_name: &Name,
    model_id: &str,
    weight_version: &str,
    ordinal: u32,
) -> String {
    format!(
        "{}::{model_id}::{weight_version}::{ordinal}",
        service_name.as_ref()
    )
}

fn active_private_session_count(
    state_transaction: &StateTransaction<'_, '_>,
    apartment_name: &Name,
) -> u32 {
    u32::try_from(
        state_transaction
            .world
            .soracloud_private_inference_sessions
            .iter()
            .filter(|((stored_apartment, _session_id), session)| {
                stored_apartment == apartment_name.as_ref()
                    && matches!(
                        session.status,
                        SoraPrivateInferenceSessionStatusV1::Admitted
                            | SoraPrivateInferenceSessionStatusV1::Running
                            | SoraPrivateInferenceSessionStatusV1::AwaitingDecryption
                    )
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn load_private_session_key(
    state_transaction: &StateTransaction<'_, '_>,
    session_id: &str,
) -> Result<(String, String), InstructionExecutionError> {
    let mut matches = state_transaction
        .world
        .soracloud_private_inference_sessions
        .iter()
        .filter_map(|((apartment_name, stored_session_id), _session)| {
            (stored_session_id == session_id)
                .then(|| (apartment_name.clone(), stored_session_id.clone()))
        })
        .collect::<Vec<_>>();
    match matches.len() {
        1 => Ok(matches.pop().expect("one private session key")),
        0 => Err(InstructionExecutionError::InvariantViolation(
            format!("private session `{session_id}` not found").into(),
        )),
        _ => Err(InstructionExecutionError::InvariantViolation(
            format!("private session `{session_id}` is duplicated in authoritative state").into(),
        )),
    }
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

fn agent_uploaded_model_binding_ready(
    record: &SoraAgentApartmentRecordV1,
) -> Result<(), InstructionExecutionError> {
    let Some(binding) = record.uploaded_model_binding.as_ref() else {
        return Ok(());
    };
    if binding.require_model_inference
        && !agent_policy_capability_active(record, "allow_model_inference")
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "apartment `{}` is bound to uploaded model `{}` but `allow_model_inference` is not active",
                record.manifest.apartment_name, binding.model_id
            )
            .into(),
        ));
    }
    match binding.status {
        SoraUploadedModelBindingStatusV1::Active => Ok(()),
        SoraUploadedModelBindingStatusV1::PolicyRevoked => Err(
            InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model binding `{}` is revoked for apartment `{}`",
                    binding.model_id, record.manifest.apartment_name
                )
                .into(),
            ),
        ),
        SoraUploadedModelBindingStatusV1::VersionRolledBack => Err(
            InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model binding `{}` version `{}` is inactive after rollback for apartment `{}`",
                    binding.model_id, binding.weight_version, record.manifest.apartment_name
                )
                .into(),
            ),
        ),
    }
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

fn derive_state_mutation_commitment(
    service_name: &str,
    binding_name: &str,
    state_key: &str,
    payload_bytes: u64,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.state_mutation.ciphertext.v1",
        service_name,
        binding_name,
        state_key,
        payload_bytes,
        encryption,
        governance_tx_hash,
    )))
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
    payload_bytes: Option<u64>,
    payload_commitment: Option<Hash>,
    encryption: SoraStateEncryptionV1,
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
            let value_size_bytes = payload_bytes.ok_or_else(|| {
                invalid_parameter("payload_bytes is required for upsert mutations")
            })?;
            let payload_bytes = std::num::NonZeroU64::new(value_size_bytes).ok_or_else(|| {
                invalid_parameter("payload_bytes must be greater than zero for upsert mutations")
            })?;
            let payload_commitment = payload_commitment.ok_or_else(|| {
                invalid_parameter("payload_commitment is required for upsert mutations")
            })?;
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
                    payload_bytes,
                    payload_commitment,
                    last_update_sequence: sequence,
                    governance_tx_hash: linkage_hash,
                    source_action: SoraServiceLifecycleActionV1::StateMutation,
                },
            )?;
        }
        SoraStateMutationOperationV1::Delete => {
            if payload_bytes.is_some() || payload_commitment.is_some() {
                return Err(invalid_parameter(
                    "delete mutations must not provide payload_bytes or payload_commitment",
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
        .map_err(|err| invalid_parameter(err.to_string()))?;
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
        .map_err(|err| invalid_parameter(err.to_string()))?;
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

fn record_uploaded_model_chunk(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraUploadedModelChunkV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    let chunk_key = uploaded_model_chunk_storage_key(
        &record.service_name,
        &record.model_id,
        &record.weight_version,
        record.ordinal,
    );
    state_transaction
        .world
        .soracloud_uploaded_model_chunks
        .insert(chunk_key, record);
    Ok(())
}

fn record_private_compile_profile(
    state_transaction: &mut StateTransaction<'_, '_>,
    hash: Hash,
    record: SoraPrivateCompileProfileV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_private_compile_profiles
        .insert(hash, record);
    Ok(())
}

fn record_private_inference_session(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraPrivateInferenceSessionV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_private_inference_sessions
        .insert(
            (
                record.apartment.as_ref().to_owned(),
                record.session_id.clone(),
            ),
            record,
        );
    Ok(())
}

fn record_private_inference_checkpoint(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: SoraPrivateInferenceCheckpointV1,
) -> Result<(), InstructionExecutionError> {
    record
        .validate()
        .map_err(|err| invalid_parameter(err.to_string()))?;
    state_transaction
        .world
        .soracloud_private_inference_checkpoints
        .insert((record.session_id.clone(), record.step), record);
    Ok(())
}

fn reconcile_uploaded_model_bindings_after_version_change(
    state_transaction: &mut StateTransaction<'_, '_>,
    service_name: &Name,
    model_name: &str,
    current_version: Option<&str>,
    sequence: u64,
) {
    let service_name_literal = service_name.as_ref().to_owned();
    let apartment_keys = state_transaction
        .world
        .soracloud_agent_apartments
        .iter()
        .filter_map(|(key, record)| {
            let binding = record.uploaded_model_binding.as_ref()?;
            if binding.service_name.as_ref() == service_name_literal.as_str()
                && binding.model_name == model_name
            {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for apartment_key in apartment_keys {
        let Some(mut record) = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
        else {
            continue;
        };
        let Some(binding) = record.uploaded_model_binding.as_mut() else {
            continue;
        };
        if current_version.is_some_and(|version| version == binding.weight_version) {
            binding.status = SoraUploadedModelBindingStatusV1::Active;
            binding.status_reason = None;
        } else {
            binding.status = SoraUploadedModelBindingStatusV1::VersionRolledBack;
            binding.status_reason = Some(format!(
                "current promoted version for `{model_name}` moved to `{}` at sequence {sequence}",
                current_version.unwrap_or("none")
            ));
        }
        touch_agent_runtime_activity(&mut record, sequence);
        state_transaction
            .world
            .soracloud_agent_apartments
            .insert(apartment_key, record);
    }
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

fn uploaded_model_ciphertext_hash(payload: &SecretEnvelopeV1) -> Hash {
    Hash::new(payload.ciphertext.as_slice())
}

fn uploaded_model_chunk_manifest_root(
    chunks: &[SoraUploadedModelChunkV1],
) -> Result<Hash, InstructionExecutionError> {
    let manifest = chunks
        .iter()
        .map(|chunk| {
            (
                chunk.ordinal,
                chunk.offset_bytes,
                chunk.plaintext_len,
                chunk.ciphertext_len,
                chunk.ciphertext_hash,
            )
        })
        .collect::<Vec<_>>();
    let encoded = norito::to_bytes(&manifest).map_err(|err| {
        invalid_parameter(format!(
            "failed to encode uploaded model chunk manifest for hashing: {err}"
        ))
    })?;
    Ok(Hash::new(encoded))
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
            encryption,
            governance_tx_hash,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        verify_state_mutation_provenance(
            authority,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            value_size_bytes,
            encryption,
            governance_tx_hash,
            &provenance,
        )?;

        let sequence = next_soracloud_audit_sequence(state_transaction);
        let payload_commitment = match operation {
            SoraStateMutationOperationV1::Upsert => {
                let payload_bytes = value_size_bytes.ok_or_else(|| {
                    invalid_parameter("value_size_bytes is required for upsert mutations")
                })?;
                Some(derive_state_mutation_commitment(
                    service_name.as_ref(),
                    binding_name.as_ref(),
                    &state_key,
                    payload_bytes,
                    encryption,
                    governance_tx_hash,
                ))
            }
            SoraStateMutationOperationV1::Delete => None,
        };
        let (deployment, bundle) = apply_soracloud_state_mutation(
            state_transaction,
            &service_name,
            &binding_name,
            &state_key,
            operation,
            value_size_bytes,
            payload_commitment,
            encryption,
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

        let output_payload_bytes = self.job.deterministic_output_payload_bytes();
        let payload_bytes = std::num::NonZeroU64::new(output_payload_bytes).ok_or_else(|| {
            invalid_parameter("fhe output payload size must be greater than zero")
        })?;
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
        let output_commitment = self.job.deterministic_output_commitment();
        record_service_state_entry(
            state_transaction,
            SoraServiceStateEntryV1 {
                schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                service_name: self.service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                binding_name: self.binding_name.clone(),
                state_key: output_state_key.clone(),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                payload_bytes,
                payload_commitment: output_commitment,
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

        require_soracloud_permission(authority, state_transaction)?;
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

        require_soracloud_permission(authority, state_transaction)?;
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

        require_soracloud_permission(authority, state_transaction)?;
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
            uploaded_model_binding: None,
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
        if normalized_capability == "allow_model_inference"
            && let Some(binding) = record.uploaded_model_binding.as_mut()
        {
            binding.status = SoraUploadedModelBindingStatusV1::PolicyRevoked;
            binding.status_reason = normalized_reason
                .clone()
                .or_else(|| Some("allow_model_inference policy capability revoked".to_string()));
        }
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
        agent_uploaded_model_binding_ready(&record)?;
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
        agent_uploaded_model_binding_ready(&record)?;
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
                private_bundle_root: None,
                compile_profile_hash: None,
                chunk_manifest_root: None,
                privacy_mode: None,
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
        reconcile_uploaded_model_bindings_after_version_change(
            state_transaction,
            &self.service_name,
            &model_name,
            registry_record.current_version.as_deref(),
            sequence,
        );
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
        reconcile_uploaded_model_bindings_after_version_change(
            state_transaction,
            &self.service_name,
            &model_name,
            registry_record.current_version.as_deref(),
            sequence,
        );
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

impl Execute for isi::AppendSoracloudUploadedModelChunk {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::AppendSoracloudUploadedModelChunk { chunk, provenance } = self;
        verify_uploaded_model_chunk_append_provenance(authority, &chunk, &provenance)?;
        chunk
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let model_id = parse_uploaded_model_id(&chunk.model_id)?;
        let weight_version = parse_model_weight_version(&chunk.weight_version)?;

        let bundle_key = (
            chunk.service_name.as_ref().to_owned(),
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
                    "uploaded model bundle `{model_id}` version `{weight_version}` not found for service `{}`",
                    chunk.service_name
                )
                .into(),
            ));
        };
        if chunk.bundle_root != bundle_record.bundle_root {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("bundle_root mismatch for uploaded model `{model_id}`").into(),
            ));
        }
        if chunk.ordinal >= bundle_record.chunk_count {
            return Err(invalid_parameter(format!(
                "chunk ordinal {} exceeds bundle chunk_count {}",
                chunk.ordinal, bundle_record.chunk_count
            )));
        }
        if u64::from(chunk.plaintext_len)
            > state_transaction
                .nexus
                .uploaded_models
                .chunk_plaintext_bytes
        {
            return Err(invalid_parameter(format!(
                "chunk plaintext_len exceeds nexus.uploaded_models.chunk_plaintext_bytes ({})",
                state_transaction
                    .nexus
                    .uploaded_models
                    .chunk_plaintext_bytes
            )));
        }
        if usize::try_from(chunk.ciphertext_len).ok()
            != Some(chunk.encrypted_payload.ciphertext.len())
        {
            return Err(invalid_parameter(
                "ciphertext_len must equal encrypted_payload.ciphertext length",
            ));
        }
        let ciphertext_hash = uploaded_model_ciphertext_hash(&chunk.encrypted_payload);
        if chunk.ciphertext_hash != ciphertext_hash {
            return Err(invalid_parameter(format!(
                "ciphertext_hash does not match encrypted payload bytes for chunk {}",
                chunk.ordinal
            )));
        }
        let expected_offset = u64::from(chunk.ordinal).saturating_mul(
            state_transaction
                .nexus
                .uploaded_models
                .chunk_plaintext_bytes,
        );
        if chunk.offset_bytes != expected_offset {
            return Err(invalid_parameter(format!(
                "chunk offset_bytes {} does not match deterministic shard offset {}",
                chunk.offset_bytes, expected_offset
            )));
        }

        let chunk_key = uploaded_model_chunk_storage_key(
            &chunk.service_name,
            &model_id,
            &weight_version,
            chunk.ordinal,
        );
        if state_transaction
            .world
            .soracloud_uploaded_model_chunks
            .get(&chunk_key)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model chunk {} already registered for `{model_id}` version `{weight_version}`",
                    chunk.ordinal
                )
                .into(),
            ));
        }
        let stored_chunk_count = state_transaction
            .world
            .soracloud_uploaded_model_chunks
            .iter()
            .filter(|(_key, stored_chunk)| {
                stored_chunk.service_name == chunk.service_name
                    && stored_chunk.model_id == model_id
                    && stored_chunk.weight_version == weight_version
            })
            .count();
        if u32::try_from(stored_chunk_count).unwrap_or(u32::MAX) != chunk.ordinal {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model chunk {} must be appended in deterministic ordinal order",
                    chunk.ordinal
                )
                .into(),
            ));
        }

        let record = SoraUploadedModelChunkV1 {
            schema_version: SORA_UPLOADED_MODEL_CHUNK_VERSION_V1,
            model_id,
            weight_version,
            ..chunk
        };
        record_uploaded_model_chunk(state_transaction, record)
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
            privacy_mode,
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
            privacy_mode,
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

        let mut chunks = state_transaction
            .world
            .soracloud_uploaded_model_chunks
            .iter()
            .filter_map(|(_key, chunk)| {
                (chunk.service_name == service_name
                    && chunk.model_id == model_id
                    && chunk.weight_version == weight_version)
                    .then(|| chunk.clone())
            })
            .collect::<Vec<_>>();
        chunks.sort_by_key(|chunk| chunk.ordinal);
        if u32::try_from(chunks.len()).unwrap_or(u32::MAX) != bundle_record.chunk_count {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model `{model_id}` version `{weight_version}` has {} stored chunks but bundle expects {}",
                    chunks.len(), bundle_record.chunk_count
                )
                .into(),
            ));
        }
        let mut expected_offset = 0_u64;
        let mut plaintext_total = 0_u64;
        let mut ciphertext_total = 0_u64;
        for (index, chunk) in chunks.iter().enumerate() {
            let expected_ordinal = u32::try_from(index).unwrap_or(u32::MAX);
            if chunk.ordinal != expected_ordinal {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "uploaded model `{model_id}` chunk ordering is not contiguous at ordinal {}",
                        expected_ordinal
                    )
                    .into(),
                ));
            }
            if chunk.offset_bytes != expected_offset {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "uploaded model `{model_id}` chunk {} offset {} does not match expected {}",
                        chunk.ordinal, chunk.offset_bytes, expected_offset
                    )
                    .into(),
                ));
            }
            expected_offset = expected_offset.saturating_add(u64::from(chunk.plaintext_len));
            plaintext_total = plaintext_total.saturating_add(u64::from(chunk.plaintext_len));
            ciphertext_total = ciphertext_total.saturating_add(u64::from(chunk.ciphertext_len));
            let ciphertext_hash = uploaded_model_ciphertext_hash(&chunk.encrypted_payload);
            if chunk.ciphertext_hash != ciphertext_hash {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "uploaded model `{model_id}` chunk {} ciphertext hash does not match payload bytes",
                        chunk.ordinal
                    )
                    .into(),
                ));
            }
        }
        if plaintext_total != bundle_record.plaintext_bytes
            || ciphertext_total != bundle_record.ciphertext_bytes
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model `{model_id}` chunk byte totals do not match bundle manifest"
                )
                .into(),
            ));
        }
        let computed_manifest_root = uploaded_model_chunk_manifest_root(&chunks)?;
        if computed_manifest_root != bundle_record.chunk_manifest_root {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model `{model_id}` chunk manifest root does not match bundle manifest"
                )
                .into(),
            ));
        }

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
                private_bundle_root: Some(bundle_root),
                compile_profile_hash: Some(bundle_record.compile_profile_hash),
                chunk_manifest_root: Some(bundle_record.chunk_manifest_root),
                privacy_mode: Some(privacy_mode),
            },
        )?;
        reconcile_uploaded_model_bindings_after_version_change(
            state_transaction,
            &service_name,
            &model_name,
            Some(weight_version.as_str()),
            sequence,
        );
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

impl Execute for isi::AdmitSoracloudPrivateCompileProfile {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::AdmitSoracloudPrivateCompileProfile {
            service_name,
            model_id,
            weight_version,
            bundle_root,
            compile_profile,
            provenance,
        } = self;
        let model_id = parse_uploaded_model_id(&model_id)?;
        let weight_version = parse_model_weight_version(&weight_version)?;
        verify_private_compile_profile_provenance(
            authority,
            &service_name,
            &model_id,
            &weight_version,
            bundle_root,
            &compile_profile,
            &provenance,
        )?;
        compile_profile
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;

        let (_deployment, service_bundle) = load_active_bundle(state_transaction, &service_name)?;
        if !service_allows_uploaded_model_plane(&service_bundle) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "service `{service_name}` active revision does not allow uploaded model compilation"
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

        let compile_profile_hash: Hash = HashOf::new(&compile_profile).into();
        if bundle_record.compile_profile_hash != compile_profile_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "compile profile hash does not match bundle manifest for uploaded model `{model_id}`"
                )
                .into(),
            ));
        }
        if state_transaction
            .world
            .soracloud_private_compile_profiles
            .get(&compile_profile_hash)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("compile profile `{compile_profile_hash}` already admitted").into(),
            ));
        }

        transfer_uploaded_model_amount(
            authority,
            bundle_record.pricing_policy.compile_xor_nanos,
            state_transaction,
        )?;
        record_private_compile_profile(
            state_transaction,
            compile_profile_hash,
            SoraPrivateCompileProfileV1 {
                schema_version: SORA_PRIVATE_COMPILE_PROFILE_VERSION_V1,
                ..compile_profile
            },
        )
    }
}

impl Execute for isi::AllowSoracloudUploadedModel {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let isi::AllowSoracloudUploadedModel {
            apartment_name,
            service_name,
            model_name,
            model_id,
            artifact_id,
            weight_version,
            bundle_root,
            compile_profile_hash,
            privacy_mode,
            require_model_inference,
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let model_name = parse_training_model_name(&model_name)?;
        let model_id = parse_uploaded_model_id(&model_id)?;
        let artifact_id = parse_uploaded_artifact_id(&artifact_id)?;
        let weight_version = parse_model_weight_version(&weight_version)?;
        verify_uploaded_model_allow_provenance(
            authority,
            &apartment_name,
            &service_name,
            &model_name,
            &model_id,
            &artifact_id,
            &weight_version,
            bundle_root,
            compile_profile_hash,
            privacy_mode,
            require_model_inference,
            &provenance,
        )?;

        let sequence = next_soracloud_audit_sequence(state_transaction);
        let apartment_key = apartment_name.as_ref().to_owned();
        let Some(mut apartment_record) = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("agent apartment `{apartment_name}` not found").into(),
            ));
        };
        if require_model_inference
            && !agent_policy_capability_active(&apartment_record, "allow_model_inference")
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{apartment_name}` does not have active `allow_model_inference` capability"
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
        if bundle_record.compile_profile_hash != compile_profile_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("compile_profile_hash mismatch for uploaded model `{model_id}`").into(),
            ));
        }
        if state_transaction
            .world
            .soracloud_private_compile_profiles
            .get(&compile_profile_hash)
            .is_none()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "compile profile `{compile_profile_hash}` is not admitted for uploaded model `{model_id}`"
                )
                .into(),
            ));
        }

        let artifact_key = (service_name.as_ref().to_owned(), artifact_id.clone());
        let Some(artifact_record) = state_transaction
            .world
            .soracloud_model_artifacts
            .get(&artifact_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("artifact `{artifact_id}` not found for service `{service_name}`").into(),
            ));
        };
        if artifact_record.model_name != model_name
            || artifact_record.weight_version.as_deref() != Some(weight_version.as_str())
            || artifact_record.private_bundle_root != Some(bundle_root)
            || artifact_record.compile_profile_hash != Some(compile_profile_hash)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "artifact `{artifact_id}` does not match uploaded model `{model_id}` version `{weight_version}`"
                )
                .into(),
            ));
        }

        let registry_key = (service_name.as_ref().to_owned(), model_name.clone());
        let Some(registry_record) = state_transaction
            .world
            .soracloud_model_registries
            .get(&registry_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("model `{model_name}` is not registered for service `{service_name}`")
                    .into(),
            ));
        };
        if registry_record.current_version.as_deref() != Some(weight_version.as_str()) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "model `{model_name}` current version {:?} does not match uploaded binding version `{weight_version}`",
                    registry_record.current_version
                )
                .into(),
            ));
        }

        apartment_record.uploaded_model_binding = Some(SoraUploadedModelBindingV1 {
            service_name,
            model_name,
            model_id,
            artifact_id,
            weight_version,
            bundle_root,
            compile_profile_hash,
            privacy_mode,
            require_model_inference,
            status: SoraUploadedModelBindingStatusV1::Active,
            status_reason: None,
            bound_sequence: sequence,
        });
        touch_agent_runtime_activity(&mut apartment_record, sequence);
        record_agent_apartment(state_transaction, apartment_key, apartment_record)
    }
}

impl Execute for isi::StartSoracloudPrivateInference {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::StartSoracloudPrivateInference {
            session,
            provenance,
        } = self;
        verify_private_inference_start_provenance(authority, &session, &provenance)?;
        session
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;
        let session_id = parse_private_session_id(&session.session_id)?;
        let model_id = parse_uploaded_model_id(&session.model_id)?;
        let weight_version = parse_model_weight_version(&session.weight_version)?;
        if !matches!(
            session.status,
            SoraPrivateInferenceSessionStatusV1::Admitted
                | SoraPrivateInferenceSessionStatusV1::Running
        ) {
            return Err(invalid_parameter(
                "private session start status must be Admitted or Running",
            ));
        }
        if session.token_budget
            > state_transaction
                .nexus
                .uploaded_models
                .max_session_token_budget
        {
            return Err(invalid_parameter(format!(
                "token_budget exceeds nexus.uploaded_models.max_session_token_budget ({})",
                state_transaction
                    .nexus
                    .uploaded_models
                    .max_session_token_budget
            )));
        }
        if session.image_budget
            > state_transaction
                .nexus
                .uploaded_models
                .max_session_image_budget
        {
            return Err(invalid_parameter(format!(
                "image_budget exceeds nexus.uploaded_models.max_session_image_budget ({})",
                state_transaction
                    .nexus
                    .uploaded_models
                    .max_session_image_budget
            )));
        }
        if active_private_session_count(state_transaction, &session.apartment)
            >= state_transaction
                .nexus
                .uploaded_models
                .max_active_private_sessions_per_apartment
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{}` reached max active private sessions ({})",
                    session.apartment,
                    state_transaction
                        .nexus
                        .uploaded_models
                        .max_active_private_sessions_per_apartment
                )
                .into(),
            ));
        }
        if state_transaction
            .world
            .soracloud_private_inference_sessions
            .iter()
            .any(|((_apartment_name, stored_session_id), _session)| {
                stored_session_id == &session_id
            })
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("private session `{session_id}` is already registered").into(),
            ));
        }

        let apartment_key = session.apartment.as_ref().to_owned();
        let Some(apartment_record) = state_transaction
            .world
            .soracloud_agent_apartments
            .get(&apartment_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("agent apartment `{}` not found", session.apartment).into(),
            ));
        };
        let current_sequence = next_soracloud_audit_sequence(state_transaction);
        if agent_runtime_status_for_sequence(&apartment_record, current_sequence)
            == SoraAgentRuntimeStatusV1::LeaseExpired
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{}` lease expired at sequence {}; renew before starting private inference",
                    session.apartment, apartment_record.lease_expires_sequence
                )
                .into(),
            ));
        }
        agent_uploaded_model_binding_ready(&apartment_record)?;
        let Some(binding) = apartment_record.uploaded_model_binding.as_ref() else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "apartment `{}` does not have an uploaded model binding",
                    session.apartment
                )
                .into(),
            ));
        };
        if binding.service_name != session.service_name
            || binding.model_id != model_id
            || binding.weight_version != weight_version
            || binding.bundle_root != session.bundle_root
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "private session `{session_id}` does not match apartment `{}` uploaded model binding",
                    session.apartment
                )
                .into(),
            ));
        }

        let session_record = SoraPrivateInferenceSessionV1 {
            schema_version: SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1,
            session_id,
            model_id,
            weight_version,
            ..session
        };
        record_private_inference_session(state_transaction, session_record)
    }
}

impl Execute for isi::RecordSoracloudPrivateInferenceCheckpoint {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        let isi::RecordSoracloudPrivateInferenceCheckpoint {
            session_id,
            status,
            receipt_root,
            xor_cost_nanos,
            checkpoint,
        } = self;
        let session_id = parse_private_session_id(&session_id)?;
        let checkpoint_session_id = parse_private_session_id(&checkpoint.session_id)?;
        if session_id != checkpoint_session_id {
            return Err(invalid_parameter(
                "checkpoint.session_id must equal session_id",
            ));
        }
        parse_private_decrypt_request_id(&checkpoint.decrypt_request_id)?;
        checkpoint
            .validate()
            .map_err(|err| invalid_parameter(err.to_string()))?;

        let session_key = load_private_session_key(state_transaction, &session_id)?;
        let Some(mut session_record) = state_transaction
            .world
            .soracloud_private_inference_sessions
            .get(&session_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("private session `{session_id}` not found").into(),
            ));
        };
        if matches!(
            session_record.status,
            SoraPrivateInferenceSessionStatusV1::Completed
                | SoraPrivateInferenceSessionStatusV1::Failed
                | SoraPrivateInferenceSessionStatusV1::Revoked
        ) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "private session `{session_id}` is already terminal with status {:?}",
                    session_record.status
                )
                .into(),
            ));
        }
        let bundle_key = (
            session_record.service_name.as_ref().to_owned(),
            session_record.model_id.clone(),
            session_record.weight_version.clone(),
        );
        let Some(bundle_record) = state_transaction
            .world
            .soracloud_uploaded_model_bundles
            .get(&bundle_key)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "uploaded model bundle `{}` version `{}` not found for session `{session_id}`",
                    session_record.model_id, session_record.weight_version
                )
                .into(),
            ));
        };
        if bundle_record.bundle_root != session_record.bundle_root {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "private session `{session_id}` bundle root no longer matches uploaded model manifest"
                )
                .into(),
            ));
        }
        let existing_checkpoint = state_transaction
            .world
            .soracloud_private_inference_checkpoints
            .get(&(session_id.clone(), checkpoint.step))
            .cloned();
        let release_finalization = existing_checkpoint.as_ref().is_some_and(|existing| {
            session_record.status == SoraPrivateInferenceSessionStatusV1::AwaitingDecryption
                && status == SoraPrivateInferenceSessionStatusV1::Completed
                && existing.released_token.is_none()
                && existing.decrypt_request_id == checkpoint.decrypt_request_id
                && existing.compute_units == checkpoint.compute_units
                && existing.ciphertext_state_root == checkpoint.ciphertext_state_root
        });
        let runtime_delta = if release_finalization {
            0
        } else {
            bundle_record
                .pricing_policy
                .runtime_step_xor_nanos
                .saturating_mul(u128::from(checkpoint.compute_units))
        };
        let release_delta = if checkpoint.released_token.is_some() {
            bundle_record.pricing_policy.decrypt_release_xor_nanos
        } else {
            0
        };
        let expected_total_cost = session_record
            .xor_cost_nanos
            .saturating_add(runtime_delta)
            .saturating_add(release_delta);
        if xor_cost_nanos != expected_total_cost {
            return Err(invalid_parameter(format!(
                "xor_cost_nanos must equal previous total {} plus runtime {} and release {} (expected {})",
                session_record.xor_cost_nanos, runtime_delta, release_delta, expected_total_cost
            )));
        }
        let incremental_charge = xor_cost_nanos.saturating_sub(session_record.xor_cost_nanos);
        transfer_uploaded_model_amount(authority, incremental_charge, state_transaction)?;
        if xor_cost_nanos < session_record.xor_cost_nanos {
            return Err(invalid_parameter(
                "xor_cost_nanos must be monotonic for private inference checkpoints",
            ));
        }
        let max_step = state_transaction
            .world
            .soracloud_private_inference_checkpoints
            .iter()
            .filter(|((stored_session_id, _step), _checkpoint)| stored_session_id == &session_id)
            .map(|((_stored_session_id, step), _checkpoint)| *step)
            .max();
        if release_finalization {
            if max_step != Some(checkpoint.step) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "private session `{session_id}` release finalization must target the latest checkpoint step {:?}, got {}",
                        max_step, checkpoint.step
                    )
                    .into(),
                ));
            }
        } else if max_step.is_some_and(|step| checkpoint.step <= step) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "private session `{session_id}` checkpoint step {} must be greater than previous step {:?}",
                    checkpoint.step, max_step
                )
                .into(),
            ));
        }
        if status == SoraPrivateInferenceSessionStatusV1::Completed
            && checkpoint.released_token.as_deref().is_none()
        {
            return Err(invalid_parameter(
                "completed private inference checkpoints must include released_token",
            ));
        }
        if release_finalization
            && checkpoint
                .released_token
                .as_deref()
                .is_some_and(|token| token.trim().is_empty())
        {
            return Err(invalid_parameter(
                "release finalization checkpoints must include a non-empty released_token",
            ));
        }

        session_record.status = status;
        session_record.receipt_root = receipt_root;
        session_record.xor_cost_nanos = xor_cost_nanos;
        record_private_inference_session(state_transaction, session_record)?;
        record_private_inference_checkpoint(
            state_transaction,
            SoraPrivateInferenceCheckpointV1 {
                schema_version: SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
                ..checkpoint
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
        require_soracloud_permission(authority, state_transaction)?;
        write_soracloud_runtime_state(state_transaction, self.state)
    }
}

impl Execute for isi::RecordSoracloudMailboxMessage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        write_soracloud_mailbox_message(state_transaction, self.message)
    }
}

impl Execute for isi::RecordSoracloudRuntimeReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_soracloud_permission(authority, state_transaction)?;
        write_soracloud_runtime_receipt(state_transaction, self.receipt)
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

    use iroha_crypto::KeyPair;
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
            AgentApartmentManifestV1, DecryptionAuthorityModeV1, DecryptionAuthorityPolicyV1,
            DecryptionRequestV1, FheDeterministicRoundingModeV1, FheExecutionPolicyV1,
            FheJobInputRefV1, FheJobOperationV1, FheJobSpecV1, FheParamLifecycleV1, FheParamSetV1,
            FheSchemeV1, SECRET_ENVELOPE_VERSION_V1, SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
            SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1, SecretEnvelopeEncryptionV1,
            SecretEnvelopeV1, SoraArtifactKindV1, SoraArtifactRefV1, SoraCapabilityPolicyV1,
            SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1, SoraContainerManifestV1,
            SoraContainerRuntimeV1, SoraHfBackendFamilyV1, SoraHfModelFormatV1,
            SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1,
            SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1, SoraHfPlacementStatusV1,
            SoraHfResourceProfileV1, SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1,
            SoraLifecycleHooksV1, SoraModelHostCapabilityRecordV1, SoraNetworkPolicyV1,
            SoraResourceLimitsV1, SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1,
            SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceManifestV1,
            SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
            SoraStateMutationOperationV1, SoraStateScopeV1, SoraTlsModeV1,
        },
    };
    use iroha_primitives::json::Json;
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID,
    };

    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, WorldReadOnly},
    };

    fn state_with_soracloud_permission(kura: &Arc<Kura>) -> Result<State, eyre::Report> {
        let world = World::with([], [], []);
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura.clone(), query_handle);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let wonderland: iroha_data_model::domain::DomainId = "wonderland".parse()?;
        Register::domain(Domain::new(wonderland.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::account(Account::new(ALICE_ID.clone().to_account_id(wonderland)))
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
        let wonderland: iroha_data_model::domain::DomainId = "wonderland".parse()?;
        Register::account(Account::new(validator.clone().to_account_id(wonderland)))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, state_transaction)?;
        let asset_definition_id = AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
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
                "wonderland".parse().expect("domain"),
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
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            capabilities: SoraCapabilityPolicyV1 {
                network: SoraNetworkPolicyV1::Allowlist(vec!["api.example.test".to_string()]),
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
        encryption: SoraStateEncryptionV1,
        governance_tx_hash: Hash,
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
            encryption,
            governance_tx_hash,
        )
        .expect("state mutation payload");
        ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), &payload),
        }
    }

    fn sample_fhe_param_set() -> FheParamSetV1 {
        FheParamSetV1 {
            schema_version: iroha_data_model::soracloud::FHE_PARAM_SET_VERSION_V1,
            param_set: "bfv-default".parse().expect("valid name"),
            version: NonZeroU32::new(1).expect("nonzero"),
            backend: "fhe/bfv-rns/v1".to_string(),
            scheme: FheSchemeV1::Bfv,
            ciphertext_modulus_bits: vec![
                NonZeroU16::new(60).expect("nonzero"),
                NonZeroU16::new(40).expect("nonzero"),
            ],
            plaintext_modulus_bits: NonZeroU16::new(20).expect("nonzero"),
            polynomial_modulus_degree: NonZeroU32::new(8_192).expect("nonzero"),
            slot_count: NonZeroU32::new(4_096).expect("nonzero"),
            security_level_bits: NonZeroU16::new(128).expect("nonzero"),
            max_multiplicative_depth: NonZeroU16::new(1).expect("nonzero"),
            lifecycle: FheParamLifecycleV1::Active,
            activation_height: Some(1),
            deprecation_height: None,
            withdraw_height: None,
            parameter_digest: Hash::new(b"fhe-params"),
        }
    }

    fn sample_fhe_policy() -> FheExecutionPolicyV1 {
        FheExecutionPolicyV1 {
            schema_version: iroha_data_model::soracloud::FHE_EXECUTION_POLICY_VERSION_V1,
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
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

    fn sample_fhe_job() -> FheJobSpecV1 {
        FheJobSpecV1 {
            schema_version: iroha_data_model::soracloud::FHE_JOB_SPEC_VERSION_V1,
            job_id: "job-1".to_string(),
            policy_name: "analytics".parse().expect("valid name"),
            param_set: "bfv-default".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(1).expect("nonzero"),
            operation: FheJobOperationV1::Add,
            inputs: vec![
                FheJobInputRefV1 {
                    state_key: "/state/private/input-1".to_string(),
                    payload_bytes: NonZeroU64::new(512).expect("nonzero"),
                    commitment: Hash::new(b"in-1"),
                },
                FheJobInputRefV1 {
                    state_key: "/state/private/input-2".to_string(),
                    payload_bytes: NonZeroU64::new(512).expect("nonzero"),
                    commitment: Hash::new(b"in-2"),
                },
            ],
            output_state_key: "/state/private/output-1".to_string(),
            requested_multiplication_depth: 0,
            rotation_count: 0,
            bootstrap_count: 0,
        }
    }

    fn fhe_job_provenance(
        service_name: &iroha_data_model::name::Name,
        binding_name: &iroha_data_model::name::Name,
        job: FheJobSpecV1,
        policy: FheExecutionPolicyV1,
        param_set: FheParamSetV1,
        governance_tx_hash: Hash,
    ) -> ManifestProvenance {
        let payload = encode_fhe_job_run_provenance_payload(
            service_name.as_ref(),
            binding_name.as_ref(),
            job,
            policy,
            param_set,
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
            "wonderland".parse().expect("domain"),
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
            "wonderland".parse().expect("domain"),
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
            "wonderland".parse().expect("domain"),
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
            "wonderland".parse().expect("domain"),
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
            "domain".parse().expect("domain"),
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
            "domain".parse().expect("domain"),
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
            "domain".parse().expect("domain"),
            "xor".parse().expect("xor"),
        );

        {
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
            "wonderland".parse().expect("domain"),
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
            "domain".parse().expect("domain"),
            "xor".parse().expect("xor"),
        );

        {
            let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
                .as_ref()
                .header();
            let mut state_block = state.block(block_header);
            let mut stx = state_block.transaction();
            let capability = sample_model_host_capability(ALICE_ID.clone(), 1, 1_000_000);
            let wonderland: iroha_data_model::domain::DomainId = "wonderland".parse()?;

            Register::account(Account::new(BOB_ID.clone().to_account_id(wonderland)))
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
            "domain".parse().expect("domain"),
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
    fn deploy_soracloud_service_rejects_native_process_runtime() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_soracloud_permission(&kura)?;
        let mut bundle = sample_bundle("portal", "1.0.0", 0);
        bundle.container.runtime = SoraContainerRuntimeV1::NativeProcess;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let error = isi::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: bundle_provenance(&bundle),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("native-process deployment must be rejected");
        assert!(
            format!("{error}").contains("admits only `Ivm`"),
            "unexpected error: {error:?}"
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
        iroha_data_model::isi::InstructionBox::from(isi::MutateSoracloudState {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            state_key: "/state/private/patient-1".to_string(),
            operation: SoraStateMutationOperationV1::Upsert,
            value_size_bytes: Some(256),
            encryption: SoraStateEncryptionV1::Plaintext,
            governance_tx_hash,
            provenance: state_mutation_provenance(
                &service_name,
                &binding_name,
                "/state/private/patient-1",
                SoraStateMutationOperationV1::Upsert,
                Some(256),
                SoraStateEncryptionV1::Plaintext,
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
        let job = sample_fhe_job();
        let policy = sample_fhe_policy();
        let param_set = sample_fhe_param_set();
        let governance_tx_hash = Hash::new(b"gov-fhe");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudFheJob {
            service_name: service_name.clone(),
            binding_name: binding_name.clone(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            governance_tx_hash,
            provenance: fhe_job_provenance(
                &service_name,
                &binding_name,
                job.clone(),
                policy.clone(),
                param_set.clone(),
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
        assert_eq!(
            entry.payload_bytes.get(),
            job.deterministic_output_payload_bytes()
        );
        assert_eq!(
            entry.payload_commitment,
            job.deterministic_output_commitment()
        );
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
    fn private_inference_release_finalization_reuses_checkpoint_step() -> Result<(), eyre::Report> {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_with_soracloud_permission(&kura)?;
        state.nexus.get_mut().fees.fee_sink_account_id = ALICE_ID.to_string();

        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid");
        let apartment_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
        let session_id = "session-1".to_string();
        let decrypt_request_id = "decrypt-1".to_string();
        let bundle_root = Hash::new(b"bundle-root");
        let compile_profile_hash = Hash::new(b"compile-profile");
        let awaiting_receipt_root = Hash::new(b"receipt-root-awaiting");
        let completed_receipt_root = Hash::new(b"receipt-root-completed");
        let existing_checkpoint = SoraPrivateInferenceCheckpointV1 {
            schema_version: SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
            session_id: session_id.clone(),
            step: 1,
            ciphertext_state_root: Hash::new(b"ciphertext-state-root"),
            receipt_hash: Hash::new(b"receipt-hash-awaiting"),
            decrypt_request_id: decrypt_request_id.clone(),
            released_token: None,
            compute_units: 5,
            updated_at_ms: 1_000,
        };
        let existing_xor_cost_nanos = 500_u128;
        let release_xor_nanos = 17_u128;

        record_uploaded_model_bundle(
            &mut stx,
            SoraUploadedModelBundleV1 {
                schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
                service_name: service_name.clone(),
                model_id: "vision_model".to_string(),
                weight_version: "v1".to_string(),
                family: "decoder-only".to_string(),
                modalities: vec!["text".to_string()],
                plaintext_root: Hash::new(b"plaintext-root"),
                runtime_format: iroha_data_model::soracloud::SoraUploadedModelRuntimeFormatV1::SoracloudPrivateIr,
                bundle_root,
                chunk_count: 1,
                plaintext_bytes: 4_096,
                ciphertext_bytes: 4_352,
                compile_profile_hash,
                chunk_manifest_root: Hash::new(b"chunk-manifest-root"),
                upload_recipient: iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                    schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                    key_id: "soracloud-upload".to_string(),
                    key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                    kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                    aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                    public_key_bytes: vec![3u8; 32],
                    public_key_fingerprint: Hash::new([3u8; 32]),
                },
                wrapped_bundle_key: iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                    schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                    recipient_key_id: "soracloud-upload".to_string(),
                    recipient_key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                    kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                    aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                    ephemeral_public_key: vec![4u8; 32],
                    nonce: vec![5u8; 12],
                    wrapped_key_ciphertext: vec![6u8; 48],
                    ciphertext_hash: Hash::new([6u8; 48]),
                    aad_digest: Hash::new(b"wrapped-aad"),
                },
                pricing_policy: iroha_data_model::soracloud::SoraUploadedModelPricingPolicyV1 {
                    storage_xor_nanos: 100,
                    compile_xor_nanos: 200,
                    runtime_step_xor_nanos: 100,
                    decrypt_release_xor_nanos: release_xor_nanos,
                },
                decryption_policy_ref: "policy/private-release".to_string(),
            },
        )?;
        record_private_inference_session(
            &mut stx,
            SoraPrivateInferenceSessionV1 {
                schema_version: SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1,
                session_id: session_id.clone(),
                apartment: apartment_name.clone(),
                service_name: service_name.clone(),
                model_id: "vision_model".to_string(),
                weight_version: "v1".to_string(),
                bundle_root,
                input_commitments: vec![Hash::new(b"input-commitment")],
                token_budget: 128,
                image_budget: 0,
                status: SoraPrivateInferenceSessionStatusV1::AwaitingDecryption,
                receipt_root: awaiting_receipt_root,
                xor_cost_nanos: existing_xor_cost_nanos,
            },
        )?;
        record_private_inference_checkpoint(&mut stx, existing_checkpoint.clone())?;

        let completed_checkpoint = SoraPrivateInferenceCheckpointV1 {
            schema_version: SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
            session_id: session_id.clone(),
            step: existing_checkpoint.step,
            ciphertext_state_root: existing_checkpoint.ciphertext_state_root,
            receipt_hash: Hash::new(b"receipt-hash-completed"),
            decrypt_request_id: decrypt_request_id.clone(),
            released_token: Some("token-42".to_string()),
            compute_units: existing_checkpoint.compute_units,
            updated_at_ms: existing_checkpoint.updated_at_ms.saturating_add(1),
        };
        let completed_xor_cost_nanos = existing_xor_cost_nanos.saturating_add(release_xor_nanos);
        iroha_data_model::isi::InstructionBox::from(
            isi::RecordSoracloudPrivateInferenceCheckpoint {
                session_id: session_id.clone(),
                status: SoraPrivateInferenceSessionStatusV1::Completed,
                receipt_root: completed_receipt_root,
                xor_cost_nanos: completed_xor_cost_nanos,
                checkpoint: completed_checkpoint.clone(),
            },
        )
        .execute(&ALICE_ID, &mut stx)?;

        stx.apply();
        state_block.commit()?;

        let view = state.view();
        let world = view.world();
        let session = world
            .soracloud_private_inference_sessions()
            .get(&(apartment_name.as_ref().to_owned(), session_id.clone()))
            .expect("private session");
        assert_eq!(
            session.status,
            SoraPrivateInferenceSessionStatusV1::Completed
        );
        assert_eq!(session.receipt_root, completed_receipt_root);
        assert_eq!(session.xor_cost_nanos, completed_xor_cost_nanos);

        let checkpoint = world
            .soracloud_private_inference_checkpoints()
            .get(&(session_id.clone(), existing_checkpoint.step))
            .expect("private checkpoint");
        assert_eq!(checkpoint.released_token.as_deref(), Some("token-42"));
        assert_eq!(checkpoint.receipt_hash, completed_checkpoint.receipt_hash);
        assert_eq!(checkpoint.updated_at_ms, completed_checkpoint.updated_at_ms);
        assert_eq!(checkpoint.compute_units, existing_checkpoint.compute_units);
        assert_eq!(
            world
                .soracloud_private_inference_checkpoints()
                .iter()
                .filter(
                    |((stored_session_id, _step), _checkpoint)| stored_session_id == &session_id
                )
                .count(),
            1
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
        assert_eq!(
            ops_record.autonomy_run_history[0]
                .workflow_input_json
                .as_deref(),
            Some(
                "{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}"
            )
        );
        let autonomy_event = world
            .soracloud_agent_apartment_audit_events()
            .get(&ops_record.autonomy_run_history[0].approved_sequence)
            .expect("autonomy audit event");
        assert_eq!(
            autonomy_event.payload_hash,
            Some(Hash::new(
                b"{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}"
            ))
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
