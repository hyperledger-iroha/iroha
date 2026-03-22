//! Soracloud lifecycle and private-runtime instruction handlers.

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        soracloud as isi,
    },
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheExecutionPolicyV1, FheJobSpecV1,
        FheParamSetV1, SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        SORA_AGENT_APARTMENT_RECORD_VERSION_V1, SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
        SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1, SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
        SORA_HF_SHARED_LEASE_POOL_VERSION_V1, SORA_HF_SOURCE_RECORD_VERSION_V1,
        SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1, SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
        SORA_MODEL_REGISTRY_VERSION_V1, SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
        SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1, SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        SORA_SERVICE_STATE_ENTRY_VERSION_V1, SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
        SORA_TRAINING_JOB_RECORD_VERSION_V1, SoraAgentApartmentActionV1,
        SoraAgentApartmentAuditEventV1, SoraAgentApartmentRecordV1, SoraAgentArtifactAllowRuleV1,
        SoraAgentAutonomyRunRecordV1, SoraAgentMailboxMessageV1, SoraAgentPersistentStateV1,
        SoraAgentRuntimeStatusV1, SoraAgentWalletDailySpendEntryV1, SoraAgentWalletSpendRequestV1,
        SoraDecryptionRequestRecordV1, SoraDeploymentBundleV1, SoraHfSharedLeaseActionV1,
        SoraHfSharedLeaseAuditEventV1, SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1,
        SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1,
        SoraHfSourceStatusV1, SoraModelArtifactActionV1, SoraModelArtifactAuditEventV1,
        SoraModelArtifactRecordV1, SoraModelProvenanceKindV1, SoraModelProvenanceRefV1,
        SoraModelRegistryV1, SoraModelWeightActionV1, SoraModelWeightAuditEventV1,
        SoraModelWeightVersionRecordV1, SoraRolloutStageV1, SoraRuntimeReceiptV1,
        SoraServiceAuditEventV1, SoraServiceDeploymentStateV1, SoraServiceLifecycleActionV1,
        SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1, SoraServiceRuntimeStateV1,
        SoraServiceStateEntryV1, SoraStateEncryptionV1, SoraStateMutationOperationV1,
        SoraTrainingJobActionV1, SoraTrainingJobAuditEventV1, SoraTrainingJobRecordV1,
        SoraTrainingJobStatusV1, encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_bundle_provenance_payload,
        encode_decryption_request_provenance_payload, encode_fhe_job_run_provenance_payload,
        encode_hf_shared_lease_join_provenance_payload,
        encode_hf_shared_lease_leave_provenance_payload,
        encode_hf_shared_lease_renew_provenance_payload,
        encode_model_artifact_register_provenance_payload,
        encode_model_weight_promote_provenance_payload,
        encode_model_weight_register_provenance_payload,
        encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_state_mutation_provenance_payload,
        encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
    },
    sorafs::pin_registry::StorageClass,
};
use mv::storage::StorageReadOnly;

use super::*;
use crate::{smartcontracts::Execute, state::StateTransaction};

const CAN_MANAGE_SORACLOUD_PERMISSION: &str = "CanManageSoracloud";
const TRAINING_MAX_RETRIES: u8 = 16;
const TRAINING_MAX_WORKER_GROUP_SIZE: u16 = 1024;
const TRAINING_MAX_REASON_BYTES: usize = 512;
const TRAINING_MAX_IDENTIFIER_BYTES: usize = 128;
const MODEL_WEIGHT_MAX_DATASET_REF_BYTES: usize = 512;
const MODEL_WEIGHT_MAX_REASON_BYTES: usize = 512;
const HF_REPO_ID_MAX_BYTES: usize = 256;
const HF_REVISION_MAX_BYTES: usize = 160;
const HF_MODEL_NAME_MAX_BYTES: usize = 128;
const AGENT_WALLET_DAY_TICKS: u64 = 10_000;
const AGENT_MAILBOX_MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_LABEL_BYTES: usize = 128;

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
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

