//! App-facing Soracloud control-plane shim.
//!
//! This module provides a deterministic control-plane surface for
//! `deploy`/`upgrade`/`rollback` workflows plus SCR host-admission snapshots.
//! Requests must carry signed payloads so admission can verify manifest
//! provenance before mutating authoritative control-plane state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    num::NonZeroU64,
    path::PathBuf,
    time::Duration,
};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_core::soracloud_runtime::{
    HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS, HF_GENERATED_AGENT_LEASE_TICKS,
    SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentExecutionRequest,
    SoracloudLocalReadKind, SoracloudPrivateInferenceExecutionAction,
    SoracloudPrivateInferenceExecutionRequest, SoracloudPrivateInferenceExecutionResult,
    SoracloudRuntimeHfSourcePlan, SoracloudRuntimeHfSourceStatus,
    SoracloudUploadedModelEncryptionRecipient, build_soracloud_hf_generated_agent_manifest,
    build_soracloud_hf_generated_service_bundle, soracloud_hf_generated_source_binding,
};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{
    Encode,
    account::AccountId,
    asset::AssetDefinitionId,
    isi::{self, InstructionBox},
    name::Name,
    prelude::ExposedPrivateKey,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, CIPHERTEXT_QUERY_PROOF_VERSION_V1,
        CIPHERTEXT_QUERY_RESPONSE_VERSION_V1, CiphertextInclusionProofV1,
        CiphertextQueryMetadataLevelV1, CiphertextQueryResponseV1, CiphertextQueryResultItemV1,
        CiphertextQuerySpecV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1,
        FheExecutionPolicyV1, FheGovernanceBundleV1, FheJobSpecV1, FheParamSetV1,
        SecretEnvelopeEncryptionV1, SecretEnvelopeV1, SoraAgentApartmentActionV1,
        SoraAgentApartmentAuditEventV1, SoraAgentApartmentRecordV1, SoraAgentArtifactAllowRuleV1,
        SoraAgentAutonomyRunRecordV1, SoraAgentMailboxMessageV1, SoraAgentRuntimeStatusV1,
        SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerRuntimeV1,
        SoraDecryptionRequestRecordV1, SoraDeploymentBundleV1, SoraHfBackendFamilyV1,
        SoraHfModelFormatV1, SoraHfPlacementRecordV1, SoraHfResourceProfileV1,
        SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1, SoraHfSharedLeaseMemberStatusV1,
        SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1,
        SoraHfSourceRecordV1, SoraHfSourceStatusV1, SoraModelArtifactActionV1,
        SoraModelArtifactAuditEventV1, SoraModelArtifactRecordV1, SoraModelHostCapabilityRecordV1,
        SoraModelPrivacyModeV1, SoraModelProvenanceKindV1, SoraModelRegistryV1,
        SoraModelWeightActionV1, SoraModelWeightAuditEventV1, SoraModelWeightVersionRecordV1,
        SoraNetworkPolicyV1, SoraPrivateCompileProfileV1, SoraPrivateInferenceCheckpointV1,
        SoraPrivateInferenceSessionStatusV1, SoraPrivateInferenceSessionV1, SoraRolloutStageV1,
        SoraRuntimeReceiptV1, SoraServiceAuditEventV1, SoraServiceConfigEntryV1,
        SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceLifecycleActionV1,
        SoraServiceRolloutStateV1, SoraServiceSecretEntryV1, SoraStateBindingV1,
        SoraStateEncryptionV1, SoraStateMutabilityV1, SoraStateMutationOperationV1,
        SoraTrainingJobActionV1, SoraTrainingJobAuditEventV1, SoraTrainingJobRecordV1,
        SoraTrainingJobStatusV1, SoraUploadedModelBindingV1, SoraUploadedModelBundleV1,
        SoraUploadedModelChunkV1, SoraUploadedModelEncryptionRecipientV1,
        encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_bundle_provenance_payload,
        encode_ciphertext_query_provenance_payload, encode_decryption_request_provenance_payload,
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
        encode_private_inference_output_release_provenance_payload,
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
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
#[cfg(test)]
use tokio::sync::RwLock;

use crate::{JsonBody, NoritoJson, NoritoQuery, SharedAppState};

const CONTROL_PLANE_SCHEMA_VERSION: u16 = 1;
const DEFAULT_AUDIT_LIMIT: usize = 20;
const MAX_AUDIT_LIMIT: usize = 500;
const AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS: u64 = 1_000;
const AGENT_WALLET_DAY_TICKS: u64 = 10_000;
const AGENT_MAILBOX_MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_LABEL_BYTES: usize = 128;
const AGENT_AUTONOMY_RECENT_RUN_LIMIT: usize = 20;
const CIPHERTEXT_QUERY_PROOF_SCHEME_V1: &str = "soracloud.audit_anchor.v1";
const HEALTH_COMPLIANCE_REPORT_VERSION_V1: u16 = 1;
const DEFAULT_HEALTH_COMPLIANCE_LIMIT: usize = 50;
const MAX_HEALTH_COMPLIANCE_LIMIT: usize = 500;
const TRAINING_JOB_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const TRAINING_MAX_RETRIES: u8 = 16;
const TRAINING_MAX_WORKER_GROUP_SIZE: u16 = 1024;
const TRAINING_MAX_REASON_BYTES: usize = 512;
pub(crate) const VERIFIED_ACCOUNT_HEADER: &str = "x-iroha-internal-soracloud-account";
pub(crate) const VERIFIED_SIGNER_HEADER: &str = "x-iroha-internal-soracloud-signer";

pub(crate) fn requires_signed_mutation_request(method: &axum::http::Method, path: &str) -> bool {
    method == axum::http::Method::POST && path.starts_with("/v1/soracloud/")
}
const TRAINING_MAX_IDENTIFIER_BYTES: usize = 128;
const MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const MODEL_WEIGHT_MAX_DATASET_REF_BYTES: usize = 512;
const MODEL_WEIGHT_MAX_REASON_BYTES: usize = 512;
const MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const UPLOADED_MODEL_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const PRIVATE_INFERENCE_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const HF_SHARED_LEASE_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const HF_REPO_ID_MAX_BYTES: usize = 256;
const HF_REVISION_MAX_BYTES: usize = 160;
const HF_MODEL_NAME_MAX_BYTES: usize = 128;
const HF_DEFAULT_RESOLVED_REVISION: &str = "main";
const SCR_HOST_MAX_CPU_MILLIS: u32 = 64_000;
const SCR_HOST_MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const SCR_HOST_MAX_EPHEMERAL_STORAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024 * 1024;
const SCR_HOST_MAX_OPEN_FILES: u32 = 131_072;
const SCR_HOST_MAX_TASKS: u16 = 16_384;
const SCR_HOST_MAX_START_GRACE_SECS: u32 = 600;
const SCR_HOST_MAX_STOP_GRACE_SECS: u32 = 600;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
    ConfigMutation,
    SecretMutation,
    StateMutation,
    FheJobRun,
    DecryptionRequest,
    CiphertextQuery,
    Rollout,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum AgentApartmentAction {
    Deploy,
    LeaseRenew,
    Restart,
    WalletSpendRequested,
    WalletSpendApproved,
    PolicyRevoked,
    MessageEnqueued,
    MessageAcknowledged,
    ArtifactAllowed,
    AutonomyRunApproved,
    AutonomyRunExecuted,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "status", content = "value")]
pub(crate) enum AgentRuntimeStatus {
    Running,
    LeaseExpired,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum TrainingJobAction {
    Start,
    Checkpoint,
    Retry,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "status", content = "value")]
pub(crate) enum TrainingJobStatus {
    Running,
    Completed,
    RetryPending,
    Exhausted,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum ModelWeightAction {
    Register,
    Promote,
    Rollback,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum ModelArtifactAction {
    Register,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum UploadedModelAction {
    Init,
    Chunk,
    Finalize,
    Compile,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum UploadedModelBindingAction {
    Allow,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "action", content = "value")]
pub(crate) enum PrivateInferenceAction {
    Start,
    OutputRelease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedBundleRequest {
    pub bundle: SoraDeploymentBundleV1,
    #[norito(default)]
    pub initial_service_configs: BTreeMap<String, Json>,
    #[norito(default)]
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RollbackPayload {
    pub service_name: String,
    #[norito(default)]
    pub target_version: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedRollbackRequest {
    pub payload: RollbackPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "operation", content = "value")]
pub(crate) enum StateMutationOperation {
    Upsert,
    Delete,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct StateMutationRequest {
    pub service_name: String,
    pub binding_name: String,
    pub key: String,
    pub operation: StateMutationOperation,
    #[norito(default)]
    pub value_size_bytes: Option<u64>,
    pub encryption: SoraStateEncryptionV1,
    pub governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedStateMutationRequest {
    pub payload: StateMutationRequest,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceConfigSetRequest {
    pub service_name: String,
    pub config_name: String,
    pub value_json: Json,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedServiceConfigSetRequest {
    pub payload: ServiceConfigSetRequest,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceConfigDeleteRequest {
    pub service_name: String,
    pub config_name: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedServiceConfigDeleteRequest {
    pub payload: ServiceConfigDeleteRequest,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceSecretSetRequest {
    pub service_name: String,
    pub secret_name: String,
    pub secret: SecretEnvelopeV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedServiceSecretSetRequest {
    pub payload: ServiceSecretSetRequest,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceSecretDeleteRequest {
    pub service_name: String,
    pub secret_name: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedServiceSecretDeleteRequest {
    pub payload: ServiceSecretDeleteRequest,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "stage", content = "value")]
pub(crate) enum RolloutStage {
    Canary,
    Promoted,
    RolledBack,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RolloutAdvancePayload {
    pub service_name: String,
    pub rollout_handle: String,
    pub healthy: bool,
    #[norito(default)]
    pub promote_to_percent: Option<u8>,
    pub governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedRolloutAdvanceRequest {
    pub payload: RolloutAdvancePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentDeployPayload {
    pub manifest: AgentApartmentManifestV1,
    pub lease_ticks: u64,
    #[norito(default)]
    pub autonomy_budget_units: Option<u64>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentDeployRequest {
    pub payload: AgentDeployPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentLeaseRenewPayload {
    pub apartment_name: String,
    pub lease_ticks: u64,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentLeaseRenewRequest {
    pub payload: AgentLeaseRenewPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct HfDeployPayload {
    pub repo_id: String,
    #[norito(default)]
    pub revision: Option<String>,
    pub model_name: String,
    pub service_name: String,
    #[norito(default)]
    pub apartment_name: Option<String>,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    pub lease_asset_definition_id: AssetDefinitionId,
    pub base_fee_nanos: u128,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedHfDeployRequest {
    pub payload: HfDeployPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub generated_service_provenance: Option<ManifestProvenance>,
    #[norito(default)]
    pub generated_apartment_provenance: Option<ManifestProvenance>,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct HfLeaseLeavePayload {
    pub repo_id: String,
    #[norito(default)]
    pub revision: Option<String>,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub apartment_name: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedHfLeaseLeaveRequest {
    pub payload: HfLeaseLeavePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct HfLeaseRenewPayload {
    pub repo_id: String,
    #[norito(default)]
    pub revision: Option<String>,
    pub model_name: String,
    pub service_name: String,
    #[norito(default)]
    pub apartment_name: Option<String>,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    pub lease_asset_definition_id: AssetDefinitionId,
    pub base_fee_nanos: u128,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedHfLeaseRenewRequest {
    pub payload: HfLeaseRenewPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub generated_service_provenance: Option<ManifestProvenance>,
    #[norito(default)]
    pub generated_apartment_provenance: Option<ManifestProvenance>,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelHostAdvertisePayload {
    pub capability: SoraModelHostCapabilityRecordV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelHostAdvertiseRequest {
    pub payload: ModelHostAdvertisePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelHostHeartbeatPayload {
    pub validator_account_id: AccountId,
    pub heartbeat_expires_at_ms: u64,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelHostHeartbeatRequest {
    pub payload: ModelHostHeartbeatPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelHostWithdrawPayload {
    pub validator_account_id: AccountId,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelHostWithdrawRequest {
    pub payload: ModelHostWithdrawPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentRestartPayload {
    pub apartment_name: String,
    pub reason: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentRestartRequest {
    pub payload: AgentRestartPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentPolicyRevokePayload {
    pub apartment_name: String,
    pub capability: String,
    #[norito(default)]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentPolicyRevokeRequest {
    pub payload: AgentPolicyRevokePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentWalletSpendPayload {
    pub apartment_name: String,
    pub asset_definition: String,
    pub amount_nanos: u64,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentWalletSpendRequest {
    pub payload: AgentWalletSpendPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentWalletApprovePayload {
    pub apartment_name: String,
    pub request_id: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentWalletApproveRequest {
    pub payload: AgentWalletApprovePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentMessageSendPayload {
    pub from_apartment: String,
    pub to_apartment: String,
    pub channel: String,
    pub payload: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentMessageSendRequest {
    pub payload: AgentMessageSendPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentMessageAckPayload {
    pub apartment_name: String,
    pub message_id: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentMessageAckRequest {
    pub payload: AgentMessageAckPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentArtifactAllowPayload {
    pub apartment_name: String,
    pub artifact_hash: String,
    #[norito(default)]
    pub provenance_hash: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentArtifactAllowRequest {
    pub payload: AgentArtifactAllowPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentAutonomyRunPayload {
    pub apartment_name: String,
    pub artifact_hash: String,
    #[norito(default)]
    pub provenance_hash: Option<String>,
    pub budget_units: u64,
    pub run_label: String,
    #[norito(default)]
    pub workflow_input_json: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentAutonomyRunRequest {
    pub payload: AgentAutonomyRunPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentAutonomyFinalizeRequest {
    pub apartment_name: String,
    pub run_id: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct FheJobRunPayload {
    pub service_name: String,
    pub binding_name: String,
    pub job: FheJobSpecV1,
    pub policy: FheExecutionPolicyV1,
    pub param_set: FheParamSetV1,
    pub governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedFheJobRunRequest {
    pub payload: FheJobRunPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct TrainingJobStartPayload {
    pub service_name: String,
    pub model_name: String,
    pub job_id: String,
    pub worker_group_size: u16,
    pub target_steps: u32,
    pub checkpoint_interval_steps: u32,
    pub max_retries: u8,
    pub step_compute_units: u64,
    pub compute_budget_units: u64,
    pub storage_budget_bytes: u64,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedTrainingJobStartRequest {
    pub payload: TrainingJobStartPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct TrainingJobCheckpointPayload {
    pub service_name: String,
    pub job_id: String,
    pub completed_step: u32,
    pub checkpoint_size_bytes: u64,
    pub metrics_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedTrainingJobCheckpointRequest {
    pub payload: TrainingJobCheckpointPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct TrainingJobRetryPayload {
    pub service_name: String,
    pub job_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedTrainingJobRetryRequest {
    pub payload: TrainingJobRetryPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelWeightRegisterPayload {
    pub service_name: String,
    pub model_name: String,
    pub weight_version: String,
    pub training_job_id: String,
    #[norito(default)]
    pub parent_version: Option<String>,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelWeightRegisterRequest {
    pub payload: ModelWeightRegisterPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelWeightPromotePayload {
    pub service_name: String,
    pub model_name: String,
    pub weight_version: String,
    pub gate_approved: bool,
    pub gate_report_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelWeightPromoteRequest {
    pub payload: ModelWeightPromotePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelWeightRollbackPayload {
    pub service_name: String,
    pub model_name: String,
    pub target_version: String,
    pub reason: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelWeightRollbackRequest {
    pub payload: ModelWeightRollbackPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ModelArtifactRegisterPayload {
    pub service_name: String,
    pub model_name: String,
    pub training_job_id: String,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedModelArtifactRegisterRequest {
    pub payload: ModelArtifactRegisterPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct UploadedModelBundleInitPayload {
    pub bundle: SoraUploadedModelBundleV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedUploadedModelBundleInitRequest {
    pub payload: UploadedModelBundleInitPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct UploadedModelChunkPayload {
    pub chunk: SoraUploadedModelChunkV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedUploadedModelChunkRequest {
    pub payload: UploadedModelChunkPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct UploadedModelFinalizePayload {
    pub service_name: String,
    pub model_name: String,
    pub model_id: String,
    pub artifact_id: String,
    pub weight_version: String,
    pub bundle_root: Hash,
    pub privacy_mode: SoraModelPrivacyModeV1,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedUploadedModelFinalizeRequest {
    pub payload: UploadedModelFinalizePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct PrivateCompilePayload {
    pub service_name: String,
    pub model_id: String,
    pub weight_version: String,
    pub bundle_root: Hash,
    pub compile_profile: SoraPrivateCompileProfileV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedPrivateCompileRequest {
    pub payload: PrivateCompilePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct UploadedModelAllowPayload {
    pub apartment_name: String,
    pub service_name: String,
    pub model_name: String,
    pub model_id: String,
    pub artifact_id: String,
    pub weight_version: String,
    pub bundle_root: Hash,
    pub compile_profile_hash: Hash,
    pub privacy_mode: SoraModelPrivacyModeV1,
    pub require_model_inference: bool,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedUploadedModelAllowRequest {
    pub payload: UploadedModelAllowPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct PrivateInferenceRunPayload {
    pub session: SoraPrivateInferenceSessionV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedPrivateInferenceRunRequest {
    pub payload: PrivateInferenceRunPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct PrivateInferenceOutputReleasePayload {
    pub session_id: String,
    pub decrypt_request_id: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedPrivateInferenceOutputReleaseRequest {
    pub payload: PrivateInferenceOutputReleasePayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct PrivateInferenceFinalizeRequest {
    pub session_id: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct DecryptionRequestPayload {
    pub service_name: String,
    pub policy: DecryptionAuthorityPolicyV1,
    pub request: DecryptionRequestV1,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedDecryptionRequest {
    pub payload: DecryptionRequestPayload,
    pub provenance: ManifestProvenance,
    #[norito(default)]
    pub authority: Option<AccountId>,
    #[norito(default)]
    pub private_key: Option<ExposedPrivateKey>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedCiphertextQueryRequest {
    pub query: CiphertextQuerySpecV1,
    pub provenance: ManifestProvenance,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RolloutResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub rollout_handle: String,
    pub stage: RolloutStage,
    pub current_version: String,
    pub traffic_percent: u8,
    pub health_failures: u32,
    pub max_health_failures: u32,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct StateMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub binding_name: String,
    pub key: String,
    pub operation: StateMutationOperation,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub current_version: String,
    pub binding_total_bytes: u64,
    pub binding_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "operation", content = "value")]
pub(crate) enum ServiceMaterialMutationOperation {
    Upsert,
    Delete,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceConfigMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub config_name: String,
    pub operation: ServiceMaterialMutationOperation,
    pub sequence: u64,
    pub current_version: String,
    pub config_generation: u64,
    pub config_entry_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub value_hash: Option<Hash>,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceSecretMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub secret_name: String,
    pub operation: ServiceMaterialMutationOperation,
    pub sequence: u64,
    pub current_version: String,
    pub secret_generation: u64,
    pub secret_entry_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<SecretEnvelopeEncryptionV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub key_version: Option<u32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub commitment: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub ciphertext_bytes: Option<u64>,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceConfigStatusQuery {
    pub service_name: String,
    #[norito(default)]
    pub config_name: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceConfigStatusEntry {
    pub config_name: String,
    pub value_hash: Hash,
    pub value_json: norito::json::Value,
    pub last_update_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceConfigStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub config_generation: u64,
    pub config_entry_count: u32,
    pub configs: Vec<ServiceConfigStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ServiceSecretStatusQuery {
    pub service_name: String,
    #[norito(default)]
    pub secret_name: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceSecretStatusEntry {
    pub secret_name: String,
    pub encryption: SecretEnvelopeEncryptionV1,
    pub key_id: String,
    pub key_version: u32,
    pub commitment: Hash,
    pub ciphertext_bytes: u64,
    pub last_update_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceSecretStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub secret_generation: u64,
    pub secret_entry_count: u32,
    pub secrets: Vec<ServiceSecretStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct FheJobRunResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub binding_name: String,
    pub job_id: String,
    pub operation: iroha_data_model::soracloud::FheJobOperationV1,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub output_state_key: String,
    pub output_payload_bytes: u64,
    pub output_commitment: Hash,
    pub current_version: String,
    pub binding_total_bytes: u64,
    pub binding_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct DecryptionRequestResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub policy_name: Name,
    pub request_id: String,
    pub binding_name: Name,
    pub state_key: String,
    pub jurisdiction_tag: String,
    pub policy_snapshot_hash: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consent_evidence_hash: Option<Hash>,
    pub break_glass: bool,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub break_glass_reason: Option<String>,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub current_version: String,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct CiphertextQueryResponse {
    pub action: SoracloudAction,
    pub response: CiphertextQueryResponseV1,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct TrainingJobMutationResponse {
    pub action: TrainingJobAction,
    pub service_name: String,
    pub model_name: String,
    pub job_id: String,
    pub sequence: u64,
    pub status: TrainingJobStatus,
    pub worker_group_size: u16,
    pub target_steps: u32,
    pub completed_steps: u32,
    pub checkpoint_interval_steps: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_step: Option<u32>,
    pub checkpoint_count: u32,
    pub retry_count: u8,
    pub max_retries: u8,
    pub step_compute_units: u64,
    pub compute_budget_units: u64,
    pub compute_consumed_units: u64,
    pub compute_remaining_units: u64,
    pub storage_budget_bytes: u64,
    pub storage_consumed_bytes: u64,
    pub storage_remaining_bytes: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub latest_metrics_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    pub training_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct TrainingJobStatusResponse {
    pub schema_version: u16,
    pub job: TrainingJobStatusEntry,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct TrainingJobStatusEntry {
    pub service_name: String,
    pub model_name: String,
    pub job_id: String,
    pub status: TrainingJobStatus,
    pub worker_group_size: u16,
    pub target_steps: u32,
    pub completed_steps: u32,
    pub checkpoint_interval_steps: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_step: Option<u32>,
    pub checkpoint_count: u32,
    pub retry_count: u8,
    pub max_retries: u8,
    pub step_compute_units: u64,
    pub compute_budget_units: u64,
    pub compute_consumed_units: u64,
    pub compute_remaining_units: u64,
    pub storage_budget_bytes: u64,
    pub storage_consumed_bytes: u64,
    pub storage_remaining_bytes: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub latest_metrics_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    pub created_sequence: u64,
    pub updated_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelWeightMutationResponse {
    pub action: ModelWeightAction,
    pub service_name: String,
    pub model_name: String,
    pub target_version: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub parent_version: Option<String>,
    pub sequence: u64,
    pub version_count: u32,
    pub model_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelWeightStatusResponse {
    pub schema_version: u16,
    pub model: ModelWeightStatusEntry,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelWeightStatusEntry {
    pub service_name: String,
    pub model_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    pub version_count: u32,
    #[norito(default)]
    pub versions: Vec<ModelWeightVersionEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelWeightVersionEntry {
    pub weight_version: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub parent_version: Option<String>,
    pub training_job_id: String,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
    pub registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub promoted_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub gate_report_hash: Option<Hash>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelArtifactMutationResponse {
    pub action: ModelArtifactAction,
    pub service_name: String,
    pub model_name: String,
    pub training_job_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "String::is_empty")]
    pub artifact_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub weight_version: Option<String>,
    pub sequence: u64,
    pub model_artifact_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub model_name: String,
    pub artifact_count: u32,
    pub artifact: ModelArtifactStatusEntry,
    #[norito(default)]
    pub artifacts: Vec<ModelArtifactStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusEntry {
    pub service_name: String,
    pub model_name: String,
    pub artifact_id: String,
    pub training_job_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub weight_version: Option<String>,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
    pub registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consumed_by_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub private_bundle_root: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub compile_profile_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub chunk_manifest_root: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub privacy_mode: Option<iroha_data_model::soracloud::SoraModelPrivacyModeV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct UploadedModelStatusResponse {
    pub schema_version: u16,
    pub bundle: SoraUploadedModelBundleV1,
    pub uploaded_chunk_count: u32,
    #[norito(default)]
    pub chunk_ordinals: Vec<u32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub compile_profile: Option<SoraPrivateCompileProfileV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ModelArtifactStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct UploadedModelEncryptionRecipientResponse {
    pub recipient: SoraUploadedModelEncryptionRecipientV1,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct UploadedModelMutationResponse {
    pub action: UploadedModelAction,
    pub status: UploadedModelStatusResponse,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct UploadedModelBindingMutationResponse {
    pub action: UploadedModelBindingAction,
    pub apartment_name: String,
    pub binding: SoraUploadedModelBindingV1,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct PrivateInferenceStatusResponse {
    pub schema_version: u16,
    pub session: SoraPrivateInferenceSessionV1,
    pub checkpoint_count: u32,
    #[norito(default)]
    pub checkpoints: Vec<SoraPrivateInferenceCheckpointV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct PrivateInferenceMutationResponse {
    pub action: PrivateInferenceAction,
    pub status: PrivateInferenceStatusResponse,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HfSharedLeaseStatusResponse {
    pub schema_version: u16,
    pub source: SoraHfSourceRecordV1,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_projection: Option<SoracloudRuntimeHfSourcePlan>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub pool: Option<SoraHfSharedLeasePoolV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub member: Option<SoraHfSharedLeaseMemberV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub placement: Option<SoraHfPlacementRecordV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub latest_audit_event: Option<SoraHfSharedLeaseAuditEventV1>,
    pub audit_event_count: u32,
    pub storage_base_fee_nanos: u128,
    pub compute_reservation_fee_nanos: u128,
    pub eligible_host_count: u32,
    pub warm_host_count: u32,
    pub importer_pending: bool,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HfSharedLeaseMutationResponse {
    pub schema_version: u16,
    pub action: SoraHfSharedLeaseActionV1,
    pub source: SoraHfSourceRecordV1,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_projection: Option<SoracloudRuntimeHfSourcePlan>,
    pub pool: SoraHfSharedLeasePoolV1,
    pub member: SoraHfSharedLeaseMemberV1,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub placement: Option<SoraHfPlacementRecordV1>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub latest_audit_event: Option<SoraHfSharedLeaseAuditEventV1>,
    pub storage_base_fee_nanos: u128,
    pub compute_reservation_fee_nanos: u128,
    pub eligible_host_count: u32,
    pub warm_host_count: u32,
    pub importer_pending: bool,
}

#[derive(Clone, Copy, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
pub(crate) enum ModelHostMutationAction {
    Advertise,
    Heartbeat,
    Withdraw,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelHostStatusResponse {
    pub schema_version: u16,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub validator_account_id: Option<AccountId>,
    pub active_host_count: u32,
    #[norito(default)]
    pub hosts: Vec<SoraModelHostCapabilityRecordV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelHostMutationResponse {
    pub action: ModelHostMutationAction,
    pub status: ModelHostStatusResponse,
    pub signed_by: String,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct HealthComplianceReportQuery {
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub jurisdiction_tag: Option<String>,
    #[norito(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct TrainingJobStatusQuery {
    pub service_name: String,
    pub job_id: String,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct ModelWeightStatusQuery {
    pub service_name: String,
    pub model_name: String,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusQuery {
    pub service_name: String,
    #[norito(default)]
    pub model_name: Option<String>,
    #[norito(default)]
    pub artifact_id: Option<String>,
    #[norito(default)]
    pub training_job_id: Option<String>,
    #[norito(default)]
    pub weight_version: Option<String>,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct UploadedModelStatusQuery {
    pub service_name: String,
    pub weight_version: String,
    #[norito(default)]
    pub model_id: Option<String>,
    #[norito(default)]
    pub model_name: Option<String>,
    #[norito(default)]
    pub bundle_root: Option<Hash>,
    #[norito(default)]
    pub compile_profile_hash: Option<Hash>,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct PrivateInferenceStatusQuery {
    pub session_id: String,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct HfSharedLeaseStatusQuery {
    pub repo_id: String,
    #[norito(default)]
    pub revision: Option<String>,
    pub storage_class: String,
    pub lease_term_ms: u64,
    #[norito(default)]
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct ModelHostStatusQuery {
    #[norito(default)]
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct AgentAutonomyStatusQuery {
    pub apartment_name: String,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct AgentStatusQuery {
    #[norito(default)]
    pub apartment_name: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct AgentMailboxStatusQuery {
    pub apartment_name: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct MutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    pub current_version: String,
    pub sequence: u64,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub revision_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_handle: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_stage: Option<RolloutStage>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_percent: Option<u8>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ControlPlaneSnapshot {
    pub schema_version: u16,
    pub service_count: u32,
    pub audit_event_count: u32,
    #[norito(default)]
    pub services: Vec<ControlPlaneServiceSnapshot>,
    #[norito(default)]
    pub recent_audit_events: Vec<ControlPlaneAuditEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalReadRouteMatch {
    pub service_name: String,
    pub service_version: String,
    pub handler_name: String,
    pub handler_class: SoracloudLocalReadKind,
    pub handler_path: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HealthComplianceReportResponse {
    pub schema_version: u16,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub jurisdiction_tag: Option<String>,
    pub generated_at_sequence: u64,
    pub total_access_events: u32,
    pub break_glass_events: u32,
    pub non_break_glass_events: u32,
    pub consent_evidence_present_events: u32,
    pub consent_evidence_coverage_bps: u16,
    #[norito(default)]
    pub recent_access_events: Vec<HealthAccessAuditEntry>,
    #[norito(default)]
    pub jurisdiction_stats: Vec<HealthJurisdictionStat>,
    #[norito(default)]
    pub data_flow_attestations: Vec<HealthDataFlowAttestation>,
    #[norito(default)]
    pub policy_diff_history: Vec<HealthPolicyDiffEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HealthAccessAuditEntry {
    pub sequence: u64,
    pub service_name: String,
    pub binding_name: String,
    pub state_key: String,
    pub policy_name: String,
    pub jurisdiction_tag: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consent_evidence_hash: Option<Hash>,
    pub break_glass: bool,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub break_glass_reason: Option<String>,
    pub governance_tx_hash: Hash,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HealthJurisdictionStat {
    pub jurisdiction_tag: String,
    pub access_event_count: u32,
    pub break_glass_event_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HealthDataFlowAttestation {
    pub service_name: String,
    pub current_version: String,
    pub binding_name: String,
    pub key_prefix: String,
    pub encryption: SoraStateEncryptionV1,
    pub mutability: SoraStateMutabilityV1,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct HealthPolicyDiffEntry {
    pub policy_name: String,
    pub jurisdiction_tag: String,
    pub policy_snapshot_hash: Hash,
    pub first_seen_sequence: u64,
    pub last_seen_sequence: u64,
    pub event_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ControlPlaneServiceSnapshot {
    pub service_name: String,
    pub current_version: String,
    pub revision_count: u32,
    #[norito(default)]
    pub config_generation: u64,
    #[norito(default)]
    pub secret_generation: u64,
    #[norito(default)]
    pub config_entry_count: u32,
    #[norito(default)]
    pub secret_entry_count: u32,
    #[norito(default)]
    pub latest_revision: Option<ControlPlaneServiceRevision>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug)]
struct ScrHostAdmission {
    runtime: SoraContainerRuntimeV1,
    allow_wallet_signing: bool,
    allow_state_writes: bool,
    allow_model_inference: bool,
    allow_model_training: bool,
    network: SoraNetworkPolicyV1,
    cpu_millis: u32,
    memory_bytes: u64,
    ephemeral_storage_bytes: u64,
    max_open_files: u32,
    max_tasks: u16,
    start_grace_secs: u32,
    stop_grace_secs: u32,
    healthcheck_path: Option<String>,
    sandbox_profile_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ControlPlaneServiceRevision {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub replicas: u16,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub route_host: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub route_path_prefix: Option<String>,
    pub state_binding_count: u32,
    #[norito(default)]
    pub state_bindings: Vec<SoraStateBindingV1>,
    #[norito(default)]
    pub allow_model_inference: bool,
    #[norito(default)]
    pub allow_model_training: bool,
    /// Runtime type admitted by SCR for this revision.
    pub runtime: SoraContainerRuntimeV1,
    /// Whether wallet-signing syscalls are exposed to the service.
    pub allow_wallet_signing: bool,
    /// Whether non-readonly state bindings are permitted.
    pub allow_state_writes: bool,
    /// Egress network policy admitted for the revision.
    pub network: SoraNetworkPolicyV1,
    /// Admitted CPU budget in millicores.
    pub cpu_millis: u32,
    /// Admitted resident-memory budget in bytes.
    pub memory_bytes: u64,
    /// Admitted ephemeral-storage budget in bytes.
    pub ephemeral_storage_bytes: u64,
    /// Admitted maximum open file descriptors.
    pub max_open_files: u32,
    /// Admitted maximum cooperative tasks/threads.
    pub max_tasks: u16,
    /// SCR startup grace period in seconds.
    pub start_grace_secs: u32,
    /// SCR shutdown grace period in seconds.
    pub stop_grace_secs: u32,
    /// Optional healthcheck path enforced by SCR.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub healthcheck_path: Option<String>,
    /// Required service-scoped configs declared by the container manifest.
    #[norito(default)]
    pub required_config_names: Vec<String>,
    /// Required service-scoped secrets declared by the container manifest.
    #[norito(default)]
    pub required_secret_names: Vec<String>,
    /// Explicit config exports declared by the container manifest.
    #[norito(default)]
    pub config_exports: Vec<SoraConfigExportV1>,
    /// Deterministic hash of sandbox/capability/resource admission inputs.
    pub sandbox_profile_hash: Hash,
    /// Monotonic simulated SCR process generation.
    pub process_generation: u64,
    /// Sequence that started the current process generation.
    pub process_started_sequence: u64,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct ControlPlaneAuditEvent {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
    pub to_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub binding_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub config_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub secret_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub governance_tx_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_handle: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub policy_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub policy_snapshot_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub jurisdiction_tag: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consent_evidence_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub break_glass: Option<bool>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub break_glass_reason: Option<String>,
    pub signed_by: String,
}

#[derive(
    Clone, Debug, Default, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize,
)]
struct BindingRuntimeState {
    total_bytes: u64,
    #[norito(default)]
    key_sizes: BTreeMap<String, u64>,
    #[norito(default)]
    ciphertext_records: BTreeMap<String, CiphertextRuntimeRecord>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct CiphertextRuntimeRecord {
    encryption: SoraStateEncryptionV1,
    payload_bytes: u64,
    commitment: Hash,
    last_update_sequence: u64,
    governance_tx_hash: Hash,
    source_action: SoracloudAction,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct TrainingJobRuntimeState {
    model_name: String,
    job_id: String,
    status: TrainingJobStatus,
    worker_group_size: u16,
    target_steps: u32,
    completed_steps: u32,
    checkpoint_interval_steps: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_checkpoint_step: Option<u32>,
    checkpoint_count: u32,
    retry_count: u8,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    compute_consumed_units: u64,
    storage_budget_bytes: u64,
    storage_consumed_bytes: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    latest_metrics_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_failure_reason: Option<String>,
    created_sequence: u64,
    updated_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct TrainingJobAuditEvent {
    sequence: u64,
    action: TrainingJobAction,
    service_name: String,
    model_name: String,
    job_id: String,
    status: TrainingJobStatus,
    completed_steps: u32,
    checkpoint_count: u32,
    retry_count: u8,
    compute_consumed_units: u64,
    storage_consumed_bytes: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_checkpoint_step: Option<u32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    latest_metrics_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_failure_reason: Option<String>,
    signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct ModelWeightVersionState {
    weight_version: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    parent_version: Option<String>,
    training_job_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    promoted_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    gate_report_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    promoted_by: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[cfg(test)]
struct ModelWeightRegistryState {
    model_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    current_version: Option<String>,
    #[norito(default)]
    versions: BTreeMap<String, ModelWeightVersionState>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct ModelWeightAuditEvent {
    sequence: u64,
    action: ModelWeightAction,
    service_name: String,
    model_name: String,
    target_version: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    current_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    parent_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    gate_approved: Option<bool>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    rollback_reason: Option<String>,
    signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct ModelArtifactState {
    model_name: String,
    training_job_id: String,
    #[norito(default)]
    artifact_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    weight_version: Option<String>,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    consumed_by_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    private_bundle_root: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    compile_profile_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    chunk_manifest_root: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    privacy_mode: Option<iroha_data_model::soracloud::SoraModelPrivacyModeV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct ModelArtifactAuditEvent {
    sequence: u64,
    action: ModelArtifactAction,
    service_name: String,
    model_name: String,
    training_job_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    consumed_by_version: Option<String>,
    signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentWalletSpendRequest {
    request_id: String,
    asset_definition: String,
    amount_nanos: u64,
    created_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentWalletDailySpendEntry {
    asset_definition: String,
    day_bucket: u64,
    spent_nanos: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentMailboxMessage {
    message_id: String,
    from_apartment: String,
    channel: String,
    payload: String,
    payload_hash: Hash,
    enqueued_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentArtifactAllowRule {
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    added_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentApartmentRuntimeState {
    manifest: AgentApartmentManifestV1,
    manifest_hash: Hash,
    status: AgentRuntimeStatus,
    deployed_sequence: u64,
    lease_started_sequence: u64,
    lease_expires_sequence: u64,
    last_renewed_sequence: u64,
    restart_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_reason: Option<String>,
    #[norito(default)]
    process_generation: u64,
    #[norito(default)]
    process_started_sequence: u64,
    #[norito(default)]
    last_active_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_checkpoint_sequence: Option<u64>,
    #[norito(default)]
    checkpoint_count: u32,
    #[norito(default)]
    persistent_state: BindingRuntimeState,
    #[norito(default)]
    revoked_policy_capabilities: BTreeSet<String>,
    #[norito(default)]
    pending_wallet_requests: BTreeMap<String, AgentWalletSpendRequest>,
    #[norito(default)]
    wallet_daily_spend: BTreeMap<String, AgentWalletDailySpendEntry>,
    #[norito(default)]
    mailbox_queue: Vec<AgentMailboxMessage>,
    autonomy_budget_ceiling_units: u64,
    autonomy_budget_remaining_units: u64,
    #[norito(default)]
    artifact_allowlist: BTreeMap<String, AgentArtifactAllowRule>,
    #[norito(default)]
    autonomy_run_history: Vec<AgentAutonomyRunRecord>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct AgentApartmentAuditEvent {
    sequence: u64,
    action: AgentApartmentAction,
    apartment_name: String,
    status: AgentRuntimeStatus,
    lease_expires_sequence: u64,
    manifest_hash: Hash,
    restart_count: u32,
    signed_by: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    asset_definition: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    amount_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    capability: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    from_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    to_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    payload_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    artifact_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    run_label: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    budget_units: Option<u64>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RolloutRuntimeState {
    pub rollout_handle: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub baseline_version: Option<String>,
    pub candidate_version: String,
    pub canary_percent: u8,
    pub traffic_percent: u8,
    pub stage: RolloutStage,
    pub health_failures: u32,
    pub max_health_failures: u32,
    pub health_window_secs: u32,
    pub created_sequence: u64,
    pub updated_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_sequence: u64,
    pub lease_remaining_ticks: u64,
    pub manifest_hash: Hash,
    pub restart_count: u32,
    pub pending_wallet_request_count: u32,
    pub revoked_policy_capability_count: u32,
    pub budget_remaining_units: u64,
    pub allowlist_count: u32,
    pub run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_restart_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_restart_reason: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentStatusResponse {
    pub schema_version: u16,
    pub apartment_count: u32,
    pub event_count: u32,
    #[norito(default)]
    pub apartments: Vec<AgentApartmentStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentApartmentStatusEntry {
    pub apartment_name: String,
    pub manifest_hash: Hash,
    pub status: AgentRuntimeStatus,
    pub lease_started_sequence: u64,
    pub lease_expires_sequence: u64,
    pub lease_remaining_ticks: u64,
    pub restart_count: u32,
    pub state_quota_bytes: u64,
    pub tool_capability_count: u32,
    pub policy_capability_count: u32,
    pub revoked_policy_capability_count: u32,
    pub pending_wallet_request_count: u32,
    pub pending_mailbox_message_count: u32,
    pub autonomy_budget_ceiling_units: u64,
    pub autonomy_budget_remaining_units: u64,
    pub artifact_allowlist_count: u32,
    pub autonomy_run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub spend_limit_count: u32,
    pub upgrade_policy: iroha_data_model::soracloud::AgentUpgradePolicyV1,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_restart_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_restart_reason: Option<String>,
}

impl AgentApartmentStatusEntry {
    fn from_state(apartment_name: &str, state: &AgentApartmentRuntimeState, sequence: u64) -> Self {
        Self {
            apartment_name: apartment_name.to_owned(),
            manifest_hash: state.manifest_hash,
            status: agent_runtime_status_for_sequence(state, sequence),
            lease_started_sequence: state.lease_started_sequence,
            lease_expires_sequence: state.lease_expires_sequence,
            lease_remaining_ticks: agent_lease_remaining_ticks(state, sequence),
            restart_count: state.restart_count,
            state_quota_bytes: state.manifest.state_quota_bytes.get(),
            tool_capability_count: u32::try_from(state.manifest.tool_capabilities.len())
                .unwrap_or(u32::MAX),
            policy_capability_count: u32::try_from(state.manifest.policy_capabilities.len())
                .unwrap_or(u32::MAX),
            revoked_policy_capability_count: agent_revoked_capability_count(state),
            pending_wallet_request_count: agent_pending_wallet_request_count(state),
            pending_mailbox_message_count: agent_pending_mailbox_message_count(state),
            autonomy_budget_ceiling_units: state.autonomy_budget_ceiling_units,
            autonomy_budget_remaining_units: state.autonomy_budget_remaining_units,
            artifact_allowlist_count: agent_allowlist_count(state),
            autonomy_run_count: agent_run_count(state),
            process_generation: state.process_generation,
            process_started_sequence: state.process_started_sequence,
            last_active_sequence: state.last_active_sequence,
            last_checkpoint_sequence: state.last_checkpoint_sequence,
            checkpoint_count: state.checkpoint_count,
            persistent_state_total_bytes: state.persistent_state.total_bytes,
            persistent_state_key_count: agent_persistent_state_key_count(state),
            spend_limit_count: u32::try_from(state.manifest.spend_limits.len()).unwrap_or(u32::MAX),
            upgrade_policy: state.manifest.upgrade_policy.clone(),
            last_restart_sequence: state.last_restart_sequence,
            last_restart_reason: state.last_restart_reason.clone(),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentWalletMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub manifest_hash: Hash,
    pub status: AgentRuntimeStatus,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub asset_definition: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub amount_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub day_bucket: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub day_spent_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub pending_request_count: u32,
    pub revoked_policy_capability_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentMailboxMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub message_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub from_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub to_apartment: Option<String>,
    pub channel: String,
    pub payload_hash: Hash,
    pub status: AgentRuntimeStatus,
    pub pending_message_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentMailboxStatusResponse {
    pub schema_version: u16,
    pub apartment_name: String,
    pub status: AgentRuntimeStatus,
    pub pending_message_count: u32,
    pub event_count: u32,
    #[norito(default)]
    pub messages: Vec<AgentMailboxMessageEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentMailboxMessageEntry {
    pub message_id: String,
    pub from_apartment: String,
    pub channel: String,
    pub payload: String,
    pub payload_hash: Hash,
    pub enqueued_sequence: u64,
}

impl AgentMailboxMessageEntry {
    fn from_message(message: &AgentMailboxMessage) -> Self {
        Self {
            message_id: message.message_id.clone(),
            from_apartment: message.from_apartment.clone(),
            channel: message.channel.clone(),
            payload: message.payload.clone(),
            payload_hash: message.payload_hash,
            enqueued_sequence: message.enqueued_sequence,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentRuntimeReceiptRecord {
    pub receipt_id: Hash,
    pub service_name: String,
    pub service_version: String,
    pub handler_name: String,
    pub handler_class: SoraServiceHandlerClassV1,
    pub request_commitment: Hash,
    pub result_commitment: Hash,
    pub certified_by: SoraCertifiedResponsePolicyV1,
    pub emitted_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub placement_id: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub selected_validator_account_id: Option<AccountId>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub selected_peer_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub journal_artifact_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact_hash: Option<Hash>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentRuntimeWorkflowStepSummary {
    pub step_index: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    pub request_commitment: Hash,
    pub result_commitment: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_json: Option<norito::json::Value>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentAutonomyExecutionAuditRecord {
    pub sequence: u64,
    pub succeeded: bool,
    pub result_commitment: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub handler_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_receipt_id: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub journal_artifact_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentRuntimeExecutionSummary {
    pub apartment_name: String,
    pub run_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub handler_name: Option<String>,
    pub succeeded: bool,
    pub result_commitment: Hash,
    pub journal_artifact_hash: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(default)]
    pub workflow_steps: Vec<AgentRuntimeWorkflowStepSummary>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_json: Option<norito::json::Value>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentAutonomyMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_sequence: u64,
    pub lease_remaining_ticks: u64,
    pub manifest_hash: Hash,
    pub artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub provenance_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub run_label: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub workflow_input_json: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub budget_units: Option<u64>,
    pub budget_remaining_units: u64,
    pub allowlist_count: u32,
    pub run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_execution: Option<AgentRuntimeExecutionSummary>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_execution_error: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_runtime_receipt_error: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_execution_audit: Option<AgentAutonomyExecutionAuditRecord>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_execution_audit_error: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentAutonomyStatusResponse {
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_sequence: u64,
    pub lease_remaining_ticks: u64,
    pub manifest_hash: Hash,
    pub revoked_policy_capability_count: u32,
    pub budget_ceiling_units: u64,
    pub budget_remaining_units: u64,
    pub allowlist_count: u32,
    pub run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    #[norito(default)]
    pub allowlist: Vec<AgentAutonomyAllowlistEntry>,
    #[norito(default)]
    pub recent_runs: Vec<AgentAutonomyRunRecord>,
    #[norito(default)]
    pub runtime_recent_runs: Vec<AgentRuntimeExecutionSummary>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct AgentAutonomyAllowlistEntry {
    pub artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub provenance_hash: Option<String>,
    pub added_sequence: u64,
}

impl AgentAutonomyAllowlistEntry {
    fn from_rule(rule: &AgentArtifactAllowRule) -> Self {
        Self {
            artifact_hash: rule.artifact_hash.clone(),
            provenance_hash: rule.provenance_hash.clone(),
            added_sequence: rule.added_sequence,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentAutonomyRunRecord {
    pub run_id: String,
    pub artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub provenance_hash: Option<String>,
    pub budget_units: u64,
    pub run_label: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub workflow_input_json: Option<String>,
    pub approved_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub authoritative_execution_audit: Option<AgentAutonomyExecutionAuditRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoracloudErrorKind {
    BadRequest,
    Unauthorized,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug, JsonSerialize)]
struct SoracloudErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
pub(crate) struct SoracloudError {
    kind: SoracloudErrorKind,
    message: String,
}

impl SoracloudError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::BadRequest,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Unauthorized,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::NotFound,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Conflict,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Internal,
            message: message.into(),
        }
    }

    fn code(&self) -> &'static str {
        match self.kind {
            SoracloudErrorKind::BadRequest => "bad_request",
            SoracloudErrorKind::Unauthorized => "invalid_signature",
            SoracloudErrorKind::NotFound => "not_found",
            SoracloudErrorKind::Conflict => "conflict",
            SoracloudErrorKind::Internal => "internal",
        }
    }

    fn status(&self) -> StatusCode {
        match self.kind {
            SoracloudErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            SoracloudErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            SoracloudErrorKind::NotFound => StatusCode::NOT_FOUND,
            SoracloudErrorKind::Conflict => StatusCode::CONFLICT,
            SoracloudErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for SoracloudError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = SoracloudErrorBody {
            code: self.code(),
            message: self.message,
        };
        (status, JsonBody(body)).into_response()
    }
}

fn admit_scr_host_bundle(
    bundle: &SoraDeploymentBundleV1,
) -> Result<ScrHostAdmission, SoracloudError> {
    let container = &bundle.container;
    let resources = container.resources;
    let lifecycle = &container.lifecycle;

    if resources.cpu_millis.get() > SCR_HOST_MAX_CPU_MILLIS {
        return Err(SoracloudError::bad_request(format!(
            "container.resources.cpu_millis exceeds SCR cap ({SCR_HOST_MAX_CPU_MILLIS})"
        )));
    }
    if resources.memory_bytes.get() > SCR_HOST_MAX_MEMORY_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "container.resources.memory_bytes exceeds SCR cap ({SCR_HOST_MAX_MEMORY_BYTES})"
        )));
    }
    if resources.ephemeral_storage_bytes.get() > SCR_HOST_MAX_EPHEMERAL_STORAGE_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "container.resources.ephemeral_storage_bytes exceeds SCR cap ({SCR_HOST_MAX_EPHEMERAL_STORAGE_BYTES})"
        )));
    }
    if resources.max_open_files.get() > SCR_HOST_MAX_OPEN_FILES {
        return Err(SoracloudError::bad_request(format!(
            "container.resources.max_open_files exceeds SCR cap ({SCR_HOST_MAX_OPEN_FILES})"
        )));
    }
    if resources.max_tasks.get() > SCR_HOST_MAX_TASKS {
        return Err(SoracloudError::bad_request(format!(
            "container.resources.max_tasks exceeds SCR cap ({SCR_HOST_MAX_TASKS})"
        )));
    }
    if lifecycle.start_grace_secs.get() > SCR_HOST_MAX_START_GRACE_SECS {
        return Err(SoracloudError::bad_request(format!(
            "container.lifecycle.start_grace_secs exceeds SCR cap ({SCR_HOST_MAX_START_GRACE_SECS})"
        )));
    }
    if lifecycle.stop_grace_secs.get() > SCR_HOST_MAX_STOP_GRACE_SECS {
        return Err(SoracloudError::bad_request(format!(
            "container.lifecycle.stop_grace_secs exceeds SCR cap ({SCR_HOST_MAX_STOP_GRACE_SECS})"
        )));
    }

    if !container.capabilities.allow_state_writes
        && bundle
            .service
            .state_bindings
            .iter()
            .any(|binding| binding.mutability != SoraStateMutabilityV1::ReadOnly)
    {
        return Err(SoracloudError::bad_request(
            "container capability `allow_state_writes=false` conflicts with non-readonly state bindings",
        ));
    }

    if let SoraNetworkPolicyV1::Allowlist(hosts) = &container.capabilities.network {
        if hosts.is_empty() {
            return Err(SoracloudError::bad_request(
                "container capability network allowlist must not be empty",
            ));
        }
        let mut seen = BTreeSet::new();
        for host in hosts {
            let normalized = host.trim();
            if normalized.is_empty() {
                return Err(SoracloudError::bad_request(
                    "container capability network allowlist contains an empty host",
                ));
            }
            if normalized.chars().any(char::is_control)
                || normalized.chars().any(char::is_whitespace)
            {
                return Err(SoracloudError::bad_request(
                    "container capability network allowlist contains invalid host characters",
                ));
            }
            if !seen.insert(normalized.to_owned()) {
                return Err(SoracloudError::bad_request(
                    "container capability network allowlist must not contain duplicates",
                ));
            }
        }
    }

    let network_policy = container.capabilities.network.to_owned();
    let healthcheck_path = lifecycle.healthcheck_path.as_deref().map(str::to_owned);
    let sandbox_profile = (
        container.runtime,
        network_policy.clone(),
        container.capabilities.allow_wallet_signing,
        container.capabilities.allow_state_writes,
        container.capabilities.allow_model_inference,
        container.capabilities.allow_model_training,
        (
            resources.cpu_millis.get(),
            resources.memory_bytes.get(),
            resources.ephemeral_storage_bytes.get(),
            resources.max_open_files.get(),
            resources.max_tasks.get(),
        ),
        (
            lifecycle.start_grace_secs.get(),
            lifecycle.stop_grace_secs.get(),
            healthcheck_path.clone(),
        ),
    );
    let sandbox_profile_hash =
        Hash::new(&norito::to_bytes(&sandbox_profile).map_err(|err| {
            SoracloudError::internal(format!("failed to encode SCR profile: {err}"))
        })?);

    Ok(ScrHostAdmission {
        runtime: container.runtime,
        allow_wallet_signing: container.capabilities.allow_wallet_signing,
        allow_state_writes: container.capabilities.allow_state_writes,
        allow_model_inference: container.capabilities.allow_model_inference,
        allow_model_training: container.capabilities.allow_model_training,
        network: network_policy,
        cpu_millis: resources.cpu_millis.get(),
        memory_bytes: resources.memory_bytes.get(),
        ephemeral_storage_bytes: resources.ephemeral_storage_bytes.get(),
        max_open_files: resources.max_open_files.get(),
        max_tasks: resources.max_tasks.get(),
        start_grace_secs: lifecycle.start_grace_secs.get(),
        stop_grace_secs: lifecycle.stop_grace_secs.get(),
        healthcheck_path,
        sandbox_profile_hash,
    })
}

fn normalize_agent_runtime_defaults(runtime: &mut AgentApartmentRuntimeState) {
    if runtime.process_generation == 0 {
        runtime.process_generation = 1;
    }
    if runtime.process_started_sequence == 0 {
        runtime.process_started_sequence = runtime.deployed_sequence;
    }
    if runtime.last_active_sequence == 0 {
        runtime.last_active_sequence = runtime
            .last_renewed_sequence
            .max(runtime.process_started_sequence);
    }
}

fn parse_agent_apartment_name(apartment_name: &str) -> Result<String, SoracloudError> {
    let normalized = apartment_name.trim();
    let parsed: Name = normalized
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid apartment_name: {err}")))?;
    Ok(parsed.to_string())
}

fn parse_service_name(service_name: &str) -> Result<Name, SoracloudError> {
    service_name
        .trim()
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))
}

fn parse_optional_service_name(service_name: Option<&str>) -> Result<Option<Name>, SoracloudError> {
    service_name.map(parse_service_name).transpose()
}

fn parse_agent_capability_name(capability: &str) -> Result<String, SoracloudError> {
    let normalized = capability.trim();
    let parsed: Name = normalized
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid capability: {err}")))?;
    Ok(parsed.to_string())
}

fn parse_training_model_name(model_name: &str) -> Result<String, SoracloudError> {
    let normalized = model_name.trim();
    let parsed: Name = normalized
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid model_name: {err}")))?;
    Ok(parsed.to_string())
}

fn normalize_hf_token(
    field_name: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<String, SoracloudError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request(format!(
            "{field_name} must not be empty"
        )));
    }
    if normalized.len() > max_bytes {
        return Err(SoracloudError::bad_request(format!(
            "{field_name} exceeds max bytes ({max_bytes})"
        )));
    }
    if normalized.chars().any(char::is_control) || normalized.chars().any(char::is_whitespace) {
        return Err(SoracloudError::bad_request(format!(
            "{field_name} must not contain control characters or whitespace"
        )));
    }
    Ok(normalized.to_owned())
}

fn parse_hf_repo_id(repo_id: &str) -> Result<String, SoracloudError> {
    normalize_hf_token("repo_id", repo_id, HF_REPO_ID_MAX_BYTES)
}

fn parse_hf_revision(resolved_revision: &str) -> Result<String, SoracloudError> {
    normalize_hf_token(
        "resolved_revision",
        resolved_revision,
        HF_REVISION_MAX_BYTES,
    )
}

fn parse_hf_resolved_revision(resolved_revision: Option<&str>) -> Result<String, SoracloudError> {
    resolved_revision
        .map(parse_hf_revision)
        .transpose()
        .map(|resolved| resolved.unwrap_or_else(|| HF_DEFAULT_RESOLVED_REVISION.to_owned()))
}

fn parse_hf_model_name(model_name: &str) -> Result<String, SoracloudError> {
    normalize_hf_token("model_name", model_name, HF_MODEL_NAME_MAX_BYTES)
}

fn parse_optional_account_id(
    account_id: Option<&str>,
) -> Result<Option<AccountId>, SoracloudError> {
    account_id
        .map(|literal| {
            AccountId::parse_encoded(literal.trim())
                .map(|parsed| parsed.into_account_id())
                .map_err(|err| SoracloudError::bad_request(format!("invalid account_id: {err}")))
        })
        .transpose()
}

fn parse_storage_class_query(storage_class: &str) -> Result<StorageClass, SoracloudError> {
    match storage_class.trim().to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        _ => Err(SoracloudError::bad_request(
            "invalid storage_class: expected one of hot, warm, or cold",
        )),
    }
}

fn hf_source_id(repo_id: &str, resolved_revision: &str) -> Result<Hash, SoracloudError> {
    let payload = norito::to_bytes(&(repo_id, resolved_revision))
        .map_err(|err| SoracloudError::internal(format!("failed to encode hf source id: {err}")))?;
    Ok(Hash::new(payload))
}

fn hf_shared_lease_pool_id(
    source_id: Hash,
    storage_class: StorageClass,
    lease_term_ms: u64,
) -> Result<Hash, SoracloudError> {
    let payload = norito::to_bytes(&(source_id, storage_class, lease_term_ms)).map_err(|err| {
        SoracloudError::internal(format!("failed to encode hf shared lease pool id: {err}"))
    })?;
    Ok(Hash::new(payload))
}

fn hf_profile_http_client() -> Result<reqwest::Client, SoracloudError> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(
            iroha_config::parameters::defaults::soracloud_runtime::hf::REQUEST_TIMEOUT_MS,
        ))
        .build()
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to build Hugging Face profile-derivation client: {err}"
            ))
        })
}

fn normalize_hf_base_url(base_url: &str) -> Result<reqwest::Url, SoracloudError> {
    let trimmed = base_url.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme).map_err(|err| {
        SoracloudError::bad_request(format!("invalid Hugging Face base URL: {err}"))
    })?;
    let normalized_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => path.to_owned(),
    };
    url.set_path(&normalized_path);
    Ok(url)
}

fn hf_model_info_url(
    repo_id: &str,
    resolved_revision: &str,
) -> Result<reqwest::Url, SoracloudError> {
    let mut url = normalize_hf_base_url(
        iroha_config::parameters::defaults::soracloud_runtime::hf::API_BASE_URL,
    )?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| SoracloudError::bad_request("invalid Hugging Face API base URL"))?;
        for component in ["models"]
            .into_iter()
            .chain(repo_id.split('/'))
            .chain(["revision", resolved_revision].into_iter())
        {
            segments.push(component);
        }
    }
    Ok(url)
}

fn hf_repo_file_url(
    repo_id: &str,
    resolved_revision: &str,
    file_path: &str,
) -> Result<reqwest::Url, SoracloudError> {
    let mut url = normalize_hf_base_url(
        iroha_config::parameters::defaults::soracloud_runtime::hf::HUB_BASE_URL,
    )?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| SoracloudError::bad_request("invalid Hugging Face Hub base URL"))?;
        for component in repo_id
            .split('/')
            .chain(["resolve", resolved_revision].into_iter())
            .chain(file_path.split('/'))
        {
            segments.push(component);
        }
    }
    Ok(url)
}

async fn hf_content_length_bytes(
    client: &reqwest::Client,
    repo_id: &str,
    resolved_revision: &str,
    file_path: &str,
) -> Result<u64, SoracloudError> {
    let file_url = hf_repo_file_url(repo_id, resolved_revision, file_path)?;
    let response = client.head(file_url.clone()).send().await.map_err(|err| {
        SoracloudError::internal(format!(
            "failed to query Hugging Face file headers from {file_url}: {err}"
        ))
    })?;
    if !response.status().is_success() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` returned {}",
            response.status()
        )));
    }
    response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` is missing Content-Length"
            ))
        })
}

async fn derive_hf_resource_profile(
    repo_id: &str,
    resolved_revision: &str,
) -> Result<SoraHfResourceProfileV1, SoracloudError> {
    let client = hf_profile_http_client()?;
    let info_url = hf_model_info_url(repo_id, resolved_revision)?;
    let response = client.get(info_url.clone()).send().await.map_err(|err| {
        SoracloudError::internal(format!(
            "failed to fetch Hugging Face model info from {info_url}: {err}"
        ))
    })?;
    if !response.status().is_success() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info request for `{repo_id}@{resolved_revision}` returned {}",
            response.status()
        )));
    }
    let body = response.bytes().await.map_err(|err| {
        SoracloudError::internal(format!(
            "failed to read Hugging Face model info response from {info_url}: {err}"
        ))
    })?;
    let model_info: norito::json::Value = norito::json::from_slice(&body).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to decode Hugging Face model info JSON for `{repo_id}@{resolved_revision}`: {err}"
        ))
    })?;
    let sibling_paths = model_info
        .get("siblings")
        .and_then(norito::json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("rfilename").and_then(norito::json::Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let select_paths = |extensions: &[&str]| {
        sibling_paths
            .iter()
            .filter(|path| {
                let normalized = path.to_ascii_lowercase();
                extensions
                    .iter()
                    .any(|extension| normalized.ends_with(extension))
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    let (backend_family, model_format, weight_files) = if let files @ [_first, ..] =
        select_paths(&[".gguf"]).as_slice()
    {
        (
            SoraHfBackendFamilyV1::Gguf,
            SoraHfModelFormatV1::Gguf,
            files.to_vec(),
        )
    } else if let files @ [_first, ..] = select_paths(&[".safetensors"]).as_slice() {
        (
            SoraHfBackendFamilyV1::Transformers,
            SoraHfModelFormatV1::Safetensors,
            files.to_vec(),
        )
    } else if let files @ [_first, ..] = select_paths(&[".bin", ".pt", ".pth"]).as_slice() {
        (
            SoraHfBackendFamilyV1::Transformers,
            SoraHfModelFormatV1::PyTorch,
            files.to_vec(),
        )
    } else {
        return Err(SoracloudError::conflict(format!(
            "no supported Hugging Face model weights were found for `{repo_id}@{resolved_revision}`"
        )));
    };

    let mut required_model_bytes = 0_u64;
    for file_path in &weight_files {
        required_model_bytes = required_model_bytes.saturating_add(
            hf_content_length_bytes(&client, repo_id, resolved_revision, file_path).await?,
        );
    }
    if required_model_bytes == 0 {
        return Err(SoracloudError::conflict(format!(
            "derived Hugging Face model size for `{repo_id}@{resolved_revision}` was zero bytes"
        )));
    }

    let disk_cache_bytes_floor = required_model_bytes;
    let ram_bytes_floor = match backend_family {
        SoraHfBackendFamilyV1::Gguf => required_model_bytes
            .saturating_mul(3)
            .saturating_div(2)
            .max(required_model_bytes),
        SoraHfBackendFamilyV1::Transformers => required_model_bytes
            .saturating_mul(2)
            .max(required_model_bytes),
    };

    Ok(SoraHfResourceProfileV1 {
        required_model_bytes,
        backend_family,
        model_format,
        disk_cache_bytes_floor,
        ram_bytes_floor,
        vram_bytes_floor: 0,
    })
}

fn verify_auxiliary_provenance_payload(
    signer: &SoracloudMutationSigner,
    provenance: &ManifestProvenance,
    payload: Vec<u8>,
    signer_error: &'static str,
    signature_error: &'static str,
) -> Result<(), SoracloudError> {
    if provenance.signer != signer.request_signer {
        return Err(SoracloudError::unauthorized(signer_error));
    }
    provenance
        .signature
        .verify(&provenance.signer, &payload)
        .map_err(|_| SoracloudError::bad_request(signature_error))?;
    Ok(())
}

fn required_generated_bundle_provenance(
    bundle: &SoraDeploymentBundleV1,
    signer: &SoracloudMutationSigner,
    provenance: Option<&ManifestProvenance>,
) -> Result<ManifestProvenance, SoracloudError> {
    let provenance = provenance.ok_or_else(|| {
        SoracloudError::bad_request(
            "generated_service_provenance is required when deploying a new HF-generated service",
        )
    })?;
    let payload = encode_bundle_signature_payload(bundle, &BTreeMap::new(), &BTreeMap::new())?;
    verify_auxiliary_provenance_payload(
        signer,
        provenance,
        payload,
        "generated service provenance signer must match the signed request signer",
        "generated service provenance signature verification failed",
    )?;
    Ok(provenance.clone())
}

fn required_generated_agent_deploy_provenance(
    manifest: &AgentApartmentManifestV1,
    signer: &SoracloudMutationSigner,
    provenance: Option<&ManifestProvenance>,
) -> Result<ManifestProvenance, SoracloudError> {
    let provenance = provenance.ok_or_else(|| {
        SoracloudError::bad_request(
            "generated_apartment_provenance is required when deploying a new HF-generated apartment",
        )
    })?;
    let payload = encode_agent_deploy_provenance_payload(
        manifest.clone(),
        HF_GENERATED_AGENT_LEASE_TICKS,
        Some(HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode generated HF agent deploy payload: {err}"
        ))
    })?;
    verify_auxiliary_provenance_payload(
        signer,
        provenance,
        payload,
        "generated apartment provenance signer must match the signed request signer",
        "generated apartment provenance signature verification failed",
    )?;
    Ok(provenance.clone())
}

fn authoritative_active_service_bundle(
    world: &impl WorldReadOnly,
    service_name: &Name,
) -> Result<Option<SoraDeploymentBundleV1>, SoracloudError> {
    let Some(deployment) = world
        .soracloud_service_deployments()
        .get(service_name)
        .cloned()
    else {
        return Ok(None);
    };
    world
        .soracloud_service_revisions()
        .get(&(
            service_name.to_string(),
            deployment.current_service_version.clone(),
        ))
        .cloned()
        .map(Some)
        .ok_or_else(|| {
            SoracloudError::internal(format!(
                "service `{service_name}` points to missing admitted revision `{}`",
                deployment.current_service_version
            ))
        })
}

fn ensure_hf_generated_service_instruction(
    app: &SharedAppState,
    signer: &SoracloudMutationSigner,
    bundle: &SoraDeploymentBundleV1,
    source_id: &Hash,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
    generated_provenance: Option<&ManifestProvenance>,
) -> Result<Option<InstructionBox>, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    if let Some(existing_bundle) =
        authoritative_active_service_bundle(world, &bundle.service.service_name)?
    {
        let Some(binding) = soracloud_hf_generated_source_binding(&existing_bundle) else {
            return Err(SoracloudError::conflict(format!(
                "service `{}` is already deployed and is not an auto-generated HF inference service",
                bundle.service.service_name
            )));
        };
        if binding.source_id == source_id.to_string()
            && binding.repo_id == repo_id
            && binding.resolved_revision == resolved_revision
            && binding.model_name == model_name
        {
            return Ok(None);
        }
        return Err(SoracloudError::conflict(format!(
            "service `{}` is already bound to HF source `{}` and cannot be reused for `{repo_id}@{resolved_revision}`",
            bundle.service.service_name, binding.source_id
        )));
    }

    admit_scr_host_bundle(bundle)?;
    let provenance = required_generated_bundle_provenance(bundle, signer, generated_provenance)?;
    Ok(Some(InstructionBox::from(
        isi::soracloud::DeploySoracloudService {
            bundle: bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance,
        },
    )))
}

fn ensure_hf_generated_agent_instruction(
    app: &SharedAppState,
    signer: &SoracloudMutationSigner,
    manifest: &AgentApartmentManifestV1,
    generated_provenance: Option<&ManifestProvenance>,
) -> Result<Option<InstructionBox>, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    if let Some(record) = world
        .soracloud_agent_apartments()
        .get(manifest.apartment_name.as_ref())
        .cloned()
    {
        if record.manifest == *manifest {
            return Ok(None);
        }
        return Err(SoracloudError::conflict(format!(
            "apartment `{}` is already deployed and is not the generated HF apartment for this service",
            manifest.apartment_name
        )));
    }

    let provenance =
        required_generated_agent_deploy_provenance(manifest, signer, generated_provenance)?;
    Ok(Some(InstructionBox::from(
        isi::soracloud::DeploySoracloudAgentApartment {
            manifest: manifest.clone(),
            lease_ticks: HF_GENERATED_AGENT_LEASE_TICKS,
            autonomy_budget_units: HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS,
            provenance,
        },
    )))
}

fn parse_training_job_id(job_id: &str) -> Result<String, SoracloudError> {
    let normalized = job_id.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request("job_id must not be empty"));
    }
    if normalized.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "job_id exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "job_id must not contain control characters",
        ));
    }
    if normalized.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(SoracloudError::bad_request(
            "job_id must not contain whitespace",
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(SoracloudError::bad_request(
            "job_id must use only ASCII letters, digits, or [- _ . : #]",
        ));
    }
    Ok(normalized.to_owned())
}

fn parse_model_weight_version(weight_version: &str) -> Result<String, SoracloudError> {
    let normalized = weight_version.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request(
            "weight_version must not be empty",
        ));
    }
    if normalized.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "weight_version exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "weight_version must not contain control characters",
        ));
    }
    if normalized.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(SoracloudError::bad_request(
            "weight_version must not contain whitespace",
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(SoracloudError::bad_request(
            "weight_version must use only ASCII letters, digits, or [- _ . : #]",
        ));
    }
    Ok(normalized.to_owned())
}

fn parse_model_weight_dataset_ref(dataset_ref: &str) -> Result<String, SoracloudError> {
    let normalized = dataset_ref.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request("dataset_ref must not be empty"));
    }
    if normalized.len() > MODEL_WEIGHT_MAX_DATASET_REF_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "dataset_ref exceeds max bytes ({MODEL_WEIGHT_MAX_DATASET_REF_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "dataset_ref must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}

fn normalize_model_weight_reason(reason: &str) -> Result<String, SoracloudError> {
    let normalized = reason.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request("reason must not be empty"));
    }
    if normalized.len() > MODEL_WEIGHT_MAX_REASON_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "reason exceeds max bytes ({MODEL_WEIGHT_MAX_REASON_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "reason must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}

fn training_job_mutation_response(
    action: TrainingJobAction,
    service_name: &str,
    runtime_state: &TrainingJobRuntimeState,
    sequence: u64,
    training_event_count: u32,
    signed_by: String,
) -> TrainingJobMutationResponse {
    TrainingJobMutationResponse {
        action,
        service_name: service_name.to_owned(),
        model_name: runtime_state.model_name.clone(),
        job_id: runtime_state.job_id.clone(),
        sequence,
        status: runtime_state.status,
        worker_group_size: runtime_state.worker_group_size,
        target_steps: runtime_state.target_steps,
        completed_steps: runtime_state.completed_steps,
        checkpoint_interval_steps: runtime_state.checkpoint_interval_steps,
        last_checkpoint_step: runtime_state.last_checkpoint_step,
        checkpoint_count: runtime_state.checkpoint_count,
        retry_count: runtime_state.retry_count,
        max_retries: runtime_state.max_retries,
        step_compute_units: runtime_state.step_compute_units,
        compute_budget_units: runtime_state.compute_budget_units,
        compute_consumed_units: runtime_state.compute_consumed_units,
        compute_remaining_units: runtime_state
            .compute_budget_units
            .saturating_sub(runtime_state.compute_consumed_units),
        storage_budget_bytes: runtime_state.storage_budget_bytes,
        storage_consumed_bytes: runtime_state.storage_consumed_bytes,
        storage_remaining_bytes: runtime_state
            .storage_budget_bytes
            .saturating_sub(runtime_state.storage_consumed_bytes),
        latest_metrics_hash: runtime_state.latest_metrics_hash,
        last_failure_reason: runtime_state.last_failure_reason.clone(),
        training_event_count,
        signed_by,
    }
}

fn training_job_status_entry(
    service_name: &str,
    runtime_state: &TrainingJobRuntimeState,
) -> TrainingJobStatusEntry {
    TrainingJobStatusEntry {
        service_name: service_name.to_owned(),
        model_name: runtime_state.model_name.clone(),
        job_id: runtime_state.job_id.clone(),
        status: runtime_state.status,
        worker_group_size: runtime_state.worker_group_size,
        target_steps: runtime_state.target_steps,
        completed_steps: runtime_state.completed_steps,
        checkpoint_interval_steps: runtime_state.checkpoint_interval_steps,
        last_checkpoint_step: runtime_state.last_checkpoint_step,
        checkpoint_count: runtime_state.checkpoint_count,
        retry_count: runtime_state.retry_count,
        max_retries: runtime_state.max_retries,
        step_compute_units: runtime_state.step_compute_units,
        compute_budget_units: runtime_state.compute_budget_units,
        compute_consumed_units: runtime_state.compute_consumed_units,
        compute_remaining_units: runtime_state
            .compute_budget_units
            .saturating_sub(runtime_state.compute_consumed_units),
        storage_budget_bytes: runtime_state.storage_budget_bytes,
        storage_consumed_bytes: runtime_state.storage_consumed_bytes,
        storage_remaining_bytes: runtime_state
            .storage_budget_bytes
            .saturating_sub(runtime_state.storage_consumed_bytes),
        latest_metrics_hash: runtime_state.latest_metrics_hash,
        last_failure_reason: runtime_state.last_failure_reason.clone(),
        created_sequence: runtime_state.created_sequence,
        updated_sequence: runtime_state.updated_sequence,
    }
}

fn model_weight_mutation_response(
    action: ModelWeightAction,
    service_name: &str,
    model_name: &str,
    target_version: &str,
    current_version: Option<String>,
    parent_version: Option<String>,
    sequence: u64,
    version_count: u32,
    model_event_count: u32,
    signed_by: String,
) -> ModelWeightMutationResponse {
    ModelWeightMutationResponse {
        action,
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        target_version: target_version.to_owned(),
        current_version,
        parent_version,
        sequence,
        version_count,
        model_event_count,
        signed_by,
    }
}

#[cfg(test)]
fn model_weight_status_entry(
    service_name: &str,
    model_registry: &ModelWeightRegistryState,
) -> ModelWeightStatusEntry {
    let versions = model_registry
        .versions
        .values()
        .map(model_weight_version_entry)
        .collect::<Vec<_>>();
    ModelWeightStatusEntry {
        service_name: service_name.to_owned(),
        model_name: model_registry.model_name.clone(),
        current_version: model_registry.current_version.clone(),
        version_count: u32::try_from(versions.len()).unwrap_or(u32::MAX),
        versions,
    }
}

fn model_weight_version_entry(version: &ModelWeightVersionState) -> ModelWeightVersionEntry {
    ModelWeightVersionEntry {
        weight_version: version.weight_version.clone(),
        parent_version: version.parent_version.clone(),
        training_job_id: version.training_job_id.clone(),
        weight_artifact_hash: version.weight_artifact_hash,
        dataset_ref: version.dataset_ref.clone(),
        training_config_hash: version.training_config_hash,
        reproducibility_hash: version.reproducibility_hash,
        provenance_attestation_hash: version.provenance_attestation_hash,
        registered_sequence: version.registered_sequence,
        promoted_sequence: version.promoted_sequence,
        gate_report_hash: version.gate_report_hash,
    }
}

fn model_artifact_status_entry(
    service_name: &str,
    artifact: &ModelArtifactState,
) -> ModelArtifactStatusEntry {
    ModelArtifactStatusEntry {
        service_name: service_name.to_owned(),
        model_name: artifact.model_name.clone(),
        artifact_id: artifact
            .artifact_id
            .clone()
            .unwrap_or_else(|| artifact.training_job_id.clone()),
        training_job_id: artifact.training_job_id.clone(),
        weight_version: artifact.weight_version.clone(),
        weight_artifact_hash: artifact.weight_artifact_hash,
        dataset_ref: artifact.dataset_ref.clone(),
        training_config_hash: artifact.training_config_hash,
        reproducibility_hash: artifact.reproducibility_hash,
        provenance_attestation_hash: artifact.provenance_attestation_hash,
        registered_sequence: artifact.registered_sequence,
        consumed_by_version: artifact.consumed_by_version.clone(),
        private_bundle_root: artifact.private_bundle_root,
        compile_profile_hash: artifact.compile_profile_hash,
        chunk_manifest_root: artifact.chunk_manifest_root,
        privacy_mode: artifact.privacy_mode,
    }
}

fn authoritative_training_job_status(status: SoraTrainingJobStatusV1) -> TrainingJobStatus {
    match status {
        SoraTrainingJobStatusV1::Running => TrainingJobStatus::Running,
        SoraTrainingJobStatusV1::Completed => TrainingJobStatus::Completed,
        SoraTrainingJobStatusV1::RetryPending => TrainingJobStatus::RetryPending,
        SoraTrainingJobStatusV1::Exhausted => TrainingJobStatus::Exhausted,
    }
}

fn authoritative_training_job_status_entry(
    service_name: &str,
    record: &SoraTrainingJobRecordV1,
) -> TrainingJobStatusEntry {
    TrainingJobStatusEntry {
        service_name: service_name.to_owned(),
        model_name: record.model_name.clone(),
        job_id: record.job_id.clone(),
        status: authoritative_training_job_status(record.status),
        worker_group_size: record.worker_group_size,
        target_steps: record.target_steps,
        completed_steps: record.completed_steps,
        checkpoint_interval_steps: record.checkpoint_interval_steps,
        last_checkpoint_step: record.last_checkpoint_step,
        checkpoint_count: record.checkpoint_count,
        retry_count: record.retry_count,
        max_retries: record.max_retries,
        step_compute_units: record.step_compute_units,
        compute_budget_units: record.compute_budget_units,
        compute_consumed_units: record.compute_consumed_units,
        compute_remaining_units: record
            .compute_budget_units
            .saturating_sub(record.compute_consumed_units),
        storage_budget_bytes: record.storage_budget_bytes,
        storage_consumed_bytes: record.storage_consumed_bytes,
        storage_remaining_bytes: record
            .storage_budget_bytes
            .saturating_sub(record.storage_consumed_bytes),
        latest_metrics_hash: record.latest_metrics_hash,
        last_failure_reason: record.last_failure_reason.clone(),
        created_sequence: record.created_sequence,
        updated_sequence: record.updated_sequence,
    }
}

fn authoritative_model_weight_version_entry(
    version: &SoraModelWeightVersionRecordV1,
) -> ModelWeightVersionEntry {
    ModelWeightVersionEntry {
        weight_version: version.weight_version.clone(),
        parent_version: version.parent_version.clone(),
        training_job_id: version.training_job_id.clone(),
        weight_artifact_hash: version.weight_artifact_hash,
        dataset_ref: version.dataset_ref.clone(),
        training_config_hash: version.training_config_hash,
        reproducibility_hash: version.reproducibility_hash,
        provenance_attestation_hash: version.provenance_attestation_hash,
        registered_sequence: version.registered_sequence,
        promoted_sequence: version.promoted_sequence,
        gate_report_hash: version.gate_report_hash,
    }
}

fn authoritative_model_weight_status_entry(
    service_name: &str,
    registry: &SoraModelRegistryV1,
    versions: Vec<ModelWeightVersionEntry>,
) -> ModelWeightStatusEntry {
    ModelWeightStatusEntry {
        service_name: service_name.to_owned(),
        model_name: registry.model_name.clone(),
        current_version: registry.current_version.clone(),
        version_count: u32::try_from(versions.len()).unwrap_or(u32::MAX),
        versions,
    }
}

fn authoritative_model_artifact_status_entry(
    service_name: &str,
    artifact: &SoraModelArtifactRecordV1,
) -> ModelArtifactStatusEntry {
    ModelArtifactStatusEntry {
        service_name: service_name.to_owned(),
        model_name: artifact.model_name.clone(),
        artifact_id: artifact.artifact_id.clone(),
        training_job_id: artifact.training_job_id.clone(),
        weight_version: artifact.weight_version.clone(),
        weight_artifact_hash: artifact.weight_artifact_hash,
        dataset_ref: artifact.dataset_ref.clone(),
        training_config_hash: artifact.training_config_hash,
        reproducibility_hash: artifact.reproducibility_hash,
        provenance_attestation_hash: artifact.provenance_attestation_hash,
        registered_sequence: artifact.registered_sequence,
        consumed_by_version: artifact.consumed_by_version.clone(),
        private_bundle_root: artifact.private_bundle_root,
        compile_profile_hash: artifact.compile_profile_hash,
        chunk_manifest_root: artifact.chunk_manifest_root,
        privacy_mode: artifact.privacy_mode,
    }
}

fn validate_agent_hash_like(flag_name: &str, value: &str) -> Result<(), SoracloudError> {
    if value.is_empty() {
        return Err(SoracloudError::bad_request(format!(
            "{flag_name} must not be empty"
        )));
    }
    if value.len() > AGENT_AUTONOMY_MAX_HASH_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "{flag_name} exceeds max bytes ({AGENT_AUTONOMY_MAX_HASH_BYTES})"
        )));
    }
    if value.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(SoracloudError::bad_request(format!(
            "{flag_name} must not contain whitespace"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '-' | '_' | '.' | '#'))
    {
        return Err(SoracloudError::bad_request(format!(
            "{flag_name} must use only ASCII letters, digits, or [: - _ . #]"
        )));
    }
    Ok(())
}

fn normalize_agent_hash_like(flag_name: &str, value: &str) -> Result<String, SoracloudError> {
    let normalized = value.trim();
    validate_agent_hash_like(flag_name, normalized)?;
    Ok(normalized.to_owned())
}

fn normalize_optional_agent_hash_like(
    flag_name: &str,
    value: Option<&str>,
) -> Result<Option<String>, SoracloudError> {
    value
        .map(|raw| normalize_agent_hash_like(flag_name, raw))
        .transpose()
}

fn normalize_run_label(run_label: &str) -> Result<String, SoracloudError> {
    let normalized = run_label.trim();
    if normalized.is_empty() {
        return Err(SoracloudError::bad_request("run_label must not be empty"));
    }
    if normalized.len() > AGENT_AUTONOMY_MAX_LABEL_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "run_label exceeds max bytes ({AGENT_AUTONOMY_MAX_LABEL_BYTES})"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "run_label must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}

fn agent_policy_capability_active(state: &AgentApartmentRuntimeState, capability: &str) -> bool {
    let declared = state
        .manifest
        .policy_capabilities
        .iter()
        .any(|candidate| candidate.as_ref() == capability);
    declared && !state.revoked_policy_capabilities.contains(capability)
}

fn agent_runtime_status_for_sequence(
    state: &AgentApartmentRuntimeState,
    current_sequence: u64,
) -> AgentRuntimeStatus {
    if current_sequence >= state.lease_expires_sequence {
        AgentRuntimeStatus::LeaseExpired
    } else {
        state.status
    }
}

fn agent_lease_remaining_ticks(state: &AgentApartmentRuntimeState, current_sequence: u64) -> u64 {
    state
        .lease_expires_sequence
        .saturating_sub(current_sequence)
}

fn agent_revoked_capability_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.revoked_policy_capabilities.len()).unwrap_or(u32::MAX)
}

fn agent_pending_wallet_request_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.pending_wallet_requests.len()).unwrap_or(u32::MAX)
}

fn agent_pending_mailbox_message_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.mailbox_queue.len()).unwrap_or(u32::MAX)
}

fn agent_allowlist_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.artifact_allowlist.len()).unwrap_or(u32::MAX)
}

fn agent_run_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.autonomy_run_history.len()).unwrap_or(u32::MAX)
}

fn agent_persistent_state_key_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.persistent_state.key_sizes.len()).unwrap_or(u32::MAX)
}

fn touch_agent_runtime_activity(state: &mut AgentApartmentRuntimeState, sequence: u64) {
    state.last_active_sequence = state.last_active_sequence.max(sequence);
}

fn projected_persistent_state_total_bytes(
    state: &AgentApartmentRuntimeState,
    key: &str,
    value_size_bytes: u64,
) -> Result<u64, SoracloudError> {
    let existing_size = state
        .persistent_state
        .key_sizes
        .get(key)
        .copied()
        .unwrap_or(0);
    state
        .persistent_state
        .total_bytes
        .saturating_sub(existing_size)
        .checked_add(value_size_bytes)
        .ok_or_else(|| {
            SoracloudError::internal(format!(
                "persistent state accounting overflow for apartment `{}`",
                state.manifest.apartment_name
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

fn wallet_day_bucket(sequence: u64) -> u64 {
    sequence / AGENT_WALLET_DAY_TICKS
}

fn wallet_day_spent(
    state: &AgentApartmentRuntimeState,
    asset_definition: &str,
    day_bucket: u64,
) -> u64 {
    let key = format!("{asset_definition}:{day_bucket}");
    state
        .wallet_daily_spend
        .get(&key)
        .map(|entry| entry.spent_nanos)
        .unwrap_or(0)
}

fn wallet_record_spend(
    state: &mut AgentApartmentRuntimeState,
    asset_definition: &str,
    day_bucket: u64,
    spent_nanos: u64,
) {
    let key = format!("{asset_definition}:{day_bucket}");
    state.wallet_daily_spend.insert(
        key,
        AgentWalletDailySpendEntry {
            asset_definition: asset_definition.to_owned(),
            day_bucket,
            spent_nanos,
        },
    );
}

fn rollout_handle(service_name: &str, sequence: u64) -> String {
    format!("{service_name}:rollout:{sequence}")
}

fn derive_ciphertext_commitment_for_state_mutation(
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

fn derive_state_key_digest(service_name: &str, binding_name: &str, state_key: &str) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.ciphertext.query.key_digest.v1",
        service_name,
        binding_name,
        state_key,
    )))
}

fn soracloud_action_label(action: SoracloudAction) -> &'static str {
    match action {
        SoracloudAction::Deploy => "deploy",
        SoracloudAction::Upgrade => "upgrade",
        SoracloudAction::Rollback => "rollback",
        SoracloudAction::ConfigMutation => "config_mutation",
        SoracloudAction::SecretMutation => "secret_mutation",
        SoracloudAction::StateMutation => "state_mutation",
        SoracloudAction::FheJobRun => "fhe_job_run",
        SoracloudAction::DecryptionRequest => "decryption_request",
        SoracloudAction::CiphertextQuery => "ciphertext_query",
        SoracloudAction::Rollout => "rollout",
    }
}

fn audit_event_leaf_hash(event: &ControlPlaneAuditEvent) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.audit.leaf.v1",
        event.sequence,
        soracloud_action_label(event.action),
        (
            event.service_name.as_str(),
            event.from_version.as_deref(),
            event.to_version.as_str(),
            event.service_manifest_hash,
            event.container_manifest_hash,
        ),
        (
            (
                event.binding_name.as_deref(),
                event.state_key.as_deref(),
                event.config_name.as_deref(),
                event.secret_name.as_deref(),
                event.governance_tx_hash,
                event.rollout_handle.as_deref(),
            ),
            (
                event.policy_name.as_deref(),
                event.policy_snapshot_hash,
                event.jurisdiction_tag.as_deref(),
                event.consent_evidence_hash,
                event.break_glass,
                event.break_glass_reason.as_deref(),
                event.signed_by.as_str(),
            ),
        ),
    )))
}

fn audit_anchor_hash(audit_log: &[ControlPlaneAuditEvent], anchor_sequence: u64) -> Hash {
    let mut accumulator = Hash::new(Encode::encode(&"soracloud.audit.anchor.seed.v1"));
    for event in audit_log
        .iter()
        .filter(|event| event.sequence <= anchor_sequence)
    {
        let leaf_hash = audit_event_leaf_hash(event);
        accumulator = Hash::new(Encode::encode(&(
            "soracloud.audit.anchor.step.v1",
            accumulator,
            event.sequence,
            leaf_hash,
        )));
    }
    accumulator
}

fn build_ciphertext_inclusion_proof(
    audit_log: &[ControlPlaneAuditEvent],
    service_name: &str,
    binding_name: &str,
    state_key: &str,
    record: &CiphertextRuntimeRecord,
    anchor_sequence: u64,
    anchor_hash: Hash,
) -> CiphertextInclusionProofV1 {
    let maybe_event = audit_log.iter().find(|event| {
        event.sequence == record.last_update_sequence
            && event.service_name == service_name
            && event.binding_name.as_deref() == Some(binding_name)
            && event.state_key.as_deref() == Some(state_key)
    });
    let (leaf_hash, event_sequence) = if let Some(event) = maybe_event {
        (audit_event_leaf_hash(event), event.sequence)
    } else {
        (
            Hash::new(Encode::encode(&(
                "soracloud.audit.synthetic_leaf.v1",
                service_name,
                binding_name,
                state_key,
                record.payload_bytes,
                record.commitment,
                record.last_update_sequence,
                record.governance_tx_hash,
                soracloud_action_label(record.source_action),
            ))),
            record.last_update_sequence,
        )
    };
    CiphertextInclusionProofV1 {
        schema_version: CIPHERTEXT_QUERY_PROOF_VERSION_V1,
        proof_scheme: CIPHERTEXT_QUERY_PROOF_SCHEME_V1.to_string(),
        leaf_hash,
        anchor_hash,
        anchor_sequence,
        event_sequence,
    }
}

fn verify_bundle_signature(request: &SignedBundleRequest) -> Result<(), SoracloudError> {
    let payload = encode_bundle_signature_payload(
        &request.bundle,
        &request.initial_service_configs,
        &request.initial_service_secrets,
    )?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("bundle provenance signature verification failed")
        })?;
    Ok(())
}

fn encode_bundle_signature_payload(
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    initial_service_secrets: &BTreeMap<String, SecretEnvelopeV1>,
) -> Result<Vec<u8>, SoracloudError> {
    iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
        bundle,
        initial_service_configs,
        initial_service_secrets,
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode bundle payload: {err}")))
}

fn verify_rollback_signature(request: &SignedRollbackRequest) -> Result<(), SoracloudError> {
    let payload = encode_rollback_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("rollback provenance signature verification failed")
        })?;
    Ok(())
}

fn encode_rollback_signature_payload(payload: &RollbackPayload) -> Result<Vec<u8>, SoracloudError> {
    encode_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.target_version.as_deref(),
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode rollback payload: {err}")))
}

fn verify_service_config_set_signature(
    request: &SignedServiceConfigSetRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_service_config_set_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("service config provenance signature verification failed")
        })?;
    Ok(())
}

fn encode_service_config_set_signature_payload(
    payload: &ServiceConfigSetRequest,
) -> Result<Vec<u8>, SoracloudError> {
    encode_set_service_config_provenance_payload(
        payload.service_name.as_str(),
        payload.config_name.as_str(),
        &payload.value_json,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode service config payload: {err}"))
    })
}

fn verify_service_config_delete_signature(
    request: &SignedServiceConfigDeleteRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_service_config_delete_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "service config delete provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn encode_service_config_delete_signature_payload(
    payload: &ServiceConfigDeleteRequest,
) -> Result<Vec<u8>, SoracloudError> {
    encode_delete_service_config_provenance_payload(
        payload.service_name.as_str(),
        payload.config_name.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode service config delete payload: {err}"
        ))
    })
}

fn verify_service_secret_set_signature(
    request: &SignedServiceSecretSetRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_service_secret_set_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("service secret provenance signature verification failed")
        })?;
    Ok(())
}

fn encode_service_secret_set_signature_payload(
    payload: &ServiceSecretSetRequest,
) -> Result<Vec<u8>, SoracloudError> {
    encode_set_service_secret_provenance_payload(
        payload.service_name.as_str(),
        payload.secret_name.as_str(),
        &payload.secret,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode service secret payload: {err}"))
    })
}

fn verify_service_secret_delete_signature(
    request: &SignedServiceSecretDeleteRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_service_secret_delete_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "service secret delete provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn encode_service_secret_delete_signature_payload(
    payload: &ServiceSecretDeleteRequest,
) -> Result<Vec<u8>, SoracloudError> {
    encode_delete_service_secret_provenance_payload(
        payload.service_name.as_str(),
        payload.secret_name.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode service secret delete payload: {err}"
        ))
    })
}

fn verify_state_mutation_signature(
    request: &SignedStateMutationRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_state_mutation_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("state mutation provenance signature verification failed")
        })?;
    Ok(())
}

fn encode_state_mutation_signature_payload(
    payload: &StateMutationRequest,
) -> Result<Vec<u8>, SoracloudError> {
    encode_state_mutation_provenance_payload(
        payload.service_name.as_str(),
        payload.binding_name.as_str(),
        payload.key.as_str(),
        state_mutation_operation_label(payload.operation),
        payload.value_size_bytes,
        payload.encryption,
        payload.governance_tx_hash,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode state mutation payload: {err}"))
    })
}

fn state_mutation_operation_label(operation: StateMutationOperation) -> &'static str {
    match operation {
        StateMutationOperation::Upsert => "upsert",
        StateMutationOperation::Delete => "delete",
    }
}

fn verify_fhe_job_run_signature(request: &SignedFheJobRunRequest) -> Result<(), SoracloudError> {
    let payload = encode_fhe_job_run_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("fhe job run provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_training_job_start_signature(
    request: &SignedTrainingJobStartRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_training_job_start_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "training job start provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_training_job_checkpoint_signature(
    request: &SignedTrainingJobCheckpointRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_training_job_checkpoint_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "training checkpoint provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_training_job_retry_signature(
    request: &SignedTrainingJobRetryRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_training_job_retry_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("training retry provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_model_weight_register_signature(
    request: &SignedModelWeightRegisterRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_weight_register_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model weight register provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_weight_promote_signature(
    request: &SignedModelWeightPromoteRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_weight_promote_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model weight promote provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_weight_rollback_signature(
    request: &SignedModelWeightRollbackRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_weight_rollback_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model weight rollback provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_artifact_register_signature(
    request: &SignedModelArtifactRegisterRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_artifact_register_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model artifact register provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_uploaded_model_bundle_init_signature(
    request: &SignedUploadedModelBundleInitRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_uploaded_model_bundle_init_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "uploaded model init provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_uploaded_model_chunk_signature(
    request: &SignedUploadedModelChunkRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_uploaded_model_chunk_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "uploaded model chunk provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_uploaded_model_finalize_signature(
    request: &SignedUploadedModelFinalizeRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_uploaded_model_finalize_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "uploaded model finalize provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_private_compile_signature(
    request: &SignedPrivateCompileRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_private_compile_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("private compile provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_uploaded_model_allow_signature(
    request: &SignedUploadedModelAllowRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_uploaded_model_allow_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "uploaded model allow provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_private_inference_run_signature(
    request: &SignedPrivateInferenceRunRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_private_inference_run_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "private inference run provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_private_inference_output_release_signature(
    request: &SignedPrivateInferenceOutputReleaseRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_private_inference_output_release_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "private inference output release provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_decryption_request_signature(
    request: &SignedDecryptionRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_decryption_request_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "decryption request provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_ciphertext_query_signature(
    request: &SignedCiphertextQueryRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_ciphertext_query_signature_payload(&request.query)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "ciphertext query provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_rollout_signature(request: &SignedRolloutAdvanceRequest) -> Result<(), SoracloudError> {
    let payload = encode_rollout_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("rollout provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_agent_deploy_signature(request: &SignedAgentDeployRequest) -> Result<(), SoracloudError> {
    let payload = encode_agent_deploy_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("agent deploy provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_agent_lease_renew_signature(
    request: &SignedAgentLeaseRenewRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_lease_renew_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent lease renew provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_restart_signature(
    request: &SignedAgentRestartRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_restart_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("agent restart provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_agent_policy_revoke_signature(
    request: &SignedAgentPolicyRevokeRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_policy_revoke_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent policy revoke provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_wallet_spend_signature(
    request: &SignedAgentWalletSpendRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_wallet_spend_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent wallet spend provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_wallet_approve_signature(
    request: &SignedAgentWalletApproveRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_wallet_approve_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent wallet approve provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_message_send_signature(
    request: &SignedAgentMessageSendRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_message_send_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent message send provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_message_ack_signature(
    request: &SignedAgentMessageAckRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_message_ack_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent message ack provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_artifact_allow_signature(
    request: &SignedAgentArtifactAllowRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_artifact_allow_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent artifact allow provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_agent_autonomy_run_signature(
    request: &SignedAgentAutonomyRunRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_agent_autonomy_run_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "agent autonomy run provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_hf_deploy_signature(request: &SignedHfDeployRequest) -> Result<(), SoracloudError> {
    let payload = encode_hf_deploy_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("hf deploy provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_hf_lease_leave_signature(
    request: &SignedHfLeaseLeaveRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_hf_lease_leave_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "hf shared-lease leave provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_hf_lease_renew_signature(
    request: &SignedHfLeaseRenewRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_hf_lease_renew_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "hf shared-lease renew provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_host_advertise_signature(
    request: &SignedModelHostAdvertiseRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_host_advertise_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model host advertise provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_host_heartbeat_signature(
    request: &SignedModelHostHeartbeatRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_host_heartbeat_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model host heartbeat provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn verify_model_host_withdraw_signature(
    request: &SignedModelHostWithdrawRequest,
) -> Result<(), SoracloudError> {
    let payload = encode_model_host_withdraw_signature_payload(&request.payload)?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized(
                "model host withdraw provenance signature verification failed",
            )
        })?;
    Ok(())
}

fn encode_rollout_signature_payload(
    payload: &RolloutAdvancePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_rollout_provenance_payload(
        payload.service_name.as_str(),
        payload.rollout_handle.as_str(),
        payload.healthy,
        payload.promote_to_percent,
        payload.governance_tx_hash.clone(),
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode rollout payload: {err}")))
}

fn encode_fhe_job_run_signature_payload(
    payload: &FheJobRunPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_fhe_job_run_provenance_payload(
        payload.service_name.as_str(),
        payload.binding_name.as_str(),
        payload.job.clone(),
        payload.policy.clone(),
        payload.param_set.clone(),
        payload.governance_tx_hash.clone(),
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode fhe job payload: {err}")))
}

fn encode_training_job_start_signature_payload(
    payload: &TrainingJobStartPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_training_job_start_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.job_id.as_str(),
        payload.worker_group_size,
        payload.target_steps,
        payload.checkpoint_interval_steps,
        payload.max_retries,
        payload.step_compute_units,
        payload.compute_budget_units,
        payload.storage_budget_bytes,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode training start payload: {err}"))
    })
}

fn encode_training_job_checkpoint_signature_payload(
    payload: &TrainingJobCheckpointPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_training_job_checkpoint_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.completed_step,
        payload.checkpoint_size_bytes,
        payload.metrics_hash.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode training checkpoint payload: {err}"
        ))
    })
}

fn encode_training_job_retry_signature_payload(
    payload: &TrainingJobRetryPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_training_job_retry_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.reason.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode training retry payload: {err}"))
    })
}

fn encode_model_weight_register_signature_payload(
    payload: &ModelWeightRegisterPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_model_weight_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.training_job_id.as_str(),
        payload.parent_version.as_deref(),
        payload.weight_artifact_hash.clone(),
        payload.dataset_ref.as_str(),
        payload.training_config_hash.clone(),
        payload.reproducibility_hash.clone(),
        payload.provenance_attestation_hash.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model weight register payload: {err}"
        ))
    })
}

fn encode_model_weight_promote_signature_payload(
    payload: &ModelWeightPromotePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_model_weight_promote_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.gate_approved,
        payload.gate_report_hash.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model weight promote payload: {err}"
        ))
    })
}

fn encode_model_weight_rollback_signature_payload(
    payload: &ModelWeightRollbackPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_model_weight_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.target_version.as_str(),
        payload.reason.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model weight rollback payload: {err}"
        ))
    })
}

fn encode_model_artifact_register_signature_payload(
    payload: &ModelArtifactRegisterPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_model_artifact_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.training_job_id.as_str(),
        payload.weight_artifact_hash.clone(),
        payload.dataset_ref.as_str(),
        payload.training_config_hash.clone(),
        payload.reproducibility_hash.clone(),
        payload.provenance_attestation_hash.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model artifact register payload: {err}"
        ))
    })
}

fn encode_uploaded_model_bundle_init_signature_payload(
    payload: &UploadedModelBundleInitPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_bundle_register_provenance_payload(payload.bundle.clone()).map_err(
        |err| {
            SoracloudError::internal(format!(
                "failed to encode uploaded model init payload: {err}"
            ))
        },
    )
}

fn encode_uploaded_model_chunk_signature_payload(
    payload: &UploadedModelChunkPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_chunk_append_provenance_payload(payload.chunk.clone()).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode uploaded model chunk payload: {err}"
        ))
    })
}

fn encode_uploaded_model_finalize_signature_payload(
    payload: &UploadedModelFinalizePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_finalize_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.model_id.as_str(),
        payload.artifact_id.as_str(),
        payload.weight_version.as_str(),
        payload.bundle_root,
        payload.privacy_mode,
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode uploaded model finalize payload: {err}"
        ))
    })
}

fn encode_private_compile_signature_payload(
    payload: &PrivateCompilePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_private_compile_profile_provenance_payload(
        payload.service_name.as_str(),
        payload.model_id.as_str(),
        payload.weight_version.as_str(),
        payload.bundle_root,
        payload.compile_profile.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode private compile payload: {err}"))
    })
}

fn encode_uploaded_model_allow_signature_payload(
    payload: &UploadedModelAllowPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_allow_provenance_payload(
        payload.apartment_name.as_str(),
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.model_id.as_str(),
        payload.artifact_id.as_str(),
        payload.weight_version.as_str(),
        payload.bundle_root,
        payload.compile_profile_hash,
        payload.privacy_mode,
        payload.require_model_inference,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode uploaded model allow payload: {err}"
        ))
    })
}

fn encode_private_inference_run_signature_payload(
    payload: &PrivateInferenceRunPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_private_inference_start_provenance_payload(payload.session.clone()).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode private inference run payload: {err}"
        ))
    })
}

fn encode_private_inference_output_release_signature_payload(
    payload: &PrivateInferenceOutputReleasePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_private_inference_output_release_provenance_payload(
        payload.session_id.as_str(),
        payload.decrypt_request_id.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode private inference output release payload: {err}"
        ))
    })
}

fn encode_decryption_request_signature_payload(
    payload: &DecryptionRequestPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_decryption_request_provenance_payload(
        payload.service_name.as_str(),
        payload.policy.clone(),
        payload.request.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode decryption request payload: {err}"
        ))
    })
}

fn encode_ciphertext_query_signature_payload(
    payload: &CiphertextQuerySpecV1,
) -> Result<Vec<u8>, SoracloudError> {
    encode_ciphertext_query_provenance_payload(payload).map_err(|err| {
        SoracloudError::internal(format!("failed to encode ciphertext query payload: {err}"))
    })
}

fn encode_agent_deploy_signature_payload(
    payload: &AgentDeployPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_deploy_provenance_payload(
        payload.manifest.clone(),
        payload.lease_ticks,
        payload.autonomy_budget_units,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode agent deploy payload: {err}"))
    })
}

fn encode_agent_lease_renew_signature_payload(
    payload: &AgentLeaseRenewPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_lease_renew_provenance_payload(
        payload.apartment_name.as_str(),
        payload.lease_ticks,
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode agent lease renew payload: {err}"))
    })
}

fn encode_agent_restart_signature_payload(
    payload: &AgentRestartPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_restart_provenance_payload(
        payload.apartment_name.as_str(),
        payload.reason.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode agent restart payload: {err}"))
    })
}

fn encode_agent_policy_revoke_signature_payload(
    payload: &AgentPolicyRevokePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_policy_revoke_provenance_payload(
        payload.apartment_name.as_str(),
        payload.capability.as_str(),
        payload.reason.as_deref(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent policy revoke payload: {err}"
        ))
    })
}

fn encode_agent_wallet_spend_signature_payload(
    payload: &AgentWalletSpendPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_wallet_spend_provenance_payload(
        payload.apartment_name.as_str(),
        payload.asset_definition.as_str(),
        payload.amount_nanos,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent wallet spend payload: {err}"
        ))
    })
}

fn encode_agent_wallet_approve_signature_payload(
    payload: &AgentWalletApprovePayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_wallet_approve_provenance_payload(
        payload.apartment_name.as_str(),
        payload.request_id.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent wallet approve payload: {err}"
        ))
    })
}

fn encode_agent_message_send_signature_payload(
    payload: &AgentMessageSendPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_message_send_provenance_payload(
        payload.from_apartment.as_str(),
        payload.to_apartment.as_str(),
        payload.channel.as_str(),
        payload.payload.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent message send payload: {err}"
        ))
    })
}

fn encode_agent_message_ack_signature_payload(
    payload: &AgentMessageAckPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_message_ack_provenance_payload(
        payload.apartment_name.as_str(),
        payload.message_id.as_str(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode agent message ack payload: {err}"))
    })
}

fn encode_agent_artifact_allow_signature_payload(
    payload: &AgentArtifactAllowPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_artifact_allow_provenance_payload(
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent artifact allow payload: {err}"
        ))
    })
}

fn encode_agent_autonomy_run_signature_payload(
    payload: &AgentAutonomyRunPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_agent_autonomy_run_provenance_payload(
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
        payload.budget_units,
        payload.run_label.as_str(),
        payload.workflow_input_json.as_deref(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent autonomy run payload: {err}"
        ))
    })
}

fn encode_hf_deploy_signature_payload(
    payload: &HfDeployPayload,
) -> Result<Vec<u8>, SoracloudError> {
    let repo_id = parse_hf_repo_id(&payload.repo_id)?;
    let resolved_revision = parse_hf_resolved_revision(payload.revision.as_deref())?;
    let model_name = parse_hf_model_name(&payload.model_name)?;
    let service_name = parse_service_name(&payload.service_name)?.to_string();
    let apartment_name = payload
        .apartment_name
        .as_deref()
        .map(parse_agent_apartment_name)
        .transpose()?;
    if payload.lease_term_ms == 0 {
        return Err(SoracloudError::bad_request(
            "lease_term_ms must be greater than zero",
        ));
    }
    if payload.base_fee_nanos == 0 {
        return Err(SoracloudError::bad_request(
            "base_fee_nanos must be greater than zero",
        ));
    }
    encode_hf_shared_lease_join_provenance_payload(
        &repo_id,
        &resolved_revision,
        &model_name,
        &service_name,
        apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        &payload.lease_asset_definition_id,
        payload.base_fee_nanos,
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode hf deploy payload: {err}")))
}

fn encode_hf_lease_leave_signature_payload(
    payload: &HfLeaseLeavePayload,
) -> Result<Vec<u8>, SoracloudError> {
    let repo_id = parse_hf_repo_id(&payload.repo_id)?;
    let resolved_revision = parse_hf_resolved_revision(payload.revision.as_deref())?;
    let service_name = payload
        .service_name
        .as_deref()
        .map(|value| parse_service_name(value).map(|name| name.to_string()))
        .transpose()?;
    let apartment_name = payload
        .apartment_name
        .as_deref()
        .map(parse_agent_apartment_name)
        .transpose()?;
    if payload.lease_term_ms == 0 {
        return Err(SoracloudError::bad_request(
            "lease_term_ms must be greater than zero",
        ));
    }
    encode_hf_shared_lease_leave_provenance_payload(
        &repo_id,
        &resolved_revision,
        payload.storage_class,
        payload.lease_term_ms,
        service_name.as_deref(),
        apartment_name.as_deref(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode hf shared-lease leave payload: {err}"
        ))
    })
}

fn encode_hf_lease_renew_signature_payload(
    payload: &HfLeaseRenewPayload,
) -> Result<Vec<u8>, SoracloudError> {
    let repo_id = parse_hf_repo_id(&payload.repo_id)?;
    let resolved_revision = parse_hf_resolved_revision(payload.revision.as_deref())?;
    let model_name = parse_hf_model_name(&payload.model_name)?;
    let service_name = parse_service_name(&payload.service_name)?.to_string();
    let apartment_name = payload
        .apartment_name
        .as_deref()
        .map(parse_agent_apartment_name)
        .transpose()?;
    if payload.lease_term_ms == 0 {
        return Err(SoracloudError::bad_request(
            "lease_term_ms must be greater than zero",
        ));
    }
    if payload.base_fee_nanos == 0 {
        return Err(SoracloudError::bad_request(
            "base_fee_nanos must be greater than zero",
        ));
    }
    encode_hf_shared_lease_renew_provenance_payload(
        &repo_id,
        &resolved_revision,
        &model_name,
        &service_name,
        apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        &payload.lease_asset_definition_id,
        payload.base_fee_nanos,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode hf shared-lease renew payload: {err}"
        ))
    })
}

fn encode_model_host_advertise_signature_payload(
    payload: &ModelHostAdvertisePayload,
) -> Result<Vec<u8>, SoracloudError> {
    payload
        .capability
        .validate()
        .map_err(|err| SoracloudError::bad_request(err.to_string()))?;
    encode_model_host_advertise_provenance_payload(&payload.capability).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model host advertise payload: {err}"
        ))
    })
}

fn encode_model_host_heartbeat_signature_payload(
    payload: &ModelHostHeartbeatPayload,
) -> Result<Vec<u8>, SoracloudError> {
    if payload.heartbeat_expires_at_ms == 0 {
        return Err(SoracloudError::bad_request(
            "heartbeat_expires_at_ms must be greater than zero",
        ));
    }
    encode_model_host_heartbeat_provenance_payload(
        &payload.validator_account_id,
        payload.heartbeat_expires_at_ms,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model host heartbeat payload: {err}"
        ))
    })
}

fn encode_model_host_withdraw_signature_payload(
    payload: &ModelHostWithdrawPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_model_host_withdraw_provenance_payload(&payload.validator_account_id).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode model host withdraw payload: {err}"
        ))
    })
}

#[derive(Clone)]
struct SoracloudMutationSigner {
    authority: AccountId,
    request_signer: PublicKey,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SoracloudMutationDraftResponse {
    ok: bool,
    authority: String,
    signed_by: String,
    tx_instructions: Vec<SoracloudTxInstr>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SoracloudTxInstr {
    wire_id: String,
    payload_hex: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct SoracloudAuditBaseline {
    service_max: u64,
    training_job_max: u64,
    model_weight_max: u64,
    model_artifact_max: u64,
    hf_shared_lease_max: u64,
    agent_apartment_max: u64,
}

#[derive(Debug)]
enum SoracloudMutationError {
    Torii(crate::Error),
    Soracloud(SoracloudError),
}

impl From<crate::Error> for SoracloudMutationError {
    fn from(err: crate::Error) -> Self {
        Self::Torii(err)
    }
}

impl From<SoracloudError> for SoracloudMutationError {
    fn from(err: SoracloudError) -> Self {
        Self::Soracloud(err)
    }
}

impl IntoResponse for SoracloudMutationError {
    fn into_response(self) -> Response {
        match self {
            Self::Torii(err) => err.into_response(),
            Self::Soracloud(err) => err.into_response(),
        }
    }
}

fn verified_soracloud_request_identity(
    headers: &HeaderMap,
) -> Result<(AccountId, PublicKey), SoracloudError> {
    let account = headers
        .get(VERIFIED_ACCOUNT_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            SoracloudError::unauthorized(
                "signed request headers are required for Soracloud mutation endpoints",
            )
        })
        .and_then(|literal| {
            AccountId::parse_encoded(literal.trim())
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .map_err(|_| {
                    SoracloudError::internal(
                        "failed to parse verified Soracloud account header".to_owned(),
                    )
                })
        })?;
    let signer = headers
        .get(VERIFIED_SIGNER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            SoracloudError::unauthorized(
                "signed request headers are required for Soracloud mutation endpoints",
            )
        })
        .and_then(|literal| {
            literal.trim().parse::<PublicKey>().map_err(|_| {
                SoracloudError::internal(
                    "failed to parse verified Soracloud signer header".to_owned(),
                )
            })
        })?;
    Ok((account, signer))
}

fn require_soracloud_mutation_signer(
    headers: &HeaderMap,
    provenance: &ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
) -> Result<SoracloudMutationSigner, SoracloudError> {
    if authority.is_some() || private_key.is_some() {
        return Err(SoracloudError::bad_request(
            "authority/private_key fields are no longer accepted; sign the HTTP request with X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, and X-Iroha-Nonce",
        ));
    }
    let (authority, request_signer) = verified_soracloud_request_identity(headers)?;
    if provenance.signer != request_signer {
        return Err(SoracloudError::unauthorized(
            "request signer must match mutation provenance signer",
        ));
    }
    Ok(SoracloudMutationSigner {
        authority,
        request_signer,
    })
}

fn require_soracloud_request_signer(
    headers: &HeaderMap,
) -> Result<SoracloudMutationSigner, SoracloudError> {
    let (authority, request_signer) = verified_soracloud_request_identity(headers)?;
    Ok(SoracloudMutationSigner {
        authority,
        request_signer,
    })
}

fn soracloud_draft_response(
    signer: &SoracloudMutationSigner,
    instructions: Vec<InstructionBox>,
) -> Response {
    let tx_instructions = instructions
        .into_iter()
        .map(soracloud_tx_instr_from_box)
        .collect();
    JsonBody(SoracloudMutationDraftResponse {
        ok: true,
        authority: signer.authority.to_string(),
        signed_by: signer.request_signer.to_string(),
        tx_instructions,
    })
    .into_response()
}

fn soracloud_tx_instr_from_box(boxed: InstructionBox) -> SoracloudTxInstr {
    use iroha_data_model::isi::Instruction;

    let type_name = Instruction::id(&*boxed);
    let payload = Instruction::dyn_encode(&*boxed);
    let framed = iroha_data_model::isi::frame_instruction_payload(type_name, &payload)
        .expect("instruction payload must use canonical Norito framing");
    SoracloudTxInstr {
        wire_id: type_name.to_string(),
        payload_hex: hex::encode(framed),
    }
}

fn latest_service_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraServiceAuditEventV1>
where
    P: Fn(&SoraServiceAuditEventV1) -> bool,
{
    world
        .soracloud_service_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

fn latest_training_job_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraTrainingJobAuditEventV1>
where
    P: Fn(&SoraTrainingJobAuditEventV1) -> bool,
{
    world
        .soracloud_training_job_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

fn latest_model_weight_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraModelWeightAuditEventV1>
where
    P: Fn(&SoraModelWeightAuditEventV1) -> bool,
{
    world
        .soracloud_model_weight_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

fn latest_model_artifact_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraModelArtifactAuditEventV1>
where
    P: Fn(&SoraModelArtifactAuditEventV1) -> bool,
{
    world
        .soracloud_model_artifact_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

fn latest_hf_shared_lease_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraHfSharedLeaseAuditEventV1>
where
    P: Fn(&SoraHfSharedLeaseAuditEventV1) -> bool,
{
    world
        .soracloud_hf_shared_lease_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

fn latest_agent_apartment_audit_event_after<'a, P>(
    world: &'a impl WorldReadOnly,
    after_sequence: u64,
    predicate: P,
) -> Option<&'a SoraAgentApartmentAuditEventV1>
where
    P: Fn(&SoraAgentApartmentAuditEventV1) -> bool,
{
    world
        .soracloud_agent_apartment_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.sequence > after_sequence && predicate(event))
        .map(|(_sequence, event)| event)
        .max_by_key(|event| event.sequence)
}

#[cfg(test)]
fn error_chain_message(error: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = Vec::new();
    let mut current = Some(error);
    while let Some(err) = current {
        let message = err.to_string();
        if !message.is_empty() && parts.last() != Some(&message) {
            parts.push(message);
        }
        current = err.source();
    }
    parts.join(": ")
}

#[cfg(test)]
fn join_nested_message(primary: String, nested: String) -> String {
    if nested.is_empty() || nested == primary {
        primary
    } else if nested.starts_with(&primary) {
        nested
    } else {
        format!("{primary}: {nested}")
    }
}

#[cfg(test)]
fn instruction_execution_message(
    error: &iroha_data_model::isi::error::InstructionExecutionError,
) -> String {
    use iroha_data_model::isi::error::InstructionExecutionError;

    match error {
        InstructionExecutionError::InvalidParameter(inner) => {
            join_nested_message(error.to_string(), inner.to_string())
        }
        _ => error_chain_message(error),
    }
}

#[cfg(test)]
fn validation_fail_message(validation: &iroha_data_model::ValidationFail) -> String {
    match validation {
        iroha_data_model::ValidationFail::InstructionFailed(error) => {
            join_nested_message(validation.to_string(), instruction_execution_message(error))
        }
        _ => error_chain_message(validation),
    }
}

#[cfg(test)]
fn transaction_rejection_message(
    reason: &iroha_data_model::transaction::error::TransactionRejectionReason,
) -> String {
    use iroha_data_model::transaction::error::TransactionRejectionReason;

    match reason {
        TransactionRejectionReason::Validation(validation) => {
            join_nested_message(reason.to_string(), validation_fail_message(validation))
        }
        TransactionRejectionReason::InstructionExecution(error) => {
            join_nested_message(reason.to_string(), error.to_string())
        }
        _ => error_chain_message(reason),
    }
}

async fn submit_confirm_and_respond<T, F>(
    _app: &SharedAppState,
    signer: SoracloudMutationSigner,
    instruction: InstructionBox,
    _endpoint: &'static str,
    _build_response: F,
) -> Result<Response, SoracloudMutationError>
where
    T: norito::json::JsonSerialize + Send,
    F: FnOnce(&SharedAppState, &SoracloudAuditBaseline) -> Result<T, SoracloudError>,
{
    submit_confirm_and_respond_instructions(
        _app,
        signer,
        vec![instruction],
        _endpoint,
        _build_response,
    )
    .await
}

async fn submit_confirm_and_respond_instructions<T, F>(
    _app: &SharedAppState,
    signer: SoracloudMutationSigner,
    instructions: Vec<InstructionBox>,
    _endpoint: &'static str,
    _build_response: F,
) -> Result<Response, SoracloudMutationError>
where
    T: norito::json::JsonSerialize + Send,
    F: FnOnce(&SharedAppState, &SoracloudAuditBaseline) -> Result<T, SoracloudError>,
{
    let tx_instructions = instructions
        .into_iter()
        .map(soracloud_tx_instr_from_box)
        .collect();
    Ok(JsonBody(SoracloudMutationDraftResponse {
        ok: true,
        authority: signer.authority.to_string(),
        signed_by: signer.request_signer.to_string(),
        tx_instructions,
    })
    .into_response())
}

fn audit_action_to_control_plane_action(action: SoraServiceLifecycleActionV1) -> SoracloudAction {
    match action {
        SoraServiceLifecycleActionV1::Deploy => SoracloudAction::Deploy,
        SoraServiceLifecycleActionV1::Upgrade => SoracloudAction::Upgrade,
        SoraServiceLifecycleActionV1::ConfigMutation => SoracloudAction::ConfigMutation,
        SoraServiceLifecycleActionV1::SecretMutation => SoracloudAction::SecretMutation,
        SoraServiceLifecycleActionV1::StateMutation => SoracloudAction::StateMutation,
        SoraServiceLifecycleActionV1::FheJobRun => SoracloudAction::FheJobRun,
        SoraServiceLifecycleActionV1::DecryptionRequest => SoracloudAction::DecryptionRequest,
        SoraServiceLifecycleActionV1::CiphertextQuery => SoracloudAction::CiphertextQuery,
        SoraServiceLifecycleActionV1::Rollout => SoracloudAction::Rollout,
        SoraServiceLifecycleActionV1::Rollback => SoracloudAction::Rollback,
    }
}

fn rollout_stage_to_control_plane_stage(stage: SoraRolloutStageV1) -> RolloutStage {
    match stage {
        SoraRolloutStageV1::Canary => RolloutStage::Canary,
        SoraRolloutStageV1::Promoted => RolloutStage::Promoted,
        SoraRolloutStageV1::RolledBack => RolloutStage::RolledBack,
    }
}

fn rollout_state_to_runtime_state(state: &SoraServiceRolloutStateV1) -> RolloutRuntimeState {
    RolloutRuntimeState {
        rollout_handle: state.rollout_handle.clone(),
        baseline_version: state.baseline_version.clone(),
        candidate_version: state.candidate_version.clone(),
        canary_percent: state.canary_percent,
        traffic_percent: state.traffic_percent,
        stage: rollout_stage_to_control_plane_stage(state.stage),
        health_failures: state.health_failures,
        max_health_failures: state.max_health_failures,
        health_window_secs: state.health_window_secs,
        created_sequence: state.created_sequence,
        updated_sequence: state.updated_sequence,
    }
}

fn audit_event_to_control_plane_audit_event(
    event: &SoraServiceAuditEventV1,
) -> ControlPlaneAuditEvent {
    ControlPlaneAuditEvent {
        sequence: event.sequence,
        action: audit_action_to_control_plane_action(event.action),
        service_name: event.service_name.to_string(),
        from_version: event.from_version.clone(),
        to_version: event.to_version.clone(),
        service_manifest_hash: event.service_manifest_hash,
        container_manifest_hash: event.container_manifest_hash,
        binding_name: event.binding_name.as_ref().map(ToString::to_string),
        state_key: event.state_key.clone(),
        config_name: event.config_name.clone(),
        secret_name: event.secret_name.clone(),
        governance_tx_hash: event.governance_tx_hash,
        rollout_handle: event.rollout_handle.clone(),
        policy_name: event.policy_name.as_ref().map(ToString::to_string),
        policy_snapshot_hash: event.policy_snapshot_hash,
        jurisdiction_tag: event.jurisdiction_tag.clone(),
        consent_evidence_hash: event.consent_evidence_hash,
        break_glass: event.break_glass,
        break_glass_reason: event.break_glass_reason.clone(),
        signed_by: event.signer.to_string(),
    }
}

fn state_mutation_operation_to_model(
    operation: StateMutationOperation,
) -> SoraStateMutationOperationV1 {
    match operation {
        StateMutationOperation::Upsert => SoraStateMutationOperationV1::Upsert,
        StateMutationOperation::Delete => SoraStateMutationOperationV1::Delete,
    }
}

fn authoritative_audit_log(app: &SharedAppState) -> Vec<ControlPlaneAuditEvent> {
    let state_view = app.state.view();
    let world = state_view.world();
    let mut audit_log = world
        .soracloud_service_audit_events()
        .iter()
        .map(|(_sequence, event)| audit_event_to_control_plane_audit_event(event))
        .collect::<Vec<_>>();
    audit_log.sort_by_key(|event| event.sequence);
    audit_log
}

fn authoritative_training_job_status_response(
    app: &SharedAppState,
    service_name: &str,
    job_id: &str,
) -> Result<TrainingJobStatusResponse, SoracloudError> {
    let service_name: Name = service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let service_name = service_name.to_string();
    let job_id = parse_training_job_id(job_id)?;
    let state_view = app.state.view();
    let world = state_view.world();

    let record = world
        .soracloud_training_jobs()
        .get(&(service_name.clone(), job_id))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "training job not found for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;

    Ok(TrainingJobStatusResponse {
        schema_version: TRAINING_JOB_STATUS_SCHEMA_VERSION_V1,
        job: authoritative_training_job_status_entry(&service_name, &record),
    })
}

fn authoritative_model_weight_status_response(
    app: &SharedAppState,
    service_name: &str,
    model_name: &str,
) -> Result<ModelWeightStatusResponse, SoracloudError> {
    let service_name: Name = service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let service_name = service_name.to_string();
    let model_name = parse_training_model_name(model_name)?;
    let state_view = app.state.view();
    let world = state_view.world();

    let registry = world
        .soracloud_model_registries()
        .get(&(service_name.clone(), model_name.clone()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "model `{model_name}` is not registered for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    let versions = world
        .soracloud_model_weight_versions()
        .iter()
        .filter(|((stored_service, stored_model, _version), _record)| {
            stored_service == &service_name && stored_model == &model_name
        })
        .map(|(_key, record)| authoritative_model_weight_version_entry(record))
        .collect::<Vec<_>>();

    Ok(ModelWeightStatusResponse {
        schema_version: MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1,
        model: authoritative_model_weight_status_entry(&service_name, &registry, versions),
    })
}

fn authoritative_model_artifact_status_response(
    app: &SharedAppState,
    service_name: &str,
    model_name: Option<&str>,
    artifact_id: Option<&str>,
    training_job_id: Option<&str>,
    weight_version: Option<&str>,
) -> Result<ModelArtifactStatusResponse, SoracloudError> {
    let service_name: Name = service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let service_name = service_name.to_string();
    let model_name = model_name.map(parse_training_model_name).transpose()?;
    let training_job_id = training_job_id.map(parse_training_job_id).transpose()?;
    let artifact_id = artifact_id
        .map(parse_training_job_id)
        .transpose()?
        .or_else(|| training_job_id.clone());
    let weight_version = weight_version.map(parse_model_weight_version).transpose()?;
    if model_name.is_none() && artifact_id.is_none() && training_job_id.is_none() {
        return Err(SoracloudError::bad_request(
            "model artifact status requires at least one of model_name, artifact_id, or training_job_id",
        ));
    }
    let state_view = app.state.view();
    let world = state_view.world();

    let mut artifacts = world
        .soracloud_model_artifacts()
        .iter()
        .filter(|((stored_service, stored_artifact_id), record)| {
            if stored_service != &service_name {
                return false;
            }
            if let Some(expected_model_name) = model_name.as_ref()
                && &record.model_name != expected_model_name
            {
                return false;
            }
            if let Some(expected_artifact_id) = artifact_id.as_ref()
                && stored_artifact_id != expected_artifact_id
            {
                return false;
            }
            if let Some(expected_training_job_id) = training_job_id.as_ref()
                && &record.training_job_id != expected_training_job_id
            {
                return false;
            }
            if let Some(expected_weight_version) = weight_version.as_ref()
                && record.weight_version.as_deref() != Some(expected_weight_version.as_str())
            {
                return false;
            }
            true
        })
        .map(|(_key, record)| record.clone())
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        right
            .registered_sequence
            .cmp(&left.registered_sequence)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
    });
    let artifact = artifacts.first().cloned().ok_or_else(|| {
        SoracloudError::not_found(format!(
            "model artifact status not found for service `{service_name}` in authoritative Soracloud state"
        ))
    })?;
    let artifact_entries = artifacts
        .iter()
        .map(|entry| authoritative_model_artifact_status_entry(&service_name, entry))
        .collect::<Vec<_>>();

    Ok(ModelArtifactStatusResponse {
        schema_version: MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1,
        service_name: service_name.clone(),
        model_name: artifact.model_name.clone(),
        artifact_count: u32::try_from(artifact_entries.len()).unwrap_or(u32::MAX),
        artifact: authoritative_model_artifact_status_entry(&service_name, &artifact),
        artifacts: artifact_entries,
    })
}

fn authoritative_uploaded_model_status_response(
    app: &SharedAppState,
    service_name: &str,
    model_id: &str,
    weight_version: &str,
) -> Result<UploadedModelStatusResponse, SoracloudError> {
    let service_name: Name = service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let service_name = service_name.to_string();
    let model_id = parse_training_job_id(model_id)?;
    let weight_version = parse_model_weight_version(weight_version)?;
    let state_view = app.state.view();
    let world = state_view.world();

    let bundle = world
        .soracloud_uploaded_model_bundles()
        .get(&(service_name.clone(), model_id.clone(), weight_version.clone()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "uploaded model `{model_id}` version `{weight_version}` not found for service `{service_name}`"
            ))
        })?;
    let mut chunks = world
        .soracloud_uploaded_model_chunks()
        .iter()
        .filter_map(|(_key, chunk)| {
            (chunk.service_name.as_ref() == service_name.as_str()
                && chunk.model_id == model_id
                && chunk.weight_version == weight_version)
                .then(|| chunk.clone())
        })
        .collect::<Vec<_>>();
    chunks.sort_by_key(|chunk| chunk.ordinal);
    let chunk_ordinals = chunks.iter().map(|chunk| chunk.ordinal).collect::<Vec<_>>();
    let compile_profile = world
        .soracloud_private_compile_profiles()
        .get(&bundle.compile_profile_hash)
        .cloned();
    let artifact = world.soracloud_model_artifacts().iter().find_map(
        |((stored_service, _artifact_id), record)| {
            (stored_service == &service_name
                && record.weight_version.as_deref() == Some(weight_version.as_str())
                && record.private_bundle_root == Some(bundle.bundle_root)
                && record
                    .source_provenance
                    .as_ref()
                    .is_some_and(|provenance| provenance.id == model_id))
            .then(|| authoritative_model_artifact_status_entry(&service_name, record))
        },
    );

    Ok(UploadedModelStatusResponse {
        schema_version: UPLOADED_MODEL_STATUS_SCHEMA_VERSION_V1,
        bundle,
        uploaded_chunk_count: u32::try_from(chunk_ordinals.len()).unwrap_or(u32::MAX),
        chunk_ordinals,
        compile_profile,
        artifact,
    })
}

fn authoritative_uploaded_model_status_from_query(
    app: &SharedAppState,
    query: &UploadedModelStatusQuery,
    require_compile_profile: bool,
) -> Result<UploadedModelStatusResponse, SoracloudError> {
    let service_name: Name = query
        .service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let service_name = service_name.to_string();
    let weight_version = parse_model_weight_version(&query.weight_version)?;
    let state_view = app.state.view();
    let world = state_view.world();

    let model_id = if let Some(model_id) = query.model_id.as_deref() {
        let model_id = parse_training_job_id(model_id)?;
        let bundle = world
            .soracloud_uploaded_model_bundles()
            .get(&(service_name.clone(), model_id.clone(), weight_version.clone()))
            .cloned()
            .ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "uploaded model `{model_id}` version `{weight_version}` not found for service `{service_name}`"
                ))
            })?;
        if let Some(bundle_root) = query.bundle_root
            && bundle.bundle_root != bundle_root
        {
            return Err(SoracloudError::conflict(format!(
                "uploaded model `{model_id}` version `{weight_version}` bundle_root does not match query"
            )));
        }
        if let Some(compile_profile_hash) = query.compile_profile_hash
            && bundle.compile_profile_hash != compile_profile_hash
        {
            return Err(SoracloudError::conflict(format!(
                "uploaded model `{model_id}` version `{weight_version}` compile_profile_hash does not match query"
            )));
        }
        model_id
    } else {
        let model_name = query.model_name.as_deref().ok_or_else(|| {
            SoracloudError::bad_request(
                "model_id or model_name must be provided for uploaded model status".to_string(),
            )
        })?;
        let model_name = parse_training_model_name(model_name)?;
        world
            .soracloud_model_artifacts()
            .iter()
            .find_map(|((stored_service, _artifact_id), record)| {
                (stored_service == &service_name
                    && record.model_name == model_name
                    && record.weight_version.as_deref() == Some(weight_version.as_str())
                    && record
                        .source_provenance
                        .as_ref()
                        .is_some_and(|provenance| provenance.kind == SoraModelProvenanceKindV1::UserUpload)
                    && query
                        .bundle_root
                        .is_none_or(|bundle_root| record.private_bundle_root == Some(bundle_root))
                    && query
                        .compile_profile_hash
                        .is_none_or(|compile_profile_hash| record.compile_profile_hash == Some(compile_profile_hash)))
                .then(|| record.source_provenance.as_ref().map(|provenance| provenance.id.clone()))
                .flatten()
            })
            .ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "uploaded model status not found for service `{service_name}`, model `{model_name}`, version `{weight_version}`"
                ))
            })?
    };

    let response = authoritative_uploaded_model_status_response(
        app,
        &service_name,
        &model_id,
        &weight_version,
    )?;
    if require_compile_profile && response.compile_profile.is_none() {
        return Err(SoracloudError::not_found(format!(
            "compile profile not yet admitted for uploaded model `{model_id}` version `{weight_version}`"
        )));
    }
    Ok(response)
}

fn authoritative_private_inference_status_response(
    app: &SharedAppState,
    session_id: &str,
) -> Result<PrivateInferenceStatusResponse, SoracloudError> {
    let session_id = parse_training_job_id(session_id)?;
    let state_view = app.state.view();
    let world = state_view.world();
    let session = world
        .soracloud_private_inference_sessions()
        .iter()
        .find_map(|((_apartment_name, stored_session_id), session)| {
            (stored_session_id == &session_id).then(|| session.clone())
        })
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "private session `{session_id}` not found in authoritative Soracloud state"
            ))
        })?;
    let mut checkpoints = world
        .soracloud_private_inference_checkpoints()
        .iter()
        .filter_map(|((stored_session_id, _step), checkpoint)| {
            (stored_session_id == &session_id).then(|| checkpoint.clone())
        })
        .collect::<Vec<_>>();
    checkpoints.sort_by_key(|checkpoint| checkpoint.step);
    Ok(PrivateInferenceStatusResponse {
        schema_version: PRIVATE_INFERENCE_STATUS_SCHEMA_VERSION_V1,
        session,
        checkpoint_count: u32::try_from(checkpoints.len()).unwrap_or(u32::MAX),
        checkpoints,
    })
}

fn authoritative_hf_shared_lease_status_response(
    app: &SharedAppState,
    repo_id: &str,
    resolved_revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    account_id: Option<&AccountId>,
) -> Result<HfSharedLeaseStatusResponse, SoracloudError> {
    if lease_term_ms == 0 {
        return Err(SoracloudError::bad_request(
            "lease_term_ms must be greater than zero",
        ));
    }

    let source_id = hf_source_id(repo_id, resolved_revision)?;
    let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
    let member_key = account_id.map(|account_id| (pool_id.to_string(), account_id.to_string()));

    let state_view = app.state.view();
    let world = state_view.world();
    let source = world
        .soracloud_hf_sources()
        .get(&source_id)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "hf source `{repo_id}@{resolved_revision}` not found in authoritative Soracloud state"
            ))
        })?;
    let pool = world
        .soracloud_hf_shared_lease_pools()
        .get(&pool_id)
        .cloned();
    let member = member_key.as_ref().and_then(|member_key| {
        world
            .soracloud_hf_shared_lease_members()
            .get(member_key)
            .cloned()
    });
    let placement = world.soracloud_hf_placements().get(&pool_id).cloned();
    let latest_audit_event = world
        .soracloud_hf_shared_lease_audit_events()
        .iter()
        .filter(|(_sequence, event)| event.pool_id == pool_id)
        .map(|(_sequence, event)| event.clone())
        .max_by_key(|event| event.sequence);
    let runtime_projection = authoritative_hf_runtime_projection(app, &source_id);
    let storage_base_fee_nanos = pool.as_ref().map_or(0, |pool| pool.base_fee_nanos);
    let compute_reservation_fee_nanos = placement
        .as_ref()
        .map_or(0, |placement| placement.total_reservation_fee_nanos);
    let eligible_host_count = placement
        .as_ref()
        .map_or(0, |placement| placement.eligible_validator_count);
    let warm_host_count = placement
        .as_ref()
        .map_or(0, |placement| placement.warm_host_count());

    Ok(HfSharedLeaseStatusResponse {
        schema_version: HF_SHARED_LEASE_STATUS_SCHEMA_VERSION_V1,
        source: source.clone(),
        runtime_projection: runtime_projection.clone(),
        pool,
        member,
        placement: placement.clone(),
        latest_audit_event,
        audit_event_count: authoritative_hf_shared_lease_event_count(world, &pool_id),
        storage_base_fee_nanos,
        compute_reservation_fee_nanos,
        eligible_host_count,
        warm_host_count,
        importer_pending: hf_importer_pending(&source, runtime_projection.as_ref()),
    })
}

fn authoritative_model_host_status_response(
    app: &SharedAppState,
    validator_account_id: Option<&AccountId>,
) -> ModelHostStatusResponse {
    let state_view = app.state.view();
    let world = state_view.world();
    let mut hosts = world
        .soracloud_model_host_capabilities()
        .iter()
        .filter_map(|(account_id, capability)| {
            validator_account_id
                .is_none_or(|validator_account_id| account_id == validator_account_id)
                .then(|| capability.clone())
        })
        .collect::<Vec<_>>();
    hosts.sort_by(|left, right| left.validator_account_id.cmp(&right.validator_account_id));
    ModelHostStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        validator_account_id: validator_account_id.cloned(),
        active_host_count: u32::try_from(hosts.len()).unwrap_or(u32::MAX),
        hosts,
    }
}

fn authoritative_training_job_action(action: SoraTrainingJobActionV1) -> TrainingJobAction {
    match action {
        SoraTrainingJobActionV1::Start => TrainingJobAction::Start,
        SoraTrainingJobActionV1::Checkpoint => TrainingJobAction::Checkpoint,
        SoraTrainingJobActionV1::Retry => TrainingJobAction::Retry,
    }
}

fn authoritative_model_weight_action(action: SoraModelWeightActionV1) -> ModelWeightAction {
    match action {
        SoraModelWeightActionV1::Register => ModelWeightAction::Register,
        SoraModelWeightActionV1::Promote => ModelWeightAction::Promote,
        SoraModelWeightActionV1::Rollback => ModelWeightAction::Rollback,
    }
}

fn authoritative_model_artifact_action(action: SoraModelArtifactActionV1) -> ModelArtifactAction {
    match action {
        SoraModelArtifactActionV1::Register => ModelArtifactAction::Register,
    }
}

fn authoritative_hf_shared_lease_event_count(world: &impl WorldReadOnly, pool_id: &Hash) -> u32 {
    u32::try_from(
        world
            .soracloud_hf_shared_lease_audit_events()
            .iter()
            .filter(|(_sequence, event)| event.pool_id == *pool_id)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_hf_runtime_projection(
    app: &SharedAppState,
    source_id: &Hash,
) -> Option<SoracloudRuntimeHfSourcePlan> {
    app.soracloud_runtime
        .as_ref()?
        .snapshot()
        .hf_sources
        .get(&source_id.to_string())
        .cloned()
}

fn hf_importer_pending(
    source: &SoraHfSourceRecordV1,
    runtime_projection: Option<&SoracloudRuntimeHfSourcePlan>,
) -> bool {
    runtime_projection.map_or(
        matches!(source.status, SoraHfSourceStatusV1::PendingImport),
        |projection| {
            matches!(
                projection.runtime_status,
                SoracloudRuntimeHfSourceStatus::PendingImport
                    | SoracloudRuntimeHfSourceStatus::PendingDeployment
                    | SoracloudRuntimeHfSourceStatus::Hydrating
            )
        },
    )
}

fn authoritative_agent_action(action: SoraAgentApartmentActionV1) -> AgentApartmentAction {
    match action {
        SoraAgentApartmentActionV1::Deploy => AgentApartmentAction::Deploy,
        SoraAgentApartmentActionV1::LeaseRenew => AgentApartmentAction::LeaseRenew,
        SoraAgentApartmentActionV1::Restart => AgentApartmentAction::Restart,
        SoraAgentApartmentActionV1::WalletSpendRequested => {
            AgentApartmentAction::WalletSpendRequested
        }
        SoraAgentApartmentActionV1::WalletSpendApproved => {
            AgentApartmentAction::WalletSpendApproved
        }
        SoraAgentApartmentActionV1::PolicyRevoked => AgentApartmentAction::PolicyRevoked,
        SoraAgentApartmentActionV1::MessageEnqueued => AgentApartmentAction::MessageEnqueued,
        SoraAgentApartmentActionV1::MessageAcknowledged => {
            AgentApartmentAction::MessageAcknowledged
        }
        SoraAgentApartmentActionV1::ArtifactAllowed => AgentApartmentAction::ArtifactAllowed,
        SoraAgentApartmentActionV1::AutonomyRunApproved => {
            AgentApartmentAction::AutonomyRunApproved
        }
        SoraAgentApartmentActionV1::AutonomyRunExecuted => {
            AgentApartmentAction::AutonomyRunExecuted
        }
    }
}

fn authoritative_service_deployment_bundle(
    world: &impl WorldReadOnly,
    service_name: &str,
) -> Result<(SoraServiceDeploymentStateV1, SoraDeploymentBundleV1), SoracloudError> {
    let service_id: Name = service_name
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
    let deployment = world
        .soracloud_service_deployments()
        .get(&service_id)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let bundle = world
        .soracloud_service_revisions()
        .get(&(
            deployment.service_name.to_string(),
            deployment.current_service_version.clone(),
        ))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "service `{service_name}` active revision `{}` is missing from authoritative state",
                deployment.current_service_version
            ))
        })?;
    Ok((deployment, bundle))
}

fn authoritative_binding_runtime_summary(
    world: &impl WorldReadOnly,
    service_name: &str,
    binding_name: &str,
) -> (u64, u32) {
    let (total_bytes, key_count) = world
        .soracloud_service_state_entries()
        .iter()
        .filter(|((stored_service, stored_binding, _state_key), _entry)| {
            stored_service == service_name && stored_binding == binding_name
        })
        .fold((0_u64, 0_u32), |(bytes, count), (_key, entry)| {
            (
                bytes.saturating_add(entry.payload_bytes.get()),
                count.saturating_add(1),
            )
        });
    (total_bytes, key_count)
}

fn authoritative_service_event_count(world: &impl WorldReadOnly, service_name: &str) -> u32 {
    u32::try_from(
        world
            .soracloud_service_audit_events()
            .iter()
            .filter(|(_sequence, event)| event.service_name.as_ref() == service_name)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_training_job_event_count(
    world: &impl WorldReadOnly,
    service_name: &str,
    job_id: &str,
) -> u32 {
    u32::try_from(
        world
            .soracloud_training_job_audit_events()
            .iter()
            .filter(|(_sequence, event)| {
                event.service_name.as_ref() == service_name && event.job_id == job_id
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_model_event_count(
    world: &impl WorldReadOnly,
    service_name: &str,
    model_name: &str,
) -> u32 {
    u32::try_from(
        world
            .soracloud_model_weight_audit_events()
            .iter()
            .filter(|(_sequence, event)| {
                event.service_name.as_ref() == service_name && event.model_name == model_name
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_model_version_count(
    world: &impl WorldReadOnly,
    service_name: &str,
    model_name: &str,
) -> u32 {
    u32::try_from(
        world
            .soracloud_model_weight_versions()
            .iter()
            .filter(|((stored_service, stored_model, _version), _record)| {
                stored_service == service_name && stored_model == model_name
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_model_artifact_count(
    world: &impl WorldReadOnly,
    service_name: &str,
    model_name: &str,
) -> u32 {
    u32::try_from(
        world
            .soracloud_model_artifacts()
            .iter()
            .filter(|((_stored_service, _job_id), record)| {
                record.service_name.as_ref() == service_name && record.model_name == model_name
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_agent_event_count(world: &impl WorldReadOnly, apartment_name: &str) -> u32 {
    u32::try_from(
        world
            .soracloud_agent_apartment_audit_events()
            .iter()
            .filter(|(_sequence, event)| event.apartment_name.as_ref() == apartment_name)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn authoritative_agent_current_sequence(app: &SharedAppState) -> u64 {
    authoritative_soracloud_sequence(app)
}

fn authoritative_agent_mutation_response(
    app: &SharedAppState,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> AgentMutationResponse {
    let current_sequence = authoritative_agent_current_sequence(app);
    AgentMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        status: authoritative_agent_runtime_status_for_sequence(record, current_sequence),
        lease_expires_sequence: record.lease_expires_sequence,
        lease_remaining_ticks: record
            .lease_expires_sequence
            .saturating_sub(current_sequence),
        manifest_hash: record.manifest_hash,
        restart_count: record.restart_count,
        pending_wallet_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
        budget_remaining_units: record.autonomy_budget_remaining_units,
        allowlist_count: u32::try_from(record.artifact_allowlist.len()).unwrap_or(u32::MAX),
        run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        process_generation: record.process_generation,
        process_started_sequence: record.process_started_sequence,
        last_active_sequence: record.last_active_sequence,
        last_checkpoint_sequence: record.last_checkpoint_sequence,
        checkpoint_count: record.checkpoint_count,
        persistent_state_total_bytes: record.persistent_state.total_bytes,
        persistent_state_key_count: u32::try_from(record.persistent_state.key_sizes.len())
            .unwrap_or(u32::MAX),
        audit_event_count: 0,
        signed_by: event.signer.to_string(),
        capability: event.capability.clone(),
        reason: event.reason.clone(),
        last_restart_sequence: record.last_restart_sequence,
        last_restart_reason: record.last_restart_reason.clone(),
    }
}

fn authoritative_agent_wallet_mutation_response(
    app: &SharedAppState,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> AgentWalletMutationResponse {
    let current_sequence = authoritative_agent_current_sequence(app);
    let day_bucket = matches!(
        event.action,
        SoraAgentApartmentActionV1::WalletSpendApproved
    )
    .then(|| wallet_day_bucket(event.sequence));
    let day_spent_nanos = match (day_bucket, event.asset_definition.as_deref()) {
        (Some(bucket), Some(asset_definition)) => record
            .wallet_daily_spend
            .get(&format!("{asset_definition}:{bucket}"))
            .map(|entry| entry.spent_nanos),
        _ => None,
    };
    AgentWalletMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        manifest_hash: record.manifest_hash,
        status: authoritative_agent_runtime_status_for_sequence(record, current_sequence),
        request_id: event.request_id.clone(),
        asset_definition: event.asset_definition.clone(),
        amount_nanos: event.amount_nanos,
        day_bucket,
        day_spent_nanos,
        capability: event.capability.clone(),
        reason: event.reason.clone(),
        pending_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
        audit_event_count: 0,
        signed_by: event.signer.to_string(),
    }
}

fn authoritative_agent_mailbox_mutation_response(
    app: &SharedAppState,
    apartment_name: &str,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> Result<AgentMailboxMutationResponse, SoracloudError> {
    let current_sequence = authoritative_agent_current_sequence(app);
    let message_id = event.request_id.clone().ok_or_else(|| {
        SoracloudError::conflict(format!(
            "agent mailbox audit event for apartment `{apartment_name}` is missing message_id"
        ))
    })?;
    let channel = event.channel.clone().ok_or_else(|| {
        SoracloudError::conflict(format!(
            "agent mailbox audit event for apartment `{apartment_name}` is missing channel"
        ))
    })?;
    let payload_hash = event.payload_hash.ok_or_else(|| {
        SoracloudError::conflict(format!(
            "agent mailbox audit event for apartment `{apartment_name}` is missing payload hash"
        ))
    })?;
    Ok(AgentMailboxMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: apartment_name.to_owned(),
        sequence: event.sequence,
        message_id,
        from_apartment: event.from_apartment.clone(),
        to_apartment: event.to_apartment.clone(),
        channel,
        payload_hash,
        status: authoritative_agent_runtime_status_for_sequence(record, current_sequence),
        pending_message_count: u32::try_from(record.mailbox_queue.len()).unwrap_or(u32::MAX),
        audit_event_count: 0,
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_agent_autonomy_mutation_response(
    app: &SharedAppState,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> Result<AgentAutonomyMutationResponse, SoracloudError> {
    let current_sequence = authoritative_agent_current_sequence(app);
    let state_view = app.state.view();
    let world = state_view.world();
    let artifact_hash = event.artifact_hash.clone().ok_or_else(|| {
        SoracloudError::conflict(format!(
            "agent autonomy audit event for apartment `{}` is missing artifact hash",
            record.manifest.apartment_name
        ))
    })?;
    let approved_run = event.run_id.as_ref().and_then(|run_id| {
        record
            .autonomy_run_history
            .iter()
            .find(|run| &run.run_id == run_id)
    });
    let workflow_input_json = approved_run.and_then(|run| run.workflow_input_json.clone());
    let authoritative_runtime_receipt =
        approved_run.and_then(|run| authoritative_agent_runtime_receipt_for_run(world, run));
    let authoritative_execution_audit = approved_run.and_then(|run| {
        authoritative_agent_execution_audit_for_run(
            world,
            record.manifest.apartment_name.as_ref(),
            run,
        )
    });
    Ok(AgentAutonomyMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        status: authoritative_agent_runtime_status_for_sequence(record, current_sequence),
        lease_expires_sequence: record.lease_expires_sequence,
        lease_remaining_ticks: record
            .lease_expires_sequence
            .saturating_sub(current_sequence),
        manifest_hash: record.manifest_hash,
        artifact_hash,
        provenance_hash: event.provenance_hash.clone(),
        run_id: event.run_id.clone(),
        run_label: event.run_label.clone(),
        workflow_input_json,
        budget_units: event.budget_units,
        budget_remaining_units: record.autonomy_budget_remaining_units,
        allowlist_count: u32::try_from(record.artifact_allowlist.len()).unwrap_or(u32::MAX),
        run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        process_generation: record.process_generation,
        process_started_sequence: record.process_started_sequence,
        last_active_sequence: record.last_active_sequence,
        last_checkpoint_sequence: record.last_checkpoint_sequence,
        checkpoint_count: record.checkpoint_count,
        persistent_state_total_bytes: record.persistent_state.total_bytes,
        persistent_state_key_count: u32::try_from(record.persistent_state.key_sizes.len())
            .unwrap_or(u32::MAX),
        audit_event_count: 0,
        signed_by: event.signer.to_string(),
        runtime_execution: None,
        runtime_execution_error: None,
        authoritative_runtime_receipt,
        authoritative_runtime_receipt_error: None,
        authoritative_execution_audit,
        authoritative_execution_audit_error: None,
    })
}

fn authoritative_service_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    expected_action: SoraServiceLifecycleActionV1,
) -> Result<MutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let event = world
        .soracloud_service_audit_events()
        .get(&deployment.process_started_sequence)
        .cloned()
        .filter(|event| {
            event.sequence > baseline.service_max
                && event.action == expected_action
                && event.service_name == deployment.service_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative Soracloud audit event for service `{service_name}` was not observed after mutation"
            ))
        })?;
    let rollout = deployment
        .active_rollout
        .as_ref()
        .or(deployment.last_rollout.as_ref());
    Ok(MutationResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: deployment.service_name.to_string(),
        previous_version: event.from_version,
        current_version: deployment.current_service_version.clone(),
        sequence: event.sequence,
        service_manifest_hash: deployment.current_service_manifest_hash,
        container_manifest_hash: deployment.current_container_manifest_hash,
        revision_count: deployment.revision_count,
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
        rollout_handle: rollout.map(|state| state.rollout_handle.clone()),
        rollout_stage: rollout.map(|state| rollout_stage_to_control_plane_stage(state.stage)),
        rollout_percent: rollout.map(|state| state.traffic_percent),
    })
}

fn authoritative_rollout_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    requested_rollout_handle: &str,
    governance_tx_hash: Hash,
) -> Result<RolloutResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let rollout = deployment
        .active_rollout
        .as_ref()
        .or(deployment.last_rollout.as_ref())
        .filter(|state| state.rollout_handle == requested_rollout_handle)
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "rollout `{requested_rollout_handle}` not found for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    let event = world
        .soracloud_service_audit_events()
        .get(&rollout.updated_sequence)
        .cloned()
        .filter(|event| {
            event.sequence > baseline.service_max
                && event.action == SoraServiceLifecycleActionV1::Rollout
                && event.service_name == deployment.service_name
                && event.rollout_handle.as_deref() == Some(requested_rollout_handle)
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative rollout audit event for service `{service_name}` was not observed after mutation"
            ))
        })?;
    Ok(RolloutResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: deployment.service_name.to_string(),
        rollout_handle: rollout.rollout_handle.clone(),
        stage: rollout_stage_to_control_plane_stage(rollout.stage),
        current_version: deployment.current_service_version.clone(),
        traffic_percent: rollout.traffic_percent,
        health_failures: rollout.health_failures,
        max_health_failures: rollout.max_health_failures,
        sequence: event.sequence,
        governance_tx_hash: event.governance_tx_hash.unwrap_or(governance_tx_hash),
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_state_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    binding_name: &str,
    key: &str,
    operation: StateMutationOperation,
) -> Result<StateMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let event = latest_service_audit_event_after(world, baseline.service_max, |event| {
        event.service_name.as_ref() == service_name
            && event.action == SoraServiceLifecycleActionV1::StateMutation
            && event.binding_name.as_ref().is_some_and(|name| name.as_ref() == binding_name)
            && event.state_key.as_deref() == Some(key)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative Soracloud state mutation event for `{service_name}`/`{binding_name}`/`{key}` was not observed after mutation"
        ))
    })?;
    let (binding_total_bytes, binding_key_count) =
        authoritative_binding_runtime_summary(world, service_name, binding_name);
    Ok(StateMutationResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: service_name.to_owned(),
        binding_name: binding_name.to_owned(),
        key: key.to_owned(),
        operation,
        sequence: event.sequence,
        governance_tx_hash: event.governance_tx_hash.ok_or_else(|| {
            SoracloudError::conflict(format!(
                "state mutation audit event for `{service_name}`/`{binding_name}`/`{key}` is missing governance_tx_hash"
            ))
        })?,
        current_version: deployment.current_service_version,
        binding_total_bytes,
        binding_key_count,
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_service_config_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    config_name: &str,
    operation: ServiceMaterialMutationOperation,
) -> Result<ServiceConfigMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let event = latest_service_audit_event_after(world, baseline.service_max, |event| {
        event.service_name.as_ref() == service_name
            && event.action == SoraServiceLifecycleActionV1::ConfigMutation
            && event.config_name.as_deref() == Some(config_name)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative Soracloud config mutation event for `{service_name}`/`{config_name}` was not observed after mutation"
        ))
    })?;
    let value_hash = deployment
        .service_configs
        .get(config_name)
        .map(|entry| entry.value_hash);
    Ok(ServiceConfigMutationResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: service_name.to_owned(),
        config_name: config_name.to_owned(),
        operation,
        sequence: event.sequence,
        current_version: deployment.current_service_version,
        config_generation: deployment.config_generation,
        config_entry_count: u32::try_from(deployment.service_configs.len()).unwrap_or(u32::MAX),
        value_hash,
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn service_secret_status_entry(entry: &SoraServiceSecretEntryV1) -> ServiceSecretStatusEntry {
    ServiceSecretStatusEntry {
        secret_name: entry.secret_name.clone(),
        encryption: entry.envelope.encryption,
        key_id: entry.envelope.key_id.clone(),
        key_version: entry.envelope.key_version.get(),
        commitment: entry.envelope.commitment,
        ciphertext_bytes: u64::try_from(entry.envelope.ciphertext.len()).unwrap_or(u64::MAX),
        last_update_sequence: entry.last_update_sequence,
    }
}

fn authoritative_service_secret_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    secret_name: &str,
    operation: ServiceMaterialMutationOperation,
) -> Result<ServiceSecretMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let event = latest_service_audit_event_after(world, baseline.service_max, |event| {
        event.service_name.as_ref() == service_name
            && event.action == SoraServiceLifecycleActionV1::SecretMutation
            && event.secret_name.as_deref() == Some(secret_name)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative Soracloud secret mutation event for `{service_name}`/`{secret_name}` was not observed after mutation"
        ))
    })?;
    let secret_entry = deployment.service_secrets.get(secret_name);
    Ok(ServiceSecretMutationResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: service_name.to_owned(),
        secret_name: secret_name.to_owned(),
        operation,
        sequence: event.sequence,
        current_version: deployment.current_service_version,
        secret_generation: deployment.secret_generation,
        secret_entry_count: u32::try_from(deployment.service_secrets.len()).unwrap_or(u32::MAX),
        encryption: secret_entry.map(|entry| entry.envelope.encryption),
        key_id: secret_entry.map(|entry| entry.envelope.key_id.clone()),
        key_version: secret_entry.map(|entry| entry.envelope.key_version.get()),
        commitment: secret_entry.map(|entry| entry.envelope.commitment),
        ciphertext_bytes: secret_entry
            .map(|entry| u64::try_from(entry.envelope.ciphertext.len()).unwrap_or(u64::MAX)),
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_service_config_status_response(
    app: &SharedAppState,
    service_name: &str,
    config_name: Option<&str>,
) -> Result<ServiceConfigStatusResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let configs = deployment
        .service_configs
        .values()
        .filter(|entry| config_name.is_none_or(|filter| filter == entry.config_name.as_str()))
        .map(|entry| {
            let value_json = entry
                .value_json
                .try_into_any_norito::<norito::json::Value>()
                .map_err(|err| {
                    SoracloudError::internal(format!(
                        "failed to decode authoritative service config json: {err}"
                    ))
                })?;
            Ok(ServiceConfigStatusEntry {
                config_name: entry.config_name.clone(),
                value_hash: entry.value_hash,
                value_json,
                last_update_sequence: entry.last_update_sequence,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if config_name.is_some() && configs.is_empty() {
        return Err(SoracloudError::not_found(format!(
            "service config `{}` not found for service `{service_name}`",
            config_name.unwrap_or_default()
        )));
    }
    Ok(ServiceConfigStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        service_name: deployment.service_name.to_string(),
        current_version: deployment.current_service_version,
        config_generation: deployment.config_generation,
        config_entry_count: u32::try_from(configs.len()).unwrap_or(u32::MAX),
        configs,
    })
}

fn authoritative_service_secret_status_response(
    app: &SharedAppState,
    service_name: &str,
    secret_name: Option<&str>,
) -> Result<ServiceSecretStatusResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let secrets = deployment
        .service_secrets
        .values()
        .filter(|entry| secret_name.is_none_or(|filter| filter == entry.secret_name.as_str()))
        .map(service_secret_status_entry)
        .collect::<Vec<_>>();
    if secret_name.is_some() && secrets.is_empty() {
        return Err(SoracloudError::not_found(format!(
            "service secret `{}` not found for service `{service_name}`",
            secret_name.unwrap_or_default()
        )));
    }
    Ok(ServiceSecretStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        service_name: deployment.service_name.to_string(),
        current_version: deployment.current_service_version,
        secret_generation: deployment.secret_generation,
        secret_entry_count: u32::try_from(secrets.len()).unwrap_or(u32::MAX),
        secrets,
    })
}

fn authoritative_fhe_job_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    binding_name: &str,
    job: &FheJobSpecV1,
    governance_tx_hash: Hash,
) -> Result<FheJobRunResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _bundle) = authoritative_service_deployment_bundle(world, service_name)?;
    let event = latest_service_audit_event_after(world, baseline.service_max, |event| {
        event.service_name.as_ref() == service_name
            && event.action == SoraServiceLifecycleActionV1::FheJobRun
            && event.binding_name.as_ref().is_some_and(|name| name.as_ref() == binding_name)
            && event.state_key.as_deref() == Some(job.output_state_key.as_str())
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative FHE audit event for service `{service_name}` job `{}` was not observed after mutation",
            job.job_id
        ))
    })?;
    let entry = world
        .soracloud_service_state_entries()
        .get(&(
            service_name.to_owned(),
            binding_name.to_owned(),
            job.output_state_key.clone(),
        ))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative ciphertext state for service `{service_name}` output `{}` is missing after FHE job application",
                job.output_state_key
            ))
        })?;
    let (binding_total_bytes, binding_key_count) =
        authoritative_binding_runtime_summary(world, service_name, binding_name);
    Ok(FheJobRunResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: service_name.to_owned(),
        binding_name: binding_name.to_owned(),
        job_id: job.job_id.clone(),
        operation: job.operation,
        sequence: event.sequence,
        governance_tx_hash: event.governance_tx_hash.unwrap_or(governance_tx_hash),
        output_state_key: job.output_state_key.clone(),
        output_payload_bytes: entry.payload_bytes.get(),
        output_commitment: entry.payload_commitment,
        current_version: deployment.current_service_version,
        binding_total_bytes,
        binding_key_count,
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_decryption_request_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    request_id: &str,
) -> Result<DecryptionRequestResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_decryption_request_records()
        .get(&(service_name.to_owned(), request_id.to_owned()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "decryption request `{request_id}` not found for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    if record.sequence <= baseline.service_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative decryption request `{request_id}` for service `{service_name}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_service_audit_events()
        .get(&record.sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraServiceLifecycleActionV1::DecryptionRequest
                && event.service_name.as_ref() == service_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "decryption request audit event `{}` for service `{service_name}` is missing from authoritative state",
                record.sequence
            ))
        })?;
    Ok(DecryptionRequestResponse {
        action: audit_action_to_control_plane_action(event.action),
        service_name: service_name.to_owned(),
        policy_name: record.request.policy_name.clone(),
        request_id: record.request.request_id.clone(),
        binding_name: record.request.binding_name.clone(),
        state_key: record.request.state_key.clone(),
        jurisdiction_tag: record.request.jurisdiction_tag.clone(),
        policy_snapshot_hash: record.policy_snapshot_hash(),
        consent_evidence_hash: record.request.consent_evidence_hash,
        break_glass: record.request.break_glass,
        break_glass_reason: record.request.break_glass_reason.clone(),
        sequence: record.sequence,
        governance_tx_hash: record.request.governance_tx_hash,
        current_version: record.service_version.clone(),
        audit_event_count: authoritative_service_event_count(world, service_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_training_job_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    job_id: &str,
    expected_action: SoraTrainingJobActionV1,
) -> Result<TrainingJobMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_training_jobs()
        .get(&(service_name.to_owned(), job_id.to_owned()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "training job `{job_id}` not found for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    if record.updated_sequence <= baseline.training_job_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative training job `{job_id}` for service `{service_name}` was not updated by the submitted mutation"
        )));
    }
    let event = world
        .soracloud_training_job_audit_events()
        .get(&record.updated_sequence)
        .cloned()
        .filter(|event| {
            event.action == expected_action
                && event.service_name.as_ref() == service_name
                && event.job_id == job_id
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "training job audit event `{}` for service `{service_name}` job `{job_id}` is missing from authoritative state",
                record.updated_sequence
            ))
        })?;
    Ok(TrainingJobMutationResponse {
        action: authoritative_training_job_action(event.action),
        service_name: service_name.to_owned(),
        model_name: record.model_name.clone(),
        job_id: record.job_id.clone(),
        sequence: event.sequence,
        status: authoritative_training_job_status(record.status),
        worker_group_size: record.worker_group_size,
        target_steps: record.target_steps,
        completed_steps: record.completed_steps,
        checkpoint_interval_steps: record.checkpoint_interval_steps,
        last_checkpoint_step: record.last_checkpoint_step,
        checkpoint_count: record.checkpoint_count,
        retry_count: record.retry_count,
        max_retries: record.max_retries,
        step_compute_units: record.step_compute_units,
        compute_budget_units: record.compute_budget_units,
        compute_consumed_units: record.compute_consumed_units,
        compute_remaining_units: record
            .compute_budget_units
            .saturating_sub(record.compute_consumed_units),
        storage_budget_bytes: record.storage_budget_bytes,
        storage_consumed_bytes: record.storage_consumed_bytes,
        storage_remaining_bytes: record
            .storage_budget_bytes
            .saturating_sub(record.storage_consumed_bytes),
        latest_metrics_hash: record.latest_metrics_hash,
        last_failure_reason: record.last_failure_reason.clone(),
        training_event_count: authoritative_training_job_event_count(world, service_name, job_id),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_model_weight_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    model_name: &str,
    target_version: &str,
    expected_action: SoraModelWeightActionV1,
) -> Result<ModelWeightMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let registry = world
        .soracloud_model_registries()
        .get(&(service_name.to_owned(), model_name.to_owned()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "model `{model_name}` is not registered for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    let weight_record = world
        .soracloud_model_weight_versions()
        .get(&(
            service_name.to_owned(),
            model_name.to_owned(),
            target_version.to_owned(),
        ))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "weight version `{target_version}` not found for model `{model_name}` in authoritative Soracloud state"
            ))
        })?;
    let event_sequence = match expected_action {
        SoraModelWeightActionV1::Register => weight_record.registered_sequence,
        SoraModelWeightActionV1::Promote => weight_record.promoted_sequence.ok_or_else(|| {
            SoracloudError::conflict(format!(
                "weight version `{target_version}` for model `{model_name}` has not been promoted in authoritative Soracloud state"
            ))
        })?,
        SoraModelWeightActionV1::Rollback => registry.updated_sequence,
    };
    if event_sequence <= baseline.model_weight_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative model-weight event for service `{service_name}` model `{model_name}` target `{target_version}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_model_weight_audit_events()
        .get(&event_sequence)
        .cloned()
        .filter(|event| {
            event.action == expected_action
                && event.service_name.as_ref() == service_name
                && event.model_name == model_name
                && event.target_version == target_version
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "model-weight audit event `{event_sequence}` for service `{service_name}` model `{model_name}` target `{target_version}` is missing from authoritative state"
            ))
        })?;
    Ok(ModelWeightMutationResponse {
        action: authoritative_model_weight_action(event.action),
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        target_version: target_version.to_owned(),
        current_version: registry.current_version.clone(),
        parent_version: weight_record.parent_version.clone(),
        sequence: event.sequence,
        version_count: authoritative_model_version_count(world, service_name, model_name),
        model_event_count: authoritative_model_event_count(world, service_name, model_name),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_model_artifact_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    training_job_id: &str,
) -> Result<ModelArtifactMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let artifact = world
        .soracloud_model_artifacts()
        .get(&(service_name.to_owned(), training_job_id.to_owned()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "artifact metadata for training job `{training_job_id}` not found for service `{service_name}` in authoritative Soracloud state"
            ))
        })?;
    if artifact.registered_sequence <= baseline.model_artifact_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative model-artifact event for service `{service_name}` training job `{training_job_id}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_model_artifact_audit_events()
        .get(&artifact.registered_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraModelArtifactActionV1::Register
                && event.service_name.as_ref() == service_name
                && event.training_job_id == training_job_id
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "model-artifact audit event `{}` for service `{service_name}` training job `{training_job_id}` is missing from authoritative state",
                artifact.registered_sequence
            ))
        })?;
    Ok(ModelArtifactMutationResponse {
        action: authoritative_model_artifact_action(event.action),
        service_name: service_name.to_owned(),
        model_name: artifact.model_name.clone(),
        training_job_id: training_job_id.to_owned(),
        artifact_id: artifact.artifact_id.clone(),
        weight_version: artifact.weight_version.clone(),
        sequence: artifact.registered_sequence,
        model_artifact_count: authoritative_model_artifact_count(
            world,
            service_name,
            &artifact.model_name,
        ),
        signed_by: event.signer.to_string(),
    })
}

fn authoritative_hf_shared_lease_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    repo_id: &str,
    resolved_revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    account_id: &AccountId,
    service_name: Option<&str>,
    apartment_name: Option<&str>,
) -> Result<HfSharedLeaseMutationResponse, SoracloudError> {
    if lease_term_ms == 0 {
        return Err(SoracloudError::bad_request(
            "lease_term_ms must be greater than zero",
        ));
    }

    let source_id = hf_source_id(repo_id, resolved_revision)?;
    let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)?;
    let member_key = (pool_id.to_string(), account_id.to_string());

    let state_view = app.state.view();
    let world = state_view.world();
    let source = world
        .soracloud_hf_sources()
        .get(&source_id)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "hf source `{repo_id}@{resolved_revision}` not found in authoritative Soracloud state"
            ))
        })?;
    let pool = world
        .soracloud_hf_shared_lease_pools()
        .get(&pool_id)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "hf shared lease pool for `{repo_id}@{resolved_revision}` is missing from authoritative Soracloud state"
            ))
        })?;
    let member = world
        .soracloud_hf_shared_lease_members()
        .get(&member_key)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "hf shared lease membership for account `{account_id}` in pool `{pool_id}` is missing from authoritative state"
            ))
        })?;
    let event = latest_hf_shared_lease_audit_event_after(world, baseline.hf_shared_lease_max, |event| {
        event.pool_id == pool_id
            && event.account_id == *account_id
            && service_name.is_none_or(|service_name| event.service_name.as_deref() == Some(service_name))
            && apartment_name.is_none_or(|apartment_name| {
                event.apartment_name.as_deref() == Some(apartment_name)
            })
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative hf shared-lease mutation for `{repo_id}@{resolved_revision}` account `{account_id}` was not observed after mutation"
        ))
    })?;
    let runtime_projection = authoritative_hf_runtime_projection(app, &source_id);
    let active_placement = world.soracloud_hf_placements().get(&pool_id).cloned();
    let placement = if event.action == SoraHfSharedLeaseActionV1::Renew {
        pool.queued_next_window
            .as_ref()
            .filter(|next_window| {
                next_window.sponsor_account_id == *account_id
                    && event.lease_expires_at_ms == next_window.window_expires_at_ms
            })
            .map(|next_window| next_window.planned_placement.clone())
            .or(active_placement.clone())
    } else {
        active_placement.clone()
    };

    let storage_base_fee_nanos = if event.action == SoraHfSharedLeaseActionV1::Renew {
        pool.queued_next_window
            .as_ref()
            .filter(|next_window| {
                next_window.sponsor_account_id == *account_id
                    && event.lease_expires_at_ms == next_window.window_expires_at_ms
            })
            .map_or(pool.base_fee_nanos, |next_window| {
                next_window.base_fee_nanos
            })
    } else {
        pool.base_fee_nanos
    };

    Ok(HfSharedLeaseMutationResponse {
        schema_version: HF_SHARED_LEASE_STATUS_SCHEMA_VERSION_V1,
        action: event.action,
        source: source.clone(),
        runtime_projection: runtime_projection.clone(),
        pool: pool.clone(),
        member,
        placement: placement.clone(),
        latest_audit_event: Some(event),
        storage_base_fee_nanos,
        compute_reservation_fee_nanos: placement
            .as_ref()
            .map_or(0, |placement| placement.total_reservation_fee_nanos),
        eligible_host_count: placement
            .as_ref()
            .map_or(0, |placement| placement.eligible_validator_count),
        warm_host_count: placement
            .as_ref()
            .map_or(0, |placement| placement.warm_host_count()),
        importer_pending: hf_importer_pending(&source, runtime_projection.as_ref()),
    })
}

fn authoritative_agent_deploy_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
) -> Result<AgentMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    if record.deployed_sequence <= baseline.agent_apartment_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative agent deploy event for apartment `{apartment_name}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&record.deployed_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::Deploy
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "agent deploy audit event `{}` for apartment `{apartment_name}` is missing from authoritative state",
                record.deployed_sequence
            ))
        })?;
    let mut response = authoritative_agent_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_lease_renew_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
) -> Result<AgentMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    if record.last_renewed_sequence <= baseline.agent_apartment_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative lease-renew event for apartment `{apartment_name}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&record.last_renewed_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::LeaseRenew
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "lease-renew audit event `{}` for apartment `{apartment_name}` is missing from authoritative state",
                record.last_renewed_sequence
            ))
        })?;
    let mut response = authoritative_agent_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_restart_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
) -> Result<AgentMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let restart_sequence = record.last_restart_sequence.ok_or_else(|| {
        SoracloudError::conflict(format!(
            "apartment `{apartment_name}` does not have an authoritative restart sequence after mutation"
        ))
    })?;
    if restart_sequence <= baseline.agent_apartment_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative restart event for apartment `{apartment_name}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&restart_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::Restart
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "restart audit event `{restart_sequence}` for apartment `{apartment_name}` is missing from authoritative state"
            ))
        })?;
    let mut response = authoritative_agent_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_policy_revoke_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    capability: &str,
) -> Result<AgentMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let event = latest_agent_apartment_audit_event_after(world, baseline.agent_apartment_max, |event| {
        event.apartment_name.as_ref() == apartment_name
            && event.action == SoraAgentApartmentActionV1::PolicyRevoked
            && event.capability.as_deref() == Some(capability)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative policy-revoke event for apartment `{apartment_name}` capability `{capability}` was not observed after mutation"
        ))
    })?;
    let mut response = authoritative_agent_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_wallet_request_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    asset_definition: &str,
    amount_nanos: u64,
) -> Result<AgentWalletMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let pending_sequence = record
        .pending_wallet_requests
        .values()
        .find(|request| {
            request.created_sequence > baseline.agent_apartment_max
                && request.asset_definition == asset_definition
                && request.amount_nanos == amount_nanos
        })
        .map(|request| request.created_sequence);
    let event = match pending_sequence {
        Some(sequence) => world
            .soracloud_agent_apartment_audit_events()
            .get(&sequence)
            .cloned()
            .filter(|event| {
                event.action == SoraAgentApartmentActionV1::WalletSpendRequested
                    && event.apartment_name.as_ref() == apartment_name
                    && event.asset_definition.as_deref() == Some(asset_definition)
                    && event.amount_nanos == Some(amount_nanos)
            }),
        None => latest_agent_apartment_audit_event_after(world, baseline.agent_apartment_max, |event| {
            event.apartment_name.as_ref() == apartment_name
                && matches!(
                    event.action,
                    SoraAgentApartmentActionV1::WalletSpendRequested
                        | SoraAgentApartmentActionV1::WalletSpendApproved
                )
                && event.asset_definition.as_deref() == Some(asset_definition)
                && event.amount_nanos == Some(amount_nanos)
        })
        .cloned(),
    }
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative wallet-spend event for apartment `{apartment_name}` asset `{asset_definition}` amount `{amount_nanos}` was not observed after mutation"
        ))
    })?;
    let mut response = authoritative_agent_wallet_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_wallet_approve_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    request_id: &str,
) -> Result<AgentWalletMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let event = latest_agent_apartment_audit_event_after(world, baseline.agent_apartment_max, |event| {
        event.apartment_name.as_ref() == apartment_name
            && event.action == SoraAgentApartmentActionV1::WalletSpendApproved
            && event.request_id.as_deref() == Some(request_id)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative wallet-approve event for apartment `{apartment_name}` request `{request_id}` was not observed after mutation"
        ))
    })?;
    let mut response = authoritative_agent_wallet_mutation_response(app, &record, &event);
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_message_send_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    from_apartment: &str,
    to_apartment: &str,
    channel: &str,
    payload: &str,
) -> Result<AgentMailboxMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let recipient = world
        .soracloud_agent_apartments()
        .get(to_apartment)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{to_apartment}` not found in authoritative Soracloud state"
            ))
        })?;
    let normalized_channel = channel.trim();
    let payload_hash = Hash::new(payload.trim().as_bytes());
    let message = recipient
        .mailbox_queue
        .iter()
        .find(|message| {
            message.enqueued_sequence > baseline.agent_apartment_max
                && message.from_apartment == from_apartment
                && message.channel == normalized_channel
                && message.payload_hash == payload_hash
        })
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative mailbox message for `{from_apartment}` -> `{to_apartment}` on channel `{normalized_channel}` was not observed after mutation"
            ))
        })?;
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&message.enqueued_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::MessageEnqueued
                && event.apartment_name.as_ref() == to_apartment
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "mailbox enqueue audit event `{}` for apartment `{to_apartment}` is missing from authoritative state",
                message.enqueued_sequence
            ))
        })?;
    let mut response =
        authoritative_agent_mailbox_mutation_response(app, to_apartment, &recipient, &event)?;
    response.audit_event_count = authoritative_agent_event_count(world, to_apartment);
    Ok(response)
}

fn authoritative_agent_message_ack_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    message_id: &str,
) -> Result<AgentMailboxMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let event = latest_agent_apartment_audit_event_after(world, baseline.agent_apartment_max, |event| {
        event.apartment_name.as_ref() == apartment_name
            && event.action == SoraAgentApartmentActionV1::MessageAcknowledged
            && event.request_id.as_deref() == Some(message_id)
    })
    .cloned()
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative mailbox-ack event for apartment `{apartment_name}` message `{message_id}` was not observed after mutation"
        ))
    })?;
    let mut response =
        authoritative_agent_mailbox_mutation_response(app, apartment_name, &record, &event)?;
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_artifact_allow_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
) -> Result<AgentAutonomyMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let rule = record
        .artifact_allowlist
        .get(artifact_hash)
        .cloned()
        .filter(|rule| {
            rule.added_sequence > baseline.agent_apartment_max
                && rule.provenance_hash.as_deref() == provenance_hash
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative artifact-allow event for apartment `{apartment_name}` artifact `{artifact_hash}` was not observed after mutation"
            ))
        })?;
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&rule.added_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::ArtifactAllowed
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "artifact-allow audit event `{}` for apartment `{apartment_name}` is missing from authoritative state",
                rule.added_sequence
            ))
        })?;
    let mut response = authoritative_agent_autonomy_mutation_response(app, &record, &event)?;
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_agent_autonomy_run_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    run_label: &str,
) -> Result<AgentAutonomyMutationResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let run = record
        .autonomy_run_history
        .iter()
        .rev()
        .find(|run| {
            run.approved_sequence > baseline.agent_apartment_max
                && run.artifact_hash == artifact_hash
                && run.provenance_hash.as_deref() == provenance_hash
                && run.run_label == run_label
        })
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative autonomy-run event for apartment `{apartment_name}` artifact `{artifact_hash}` label `{run_label}` was not observed after mutation"
            ))
        })?;
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&run.approved_sequence)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::AutonomyRunApproved
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "autonomy-run audit event `{}` for apartment `{apartment_name}` is missing from authoritative state",
                run.approved_sequence
            ))
        })?;
    let mut response = authoritative_agent_autonomy_mutation_response(app, &record, &event)?;
    response.audit_event_count = authoritative_agent_event_count(world, apartment_name);
    Ok(response)
}

fn authoritative_soracloud_sequence(app: &SharedAppState) -> u64 {
    let state_view = app.state.view();
    let world = state_view.world();
    [
        world
            .soracloud_service_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        world
            .soracloud_training_job_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        world
            .soracloud_model_weight_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        world
            .soracloud_model_artifact_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        world
            .soracloud_hf_shared_lease_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
        world
            .soracloud_agent_apartment_audit_events()
            .iter()
            .map(|(sequence, _event)| *sequence)
            .max()
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
    .saturating_add(1)
}

fn authoritative_agent_runtime_status(status: SoraAgentRuntimeStatusV1) -> AgentRuntimeStatus {
    match status {
        SoraAgentRuntimeStatusV1::Running => AgentRuntimeStatus::Running,
        SoraAgentRuntimeStatusV1::LeaseExpired => AgentRuntimeStatus::LeaseExpired,
    }
}

fn authoritative_agent_runtime_status_for_sequence(
    record: &SoraAgentApartmentRecordV1,
    sequence: u64,
) -> AgentRuntimeStatus {
    authoritative_agent_runtime_status(if sequence >= record.lease_expires_sequence {
        SoraAgentRuntimeStatusV1::LeaseExpired
    } else {
        record.status
    })
}

fn authoritative_agent_mailbox_message_entry(
    message: &SoraAgentMailboxMessageV1,
) -> AgentMailboxMessageEntry {
    AgentMailboxMessageEntry {
        message_id: message.message_id.clone(),
        from_apartment: message.from_apartment.clone(),
        channel: message.channel.clone(),
        payload: message.payload.clone(),
        payload_hash: message.payload_hash,
        enqueued_sequence: message.enqueued_sequence,
    }
}

fn authoritative_agent_allowlist_entry(
    rule: &SoraAgentArtifactAllowRuleV1,
) -> AgentAutonomyAllowlistEntry {
    AgentAutonomyAllowlistEntry {
        artifact_hash: rule.artifact_hash.clone(),
        provenance_hash: rule.provenance_hash.clone(),
        added_sequence: rule.added_sequence,
    }
}

fn authoritative_agent_runtime_receipt_record(
    receipt: &SoraRuntimeReceiptV1,
) -> AgentRuntimeReceiptRecord {
    AgentRuntimeReceiptRecord {
        receipt_id: receipt.receipt_id,
        service_name: receipt.service_name.to_string(),
        service_version: receipt.service_version.clone(),
        handler_name: receipt.handler_name.to_string(),
        handler_class: receipt.handler_class,
        request_commitment: receipt.request_commitment,
        result_commitment: receipt.result_commitment,
        certified_by: receipt.certified_by,
        emitted_sequence: receipt.emitted_sequence,
        placement_id: receipt.placement_id,
        selected_validator_account_id: receipt.selected_validator_account_id.clone(),
        selected_peer_id: receipt.selected_peer_id.clone(),
        journal_artifact_hash: receipt.journal_artifact_hash,
        checkpoint_artifact_hash: receipt.checkpoint_artifact_hash,
    }
}

fn authoritative_agent_execution_audit_record(
    event: &SoraAgentApartmentAuditEventV1,
) -> AgentAutonomyExecutionAuditRecord {
    AgentAutonomyExecutionAuditRecord {
        sequence: event.sequence,
        succeeded: event.succeeded.unwrap_or(false),
        result_commitment: event
            .result_commitment
            .expect("execution audit records must carry result commitments"),
        service_name: event.service_name.clone(),
        service_version: event.service_version.clone(),
        handler_name: event.handler_name.clone(),
        runtime_receipt_id: event.runtime_receipt_id,
        journal_artifact_hash: event.journal_artifact_hash,
        checkpoint_artifact_hash: event.checkpoint_artifact_hash,
        reason: event.reason.clone(),
    }
}

fn authoritative_uploaded_model_encryption_recipient(
    recipient: SoracloudUploadedModelEncryptionRecipient,
) -> SoraUploadedModelEncryptionRecipientV1 {
    SoraUploadedModelEncryptionRecipientV1 {
        schema_version: recipient.schema_version,
        key_id: recipient.key_id,
        key_version: recipient.key_version,
        kem: recipient.kem,
        aead: recipient.aead,
        public_key_bytes: recipient.public_key_bytes,
        public_key_fingerprint: recipient.public_key_fingerprint,
    }
}

fn authoritative_agent_runtime_receipt_for_run(
    world: &impl WorldReadOnly,
    run: &SoraAgentAutonomyRunRecordV1,
) -> Option<AgentRuntimeReceiptRecord> {
    world
        .soracloud_runtime_receipts()
        .iter()
        .filter_map(|(_receipt_id, receipt)| {
            (receipt.request_commitment == run.request_commitment).then_some(receipt)
        })
        .max_by(|left, right| {
            left.emitted_sequence
                .cmp(&right.emitted_sequence)
                .then_with(|| left.receipt_id.cmp(&right.receipt_id))
        })
        .map(authoritative_agent_runtime_receipt_record)
}

fn authoritative_agent_execution_audit_for_run(
    world: &impl WorldReadOnly,
    apartment_name: &str,
    run: &SoraAgentAutonomyRunRecordV1,
) -> Option<AgentAutonomyExecutionAuditRecord> {
    world
        .soracloud_agent_apartment_audit_events()
        .iter()
        .filter_map(|(_sequence, event)| {
            (event.action == SoraAgentApartmentActionV1::AutonomyRunExecuted
                && event.apartment_name.as_ref() == apartment_name
                && event.run_id.as_deref() == Some(run.run_id.as_str()))
            .then_some(event)
        })
        .max_by_key(|event| event.sequence)
        .map(authoritative_agent_execution_audit_record)
}

fn authoritative_agent_run_record(
    world: &impl WorldReadOnly,
    apartment_name: &str,
    record: &SoraAgentAutonomyRunRecordV1,
) -> AgentAutonomyRunRecord {
    AgentAutonomyRunRecord {
        run_id: record.run_id.clone(),
        artifact_hash: record.artifact_hash.clone(),
        provenance_hash: record.provenance_hash.clone(),
        budget_units: record.budget_units,
        run_label: record.run_label.clone(),
        workflow_input_json: record.workflow_input_json.clone(),
        approved_sequence: record.approved_sequence,
        authoritative_runtime_receipt: authoritative_agent_runtime_receipt_for_run(world, record),
        authoritative_execution_audit: authoritative_agent_execution_audit_for_run(
            world,
            apartment_name,
            record,
        ),
    }
}

fn authoritative_agent_runtime_recent_runs(
    app: &SharedAppState,
    apartment_name: &str,
) -> Vec<AgentRuntimeExecutionSummary> {
    let Some(runtime) = app.soracloud_runtime.as_ref() else {
        return Vec::new();
    };
    let state_view = app.state.view();
    let Some(record) = state_view
        .world()
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
    else {
        return Vec::new();
    };
    record
        .autonomy_run_history
        .iter()
        .rev()
        .take(AGENT_AUTONOMY_RECENT_RUN_LIMIT)
        .filter_map(|run| {
            read_agent_runtime_execution_summary(
                runtime.state_dir().as_path(),
                apartment_name,
                &run.run_id,
            )
            .ok()
            .flatten()
        })
        .collect()
}

fn read_agent_runtime_execution_summary(
    state_dir: &std::path::Path,
    apartment_name: &str,
    run_id: &str,
) -> Result<Option<AgentRuntimeExecutionSummary>, SoracloudError> {
    let summary_path = state_dir
        .join("apartments")
        .join(sanitize_runtime_path_component(apartment_name))
        .join("runs")
        .join(sanitize_runtime_path_component(run_id))
        .join("execution_summary.json");
    let summary_bytes = match fs::read(&summary_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(SoracloudError::internal(format!(
                "failed to read runtime execution summary {}: {error}",
                summary_path.display()
            )));
        }
    };
    let summary =
        norito::json::from_slice::<SoracloudApartmentAutonomyExecutionSummaryV1>(&summary_bytes)
            .map_err(|error| {
                SoracloudError::internal(format!(
                    "failed to decode runtime execution summary {}: {error}",
                    summary_path.display()
                ))
            })?;
    Ok(Some(AgentRuntimeExecutionSummary {
        apartment_name: summary.apartment_name,
        run_id: summary.run_id,
        service_name: summary.service_name,
        service_version: summary.service_version,
        handler_name: summary.handler_name,
        succeeded: summary.succeeded,
        result_commitment: summary.result_commitment,
        journal_artifact_hash: Hash::new(&summary_bytes),
        checkpoint_artifact_hash: summary.checkpoint_artifact_hash,
        runtime_receipt: summary
            .runtime_receipt
            .as_ref()
            .map(authoritative_agent_runtime_receipt_record),
        workflow_steps: summary
            .workflow_steps
            .iter()
            .map(|step| AgentRuntimeWorkflowStepSummary {
                step_index: step.step_index,
                step_id: step.step_id.clone(),
                request_commitment: step.request_commitment,
                result_commitment: step.result_commitment,
                runtime_receipt: step
                    .runtime_receipt
                    .as_ref()
                    .map(authoritative_agent_runtime_receipt_record),
                content_type: step.content_type.clone(),
                response_json: step.response_json.clone(),
                response_text: step.response_text.clone(),
            })
            .collect(),
        content_type: summary.content_type,
        response_json: summary.response_json,
        response_text: summary.response_text,
        error: summary.error,
    }))
}

fn sanitize_runtime_path_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect()
}

fn authoritative_agent_status_entry(
    apartment_name: &str,
    record: &SoraAgentApartmentRecordV1,
    sequence: u64,
) -> AgentApartmentStatusEntry {
    AgentApartmentStatusEntry {
        apartment_name: apartment_name.to_owned(),
        manifest_hash: record.manifest_hash,
        status: authoritative_agent_runtime_status_for_sequence(record, sequence),
        lease_started_sequence: record.lease_started_sequence,
        lease_expires_sequence: record.lease_expires_sequence,
        lease_remaining_ticks: record.lease_expires_sequence.saturating_sub(sequence),
        restart_count: record.restart_count,
        state_quota_bytes: record.manifest.state_quota_bytes.get(),
        tool_capability_count: u32::try_from(record.manifest.tool_capabilities.len())
            .unwrap_or(u32::MAX),
        policy_capability_count: u32::try_from(record.manifest.policy_capabilities.len())
            .unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
        pending_wallet_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        pending_mailbox_message_count: u32::try_from(record.mailbox_queue.len())
            .unwrap_or(u32::MAX),
        autonomy_budget_ceiling_units: record.autonomy_budget_ceiling_units,
        autonomy_budget_remaining_units: record.autonomy_budget_remaining_units,
        artifact_allowlist_count: u32::try_from(record.artifact_allowlist.len())
            .unwrap_or(u32::MAX),
        autonomy_run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        process_generation: record.process_generation,
        process_started_sequence: record.process_started_sequence,
        last_active_sequence: record.last_active_sequence,
        last_checkpoint_sequence: record.last_checkpoint_sequence,
        checkpoint_count: record.checkpoint_count,
        persistent_state_total_bytes: record.persistent_state.total_bytes,
        persistent_state_key_count: u32::try_from(record.persistent_state.key_sizes.len())
            .unwrap_or(u32::MAX),
        spend_limit_count: u32::try_from(record.manifest.spend_limits.len()).unwrap_or(u32::MAX),
        upgrade_policy: record.manifest.upgrade_policy.clone(),
        last_restart_sequence: record.last_restart_sequence,
        last_restart_reason: record.last_restart_reason.clone(),
    }
}

fn authoritative_agent_status_response(
    app: &SharedAppState,
    apartment_name: Option<&str>,
) -> Result<AgentStatusResponse, SoracloudError> {
    let apartment_filter = apartment_name.map(parse_agent_apartment_name).transpose()?;
    let sequence = authoritative_soracloud_sequence(app);
    let state_view = app.state.view();
    let world = state_view.world();

    let mut apartments = world
        .soracloud_agent_apartments()
        .iter()
        .filter(|(apartment_name, _record)| {
            apartment_filter
                .as_ref()
                .is_none_or(|filter| filter.as_str() == apartment_name.as_str())
        })
        .map(|(apartment_name, record)| {
            authoritative_agent_status_entry(apartment_name, record, sequence)
        })
        .collect::<Vec<_>>();
    apartments.sort_by(|left, right| left.apartment_name.cmp(&right.apartment_name));

    Ok(AgentStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        apartment_count: u32::try_from(apartments.len()).unwrap_or(u32::MAX),
        event_count: u32::try_from(
            world
                .soracloud_agent_apartment_audit_events()
                .iter()
                .count(),
        )
        .unwrap_or(u32::MAX),
        apartments,
    })
}

fn authoritative_agent_mailbox_status_response(
    app: &SharedAppState,
    apartment_name: &str,
) -> Result<AgentMailboxStatusResponse, SoracloudError> {
    let apartment_name = parse_agent_apartment_name(apartment_name)?;
    let sequence = authoritative_soracloud_sequence(app);
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(&apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let messages = record
        .mailbox_queue
        .iter()
        .map(authoritative_agent_mailbox_message_entry)
        .collect::<Vec<_>>();

    Ok(AgentMailboxStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        apartment_name,
        status: authoritative_agent_runtime_status_for_sequence(&record, sequence),
        pending_message_count: u32::try_from(messages.len()).unwrap_or(u32::MAX),
        event_count: u32::try_from(
            world
                .soracloud_agent_apartment_audit_events()
                .iter()
                .count(),
        )
        .unwrap_or(u32::MAX),
        messages,
    })
}

fn authoritative_agent_autonomy_status_response(
    app: &SharedAppState,
    apartment_name: &str,
) -> Result<AgentAutonomyStatusResponse, SoracloudError> {
    let apartment_name = parse_agent_apartment_name(apartment_name)?;
    let apartment_name_for_runs = apartment_name.clone();
    let sequence = authoritative_soracloud_sequence(app);
    let state_view = app.state.view();
    let world = state_view.world();
    let record = world
        .soracloud_agent_apartments()
        .get(&apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let runtime_recent_runs = authoritative_agent_runtime_recent_runs(app, apartment_name.as_ref());

    Ok(AgentAutonomyStatusResponse {
        apartment_name,
        sequence,
        status: authoritative_agent_runtime_status_for_sequence(&record, sequence),
        lease_expires_sequence: record.lease_expires_sequence,
        lease_remaining_ticks: record.lease_expires_sequence.saturating_sub(sequence),
        manifest_hash: record.manifest_hash,
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
        budget_ceiling_units: record.autonomy_budget_ceiling_units,
        budget_remaining_units: record.autonomy_budget_remaining_units,
        allowlist_count: u32::try_from(record.artifact_allowlist.len()).unwrap_or(u32::MAX),
        run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        process_generation: record.process_generation,
        process_started_sequence: record.process_started_sequence,
        last_active_sequence: record.last_active_sequence,
        last_checkpoint_sequence: record.last_checkpoint_sequence,
        checkpoint_count: record.checkpoint_count,
        persistent_state_total_bytes: record.persistent_state.total_bytes,
        persistent_state_key_count: u32::try_from(record.persistent_state.key_sizes.len())
            .unwrap_or(u32::MAX),
        allowlist: record
            .artifact_allowlist
            .values()
            .map(authoritative_agent_allowlist_entry)
            .collect(),
        recent_runs: record
            .autonomy_run_history
            .iter()
            .rev()
            .take(AGENT_AUTONOMY_RECENT_RUN_LIMIT)
            .map(|run| authoritative_agent_run_record(world, apartment_name_for_runs.as_ref(), run))
            .collect(),
        runtime_recent_runs,
    })
}

fn authoritative_ciphertext_query_response(
    app: &SharedAppState,
    request: SignedCiphertextQueryRequest,
) -> Result<CiphertextQueryResponse, SoracloudError> {
    verify_ciphertext_query_signature(&request)?;
    request.query.validate().map_err(|err| {
        SoracloudError::bad_request(format!("ciphertext query failed validation: {err}"))
    })?;

    let state_view = app.state.view();
    let world = state_view.world();
    let service_name = request.query.service_name.clone();
    let binding_name = request.query.binding_name.clone();
    let signer = request.provenance.signer.to_string();
    let query_hash = Hash::new(Encode::encode(&request.query));
    let limit = usize::from(request.query.max_results.get());

    let deployment = world
        .soracloud_service_deployments()
        .get(&service_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let bundle = world
        .soracloud_service_revisions()
        .get(&(
            service_name.as_ref().to_owned(),
            deployment.current_service_version.clone(),
        ))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "service `{service_name}` active revision `{}` is missing from authoritative state",
                deployment.current_service_version
            ))
        })?;
    let binding = bundle
        .service
        .state_bindings
        .iter()
        .find(|binding| binding.binding_name == binding_name)
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "binding `{binding_name}` is not declared for service `{service_name}`"
            ))
        })?;
    if binding.encryption == SoraStateEncryptionV1::Plaintext {
        return Err(SoracloudError::conflict(format!(
            "binding `{binding_name}` is plaintext; ciphertext query interface is not applicable"
        )));
    }
    if !request
        .query
        .state_key_prefix
        .starts_with(&binding.key_prefix)
    {
        return Err(SoracloudError::conflict(format!(
            "query prefix `{}` is outside binding prefix `{}`",
            request.query.state_key_prefix, binding.key_prefix
        )));
    }

    let audit_log = authoritative_audit_log(app);
    let served_sequence = audit_log
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(0);
    let anchor_hash = audit_anchor_hash(&audit_log, served_sequence);
    let mut rows = Vec::new();
    let mut truncated = false;

    for ((stored_service, stored_binding, state_key), entry) in
        world.soracloud_service_state_entries().iter()
    {
        if stored_service != service_name.as_ref() || stored_binding != binding_name.as_ref() {
            continue;
        }
        if entry.encryption == SoraStateEncryptionV1::Plaintext {
            continue;
        }
        if !state_key.starts_with(&request.query.state_key_prefix) {
            continue;
        }
        if rows.len() >= limit {
            truncated = true;
            break;
        }

        let runtime_record = CiphertextRuntimeRecord {
            encryption: entry.encryption,
            payload_bytes: entry.payload_bytes.get(),
            commitment: entry.payload_commitment,
            last_update_sequence: entry.last_update_sequence,
            governance_tx_hash: entry.governance_tx_hash,
            source_action: audit_action_to_control_plane_action(entry.source_action),
        };
        let proof = if request.query.include_proof {
            Some(build_ciphertext_inclusion_proof(
                &audit_log,
                service_name.as_ref(),
                binding_name.as_ref(),
                state_key,
                &runtime_record,
                served_sequence,
                anchor_hash,
            ))
        } else {
            None
        };
        rows.push(CiphertextQueryResultItemV1 {
            binding_name: binding_name.clone(),
            state_key: match request.query.metadata_level {
                CiphertextQueryMetadataLevelV1::Minimal => None,
                CiphertextQueryMetadataLevelV1::Standard => Some(state_key.clone()),
            },
            state_key_digest: derive_state_key_digest(
                service_name.as_ref(),
                binding_name.as_ref(),
                state_key,
            ),
            payload_bytes: entry.payload_bytes,
            ciphertext_commitment: entry.payload_commitment,
            encryption: entry.encryption,
            last_update_sequence: entry.last_update_sequence,
            governance_tx_hash: entry.governance_tx_hash,
            proof,
        });
    }

    let response = CiphertextQueryResponseV1 {
        schema_version: CIPHERTEXT_QUERY_RESPONSE_VERSION_V1,
        query_hash,
        service_name,
        binding_name,
        metadata_level: request.query.metadata_level,
        served_sequence,
        result_count: u16::try_from(rows.len()).unwrap_or(u16::MAX),
        truncated,
        results: rows,
    };
    response.validate().map_err(|err| {
        SoracloudError::internal(format!(
            "ciphertext query response validation failed unexpectedly: {err}"
        ))
    })?;

    Ok(CiphertextQueryResponse {
        action: SoracloudAction::CiphertextQuery,
        response,
        signed_by: signer,
    })
}

fn authoritative_health_compliance_report(
    app: &SharedAppState,
    service_name: Option<&str>,
    jurisdiction_tag: Option<&str>,
    limit: usize,
) -> Result<HealthComplianceReportResponse, SoracloudError> {
    let service_name = service_name
        .map(|literal| {
            literal
                .parse::<Name>()
                .map(|name| name.to_string())
                .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))
        })
        .transpose()?;
    let limit = limit.max(1).min(MAX_HEALTH_COMPLIANCE_LIMIT);
    let audit_log = authoritative_audit_log(app);
    let generated_at_sequence = audit_log
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(0);

    let access_events = audit_log
        .iter()
        .filter(|event| event.action == SoracloudAction::DecryptionRequest)
        .filter(|event| {
            service_name
                .as_deref()
                .is_none_or(|filter| filter == event.service_name.as_str())
        })
        .filter(|event| {
            jurisdiction_tag.is_none_or(|filter| event.jurisdiction_tag.as_deref() == Some(filter))
        })
        .collect::<Vec<_>>();

    let total_access_events = u32::try_from(access_events.len()).unwrap_or(u32::MAX);
    let break_glass_events = u32::try_from(
        access_events
            .iter()
            .filter(|event| event.break_glass.unwrap_or(false))
            .count(),
    )
    .unwrap_or(u32::MAX);
    let non_break_glass_events = total_access_events.saturating_sub(break_glass_events);
    let consent_evidence_present_events = u32::try_from(
        access_events
            .iter()
            .filter(|event| event.consent_evidence_hash.is_some())
            .count(),
    )
    .unwrap_or(u32::MAX);
    let consent_evidence_coverage_bps = if total_access_events == 0 {
        0
    } else {
        let numerator = u128::from(consent_evidence_present_events).saturating_mul(10_000);
        let denominator = u128::from(total_access_events);
        u16::try_from(numerator / denominator).unwrap_or(u16::MAX)
    };

    let recent_access_events = access_events
        .iter()
        .rev()
        .take(limit)
        .map(|event| HealthAccessAuditEntry {
            sequence: event.sequence,
            service_name: event.service_name.clone(),
            binding_name: event.binding_name.clone().unwrap_or_default(),
            state_key: event.state_key.clone().unwrap_or_default(),
            policy_name: event.policy_name.clone().unwrap_or_default(),
            jurisdiction_tag: event.jurisdiction_tag.clone().unwrap_or_default(),
            consent_evidence_hash: event.consent_evidence_hash,
            break_glass: event.break_glass.unwrap_or(false),
            break_glass_reason: event.break_glass_reason.clone(),
            governance_tx_hash: event.governance_tx_hash.unwrap_or_else(|| {
                Hash::new(Encode::encode(&(
                    "soracloud.health_compliance.synthetic_governance_hash.v1",
                    event.sequence,
                    event.service_name.as_str(),
                )))
            }),
            signed_by: event.signed_by.clone(),
        })
        .collect::<Vec<_>>();

    let mut jurisdiction_stats_acc: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for event in &access_events {
        let tag = event.jurisdiction_tag.clone().unwrap_or_default();
        let entry = jurisdiction_stats_acc.entry(tag).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(1);
        if event.break_glass.unwrap_or(false) {
            entry.1 = entry.1.saturating_add(1);
        }
    }
    let jurisdiction_stats = jurisdiction_stats_acc
        .into_iter()
        .map(
            |(jurisdiction_tag, (access_event_count, break_glass_event_count))| {
                HealthJurisdictionStat {
                    jurisdiction_tag,
                    access_event_count,
                    break_glass_event_count,
                }
            },
        )
        .collect::<Vec<_>>();

    let mut policy_history_acc: BTreeMap<(String, String, String), HealthPolicyDiffEntry> =
        BTreeMap::new();
    for event in &access_events {
        let Some(policy_name) = event.policy_name.clone() else {
            continue;
        };
        let Some(policy_snapshot_hash) = event.policy_snapshot_hash else {
            continue;
        };
        let jurisdiction = event.jurisdiction_tag.clone().unwrap_or_default();
        let key = (
            policy_name.clone(),
            jurisdiction.clone(),
            policy_snapshot_hash.to_string(),
        );
        let entry = policy_history_acc
            .entry(key)
            .or_insert(HealthPolicyDiffEntry {
                policy_name,
                jurisdiction_tag: jurisdiction,
                policy_snapshot_hash,
                first_seen_sequence: event.sequence,
                last_seen_sequence: event.sequence,
                event_count: 0,
            });
        entry.first_seen_sequence = entry.first_seen_sequence.min(event.sequence);
        entry.last_seen_sequence = entry.last_seen_sequence.max(event.sequence);
        entry.event_count = entry.event_count.saturating_add(1);
    }
    let mut policy_diff_history = policy_history_acc.into_values().collect::<Vec<_>>();
    policy_diff_history.sort_by(|left, right| {
        right
            .last_seen_sequence
            .cmp(&left.last_seen_sequence)
            .then_with(|| left.policy_name.cmp(&right.policy_name))
            .then_with(|| left.jurisdiction_tag.cmp(&right.jurisdiction_tag))
    });
    if policy_diff_history.len() > limit {
        policy_diff_history.truncate(limit);
    }

    let state_view = app.state.view();
    let world = state_view.world();
    let mut data_flow_services = BTreeSet::new();
    if let Some(service_name) = service_name.clone() {
        data_flow_services.insert(service_name);
    } else {
        for event in &access_events {
            data_flow_services.insert(event.service_name.clone());
        }
    }
    let mut data_flow_attestations = Vec::new();
    for service_name in data_flow_services {
        let Ok(service_id) = service_name.parse::<Name>() else {
            continue;
        };
        let Some(deployment) = world.soracloud_service_deployments().get(&service_id) else {
            continue;
        };
        let Some(bundle) = world.soracloud_service_revisions().get(&(
            service_name.clone(),
            deployment.current_service_version.clone(),
        )) else {
            continue;
        };
        for binding in &bundle.service.state_bindings {
            if binding.encryption == SoraStateEncryptionV1::Plaintext {
                continue;
            }
            data_flow_attestations.push(HealthDataFlowAttestation {
                service_name: service_name.clone(),
                current_version: deployment.current_service_version.clone(),
                binding_name: binding.binding_name.to_string(),
                key_prefix: binding.key_prefix.clone(),
                encryption: binding.encryption,
                mutability: binding.mutability,
            });
        }
    }

    Ok(HealthComplianceReportResponse {
        schema_version: HEALTH_COMPLIANCE_REPORT_VERSION_V1,
        service_name,
        jurisdiction_tag: jurisdiction_tag.map(ToOwned::to_owned),
        generated_at_sequence,
        total_access_events,
        break_glass_events,
        non_break_glass_events,
        consent_evidence_present_events,
        consent_evidence_coverage_bps,
        recent_access_events,
        jurisdiction_stats,
        data_flow_attestations,
        policy_diff_history,
    })
}

fn deployment_bundle_to_control_plane_revision(
    deployment: &SoraServiceDeploymentStateV1,
    bundle: &SoraDeploymentBundleV1,
    latest_audit: Option<&SoraServiceAuditEventV1>,
) -> ControlPlaneServiceRevision {
    let host_admission = admit_scr_host_bundle(bundle).ok();
    let route = bundle.service.route.as_ref();
    let network = host_admission
        .as_ref()
        .map(|admission| admission.network.clone())
        .unwrap_or_else(|| bundle.container.capabilities.network.clone());
    let sandbox_profile_hash = host_admission
        .as_ref()
        .map(|admission| admission.sandbox_profile_hash)
        .unwrap_or_else(|| bundle.container_manifest_hash());

    ControlPlaneServiceRevision {
        sequence: latest_audit.map_or(deployment.process_started_sequence, |event| event.sequence),
        action: latest_audit
            .map(|event| audit_action_to_control_plane_action(event.action))
            .unwrap_or(SoracloudAction::Deploy),
        service_version: bundle.service.service_version.clone(),
        service_manifest_hash: bundle.service_manifest_hash(),
        container_manifest_hash: bundle.container_manifest_hash(),
        replicas: bundle.service.replicas.get(),
        route_host: route.map(|route| route.host.clone()),
        route_path_prefix: route.map(|route| route.path_prefix.clone()),
        state_binding_count: u32::try_from(bundle.service.state_bindings.len()).unwrap_or(u32::MAX),
        state_bindings: bundle.service.state_bindings.clone(),
        allow_model_inference: bundle.container.capabilities.allow_model_inference,
        allow_model_training: bundle.container.capabilities.allow_model_training,
        runtime: bundle.container.runtime,
        allow_wallet_signing: bundle.container.capabilities.allow_wallet_signing,
        allow_state_writes: bundle.container.capabilities.allow_state_writes,
        network,
        cpu_millis: bundle.container.resources.cpu_millis.get(),
        memory_bytes: bundle.container.resources.memory_bytes.get(),
        ephemeral_storage_bytes: bundle.container.resources.ephemeral_storage_bytes.get(),
        max_open_files: bundle.container.resources.max_open_files.get(),
        max_tasks: bundle.container.resources.max_tasks.get(),
        start_grace_secs: bundle.container.lifecycle.start_grace_secs.get(),
        stop_grace_secs: bundle.container.lifecycle.stop_grace_secs.get(),
        healthcheck_path: bundle.container.lifecycle.healthcheck_path.clone(),
        required_config_names: bundle.container.required_config_names.clone(),
        required_secret_names: bundle.container.required_secret_names.clone(),
        config_exports: bundle.container.config_exports.clone(),
        sandbox_profile_hash,
        process_generation: deployment.process_generation,
        process_started_sequence: deployment.process_started_sequence,
        signed_by: latest_audit
            .map(|event| event.signer.to_string())
            .unwrap_or_else(|| "<unknown>".to_string()),
    }
}

pub(crate) fn resolve_public_local_read_route(
    app: &SharedAppState,
    host: &str,
    request_path: &str,
) -> Option<LocalReadRouteMatch> {
    let normalized_host = normalize_public_route_host(host);
    if normalized_host.is_empty() {
        return None;
    }
    let normalized_path = normalize_public_route_path(request_path);
    let state_view = app.state.view();
    let world = state_view.world();
    let mut best_match: Option<(usize, LocalReadRouteMatch)> = None;

    for (service_id, deployment) in world.soracloud_service_deployments().iter() {
        let service_name = service_id.to_string();
        let Some(bundle) = world.soracloud_service_revisions().get(&(
            service_name.clone(),
            deployment.current_service_version.clone(),
        )) else {
            continue;
        };
        let Some(route) = bundle.service.route.as_ref() else {
            continue;
        };
        if route.visibility != iroha_data_model::soracloud::SoraRouteVisibilityV1::Public {
            continue;
        }
        if !route.host.eq_ignore_ascii_case(normalized_host) {
            continue;
        }

        for handler in &bundle.service.handlers {
            let handler_class = match handler.class {
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Asset => {
                    SoracloudLocalReadKind::Asset
                }
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query => {
                    SoracloudLocalReadKind::Query
                }
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update
                | iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate => {
                    continue;
                }
            };
            let full_route = join_public_route_paths(
                route.path_prefix.as_str(),
                handler.route_path.as_deref().unwrap_or("/"),
            );
            let Some(handler_path) = split_public_handler_path(normalized_path, &full_route) else {
                continue;
            };
            let route_len = full_route.len();
            let route_match = LocalReadRouteMatch {
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                handler_name: handler.handler_name.to_string(),
                handler_class,
                handler_path,
            };
            let replace = best_match.as_ref().is_none_or(|(best_len, best)| {
                route_len > *best_len
                    || (route_len == *best_len
                        && (
                            route_match.service_name.as_str(),
                            route_match.service_version.as_str(),
                            route_match.handler_name.as_str(),
                        ) < (
                            best.service_name.as_str(),
                            best.service_version.as_str(),
                            best.handler_name.as_str(),
                        ))
            });
            if replace {
                best_match = Some((route_len, route_match));
            }
        }
    }

    best_match.map(|(_route_len, route_match)| route_match)
}

fn normalize_public_route_host(host: &str) -> &str {
    host.trim()
        .trim_end_matches('.')
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
}

fn normalize_public_route_path(path: &str) -> &str {
    if path.is_empty() { "/" } else { path }
}

fn join_public_route_paths(prefix: &str, handler_path: &str) -> String {
    let prefix = normalize_public_route_path(prefix).trim_end_matches('/');
    let handler_path = normalize_public_route_path(handler_path).trim_start_matches('/');
    match (prefix.is_empty(), handler_path.is_empty()) {
        (true, true) => "/".to_owned(),
        (true, false) => format!("/{handler_path}"),
        (false, true) => prefix.to_owned(),
        (false, false) => format!("{prefix}/{handler_path}"),
    }
}

fn split_public_handler_path(request_path: &str, full_route: &str) -> Option<String> {
    if full_route == "/" {
        return Some(request_path.to_owned());
    }
    if request_path == full_route {
        return Some("/".to_owned());
    }
    if request_path.starts_with(full_route)
        && request_path
            .as_bytes()
            .get(full_route.len())
            .is_some_and(|separator| *separator == b'/')
    {
        return Some(request_path[full_route.len()..].to_owned());
    }
    None
}

pub(crate) fn control_plane_snapshot(
    app: &SharedAppState,
    service_name: Option<&str>,
    audit_limit: usize,
) -> ControlPlaneSnapshot {
    let state_view = app.state.view();
    let world = state_view.world();
    let mut services = Vec::new();

    for (service_id, deployment) in world.soracloud_service_deployments().iter() {
        let service_label = service_id.to_string();
        if service_name.is_some_and(|filter| filter != service_label) {
            continue;
        }

        let revision_key = (
            service_label.clone(),
            deployment.current_service_version.clone(),
        );
        let current_bundle = world
            .soracloud_service_revisions()
            .get(&revision_key)
            .cloned();
        let latest_audit = world
            .soracloud_service_audit_events()
            .iter()
            .filter(|(_sequence, event)| {
                &event.service_name == service_id
                    && matches!(
                        event.action,
                        SoraServiceLifecycleActionV1::Deploy
                            | SoraServiceLifecycleActionV1::Upgrade
                            | SoraServiceLifecycleActionV1::Rollout
                            | SoraServiceLifecycleActionV1::Rollback
                    )
            })
            .map(|(_sequence, event)| event)
            .max_by_key(|event| event.sequence);

        services.push(ControlPlaneServiceSnapshot {
            service_name: service_label,
            current_version: deployment.current_service_version.clone(),
            revision_count: deployment.revision_count,
            config_generation: deployment.config_generation,
            secret_generation: deployment.secret_generation,
            config_entry_count: u32::try_from(deployment.service_configs.len()).unwrap_or(u32::MAX),
            secret_entry_count: u32::try_from(deployment.service_secrets.len()).unwrap_or(u32::MAX),
            latest_revision: current_bundle.as_ref().map(|bundle| {
                deployment_bundle_to_control_plane_revision(deployment, bundle, latest_audit)
            }),
            active_rollout: deployment
                .active_rollout
                .as_ref()
                .map(rollout_state_to_runtime_state),
            last_rollout: deployment
                .last_rollout
                .as_ref()
                .map(rollout_state_to_runtime_state),
        });
    }

    let limit = audit_limit.max(1).min(MAX_AUDIT_LIMIT);
    let mut recent_audit_events = world
        .soracloud_service_audit_events()
        .iter()
        .filter(|(_sequence, event)| {
            service_name.is_none_or(|filter| filter == event.service_name.as_ref())
        })
        .map(|(_sequence, event)| audit_event_to_control_plane_audit_event(event))
        .collect::<Vec<_>>();
    recent_audit_events.sort_by_key(|event| std::cmp::Reverse(event.sequence));
    recent_audit_events.truncate(limit);

    ControlPlaneSnapshot {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        service_count: u32::try_from(services.len()).unwrap_or(u32::MAX),
        audit_event_count: u32::try_from(world.soracloud_service_audit_events().iter().count())
            .unwrap_or(u32::MAX),
        services,
        recent_audit_events,
    }
}

pub(crate) async fn handle_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/deploy").await {
        return err.into_response();
    }

    if let Err(err) = verify_bundle_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = admit_scr_host_bundle(&request.bundle) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = request.bundle.service.service_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::DeploySoracloudService {
            bundle: request.bundle,
            initial_service_configs: request.initial_service_configs,
            initial_service_secrets: request.initial_service_secrets,
            provenance: request.provenance,
        }),
        "/v1/soracloud/deploy",
        move |app, baseline| {
            authoritative_service_mutation_response(
                app,
                baseline,
                &service_name,
                SoraServiceLifecycleActionV1::Deploy,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_upgrade(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/upgrade").await {
        return err.into_response();
    }

    if let Err(err) = verify_bundle_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = admit_scr_host_bundle(&request.bundle) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = request.bundle.service.service_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::UpgradeSoracloudService {
            bundle: request.bundle,
            initial_service_configs: request.initial_service_configs,
            initial_service_secrets: request.initial_service_secrets,
            provenance: request.provenance,
        }),
        "/v1/soracloud/upgrade",
        move |app, baseline| {
            authoritative_service_mutation_response(
                app,
                baseline,
                &service_name,
                SoraServiceLifecycleActionV1::Upgrade,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_rollback(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedRollbackRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/rollback").await {
        return err.into_response();
    }

    if let Err(err) = verify_rollback_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };

    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RollbackSoracloudService {
            service_name,
            target_version: request.payload.target_version,
            provenance: request.provenance,
        }),
        "/v1/soracloud/rollback",
        move |app, baseline| {
            authoritative_service_mutation_response(
                app,
                baseline,
                &service_label,
                SoraServiceLifecycleActionV1::Rollback,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_rollout(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedRolloutAdvanceRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/rollout").await {
        return err.into_response();
    }

    if let Err(err) = verify_rollout_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };

    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let rollout_handle = request.payload.rollout_handle.clone();
    let governance_tx_hash = request.payload.governance_tx_hash;
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AdvanceSoracloudRollout {
            service_name,
            rollout_handle: request.payload.rollout_handle,
            healthy: request.payload.healthy,
            promote_to_percent: request.payload.promote_to_percent,
            governance_tx_hash: request.payload.governance_tx_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/rollout",
        move |app, baseline| {
            authoritative_rollout_mutation_response(
                app,
                baseline,
                &service_label,
                &rollout_handle,
                governance_tx_hash,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_state_mutation(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedStateMutationRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/state/mutate").await {
        return err.into_response();
    }

    if let Err(err) = verify_state_mutation_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let binding_name: Name = match request.payload.binding_name.parse() {
        Ok(binding_name) => binding_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid binding_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let binding_label = binding_name.to_string();
    let state_key = request.payload.key.clone();
    let operation = request.payload.operation;
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::MutateSoracloudState {
            service_name,
            binding_name,
            state_key: request.payload.key,
            operation: state_mutation_operation_to_model(request.payload.operation),
            value_size_bytes: request.payload.value_size_bytes,
            encryption: request.payload.encryption,
            governance_tx_hash: request.payload.governance_tx_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/state/mutate",
        move |app, baseline| {
            authoritative_state_mutation_response(
                app,
                baseline,
                &service_label,
                &binding_label,
                &state_key,
                operation,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_config_set(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedServiceConfigSetRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/config/set").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_service_config_set_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let service_label = service_name.to_string();
    let config_name = request.payload.config_name.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::SetSoracloudServiceConfig {
            service_name,
            config_name: config_name.clone(),
            value_json: request.payload.value_json,
            provenance: request.provenance,
        }),
        "/v1/soracloud/service/config/set",
        move |app, baseline| {
            authoritative_service_config_mutation_response(
                app,
                baseline,
                &service_label,
                &config_name,
                ServiceMaterialMutationOperation::Upsert,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_config_delete(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedServiceConfigDeleteRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/config/delete").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_service_config_delete_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let service_label = service_name.to_string();
    let config_name = request.payload.config_name.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::DeleteSoracloudServiceConfig {
            service_name,
            config_name: config_name.clone(),
            provenance: request.provenance,
        }),
        "/v1/soracloud/service/config/delete",
        move |app, baseline| {
            authoritative_service_config_mutation_response(
                app,
                baseline,
                &service_label,
                &config_name,
                ServiceMaterialMutationOperation::Delete,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_config_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<ServiceConfigStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/config/status").await
    {
        return err.into_response();
    }

    let service_name = match parse_service_name(&query.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    match authoritative_service_config_status_response(
        &app,
        service_name.as_ref(),
        query.config_name.as_deref(),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_secret_set(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedServiceSecretSetRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/secret/set").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_service_secret_set_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let service_label = service_name.to_string();
    let secret_name = request.payload.secret_name.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::SetSoracloudServiceSecret {
            service_name,
            secret_name: secret_name.clone(),
            secret: request.payload.secret,
            provenance: request.provenance,
        }),
        "/v1/soracloud/service/secret/set",
        move |app, baseline| {
            authoritative_service_secret_mutation_response(
                app,
                baseline,
                &service_label,
                &secret_name,
                ServiceMaterialMutationOperation::Upsert,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_secret_delete(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedServiceSecretDeleteRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/secret/delete").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_service_secret_delete_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let service_label = service_name.to_string();
    let secret_name = request.payload.secret_name.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::DeleteSoracloudServiceSecret {
            service_name,
            secret_name: secret_name.clone(),
            provenance: request.provenance,
        }),
        "/v1/soracloud/service/secret/delete",
        move |app, baseline| {
            authoritative_service_secret_mutation_response(
                app,
                baseline,
                &service_label,
                &secret_name,
                ServiceMaterialMutationOperation::Delete,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_service_secret_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<ServiceSecretStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/service/secret/status").await
    {
        return err.into_response();
    }

    let service_name = match parse_service_name(&query.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    match authoritative_service_secret_status_response(
        &app,
        service_name.as_ref(),
        query.secret_name.as_deref(),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_fhe_job_run(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedFheJobRunRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/fhe/job/run").await {
        return err.into_response();
    }

    if let Err(err) = verify_fhe_job_run_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let binding_name: Name = match request.payload.binding_name.parse() {
        Ok(binding_name) => binding_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid binding_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let binding_label = binding_name.to_string();
    let job = request.payload.job.clone();
    let governance_tx_hash = request.payload.governance_tx_hash;
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RunSoracloudFheJob {
            service_name,
            binding_name,
            job: request.payload.job,
            policy: request.payload.policy,
            param_set: request.payload.param_set,
            governance_tx_hash: request.payload.governance_tx_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/fhe/job/run",
        move |app, baseline| {
            authoritative_fhe_job_mutation_response(
                app,
                baseline,
                &service_label,
                &binding_label,
                &job,
                governance_tx_hash,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_decryption_request(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedDecryptionRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/decrypt/request").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_decryption_request_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let request_id = request.payload.request.request_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RecordSoracloudDecryptionRequest {
            service_name,
            policy: request.payload.policy,
            request: request.payload.request,
            provenance: request.provenance,
        }),
        "/v1/soracloud/decrypt/request",
        move |app, baseline| {
            authoritative_decryption_request_mutation_response(
                app,
                baseline,
                &service_label,
                &request_id,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_health_access_request(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedDecryptionRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/health/access/request").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_decryption_request_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let request_id = request.payload.request.request_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RecordSoracloudDecryptionRequest {
            service_name,
            policy: request.payload.policy,
            request: request.payload.request,
            provenance: request.provenance,
        }),
        "/v1/soracloud/health/access/request",
        move |app, baseline| {
            authoritative_decryption_request_mutation_response(
                app,
                baseline,
                &service_label,
                &request_id,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_ciphertext_query(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedCiphertextQueryRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/ciphertext/query").await
    {
        return err.into_response();
    }

    match authoritative_ciphertext_query_response(&app, request) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_training_job_start(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedTrainingJobStartRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/training/job/start").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_training_job_start_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let job_id = request.payload.job_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::StartSoracloudTrainingJob {
            service_name,
            model_name: request.payload.model_name,
            job_id: request.payload.job_id,
            worker_group_size: request.payload.worker_group_size,
            target_steps: request.payload.target_steps,
            checkpoint_interval_steps: request.payload.checkpoint_interval_steps,
            max_retries: request.payload.max_retries,
            step_compute_units: request.payload.step_compute_units,
            compute_budget_units: request.payload.compute_budget_units,
            storage_budget_bytes: request.payload.storage_budget_bytes,
            provenance: request.provenance,
        }),
        "/v1/soracloud/training/job/start",
        move |app, baseline| {
            authoritative_training_job_mutation_response(
                app,
                baseline,
                &service_label,
                &job_id,
                SoraTrainingJobActionV1::Start,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_training_job_checkpoint(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedTrainingJobCheckpointRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/training/job/checkpoint").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_training_job_checkpoint_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let job_id = request.payload.job_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::CheckpointSoracloudTrainingJob {
            service_name,
            job_id: request.payload.job_id,
            completed_step: request.payload.completed_step,
            checkpoint_size_bytes: request.payload.checkpoint_size_bytes,
            metrics_hash: request.payload.metrics_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/training/job/checkpoint",
        move |app, baseline| {
            authoritative_training_job_mutation_response(
                app,
                baseline,
                &service_label,
                &job_id,
                SoraTrainingJobActionV1::Checkpoint,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_training_job_retry(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedTrainingJobRetryRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/training/job/retry").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_training_job_retry_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let job_id = request.payload.job_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RetrySoracloudTrainingJob {
            service_name,
            job_id: request.payload.job_id,
            reason: request.payload.reason,
            provenance: request.provenance,
        }),
        "/v1/soracloud/training/job/retry",
        move |app, baseline| {
            authoritative_training_job_mutation_response(
                app,
                baseline,
                &service_label,
                &job_id,
                SoraTrainingJobActionV1::Retry,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_training_job_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<TrainingJobStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/training/job/status").await
    {
        return err.into_response();
    }

    match authoritative_training_job_status_response(&app, &query.service_name, &query.job_id) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_weight_register(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelWeightRegisterRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/weight/register").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_weight_register_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let model_name = request.payload.model_name.clone();
    let target_version = request.payload.weight_version.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RegisterSoracloudModelWeight {
            service_name,
            model_name: request.payload.model_name,
            weight_version: request.payload.weight_version,
            training_job_id: request.payload.training_job_id,
            parent_version: request.payload.parent_version,
            weight_artifact_hash: request.payload.weight_artifact_hash,
            dataset_ref: request.payload.dataset_ref,
            training_config_hash: request.payload.training_config_hash,
            reproducibility_hash: request.payload.reproducibility_hash,
            provenance_attestation_hash: request.payload.provenance_attestation_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/weight/register",
        move |app, baseline| {
            authoritative_model_weight_mutation_response(
                app,
                baseline,
                &service_label,
                &model_name,
                &target_version,
                SoraModelWeightActionV1::Register,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_weight_promote(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelWeightPromoteRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/weight/promote").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_weight_promote_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let model_name = request.payload.model_name.clone();
    let target_version = request.payload.weight_version.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::PromoteSoracloudModelWeight {
            service_name,
            model_name: request.payload.model_name,
            weight_version: request.payload.weight_version,
            gate_approved: request.payload.gate_approved,
            gate_report_hash: request.payload.gate_report_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/weight/promote",
        move |app, baseline| {
            authoritative_model_weight_mutation_response(
                app,
                baseline,
                &service_label,
                &model_name,
                &target_version,
                SoraModelWeightActionV1::Promote,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_weight_rollback(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelWeightRollbackRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/weight/rollback").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_weight_rollback_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let model_name = request.payload.model_name.clone();
    let target_version = request.payload.target_version.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RollbackSoracloudModelWeight {
            service_name,
            model_name: request.payload.model_name,
            target_version: request.payload.target_version,
            reason: request.payload.reason,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/weight/rollback",
        move |app, baseline| {
            authoritative_model_weight_mutation_response(
                app,
                baseline,
                &service_label,
                &model_name,
                &target_version,
                SoraModelWeightActionV1::Rollback,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_weight_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<ModelWeightStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/weight/status").await
    {
        return err.into_response();
    }

    match authoritative_model_weight_status_response(&app, &query.service_name, &query.model_name) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_artifact_register(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelArtifactRegisterRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/artifact/register").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_artifact_register_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let training_job_id = request.payload.training_job_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RegisterSoracloudModelArtifact {
            service_name,
            model_name: request.payload.model_name,
            training_job_id: request.payload.training_job_id,
            weight_artifact_hash: request.payload.weight_artifact_hash,
            dataset_ref: request.payload.dataset_ref,
            training_config_hash: request.payload.training_config_hash,
            reproducibility_hash: request.payload.reproducibility_hash,
            provenance_attestation_hash: request.payload.provenance_attestation_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/artifact/register",
        move |app, baseline| {
            authoritative_model_artifact_mutation_response(
                app,
                baseline,
                &service_label,
                &training_job_id,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_artifact_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<ModelArtifactStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/artifact/status").await
    {
        return err.into_response();
    }

    match authoritative_model_artifact_status_response(
        &app,
        &query.service_name,
        query.model_name.as_deref(),
        query.artifact_id.as_deref(),
        query.training_job_id.as_deref(),
        query.weight_version.as_deref(),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_uploaded_model_init(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedUploadedModelBundleInitRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/upload/init").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_uploaded_model_bundle_init_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = request.payload.bundle.service_name.to_string();
    let model_id = request.payload.bundle.model_id.clone();
    let weight_version = request.payload.bundle.weight_version.clone();
    let signed_by = request.provenance.signer.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RegisterSoracloudUploadedModelBundle {
            bundle: request.payload.bundle,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/upload/init",
        move |app, _baseline| {
            authoritative_uploaded_model_status_response(
                app,
                &service_name,
                &model_id,
                &weight_version,
            )
            .map(|status| UploadedModelMutationResponse {
                action: UploadedModelAction::Init,
                status,
                signed_by,
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_uploaded_model_chunk(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedUploadedModelChunkRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/upload/chunk").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_uploaded_model_chunk_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = request.payload.chunk.service_name.to_string();
    let model_id = request.payload.chunk.model_id.clone();
    let weight_version = request.payload.chunk.weight_version.clone();
    let signed_by = request.provenance.signer.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AppendSoracloudUploadedModelChunk {
            chunk: request.payload.chunk,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/upload/chunk",
        move |app, _baseline| {
            authoritative_uploaded_model_status_response(
                app,
                &service_name,
                &model_id,
                &weight_version,
            )
            .map(|status| UploadedModelMutationResponse {
                action: UploadedModelAction::Chunk,
                status,
                signed_by,
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_uploaded_model_finalize(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedUploadedModelFinalizeRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/upload/finalize").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_uploaded_model_finalize_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let model_id = request.payload.model_id.clone();
    let weight_version = request.payload.weight_version.clone();
    let signed_by = request.provenance.signer.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::FinalizeSoracloudUploadedModelBundle {
            service_name,
            model_name: request.payload.model_name,
            model_id: request.payload.model_id,
            artifact_id: request.payload.artifact_id,
            weight_version: request.payload.weight_version,
            bundle_root: request.payload.bundle_root,
            privacy_mode: request.payload.privacy_mode,
            weight_artifact_hash: request.payload.weight_artifact_hash,
            dataset_ref: request.payload.dataset_ref,
            training_config_hash: request.payload.training_config_hash,
            reproducibility_hash: request.payload.reproducibility_hash,
            provenance_attestation_hash: request.payload.provenance_attestation_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/upload/finalize",
        move |app, _baseline| {
            authoritative_uploaded_model_status_response(
                app,
                &service_label,
                &model_id,
                &weight_version,
            )
            .map(|status| UploadedModelMutationResponse {
                action: UploadedModelAction::Finalize,
                status,
                signed_by,
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_private_compile(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedPrivateCompileRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/model/compile").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_private_compile_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let model_id = request.payload.model_id.clone();
    let weight_version = request.payload.weight_version.clone();
    let signed_by = request.provenance.signer.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AdmitSoracloudPrivateCompileProfile {
            service_name,
            model_id: request.payload.model_id,
            weight_version: request.payload.weight_version,
            bundle_root: request.payload.bundle_root,
            compile_profile: request.payload.compile_profile,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/compile",
        move |app, _baseline| {
            authoritative_uploaded_model_status_response(
                app,
                &service_label,
                &model_id,
                &weight_version,
            )
            .map(|status| UploadedModelMutationResponse {
                action: UploadedModelAction::Compile,
                status,
                signed_by,
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_uploaded_model_allow(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedUploadedModelAllowRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/model/allow").await {
        return err.into_response();
    }

    if let Err(err) = verify_uploaded_model_allow_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let service_name: Name = match request.payload.service_name.parse() {
        Ok(service_name) => service_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid service_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let signed_by = request.provenance.signer.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AllowSoracloudUploadedModel {
            apartment_name,
            service_name,
            model_name: request.payload.model_name,
            model_id: request.payload.model_id,
            artifact_id: request.payload.artifact_id,
            weight_version: request.payload.weight_version,
            bundle_root: request.payload.bundle_root,
            compile_profile_hash: request.payload.compile_profile_hash,
            privacy_mode: request.payload.privacy_mode,
            require_model_inference: request.payload.require_model_inference,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/allow",
        move |app, _baseline| {
            let state_view = app.state.view();
            let world = state_view.world();
            let record = world
                .soracloud_agent_apartments()
                .get(&apartment_label)
                .cloned()
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "agent apartment `{apartment_label}` not found in authoritative Soracloud state"
                    ))
                })?;
            let binding = record.uploaded_model_binding.ok_or_else(|| {
                SoracloudError::conflict(format!(
                    "agent apartment `{apartment_label}` has no uploaded model binding"
                ))
            })?;
            Ok(UploadedModelBindingMutationResponse {
                action: UploadedModelBindingAction::Allow,
                apartment_name: apartment_label,
                binding,
                signed_by,
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_private_inference_run(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedPrivateInferenceRunRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/run-private").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_private_inference_run_signature(&request) {
        return err.into_response();
    }
    if app.soracloud_runtime.is_none() {
        return SoracloudError::conflict("private uploaded-model runtime is not available")
            .into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    match submit_confirm_and_respond::<(), _>(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::StartSoracloudPrivateInference {
            session: request.payload.session,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model/run-private",
        |_app, _baseline| {
            Err(SoracloudError::internal(
                "private inference start returns a draft response".to_owned(),
            ))
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return err.into_response(),
    }
}

pub(crate) async fn handle_private_inference_run_finalize(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PrivateInferenceFinalizeRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/run-private").await
    {
        return err.into_response();
    }
    if app.soracloud_runtime.is_none() {
        return SoracloudError::conflict("private uploaded-model runtime is not available")
            .into_response();
    }
    let signer = match require_soracloud_request_signer(&headers) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let session_id = match parse_training_job_id(&request.session_id) {
        Ok(session_id) => session_id,
        Err(err) => return err.into_response(),
    };
    let runtime_execution = match execute_runtime_private_inference(
        &app,
        &session_id,
        SoracloudPrivateInferenceExecutionAction::Start,
    ) {
        Ok(execution) => execution,
        Err(error_message) => {
            match failed_private_inference_runtime_result(&app, &session_id, &error_message) {
                Ok(execution) => execution,
                Err(err) => return err.into_response(),
            }
        }
    };
    soracloud_draft_response(
        &signer,
        vec![InstructionBox::from(
            isi::soracloud::RecordSoracloudPrivateInferenceCheckpoint {
                session_id,
                status: runtime_execution.status,
                receipt_root: runtime_execution.receipt_root,
                xor_cost_nanos: runtime_execution.xor_cost_nanos,
                checkpoint: runtime_execution.checkpoint,
            },
        )],
    )
}

pub(crate) async fn handle_private_inference_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<PrivateInferenceStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/run-status").await
    {
        return err.into_response();
    }

    match authoritative_private_inference_status_response(&app, &query.session_id) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_uploaded_model_encryption_recipient(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        None,
        "v1/soracloud/model/upload/encryption-recipient",
    )
    .await
    {
        return err.into_response();
    }

    let Some(runtime) = app.soracloud_runtime.as_ref() else {
        return SoracloudError::conflict("Soracloud runtime is not available").into_response();
    };
    let Some(recipient) = runtime.uploaded_model_encryption_recipient() else {
        return SoracloudError::conflict(
            "uploaded model encryption recipient is not available on this Soracloud node",
        )
        .into_response();
    };
    JsonBody(UploadedModelEncryptionRecipientResponse {
        recipient: authoritative_uploaded_model_encryption_recipient(recipient),
    })
    .into_response()
}

pub(crate) async fn handle_uploaded_model_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<UploadedModelStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/upload/status").await
    {
        return err.into_response();
    }

    match authoritative_uploaded_model_status_from_query(&app, &query, false) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_private_compile_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<UploadedModelStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/compile/status").await
    {
        return err.into_response();
    }

    match authoritative_uploaded_model_status_from_query(&app, &query, true) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_private_inference_checkpoint(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedPrivateInferenceOutputReleaseRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model/decrypt-output").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_private_inference_output_release_signature(&request) {
        return err.into_response();
    }
    if app.soracloud_runtime.is_none() {
        return SoracloudError::conflict("private uploaded-model runtime is not available")
            .into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let session_id = request.payload.session_id.clone();
    let runtime_execution = match execute_runtime_private_inference(
        &app,
        &session_id,
        SoracloudPrivateInferenceExecutionAction::Release {
            decrypt_request_id: request.payload.decrypt_request_id.clone(),
        },
    ) {
        Ok(execution) => execution,
        Err(error_message) => {
            return SoracloudError::conflict(error_message).into_response();
        }
    };
    soracloud_draft_response(
        &signer,
        vec![InstructionBox::from(
            isi::soracloud::RecordSoracloudPrivateInferenceCheckpoint {
                session_id,
                status: runtime_execution.status,
                receipt_root: runtime_execution.receipt_root,
                xor_cost_nanos: runtime_execution.xor_cost_nanos,
                checkpoint: runtime_execution.checkpoint,
            },
        )],
    )
}

pub(crate) async fn handle_hf_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedHfDeployRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/hf/deploy").await {
        return err.into_response();
    }

    if let Err(err) = verify_hf_deploy_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_resolved_revision(request.payload.revision.as_deref()) {
        Ok(resolved_revision) => resolved_revision,
        Err(err) => return err.into_response(),
    };
    let model_name = match parse_hf_model_name(&request.payload.model_name) {
        Ok(model_name) => model_name,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let apartment_name = match request
        .payload
        .apartment_name
        .as_deref()
        .map(|value| value.trim().parse::<Name>())
        .transpose()
    {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let apartment_label = apartment_name.as_ref().map(ToString::to_string);
    let storage_class = request.payload.storage_class;
    let lease_term_ms = request.payload.lease_term_ms;
    let lease_asset_definition_id = request.payload.lease_asset_definition_id.clone();
    let base_fee_nanos = request.payload.base_fee_nanos;
    let source_id = match hf_source_id(&repo_id, &resolved_revision) {
        Ok(source_id) => source_id,
        Err(err) => return err.into_response(),
    };
    let resource_profile = match derive_hf_resource_profile(&repo_id, &resolved_revision).await {
        Ok(resource_profile) => resource_profile,
        Err(err) => return err.into_response(),
    };
    let generated_bundle = build_soracloud_hf_generated_service_bundle(
        service_name.clone(),
        &source_id.to_string(),
        &repo_id,
        &resolved_revision,
        &model_name,
    );
    let generated_apartment_manifest = apartment_name
        .clone()
        .map(|name| build_soracloud_hf_generated_agent_manifest(name, &generated_bundle));
    let mut instructions = Vec::new();
    match ensure_hf_generated_service_instruction(
        &app,
        &signer,
        &generated_bundle,
        &source_id,
        &repo_id,
        &resolved_revision,
        &model_name,
        request.generated_service_provenance.as_ref(),
    ) {
        Ok(Some(instruction)) => instructions.push(instruction),
        Ok(None) => {}
        Err(err) => return err.into_response(),
    }
    if let Some(manifest) = generated_apartment_manifest.as_ref() {
        match ensure_hf_generated_agent_instruction(
            &app,
            &signer,
            manifest,
            request.generated_apartment_provenance.as_ref(),
        ) {
            Ok(Some(instruction)) => instructions.push(instruction),
            Ok(None) => {}
            Err(err) => return err.into_response(),
        }
    }
    instructions.push(InstructionBox::from(
        isi::soracloud::JoinSoracloudHfSharedLease {
            repo_id: repo_id.clone(),
            resolved_revision: resolved_revision.clone(),
            model_name,
            service_name,
            apartment_name,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee_nanos,
            resource_profile: Some(resource_profile),
            provenance: request.provenance,
        },
    ));

    match submit_confirm_and_respond_instructions(
        &app,
        signer,
        instructions,
        "/v1/soracloud/hf/deploy",
        move |app, baseline| {
            authoritative_hf_shared_lease_mutation_response(
                app,
                baseline,
                &repo_id,
                &resolved_revision,
                storage_class,
                lease_term_ms,
                &authority,
                Some(service_label.as_str()),
                apartment_label.as_deref(),
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_hf_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<HfSharedLeaseStatusQuery>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/hf/status").await {
        return err.into_response();
    }

    let repo_id = match parse_hf_repo_id(&query.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_resolved_revision(query.revision.as_deref()) {
        Ok(resolved_revision) => resolved_revision,
        Err(err) => return err.into_response(),
    };
    let storage_class = match parse_storage_class_query(&query.storage_class) {
        Ok(storage_class) => storage_class,
        Err(err) => return err.into_response(),
    };
    let account_id = match parse_optional_account_id(query.account_id.as_deref()) {
        Ok(account_id) => account_id,
        Err(err) => return err.into_response(),
    };

    match authoritative_hf_shared_lease_status_response(
        &app,
        &repo_id,
        &resolved_revision,
        storage_class,
        query.lease_term_ms,
        account_id.as_ref(),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_hf_lease_leave(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedHfLeaseLeaveRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/hf/lease/leave").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_hf_lease_leave_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_resolved_revision(request.payload.revision.as_deref()) {
        Ok(resolved_revision) => resolved_revision,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_optional_service_name(request.payload.service_name.as_deref()) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let apartment_name = match request
        .payload
        .apartment_name
        .as_deref()
        .map(|value| value.trim().parse::<Name>())
        .transpose()
    {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.as_ref().map(ToString::to_string);
    let apartment_label = apartment_name.as_ref().map(ToString::to_string);
    let storage_class = request.payload.storage_class;
    let lease_term_ms = request.payload.lease_term_ms;

    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::LeaveSoracloudHfSharedLease {
            repo_id: repo_id.clone(),
            resolved_revision: resolved_revision.clone(),
            storage_class,
            lease_term_ms,
            service_name,
            apartment_name,
            provenance: request.provenance,
        }),
        "/v1/soracloud/hf/lease/leave",
        move |app, baseline| {
            authoritative_hf_shared_lease_mutation_response(
                app,
                baseline,
                &repo_id,
                &resolved_revision,
                storage_class,
                lease_term_ms,
                &authority,
                service_label.as_deref(),
                apartment_label.as_deref(),
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_hf_lease_renew(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedHfLeaseRenewRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/hf/lease/renew").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_hf_lease_renew_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_resolved_revision(request.payload.revision.as_deref()) {
        Ok(resolved_revision) => resolved_revision,
        Err(err) => return err.into_response(),
    };
    let model_name = match parse_hf_model_name(&request.payload.model_name) {
        Ok(model_name) => model_name,
        Err(err) => return err.into_response(),
    };
    let service_name = match parse_service_name(&request.payload.service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let apartment_name = match request
        .payload
        .apartment_name
        .as_deref()
        .map(|value| value.trim().parse::<Name>())
        .transpose()
    {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let service_label = service_name.to_string();
    let apartment_label = apartment_name.as_ref().map(ToString::to_string);
    let storage_class = request.payload.storage_class;
    let lease_term_ms = request.payload.lease_term_ms;
    let lease_asset_definition_id = request.payload.lease_asset_definition_id.clone();
    let base_fee_nanos = request.payload.base_fee_nanos;
    let source_id = match hf_source_id(&repo_id, &resolved_revision) {
        Ok(source_id) => source_id,
        Err(err) => return err.into_response(),
    };
    let resource_profile = match derive_hf_resource_profile(&repo_id, &resolved_revision).await {
        Ok(resource_profile) => resource_profile,
        Err(err) => return err.into_response(),
    };
    let generated_bundle = build_soracloud_hf_generated_service_bundle(
        service_name.clone(),
        &source_id.to_string(),
        &repo_id,
        &resolved_revision,
        &model_name,
    );
    let generated_apartment_manifest = apartment_name
        .clone()
        .map(|name| build_soracloud_hf_generated_agent_manifest(name, &generated_bundle));
    let mut instructions = Vec::new();
    match ensure_hf_generated_service_instruction(
        &app,
        &signer,
        &generated_bundle,
        &source_id,
        &repo_id,
        &resolved_revision,
        &model_name,
        request.generated_service_provenance.as_ref(),
    ) {
        Ok(Some(instruction)) => instructions.push(instruction),
        Ok(None) => {}
        Err(err) => return err.into_response(),
    }
    if let Some(manifest) = generated_apartment_manifest.as_ref() {
        match ensure_hf_generated_agent_instruction(
            &app,
            &signer,
            manifest,
            request.generated_apartment_provenance.as_ref(),
        ) {
            Ok(Some(instruction)) => instructions.push(instruction),
            Ok(None) => {}
            Err(err) => return err.into_response(),
        }
    }
    instructions.push(InstructionBox::from(
        isi::soracloud::RenewSoracloudHfSharedLease {
            repo_id: repo_id.clone(),
            resolved_revision: resolved_revision.clone(),
            model_name,
            service_name,
            apartment_name,
            storage_class,
            lease_term_ms,
            lease_asset_definition_id,
            base_fee_nanos,
            resource_profile: Some(resource_profile),
            provenance: request.provenance,
        },
    ));

    match submit_confirm_and_respond_instructions(
        &app,
        signer,
        instructions,
        "/v1/soracloud/hf/lease/renew",
        move |app, baseline| {
            authoritative_hf_shared_lease_mutation_response(
                app,
                baseline,
                &repo_id,
                &resolved_revision,
                storage_class,
                lease_term_ms,
                &authority,
                Some(service_label.as_str()),
                apartment_label.as_deref(),
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_host_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<ModelHostStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model-host/status").await
    {
        return err.into_response();
    }

    let validator_account_id = match parse_optional_account_id(query.account_id.as_deref()) {
        Ok(account_id) => account_id,
        Err(err) => return err.into_response(),
    };
    JsonBody(authoritative_model_host_status_response(
        &app,
        validator_account_id.as_ref(),
    ))
    .into_response()
}

pub(crate) async fn handle_model_host_advertise(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelHostAdvertiseRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model-host/advertise").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_host_advertise_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let validator_account_id = request.payload.capability.validator_account_id.clone();
    let signed_by = signer.authority.to_string();

    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AdvertiseSoracloudModelHost {
            capability: request.payload.capability,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model-host/advertise",
        move |app, _baseline| {
            Ok(ModelHostMutationResponse {
                action: ModelHostMutationAction::Advertise,
                status: authoritative_model_host_status_response(app, Some(&validator_account_id)),
                signed_by: signed_by.clone(),
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_host_heartbeat(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelHostHeartbeatRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model-host/heartbeat").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_host_heartbeat_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let validator_account_id = request.payload.validator_account_id.clone();
    let heartbeat_expires_at_ms = request.payload.heartbeat_expires_at_ms;
    let signed_by = signer.authority.to_string();

    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::HeartbeatSoracloudModelHost {
            validator_account_id: validator_account_id.clone(),
            heartbeat_expires_at_ms,
            provenance: request.provenance,
        }),
        "/v1/soracloud/model-host/heartbeat",
        move |app, _baseline| {
            Ok(ModelHostMutationResponse {
                action: ModelHostMutationAction::Heartbeat,
                status: authoritative_model_host_status_response(app, Some(&validator_account_id)),
                signed_by: signed_by.clone(),
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_model_host_withdraw(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedModelHostWithdrawRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/model-host/withdraw").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_model_host_withdraw_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let validator_account_id = request.payload.validator_account_id.clone();
    let signed_by = signer.authority.to_string();

    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::WithdrawSoracloudModelHost {
            validator_account_id: validator_account_id.clone(),
            provenance: request.provenance,
        }),
        "/v1/soracloud/model-host/withdraw",
        move |app, _baseline| {
            Ok(ModelHostMutationResponse {
                action: ModelHostMutationAction::Withdraw,
                status: authoritative_model_host_status_response(app, Some(&validator_account_id)),
                signed_by: signed_by.clone(),
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentDeployRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/agent/deploy").await {
        return err.into_response();
    }

    if let Err(err) = verify_agent_deploy_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let autonomy_budget_units = request
        .payload
        .autonomy_budget_units
        .unwrap_or(AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS);
    let apartment_name = request.payload.manifest.apartment_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::DeploySoracloudAgentApartment {
            manifest: request.payload.manifest,
            lease_ticks: request.payload.lease_ticks,
            autonomy_budget_units,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/deploy",
        move |app, baseline| {
            authoritative_agent_deploy_mutation_response(app, baseline, &apartment_name)
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_lease_renew(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentLeaseRenewRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/lease/renew").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_lease_renew_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RenewSoracloudAgentLease {
            apartment_name,
            lease_ticks: request.payload.lease_ticks,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/lease/renew",
        move |app, baseline| {
            authoritative_agent_lease_renew_mutation_response(app, baseline, &apartment_label)
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_restart(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentRestartRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/agent/restart").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_restart_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RestartSoracloudAgentApartment {
            apartment_name,
            reason: request.payload.reason,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/restart",
        move |app, baseline| {
            authoritative_agent_restart_mutation_response(app, baseline, &apartment_label)
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<AgentStatusQuery>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/agent/status").await {
        return err.into_response();
    }

    match authoritative_agent_status_response(&app, query.apartment_name.as_deref()) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_wallet_spend(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentWalletSpendRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/wallet/spend").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_wallet_spend_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let asset_definition = request.payload.asset_definition.clone();
    let amount_nanos = request.payload.amount_nanos;
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RequestSoracloudAgentWalletSpend {
            apartment_name,
            asset_definition: request.payload.asset_definition,
            amount_nanos: request.payload.amount_nanos,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/wallet/spend",
        move |app, baseline| {
            authoritative_agent_wallet_request_mutation_response(
                app,
                baseline,
                &apartment_label,
                &asset_definition,
                amount_nanos,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_wallet_approve(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentWalletApproveRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/wallet/approve").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_wallet_approve_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let request_id = request.payload.request_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::ApproveSoracloudAgentWalletSpend {
            apartment_name,
            request_id: request.payload.request_id,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/wallet/approve",
        move |app, baseline| {
            authoritative_agent_wallet_approve_mutation_response(
                app,
                baseline,
                &apartment_label,
                &request_id,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_policy_revoke(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentPolicyRevokeRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/policy/revoke").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_policy_revoke_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let capability = request.payload.capability.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RevokeSoracloudAgentPolicy {
            apartment_name,
            capability: request.payload.capability,
            reason: request.payload.reason,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/policy/revoke",
        move |app, baseline| {
            authoritative_agent_policy_revoke_mutation_response(
                app,
                baseline,
                &apartment_label,
                &capability,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_message_send(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentMessageSendRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/message/send").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_message_send_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let from_apartment: Name = match request.payload.from_apartment.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid from_apartment: {err}"))
                .into_response();
        }
    };
    let to_apartment: Name = match request.payload.to_apartment.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid to_apartment: {err}"))
                .into_response();
        }
    };
    let from_apartment_label = from_apartment.to_string();
    let to_apartment_label = to_apartment.to_string();
    let channel = request.payload.channel.clone();
    let payload = request.payload.payload.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::EnqueueSoracloudAgentMessage {
            from_apartment,
            to_apartment,
            channel: request.payload.channel,
            payload: request.payload.payload,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/message/send",
        move |app, baseline| {
            authoritative_agent_message_send_mutation_response(
                app,
                baseline,
                &from_apartment_label,
                &to_apartment_label,
                &channel,
                &payload,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_message_ack(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentMessageAckRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/message/ack").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_message_ack_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let message_id = request.payload.message_id.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AcknowledgeSoracloudAgentMessage {
            apartment_name,
            message_id: request.payload.message_id,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/message/ack",
        move |app, baseline| {
            authoritative_agent_message_ack_mutation_response(
                app,
                baseline,
                &apartment_label,
                &message_id,
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_mailbox_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<AgentMailboxStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/mailbox/status").await
    {
        return err.into_response();
    }

    match authoritative_agent_mailbox_status_response(&app, &query.apartment_name) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_agent_autonomy_allow(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentArtifactAllowRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/autonomy/allow").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_artifact_allow_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let apartment_label = apartment_name.to_string();
    let artifact_hash = request.payload.artifact_hash.clone();
    let provenance_hash = request.payload.provenance_hash.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::AllowSoracloudAgentAutonomyArtifact {
            apartment_name,
            artifact_hash: request.payload.artifact_hash,
            provenance_hash: request.payload.provenance_hash,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/autonomy/allow",
        move |app, baseline| {
            authoritative_agent_artifact_allow_mutation_response(
                app,
                baseline,
                &apartment_label,
                &artifact_hash,
                provenance_hash.as_deref(),
            )
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

fn execute_runtime_agent_autonomy_run(
    app: &SharedAppState,
    response: &AgentAutonomyMutationResponse,
) -> Result<Option<AgentRuntimeExecutionSummary>, String> {
    let Some(runtime) = app.soracloud_runtime.as_ref() else {
        return Ok(None);
    };
    let Some(run_id) = response.run_id.as_deref() else {
        return Ok(None);
    };

    let state_view = app.state.view();
    let record = state_view
        .world()
        .soracloud_agent_apartments()
        .get(response.apartment_name.as_str())
        .cloned()
        .ok_or_else(|| {
            format!(
                "apartment `{}` was committed without a readable authoritative record",
                response.apartment_name
            )
        })?;
    if !record
        .manifest
        .tool_capabilities
        .iter()
        .any(|capability| capability.tool == "soracloud.hf.infer")
    {
        return Ok(None);
    }
    let approved_run = record
        .autonomy_run_history
        .iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| {
            format!(
                "apartment `{}` does not contain approved run `{run_id}` after commit",
                response.apartment_name
            )
        })?;
    let observed_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let observed_block_hash = state_view.latest_block_hash().map(Hash::from);
    drop(state_view);

    let request = SoracloudApartmentExecutionRequest {
        observed_height,
        observed_block_hash,
        apartment_name: response.apartment_name.clone(),
        process_generation: record.process_generation,
        operation: format!("autonomy-run:{run_id}"),
        request_commitment: approved_run.request_commitment,
    };
    runtime.execute_apartment(request).map_err(|error| {
        format!(
            "runtime execution for apartment `{}` run `{run_id}` failed: {}",
            response.apartment_name, error.message
        )
    })?;
    read_agent_runtime_execution_summary(
        runtime.state_dir().as_path(),
        &response.apartment_name,
        run_id,
    )
    .map_err(|error| error.message)
}

fn private_inference_runtime_request_commitment(
    session_id: &str,
    action: &SoracloudPrivateInferenceExecutionAction,
) -> Hash {
    let (action_label, decrypt_request_id) = match action {
        SoracloudPrivateInferenceExecutionAction::Start => ("start", None),
        SoracloudPrivateInferenceExecutionAction::Release { decrypt_request_id } => {
            ("release", Some(decrypt_request_id.as_str()))
        }
    };
    Hash::new(
        norito::to_bytes(&(session_id, action_label, decrypt_request_id))
            .expect("private inference runtime request commitment encoding should be infallible"),
    )
}

fn execute_runtime_private_inference(
    app: &SharedAppState,
    session_id: &str,
    action: SoracloudPrivateInferenceExecutionAction,
) -> Result<SoracloudPrivateInferenceExecutionResult, String> {
    let runtime = app
        .soracloud_runtime
        .as_ref()
        .ok_or_else(|| "private uploaded-model runtime is not available".to_owned())?;
    let state_view = app.state.view();
    let session = state_view
        .world()
        .soracloud_private_inference_sessions()
        .iter()
        .find_map(|((_apartment_name, stored_session_id), session)| {
            (stored_session_id == session_id).then(|| session.clone())
        })
        .ok_or_else(|| {
            format!("private session `{session_id}` was committed without a readable authoritative record")
        })?;
    let apartment_name = session.apartment.to_string();
    let apartment_record = state_view
        .world()
        .soracloud_agent_apartments()
        .get(apartment_name.as_str())
        .cloned()
        .ok_or_else(|| {
            format!(
                "private session `{session_id}` apartment `{apartment_name}` is missing from authoritative Soracloud state"
            )
        })?;
    let observed_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let observed_block_hash = state_view.latest_block_hash().map(Hash::from);
    drop(state_view);

    let request = SoracloudPrivateInferenceExecutionRequest {
        observed_height,
        observed_block_hash,
        apartment_name: apartment_name.clone(),
        process_generation: apartment_record.process_generation,
        session_id: session_id.to_owned(),
        request_commitment: private_inference_runtime_request_commitment(session_id, &action),
        action,
    };
    runtime.execute_private_inference(request).map_err(|error| {
        format!(
            "runtime execution for private session `{session_id}` apartment `{apartment_name}` failed: {}",
            error.message
        )
    })
}

fn failed_private_inference_runtime_result(
    app: &SharedAppState,
    session_id: &str,
    error_message: &str,
) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let session = world
        .soracloud_private_inference_sessions()
        .iter()
        .find_map(|((_apartment_name, stored_session_id), session)| {
            (stored_session_id == session_id).then(|| session.clone())
        })
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "private session `{session_id}` not found in authoritative Soracloud state"
            ))
        })?;
    let bundle_key = (
        session.service_name.as_ref().to_owned(),
        session.model_id.clone(),
        session.weight_version.clone(),
    );
    let bundle = world
        .soracloud_uploaded_model_bundles()
        .get(&bundle_key)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "uploaded model bundle `{}` version `{}` not found for session `{session_id}`",
                session.model_id, session.weight_version
            ))
        })?;
    let latest_step = world
        .soracloud_private_inference_checkpoints()
        .iter()
        .filter(|((stored_session_id, _step), _checkpoint)| stored_session_id == session_id)
        .map(|((_stored_session_id, step), _checkpoint)| *step)
        .max()
        .unwrap_or(0);
    let step = latest_step.saturating_add(1);
    let compute_units = u64::from(session.token_budget).max(1).min(16);
    let updated_at_ms = state_view
        .latest_block()
        .map(|block| u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1)
        .max(1);
    let ciphertext_state_root = Hash::new(Encode::encode(&(
        "soracloud.private.failure.ciphertext.v1",
        session.session_id.as_str(),
        session.bundle_root,
        step,
        error_message,
    )));
    let receipt_hash = Hash::new(Encode::encode(&(
        "soracloud.private.failure.receipt.v1",
        session.session_id.as_str(),
        step,
        error_message,
        ciphertext_state_root,
    )));
    let xor_cost_nanos = session.xor_cost_nanos.saturating_add(
        bundle
            .pricing_policy
            .runtime_step_xor_nanos
            .saturating_mul(u128::from(compute_units)),
    );
    let receipt_root = Hash::new(Encode::encode(&(
        "soracloud.private.failure.root.v1",
        session.session_id.as_str(),
        step,
        error_message,
        xor_cost_nanos,
        receipt_hash,
    )));
    let checkpoint = SoraPrivateInferenceCheckpointV1 {
        schema_version: iroha_data_model::soracloud::SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
        session_id: session.session_id.clone(),
        step,
        ciphertext_state_root,
        receipt_hash,
        decrypt_request_id: format!("{session_id}:failed:{step}"),
        released_token: None,
        compute_units,
        updated_at_ms,
    };
    Ok(SoracloudPrivateInferenceExecutionResult {
        status: SoraPrivateInferenceSessionStatusV1::Failed,
        receipt_root,
        xor_cost_nanos,
        result_commitment: Hash::new(Encode::encode(&(
            "soracloud.private.failure.result.v1",
            session.session_id.as_str(),
            error_message,
            receipt_root,
            xor_cost_nanos,
            checkpoint.clone(),
        ))),
        checkpoint,
    })
}

fn build_authoritative_agent_runtime_receipt_instruction(
    app: &SharedAppState,
    runtime_execution: &AgentRuntimeExecutionSummary,
) -> Result<Option<InstructionBox>, String> {
    let Some(runtime_receipt) = runtime_execution.runtime_receipt.as_ref() else {
        return Ok(None);
    };
    {
        let state_view = app.state.view();
        if state_view
            .world()
            .soracloud_runtime_receipts()
            .get(&runtime_receipt.receipt_id)
            .is_some()
        {
            return Ok(None);
        }
    }
    Ok(Some(InstructionBox::from(
        isi::soracloud::RecordSoracloudRuntimeReceipt {
            receipt: SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: runtime_receipt.receipt_id,
                service_name: runtime_receipt
                    .service_name
                    .parse()
                    .map_err(|error| format!("invalid runtime receipt service name: {error}"))?,
                service_version: runtime_receipt.service_version.clone(),
                handler_name: runtime_receipt
                    .handler_name
                    .parse()
                    .map_err(|error| format!("invalid runtime receipt handler name: {error}"))?,
                handler_class: runtime_receipt.handler_class,
                request_commitment: runtime_receipt.request_commitment,
                result_commitment: runtime_receipt.result_commitment,
                certified_by: runtime_receipt.certified_by,
                emitted_sequence: runtime_receipt.emitted_sequence,
                placement_id: runtime_receipt.placement_id,
                selected_validator_account_id: runtime_receipt
                    .selected_validator_account_id
                    .clone(),
                selected_peer_id: runtime_receipt.selected_peer_id.clone(),
                mailbox_message_id: None,
                journal_artifact_hash: runtime_receipt.journal_artifact_hash,
                checkpoint_artifact_hash: runtime_receipt.checkpoint_artifact_hash,
            },
        },
    )))
}

fn build_authoritative_agent_autonomy_execution_audit_instruction(
    app: &SharedAppState,
    apartment_name: &str,
    process_generation: u64,
    runtime_execution: &AgentRuntimeExecutionSummary,
    runtime_receipt_id: Option<Hash>,
) -> Result<Option<InstructionBox>, String> {
    {
        let state_view = app.state.view();
        if state_view
            .world()
            .soracloud_agent_apartment_audit_events()
            .iter()
            .filter_map(|(_sequence, event)| {
                (event.action == SoraAgentApartmentActionV1::AutonomyRunExecuted
                    && event.apartment_name.as_ref() == apartment_name
                    && event.run_id.as_deref() == Some(runtime_execution.run_id.as_str()))
                .then_some(event)
            })
            .max_by_key(|event| event.sequence)
            .filter(|event| event.result_commitment == Some(runtime_execution.result_commitment))
            .is_some()
        {
            return Ok(None);
        }
    }

    Ok(Some(InstructionBox::from(
        isi::soracloud::RecordSoracloudAgentAutonomyExecution {
            apartment_name: apartment_name.parse().map_err(|error| {
                format!("invalid apartment execution audit apartment name: {error}")
            })?,
            run_id: runtime_execution.run_id.clone(),
            process_generation,
            succeeded: runtime_execution.succeeded,
            result_commitment: runtime_execution.result_commitment,
            service_name: runtime_execution
                .service_name
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(|error| {
                    format!("invalid apartment execution audit service name: {error}")
                })?,
            service_version: runtime_execution.service_version.clone(),
            handler_name: runtime_execution
                .handler_name
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(|error| {
                    format!("invalid apartment execution audit handler name: {error}")
                })?,
            runtime_receipt_id: runtime_receipt_id.or_else(|| {
                runtime_execution
                    .runtime_receipt
                    .as_ref()
                    .map(|receipt| receipt.receipt_id)
            }),
            journal_artifact_hash: Some(runtime_execution.journal_artifact_hash),
            checkpoint_artifact_hash: runtime_execution.checkpoint_artifact_hash,
            error: runtime_execution.error.clone(),
        },
    )))
}

pub(crate) async fn handle_agent_autonomy_run(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedAgentAutonomyRunRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/autonomy/run").await
    {
        return err.into_response();
    }

    if let Err(err) = verify_agent_autonomy_run_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(
        &headers,
        &request.provenance,
        request.authority,
        request.private_key,
    ) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.payload.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    match submit_confirm_and_respond::<(), _>(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RunSoracloudAgentAutonomy {
            apartment_name,
            artifact_hash: request.payload.artifact_hash,
            provenance_hash: request.payload.provenance_hash,
            budget_units: request.payload.budget_units,
            run_label: request.payload.run_label,
            workflow_input_json: request.payload.workflow_input_json,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/autonomy/run",
        |_app, _baseline| {
            Err(SoracloudError::internal(
                "agent autonomy approval returns a draft response".to_owned(),
            ))
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return err.into_response(),
    }
}

pub(crate) async fn handle_agent_autonomy_run_finalize(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<AgentAutonomyFinalizeRequest>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/autonomy/run").await
    {
        return err.into_response();
    }
    let signer = match require_soracloud_request_signer(&headers) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let apartment_name: Name = match request.apartment_name.parse() {
        Ok(apartment_name) => apartment_name,
        Err(err) => {
            return SoracloudError::bad_request(format!("invalid apartment_name: {err}"))
                .into_response();
        }
    };
    let run_id = request.run_id.trim();
    if run_id.is_empty() {
        return SoracloudError::bad_request("run_id must not be empty").into_response();
    }

    let response = {
        let state_view = app.state.view();
        let world = state_view.world();
        let record = match world
            .soracloud_agent_apartments()
            .get(apartment_name.as_ref())
            .cloned()
        {
            Some(record) => record,
            None => {
                return SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in authoritative Soracloud state"
                ))
                .into_response();
            }
        };
        let run = match record
            .autonomy_run_history
            .iter()
            .find(|run| run.run_id == run_id)
            .cloned()
        {
            Some(run) => run,
            None => {
                return SoracloudError::not_found(format!(
                    "approved autonomy run `{run_id}` not found for apartment `{apartment_name}`"
                ))
                .into_response();
            }
        };
        let event = match world
            .soracloud_agent_apartment_audit_events()
            .get(&run.approved_sequence)
            .cloned()
            .filter(|event| {
                event.action == SoraAgentApartmentActionV1::AutonomyRunApproved
                    && event.apartment_name == apartment_name
                    && event.run_id.as_deref() == Some(run_id)
            }) {
            Some(event) => event,
            None => {
                return SoracloudError::conflict(format!(
                    "autonomy approval audit event for apartment `{apartment_name}` run `{run_id}` is missing from authoritative state"
                ))
                .into_response();
            }
        };
        if event.signer != signer.request_signer {
            return SoracloudError::unauthorized(
                "agent autonomy finalize signer must match the original run approval signer",
            )
            .into_response();
        }
        match authoritative_agent_autonomy_mutation_response(&app, &record, &event) {
            Ok(response) => response,
            Err(err) => return err.into_response(),
        }
    };

    let runtime_execution = match execute_runtime_agent_autonomy_run(&app, &response) {
        Ok(runtime_execution) => runtime_execution,
        Err(error) => return SoracloudError::conflict(error).into_response(),
    };
    let Some(runtime_execution) = runtime_execution else {
        return soracloud_draft_response(&signer, Vec::new());
    };
    let runtime_receipt_id = runtime_execution
        .runtime_receipt
        .as_ref()
        .map(|receipt| receipt.receipt_id);
    let mut instructions = Vec::new();
    match build_authoritative_agent_runtime_receipt_instruction(&app, &runtime_execution) {
        Ok(Some(instruction)) => instructions.push(instruction),
        Ok(None) => {}
        Err(error) => return SoracloudError::conflict(error).into_response(),
    }
    match build_authoritative_agent_autonomy_execution_audit_instruction(
        &app,
        response.apartment_name.as_str(),
        response.process_generation,
        &runtime_execution,
        runtime_receipt_id,
    ) {
        Ok(Some(instruction)) => instructions.push(instruction),
        Ok(None) => {}
        Err(error) => return SoracloudError::conflict(error).into_response(),
    }
    soracloud_draft_response(&signer, instructions)
}

pub(crate) async fn handle_agent_autonomy_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<AgentAutonomyStatusQuery>,
) -> Response {
    if let Err(err) =
        crate::check_access(&app, &headers, None, "v1/soracloud/agent/autonomy/status").await
    {
        return err.into_response();
    }

    match authoritative_agent_autonomy_status_response(&app, &query.apartment_name) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_health_compliance_report(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<HealthComplianceReportQuery>,
) -> Response {
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        None,
        "v1/soracloud/health/compliance/report",
    )
    .await
    {
        return err.into_response();
    }

    let limit = query
        .limit
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_HEALTH_COMPLIANCE_LIMIT)
        .max(1);
    match authoritative_health_compliance_report(
        &app,
        query.service_name.as_deref(),
        query.jurisdiction_tag.as_deref(),
        limit,
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        fs,
        num::{NonZeroU32, NonZeroU64},
        path::{Path, PathBuf},
        sync::Arc,
    };

    use iroha_core::soracloud_runtime::{
        SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
        SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentExecutionRequest,
        SoracloudApartmentExecutionResult, SoracloudLocalReadRequest, SoracloudLocalReadResponse,
        SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
        SoracloudRuntime, SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
        SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
    };
    use iroha_crypto::{KeyPair, Signature};
    use iroha_data_model::{
        Encode,
        account::{Account, AccountId},
        asset::AssetDefinitionId,
        domain::Domain,
        isi::Grant,
        name::Name,
        permission::Permission,
        prelude::Register,
        soracloud::{
            AgentApartmentManifestV1, CiphertextQueryMetadataLevelV1, CiphertextQuerySpecV1,
            DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheExecutionPolicyV1, FheJobSpecV1,
            FheParamSetV1, SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
            SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            SORA_HF_SOURCE_RECORD_VERSION_V1, SecretEnvelopeEncryptionV1, SecretEnvelopeV1,
            SoraAgentApartmentActionV1, SoraAgentApartmentAuditEventV1,
            SoraAgentAutonomyRunRecordV1, SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1,
            SoraContainerManifestV1, SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1,
            SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1,
            SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1, SoraHfSourceStatusV1,
            SoraModelPrivacyModeV1, SoraModelProvenanceKindV1, SoraModelProvenanceRefV1,
            SoraPrivateCompileProfileV1, SoraPrivateInferenceCheckpointV1,
            SoraPrivateInferenceSessionStatusV1, SoraPrivateInferenceSessionV1,
            SoraServiceAuditEventV1, SoraServiceConfigEntryV1, SoraServiceDeploymentStateV1,
            SoraServiceLifecycleActionV1, SoraServiceManifestV1, SoraServiceSecretEntryV1,
            SoraServiceStateEntryV1, SoraStateEncryptionV1, SoraUploadedModelBundleV1,
            SoraUploadedModelChunkV1, SoraUploadedModelPricingPolicyV1,
            SoraUploadedModelRuntimeFormatV1,
        },
        sorafs::pin_registry::StorageClass,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, SAMPLE_GENESIS_ACCOUNT_ID};

    use crate::tests_runtime_handlers::mk_app_state_for_tests_with_world;

    struct TestHfRuntimeHandle {
        snapshot: SoracloudRuntimeSnapshot,
        state_dir: PathBuf,
    }

    impl SoracloudRuntimeReadHandle for TestHfRuntimeHandle {
        fn snapshot(&self) -> SoracloudRuntimeSnapshot {
            self.snapshot.clone()
        }

        fn state_dir(&self) -> PathBuf {
            self.state_dir.clone()
        }
    }

    impl SoracloudRuntime for TestHfRuntimeHandle {
        fn execute_local_read(
            &self,
            _request: SoracloudLocalReadRequest,
        ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
            Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test hf runtime handle exposes only the runtime snapshot",
            ))
        }

        fn execute_ordered_mailbox(
            &self,
            _request: SoracloudOrderedMailboxExecutionRequest,
        ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError>
        {
            Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test hf runtime handle exposes only the runtime snapshot",
            ))
        }

        fn execute_apartment(
            &self,
            _request: SoracloudApartmentExecutionRequest,
        ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
            Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test hf runtime handle exposes only the runtime snapshot",
            ))
        }

        fn execute_private_inference(
            &self,
            _request: SoracloudPrivateInferenceExecutionRequest,
        ) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError>
        {
            Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test hf runtime handle exposes only the runtime snapshot",
            ))
        }
    }

    fn workspace_fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }

    fn load_json<T>(path: &Path) -> T
    where
        T: norito::json::JsonDeserialize,
    {
        let bytes = fs::read(path).expect("read fixture");
        norito::json::from_slice(&bytes).expect("decode fixture")
    }

    fn fixture_bundle(version: &str) -> SoraDeploymentBundleV1 {
        let container: SoraContainerManifestV1 = load_json(&workspace_fixture(
            "fixtures/soracloud/sora_container_manifest_v1.json",
        ));
        let mut service: SoraServiceManifestV1 = load_json(&workspace_fixture(
            "fixtures/soracloud/sora_service_manifest_v1.json",
        ));
        service.service_version = version.to_string();
        service.container.manifest_hash = Hash::new(Encode::encode(&container));
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        }
    }

    fn fixture_bundle_with_training(
        version: &str,
        allow_model_training: bool,
    ) -> SoraDeploymentBundleV1 {
        let mut bundle = fixture_bundle(version);
        bundle.container.capabilities.allow_model_training = allow_model_training;
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        bundle
    }

    fn fixture_fhe_param_set() -> FheParamSetV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/fhe_param_set_v1.json",
        ))
    }

    fn fixture_fhe_execution_policy() -> FheExecutionPolicyV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/fhe_execution_policy_v1.json",
        ))
    }

    fn fixture_fhe_job_spec() -> FheJobSpecV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/fhe_job_spec_v1.json",
        ))
    }

    fn fixture_decryption_authority_policy() -> DecryptionAuthorityPolicyV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/decryption_authority_policy_v1.json",
        ))
    }

    fn fixture_decryption_request() -> DecryptionRequestV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/decryption_request_v1.json",
        ))
    }

    fn fixture_ciphertext_query_spec() -> CiphertextQuerySpecV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/ciphertext_query_spec_v1.json",
        ))
    }

    fn hf_shared_lease_asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("valid asset definition")
    }

    fn signed_bundle_request(
        bundle: SoraDeploymentBundleV1,
        key_pair: &KeyPair,
    ) -> SignedBundleRequest {
        let initial_service_configs = BTreeMap::new();
        let initial_service_secrets = BTreeMap::new();
        let payload = encode_bundle_signature_payload(
            &bundle,
            &initial_service_configs,
            &initial_service_secrets,
        )
        .expect("encode bundle payload");
        let signature = Signature::new(key_pair.private_key(), &payload);
        SignedBundleRequest {
            bundle,
            initial_service_configs,
            initial_service_secrets,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_rollback_request(
        service_name: &str,
        target_version: Option<&str>,
        key_pair: &KeyPair,
    ) -> SignedRollbackRequest {
        let payload = RollbackPayload {
            service_name: service_name.to_string(),
            target_version: target_version.map(ToOwned::to_owned),
        };
        let encoded = encode_rollback_signature_payload(&payload).expect("encode rollback payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedRollbackRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn verified_request_headers(account: &AccountId, signer: &PublicKey) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            VERIFIED_ACCOUNT_HEADER,
            account.to_string().parse().expect("valid account header"),
        );
        headers.insert(
            VERIFIED_SIGNER_HEADER,
            signer.to_string().parse().expect("valid signer header"),
        );
        headers
    }

    fn test_soracloud_mutation_signer(key_pair: &KeyPair) -> SoracloudMutationSigner {
        SoracloudMutationSigner {
            authority: AccountId::new(key_pair.public_key().clone()),
            request_signer: key_pair.public_key().clone(),
        }
    }

    fn signed_generated_service_provenance(
        bundle: &SoraDeploymentBundleV1,
        key_pair: &KeyPair,
    ) -> ManifestProvenance {
        let payload = encode_bundle_signature_payload(bundle, &BTreeMap::new(), &BTreeMap::new())
            .expect("bundle payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: Signature::new(key_pair.private_key(), &payload),
        }
    }

    fn signed_generated_apartment_provenance(
        manifest: &AgentApartmentManifestV1,
        key_pair: &KeyPair,
    ) -> ManifestProvenance {
        let payload = encode_agent_deploy_provenance_payload(
            manifest.clone(),
            HF_GENERATED_AGENT_LEASE_TICKS,
            Some(HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS),
        )
        .expect("agent deploy payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: Signature::new(key_pair.private_key(), &payload),
        }
    }

    fn fixture_agent_run_record(
        apartment_name: &str,
        run_id: &str,
        approved_sequence: u64,
        process_generation: u64,
    ) -> SoraAgentAutonomyRunRecordV1 {
        let artifact_hash = "hash:artifact#1".to_owned();
        let provenance_hash = Some("hash:prov#1".to_owned());
        let budget_units = 25;
        let run_label = "nightly".to_owned();
        let workflow_input_json = Some("{\"inputs\":\"nightly\"}".to_owned());
        let request_commitment =
            iroha_data_model::soracloud::derive_agent_autonomy_request_commitment(
                apartment_name,
                &artifact_hash,
                provenance_hash.as_deref(),
                budget_units,
                run_id,
                &run_label,
                workflow_input_json.as_deref(),
                process_generation,
            );
        SoraAgentAutonomyRunRecordV1 {
            run_id: run_id.to_owned(),
            artifact_hash,
            provenance_hash,
            budget_units,
            run_label,
            workflow_input_json,
            approved_process_generation: process_generation,
            request_commitment,
            approved_sequence,
        }
    }

    fn fixture_agent_apartment_record(
        manifest: AgentApartmentManifestV1,
        run: SoraAgentAutonomyRunRecordV1,
        process_generation: u64,
    ) -> SoraAgentApartmentRecordV1 {
        SoraAgentApartmentRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest_hash: Hash::new(Encode::encode(&manifest)),
            manifest,
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: 1,
            lease_started_sequence: 1,
            lease_expires_sequence: 100,
            last_renewed_sequence: 1,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation,
            process_started_sequence: 1,
            last_active_sequence: run.approved_sequence,
            last_checkpoint_sequence: None,
            checkpoint_count: 0,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 0,
                key_sizes: BTreeMap::new(),
            },
            revoked_policy_capabilities: BTreeSet::new(),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: 100,
            autonomy_budget_remaining_units: 75,
            uploaded_model_binding: None,
            artifact_allowlist: BTreeMap::new(),
            autonomy_run_history: vec![run],
        }
    }

    fn fixture_autonomy_approval_event(
        apartment_name: &str,
        manifest_hash: Hash,
        signer: &KeyPair,
        run: &SoraAgentAutonomyRunRecordV1,
    ) -> SoraAgentApartmentAuditEventV1 {
        SoraAgentApartmentAuditEventV1 {
            schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
            sequence: run.approved_sequence,
            action: SoraAgentApartmentActionV1::AutonomyRunApproved,
            apartment_name: apartment_name.parse().expect("valid apartment name"),
            status: SoraAgentRuntimeStatusV1::Running,
            lease_expires_sequence: 100,
            manifest_hash,
            restart_count: 0,
            signer: signer.public_key().clone(),
            request_id: Some(run.run_id.clone()),
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: None,
            from_apartment: None,
            to_apartment: None,
            channel: None,
            payload_hash: None,
            artifact_hash: Some(run.artifact_hash.clone()),
            provenance_hash: run.provenance_hash.clone(),
            run_id: Some(run.run_id.clone()),
            run_label: Some(run.run_label.clone()),
            budget_units: Some(run.budget_units),
            service_name: None,
            service_version: None,
            handler_name: None,
            result_commitment: None,
            runtime_receipt_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            succeeded: None,
        }
    }

    fn attach_test_runtime(app: &mut SharedAppState, state_dir: PathBuf) {
        Arc::get_mut(app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestHfRuntimeHandle {
            snapshot: SoracloudRuntimeSnapshot::default(),
            state_dir,
        }));
    }

    #[test]
    fn signed_mutation_request_matcher_targets_only_soracloud_posts() {
        assert!(requires_signed_mutation_request(
            &axum::http::Method::POST,
            "/v1/soracloud/deploy",
        ));
        assert!(!requires_signed_mutation_request(
            &axum::http::Method::GET,
            "/v1/soracloud/deploy",
        ));
        assert!(!requires_signed_mutation_request(
            &axum::http::Method::POST,
            "/v1/zk/attachments",
        ));
    }

    #[test]
    fn require_soracloud_mutation_signer_rejects_inline_signing_material() {
        let key_pair = KeyPair::random();
        let account = AccountId::new(key_pair.public_key().clone());
        let headers = verified_request_headers(&account, key_pair.public_key());
        let provenance = ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: Signature::new(key_pair.private_key(), b"mutation"),
        };

        let error = match require_soracloud_mutation_signer(
            &headers,
            &provenance,
            Some(account),
            Some(ExposedPrivateKey(key_pair.private_key().clone())),
        ) {
            Ok(_) => panic!("inline signing material must be rejected"),
            Err(error) => error,
        };

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(
            error
                .message
                .contains("authority/private_key fields are no longer accepted")
        );
    }

    #[test]
    fn require_soracloud_mutation_signer_binds_provenance_to_request_signer() {
        let request_keypair = KeyPair::random();
        let provenance_keypair = KeyPair::random();
        let account = AccountId::new(request_keypair.public_key().clone());
        let headers = verified_request_headers(&account, request_keypair.public_key());
        let provenance = ManifestProvenance {
            signer: provenance_keypair.public_key().clone(),
            signature: Signature::new(provenance_keypair.private_key(), b"mutation"),
        };

        let error = match require_soracloud_mutation_signer(&headers, &provenance, None, None) {
            Ok(_) => panic!("provenance signer mismatch must be rejected"),
            Err(error) => error,
        };

        assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn control_plane_snapshot_uses_authoritative_soracloud_state() -> Result<(), eyre::Report> {
        use iroha_core::{smartcontracts::Execute, state::World};
        use iroha_data_model::block::BlockHeader;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let app = mk_app_state_for_tests_with_world(World::default());
            let block_header = BlockHeader {
                height: NonZeroU64::new(1).expect("non-zero block height"),
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                prev_roster_evidence_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            };
            let mut state_block = app.state.block(block_header);
            let mut stx = state_block.transaction();
            let wonderland: iroha_data_model::domain::DomainId = "wonderland".parse()?;
            Register::domain(Domain::new(wonderland.clone()))
                .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Register::account(Account::new(ALICE_ID.clone().to_account_id(wonderland)))
                .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Grant::account_permission(
                Permission::new("CanManageSoracloud".into(), Json::new(())),
                ALICE_ID.clone(),
            )
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;

            let mut bundle = fixture_bundle("1.0.0");
            bundle.container.required_config_names = vec!["ui/theme".to_string()];
            bundle.container.config_exports = vec![
                SoraConfigExportV1 {
                    config_name: "ui/theme".to_string(),
                    target: iroha_data_model::soracloud::SoraConfigExportTargetV1::Env(
                        "UI_THEME_JSON".to_string(),
                    ),
                },
                SoraConfigExportV1 {
                    config_name: "ui/theme".to_string(),
                    target: iroha_data_model::soracloud::SoraConfigExportTargetV1::File(
                        "runtime/ui/theme.json".to_string(),
                    ),
                },
            ];
            bundle.service.container.manifest_hash = bundle.container_manifest_hash();
            let provenance = {
                let payload =
                    encode_bundle_signature_payload(&bundle, &BTreeMap::new(), &BTreeMap::new())
                        .expect("encode bundle payload");
                ManifestProvenance {
                    signer: ALICE_ID.signatory().clone(),
                    signature: Signature::new(
                        iroha_test_samples::ALICE_KEYPAIR.private_key(),
                        &payload,
                    ),
                }
            };
            isi::soracloud::DeploySoracloudService {
                bundle,
                initial_service_configs: BTreeMap::new(),
                initial_service_secrets: BTreeMap::new(),
                provenance,
            }
            .execute(&ALICE_ID, &mut stx)?;
            stx.apply();
            state_block.commit()?;

            let snapshot = control_plane_snapshot(&app, Some("web_portal"), 10);
            assert_eq!(snapshot.service_count, 1);
            assert_eq!(snapshot.audit_event_count, 1);
            assert_eq!(snapshot.services[0].current_version, "1.0.0");
            assert_eq!(
                snapshot.services[0]
                    .latest_revision
                    .as_ref()
                    .expect("latest revision")
                    .signed_by,
                ALICE_ID.signatory().to_string()
            );
            assert_eq!(
                snapshot.services[0]
                    .latest_revision
                    .as_ref()
                    .expect("latest revision")
                    .config_exports
                    .len(),
                2
            );
            assert_eq!(snapshot.recent_audit_events.len(), 1);
            Ok(())
        })
    }

    #[test]
    fn error_chain_message_includes_nested_validation_details() {
        let error = iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                    iroha_data_model::isi::error::InvalidParameterError::SmartContract(
                        "resources.cpu_millis exceeds SCR cap".to_owned(),
                    ),
                ),
            ),
        );
        let message = transaction_rejection_message(&error);
        assert!(message.contains("Validation failed"));
        assert!(message.contains("Instruction execution failed"));
        assert!(message.contains("Invalid instruction parameter"));
        assert!(message.contains("resources.cpu_millis exceeds SCR cap"));
    }

    #[test]
    fn admit_scr_host_bundle_rejects_over_cap_cpu() {
        let mut bundle = fixture_bundle("1.0.0");
        bundle.container.resources.cpu_millis = NonZeroU32::new(64_001).expect("non-zero cpu");
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        let error = admit_scr_host_bundle(&bundle).expect_err("SCR over-cap cpu should fail");
        assert!(
            error
                .message
                .contains("container.resources.cpu_millis exceeds SCR cap")
        );
    }

    #[test]
    fn resolve_public_local_read_route_uses_authoritative_service_route_state() {
        use iroha_core::state::World;

        let mut world = World::new();
        let bundle = fixture_bundle("2026.02.0");
        let service_name = bundle.service.service_name.clone();
        world.soracloud_service_revisions_mut_for_testing().insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                service_name.clone(),
                SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                    service_name: service_name.clone(),
                    current_service_version: bundle.service.service_version.clone(),
                    current_service_manifest_hash: bundle.service_manifest_hash(),
                    current_container_manifest_hash: bundle.container_manifest_hash(),
                    revision_count: 1,
                    process_generation: 1,
                    process_started_sequence: 1,
                    active_rollout: None,
                    last_rollout: None,
                    config_generation: 0,
                    secret_generation: 0,
                    service_configs: BTreeMap::new(),
                    service_secrets: BTreeMap::new(),
                },
            );
        let app = mk_app_state_for_tests_with_world(world);

        let assets = resolve_public_local_read_route(&app, "portal.sora:443", "/app/assets")
            .expect("asset route");
        assert_eq!(assets.service_name, "web_portal");
        assert_eq!(assets.service_version, "2026.02.0");
        assert_eq!(assets.handler_name, "assets");
        assert_eq!(assets.handler_class, SoracloudLocalReadKind::Asset);
        assert_eq!(assets.handler_path, "/");

        let query = resolve_public_local_read_route(&app, "portal.sora", "/app/query/stats")
            .expect("query route");
        assert_eq!(query.handler_name, "query");
        assert_eq!(query.handler_class, SoracloudLocalReadKind::Query);
        assert_eq!(query.handler_path, "/stats");

        assert!(
            resolve_public_local_read_route(&app, "portal.sora", "/app/private/update").is_none(),
            "replicated write handlers must not resolve through the local read fast path"
        );
        assert!(
            resolve_public_local_read_route(&app, "wrong.sora", "/app/assets").is_none(),
            "host matching must stay authoritative"
        );
    }

    #[test]
    fn authoritative_ciphertext_query_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let binding_name: Name = "patient_records".parse()?;
            let state_key = "/state/health/patient-1".to_string();
            let governance_tx_hash = Hash::new(b"gov-state");
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            world
                .soracloud_service_audit_events_mut_for_testing()
                .insert(
                    1,
                    SoraServiceAuditEventV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                        sequence: 1,
                        action: SoraServiceLifecycleActionV1::StateMutation,
                        service_name: service_name.clone(),
                        from_version: None,
                        to_version: bundle.service.service_version.clone(),
                        service_manifest_hash: bundle.service_manifest_hash(),
                        container_manifest_hash: bundle.container_manifest_hash(),
                        governance_tx_hash: Some(governance_tx_hash),
                        binding_name: Some(binding_name.clone()),
                        state_key: Some(state_key.clone()),
                        config_name: None,
                        secret_name: None,
                        rollout_handle: None,
                        policy_name: None,
                        policy_snapshot_hash: None,
                        jurisdiction_tag: None,
                        consent_evidence_hash: None,
                        break_glass: None,
                        break_glass_reason: None,
                        signer: KeyPair::random().public_key().clone(),
                    },
                );
            world
                .soracloud_service_state_entries_mut_for_testing()
                .insert(
                    (
                        service_name.as_ref().to_owned(),
                        binding_name.as_ref().to_owned(),
                        state_key.clone(),
                    ),
                    SoraServiceStateEntryV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                        service_name: service_name.clone(),
                        service_version: bundle.service.service_version.clone(),
                        binding_name: binding_name.clone(),
                        state_key: state_key.clone(),
                        encryption: SoraStateEncryptionV1::FheCiphertext,
                        payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                        payload_commitment: Hash::new(b"ciphertext"),
                        last_update_sequence: 1,
                        governance_tx_hash,
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_ciphertext_query_response(
                &app,
                signed_ciphertext_query_request(
                    fixture_ciphertext_query_spec(),
                    &KeyPair::random(),
                ),
            )
            .map_err(|err| eyre::eyre!("authoritative ciphertext query failed: {err:?}"))?;
            assert_eq!(response.action, SoracloudAction::CiphertextQuery);
            assert_eq!(response.response.result_count, 1);
            assert_eq!(
                response.response.results[0].ciphertext_commitment,
                Hash::new(b"ciphertext")
            );
            assert_eq!(
                response.response.results[0]
                    .proof
                    .as_ref()
                    .expect("inclusion proof")
                    .event_sequence,
                1
            );
            Ok(())
        })
    }

    #[test]
    fn authoritative_health_compliance_report_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let policy = fixture_decryption_authority_policy();
            let policy_snapshot_hash = Hash::new(Encode::encode(&policy));
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            for (sequence, state_key, break_glass, consent_evidence_hash) in [
                (
                    2,
                    "/state/health/patient-1",
                    false,
                    Some(Hash::new(b"consent-1")),
                ),
                (3, "/state/health/patient-2", true, None),
            ] {
                world
                    .soracloud_service_audit_events_mut_for_testing()
                    .insert(
                        sequence,
                        SoraServiceAuditEventV1 {
                            schema_version:
                                iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                            sequence,
                            action: SoraServiceLifecycleActionV1::DecryptionRequest,
                            service_name: service_name.clone(),
                            from_version: None,
                            to_version: bundle.service.service_version.clone(),
                            service_manifest_hash: bundle.service_manifest_hash(),
                            container_manifest_hash: bundle.container_manifest_hash(),
                            governance_tx_hash: Some(Hash::new(Encode::encode(&(
                                "gov-health",
                                sequence,
                            )))),
                            binding_name: Some("patient_records".parse()?),
                            state_key: Some(state_key.to_string()),
                            config_name: None,
                            secret_name: None,
                            rollout_handle: None,
                            policy_name: Some(policy.policy_name.clone()),
                            policy_snapshot_hash: Some(policy_snapshot_hash),
                            jurisdiction_tag: Some(policy.jurisdiction_tag.clone()),
                            consent_evidence_hash,
                            break_glass: Some(break_glass),
                            break_glass_reason: break_glass
                                .then(|| "emergency override".to_string()),
                            signer: KeyPair::random().public_key().clone(),
                        },
                    );
            }

            let app = mk_app_state_for_tests_with_world(world);
            let report = authoritative_health_compliance_report(
                &app,
                Some(service_name.as_ref()),
                Some("us_hipaa"),
                20,
            )
            .map_err(|err| eyre::eyre!("authoritative health report failed: {err:?}"))?;
            assert_eq!(report.total_access_events, 2);
            assert_eq!(report.break_glass_events, 1);
            assert_eq!(report.non_break_glass_events, 1);
            assert_eq!(report.consent_evidence_present_events, 1);
            assert_eq!(report.consent_evidence_coverage_bps, 5_000);
            assert_eq!(report.recent_access_events.len(), 2);
            assert!(
                report
                    .data_flow_attestations
                    .iter()
                    .any(|entry| entry.binding_name == "patient_records"),
                "expected authoritative data-flow attestation"
            );
            assert_eq!(report.policy_diff_history.len(), 1);
            assert_eq!(
                report.policy_diff_history[0].policy_snapshot_hash,
                policy_snapshot_hash
            );
            Ok(())
        })
    }

    #[test]
    fn authoritative_training_job_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            world.soracloud_training_jobs_mut_for_testing().insert(
                (service_name.as_ref().to_owned(), "job-1".to_string()),
                iroha_data_model::soracloud::SoraTrainingJobRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_TRAINING_JOB_RECORD_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    model_name: "vision_model".to_string(),
                    job_id: "job-1".to_string(),
                    status: iroha_data_model::soracloud::SoraTrainingJobStatusV1::Completed,
                    worker_group_size: 4,
                    target_steps: 100,
                    completed_steps: 100,
                    checkpoint_interval_steps: 20,
                    last_checkpoint_step: Some(100),
                    checkpoint_count: 5,
                    retry_count: 1,
                    max_retries: 3,
                    step_compute_units: 50,
                    compute_budget_units: 40_000,
                    compute_consumed_units: 20_000,
                    storage_budget_bytes: 8_192,
                    storage_consumed_bytes: 4_096,
                    latest_metrics_hash: Some(Hash::new(b"metrics")),
                    last_failure_reason: None,
                    created_sequence: 1,
                    updated_sequence: 5,
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_training_job_status_response(&app, "web_portal", "job-1")
                .map_err(|err| {
                    eyre::eyre!("authoritative training job status query failed: {err:?}")
                })?;
            assert_eq!(response.job.job_id, "job-1");
            assert_eq!(response.job.status, TrainingJobStatus::Completed);
            assert_eq!(response.job.compute_remaining_units, 20_000);
            Ok(())
        })
    }

    #[test]
    fn authoritative_service_config_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let config_value = norito::json!({
                "theme": "dark",
                "max_connections": 32
            });
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                service_name.clone(),
                SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                    service_name: service_name.clone(),
                    current_service_version: bundle.service.service_version.clone(),
                    current_service_manifest_hash: bundle.service_manifest_hash(),
                    current_container_manifest_hash: bundle.container_manifest_hash(),
                    revision_count: 1,
                    process_generation: 1,
                    process_started_sequence: 1,
                    active_rollout: None,
                    last_rollout: None,
                    config_generation: 4,
                    secret_generation: 0,
                    service_configs: BTreeMap::from([(
                        "ui/theme".to_string(),
                        SoraServiceConfigEntryV1 {
                            schema_version:
                                iroha_data_model::soracloud::SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                            config_name: "ui/theme".to_string(),
                            value_hash: Hash::new(
                                norito::json::to_vec(&config_value)
                                    .expect("config json should encode"),
                            ),
                            value_json: Json::from(config_value.clone()),
                            last_update_sequence: 12,
                        },
                    )]),
                    service_secrets: BTreeMap::new(),
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response =
                authoritative_service_config_status_response(&app, "web_portal", Some("ui/theme"))
                    .map_err(|err| {
                        eyre::eyre!("authoritative service config status query failed: {err:?}")
                    })?;
            assert_eq!(response.service_name, "web_portal");
            assert_eq!(response.current_version, "1.0.0");
            assert_eq!(response.config_generation, 4);
            assert_eq!(response.config_entry_count, 1);
            assert_eq!(response.configs[0].config_name, "ui/theme");
            assert_eq!(response.configs[0].value_json, config_value);
            assert_eq!(response.configs[0].last_update_sequence, 12);
            Ok(())
        })
    }

    #[test]
    fn authoritative_service_secret_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let secret = SecretEnvelopeV1 {
                schema_version: iroha_data_model::soracloud::SECRET_ENVELOPE_VERSION_V1,
                encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
                key_id: "kms/config/test".to_string(),
                key_version: std::num::NonZeroU32::new(7).expect("non-zero"),
                nonce: vec![1, 2, 3, 4],
                ciphertext: b"encrypted-db-password".to_vec(),
                commitment: Hash::new(b"service-secret"),
                aad_digest: None,
            };
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                service_name.clone(),
                SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                    service_name: service_name.clone(),
                    current_service_version: bundle.service.service_version.clone(),
                    current_service_manifest_hash: bundle.service_manifest_hash(),
                    current_container_manifest_hash: bundle.container_manifest_hash(),
                    revision_count: 1,
                    process_generation: 1,
                    process_started_sequence: 1,
                    active_rollout: None,
                    last_rollout: None,
                    config_generation: 0,
                    secret_generation: 3,
                    service_configs: BTreeMap::new(),
                    service_secrets: BTreeMap::from([(
                        "db/password".to_string(),
                        SoraServiceSecretEntryV1 {
                            schema_version:
                                iroha_data_model::soracloud::SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                            secret_name: "db/password".to_string(),
                            envelope: secret.clone(),
                            last_update_sequence: 9,
                        },
                    )]),
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_service_secret_status_response(
                &app,
                "web_portal",
                Some("db/password"),
            )
            .map_err(|err| {
                eyre::eyre!("authoritative service secret status query failed: {err:?}")
            })?;
            assert_eq!(response.service_name, "web_portal");
            assert_eq!(response.current_version, "1.0.0");
            assert_eq!(response.secret_generation, 3);
            assert_eq!(response.secret_entry_count, 1);
            assert_eq!(response.secrets[0].secret_name, "db/password");
            assert_eq!(
                response.secrets[0].encryption,
                SecretEnvelopeEncryptionV1::ClientCiphertext
            );
            assert_eq!(response.secrets[0].key_id, "kms/config/test");
            assert_eq!(response.secrets[0].key_version, 7);
            assert_eq!(response.secrets[0].commitment, Hash::new(b"service-secret"));
            assert_eq!(
                response.secrets[0].ciphertext_bytes,
                u64::try_from(secret.ciphertext.len()).expect("fits in u64")
            );
            assert_eq!(response.secrets[0].last_update_sequence, 9);
            Ok(())
        })
    }

    #[test]
    fn authoritative_model_weight_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            world.soracloud_model_registries_mut_for_testing().insert(
                (service_name.as_ref().to_owned(), "vision_model".to_string()),
                iroha_data_model::soracloud::SoraModelRegistryV1 {
                    schema_version: iroha_data_model::soracloud::SORA_MODEL_REGISTRY_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    model_name: "vision_model".to_string(),
                    current_version: Some("v2".to_string()),
                    updated_sequence: 9,
                },
            );
            world
                .soracloud_model_weight_versions_mut_for_testing()
                .insert(
                (
                    service_name.as_ref().to_owned(),
                    "vision_model".to_string(),
                    "v2".to_string(),
                ),
                iroha_data_model::soracloud::SoraModelWeightVersionRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    model_name: "vision_model".to_string(),
                    weight_version: "v2".to_string(),
                    parent_version: Some("v1".to_string()),
                    training_job_id: "job-1".to_string(),
                    source_provenance: Some(
                        iroha_data_model::soracloud::SoraModelProvenanceRefV1 {
                            kind:
                                iroha_data_model::soracloud::SoraModelProvenanceKindV1::TrainingJob,
                            id: "job-1".to_string(),
                        },
                    ),
                    weight_artifact_hash: Hash::new(b"weights"),
                    dataset_ref: "dataset://train".to_string(),
                    training_config_hash: Hash::new(b"train-config"),
                    reproducibility_hash: Hash::new(b"repro"),
                    provenance_attestation_hash: Hash::new(b"prov"),
                    registered_sequence: 7,
                    promoted_sequence: Some(9),
                    gate_report_hash: Some(Hash::new(b"gate")),
                    promoted_by: Some(KeyPair::random().public_key().clone()),
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response =
                authoritative_model_weight_status_response(&app, "web_portal", "vision_model")
                    .map_err(|err| {
                        eyre::eyre!("authoritative model weight status query failed: {err:?}")
                    })?;
            assert_eq!(response.model.current_version.as_deref(), Some("v2"));
            assert_eq!(response.model.version_count, 1);
            assert_eq!(response.model.versions[0].training_job_id, "job-1");
            Ok(())
        })
    }

    #[test]
    fn authoritative_model_artifact_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    service_name.as_ref().to_owned(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            world.soracloud_model_artifacts_mut_for_testing().insert(
                (service_name.as_ref().to_owned(), "job-1".to_string()),
                iroha_data_model::soracloud::SoraModelArtifactRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    model_name: "vision_model".to_string(),
                    artifact_id: "job-1".to_string(),
                    training_job_id: "job-1".to_string(),
                    weight_version: Some("v2".to_string()),
                    source_provenance: Some(
                        iroha_data_model::soracloud::SoraModelProvenanceRefV1 {
                            kind:
                                iroha_data_model::soracloud::SoraModelProvenanceKindV1::TrainingJob,
                            id: "job-1".to_string(),
                        },
                    ),
                    weight_artifact_hash: Hash::new(b"weights"),
                    dataset_ref: "dataset://train".to_string(),
                    training_config_hash: Hash::new(b"train-config"),
                    reproducibility_hash: Hash::new(b"repro"),
                    provenance_attestation_hash: Hash::new(b"prov"),
                    registered_sequence: 8,
                    consumed_by_version: Some("v2".to_string()),
                    private_bundle_root: None,
                    compile_profile_hash: None,
                    chunk_manifest_root: None,
                    privacy_mode: None,
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_model_artifact_status_response(
                &app,
                "web_portal",
                Some("vision_model"),
                Some("job-1"),
                Some("job-1"),
                Some("v2"),
            )
            .map_err(|err| {
                eyre::eyre!("authoritative model artifact status query failed: {err:?}")
            })?;
            assert_eq!(response.artifact.training_job_id, "job-1");
            assert_eq!(response.artifact.artifact_id, "job-1");
            assert_eq!(response.artifact.consumed_by_version.as_deref(), Some("v2"));
            Ok(())
        })
    }

    #[test]
    fn authoritative_uploaded_model_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let service_name: Name = "web_portal".parse().expect("service name");
            let compile_profile = SoraPrivateCompileProfileV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_PRIVATE_COMPILE_PROFILE_VERSION_V1,
                family: "decoder-only".to_string(),
                quantization: "int8-int16-int32".to_string(),
                opset_version: "private-ir.v1".to_string(),
                max_context: 4096,
                max_images: 1,
                vision_patch_policy: "text-image".to_string(),
                fhe_param_set: "bfv-small".to_string(),
                execution_policy: "deterministic".to_string(),
            };
            let compile_profile_hash: Hash = HashOf::new(&compile_profile).into();
            let bundle_root = Hash::new(b"bundle-root");
            let chunk_manifest_root = Hash::new(b"chunk-manifest-root");

            world
                .soracloud_uploaded_model_bundles_mut_for_testing()
                .insert(
                    (
                        service_name.as_ref().to_owned(),
                        "upload-1".to_string(),
                        "v1".to_string(),
                    ),
                    SoraUploadedModelBundleV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
                        service_name: service_name.clone(),
                        model_id: "upload-1".to_string(),
                        weight_version: "v1".to_string(),
                        family: "decoder-only".to_string(),
                        modalities: vec!["text".to_string(), "image".to_string()],
                        plaintext_root: Hash::new(b"plaintext-root"),
                        runtime_format: SoraUploadedModelRuntimeFormatV1::SoracloudPrivateIr,
                        bundle_root,
                        chunk_count: 1,
                        plaintext_bytes: 16,
                        ciphertext_bytes: 24,
                        compile_profile_hash,
                        chunk_manifest_root,
                        upload_recipient:
                            iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                                schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                                key_id: "soracloud-upload".to_string(),
                                key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                                kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                                aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                                public_key_bytes: vec![7u8; 32],
                                public_key_fingerprint: Hash::new([7u8; 32]),
                            },
                        wrapped_bundle_key:
                            iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                                schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                                recipient_key_id: "soracloud-upload".to_string(),
                                recipient_key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                                kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                                aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                                ephemeral_public_key: vec![8u8; 32],
                                nonce: vec![9u8; 12],
                                wrapped_key_ciphertext: vec![10u8; 48],
                                ciphertext_hash: Hash::new([10u8; 48]),
                                aad_digest: Hash::new(b"wrapped-aad"),
                            },
                        pricing_policy: SoraUploadedModelPricingPolicyV1 {
                            storage_xor_nanos: 1,
                            compile_xor_nanos: 2,
                            runtime_step_xor_nanos: 3,
                            decrypt_release_xor_nanos: 4,
                        },
                        decryption_policy_ref: "policy-1".to_string(),
                    },
                );
            world
                .soracloud_uploaded_model_chunks_mut_for_testing()
                .insert(
                    "web_portal::upload-1::v1::0".to_string(),
                    SoraUploadedModelChunkV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_UPLOADED_MODEL_CHUNK_VERSION_V1,
                        service_name: service_name.clone(),
                        model_id: "upload-1".to_string(),
                        weight_version: "v1".to_string(),
                        bundle_root,
                        ordinal: 0,
                        offset_bytes: 0,
                        plaintext_len: 16,
                        ciphertext_len: 24,
                        ciphertext_hash: Hash::new(b"ciphertext"),
                        encrypted_payload: SecretEnvelopeV1 {
                            schema_version: iroha_data_model::soracloud::SECRET_ENVELOPE_VERSION_V1,
                            encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
                            key_id: "kms://test".to_string(),
                            key_version: NonZeroU32::new(1).expect("non-zero key version"),
                            nonce: vec![1, 2, 3, 4],
                            ciphertext: vec![0; 24],
                            commitment: Hash::new(b"commitment"),
                            aad_digest: None,
                        },
                    },
                );
            world
                .soracloud_private_compile_profiles_mut_for_testing()
                .insert(compile_profile_hash, compile_profile.clone());
            world.soracloud_model_artifacts_mut_for_testing().insert(
                (service_name.as_ref().to_owned(), "artifact-1".to_string()),
                iroha_data_model::soracloud::SoraModelArtifactRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: "1.0.0".to_string(),
                    model_name: "vision_model".to_string(),
                    artifact_id: "artifact-1".to_string(),
                    training_job_id: "artifact-1".to_string(),
                    weight_version: Some("v1".to_string()),
                    source_provenance: Some(SoraModelProvenanceRefV1 {
                        kind: SoraModelProvenanceKindV1::UserUpload,
                        id: "upload-1".to_string(),
                    }),
                    weight_artifact_hash: Hash::new(b"weights"),
                    dataset_ref: "hf://repo".to_string(),
                    training_config_hash: Hash::new(b"cfg"),
                    reproducibility_hash: Hash::new(b"repro"),
                    provenance_attestation_hash: Hash::new(b"prov"),
                    registered_sequence: 11,
                    consumed_by_version: Some("v1".to_string()),
                    private_bundle_root: Some(bundle_root),
                    compile_profile_hash: Some(compile_profile_hash),
                    chunk_manifest_root: Some(chunk_manifest_root),
                    privacy_mode: Some(SoraModelPrivacyModeV1::PrivateExecution),
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let response =
                authoritative_uploaded_model_status_response(&app, "web_portal", "upload-1", "v1")
                    .map_err(|err| eyre::eyre!("uploaded model status query failed: {err:?}"))?;
            assert_eq!(response.uploaded_chunk_count, 1);
            assert_eq!(response.chunk_ordinals, vec![0]);
            assert!(response.compile_profile.is_some());
            assert_eq!(
                response
                    .artifact
                    .as_ref()
                    .map(|artifact| artifact.artifact_id.as_str()),
                Some("artifact-1")
            );
            Ok(())
        })
    }

    #[test]
    fn authoritative_private_inference_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let mut world = World::default();
            let apartment: Name = "arena_suite".parse().expect("apartment");
            let service_name: Name = "web_portal".parse().expect("service");
            world
                .soracloud_private_inference_sessions_mut_for_testing()
                .insert(
                    (apartment.as_ref().to_owned(), "session-1".to_string()),
                    SoraPrivateInferenceSessionV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1,
                        session_id: "session-1".to_string(),
                        apartment: apartment.clone(),
                        service_name,
                        model_id: "upload-1".to_string(),
                        weight_version: "v1".to_string(),
                        bundle_root: Hash::new(b"bundle-root"),
                        input_commitments: vec![Hash::new(b"input-1")],
                        token_budget: 256,
                        image_budget: 1,
                        status: SoraPrivateInferenceSessionStatusV1::AwaitingDecryption,
                        receipt_root: Hash::new(b"receipt-root"),
                        xor_cost_nanos: 512,
                    },
                );
            world
                .soracloud_private_inference_checkpoints_mut_for_testing()
                .insert(
                    ("session-1".to_string(), 1),
                    SoraPrivateInferenceCheckpointV1 {
                        schema_version: iroha_data_model::soracloud::SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
                        session_id: "session-1".to_string(),
                        step: 1,
                        ciphertext_state_root: Hash::new(b"ciphertext-root"),
                        receipt_hash: Hash::new(b"receipt-hash"),
                        decrypt_request_id: "decrypt-1".to_string(),
                        released_token: Some("42".to_string()),
                        compute_units: 64,
                        updated_at_ms: 1,
                    },
                );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_private_inference_status_response(&app, "session-1")
                .map_err(|err| eyre::eyre!("private inference status query failed: {err:?}"))?;
            assert_eq!(response.session.model_id, "upload-1");
            assert_eq!(response.checkpoint_count, 1);
            assert_eq!(response.checkpoints[0].decrypt_request_id, "decrypt-1");
            Ok(())
        })
    }

    fn signed_state_mutation_request(
        payload: StateMutationRequest,
        key_pair: &KeyPair,
    ) -> SignedStateMutationRequest {
        let encoded = encode_state_mutation_signature_payload(&payload)
            .expect("encode state mutation payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedStateMutationRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_fhe_job_run_request(
        payload: FheJobRunPayload,
        key_pair: &KeyPair,
    ) -> SignedFheJobRunRequest {
        let encoded =
            encode_fhe_job_run_signature_payload(&payload).expect("encode fhe job run payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedFheJobRunRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_training_job_start_request(
        payload: TrainingJobStartPayload,
        key_pair: &KeyPair,
    ) -> SignedTrainingJobStartRequest {
        let encoded = encode_training_job_start_signature_payload(&payload)
            .expect("encode training start payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedTrainingJobStartRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_training_job_checkpoint_request(
        payload: TrainingJobCheckpointPayload,
        key_pair: &KeyPair,
    ) -> SignedTrainingJobCheckpointRequest {
        let encoded = encode_training_job_checkpoint_signature_payload(&payload)
            .expect("encode training checkpoint payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedTrainingJobCheckpointRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_training_job_retry_request(
        payload: TrainingJobRetryPayload,
        key_pair: &KeyPair,
    ) -> SignedTrainingJobRetryRequest {
        let encoded = encode_training_job_retry_signature_payload(&payload)
            .expect("encode training retry payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedTrainingJobRetryRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_model_weight_register_request(
        payload: ModelWeightRegisterPayload,
        key_pair: &KeyPair,
    ) -> SignedModelWeightRegisterRequest {
        let encoded = encode_model_weight_register_signature_payload(&payload)
            .expect("encode model weight register payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedModelWeightRegisterRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_model_weight_promote_request(
        payload: ModelWeightPromotePayload,
        key_pair: &KeyPair,
    ) -> SignedModelWeightPromoteRequest {
        let encoded = encode_model_weight_promote_signature_payload(&payload)
            .expect("encode model weight promote payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedModelWeightPromoteRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_model_weight_rollback_request(
        payload: ModelWeightRollbackPayload,
        key_pair: &KeyPair,
    ) -> SignedModelWeightRollbackRequest {
        let encoded = encode_model_weight_rollback_signature_payload(&payload)
            .expect("encode model weight rollback payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedModelWeightRollbackRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_model_artifact_register_request(
        payload: ModelArtifactRegisterPayload,
        key_pair: &KeyPair,
    ) -> SignedModelArtifactRegisterRequest {
        let encoded = encode_model_artifact_register_signature_payload(&payload)
            .expect("encode model artifact register payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedModelArtifactRegisterRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_decryption_request(
        payload: DecryptionRequestPayload,
        key_pair: &KeyPair,
    ) -> SignedDecryptionRequest {
        let encoded = encode_decryption_request_signature_payload(&payload)
            .expect("encode decryption request payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedDecryptionRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_ciphertext_query_request(
        query: CiphertextQuerySpecV1,
        key_pair: &KeyPair,
    ) -> SignedCiphertextQueryRequest {
        let encoded = encode_ciphertext_query_signature_payload(&query)
            .expect("encode ciphertext query payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedCiphertextQueryRequest {
            query,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }

    fn signed_rollout_request(
        service_name: &str,
        rollout_handle: &str,
        healthy: bool,
        promote_to_percent: Option<u8>,
        governance_seed: &[u8],
        key_pair: &KeyPair,
    ) -> SignedRolloutAdvanceRequest {
        let payload = RolloutAdvancePayload {
            service_name: service_name.to_string(),
            rollout_handle: rollout_handle.to_string(),
            healthy,
            promote_to_percent,
            governance_tx_hash: Hash::new(governance_seed),
        };
        let encoded = encode_rollout_signature_payload(&payload).expect("encode rollout payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedRolloutAdvanceRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn fixture_agent_manifest() -> AgentApartmentManifestV1 {
        let mut manifest: AgentApartmentManifestV1 = load_json(&workspace_fixture(
            "fixtures/soracloud/agent_apartment_manifest_v1.json",
        ));
        manifest.policy_capabilities.push(
            "agent.autonomy.run"
                .parse::<Name>()
                .expect("valid capability"),
        );
        manifest.policy_capabilities.push(
            "agent.autonomy.allow"
                .parse::<Name>()
                .expect("valid capability"),
        );
        manifest.validate().expect("agent manifest should validate");
        manifest
    }

    fn fixture_agent_manifest_with_capabilities(
        apartment_name: &str,
        extra_capabilities: &[&str],
    ) -> AgentApartmentManifestV1 {
        let mut manifest = fixture_agent_manifest();
        manifest.apartment_name = apartment_name.parse().expect("valid apartment name");
        for capability in extra_capabilities {
            let parsed = capability.parse::<Name>().expect("valid capability");
            if !manifest.policy_capabilities.contains(&parsed) {
                manifest.policy_capabilities.push(parsed);
            }
        }
        manifest.validate().expect("agent manifest should validate");
        manifest
    }

    fn signed_agent_deploy_request(
        payload: AgentDeployPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentDeployRequest {
        let encoded =
            encode_agent_deploy_signature_payload(&payload).expect("encode agent deploy payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentDeployRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_lease_renew_request(
        payload: AgentLeaseRenewPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentLeaseRenewRequest {
        let encoded = encode_agent_lease_renew_signature_payload(&payload)
            .expect("encode agent lease renew payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentLeaseRenewRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_restart_request(
        payload: AgentRestartPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentRestartRequest {
        let encoded =
            encode_agent_restart_signature_payload(&payload).expect("encode agent restart payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentRestartRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_policy_revoke_request(
        payload: AgentPolicyRevokePayload,
        key_pair: &KeyPair,
    ) -> SignedAgentPolicyRevokeRequest {
        let encoded = encode_agent_policy_revoke_signature_payload(&payload)
            .expect("encode agent policy revoke payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentPolicyRevokeRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_wallet_spend_request(
        payload: AgentWalletSpendPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentWalletSpendRequest {
        let encoded = encode_agent_wallet_spend_signature_payload(&payload)
            .expect("encode agent wallet spend payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentWalletSpendRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_wallet_approve_request(
        payload: AgentWalletApprovePayload,
        key_pair: &KeyPair,
    ) -> SignedAgentWalletApproveRequest {
        let encoded = encode_agent_wallet_approve_signature_payload(&payload)
            .expect("encode agent wallet approve payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentWalletApproveRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_message_send_request(
        payload: AgentMessageSendPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentMessageSendRequest {
        let encoded = encode_agent_message_send_signature_payload(&payload)
            .expect("encode agent message send payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentMessageSendRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_message_ack_request(
        payload: AgentMessageAckPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentMessageAckRequest {
        let encoded = encode_agent_message_ack_signature_payload(&payload)
            .expect("encode agent message ack payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentMessageAckRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_artifact_allow_request(
        payload: AgentArtifactAllowPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentArtifactAllowRequest {
        let encoded = encode_agent_artifact_allow_signature_payload(&payload)
            .expect("encode agent artifact allow payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentArtifactAllowRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn signed_agent_autonomy_run_request(
        payload: AgentAutonomyRunPayload,
        key_pair: &KeyPair,
    ) -> SignedAgentAutonomyRunRequest {
        let encoded = encode_agent_autonomy_run_signature_payload(&payload)
            .expect("encode agent autonomy run payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedAgentAutonomyRunRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
            authority: None,
            private_key: None,
        }
    }

    fn legacy_struct_layout_signature<T: Encode>(payload: &T, key_pair: &KeyPair) -> Signature {
        let encoded = norito::to_bytes(payload).expect("encode legacy struct-layout payload");
        Signature::new(key_pair.private_key(), &encoded)
    }

    #[test]
    fn bundle_signature_payload_layout_is_canonical_layout() {
        let bundle = fixture_bundle("1.0.0");
        let encoded = encode_bundle_signature_payload(&bundle, &BTreeMap::new(), &BTreeMap::new())
            .expect("encode signature payload");
        let expected = norito::to_bytes(&bundle).expect("encode canonical layout");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn state_mutation_signature_payload_layout_is_canonical_layout() {
        let governance_tx_hash = Hash::new(b"governance");
        let payload = StateMutationRequest {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            key: "/state/private/records/1".to_owned(),
            operation: StateMutationOperation::Upsert,
            value_size_bytes: Some(512),
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            governance_tx_hash,
        };
        let encoded =
            encode_state_mutation_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            payload.key.as_str(),
            "upsert",
            payload.value_size_bytes,
            payload.encryption,
            governance_tx_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn state_mutation_signature_payload_uses_delete_operation_label() {
        let governance_tx_hash = Hash::new(b"delete-governance");
        let payload = StateMutationRequest {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            key: "/state/private/records/1".to_owned(),
            operation: StateMutationOperation::Delete,
            value_size_bytes: None,
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            governance_tx_hash,
        };
        let encoded =
            encode_state_mutation_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            payload.key.as_str(),
            "delete",
            None::<u64>,
            payload.encryption,
            governance_tx_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn fhe_job_run_signature_payload_layout_is_canonical_tuple() {
        let job = fixture_fhe_job_spec();
        let policy = fixture_fhe_execution_policy();
        let param_set = fixture_fhe_param_set();
        let governance_tx_hash = Hash::new(b"governance");
        let payload = FheJobRunPayload {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            job: job.clone(),
            policy: policy.clone(),
            param_set: param_set.clone(),
            governance_tx_hash: governance_tx_hash.clone(),
        };
        let encoded =
            encode_fhe_job_run_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            job,
            policy,
            param_set,
            governance_tx_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn decryption_request_signature_payload_layout_is_canonical_tuple() {
        let policy = fixture_decryption_authority_policy();
        let request = fixture_decryption_request();
        let payload = DecryptionRequestPayload {
            service_name: "health_portal".to_owned(),
            policy: policy.clone(),
            request: request.clone(),
        };
        let encoded = encode_decryption_request_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(payload.service_name.as_str(), policy, request))
            .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn ciphertext_query_signature_payload_layout_is_canonical_layout() {
        let query = fixture_ciphertext_query_spec();
        let encoded =
            encode_ciphertext_query_signature_payload(&query).expect("encode signature payload");
        let expected = norito::to_bytes(&query).expect("encode canonical layout");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn rollback_signature_payload_layout_is_canonical_tuple() {
        let payload = RollbackPayload {
            service_name: "web_portal".to_owned(),
            target_version: Some("1.0.0".to_owned()),
        };
        let encoded =
            encode_rollback_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.target_version.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn rollout_signature_payload_layout_is_canonical_tuple() {
        let governance_tx_hash = Hash::new(b"governance");
        let payload = RolloutAdvancePayload {
            service_name: "web_portal".to_owned(),
            rollout_handle: "web_portal:rollout:2".to_owned(),
            healthy: true,
            promote_to_percent: Some(100),
            governance_tx_hash: governance_tx_hash.clone(),
        };
        let encoded = encode_rollout_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.rollout_handle.as_str(),
            payload.healthy,
            payload.promote_to_percent,
            governance_tx_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_deploy_signature_payload_layout_is_canonical_tuple() {
        let manifest = fixture_agent_manifest();
        let payload = AgentDeployPayload {
            manifest: manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: Some(500),
        };
        let encoded =
            encode_agent_deploy_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(manifest, 120u64, Some(500u64))).expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_lease_renew_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentLeaseRenewPayload {
            apartment_name: "ops_agent".to_owned(),
            lease_ticks: 120,
        };
        let encoded =
            encode_agent_lease_renew_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(payload.apartment_name.as_str(), payload.lease_ticks))
            .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_restart_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentRestartPayload {
            apartment_name: "ops_agent".to_owned(),
            reason: "manual-restart".to_owned(),
        };
        let encoded =
            encode_agent_restart_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.reason.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_policy_revoke_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentPolicyRevokePayload {
            apartment_name: "ops_agent".to_owned(),
            capability: "agent.autonomy.run".to_owned(),
            reason: Some("manual-review".to_owned()),
        };
        let encoded = encode_agent_policy_revoke_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.capability.as_str(),
            payload.reason.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_spend_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentWalletSpendPayload {
            apartment_name: "ops_agent".to_owned(),
            asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
            amount_nanos: 1_000_000,
        };
        let encoded = encode_agent_wallet_spend_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.asset_definition.as_str(),
            payload.amount_nanos,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_approve_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentWalletApprovePayload {
            apartment_name: "ops_agent".to_owned(),
            request_id: "ops_agent:wallet:7".to_owned(),
        };
        let encoded = encode_agent_wallet_approve_signature_payload(&payload)
            .expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.request_id.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_send_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentMessageSendPayload {
            from_apartment: "ops_agent".to_owned(),
            to_apartment: "worker_agent".to_owned(),
            channel: "ops.sync".to_owned(),
            payload: "rotate-key-42".to_owned(),
        };
        let encoded = encode_agent_message_send_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.from_apartment.as_str(),
            payload.to_apartment.as_str(),
            payload.channel.as_str(),
            payload.payload.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_ack_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentMessageAckPayload {
            apartment_name: "worker_agent".to_owned(),
            message_id: "worker_agent:mail:3".to_owned(),
        };
        let encoded =
            encode_agent_message_ack_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.message_id.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_artifact_allow_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentArtifactAllowPayload {
            apartment_name: "ops_agent".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: Some("hash:PROV0001#01".to_owned()),
        };
        let encoded = encode_agent_artifact_allow_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.artifact_hash.as_str(),
            payload.provenance_hash.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_autonomy_run_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentAutonomyRunPayload {
            apartment_name: "ops_agent".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: Some("hash:PROV0001#01".to_owned()),
            budget_units: 120,
            run_label: "nightly-train-step-1".to_owned(),
            workflow_input_json: Some("{\"inputs\":\"nightly\"}".to_owned()),
        };
        let encoded = encode_agent_autonomy_run_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.artifact_hash.as_str(),
            payload.provenance_hash.as_deref(),
            payload.budget_units,
            payload.run_label.as_str(),
            payload.workflow_input_json.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_start_signature_payload_layout_is_canonical_tuple() {
        let payload = TrainingJobStartPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            job_id: "job-1".to_owned(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 500,
            compute_budget_units: 50_000,
            storage_budget_bytes: 4_096,
        };
        let encoded = encode_training_job_start_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.job_id.as_str(),
            payload.worker_group_size,
            payload.target_steps,
            payload.checkpoint_interval_steps,
            payload.max_retries,
            payload.step_compute_units,
            payload.compute_budget_units,
            payload.storage_budget_bytes,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_checkpoint_signature_payload_layout_is_canonical_tuple() {
        let metrics_hash = Hash::new(b"metrics");
        let payload = TrainingJobCheckpointPayload {
            service_name: "web_portal".to_owned(),
            job_id: "job-1".to_owned(),
            completed_step: 20,
            checkpoint_size_bytes: 1_024,
            metrics_hash: metrics_hash.clone(),
        };
        let encoded = encode_training_job_checkpoint_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.job_id.as_str(),
            payload.completed_step,
            payload.checkpoint_size_bytes,
            metrics_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_retry_signature_payload_layout_is_canonical_tuple() {
        let payload = TrainingJobRetryPayload {
            service_name: "web_portal".to_owned(),
            job_id: "job-1".to_owned(),
            reason: "worker unavailable".to_owned(),
        };
        let encoded = encode_training_job_retry_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.job_id.as_str(),
            payload.reason.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_artifact_register_signature_payload_layout_is_canonical_tuple() {
        let weight_artifact_hash = Hash::new(b"weight-artifact");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"attestation");
        let payload = ModelArtifactRegisterPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            training_job_id: "job-1".to_owned(),
            weight_artifact_hash: weight_artifact_hash.clone(),
            dataset_ref: "dataset://synthetic/v2".to_owned(),
            training_config_hash: training_config_hash.clone(),
            reproducibility_hash: reproducibility_hash.clone(),
            provenance_attestation_hash: provenance_attestation_hash.clone(),
        };
        let encoded = encode_model_artifact_register_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.training_job_id.as_str(),
            weight_artifact_hash,
            payload.dataset_ref.as_str(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_register_signature_payload_layout_is_canonical_tuple() {
        let weight_artifact_hash = Hash::new(b"weight-artifact");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"attestation");
        let payload = ModelWeightRegisterPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            weight_version: "1.0.0".to_owned(),
            training_job_id: "job-1".to_owned(),
            parent_version: Some("0.9.0".to_owned()),
            weight_artifact_hash: weight_artifact_hash.clone(),
            dataset_ref: "dataset://synthetic/v2".to_owned(),
            training_config_hash: training_config_hash.clone(),
            reproducibility_hash: reproducibility_hash.clone(),
            provenance_attestation_hash: provenance_attestation_hash.clone(),
        };
        let encoded = encode_model_weight_register_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.weight_version.as_str(),
            payload.training_job_id.as_str(),
            payload.parent_version.as_deref(),
            weight_artifact_hash,
            payload.dataset_ref.as_str(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_promote_signature_payload_layout_is_canonical_tuple() {
        let gate_report_hash = Hash::new(b"gate-report");
        let payload = ModelWeightPromotePayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            weight_version: "1.0.0".to_owned(),
            gate_approved: true,
            gate_report_hash: gate_report_hash.clone(),
        };
        let encoded = encode_model_weight_promote_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.weight_version.as_str(),
            payload.gate_approved,
            gate_report_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_rollback_signature_payload_layout_is_canonical_tuple() {
        let payload = ModelWeightRollbackPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            target_version: "0.9.0".to_owned(),
            reason: "gate regression".to_owned(),
        };
        let encoded = encode_model_weight_rollback_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.target_version.as_str(),
            payload.reason.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn hf_deploy_signature_payload_layout_is_canonical_tuple() {
        let payload = HfDeployPayload {
            repo_id: "openai/gpt-oss".to_owned(),
            revision: None,
            model_name: "gpt_oss_20b".to_owned(),
            service_name: "vision_portal".to_owned(),
            apartment_name: Some("ops_agent".to_owned()),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            lease_asset_definition_id: hf_shared_lease_asset_definition(),
            base_fee_nanos: 10_000,
        };
        let encoded =
            encode_hf_deploy_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "main",
            "gpt_oss_20b",
            "vision_portal",
            Some("ops_agent"),
            StorageClass::Warm,
            604_800_000_u64,
            hf_shared_lease_asset_definition(),
            10_000_u128,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn hf_lease_leave_signature_payload_layout_is_canonical_tuple() {
        let payload = HfLeaseLeavePayload {
            repo_id: "openai/gpt-oss".to_owned(),
            revision: Some("refs/pr/7".to_owned()),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            service_name: Some("vision_portal".to_owned()),
            apartment_name: Some("ops_agent".to_owned()),
        };
        let encoded =
            encode_hf_lease_leave_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "refs/pr/7",
            StorageClass::Warm,
            604_800_000_u64,
            Some("vision_portal"),
            Some("ops_agent"),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn hf_lease_renew_signature_payload_layout_is_canonical_tuple() {
        let payload = HfLeaseRenewPayload {
            repo_id: "openai/gpt-oss".to_owned(),
            revision: Some("0123456789abcdef".to_owned()),
            model_name: "gpt_oss_20b".to_owned(),
            service_name: "vision_portal".to_owned(),
            apartment_name: Some("ops_agent".to_owned()),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            lease_asset_definition_id: hf_shared_lease_asset_definition(),
            base_fee_nanos: 20_000,
        };
        let encoded =
            encode_hf_lease_renew_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "0123456789abcdef",
            "gpt_oss_20b",
            "vision_portal",
            Some("ops_agent"),
            StorageClass::Warm,
            604_800_000_u64,
            hf_shared_lease_asset_definition(),
            20_000_u128,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn ensure_hf_generated_service_instruction_reuses_matching_existing_service()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", "main")
                .map_err(|err| eyre::eyre!("hf source id failed: {}", err.message))?;
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let mut world = World::default();
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: bundle.service.service_name.clone(),
                        current_service_version: bundle.service.service_version.clone(),
                        current_service_manifest_hash: bundle.service_manifest_hash(),
                        current_container_manifest_hash: bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.signatory().clone(),
            };

            let instruction = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
                None,
            )
            .map_err(|err| eyre::eyre!("ensure service instruction failed: {}", err.message))?;
            assert!(instruction.is_none());
            Ok(())
        })
    }

    #[test]
    fn ensure_hf_generated_service_instruction_requires_generated_provenance() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let app = mk_app_state_for_tests_with_world(World::default());
            let signer = test_soracloud_mutation_signer(&KeyPair::random());

            let error = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
                None,
            )
            .expect_err("new HF-generated service must require auxiliary provenance");

            assert_eq!(error.status(), StatusCode::BAD_REQUEST);
            assert!(
                error
                    .message
                    .contains("generated_service_provenance is required")
            );
        });
    }

    #[test]
    fn ensure_hf_generated_service_instruction_accepts_valid_generated_provenance() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let key_pair = KeyPair::random();
            let signer = test_soracloud_mutation_signer(&key_pair);
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let provenance = signed_generated_service_provenance(&bundle, &key_pair);
            let app = mk_app_state_for_tests_with_world(World::default());

            let instruction = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
                Some(&provenance),
            )
            .expect("valid generated provenance should be accepted");

            assert!(instruction.is_some());
        });
    }

    #[test]
    fn ensure_hf_generated_service_instruction_rejects_signer_mismatch() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let signer_keypair = KeyPair::random();
            let provenance_keypair = KeyPair::random();
            let signer = test_soracloud_mutation_signer(&signer_keypair);
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let provenance = signed_generated_service_provenance(&bundle, &provenance_keypair);
            let app = mk_app_state_for_tests_with_world(World::default());

            let error = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
                Some(&provenance),
            )
            .expect_err("mismatched generated service signer must be rejected");

            assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
            assert!(
                error
                    .message
                    .contains("generated service provenance signer must match")
            );
        });
    }

    #[test]
    fn ensure_hf_generated_service_instruction_rejects_unrelated_existing_service() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let expected_bundle = build_soracloud_hf_generated_service_bundle(
                "web_portal".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let mut existing_bundle = fixture_bundle("1.0.0");
            existing_bundle.service.service_name =
                "web_portal".parse().expect("valid service name");
            existing_bundle.service.container.manifest_hash =
                existing_bundle.container_manifest_hash();

            let mut world = World::default();
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    existing_bundle.service.service_name.to_string(),
                    existing_bundle.service.service_version.clone(),
                ),
                existing_bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    existing_bundle.service.service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: existing_bundle.service.service_name.clone(),
                        current_service_version: existing_bundle.service.service_version.clone(),
                        current_service_manifest_hash: existing_bundle.service_manifest_hash(),
                        current_container_manifest_hash: existing_bundle.container_manifest_hash(),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                    },
                );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.signatory().clone(),
            };

            let error = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &expected_bundle,
                &source_id,
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
                None,
            )
            .expect_err("unrelated existing service should be rejected");
            assert!(
                error
                    .message
                    .contains("not an auto-generated HF inference service")
            );
        });
    }

    #[test]
    fn ensure_hf_generated_agent_instruction_reuses_matching_existing_apartment()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", "main")
                .map_err(|err| eyre::eyre!("hf source id failed: {}", err.message))?;
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let manifest = build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent".parse().expect("valid apartment name"),
                &bundle,
            );
            let mut world = World::default();
            world.soracloud_agent_apartments_mut_for_testing().insert(
                manifest.apartment_name.to_string(),
                SoraAgentApartmentRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
                    manifest_hash: Hash::new(Encode::encode(&manifest)),
                    manifest: manifest.clone(),
                    status: SoraAgentRuntimeStatusV1::Running,
                    deployed_sequence: 1,
                    lease_started_sequence: 1,
                    lease_expires_sequence: 100,
                    last_renewed_sequence: 1,
                    restart_count: 0,
                    last_restart_sequence: None,
                    last_restart_reason: None,
                    process_generation: 1,
                    process_started_sequence: 1,
                    last_active_sequence: 1,
                    last_checkpoint_sequence: None,
                    checkpoint_count: 0,
                    persistent_state: iroha_data_model::soracloud::SoraAgentPersistentStateV1 {
                        total_bytes: 0,
                        key_sizes: BTreeMap::new(),
                    },
                    revoked_policy_capabilities: BTreeSet::new(),
                    pending_wallet_requests: BTreeMap::new(),
                    wallet_daily_spend: BTreeMap::new(),
                    mailbox_queue: Vec::new(),
                    autonomy_budget_ceiling_units: AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS,
                    autonomy_budget_remaining_units: AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS,
                    artifact_allowlist: BTreeMap::new(),
                    autonomy_run_history: Vec::new(),
                    uploaded_model_binding: None,
                },
            );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.signatory().clone(),
            };

            let instruction = ensure_hf_generated_agent_instruction(&app, &signer, &manifest, None)
                .map_err(|err| {
                    eyre::eyre!("ensure apartment instruction failed: {}", err.message)
                })?;
            assert!(instruction.is_none());
            Ok(())
        })
    }

    #[test]
    fn ensure_hf_generated_agent_instruction_requires_generated_provenance() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let manifest = build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent".parse().expect("valid apartment name"),
                &bundle,
            );
            let app = mk_app_state_for_tests_with_world(World::default());
            let signer = test_soracloud_mutation_signer(&KeyPair::random());

            let error = ensure_hf_generated_agent_instruction(&app, &signer, &manifest, None)
                .expect_err("new HF-generated apartment must require auxiliary provenance");

            assert_eq!(error.status(), StatusCode::BAD_REQUEST);
            assert!(
                error
                    .message
                    .contains("generated_apartment_provenance is required")
            );
        });
    }

    #[test]
    fn ensure_hf_generated_agent_instruction_accepts_valid_generated_provenance() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let key_pair = KeyPair::random();
            let signer = test_soracloud_mutation_signer(&key_pair);
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let manifest = build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent".parse().expect("valid apartment name"),
                &bundle,
            );
            let provenance = signed_generated_apartment_provenance(&manifest, &key_pair);
            let app = mk_app_state_for_tests_with_world(World::default());

            let instruction =
                ensure_hf_generated_agent_instruction(&app, &signer, &manifest, Some(&provenance))
                    .expect("valid generated provenance should be accepted");

            assert!(instruction.is_some());
        });
    }

    #[test]
    fn ensure_hf_generated_agent_instruction_rejects_signer_mismatch() {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async move {
            let signer_keypair = KeyPair::random();
            let provenance_keypair = KeyPair::random();
            let signer = test_soracloud_mutation_signer(&signer_keypair);
            let source_id = hf_source_id("openai/gpt-oss", "main").expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                "main",
                "gpt_oss_20b",
            );
            let manifest = build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent".parse().expect("valid apartment name"),
                &bundle,
            );
            let provenance = signed_generated_apartment_provenance(&manifest, &provenance_keypair);
            let app = mk_app_state_for_tests_with_world(World::default());

            let error =
                ensure_hf_generated_agent_instruction(&app, &signer, &manifest, Some(&provenance))
                    .expect_err("mismatched generated apartment signer must be rejected");

            assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
            assert!(
                error
                    .message
                    .contains("generated apartment provenance signer must match")
            );
        });
    }

    #[test]
    fn admit_scr_host_bundle_exposes_model_inference_capability() {
        let mut bundle = fixture_bundle("2026.04.0");
        bundle.container.capabilities.allow_model_inference = true;
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();

        let admission = admit_scr_host_bundle(&bundle).expect("SCR admission should succeed");
        assert!(admission.allow_model_inference);

        let deployment = SoraServiceDeploymentStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            current_service_version: bundle.service.service_version.clone(),
            current_service_manifest_hash: bundle.service_manifest_hash(),
            current_container_manifest_hash: bundle.container_manifest_hash(),
            revision_count: 1,
            process_generation: 1,
            process_started_sequence: 1,
            active_rollout: None,
            last_rollout: None,
            config_generation: 0,
            secret_generation: 0,
            service_configs: BTreeMap::new(),
            service_secrets: BTreeMap::new(),
        };
        let revision = deployment_bundle_to_control_plane_revision(&deployment, &bundle, None);
        assert!(revision.allow_model_inference);
    }

    #[test]
    fn parse_storage_class_query_accepts_case_insensitive_labels() {
        assert_eq!(
            parse_storage_class_query("warm").expect("warm should parse"),
            StorageClass::Warm
        );
        assert_eq!(
            parse_storage_class_query("Hot").expect("Hot should parse"),
            StorageClass::Hot
        );
        assert!(parse_storage_class_query("archive").is_err());
    }

    #[test]
    fn authoritative_hf_shared_lease_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = "0123456789abcdef";
            let model_name = "gpt_oss_20b";
            let storage_class = StorageClass::Warm;
            let lease_term_ms = 604_800_000_u64;
            let source_id = hf_source_id(repo_id, resolved_revision)
                .map_err(|err| eyre::eyre!("failed to derive hf source id: {err:?}"))?;
            let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)
                .map_err(|err| eyre::eyre!("failed to derive hf pool id: {err:?}"))?;
            let asset_definition = hf_shared_lease_asset_definition();
            let mut world = World::default();

            world.soracloud_hf_sources_mut_for_testing().insert(
                source_id,
                SoraHfSourceRecordV1 {
                    schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                    source_id,
                    repo_id: repo_id.to_owned(),
                    resolved_revision: resolved_revision.to_owned(),
                    model_name: model_name.to_owned(),
                    adapter_id: "hf.shared.v1".to_owned(),
                    normalized_runtime_hash: Hash::new(b"hf-runtime"),
                    resource_profile: None,
                    status: SoraHfSourceStatusV1::PendingImport,
                    created_at_ms: 10,
                    updated_at_ms: 20,
                    last_error: None,
                },
            );
            world
                .soracloud_hf_shared_lease_pools_mut_for_testing()
                .insert(
                    pool_id,
                    SoraHfSharedLeasePoolV1 {
                        schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
                        pool_id,
                        source_id,
                        storage_class,
                        lease_asset_definition_id: asset_definition.clone(),
                        base_fee_nanos: 10_000,
                        lease_term_ms,
                        window_started_at_ms: 10,
                        window_expires_at_ms: lease_term_ms + 10,
                        active_member_count: 1,
                        status: SoraHfSharedLeaseStatusV1::Active,
                        queued_next_window: Some(
                            iroha_data_model::soracloud::SoraHfSharedLeaseQueuedWindowV1 {
                                sponsor_account_id: ALICE_ID.clone(),
                                model_name: "gpt_oss_20b_v2".to_owned(),
                                lease_asset_definition_id: asset_definition.clone(),
                                base_fee_nanos: 20_000,
                                compute_reservation_fee_nanos: 8_000,
                                planned_placement:
                                    iroha_data_model::soracloud::SoraHfPlacementRecordV1 {
                                        schema_version:
                                            iroha_data_model::soracloud::SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                                        placement_id: Hash::new(b"queued-placement"),
                                        source_id,
                                        pool_id,
                                        status:
                                            iroha_data_model::soracloud::SoraHfPlacementStatusV1::Selecting,
                                        selection_seed_hash: Hash::new(b"queued-seed"),
                                        resource_profile:
                                            iroha_data_model::soracloud::SoraHfResourceProfileV1 {
                                                required_model_bytes: 4_096,
                                                backend_family:
                                                    iroha_data_model::soracloud::SoraHfBackendFamilyV1::Transformers,
                                                model_format:
                                                    iroha_data_model::soracloud::SoraHfModelFormatV1::Safetensors,
                                                disk_cache_bytes_floor: 8_192,
                                                ram_bytes_floor: 8_192,
                                                vram_bytes_floor: 0,
                                            },
                                        eligible_validator_count: 0,
                                        adaptive_target_host_count: 1,
                                        assigned_hosts: Vec::new(),
                                        total_reservation_fee_nanos: 8_000,
                                        last_rebalance_at_ms: 15,
                                        last_error: None,
                                    },
                                sponsored_at_ms: 15,
                                window_started_at_ms: lease_term_ms + 10,
                                window_expires_at_ms: (lease_term_ms * 2) + 10,
                                service_name: "vision_portal_v2".parse().expect("service"),
                                apartment_name: Some("ops_agent_v2".parse().expect("apartment")),
                            },
                        ),
                    },
                );
            world
                .soracloud_hf_shared_lease_members_mut_for_testing()
                .insert(
                    (pool_id.to_string(), ALICE_ID.to_string()),
                    SoraHfSharedLeaseMemberV1 {
                        schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                        pool_id,
                        source_id,
                        account_id: ALICE_ID.clone(),
                        status: SoraHfSharedLeaseMemberStatusV1::Active,
                        joined_at_ms: 10,
                        updated_at_ms: 20,
                        total_paid_nanos: 10_000,
                        total_refunded_nanos: 0,
                        last_charge_nanos: 10_000,
                        total_compute_paid_nanos: 0,
                        total_compute_refunded_nanos: 0,
                        last_compute_charge_nanos: 0,
                        service_bindings: std::collections::BTreeSet::from([
                            "vision_portal".to_owned()
                        ]),
                        apartment_bindings: std::collections::BTreeSet::from([
                            "ops_agent".to_owned()
                        ]),
                    },
                );
            world
                .soracloud_hf_shared_lease_audit_events_mut_for_testing()
                .insert(
                    7,
                    SoraHfSharedLeaseAuditEventV1 {
                        schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                        sequence: 7,
                        action: SoraHfSharedLeaseActionV1::CreateWindow,
                        pool_id,
                        source_id,
                        account_id: ALICE_ID.clone(),
                        occurred_at_ms: 10,
                        active_member_count: 1,
                        charged_nanos: 10_000,
                        refunded_nanos: 0,
                        lease_expires_at_ms: lease_term_ms + 10,
                        service_name: Some("vision_portal".to_owned()),
                        apartment_name: Some("ops_agent".to_owned()),
                    },
                );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_hf_shared_lease_status_response(
                &app,
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                Some(&ALICE_ID),
            )
            .map_err(|err| eyre::eyre!("authoritative hf status failed: {err:?}"))?;

            assert_eq!(
                response.schema_version,
                HF_SHARED_LEASE_STATUS_SCHEMA_VERSION_V1
            );
            assert_eq!(response.source.source_id, source_id);
            assert_eq!(response.pool.as_ref().expect("pool").pool_id, pool_id);
            assert_eq!(
                response.member.as_ref().expect("member").account_id,
                ALICE_ID.clone()
            );
            assert_eq!(response.audit_event_count, 1);
            assert!(response.importer_pending);
            assert!(response.runtime_projection.is_none());
            assert_eq!(
                response
                    .latest_audit_event
                    .as_ref()
                    .expect("audit event")
                    .action,
                SoraHfSharedLeaseActionV1::CreateWindow
            );
            assert_eq!(
                response
                    .pool
                    .as_ref()
                    .expect("pool")
                    .queued_next_window
                    .as_ref()
                    .expect("queued next window")
                    .service_name
                    .as_ref(),
                "vision_portal_v2"
            );
            Ok(())
        })
    }

    #[test]
    fn authoritative_hf_shared_lease_mutation_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = "0123456789abcdef";
            let model_name = "gpt_oss_20b";
            let storage_class = StorageClass::Warm;
            let lease_term_ms = 604_800_000_u64;
            let source_id = hf_source_id(repo_id, resolved_revision)
                .map_err(|err| eyre::eyre!("failed to derive hf source id: {err:?}"))?;
            let pool_id = hf_shared_lease_pool_id(source_id, storage_class, lease_term_ms)
                .map_err(|err| eyre::eyre!("failed to derive hf pool id: {err:?}"))?;
            let asset_definition = hf_shared_lease_asset_definition();
            let mut world = World::default();

            world.soracloud_hf_sources_mut_for_testing().insert(
                source_id,
                SoraHfSourceRecordV1 {
                    schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                    source_id,
                    repo_id: repo_id.to_owned(),
                    resolved_revision: resolved_revision.to_owned(),
                    model_name: model_name.to_owned(),
                    adapter_id: "hf.shared.v1".to_owned(),
                    normalized_runtime_hash: Hash::new(b"hf-runtime"),
                    resource_profile: None,
                    status: SoraHfSourceStatusV1::Ready,
                    created_at_ms: 10,
                    updated_at_ms: 30,
                    last_error: None,
                },
            );
            world
                .soracloud_hf_shared_lease_pools_mut_for_testing()
                .insert(
                    pool_id,
                    SoraHfSharedLeasePoolV1 {
                        schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
                        pool_id,
                        source_id,
                        storage_class,
                        lease_asset_definition_id: asset_definition.clone(),
                        base_fee_nanos: 10_000,
                        lease_term_ms,
                        window_started_at_ms: 10,
                        window_expires_at_ms: lease_term_ms + 10,
                        active_member_count: 2,
                        status: SoraHfSharedLeaseStatusV1::Active,
                        queued_next_window: None,
                    },
                );
            world
                .soracloud_hf_shared_lease_members_mut_for_testing()
                .insert(
                    (pool_id.to_string(), ALICE_ID.to_string()),
                    SoraHfSharedLeaseMemberV1 {
                        schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                        pool_id,
                        source_id,
                        account_id: ALICE_ID.clone(),
                        status: SoraHfSharedLeaseMemberStatusV1::Active,
                        joined_at_ms: 30,
                        updated_at_ms: 30,
                        total_paid_nanos: 13_333,
                        total_refunded_nanos: 0,
                        last_charge_nanos: 3_333,
                        total_compute_paid_nanos: 0,
                        total_compute_refunded_nanos: 0,
                        last_compute_charge_nanos: 0,
                        service_bindings: std::collections::BTreeSet::from([
                            "vision_portal".to_owned()
                        ]),
                        apartment_bindings: std::collections::BTreeSet::from([
                            "ops_agent".to_owned()
                        ]),
                    },
                );
            world
                .soracloud_hf_shared_lease_audit_events_mut_for_testing()
                .insert(
                    5,
                    SoraHfSharedLeaseAuditEventV1 {
                        schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                        sequence: 5,
                        action: SoraHfSharedLeaseActionV1::Join,
                        pool_id,
                        source_id,
                        account_id: ALICE_ID.clone(),
                        occurred_at_ms: 30,
                        active_member_count: 2,
                        charged_nanos: 3_333,
                        refunded_nanos: 0,
                        lease_expires_at_ms: lease_term_ms + 10,
                        service_name: Some("vision_portal".to_owned()),
                        apartment_name: Some("ops_agent".to_owned()),
                    },
                );

            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_hf_shared_lease_mutation_response(
                &app,
                &SoracloudAuditBaseline {
                    hf_shared_lease_max: 4,
                    ..SoracloudAuditBaseline::default()
                },
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                &ALICE_ID,
                Some("vision_portal"),
                Some("ops_agent"),
            )
            .map_err(|err| eyre::eyre!("authoritative hf mutation failed: {err:?}"))?;

            assert_eq!(response.action, SoraHfSharedLeaseActionV1::Join);
            assert_eq!(response.source.status, SoraHfSourceStatusV1::Ready);
            assert_eq!(response.pool.pool_id, pool_id);
            assert_eq!(response.member.account_id, ALICE_ID.clone());
            assert_eq!(
                response
                    .latest_audit_event
                    .as_ref()
                    .expect("audit event")
                    .sequence,
                5
            );
            assert!(!response.importer_pending);
            assert!(response.runtime_projection.is_none());
            Ok(())
        })
    }

    #[test]
    fn authoritative_hf_shared_lease_status_uses_runtime_projection_when_available()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = "0123456789abcdef";
            let source_id = hf_source_id(repo_id, resolved_revision)
                .map_err(|err| eyre::eyre!("failed to derive hf source id: {err:?}"))?;
            let mut world = World::default();
            world.soracloud_hf_sources_mut_for_testing().insert(
                source_id,
                SoraHfSourceRecordV1 {
                    schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                    source_id,
                    repo_id: repo_id.to_owned(),
                    resolved_revision: resolved_revision.to_owned(),
                    model_name: "gpt_oss_20b".to_owned(),
                    adapter_id: "hf.shared.v1".to_owned(),
                    normalized_runtime_hash: Hash::new(b"hf-runtime"),
                    resource_profile: None,
                    status: SoraHfSourceStatusV1::PendingImport,
                    created_at_ms: 10,
                    updated_at_ms: 20,
                    last_error: None,
                },
            );

            let mut app = mk_app_state_for_tests_with_world(world);
            let runtime_snapshot = SoracloudRuntimeSnapshot {
                hf_sources: std::collections::BTreeMap::from([(
                    source_id.to_string(),
                    SoracloudRuntimeHfSourcePlan {
                        source_id: source_id.to_string(),
                        repo_id: repo_id.to_owned(),
                        resolved_revision: resolved_revision.to_owned(),
                        model_name: "gpt_oss_20b".to_owned(),
                        adapter_id: "hf.shared.v1".to_owned(),
                        authoritative_status: SoraHfSourceStatusV1::PendingImport,
                        runtime_status: SoracloudRuntimeHfSourceStatus::Ready,
                        pool_count: 1,
                        active_pool_count: 1,
                        active_member_count: 1,
                        queued_window_count: 0,
                        bound_service_count: 1,
                        bound_service_names: vec!["vision_portal".to_owned()],
                        materialized_service_count: 1,
                        materialized_service_names: vec!["vision_portal".to_owned()],
                        hydrating_service_count: 0,
                        bound_apartment_count: 0,
                        bound_apartment_names: Vec::new(),
                        materialized_apartment_count: 0,
                        materialized_apartment_names: Vec::new(),
                        bundle_cache_miss_count: 0,
                        artifact_cache_miss_count: 0,
                        last_error: None,
                    },
                )]),
                ..SoracloudRuntimeSnapshot::default()
            };
            Arc::get_mut(&mut app)
                .expect("unique app state")
                .soracloud_runtime = Some(Arc::new(TestHfRuntimeHandle {
                snapshot: runtime_snapshot,
                state_dir: PathBuf::from("/tmp/soracloud/hf-runtime"),
            }));

            let response = authoritative_hf_shared_lease_status_response(
                &app,
                repo_id,
                resolved_revision,
                StorageClass::Warm,
                60_000,
                None,
            )
            .map_err(|err| eyre::eyre!("authoritative hf status failed: {err:?}"))?;

            assert_eq!(response.source.status, SoraHfSourceStatusV1::PendingImport);
            assert!(!response.importer_pending);
            assert_eq!(
                response
                    .runtime_projection
                    .as_ref()
                    .expect("runtime projection")
                    .runtime_status,
                SoracloudRuntimeHfSourceStatus::Ready
            );
            Ok(())
        })
    }

    #[test]
    fn hf_importer_pending_false_for_failed_sources_without_runtime() {
        let source = SoraHfSourceRecordV1 {
            schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
            source_id: Hash::new(b"hf-source"),
            repo_id: "openai/gpt-oss".to_owned(),
            resolved_revision: "main".to_owned(),
            model_name: "gpt_oss_20b".to_owned(),
            adapter_id: "hf.shared.v1".to_owned(),
            normalized_runtime_hash: Hash::new(b"hf-runtime"),
            resource_profile: None,
            status: SoraHfSourceStatusV1::Failed,
            created_at_ms: 10,
            updated_at_ms: 20,
            last_error: Some("download failed".to_owned()),
        };

        assert!(!hf_importer_pending(&source, None));
    }

    #[test]
    fn authoritative_agent_runtime_receipt_for_run_prefers_latest_receipt() {
        use iroha_core::state::World;

        let request_commitment = Hash::new(b"ops-agent-request");
        let run = SoraAgentAutonomyRunRecordV1 {
            run_id: "ops_agent:autonomy:9".to_owned(),
            artifact_hash: "hash:artifact#1".to_owned(),
            provenance_hash: Some("hash:prov#1".to_owned()),
            budget_units: 25,
            run_label: "nightly".to_owned(),
            workflow_input_json: Some("{\"inputs\":\"nightly\"}".to_owned()),
            approved_process_generation: 1,
            request_commitment,
            approved_sequence: 9,
        };
        let service_name: Name = "hf_agent_service".parse().expect("valid service name");
        let handler_name: Name = "infer".parse().expect("valid handler name");
        let mut world = World::default();
        world.soracloud_runtime_receipts_mut_for_testing().insert(
            Hash::new(b"ops-agent-receipt-older"),
            SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(b"ops-agent-receipt-older"),
                service_name: service_name.clone(),
                service_version: "hf.generated.v1".to_owned(),
                handler_name: handler_name.clone(),
                handler_class: SoraServiceHandlerClassV1::Query,
                request_commitment,
                result_commitment: Hash::new(b"ops-agent-result-older"),
                certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                emitted_sequence: 40,
                mailbox_message_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                placement_id: None,
                selected_validator_account_id: None,
                selected_peer_id: None,
            },
        );
        world.soracloud_runtime_receipts_mut_for_testing().insert(
            Hash::new(b"ops-agent-receipt-newer"),
            SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(b"ops-agent-receipt-newer"),
                service_name,
                service_version: "hf.generated.v1".to_owned(),
                handler_name,
                handler_class: SoraServiceHandlerClassV1::Query,
                request_commitment,
                result_commitment: Hash::new(b"ops-agent-result-newer"),
                certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                emitted_sequence: 41,
                mailbox_message_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                placement_id: None,
                selected_validator_account_id: None,
                selected_peer_id: None,
            },
        );

        let world_view = world.view();
        let receipt = authoritative_agent_runtime_receipt_for_run(&world_view, &run)
            .expect("matching receipt should be resolved");
        assert_eq!(receipt.receipt_id, Hash::new(b"ops-agent-receipt-newer"));
        assert_eq!(receipt.emitted_sequence, 41);
        assert_eq!(
            receipt.result_commitment,
            Hash::new(b"ops-agent-result-newer")
        );
    }

    #[test]
    fn handle_agent_autonomy_run_finalize_rejects_signer_mismatch() -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let apartment_name = "ops_agent";
            let run = fixture_agent_run_record(apartment_name, "ops_agent:autonomy:1", 7, 1);
            let manifest = fixture_agent_manifest();
            let record = fixture_agent_apartment_record(manifest.clone(), run.clone(), 1);
            let approval_signer = KeyPair::random();
            let finalize_signer = KeyPair::random();

            let mut world = World::default();
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment_name.to_owned(), record.clone());
            world
                .soracloud_agent_apartment_audit_events_mut_for_testing()
                .insert(
                    run.approved_sequence,
                    fixture_autonomy_approval_event(
                        apartment_name,
                        record.manifest_hash,
                        &approval_signer,
                        &run,
                    ),
                );

            let mut app = mk_app_state_for_tests_with_world(world);
            let temp_dir = tempfile::tempdir()?;
            attach_test_runtime(&mut app, temp_dir.path().to_path_buf());

            let account = AccountId::new(finalize_signer.public_key().clone());
            let headers = verified_request_headers(&account, finalize_signer.public_key());
            let response = handle_agent_autonomy_run_finalize(
                State(app),
                headers,
                NoritoJson(AgentAutonomyFinalizeRequest {
                    apartment_name: apartment_name.to_owned(),
                    run_id: run.run_id.clone(),
                }),
            )
            .await;

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            Ok(())
        })
    }

    #[test]
    fn execute_runtime_agent_autonomy_run_returns_none_for_non_hf_apartment()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let apartment_name = "ops_agent";
            let run = fixture_agent_run_record(apartment_name, "ops_agent:autonomy:2", 9, 1);
            let mut manifest = fixture_agent_manifest();
            manifest.tool_capabilities.clear();
            let record = fixture_agent_apartment_record(manifest, run.clone(), 1);
            let signer = KeyPair::random();

            let mut world = World::default();
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment_name.to_owned(), record.clone());
            world
                .soracloud_agent_apartment_audit_events_mut_for_testing()
                .insert(
                    run.approved_sequence,
                    fixture_autonomy_approval_event(
                        apartment_name,
                        record.manifest_hash,
                        &signer,
                        &run,
                    ),
                );

            let mut app = mk_app_state_for_tests_with_world(world);
            let temp_dir = tempfile::tempdir()?;
            attach_test_runtime(&mut app, temp_dir.path().to_path_buf());

            let response = authoritative_agent_autonomy_mutation_response(
                &app,
                &record,
                &fixture_autonomy_approval_event(
                    apartment_name,
                    record.manifest_hash,
                    &signer,
                    &run,
                ),
            )
            .map_err(|err| eyre::eyre!("autonomy mutation response failed: {err:?}"))?;
            let runtime_execution = execute_runtime_agent_autonomy_run(&app, &response)
                .map_err(|error| eyre::eyre!("runtime execution check failed: {error}"))?;
            assert!(runtime_execution.is_none());
            Ok(())
        })
    }

    #[test]
    fn build_authoritative_agent_runtime_receipt_instruction_is_idempotent()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let receipt_id = Hash::new(b"ops-agent-runtime-receipt");
            let request_commitment = Hash::new(b"ops-agent-runtime-request");
            let result_commitment = Hash::new(b"ops-agent-runtime-result");
            let journal_artifact_hash = Hash::new(b"ops-agent-runtime-journal");
            let checkpoint_artifact_hash = Hash::new(b"ops-agent-runtime-checkpoint");
            let mut world = World::default();
            world.soracloud_runtime_receipts_mut_for_testing().insert(
                receipt_id,
                SoraRuntimeReceiptV1 {
                    schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                    receipt_id,
                    service_name: "hf_agent_service".parse().expect("valid service name"),
                    service_version: "hf.generated.v1".to_owned(),
                    handler_name: "infer".parse().expect("valid handler name"),
                    handler_class: SoraServiceHandlerClassV1::Query,
                    request_commitment,
                    result_commitment,
                    certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    emitted_sequence: 77,
                    mailbox_message_id: None,
                    journal_artifact_hash: Some(journal_artifact_hash),
                    checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                },
            );

            let app = mk_app_state_for_tests_with_world(world);
            let summary = AgentRuntimeExecutionSummary {
                apartment_name: "ops_agent".to_owned(),
                run_id: "ops_agent:autonomy:runtime".to_owned(),
                service_name: Some("hf_agent_service".to_owned()),
                service_version: Some("hf.generated.v1".to_owned()),
                handler_name: Some("infer".to_owned()),
                succeeded: true,
                result_commitment,
                journal_artifact_hash,
                checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                runtime_receipt: Some(AgentRuntimeReceiptRecord {
                    receipt_id,
                    service_name: "hf_agent_service".to_owned(),
                    service_version: "hf.generated.v1".to_owned(),
                    handler_name: "infer".to_owned(),
                    handler_class: SoraServiceHandlerClassV1::Query,
                    request_commitment,
                    result_commitment,
                    certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    emitted_sequence: 77,
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                    journal_artifact_hash: Some(journal_artifact_hash),
                    checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                }),
                workflow_steps: Vec::new(),
                content_type: None,
                response_json: None,
                response_text: None,
                error: None,
            };

            let instruction = build_authoritative_agent_runtime_receipt_instruction(&app, &summary)
                .expect("idempotent receipt helper should succeed");
            assert!(instruction.is_none());
            Ok(())
        })
    }

    #[test]
    fn build_authoritative_agent_autonomy_execution_audit_instruction_is_idempotent()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let result_commitment = Hash::new(b"ops-agent-executed-result");
            let runtime_receipt_id = Hash::new(b"ops-agent-executed-receipt");
            let journal_artifact_hash = Hash::new(b"ops-agent-executed-journal");
            let checkpoint_artifact_hash = Hash::new(b"ops-agent-executed-checkpoint");
            let mut world = World::default();
            world
                .soracloud_agent_apartment_audit_events_mut_for_testing()
                .insert(
                    88,
                    SoraAgentApartmentAuditEventV1 {
                        schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                        sequence: 88,
                        action: SoraAgentApartmentActionV1::AutonomyRunExecuted,
                        apartment_name: "ops_agent".parse().expect("valid apartment name"),
                        status: SoraAgentRuntimeStatusV1::Running,
                        lease_expires_sequence: 100,
                        manifest_hash: Hash::new(b"agent-manifest"),
                        restart_count: 0,
                        signer: KeyPair::random().public_key().clone(),
                        request_id: Some("ops_agent:autonomy:executed".to_owned()),
                        asset_definition: None,
                        amount_nanos: None,
                        capability: None,
                        reason: None,
                        from_apartment: None,
                        to_apartment: None,
                        channel: None,
                        payload_hash: None,
                        artifact_hash: Some("hash:artifact#1".to_owned()),
                        provenance_hash: Some("hash:prov#1".to_owned()),
                        run_id: Some("ops_agent:autonomy:executed".to_owned()),
                        run_label: Some("nightly".to_owned()),
                        budget_units: Some(25),
                        service_name: Some("hf_agent_service".to_owned()),
                        service_version: Some("hf.generated.v1".to_owned()),
                        handler_name: Some("infer".to_owned()),
                        result_commitment: Some(result_commitment),
                        runtime_receipt_id: Some(runtime_receipt_id),
                        journal_artifact_hash: Some(journal_artifact_hash),
                        checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                        succeeded: Some(true),
                    },
                );

            let app = mk_app_state_for_tests_with_world(world);
            let summary = AgentRuntimeExecutionSummary {
                apartment_name: "ops_agent".to_owned(),
                run_id: "ops_agent:autonomy:executed".to_owned(),
                service_name: Some("hf_agent_service".to_owned()),
                service_version: Some("hf.generated.v1".to_owned()),
                handler_name: Some("infer".to_owned()),
                succeeded: true,
                result_commitment,
                journal_artifact_hash,
                checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                runtime_receipt: Some(AgentRuntimeReceiptRecord {
                    receipt_id: runtime_receipt_id,
                    service_name: "hf_agent_service".to_owned(),
                    service_version: "hf.generated.v1".to_owned(),
                    handler_name: "infer".to_owned(),
                    handler_class: SoraServiceHandlerClassV1::Query,
                    request_commitment: Hash::new(b"ops-agent-executed-request"),
                    result_commitment,
                    certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    emitted_sequence: 88,
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                    journal_artifact_hash: Some(journal_artifact_hash),
                    checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
                }),
                workflow_steps: Vec::new(),
                content_type: None,
                response_json: None,
                response_text: None,
                error: None,
            };

            let instruction = build_authoritative_agent_autonomy_execution_audit_instruction(
                &app,
                "ops_agent",
                1,
                &summary,
                Some(runtime_receipt_id),
            )
            .expect("idempotent audit helper should succeed");
            assert!(instruction.is_none());
            Ok(())
        })
    }

    #[test]
    fn authoritative_agent_autonomy_status_includes_runtime_recent_runs() -> Result<(), eyre::Report>
    {
        use iroha_core::state::World;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let temp_dir = tempfile::tempdir()?;
            let mut world = World::default();
            let manifest = fixture_agent_manifest();
            let request_commitment =
                iroha_data_model::soracloud::derive_agent_autonomy_request_commitment(
                    "ops_agent",
                    "hash:artifact#1",
                    Some("hash:prov#1"),
                    25,
                    "ops_agent:autonomy:7",
                    "nightly",
                    Some("{\"inputs\":\"nightly\"}"),
                    1,
                );
            let run = SoraAgentAutonomyRunRecordV1 {
                run_id: "ops_agent:autonomy:7".to_owned(),
                artifact_hash: "hash:artifact#1".to_owned(),
                provenance_hash: Some("hash:prov#1".to_owned()),
                budget_units: 25,
                run_label: "nightly".to_owned(),
                workflow_input_json: Some("{\"inputs\":\"nightly\"}".to_owned()),
                approved_process_generation: 1,
                request_commitment,
                approved_sequence: 7,
            };
            world.soracloud_agent_apartments_mut_for_testing().insert(
                manifest.apartment_name.to_string(),
                SoraAgentApartmentRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
                    manifest_hash: Hash::new(b"agent-manifest"),
                    status: SoraAgentRuntimeStatusV1::Running,
                    deployed_sequence: 1,
                    lease_started_sequence: 1,
                    lease_expires_sequence: 100,
                    last_renewed_sequence: 1,
                    restart_count: 0,
                    last_restart_sequence: None,
                    last_restart_reason: None,
                    process_generation: 1,
                    process_started_sequence: 1,
                    last_active_sequence: 7,
                    last_checkpoint_sequence: Some(7),
                    checkpoint_count: 1,
                    persistent_state: SoraAgentPersistentStateV1 {
                        total_bytes: 64,
                        key_sizes: BTreeMap::from([("/agent/checkpoint/7".to_owned(), 64)]),
                    },
                    revoked_policy_capabilities: BTreeSet::new(),
                    pending_wallet_requests: BTreeMap::new(),
                    wallet_daily_spend: BTreeMap::new(),
                    mailbox_queue: Vec::new(),
                    autonomy_budget_ceiling_units: 100,
                    autonomy_budget_remaining_units: 75,
                    uploaded_model_binding: None,
                    artifact_allowlist: BTreeMap::new(),
                    autonomy_run_history: vec![run.clone()],
                    manifest,
                },
            );
            world.soracloud_runtime_receipts_mut_for_testing().insert(
                Hash::new(b"ops-agent-authoritative-receipt"),
                SoraRuntimeReceiptV1 {
                    schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                    receipt_id: Hash::new(b"ops-agent-authoritative-receipt"),
                    service_name: "hf_agent_service".parse().expect("valid service name"),
                    service_version: "hf.generated.v1".to_owned(),
                    handler_name: "infer".parse().expect("valid handler name"),
                    handler_class: SoraServiceHandlerClassV1::Query,
                    request_commitment: run.request_commitment,
                    result_commitment: Hash::new(b"authoritative-runtime-result"),
                    certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    emitted_sequence: 77,
                    mailbox_message_id: None,
                    journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                    checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                },
            );
            world
                .soracloud_agent_apartment_audit_events_mut_for_testing()
                .insert(
                    78,
                    SoraAgentApartmentAuditEventV1 {
                        schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                        sequence: 78,
                        action: SoraAgentApartmentActionV1::AutonomyRunExecuted,
                        apartment_name: "ops_agent".parse().expect("valid apartment name"),
                        status: SoraAgentRuntimeStatusV1::Running,
                        lease_expires_sequence: 100,
                        manifest_hash: Hash::new(b"agent-manifest"),
                        restart_count: 0,
                        signer: KeyPair::random().public_key().clone(),
                        request_id: Some(run.run_id.clone()),
                        asset_definition: None,
                        amount_nanos: None,
                        capability: None,
                        reason: None,
                        from_apartment: None,
                        to_apartment: None,
                        channel: None,
                        payload_hash: None,
                        artifact_hash: Some(run.artifact_hash.clone()),
                        provenance_hash: run.provenance_hash.clone(),
                        run_id: Some(run.run_id.clone()),
                        run_label: Some(run.run_label.clone()),
                        budget_units: Some(run.budget_units),
                        service_name: Some("hf_agent_service".to_owned()),
                        service_version: Some("hf.generated.v1".to_owned()),
                        handler_name: Some("infer".to_owned()),
                        result_commitment: Some(Hash::new(b"authoritative-runtime-result")),
                        runtime_receipt_id: Some(Hash::new(b"ops-agent-authoritative-receipt")),
                        journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                        checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                        succeeded: Some(true),
                    },
                );

            let mut app = mk_app_state_for_tests_with_world(world);
            Arc::get_mut(&mut app)
                .expect("unique app state")
                .soracloud_runtime = Some(Arc::new(TestHfRuntimeHandle {
                snapshot: SoracloudRuntimeSnapshot::default(),
                state_dir: temp_dir.path().to_path_buf(),
            }));

            let summary_dir = temp_dir
                .path()
                .join("apartments")
                .join(sanitize_runtime_path_component("ops_agent"))
                .join("runs")
                .join(sanitize_runtime_path_component(&run.run_id));
            fs::create_dir_all(&summary_dir)?;
            let summary = SoracloudApartmentAutonomyExecutionSummaryV1 {
                schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
                apartment_name: "ops_agent".to_owned(),
                run_id: run.run_id.clone(),
                service_name: Some("hf_agent_service".to_owned()),
                service_version: Some("hf.generated.v1".to_owned()),
                handler_name: Some("infer".to_owned()),
                succeeded: true,
                result_commitment: Hash::new(b"runtime-result"),
                checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                runtime_receipt: Some(SoraRuntimeReceiptV1 {
                    schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                    receipt_id: Hash::new(b"ops-agent-authoritative-receipt"),
                    service_name: "hf_agent_service".parse().expect("valid service name"),
                    service_version: "hf.generated.v1".to_owned(),
                    handler_name: "infer".parse().expect("valid handler name"),
                    handler_class: SoraServiceHandlerClassV1::Query,
                    request_commitment: run.request_commitment,
                    result_commitment: Hash::new(b"authoritative-runtime-result"),
                    certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                    emitted_sequence: 77,
                    mailbox_message_id: None,
                    journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                    checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                }),
                workflow_steps: Vec::new(),
                content_type: Some("application/json".to_owned()),
                response_json: Some(norito::json!({"text":"ok","backend":"local_fixture"})),
                response_text: Some(r#"{"text":"ok","backend":"local_fixture"}"#.to_owned()),
                error: None,
            };
            let summary_bytes = norito::json::to_vec_pretty(&summary)?;
            fs::write(summary_dir.join("execution_summary.json"), &summary_bytes)?;

            let status = authoritative_agent_autonomy_status_response(&app, "ops_agent")
                .map_err(|err| eyre::eyre!("agent autonomy status failed: {err:?}"))?;
            assert_eq!(
                status.recent_runs[0].workflow_input_json.as_deref(),
                Some("{\"inputs\":\"nightly\"}")
            );
            assert_eq!(
                status.recent_runs[0]
                    .authoritative_runtime_receipt
                    .as_ref()
                    .map(|receipt| receipt.receipt_id),
                Some(Hash::new(b"ops-agent-authoritative-receipt"))
            );
            assert_eq!(
                status.recent_runs[0]
                    .authoritative_execution_audit
                    .as_ref()
                    .map(|audit| audit.sequence),
                Some(78)
            );
            assert_eq!(
                status.recent_runs[0]
                    .authoritative_execution_audit
                    .as_ref()
                    .and_then(|audit| audit.runtime_receipt_id),
                Some(Hash::new(b"ops-agent-authoritative-receipt"))
            );
            assert_eq!(
                status.recent_runs[0]
                    .authoritative_execution_audit
                    .as_ref()
                    .map(|audit| audit.succeeded),
                Some(true)
            );
            assert_eq!(status.runtime_recent_runs.len(), 1);
            assert_eq!(
                status.runtime_recent_runs[0].service_name.as_deref(),
                Some("hf_agent_service")
            );
            assert_eq!(
                status.runtime_recent_runs[0]
                    .runtime_receipt
                    .as_ref()
                    .map(|receipt| receipt.request_commitment),
                Some(run.request_commitment)
            );
            assert_eq!(
                status.runtime_recent_runs[0]
                    .response_json
                    .as_ref()
                    .and_then(|value| value.get("backend"))
                    .and_then(norito::json::Value::as_str),
                Some("local_fixture")
            );
            assert_eq!(
                status.runtime_recent_runs[0].journal_artifact_hash,
                Hash::new(&summary_bytes)
            );
            Ok(())
        })
    }
}