fn verify_bundle_provenance(
    authority: &AccountId,
    bundle: &SoraDeploymentBundleV1,
    provenance: &ManifestProvenance,
) -> Result<(), InstructionExecutionError> {
    if authority.signatory() != &provenance.signer {
        return Err(invalid_parameter(
            "bundle provenance signer must match the transaction authority",
        ));
    }
    let payload = encode_bundle_provenance_payload(bundle)
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
) -> u64 {
    let mut value_size = u64::try_from(artifact_hash.len()).unwrap_or(u64::MAX);
    value_size = value_size.saturating_add(u64::try_from(run_label.len()).unwrap_or(u64::MAX));
    value_size = value_size
        .saturating_add(u64::try_from(budget_units.to_string().len()).unwrap_or(u64::MAX));
    if let Some(hash) = provenance_hash {
        value_size = value_size.saturating_add(u64::try_from(hash.len()).unwrap_or(u64::MAX));
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
            record.training_job_id.clone(),
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

fn resolve_fee_sink_account(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AccountId, InstructionExecutionError> {
    crate::block::parse_account_literal_with_world(
        &state_transaction.world,
        &state_transaction.nexus.fees.fee_sink_account_id,
    )
    .or_else(|| {
        AccountId::parse_encoded(&state_transaction.nexus.fees.fee_sink_account_id)
            .map(|parsed| parsed.into_account_id())
            .ok()
    })
    .ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "invalid nexus.fees.fee_sink_account_id; expected account identifier".into(),
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
    let members = canonical_hf_member_order(state_transaction, pool_id);
    for mut member in members {
        member.status = SoraHfSharedLeaseMemberStatusV1::Left;
        member.updated_at_ms = updated_at_ms;
        member.last_charge_nanos = 0;
        record_hf_shared_lease_member(state_transaction, member)?;
    }
    Ok(())
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
    provenance: ManifestProvenance,
    action: SoraServiceLifecycleActionV1,
) -> Result<(), InstructionExecutionError> {
    require_soracloud_permission(authority, state_transaction)?;
    verify_bundle_provenance(authority, &bundle, &provenance)?;
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
        source_record.model_name = model_name.clone();
        source_record.updated_at_ms = now_ms;
        if source_record.status == SoraHfSourceStatusV1::Retired {
            source_record.status = SoraHfSourceStatusV1::PendingImport;
        }
        record_hf_source(state_transaction, source_record)?;

        let member_key = (pool_id.to_string(), authority.to_string());
        let existing_member = state_transaction
            .world
            .soracloud_hf_shared_lease_members
            .get(&member_key)
            .cloned();
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
            expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
            pool.active_member_count = 0;
            pool.status = SoraHfSharedLeaseStatusV1::Expired;
            record_hf_shared_lease_pool(state_transaction, pool.clone())?;
        }

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
                base_fee_nanos.saturating_mul(u128::from(remaining_ms)) / u128::from(lease_term_ms);
            let join_fee_nanos = remaining_fee_nanos
                / u128::try_from(existing_members.len().saturating_add(1)).unwrap_or(u128::MAX);

            if !existing_members.is_empty() && join_fee_nanos > 0 {
                let member_count_u128 = u128::try_from(existing_members.len()).unwrap_or(u128::MAX);
                let base_refund_nanos = join_fee_nanos / member_count_u128;
                let remainder = usize::try_from(join_fee_nanos % member_count_u128).unwrap_or(0);
                for (index, existing_member) in existing_members.iter_mut().enumerate() {
                    let refund_nanos = base_refund_nanos + u128::from((index < remainder) as u8);
                    transfer_hf_shared_lease_amount(
                        authority,
                        &lease_asset_definition_id,
                        refund_nanos,
                        &existing_member.account_id,
                        state_transaction,
                    )?;
                    existing_member.total_refunded_nanos = existing_member
                        .total_refunded_nanos
                        .saturating_add(refund_nanos);
                    existing_member.updated_at_ms = now_ms;
                    record_hf_shared_lease_member(state_transaction, existing_member.clone())?;
                }
            }

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
                service_bindings: std::collections::BTreeSet::new(),
                apartment_bindings: std::collections::BTreeSet::new(),
            });
            member.status = SoraHfSharedLeaseMemberStatusV1::Active;
            member.joined_at_ms = now_ms;
            member.updated_at_ms = now_ms;
            member.total_paid_nanos = member.total_paid_nanos.saturating_add(join_fee_nanos);
            member.last_charge_nanos = join_fee_nanos;
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

        let sink_account = resolve_fee_sink_account(state_transaction)?;
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            base_fee_nanos,
            &sink_account,
            state_transaction,
        )?;

        let pool = SoraHfSharedLeasePoolV1 {
            schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            pool_id,
            source_id,
            storage_class,
            lease_asset_definition_id,
            base_fee_nanos,
            lease_term_ms,
            window_started_at_ms: now_ms,
            window_expires_at_ms: now_ms.saturating_add(lease_term_ms),
            active_member_count: 1,
            status: SoraHfSharedLeaseStatusV1::Active,
        };
        let previous_paid_nanos = existing_member
            .as_ref()
            .map(|member| member.total_paid_nanos)
            .unwrap_or(0);
        let previous_refunded_nanos = existing_member
            .as_ref()
            .map(|member| member.total_refunded_nanos)
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
            service_bindings: std::collections::BTreeSet::new(),
            apartment_bindings: std::collections::BTreeSet::new(),
        };
        bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());

        record_hf_shared_lease_pool(state_transaction, pool.clone())?;
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
            expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
            pool.active_member_count = 0;
            pool.status = SoraHfSharedLeaseStatusV1::Expired;
            record_hf_shared_lease_pool(state_transaction, pool.clone())?;
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

        member.status = SoraHfSharedLeaseMemberStatusV1::Left;
        member.updated_at_ms = now_ms;
        member.last_charge_nanos = 0;
        member.service_bindings.clear();
        member.apartment_bindings.clear();
        record_hf_shared_lease_member(state_transaction, member)?;

        let remaining_members = canonical_hf_member_order(state_transaction, &pool_id);
        pool.active_member_count = u32::try_from(remaining_members.len()).unwrap_or(u32::MAX);
        if remaining_members.is_empty() {
            pool.status = SoraHfSharedLeaseStatusV1::Draining;
            // TODO: replace immediate clamp with configurable drain grace once SCR eviction is wired.
            pool.window_expires_at_ms = now_ms;
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
        if pool.window_expires_at_ms > now_ms && pool.status == SoraHfSharedLeaseStatusV1::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                "renewal before current-window expiry is not yet supported".into(),
            ));
        }

        expire_hf_shared_lease_members(state_transaction, &pool_id, now_ms)?;
        let sink_account = resolve_fee_sink_account(state_transaction)?;
        transfer_hf_shared_lease_amount(
            authority,
            &lease_asset_definition_id,
            base_fee_nanos,
            &sink_account,
            state_transaction,
        )?;

        source_record.model_name = model_name;
        source_record.updated_at_ms = now_ms;
        if source_record.status == SoraHfSourceStatusV1::Retired {
            source_record.status = SoraHfSourceStatusV1::PendingImport;
        }
        record_hf_source(state_transaction, source_record)?;

        pool.lease_asset_definition_id = lease_asset_definition_id;
        pool.base_fee_nanos = base_fee_nanos;
        pool.window_started_at_ms = now_ms;
        pool.window_expires_at_ms = now_ms.saturating_add(lease_term_ms);
        pool.active_member_count = 1;
        pool.status = SoraHfSharedLeaseStatusV1::Active;
        record_hf_shared_lease_pool(state_transaction, pool.clone())?;

        let member_key = (pool_id.to_string(), authority.to_string());
        let previous_member = state_transaction
            .world
            .soracloud_hf_shared_lease_members
            .get(&member_key)
            .cloned();
        let previous_paid_nanos = previous_member
            .as_ref()
            .map(|member| member.total_paid_nanos)
            .unwrap_or(0);
        let previous_refunded_nanos = previous_member
            .as_ref()
            .map(|member| member.total_refunded_nanos)
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
            service_bindings: std::collections::BTreeSet::new(),
            apartment_bindings: std::collections::BTreeSet::new(),
        };
        bind_hf_shared_lease_targets(&mut member, &service_name, apartment_name.as_ref());
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
        let spend_limit = record
            .manifest
            .spend_limits
            .iter()
            .find(|limit| limit.asset_definition == normalized_asset_definition)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "apartment `{apartment_name}` has no spend limit configured for asset `{normalized_asset_definition}`"
                    )
                    .into(),
                )
            })?;
        if amount_nanos > spend_limit.max_per_tx_nanos.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "requested amount {amount_nanos} exceeds max_per_tx_nanos {} for asset `{normalized_asset_definition}`",
                    spend_limit.max_per_tx_nanos.get()
                )
                .into(),
            ));
        }

        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(&record, &normalized_asset_definition, day_bucket);
        let projected_day_spent = current_day_spent.checked_add(amount_nanos).ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("wallet daily spend overflow for apartment `{apartment_name}`").into(),
            )
        })?;
        if projected_day_spent > spend_limit.max_per_day_nanos.get() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "projected daily spend {projected_day_spent} exceeds max_per_day_nanos {} for asset `{normalized_asset_definition}`",
                    spend_limit.max_per_day_nanos.get()
                )
                .into(),
            ));
        }

        let request_id = format!("{apartment_key}:wallet:{sequence}");
        let action = if agent_policy_capability_active(&record, "wallet.auto_approve") {
            wallet_record_spend(
                &mut record,
                &normalized_asset_definition,
                day_bucket,
                projected_day_spent,
            );
            SoraAgentApartmentActionV1::WalletSpendApproved
        } else {
            record.pending_wallet_requests.insert(
                request_id.clone(),
                SoraAgentWalletSpendRequestV1 {
                    request_id: request_id.clone(),
                    asset_definition: normalized_asset_definition.clone(),
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
                asset_definition: Some(normalized_asset_definition),
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
        let spend_limit = record
            .manifest
            .spend_limits
            .iter()
            .find(|limit| limit.asset_definition == pending.asset_definition)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "apartment `{apartment_name}` has no spend limit configured for asset `{}`",
                        pending.asset_definition
                    )
                    .into(),
                )
            })?;
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
            provenance,
        } = self;
        require_soracloud_permission(authority, state_transaction)?;
        let normalized_artifact_hash = normalize_agent_hash_like("artifact_hash", &artifact_hash)?;
        let normalized_provenance_hash =
            normalize_optional_agent_hash_like("provenance_hash", provenance_hash.as_deref())?;
        let normalized_run_label = normalize_agent_run_label(&run_label)?;
        let payload = encode_agent_autonomy_run_provenance_payload(
            apartment_name.as_ref(),
            normalized_artifact_hash.as_str(),
            normalized_provenance_hash.as_deref(),
            budget_units,
            normalized_run_label.as_str(),
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
        let checkpoint_key = autonomy_checkpoint_key(&apartment_key, &run_id);
        let checkpoint_value_size = autonomy_checkpoint_value_size(
            &normalized_artifact_hash,
            normalized_provenance_hash.as_deref(),
            &normalized_run_label,
            budget_units,
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
                payload_hash: None,
                artifact_hash: Some(normalized_artifact_hash),
                provenance_hash: normalized_provenance_hash,
                run_id: Some(run_id),
                run_label: Some(normalized_run_label),
                budget_units: Some(budget_units),
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
                consumed_by_version: None,
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
    };

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        Encode,
        account::Account,
        domain::Domain,
        isi::Grant,
        permission::Permission,
        prelude::Register,
        soracloud::{
            AgentApartmentManifestV1, DecryptionAuthorityModeV1, DecryptionAuthorityPolicyV1,
            DecryptionRequestV1, FheDeterministicRoundingModeV1, FheExecutionPolicyV1,
            FheJobInputRefV1, FheJobOperationV1, FheJobSpecV1, FheParamLifecycleV1, FheParamSetV1,
            FheSchemeV1, SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1, SoraArtifactKindV1,
            SoraArtifactRefV1, SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1,
            SoraContainerManifestRefV1, SoraContainerManifestV1, SoraContainerRuntimeV1,
            SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1, SoraLifecycleHooksV1,
            SoraNetworkPolicyV1, SoraResourceLimitsV1, SoraRolloutPolicyV1, SoraRouteTargetV1,
            SoraRouteVisibilityV1, SoraServiceHandlerClassV1, SoraServiceHandlerV1,
            SoraServiceManifestV1, SoraStateBindingV1, SoraStateEncryptionV1,
            SoraStateMutabilityV1, SoraStateMutationOperationV1, SoraStateScopeV1, SoraTlsModeV1,
        },
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID};

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
        state_transaction.apply();
        state_block.commit()?;
        Ok(state)
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
        let payload = encode_bundle_provenance_payload(bundle).expect("bundle payload");
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
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
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
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
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
            provenance: bundle_provenance(&deploy_bundle),
        }
        .execute(&ALICE_ID, &mut stx)?;
        isi::UpgradeSoracloudService {
            bundle: upgrade_bundle.clone(),
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

        let wallet_spend_payload =
            encode_agent_wallet_spend_provenance_payload(ops_name.as_ref(), "xor#sora", 1_000_000)
                .expect("wallet spend payload");
        iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
            apartment_name: ops_name.clone(),
            asset_definition: "xor#sora".to_string(),
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
        )
        .expect("autonomy run payload");
        iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
            apartment_name: ops_name,
            artifact_hash: "hash:artifact#1".to_string(),
            provenance_hash: Some("hash:prov#1".to_string()),
            budget_units: 120,
            run_label: "nightly-batch-1".to_string(),
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
                .get("xor#sora:0")
                .expect("wallet day aggregate")
                .spent_nanos,
            1_000_000
        );
        assert_eq!(ops_record.autonomy_budget_remaining_units, 380);
        assert_eq!(ops_record.autonomy_run_history.len(), 1);
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
}
