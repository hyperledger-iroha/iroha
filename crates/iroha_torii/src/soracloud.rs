//! App-facing Soracloud control-plane shim.
//!
//! This module provides a deterministic control-plane surface for
//! `deploy`/`upgrade`/`rollback` workflows plus SCR host-admission snapshots.
//! Requests must carry signed payloads so admission can verify manifest
//! provenance before mutating registry state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    num::NonZeroU64,
    path::PathBuf,
};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    Encode,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, CIPHERTEXT_QUERY_PROOF_VERSION_V1,
        CIPHERTEXT_QUERY_RESPONSE_VERSION_V1, CiphertextInclusionProofV1,
        CiphertextQueryMetadataLevelV1, CiphertextQueryResponseV1, CiphertextQueryResultItemV1,
        CiphertextQuerySpecV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1,
        FheExecutionPolicyV1, FheGovernanceBundleV1, FheJobSpecV1, FheParamSetV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraNetworkPolicyV1, SoraStateBindingV1,
        SoraStateEncryptionV1, SoraStateMutabilityV1,
        encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload,
        encode_model_artifact_register_provenance_payload,
        encode_model_weight_promote_provenance_payload,
        encode_model_weight_register_provenance_payload,
        encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use tokio::sync::RwLock;

use crate::{JsonBody, NoritoJson, NoritoQuery, SharedAppState};

const REGISTRY_SCHEMA_VERSION: u16 = 1;
const DEFAULT_AUDIT_LIMIT: usize = 20;
const MAX_AUDIT_LIMIT: usize = 500;
const AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS: u64 = 1_000;
const AGENT_WALLET_DAY_TICKS: u64 = 10_000;
const AGENT_MAILBOX_MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_LABEL_BYTES: usize = 128;
const AGENT_AUTONOMY_RECENT_RUN_LIMIT: usize = 20;
const REGISTRY_PERSISTENCE_VERSION_V1: u8 = 1;
const CIPHERTEXT_QUERY_PROOF_SCHEME_V1: &str = "soracloud.audit_anchor.v1";
const HEALTH_COMPLIANCE_REPORT_VERSION_V1: u16 = 1;
const DEFAULT_HEALTH_COMPLIANCE_LIMIT: usize = 50;
const MAX_HEALTH_COMPLIANCE_LIMIT: usize = 500;
const TRAINING_JOB_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const TRAINING_MAX_RETRIES: u8 = 16;
const TRAINING_MAX_WORKER_GROUP_SIZE: u16 = 1024;
const TRAINING_MAX_REASON_BYTES: usize = 512;
const TRAINING_MAX_IDENTIFIER_BYTES: usize = 128;
const MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const MODEL_WEIGHT_MAX_DATASET_REF_BYTES: usize = 512;
const MODEL_WEIGHT_MAX_REASON_BYTES: usize = 512;
const MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedBundleRequest {
    pub bundle: SoraDeploymentBundleV1,
    pub provenance: ManifestProvenance,
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
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct AgentAutonomyRunPayload {
    pub apartment_name: String,
    pub artifact_hash: String,
    #[norito(default)]
    pub provenance_hash: Option<String>,
    pub budget_units: u64,
    pub run_label: String,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedAgentAutonomyRunRequest {
    pub payload: AgentAutonomyRunPayload,
    pub provenance: ManifestProvenance,
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
    pub sequence: u64,
    pub model_artifact_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusResponse {
    pub schema_version: u16,
    pub artifact: ModelArtifactStatusEntry,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusEntry {
    pub service_name: String,
    pub model_name: String,
    pub training_job_id: String,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
    pub registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consumed_by_version: Option<String>,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct RegistryStatusQuery {
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub audit_limit: Option<u32>,
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

#[derive(Clone, Debug, JsonDeserialize)]
pub(crate) struct ModelArtifactStatusQuery {
    pub service_name: String,
    pub training_job_id: String,
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
pub(crate) struct RegistrySnapshot {
    pub schema_version: u16,
    pub service_count: u32,
    pub audit_event_count: u32,
    #[norito(default)]
    pub services: Vec<ServiceStatusSnapshot>,
    #[norito(default)]
    pub recent_audit_events: Vec<RegistryAuditEvent>,
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
pub(crate) struct ServiceStatusSnapshot {
    pub service_name: String,
    pub current_version: String,
    pub revision_count: u32,
    #[norito(default)]
    pub latest_revision: Option<RegistryServiceRevision>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct RegistryState {
    schema_version: u16,
    next_sequence: u64,
    #[norito(default)]
    services: BTreeMap<String, RegistryServiceEntry>,
    #[norito(default)]
    audit_log: Vec<RegistryAuditEvent>,
    #[norito(default)]
    apartments: BTreeMap<String, AgentApartmentRuntimeState>,
    #[norito(default)]
    apartment_audit_log: Vec<AgentApartmentAuditEvent>,
    #[norito(default)]
    training_audit_log: Vec<TrainingJobAuditEvent>,
    #[norito(default)]
    model_weight_audit_log: Vec<ModelWeightAuditEvent>,
    #[norito(default)]
    model_artifact_audit_log: Vec<ModelArtifactAuditEvent>,
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            next_sequence: 1,
            services: BTreeMap::new(),
            audit_log: Vec::new(),
            apartments: BTreeMap::new(),
            apartment_audit_log: Vec::new(),
            training_audit_log: Vec::new(),
            model_weight_audit_log: Vec::new(),
            model_artifact_audit_log: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
struct RegistryServiceEntry {
    current_version: String,
    #[norito(default)]
    revisions: Vec<RegistryServiceRevision>,
    #[norito(default)]
    binding_states: BTreeMap<String, BindingRuntimeState>,
    #[norito(default)]
    training_jobs: BTreeMap<String, TrainingJobRuntimeState>,
    #[norito(default)]
    model_weights: BTreeMap<String, ModelWeightRegistryState>,
    #[norito(default)]
    model_artifacts: BTreeMap<String, ModelArtifactState>,
    #[norito(default)]
    active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug)]
struct ScrHostAdmission {
    runtime: SoraContainerRuntimeV1,
    allow_wallet_signing: bool,
    allow_state_writes: bool,
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
pub(crate) struct RegistryServiceRevision {
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
    /// Deterministic hash of sandbox/capability/resource admission inputs.
    pub sandbox_profile_hash: Hash,
    /// Monotonic simulated SCR process generation.
    pub process_generation: u64,
    /// Sequence that started the current process generation.
    pub process_started_sequence: u64,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RegistryAuditEvent {
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
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    registered_sequence: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    consumed_by_version: Option<String>,
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
    pub approved_sequence: u64,
}

#[derive(Clone, Debug, NoritoDeserialize, NoritoSerialize)]
struct RegistryPersistenceSnapshot {
    version: u8,
    state: RegistryState,
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegistryPersistenceSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<RegistryPersistenceSnapshot>(bytes)
    }
}

#[derive(Debug, Clone)]
struct RegistryPersistence {
    path: PathBuf,
    temp_path: PathBuf,
}

impl RegistryPersistence {
    fn new(path: PathBuf) -> Self {
        let temp_path = path.with_added_extension("tmp");
        Self { path, temp_path }
    }

    fn load(&self) -> Result<RegistryState, SoracloudError> {
        let candidate = if self.path.exists() {
            Some(self.path.clone())
        } else if self.temp_path.exists() {
            Some(self.temp_path.clone())
        } else {
            None
        };

        let Some(path) = candidate else {
            return Ok(RegistryState::default());
        };
        let bytes = fs::read(&path).map_err(|err| {
            SoracloudError::internal(format!(
                "failed to read Soracloud registry snapshot `{}`: {err}",
                path.display()
            ))
        })?;
        if bytes.is_empty() {
            return Ok(RegistryState::default());
        }

        let snapshot: RegistryPersistenceSnapshot =
            norito::decode_from_bytes(&bytes).map_err(|err| {
                SoracloudError::internal(format!(
                    "failed to decode Soracloud registry snapshot `{}`: {err}",
                    path.display()
                ))
            })?;
        if snapshot.version != REGISTRY_PERSISTENCE_VERSION_V1 {
            return Err(SoracloudError::internal(format!(
                "unsupported Soracloud registry snapshot version {} (expected {REGISTRY_PERSISTENCE_VERSION_V1})",
                snapshot.version
            )));
        }
        let mut state = snapshot.state;
        ensure_registry_schema(&state)?;
        normalize_registry_runtime_defaults(&mut state);
        Ok(state)
    }

    fn store(&self, state: &RegistryState) -> Result<(), SoracloudError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                SoracloudError::internal(format!(
                    "failed to prepare Soracloud registry persistence directory `{}`: {err}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = RegistryPersistenceSnapshot {
            version: REGISTRY_PERSISTENCE_VERSION_V1,
            state: state.clone(),
        };
        let bytes = norito::to_bytes(&snapshot).map_err(|err| {
            SoracloudError::internal(format!(
                "failed to encode Soracloud registry persistence snapshot: {err}"
            ))
        })?;
        fs::write(&self.temp_path, &bytes).map_err(|err| {
            SoracloudError::internal(format!(
                "failed to write Soracloud registry temp snapshot `{}`: {err}",
                self.temp_path.display()
            ))
        })?;
        fs::rename(&self.temp_path, &self.path).map_err(|err| {
            SoracloudError::internal(format!(
                "failed to persist Soracloud registry snapshot `{}`: {err}",
                self.path.display()
            ))
        })?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Registry {
    state: RwLock<RegistryState>,
    persistence: Option<RegistryPersistence>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl Registry {
    pub(crate) fn in_memory() -> Self {
        Self {
            state: RwLock::new(RegistryState::default()),
            persistence: None,
        }
    }

    pub(crate) fn with_default_persistence() -> Self {
        Self::with_persistence(Self::default_persistence_path())
    }

    pub(crate) fn with_persistence(path: PathBuf) -> Self {
        let persistence = RegistryPersistence::new(path.clone());
        let state = match persistence.load() {
            Ok(state) => state,
            Err(err) => {
                iroha_logger::warn!(
                    path = %path.display(),
                    ?err,
                    "failed to restore Soracloud registry snapshot; using empty in-memory registry"
                );
                RegistryState::default()
            }
        };

        Self {
            state: RwLock::new(state),
            persistence: Some(persistence),
        }
    }

    fn default_persistence_path() -> PathBuf {
        crate::data_dir::base_dir()
            .join("soracloud")
            .join("registry_state.to")
    }

    fn persist_state_or_rollback(
        &self,
        state: &mut RegistryState,
        previous_state: RegistryState,
    ) -> Result<(), SoracloudError> {
        let Some(persistence) = &self.persistence else {
            return Ok(());
        };
        if let Err(err) = persistence.store(state) {
            *state = previous_state;
            return Err(err);
        }
        Ok(())
    }

    pub(crate) async fn snapshot(
        &self,
        service_name: Option<&str>,
        audit_limit: usize,
    ) -> RegistrySnapshot {
        let state = self.state.read().await;
        let mut services = Vec::new();
        for (name, entry) in &state.services {
            if service_name.is_some_and(|filter| filter != name) {
                continue;
            }
            services.push(ServiceStatusSnapshot {
                service_name: name.clone(),
                current_version: entry.current_version.clone(),
                revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
                latest_revision: entry.revisions.last().cloned(),
                active_rollout: entry.active_rollout.clone(),
                last_rollout: entry.last_rollout.clone(),
            });
        }

        let limit = audit_limit.max(1).min(MAX_AUDIT_LIMIT);
        let recent_audit_events = state
            .audit_log
            .iter()
            .rev()
            .filter(|event| service_name.is_none_or(|filter| filter == event.service_name.as_str()))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();

        RegistrySnapshot {
            schema_version: state.schema_version,
            service_count: u32::try_from(services.len()).unwrap_or(u32::MAX),
            audit_event_count: u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX),
            services,
            recent_audit_events,
        }
    }

    pub(crate) async fn health_compliance_report(
        &self,
        service_name: Option<&str>,
        jurisdiction_tag: Option<&str>,
        limit: usize,
    ) -> Result<HealthComplianceReportResponse, SoracloudError> {
        let state = self.state.read().await;
        ensure_registry_schema(&state)?;

        let limit = limit.max(1).min(MAX_HEALTH_COMPLIANCE_LIMIT);
        let generated_at_sequence = state.next_sequence.saturating_sub(1);

        let access_events = state
            .audit_log
            .iter()
            .filter(|event| event.action == SoracloudAction::DecryptionRequest)
            .filter(|event| service_name.is_none_or(|filter| filter == event.service_name.as_str()))
            .filter(|event| {
                jurisdiction_tag
                    .is_none_or(|filter| event.jurisdiction_tag.as_deref() == Some(filter))
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

        let mut data_flow_services = BTreeSet::new();
        if let Some(service_name) = service_name {
            data_flow_services.insert(service_name.to_string());
        } else {
            for event in &access_events {
                data_flow_services.insert(event.service_name.clone());
            }
        }
        let mut data_flow_attestations = Vec::new();
        for service_name in data_flow_services {
            let Some(entry) = state.services.get(&service_name) else {
                continue;
            };
            let Some(revision) = entry.revisions.last() else {
                continue;
            };
            for binding in &revision.state_bindings {
                if binding.encryption == SoraStateEncryptionV1::Plaintext {
                    continue;
                }
                data_flow_attestations.push(HealthDataFlowAttestation {
                    service_name: service_name.clone(),
                    current_version: entry.current_version.clone(),
                    binding_name: binding.binding_name.to_string(),
                    key_prefix: binding.key_prefix.clone(),
                    encryption: binding.encryption,
                    mutability: binding.mutability,
                });
            }
        }

        Ok(HealthComplianceReportResponse {
            schema_version: HEALTH_COMPLIANCE_REPORT_VERSION_V1,
            service_name: service_name.map(ToOwned::to_owned),
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

    pub(crate) async fn apply_deploy(
        &self,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        self.apply_bundle_mutation(MutationMode::Deploy, request)
            .await
    }

    pub(crate) async fn apply_upgrade(
        &self,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        self.apply_bundle_mutation(MutationMode::Upgrade, request)
            .await
    }

    pub(crate) async fn apply_rollback(
        &self,
        request: SignedRollbackRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        verify_rollback_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();

        let sequence = state.next_sequence;
        let signer = request.provenance.signer.to_string();
        let target_version = request.payload.target_version.clone();
        let (previous_version, target, revision_count) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let previous_version = Some(entry.current_version.clone());
            let target = if let Some(target_version) = target_version.as_deref() {
                entry
                    .revisions
                    .iter()
                    .rev()
                    .find(|revision| revision.service_version == target_version)
                    .cloned()
                    .ok_or_else(|| {
                        SoracloudError::not_found(format!(
                            "service `{service_name}` has no deployed revision for version `{target_version}`"
                        ))
                    })?
            } else {
                entry
                    .revisions
                    .iter()
                    .rev()
                    .find(|revision| revision.service_version != entry.current_version)
                    .cloned()
                    .ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no previous revision to roll back to"
                        ))
                    })?
            };

            let process_generation = entry
                .revisions
                .last()
                .map_or(1, |revision| revision.process_generation.saturating_add(1));
            let revision = RegistryServiceRevision {
                sequence,
                action: SoracloudAction::Rollback,
                service_version: target.service_version.clone(),
                service_manifest_hash: target.service_manifest_hash,
                container_manifest_hash: target.container_manifest_hash,
                replicas: target.replicas,
                route_host: target.route_host.clone(),
                route_path_prefix: target.route_path_prefix.clone(),
                state_binding_count: target.state_binding_count,
                state_bindings: target.state_bindings.clone(),
                allow_model_training: target.allow_model_training,
                runtime: target.runtime,
                allow_wallet_signing: target.allow_wallet_signing,
                allow_state_writes: target.allow_state_writes,
                network: target.network.clone(),
                cpu_millis: target.cpu_millis,
                memory_bytes: target.memory_bytes,
                ephemeral_storage_bytes: target.ephemeral_storage_bytes,
                max_open_files: target.max_open_files,
                max_tasks: target.max_tasks,
                start_grace_secs: target.start_grace_secs,
                stop_grace_secs: target.stop_grace_secs,
                healthcheck_path: target.healthcheck_path.clone(),
                sandbox_profile_hash: target.sandbox_profile_hash,
                process_generation,
                process_started_sequence: sequence,
                signed_by: signer.clone(),
            };

            entry.current_version = target.service_version.clone();
            sync_binding_states(entry, &target.state_bindings);
            entry.revisions.push(revision);
            entry.active_rollout = None;
            entry.last_rollout = None;
            let revision_count = u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX);
            (previous_version, target, revision_count)
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::Rollback,
            service_name: service_name.clone(),
            from_version: previous_version.clone(),
            to_version: target.service_version.clone(),
            service_manifest_hash: target.service_manifest_hash,
            container_manifest_hash: target.container_manifest_hash,
            binding_name: None,
            state_key: None,
            governance_tx_hash: None,
            rollout_handle: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(MutationResponse {
            action: SoracloudAction::Rollback,
            service_name,
            previous_version,
            current_version: target.service_version,
            sequence,
            service_manifest_hash: target.service_manifest_hash,
            container_manifest_hash: target.container_manifest_hash,
            revision_count,
            audit_event_count,
            signed_by: signer,
            rollout_handle: None,
            rollout_stage: None,
            rollout_percent: None,
        })
    }

    pub(crate) async fn apply_state_mutation(
        &self,
        request: SignedStateMutationRequest,
    ) -> Result<StateMutationResponse, SoracloudError> {
        verify_state_mutation_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let binding_name: Name =
            request.payload.binding_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid binding_name: {err}"))
            })?;
        if request.payload.key.trim().is_empty() {
            return Err(SoracloudError::bad_request(
                "state mutation key must not be empty",
            ));
        }

        let service_name = service_name.to_string();
        let binding_name = binding_name.to_string();
        let signer = request.provenance.signer.to_string();
        let operation = request.payload.operation;
        let key = request.payload.key.clone();
        let governance_tx_hash = request.payload.governance_tx_hash;

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();

        let sequence = state.next_sequence;
        let (
            current_version,
            service_manifest_hash,
            container_manifest_hash,
            binding_total_bytes,
            binding_key_count,
        ) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;

            let binding = current_revision
                .state_bindings
                .iter()
                .find(|binding| binding.binding_name.as_ref() == binding_name.as_str())
                .cloned()
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "binding `{binding_name}` is not declared for service `{service_name}`"
                    ))
                })?;

            if request.payload.encryption != binding.encryption {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` requires {:?} encryption",
                    binding.encryption
                )));
            }
            if !key.starts_with(&binding.key_prefix) {
                return Err(SoracloudError::conflict(format!(
                    "key `{key}` is outside binding prefix `{}`",
                    binding.key_prefix
                )));
            }

            let runtime_state = entry
                .binding_states
                .entry(binding_name.clone())
                .or_insert_with(BindingRuntimeState::default);
            match operation {
                StateMutationOperation::Upsert => {
                    if binding.mutability == SoraStateMutabilityV1::ReadOnly {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` is read-only"
                        )));
                    }
                    let value_size = request.payload.value_size_bytes.ok_or_else(|| {
                        SoracloudError::bad_request(
                            "value_size_bytes is required for upsert mutations",
                        )
                    })?;
                    if value_size > binding.max_item_bytes.get() {
                        return Err(SoracloudError::conflict(format!(
                            "value_size_bytes {value_size} exceeds binding max_item_bytes {}",
                            binding.max_item_bytes
                        )));
                    }

                    let existing_size = runtime_state.key_sizes.get(&key).copied().unwrap_or(0);
                    if binding.mutability == SoraStateMutabilityV1::AppendOnly && existing_size > 0
                    {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` is append-only; key `{key}` already exists"
                        )));
                    }
                    let tentative_total = runtime_state
                        .total_bytes
                        .saturating_sub(existing_size)
                        .saturating_add(value_size);
                    if tentative_total > binding.max_total_bytes.get() {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` max_total_bytes {} would be exceeded",
                            binding.max_total_bytes
                        )));
                    }

                    runtime_state.total_bytes = tentative_total;
                    runtime_state.key_sizes.insert(key.clone(), value_size);
                    if binding.encryption != SoraStateEncryptionV1::Plaintext {
                        let commitment = derive_ciphertext_commitment_for_state_mutation(
                            &service_name,
                            &binding_name,
                            &key,
                            value_size,
                            binding.encryption,
                            governance_tx_hash,
                        );
                        runtime_state.ciphertext_records.insert(
                            key.clone(),
                            CiphertextRuntimeRecord {
                                encryption: binding.encryption,
                                payload_bytes: value_size,
                                commitment,
                                last_update_sequence: sequence,
                                governance_tx_hash,
                                source_action: SoracloudAction::StateMutation,
                            },
                        );
                    }
                }
                StateMutationOperation::Delete => {
                    if binding.mutability != SoraStateMutabilityV1::ReadWrite {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` does not allow deletes"
                        )));
                    }
                    if let Some(existing_size) = runtime_state.key_sizes.remove(&key) {
                        runtime_state.total_bytes =
                            runtime_state.total_bytes.saturating_sub(existing_size);
                    }
                    runtime_state.ciphertext_records.remove(&key);
                }
            }

            (
                entry.current_version.clone(),
                current_revision.service_manifest_hash,
                current_revision.container_manifest_hash,
                runtime_state.total_bytes,
                u32::try_from(runtime_state.key_sizes.len()).unwrap_or(u32::MAX),
            )
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::StateMutation,
            service_name: service_name.clone(),
            from_version: None,
            to_version: current_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: Some(binding_name.clone()),
            state_key: Some(key.clone()),
            governance_tx_hash: Some(governance_tx_hash),
            rollout_handle: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(StateMutationResponse {
            action: SoracloudAction::StateMutation,
            service_name,
            binding_name,
            key,
            operation,
            sequence,
            governance_tx_hash,
            current_version,
            binding_total_bytes,
            binding_key_count,
            audit_event_count,
            signed_by: signer,
        })
    }

    pub(crate) async fn apply_fhe_job_run(
        &self,
        request: SignedFheJobRunRequest,
    ) -> Result<FheJobRunResponse, SoracloudError> {
        verify_fhe_job_run_signature(&request)?;
        request.payload.param_set.validate().map_err(|err| {
            SoracloudError::bad_request(format!("fhe parameter set failed validation: {err}"))
        })?;
        request
            .payload
            .policy
            .validate_for_param_set(&request.payload.param_set)
            .map_err(|err| {
                SoracloudError::bad_request(format!(
                    "fhe execution policy failed validation: {err}"
                ))
            })?;
        request
            .payload
            .job
            .validate_for_execution(&request.payload.policy, &request.payload.param_set)
            .map_err(|err| {
                SoracloudError::bad_request(format!("fhe job failed validation: {err}"))
            })?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let binding_name: Name =
            request.payload.binding_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid binding_name: {err}"))
            })?;

        let service_name = service_name.to_string();
        let binding_name = binding_name.to_string();
        let signer = request.provenance.signer.to_string();
        let governance_tx_hash = request.payload.governance_tx_hash;
        let output_state_key = request.payload.job.output_state_key.clone();
        let output_payload_bytes = request.payload.job.deterministic_output_payload_bytes();
        let output_commitment = request.payload.job.deterministic_output_commitment();
        let operation = request.payload.job.operation;
        let job_id = request.payload.job.job_id.clone();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (
            current_version,
            service_manifest_hash,
            container_manifest_hash,
            binding_total_bytes,
            binding_key_count,
        ) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            let binding = current_revision
                .state_bindings
                .iter()
                .find(|binding| binding.binding_name.as_ref() == binding_name.as_str())
                .cloned()
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "binding `{binding_name}` is not declared for service `{service_name}`"
                    ))
                })?;
            if binding.encryption != SoraStateEncryptionV1::FheCiphertext {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` is not configured for FHE ciphertexts"
                )));
            }
            if binding.mutability == SoraStateMutabilityV1::ReadOnly {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` is read-only"
                )));
            }
            if !output_state_key.starts_with(&binding.key_prefix) {
                return Err(SoracloudError::conflict(format!(
                    "fhe output key `{output_state_key}` is outside binding prefix `{}`",
                    binding.key_prefix
                )));
            }
            if output_payload_bytes > binding.max_item_bytes.get() {
                return Err(SoracloudError::conflict(format!(
                    "fhe output size {output_payload_bytes} exceeds binding max_item_bytes {}",
                    binding.max_item_bytes
                )));
            }

            let runtime_state = entry
                .binding_states
                .entry(binding_name.clone())
                .or_insert_with(BindingRuntimeState::default);
            let existing_size = runtime_state
                .key_sizes
                .get(&output_state_key)
                .copied()
                .unwrap_or(0);
            if binding.mutability == SoraStateMutabilityV1::AppendOnly && existing_size > 0 {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` is append-only; key `{output_state_key}` already exists"
                )));
            }
            let tentative_total = runtime_state
                .total_bytes
                .saturating_sub(existing_size)
                .saturating_add(output_payload_bytes);
            if tentative_total > binding.max_total_bytes.get() {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` max_total_bytes {} would be exceeded",
                    binding.max_total_bytes
                )));
            }
            runtime_state.total_bytes = tentative_total;
            runtime_state
                .key_sizes
                .insert(output_state_key.clone(), output_payload_bytes);
            runtime_state.ciphertext_records.insert(
                output_state_key.clone(),
                CiphertextRuntimeRecord {
                    encryption: SoraStateEncryptionV1::FheCiphertext,
                    payload_bytes: output_payload_bytes,
                    commitment: output_commitment,
                    last_update_sequence: sequence,
                    governance_tx_hash,
                    source_action: SoracloudAction::FheJobRun,
                },
            );

            (
                entry.current_version.clone(),
                current_revision.service_manifest_hash,
                current_revision.container_manifest_hash,
                runtime_state.total_bytes,
                u32::try_from(runtime_state.key_sizes.len()).unwrap_or(u32::MAX),
            )
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::FheJobRun,
            service_name: service_name.clone(),
            from_version: None,
            to_version: current_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: Some(binding_name.clone()),
            state_key: Some(output_state_key.clone()),
            governance_tx_hash: Some(governance_tx_hash),
            rollout_handle: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(FheJobRunResponse {
            action: SoracloudAction::FheJobRun,
            service_name,
            binding_name,
            job_id,
            operation,
            sequence,
            governance_tx_hash,
            output_state_key,
            output_payload_bytes,
            output_commitment,
            current_version,
            binding_total_bytes,
            binding_key_count,
            audit_event_count,
            signed_by: signer,
        })
    }

    pub(crate) async fn apply_decryption_request(
        &self,
        request: SignedDecryptionRequest,
    ) -> Result<DecryptionRequestResponse, SoracloudError> {
        verify_decryption_request_signature(&request)?;
        request.payload.policy.validate().map_err(|err| {
            SoracloudError::bad_request(format!(
                "decryption authority policy failed validation: {err}"
            ))
        })?;
        request
            .payload
            .request
            .validate_for_policy(&request.payload.policy)
            .map_err(|err| {
                SoracloudError::bad_request(format!("decryption request failed validation: {err}"))
            })?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let signer = request.provenance.signer.to_string();
        let governance_tx_hash = request.payload.request.governance_tx_hash;
        let state_key = request.payload.request.state_key.clone();
        let binding_name = request.payload.request.binding_name.clone();
        let request_id = request.payload.request.request_id.clone();
        let policy_name = request.payload.request.policy_name.clone();
        let jurisdiction_tag = request.payload.request.jurisdiction_tag.clone();
        let policy_snapshot_hash = Hash::new(Encode::encode(&request.payload.policy));
        let consent_evidence_hash = request.payload.request.consent_evidence_hash;
        let break_glass = request.payload.request.break_glass;
        let break_glass_reason = request.payload.request.break_glass_reason.clone();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (current_version, service_manifest_hash, container_manifest_hash) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            let binding = current_revision
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
                    "binding `{binding_name}` is plaintext; decryption authority policy is not applicable"
                )));
            }
            if !state_key.starts_with(&binding.key_prefix) {
                return Err(SoracloudError::conflict(format!(
                    "decryption request key `{state_key}` is outside binding prefix `{}`",
                    binding.key_prefix
                )));
            }

            (
                entry.current_version.clone(),
                current_revision.service_manifest_hash,
                current_revision.container_manifest_hash,
            )
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::DecryptionRequest,
            service_name: service_name.clone(),
            from_version: None,
            to_version: current_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: Some(binding_name.to_string()),
            state_key: Some(state_key.clone()),
            governance_tx_hash: Some(governance_tx_hash),
            rollout_handle: None,
            policy_name: Some(policy_name.to_string()),
            policy_snapshot_hash: Some(policy_snapshot_hash),
            jurisdiction_tag: Some(jurisdiction_tag.clone()),
            consent_evidence_hash,
            break_glass: Some(break_glass),
            break_glass_reason: break_glass_reason.clone(),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(DecryptionRequestResponse {
            action: SoracloudAction::DecryptionRequest,
            service_name,
            policy_name,
            request_id,
            binding_name,
            state_key,
            jurisdiction_tag,
            policy_snapshot_hash,
            consent_evidence_hash,
            break_glass,
            break_glass_reason,
            sequence,
            governance_tx_hash,
            current_version,
            audit_event_count,
            signed_by: signer,
        })
    }

    pub(crate) async fn apply_ciphertext_query(
        &self,
        request: SignedCiphertextQueryRequest,
    ) -> Result<CiphertextQueryResponse, SoracloudError> {
        verify_ciphertext_query_signature(&request)?;
        request.query.validate().map_err(|err| {
            SoracloudError::bad_request(format!("ciphertext query failed validation: {err}"))
        })?;

        let service_name = request.query.service_name.to_string();
        let binding_name = request.query.binding_name.to_string();
        let signer = request.provenance.signer.to_string();
        let query_hash = Hash::new(Encode::encode(&request.query));
        let limit = usize::from(request.query.max_results.get());

        let state = self.state.read().await;
        ensure_registry_schema(&state)?;

        let entry = state.services.get(&service_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in control-plane registry"
            ))
        })?;
        let current_revision = entry.revisions.last().ok_or_else(|| {
            SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
        })?;
        let binding = current_revision
            .state_bindings
            .iter()
            .find(|binding| binding.binding_name.as_ref() == binding_name.as_str())
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

        let mut rows = Vec::new();
        let runtime_state = entry
            .binding_states
            .get(&binding_name)
            .cloned()
            .unwrap_or_default();
        let served_sequence = state.next_sequence.saturating_sub(1);
        let anchor_hash = audit_anchor_hash(&state.audit_log, served_sequence);
        let mut truncated = false;

        for (state_key, record) in runtime_state.ciphertext_records {
            if !state_key.starts_with(&request.query.state_key_prefix) {
                continue;
            }
            if rows.len() >= limit {
                truncated = true;
                break;
            }

            let Some(payload_bytes) = NonZeroU64::new(record.payload_bytes) else {
                continue;
            };
            let state_key_digest =
                derive_state_key_digest(&service_name, &binding_name, &state_key);
            let proof = if request.query.include_proof {
                Some(build_ciphertext_inclusion_proof(
                    &state.audit_log,
                    &service_name,
                    &binding_name,
                    &state_key,
                    &record,
                    served_sequence,
                    anchor_hash,
                ))
            } else {
                None
            };

            let state_key = match request.query.metadata_level {
                CiphertextQueryMetadataLevelV1::Minimal => None,
                CiphertextQueryMetadataLevelV1::Standard => Some(state_key.clone()),
            };
            rows.push(CiphertextQueryResultItemV1 {
                binding_name: request.query.binding_name.clone(),
                state_key,
                state_key_digest,
                payload_bytes,
                ciphertext_commitment: record.commitment,
                encryption: record.encryption,
                last_update_sequence: record.last_update_sequence,
                governance_tx_hash: record.governance_tx_hash,
                proof,
            });
        }

        let response = CiphertextQueryResponseV1 {
            schema_version: CIPHERTEXT_QUERY_RESPONSE_VERSION_V1,
            query_hash,
            service_name: request.query.service_name.clone(),
            binding_name: request.query.binding_name.clone(),
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

    pub(crate) async fn apply_rollout(
        &self,
        request: SignedRolloutAdvanceRequest,
    ) -> Result<RolloutResponse, SoracloudError> {
        verify_rollout_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        if request.payload.rollout_handle.trim().is_empty() {
            return Err(SoracloudError::bad_request(
                "rollout_handle must not be empty",
            ));
        }
        if request
            .payload
            .promote_to_percent
            .is_some_and(|value| value > 100)
        {
            return Err(SoracloudError::bad_request(
                "promote_to_percent must be within 0..=100",
            ));
        }

        let service_name = service_name.to_string();
        let signer = request.provenance.signer.to_string();
        let rollout_handle = request.payload.rollout_handle.clone();
        let governance_tx_hash = request.payload.governance_tx_hash;
        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (mut response, audit_event) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;

            let mut rollout = entry.active_rollout.clone().ok_or_else(|| {
                SoracloudError::conflict(format!(
                    "service `{service_name}` has no active rollout to advance"
                ))
            })?;
            if rollout.rollout_handle != rollout_handle {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active rollout handle mismatch (expected `{}`)",
                    rollout.rollout_handle
                )));
            }

            if request.payload.healthy {
                let promote_to = request.payload.promote_to_percent.unwrap_or(100);
                if promote_to < rollout.traffic_percent {
                    return Err(SoracloudError::conflict(format!(
                        "rollout traffic cannot decrease from {} to {promote_to}",
                        rollout.traffic_percent
                    )));
                }
                if promote_to < rollout.canary_percent {
                    return Err(SoracloudError::conflict(format!(
                        "rollout traffic cannot be below canary_percent {}",
                        rollout.canary_percent
                    )));
                }
                rollout.traffic_percent = promote_to;
                rollout.stage = if promote_to >= 100 {
                    RolloutStage::Promoted
                } else {
                    RolloutStage::Canary
                };
                rollout.health_failures = 0;
                rollout.updated_sequence = sequence;

                if rollout.stage == RolloutStage::Promoted {
                    entry.active_rollout = None;
                } else {
                    entry.active_rollout = Some(rollout.clone());
                }
                entry.last_rollout = Some(rollout.clone());

                let current_version = entry.current_version.clone();
                let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                    SoracloudError::conflict(format!(
                        "service `{service_name}` has no active revision"
                    ))
                })?;
                let audit_event = RegistryAuditEvent {
                    sequence,
                    action: SoracloudAction::Rollout,
                    service_name: service_name.clone(),
                    from_version: Some(current_version.clone()),
                    to_version: current_version.clone(),
                    service_manifest_hash: current_revision.service_manifest_hash,
                    container_manifest_hash: current_revision.container_manifest_hash,
                    binding_name: None,
                    state_key: None,
                    governance_tx_hash: Some(governance_tx_hash),
                    rollout_handle: Some(rollout_handle.clone()),
                    policy_name: None,
                    policy_snapshot_hash: None,
                    jurisdiction_tag: None,
                    consent_evidence_hash: None,
                    break_glass: None,
                    break_glass_reason: None,
                    signed_by: signer.clone(),
                };
                let response = RolloutResponse {
                    action: SoracloudAction::Rollout,
                    service_name: service_name.clone(),
                    rollout_handle: rollout_handle.clone(),
                    stage: rollout.stage,
                    current_version,
                    traffic_percent: rollout.traffic_percent,
                    health_failures: rollout.health_failures,
                    max_health_failures: rollout.max_health_failures,
                    sequence,
                    governance_tx_hash,
                    audit_event_count: 0,
                    signed_by: signer.clone(),
                };
                (response, audit_event)
            } else {
                rollout.health_failures = rollout.health_failures.saturating_add(1);
                rollout.updated_sequence = sequence;

                if rollout.health_failures >= rollout.max_health_failures {
                    let baseline_version = rollout.baseline_version.clone().ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no baseline version for auto rollback"
                        ))
                    })?;
                    let previous_version = entry.current_version.clone();
                    let target = entry
                        .revisions
                        .iter()
                        .rev()
                        .find(|revision| revision.service_version == baseline_version)
                        .cloned()
                        .ok_or_else(|| {
                            SoracloudError::not_found(format!(
                                "service `{service_name}` missing baseline revision `{baseline_version}`"
                            ))
                        })?;
                    let process_generation = entry
                        .revisions
                        .last()
                        .map_or(1, |revision| revision.process_generation.saturating_add(1));

                    let rollback_revision = RegistryServiceRevision {
                        sequence,
                        action: SoracloudAction::Rollback,
                        service_version: target.service_version.clone(),
                        service_manifest_hash: target.service_manifest_hash,
                        container_manifest_hash: target.container_manifest_hash,
                        replicas: target.replicas,
                        route_host: target.route_host.clone(),
                        route_path_prefix: target.route_path_prefix.clone(),
                        state_binding_count: target.state_binding_count,
                        state_bindings: target.state_bindings.clone(),
                        allow_model_training: target.allow_model_training,
                        runtime: target.runtime,
                        allow_wallet_signing: target.allow_wallet_signing,
                        allow_state_writes: target.allow_state_writes,
                        network: target.network.clone(),
                        cpu_millis: target.cpu_millis,
                        memory_bytes: target.memory_bytes,
                        ephemeral_storage_bytes: target.ephemeral_storage_bytes,
                        max_open_files: target.max_open_files,
                        max_tasks: target.max_tasks,
                        start_grace_secs: target.start_grace_secs,
                        stop_grace_secs: target.stop_grace_secs,
                        healthcheck_path: target.healthcheck_path.clone(),
                        sandbox_profile_hash: target.sandbox_profile_hash,
                        process_generation,
                        process_started_sequence: sequence,
                        signed_by: signer.clone(),
                    };
                    entry.current_version = target.service_version.clone();
                    sync_binding_states(entry, &target.state_bindings);
                    entry.revisions.push(rollback_revision);

                    rollout.stage = RolloutStage::RolledBack;
                    rollout.traffic_percent = 0;
                    entry.active_rollout = None;
                    entry.last_rollout = Some(rollout.clone());

                    let audit_event = RegistryAuditEvent {
                        sequence,
                        action: SoracloudAction::Rollback,
                        service_name: service_name.clone(),
                        from_version: Some(previous_version),
                        to_version: target.service_version.clone(),
                        service_manifest_hash: target.service_manifest_hash,
                        container_manifest_hash: target.container_manifest_hash,
                        binding_name: None,
                        state_key: None,
                        governance_tx_hash: Some(governance_tx_hash),
                        rollout_handle: Some(rollout_handle.clone()),
                        policy_name: None,
                        policy_snapshot_hash: None,
                        jurisdiction_tag: None,
                        consent_evidence_hash: None,
                        break_glass: None,
                        break_glass_reason: None,
                        signed_by: signer.clone(),
                    };
                    let response = RolloutResponse {
                        action: SoracloudAction::Rollback,
                        service_name: service_name.clone(),
                        rollout_handle: rollout_handle.clone(),
                        stage: rollout.stage,
                        current_version: target.service_version,
                        traffic_percent: rollout.traffic_percent,
                        health_failures: rollout.health_failures,
                        max_health_failures: rollout.max_health_failures,
                        sequence,
                        governance_tx_hash,
                        audit_event_count: 0,
                        signed_by: signer.clone(),
                    };
                    (response, audit_event)
                } else {
                    entry.active_rollout = Some(rollout.clone());
                    entry.last_rollout = Some(rollout.clone());
                    let current_version = entry.current_version.clone();
                    let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no active revision"
                        ))
                    })?;
                    let audit_event = RegistryAuditEvent {
                        sequence,
                        action: SoracloudAction::Rollout,
                        service_name: service_name.clone(),
                        from_version: Some(current_version.clone()),
                        to_version: current_version.clone(),
                        service_manifest_hash: current_revision.service_manifest_hash,
                        container_manifest_hash: current_revision.container_manifest_hash,
                        binding_name: None,
                        state_key: None,
                        governance_tx_hash: Some(governance_tx_hash),
                        rollout_handle: Some(rollout_handle.clone()),
                        policy_name: None,
                        policy_snapshot_hash: None,
                        jurisdiction_tag: None,
                        consent_evidence_hash: None,
                        break_glass: None,
                        break_glass_reason: None,
                        signed_by: signer.clone(),
                    };
                    let response = RolloutResponse {
                        action: SoracloudAction::Rollout,
                        service_name: service_name.clone(),
                        rollout_handle: rollout_handle.clone(),
                        stage: rollout.stage,
                        current_version,
                        traffic_percent: rollout.traffic_percent,
                        health_failures: rollout.health_failures,
                        max_health_failures: rollout.max_health_failures,
                        sequence,
                        governance_tx_hash,
                        audit_event_count: 0,
                        signed_by: signer.clone(),
                    };
                    (response, audit_event)
                }
            }
        };

        state.audit_log.push(audit_event);
        state.next_sequence = state.next_sequence.saturating_add(1);
        response.audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;
        Ok(response)
    }

    pub(crate) async fn apply_training_job_start(
        &self,
        request: SignedTrainingJobStartRequest,
    ) -> Result<TrainingJobMutationResponse, SoracloudError> {
        verify_training_job_start_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let model_name = parse_training_model_name(&request.payload.model_name)?;
        let job_id = parse_training_job_id(&request.payload.job_id)?;
        if request.payload.worker_group_size == 0
            || request.payload.worker_group_size > TRAINING_MAX_WORKER_GROUP_SIZE
        {
            return Err(SoracloudError::bad_request(format!(
                "worker_group_size must be within 1..={TRAINING_MAX_WORKER_GROUP_SIZE}"
            )));
        }
        if request.payload.target_steps == 0 {
            return Err(SoracloudError::bad_request(
                "target_steps must be greater than zero",
            ));
        }
        if request.payload.checkpoint_interval_steps == 0 {
            return Err(SoracloudError::bad_request(
                "checkpoint_interval_steps must be greater than zero",
            ));
        }
        if request.payload.checkpoint_interval_steps > request.payload.target_steps {
            return Err(SoracloudError::bad_request(
                "checkpoint_interval_steps must not exceed target_steps",
            ));
        }
        if request.payload.max_retries > TRAINING_MAX_RETRIES {
            return Err(SoracloudError::bad_request(format!(
                "max_retries must be within 0..={TRAINING_MAX_RETRIES}"
            )));
        }
        if request.payload.step_compute_units == 0 {
            return Err(SoracloudError::bad_request(
                "step_compute_units must be greater than zero",
            ));
        }
        if request.payload.compute_budget_units == 0 {
            return Err(SoracloudError::bad_request(
                "compute_budget_units must be greater than zero",
            ));
        }
        if request.payload.storage_budget_bytes == 0 {
            return Err(SoracloudError::bad_request(
                "storage_budget_bytes must be greater than zero",
            ));
        }
        let minimum_step_units = request
            .payload
            .step_compute_units
            .checked_mul(u64::from(request.payload.worker_group_size))
            .ok_or_else(|| {
                SoracloudError::bad_request("step_compute_units * worker_group_size overflows u64")
            })?;
        if request.payload.compute_budget_units < minimum_step_units {
            return Err(SoracloudError::bad_request(format!(
                "compute_budget_units must cover at least one worker-group step ({minimum_step_units})"
            )));
        }

        let service_name = service_name.to_string();
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let runtime_state = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }
            if entry.training_jobs.contains_key(&job_id) {
                return Err(SoracloudError::conflict(format!(
                    "training job `{job_id}` already exists for service `{service_name}`"
                )));
            }

            let runtime_state = TrainingJobRuntimeState {
                model_name: model_name.clone(),
                job_id: job_id.clone(),
                status: TrainingJobStatus::Running,
                worker_group_size: request.payload.worker_group_size,
                target_steps: request.payload.target_steps,
                completed_steps: 0,
                checkpoint_interval_steps: request.payload.checkpoint_interval_steps,
                last_checkpoint_step: None,
                checkpoint_count: 0,
                retry_count: 0,
                max_retries: request.payload.max_retries,
                step_compute_units: request.payload.step_compute_units,
                compute_budget_units: request.payload.compute_budget_units,
                compute_consumed_units: 0,
                storage_budget_bytes: request.payload.storage_budget_bytes,
                storage_consumed_bytes: 0,
                latest_metrics_hash: None,
                last_failure_reason: None,
                created_sequence: sequence,
                updated_sequence: sequence,
            };
            entry
                .training_jobs
                .insert(job_id.clone(), runtime_state.clone());
            runtime_state
        };

        state.training_audit_log.push(TrainingJobAuditEvent {
            sequence,
            action: TrainingJobAction::Start,
            service_name: service_name.clone(),
            model_name: model_name.clone(),
            job_id: job_id.clone(),
            status: runtime_state.status,
            completed_steps: runtime_state.completed_steps,
            checkpoint_count: runtime_state.checkpoint_count,
            retry_count: runtime_state.retry_count,
            compute_consumed_units: runtime_state.compute_consumed_units,
            storage_consumed_bytes: runtime_state.storage_consumed_bytes,
            last_checkpoint_step: runtime_state.last_checkpoint_step,
            latest_metrics_hash: runtime_state.latest_metrics_hash,
            last_failure_reason: runtime_state.last_failure_reason.clone(),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let training_event_count =
            u32::try_from(state.training_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(training_job_mutation_response(
            TrainingJobAction::Start,
            &service_name,
            &runtime_state,
            sequence,
            training_event_count,
            signer,
        ))
    }

    pub(crate) async fn apply_training_job_checkpoint(
        &self,
        request: SignedTrainingJobCheckpointRequest,
    ) -> Result<TrainingJobMutationResponse, SoracloudError> {
        verify_training_job_checkpoint_signature(&request)?;
        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let job_id = parse_training_job_id(&request.payload.job_id)?;
        if request.payload.completed_step == 0 {
            return Err(SoracloudError::bad_request(
                "completed_step must be greater than zero",
            ));
        }
        if request.payload.checkpoint_size_bytes == 0 {
            return Err(SoracloudError::bad_request(
                "checkpoint_size_bytes must be greater than zero",
            ));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let runtime_state = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }
            let job = entry.training_jobs.get_mut(&job_id).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "training job `{job_id}` not found for service `{service_name}`"
                ))
            })?;
            if matches!(
                job.status,
                TrainingJobStatus::Completed | TrainingJobStatus::Exhausted
            ) {
                return Err(SoracloudError::conflict(format!(
                    "training job `{job_id}` is not accepting checkpoints in {:?} status",
                    job.status
                )));
            }
            if request.payload.completed_step <= job.completed_steps {
                return Err(SoracloudError::conflict(format!(
                    "completed_step {} must be greater than current completed_steps {}",
                    request.payload.completed_step, job.completed_steps
                )));
            }
            if request.payload.completed_step > job.target_steps {
                return Err(SoracloudError::conflict(format!(
                    "completed_step {} exceeds target_steps {}",
                    request.payload.completed_step, job.target_steps
                )));
            }
            if request.payload.completed_step != job.target_steps
                && request.payload.completed_step % job.checkpoint_interval_steps != 0
            {
                return Err(SoracloudError::conflict(format!(
                    "completed_step {} must align with checkpoint_interval_steps {} (or equal target_steps {})",
                    request.payload.completed_step, job.checkpoint_interval_steps, job.target_steps
                )));
            }

            let delta_steps = request.payload.completed_step - job.completed_steps;
            let checkpoint_compute_units = u64::from(delta_steps)
                .checked_mul(job.step_compute_units)
                .and_then(|value| value.checked_mul(u64::from(job.worker_group_size)))
                .ok_or_else(|| {
                    SoracloudError::conflict(
                        "training checkpoint compute-cost calculation overflowed u64",
                    )
                })?;
            let next_compute_total = job
                .compute_consumed_units
                .checked_add(checkpoint_compute_units)
                .ok_or_else(|| {
                    SoracloudError::conflict("training compute consumption overflowed u64")
                })?;
            if next_compute_total > job.compute_budget_units {
                return Err(SoracloudError::conflict(format!(
                    "training checkpoint would exceed compute budget {}",
                    job.compute_budget_units
                )));
            }
            let next_storage_total = job
                .storage_consumed_bytes
                .checked_add(request.payload.checkpoint_size_bytes)
                .ok_or_else(|| {
                    SoracloudError::conflict("training storage consumption overflowed u64")
                })?;
            if next_storage_total > job.storage_budget_bytes {
                return Err(SoracloudError::conflict(format!(
                    "training checkpoint would exceed storage budget {}",
                    job.storage_budget_bytes
                )));
            }

            job.compute_consumed_units = next_compute_total;
            job.storage_consumed_bytes = next_storage_total;
            job.completed_steps = request.payload.completed_step;
            job.checkpoint_count = job.checkpoint_count.saturating_add(1);
            job.last_checkpoint_step = Some(request.payload.completed_step);
            job.latest_metrics_hash = Some(request.payload.metrics_hash);
            job.last_failure_reason = None;
            job.status = if job.completed_steps >= job.target_steps {
                TrainingJobStatus::Completed
            } else {
                TrainingJobStatus::Running
            };
            job.updated_sequence = sequence;
            job.clone()
        };

        state.training_audit_log.push(TrainingJobAuditEvent {
            sequence,
            action: TrainingJobAction::Checkpoint,
            service_name: service_name.clone(),
            model_name: runtime_state.model_name.clone(),
            job_id: job_id.clone(),
            status: runtime_state.status,
            completed_steps: runtime_state.completed_steps,
            checkpoint_count: runtime_state.checkpoint_count,
            retry_count: runtime_state.retry_count,
            compute_consumed_units: runtime_state.compute_consumed_units,
            storage_consumed_bytes: runtime_state.storage_consumed_bytes,
            last_checkpoint_step: runtime_state.last_checkpoint_step,
            latest_metrics_hash: runtime_state.latest_metrics_hash,
            last_failure_reason: runtime_state.last_failure_reason.clone(),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let training_event_count =
            u32::try_from(state.training_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(training_job_mutation_response(
            TrainingJobAction::Checkpoint,
            &service_name,
            &runtime_state,
            sequence,
            training_event_count,
            signer,
        ))
    }

    pub(crate) async fn apply_training_job_retry(
        &self,
        request: SignedTrainingJobRetryRequest,
    ) -> Result<TrainingJobMutationResponse, SoracloudError> {
        verify_training_job_retry_signature(&request)?;
        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let job_id = parse_training_job_id(&request.payload.job_id)?;
        let reason = request.payload.reason.trim();
        if reason.is_empty() {
            return Err(SoracloudError::bad_request("reason must not be empty"));
        }
        if reason.len() > TRAINING_MAX_REASON_BYTES {
            return Err(SoracloudError::bad_request(format!(
                "reason exceeds max bytes ({TRAINING_MAX_REASON_BYTES})"
            )));
        }
        if reason.chars().any(char::is_control) {
            return Err(SoracloudError::bad_request(
                "reason must not contain control characters",
            ));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let runtime_state = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }
            let job = entry.training_jobs.get_mut(&job_id).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "training job `{job_id}` not found for service `{service_name}`"
                ))
            })?;
            if job.status == TrainingJobStatus::Completed {
                return Err(SoracloudError::conflict(format!(
                    "training job `{job_id}` is already completed"
                )));
            }
            if job.status == TrainingJobStatus::Exhausted {
                return Err(SoracloudError::conflict(format!(
                    "training job `{job_id}` retry budget is exhausted"
                )));
            }
            if job.retry_count >= job.max_retries {
                return Err(SoracloudError::conflict(format!(
                    "training job `{job_id}` cannot retry because retry_count {} reached max_retries {}",
                    job.retry_count, job.max_retries
                )));
            }

            job.retry_count = job.retry_count.saturating_add(1);
            job.status = TrainingJobStatus::RetryPending;
            job.last_failure_reason = Some(reason.to_string());
            job.updated_sequence = sequence;
            job.clone()
        };

        state.training_audit_log.push(TrainingJobAuditEvent {
            sequence,
            action: TrainingJobAction::Retry,
            service_name: service_name.clone(),
            model_name: runtime_state.model_name.clone(),
            job_id: job_id.clone(),
            status: runtime_state.status,
            completed_steps: runtime_state.completed_steps,
            checkpoint_count: runtime_state.checkpoint_count,
            retry_count: runtime_state.retry_count,
            compute_consumed_units: runtime_state.compute_consumed_units,
            storage_consumed_bytes: runtime_state.storage_consumed_bytes,
            last_checkpoint_step: runtime_state.last_checkpoint_step,
            latest_metrics_hash: runtime_state.latest_metrics_hash,
            last_failure_reason: runtime_state.last_failure_reason.clone(),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let training_event_count =
            u32::try_from(state.training_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(training_job_mutation_response(
            TrainingJobAction::Retry,
            &service_name,
            &runtime_state,
            sequence,
            training_event_count,
            signer,
        ))
    }

    pub(crate) async fn training_job_status(
        &self,
        service_name: &str,
        job_id: &str,
    ) -> Result<TrainingJobStatusResponse, SoracloudError> {
        let service_name: Name = service_name
            .parse()
            .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
        let service_name = service_name.to_string();
        let job_id = parse_training_job_id(job_id)?;

        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let entry = state.services.get(&service_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in control-plane registry"
            ))
        })?;
        let job = entry.training_jobs.get(&job_id).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "training job `{job_id}` not found for service `{service_name}`"
            ))
        })?;
        Ok(TrainingJobStatusResponse {
            schema_version: TRAINING_JOB_STATUS_SCHEMA_VERSION_V1,
            job: training_job_status_entry(&service_name, job),
        })
    }

    pub(crate) async fn apply_model_artifact_register(
        &self,
        request: SignedModelArtifactRegisterRequest,
    ) -> Result<ModelArtifactMutationResponse, SoracloudError> {
        verify_model_artifact_register_signature(&request)?;
        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let model_name = parse_training_model_name(&request.payload.model_name)?;
        let training_job_id = parse_training_job_id(&request.payload.training_job_id)?;
        let dataset_ref = parse_model_weight_dataset_ref(&request.payload.dataset_ref)?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let model_artifact_count = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }
            let training_job = entry.training_jobs.get(&training_job_id).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "training job `{training_job_id}` not found for service `{service_name}`"
                ))
            })?;
            if training_job.model_name != model_name {
                return Err(SoracloudError::conflict(format!(
                    "training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    training_job.model_name
                )));
            }
            if training_job.status != TrainingJobStatus::Completed {
                return Err(SoracloudError::conflict(format!(
                    "training job `{training_job_id}` is not completed"
                )));
            }
            if entry.model_artifacts.contains_key(&training_job_id) {
                return Err(SoracloudError::conflict(format!(
                    "artifact metadata for training job `{training_job_id}` already registered for service `{service_name}`"
                )));
            }

            entry.model_artifacts.insert(
                training_job_id.clone(),
                ModelArtifactState {
                    model_name: model_name.clone(),
                    training_job_id: training_job_id.clone(),
                    weight_artifact_hash: request.payload.weight_artifact_hash,
                    dataset_ref,
                    training_config_hash: request.payload.training_config_hash,
                    reproducibility_hash: request.payload.reproducibility_hash,
                    provenance_attestation_hash: request.payload.provenance_attestation_hash,
                    registered_sequence: sequence,
                    consumed_by_version: None,
                },
            );

            u32::try_from(entry.model_artifacts.len()).unwrap_or(u32::MAX)
        };

        state
            .model_artifact_audit_log
            .push(ModelArtifactAuditEvent {
                sequence,
                action: ModelArtifactAction::Register,
                service_name: service_name.clone(),
                model_name: model_name.clone(),
                training_job_id: training_job_id.clone(),
                consumed_by_version: None,
                signed_by: signer.clone(),
            });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(ModelArtifactMutationResponse {
            action: ModelArtifactAction::Register,
            service_name,
            model_name,
            training_job_id,
            sequence,
            model_artifact_count,
            signed_by: signer,
        })
    }

    pub(crate) async fn model_artifact_status(
        &self,
        service_name: &str,
        training_job_id: &str,
    ) -> Result<ModelArtifactStatusResponse, SoracloudError> {
        let service_name: Name = service_name
            .parse()
            .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
        let service_name = service_name.to_string();
        let training_job_id = parse_training_job_id(training_job_id)?;

        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let entry = state.services.get(&service_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in control-plane registry"
            ))
        })?;
        let artifact = entry.model_artifacts.get(&training_job_id).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "artifact metadata for training job `{training_job_id}` not found for service `{service_name}`"
            ))
        })?;

        Ok(ModelArtifactStatusResponse {
            schema_version: MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1,
            artifact: model_artifact_status_entry(&service_name, artifact),
        })
    }

    pub(crate) async fn apply_model_weight_register(
        &self,
        request: SignedModelWeightRegisterRequest,
    ) -> Result<ModelWeightMutationResponse, SoracloudError> {
        verify_model_weight_register_signature(&request)?;
        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let model_name = parse_training_model_name(&request.payload.model_name)?;
        let weight_version = parse_model_weight_version(&request.payload.weight_version)?;
        let training_job_id = parse_training_job_id(&request.payload.training_job_id)?;
        let parent_version = request
            .payload
            .parent_version
            .as_deref()
            .map(parse_model_weight_version)
            .transpose()?;
        let dataset_ref = parse_model_weight_dataset_ref(&request.payload.dataset_ref)?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (current_version, lineage_parent, version_count) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }
            let training_job = entry.training_jobs.get(&training_job_id).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "training job `{training_job_id}` not found for service `{service_name}`"
                ))
            })?;
            if training_job.model_name != model_name {
                return Err(SoracloudError::conflict(format!(
                    "training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    training_job.model_name
                )));
            }
            if training_job.status != TrainingJobStatus::Completed {
                return Err(SoracloudError::conflict(format!(
                    "training job `{training_job_id}` is not completed"
                )));
            }
            let artifact = entry.model_artifacts.get_mut(&training_job_id).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "artifact metadata for training job `{training_job_id}` not found for service `{service_name}`"
                ))
            })?;
            if artifact.model_name != model_name {
                return Err(SoracloudError::conflict(format!(
                    "artifact metadata for training job `{training_job_id}` model `{}` does not match requested model `{model_name}`",
                    artifact.model_name
                )));
            }
            if artifact.consumed_by_version.is_some() {
                return Err(SoracloudError::conflict(format!(
                    "artifact metadata for training job `{training_job_id}` was already consumed by another model weight version"
                )));
            }
            if artifact.weight_artifact_hash != request.payload.weight_artifact_hash {
                return Err(SoracloudError::conflict(format!(
                    "weight_artifact_hash mismatch for training job `{training_job_id}`"
                )));
            }
            if artifact.dataset_ref != dataset_ref {
                return Err(SoracloudError::conflict(format!(
                    "dataset_ref mismatch for training job `{training_job_id}`"
                )));
            }
            if artifact.training_config_hash != request.payload.training_config_hash {
                return Err(SoracloudError::conflict(format!(
                    "training_config_hash mismatch for training job `{training_job_id}`"
                )));
            }
            if artifact.reproducibility_hash != request.payload.reproducibility_hash {
                return Err(SoracloudError::conflict(format!(
                    "reproducibility_hash mismatch for training job `{training_job_id}`"
                )));
            }
            if artifact.provenance_attestation_hash != request.payload.provenance_attestation_hash {
                return Err(SoracloudError::conflict(format!(
                    "provenance_attestation_hash mismatch for training job `{training_job_id}`"
                )));
            }

            let model_registry = entry
                .model_weights
                .entry(model_name.clone())
                .or_insert_with(|| ModelWeightRegistryState {
                    model_name: model_name.clone(),
                    current_version: None,
                    versions: BTreeMap::new(),
                });
            if model_registry.versions.contains_key(&weight_version) {
                return Err(SoracloudError::conflict(format!(
                    "model `{model_name}` weight version `{weight_version}` already exists for service `{service_name}`"
                )));
            }
            let lineage_parent = match (model_registry.versions.is_empty(), parent_version.clone())
            {
                (true, None) => None,
                (true, Some(_)) => {
                    return Err(SoracloudError::conflict(
                        "parent_version must be omitted for the first model weight version",
                    ));
                }
                (false, None) => {
                    return Err(SoracloudError::conflict(
                        "parent_version is required when registering subsequent weight versions",
                    ));
                }
                (false, Some(parent)) => {
                    if !model_registry.versions.contains_key(&parent) {
                        return Err(SoracloudError::not_found(format!(
                            "parent_version `{parent}` not found for model `{model_name}`"
                        )));
                    }
                    Some(parent)
                }
            };

            model_registry.versions.insert(
                weight_version.clone(),
                ModelWeightVersionState {
                    weight_version: weight_version.clone(),
                    parent_version: lineage_parent.clone(),
                    training_job_id: training_job_id.clone(),
                    weight_artifact_hash: request.payload.weight_artifact_hash,
                    dataset_ref,
                    training_config_hash: request.payload.training_config_hash,
                    reproducibility_hash: request.payload.reproducibility_hash,
                    provenance_attestation_hash: request.payload.provenance_attestation_hash,
                    registered_sequence: sequence,
                    promoted_sequence: None,
                    gate_report_hash: None,
                    promoted_by: None,
                },
            );
            artifact.consumed_by_version = Some(weight_version.clone());

            (
                model_registry.current_version.clone(),
                lineage_parent,
                u32::try_from(model_registry.versions.len()).unwrap_or(u32::MAX),
            )
        };

        state.model_weight_audit_log.push(ModelWeightAuditEvent {
            sequence,
            action: ModelWeightAction::Register,
            service_name: service_name.clone(),
            model_name: model_name.clone(),
            target_version: weight_version.clone(),
            current_version: current_version.clone(),
            parent_version: lineage_parent.clone(),
            gate_approved: None,
            rollback_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let model_event_count =
            u32::try_from(state.model_weight_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(model_weight_mutation_response(
            ModelWeightAction::Register,
            &service_name,
            &model_name,
            &weight_version,
            current_version,
            lineage_parent,
            sequence,
            version_count,
            model_event_count,
            signer,
        ))
    }

    pub(crate) async fn apply_model_weight_promote(
        &self,
        request: SignedModelWeightPromoteRequest,
    ) -> Result<ModelWeightMutationResponse, SoracloudError> {
        verify_model_weight_promote_signature(&request)?;
        if !request.payload.gate_approved {
            return Err(SoracloudError::conflict(
                "model promotion gate is not approved",
            ));
        }

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let model_name = parse_training_model_name(&request.payload.model_name)?;
        let weight_version = parse_model_weight_version(&request.payload.weight_version)?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (current_version, parent_version, version_count) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }

            let model_registry = entry.model_weights.get_mut(&model_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "model `{model_name}` is not registered for service `{service_name}`"
                ))
            })?;
            if model_registry.current_version.as_deref() == Some(weight_version.as_str()) {
                return Err(SoracloudError::conflict(format!(
                    "model `{model_name}` weight version `{weight_version}` is already promoted"
                )));
            }
            let weight = model_registry
                .versions
                .get_mut(&weight_version)
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "weight version `{weight_version}` not found for model `{model_name}`"
                    ))
                })?;
            weight.promoted_sequence = Some(sequence);
            weight.gate_report_hash = Some(request.payload.gate_report_hash);
            weight.promoted_by = Some(signer.clone());
            let parent_version = weight.parent_version.clone();

            model_registry.current_version = Some(weight_version.clone());
            (
                model_registry.current_version.clone(),
                parent_version,
                u32::try_from(model_registry.versions.len()).unwrap_or(u32::MAX),
            )
        };

        state.model_weight_audit_log.push(ModelWeightAuditEvent {
            sequence,
            action: ModelWeightAction::Promote,
            service_name: service_name.clone(),
            model_name: model_name.clone(),
            target_version: weight_version.clone(),
            current_version: current_version.clone(),
            parent_version: parent_version.clone(),
            gate_approved: Some(true),
            rollback_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let model_event_count =
            u32::try_from(state.model_weight_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(model_weight_mutation_response(
            ModelWeightAction::Promote,
            &service_name,
            &model_name,
            &weight_version,
            current_version,
            parent_version,
            sequence,
            version_count,
            model_event_count,
            signer,
        ))
    }

    pub(crate) async fn apply_model_weight_rollback(
        &self,
        request: SignedModelWeightRollbackRequest,
    ) -> Result<ModelWeightMutationResponse, SoracloudError> {
        verify_model_weight_rollback_signature(&request)?;
        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let model_name = parse_training_model_name(&request.payload.model_name)?;
        let target_version = parse_model_weight_version(&request.payload.target_version)?;
        let reason = normalize_model_weight_reason(&request.payload.reason)?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let (current_version, parent_version, version_count) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;
            if !current_revision.allow_model_training {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active revision does not allow model training"
                )));
            }

            let model_registry = entry.model_weights.get_mut(&model_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "model `{model_name}` is not registered for service `{service_name}`"
                ))
            })?;
            if !model_registry.versions.contains_key(&target_version) {
                return Err(SoracloudError::not_found(format!(
                    "weight version `{target_version}` not found for model `{model_name}`"
                )));
            }
            if model_registry.current_version.as_deref() == Some(target_version.as_str()) {
                return Err(SoracloudError::conflict(format!(
                    "model `{model_name}` is already at weight version `{target_version}`"
                )));
            }
            let parent_version = model_registry
                .versions
                .get(&target_version)
                .and_then(|version| version.parent_version.clone());
            model_registry.current_version = Some(target_version.clone());
            (
                model_registry.current_version.clone(),
                parent_version,
                u32::try_from(model_registry.versions.len()).unwrap_or(u32::MAX),
            )
        };

        state.model_weight_audit_log.push(ModelWeightAuditEvent {
            sequence,
            action: ModelWeightAction::Rollback,
            service_name: service_name.clone(),
            model_name: model_name.clone(),
            target_version: target_version.clone(),
            current_version: current_version.clone(),
            parent_version: parent_version.clone(),
            gate_approved: None,
            rollback_reason: Some(reason),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let model_event_count =
            u32::try_from(state.model_weight_audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(model_weight_mutation_response(
            ModelWeightAction::Rollback,
            &service_name,
            &model_name,
            &target_version,
            current_version,
            parent_version,
            sequence,
            version_count,
            model_event_count,
            signer,
        ))
    }

    pub(crate) async fn model_weight_status(
        &self,
        service_name: &str,
        model_name: &str,
    ) -> Result<ModelWeightStatusResponse, SoracloudError> {
        let service_name: Name = service_name
            .parse()
            .map_err(|err| SoracloudError::bad_request(format!("invalid service_name: {err}")))?;
        let service_name = service_name.to_string();
        let model_name = parse_training_model_name(model_name)?;

        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let entry = state.services.get(&service_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` not found in control-plane registry"
            ))
        })?;
        let model_registry = entry.model_weights.get(&model_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "model `{model_name}` is not registered for service `{service_name}`"
            ))
        })?;

        Ok(ModelWeightStatusResponse {
            schema_version: MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1,
            model: model_weight_status_entry(&service_name, model_registry),
        })
    }

    pub(crate) async fn apply_agent_deploy(
        &self,
        request: SignedAgentDeployRequest,
    ) -> Result<AgentMutationResponse, SoracloudError> {
        verify_agent_deploy_signature(&request)?;
        request.payload.manifest.validate().map_err(|err| {
            SoracloudError::bad_request(format!(
                "agent apartment manifest failed validation: {err}"
            ))
        })?;
        if request.payload.lease_ticks == 0 {
            return Err(SoracloudError::bad_request(
                "lease_ticks must be greater than zero",
            ));
        }

        let autonomy_budget_units = request
            .payload
            .autonomy_budget_units
            .unwrap_or(AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS);
        if autonomy_budget_units == 0 {
            return Err(SoracloudError::bad_request(
                "autonomy_budget_units must be greater than zero",
            ));
        }

        let apartment_name = request.payload.manifest.apartment_name.to_string();
        let signer = request.provenance.signer.to_string();
        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        if state.apartments.contains_key(&apartment_name) {
            return Err(SoracloudError::conflict(format!(
                "apartment `{apartment_name}` already exists in control-plane runtime"
            )));
        }

        let sequence = state.next_sequence;
        let manifest_hash =
            Hash::new(norito::to_bytes(&request.payload.manifest).map_err(|err| {
                SoracloudError::internal(format!("failed to encode agent manifest payload: {err}"))
            })?);
        let runtime_state = AgentApartmentRuntimeState {
            manifest: request.payload.manifest,
            manifest_hash,
            status: AgentRuntimeStatus::Running,
            deployed_sequence: sequence,
            lease_started_sequence: sequence,
            lease_expires_sequence: sequence.saturating_add(request.payload.lease_ticks),
            last_renewed_sequence: sequence,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 1,
            process_started_sequence: sequence,
            last_active_sequence: sequence,
            last_checkpoint_sequence: None,
            checkpoint_count: 0,
            persistent_state: BindingRuntimeState::default(),
            revoked_policy_capabilities: BTreeSet::new(),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: autonomy_budget_units,
            autonomy_budget_remaining_units: autonomy_budget_units,
            artifact_allowlist: BTreeMap::new(),
            autonomy_run_history: Vec::new(),
        };
        let response = AgentMutationResponse {
            action: AgentApartmentAction::Deploy,
            apartment_name: apartment_name.clone(),
            sequence,
            status: agent_runtime_status_for_sequence(&runtime_state, sequence.saturating_add(1)),
            lease_expires_sequence: runtime_state.lease_expires_sequence,
            lease_remaining_ticks: agent_lease_remaining_ticks(
                &runtime_state,
                sequence.saturating_add(1),
            ),
            manifest_hash,
            restart_count: 0,
            pending_wallet_request_count: 0,
            revoked_policy_capability_count: 0,
            budget_remaining_units: runtime_state.autonomy_budget_remaining_units,
            allowlist_count: 0,
            run_count: 0,
            process_generation: runtime_state.process_generation,
            process_started_sequence: runtime_state.process_started_sequence,
            last_active_sequence: runtime_state.last_active_sequence,
            last_checkpoint_sequence: runtime_state.last_checkpoint_sequence,
            checkpoint_count: runtime_state.checkpoint_count,
            persistent_state_total_bytes: runtime_state.persistent_state.total_bytes,
            persistent_state_key_count: 0,
            audit_event_count: 0,
            signed_by: signer.clone(),
            capability: None,
            reason: None,
            last_restart_sequence: None,
            last_restart_reason: None,
        };

        state
            .apartments
            .insert(apartment_name.clone(), runtime_state);
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::Deploy,
            apartment_name,
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash,
            restart_count: response.restart_count,
            signed_by: signer,
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
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_lease_renew(
        &self,
        request: SignedAgentLeaseRenewRequest,
    ) -> Result<AgentMutationResponse, SoracloudError> {
        verify_agent_lease_renew_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        if request.payload.lease_ticks == 0 {
            return Err(SoracloudError::bad_request(
                "lease_ticks must be greater than zero",
            ));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            let base = runtime.lease_expires_sequence.max(sequence);
            runtime.lease_expires_sequence = base.saturating_add(request.payload.lease_ticks);
            runtime.last_renewed_sequence = sequence;
            runtime.status = AgentRuntimeStatus::Running;
            touch_agent_runtime_activity(runtime, sequence);

            AgentMutationResponse {
                action: AgentApartmentAction::LeaseRenew,
                apartment_name: apartment_name.clone(),
                sequence,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                lease_expires_sequence: runtime.lease_expires_sequence,
                lease_remaining_ticks: agent_lease_remaining_ticks(
                    runtime,
                    sequence.saturating_add(1),
                ),
                manifest_hash: runtime.manifest_hash,
                restart_count: runtime.restart_count,
                pending_wallet_request_count: agent_pending_wallet_request_count(runtime),
                revoked_policy_capability_count: agent_revoked_capability_count(runtime),
                budget_remaining_units: runtime.autonomy_budget_remaining_units,
                allowlist_count: agent_allowlist_count(runtime),
                run_count: agent_run_count(runtime),
                process_generation: runtime.process_generation,
                process_started_sequence: runtime.process_started_sequence,
                last_active_sequence: runtime.last_active_sequence,
                last_checkpoint_sequence: runtime.last_checkpoint_sequence,
                checkpoint_count: runtime.checkpoint_count,
                persistent_state_total_bytes: runtime.persistent_state.total_bytes,
                persistent_state_key_count: agent_persistent_state_key_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
                capability: None,
                reason: None,
                last_restart_sequence: runtime.last_restart_sequence,
                last_restart_reason: runtime.last_restart_reason.clone(),
            }
        };

        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::LeaseRenew,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count: response.restart_count,
            signed_by: signer,
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
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_restart(
        &self,
        request: SignedAgentRestartRequest,
    ) -> Result<AgentMutationResponse, SoracloudError> {
        verify_agent_restart_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let reason = request.payload.reason.trim();
        if reason.is_empty() {
            return Err(SoracloudError::bad_request("reason must not be empty"));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(runtime, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before restart",
                    runtime.lease_expires_sequence
                )));
            }

            runtime.status = AgentRuntimeStatus::Running;
            runtime.restart_count = runtime.restart_count.saturating_add(1);
            runtime.last_restart_sequence = Some(sequence);
            runtime.last_restart_reason = Some(reason.to_owned());
            runtime.process_generation = runtime.process_generation.saturating_add(1).max(1);
            runtime.process_started_sequence = sequence;
            touch_agent_runtime_activity(runtime, sequence);

            AgentMutationResponse {
                action: AgentApartmentAction::Restart,
                apartment_name: apartment_name.clone(),
                sequence,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                lease_expires_sequence: runtime.lease_expires_sequence,
                lease_remaining_ticks: agent_lease_remaining_ticks(
                    runtime,
                    sequence.saturating_add(1),
                ),
                manifest_hash: runtime.manifest_hash,
                restart_count: runtime.restart_count,
                pending_wallet_request_count: agent_pending_wallet_request_count(runtime),
                revoked_policy_capability_count: agent_revoked_capability_count(runtime),
                budget_remaining_units: runtime.autonomy_budget_remaining_units,
                allowlist_count: agent_allowlist_count(runtime),
                run_count: agent_run_count(runtime),
                process_generation: runtime.process_generation,
                process_started_sequence: runtime.process_started_sequence,
                last_active_sequence: runtime.last_active_sequence,
                last_checkpoint_sequence: runtime.last_checkpoint_sequence,
                checkpoint_count: runtime.checkpoint_count,
                persistent_state_total_bytes: runtime.persistent_state.total_bytes,
                persistent_state_key_count: agent_persistent_state_key_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
                capability: None,
                reason: Some(reason.to_owned()),
                last_restart_sequence: runtime.last_restart_sequence,
                last_restart_reason: runtime.last_restart_reason.clone(),
            }
        };

        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::Restart,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count: response.restart_count,
            signed_by: signer,
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: response.reason.clone(),
            from_apartment: None,
            to_apartment: None,
            channel: None,
            payload_hash: None,
            artifact_hash: None,
            provenance_hash: None,
            run_id: None,
            run_label: None,
            budget_units: None,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn agent_status(
        &self,
        apartment_name: Option<&str>,
    ) -> Result<AgentStatusResponse, SoracloudError> {
        let apartment_filter = apartment_name.map(parse_agent_apartment_name).transpose()?;
        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let sequence = state.next_sequence;

        let mut apartments = Vec::new();
        for (name, runtime) in &state.apartments {
            if apartment_filter
                .as_ref()
                .is_some_and(|filter| filter.as_str() != name.as_str())
            {
                continue;
            }
            apartments.push(AgentApartmentStatusEntry::from_state(
                name, runtime, sequence,
            ));
        }

        Ok(AgentStatusResponse {
            schema_version: state.schema_version,
            apartment_count: u32::try_from(apartments.len()).unwrap_or(u32::MAX),
            event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            apartments,
        })
    }

    pub(crate) async fn apply_agent_policy_revoke(
        &self,
        request: SignedAgentPolicyRevokeRequest,
    ) -> Result<AgentMutationResponse, SoracloudError> {
        verify_agent_policy_revoke_signature(&request)?;

        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let capability = parse_agent_capability_name(&request.payload.capability)?;
        let reason = request
            .payload
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            let capability_declared = runtime
                .manifest
                .policy_capabilities
                .iter()
                .any(|candidate| candidate.as_ref() == capability.as_str());
            if !capability_declared {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not declare policy capability `{capability}`"
                )));
            }
            if runtime
                .revoked_policy_capabilities
                .contains(capability.as_str())
            {
                return Err(SoracloudError::conflict(format!(
                    "policy capability `{capability}` already revoked for apartment `{apartment_name}`"
                )));
            }
            runtime
                .revoked_policy_capabilities
                .insert(capability.clone());
            touch_agent_runtime_activity(runtime, sequence);

            AgentMutationResponse {
                action: AgentApartmentAction::PolicyRevoked,
                apartment_name: apartment_name.clone(),
                sequence,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                lease_expires_sequence: runtime.lease_expires_sequence,
                lease_remaining_ticks: agent_lease_remaining_ticks(
                    runtime,
                    sequence.saturating_add(1),
                ),
                manifest_hash: runtime.manifest_hash,
                restart_count: runtime.restart_count,
                pending_wallet_request_count: agent_pending_wallet_request_count(runtime),
                revoked_policy_capability_count: agent_revoked_capability_count(runtime),
                budget_remaining_units: runtime.autonomy_budget_remaining_units,
                allowlist_count: agent_allowlist_count(runtime),
                run_count: agent_run_count(runtime),
                process_generation: runtime.process_generation,
                process_started_sequence: runtime.process_started_sequence,
                last_active_sequence: runtime.last_active_sequence,
                last_checkpoint_sequence: runtime.last_checkpoint_sequence,
                checkpoint_count: runtime.checkpoint_count,
                persistent_state_total_bytes: runtime.persistent_state.total_bytes,
                persistent_state_key_count: agent_persistent_state_key_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
                capability: Some(capability.clone()),
                reason: reason.clone(),
                last_restart_sequence: runtime.last_restart_sequence,
                last_restart_reason: runtime.last_restart_reason.clone(),
            }
        };

        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::PolicyRevoked,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count: response.restart_count,
            signed_by: signer,
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            capability: Some(capability),
            reason: reason.clone(),
            from_apartment: None,
            to_apartment: None,
            channel: None,
            payload_hash: None,
            artifact_hash: None,
            provenance_hash: None,
            run_id: None,
            run_label: None,
            budget_units: None,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_wallet_spend(
        &self,
        request: SignedAgentWalletSpendRequest,
    ) -> Result<AgentWalletMutationResponse, SoracloudError> {
        verify_agent_wallet_spend_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let asset_definition = request.payload.asset_definition.trim();
        if asset_definition.is_empty() {
            return Err(SoracloudError::bad_request(
                "asset_definition must not be empty",
            ));
        }
        if request.payload.amount_nanos == 0 {
            return Err(SoracloudError::bad_request(
                "amount_nanos must be greater than zero",
            ));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(runtime, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                    runtime.lease_expires_sequence
                )));
            }
            if !agent_policy_capability_active(runtime, "wallet.sign") {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not have active `wallet.sign` capability"
                )));
            }
            let spend_limit = runtime
                .manifest
                .spend_limits
                .iter()
                .find(|limit| limit.asset_definition == asset_definition)
                .ok_or_else(|| {
                    SoracloudError::conflict(format!(
                        "apartment `{apartment_name}` has no spend limit configured for asset `{asset_definition}`"
                    ))
                })?;
            if request.payload.amount_nanos > spend_limit.max_per_tx_nanos.get() {
                return Err(SoracloudError::conflict(format!(
                    "requested amount {} exceeds max_per_tx_nanos {} for asset `{asset_definition}`",
                    request.payload.amount_nanos,
                    spend_limit.max_per_tx_nanos.get()
                )));
            }

            let day_bucket = wallet_day_bucket(sequence);
            let current_day_spent = wallet_day_spent(runtime, asset_definition, day_bucket);
            let projected_day_spent = current_day_spent
                .checked_add(request.payload.amount_nanos)
                .ok_or_else(|| {
                    SoracloudError::internal(format!(
                        "wallet daily spend overflow for apartment `{apartment_name}`"
                    ))
                })?;
            if projected_day_spent > spend_limit.max_per_day_nanos.get() {
                return Err(SoracloudError::conflict(format!(
                    "projected daily spend {} exceeds max_per_day_nanos {} for asset `{asset_definition}`",
                    projected_day_spent,
                    spend_limit.max_per_day_nanos.get()
                )));
            }

            let request_id = format!("{apartment_name}:wallet:{sequence}");
            let action = if agent_policy_capability_active(runtime, "wallet.auto_approve") {
                wallet_record_spend(runtime, asset_definition, day_bucket, projected_day_spent);
                AgentApartmentAction::WalletSpendApproved
            } else {
                runtime.pending_wallet_requests.insert(
                    request_id.clone(),
                    AgentWalletSpendRequest {
                        request_id: request_id.clone(),
                        asset_definition: asset_definition.to_owned(),
                        amount_nanos: request.payload.amount_nanos,
                        created_sequence: sequence,
                    },
                );
                AgentApartmentAction::WalletSpendRequested
            };
            touch_agent_runtime_activity(runtime, sequence);

            let day_spent_nanos = wallet_day_spent(runtime, asset_definition, day_bucket);
            AgentWalletMutationResponse {
                action,
                apartment_name: apartment_name.clone(),
                sequence,
                manifest_hash: runtime.manifest_hash,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                request_id: Some(request_id),
                asset_definition: Some(asset_definition.to_owned()),
                amount_nanos: Some(request.payload.amount_nanos),
                day_bucket: Some(day_bucket),
                day_spent_nanos: Some(day_spent_nanos),
                capability: None,
                reason: None,
                pending_request_count: agent_pending_wallet_request_count(runtime),
                revoked_policy_capability_count: agent_revoked_capability_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };

        let (lease_expires_sequence, restart_count) = state
            .apartments
            .get(&apartment_name)
            .map(|runtime| (runtime.lease_expires_sequence, runtime.restart_count))
            .unwrap_or((sequence, 0));
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: response.action,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: response.request_id.clone(),
            asset_definition: response.asset_definition.clone(),
            amount_nanos: response.amount_nanos,
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
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentWalletMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_wallet_approve(
        &self,
        request: SignedAgentWalletApproveRequest,
    ) -> Result<AgentWalletMutationResponse, SoracloudError> {
        verify_agent_wallet_approve_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let request_id = request.payload.request_id.trim();
        if request_id.is_empty() {
            return Err(SoracloudError::bad_request("request_id must not be empty"));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(runtime, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                    runtime.lease_expires_sequence
                )));
            }
            if !agent_policy_capability_active(runtime, "wallet.sign") {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not have active `wallet.sign` capability"
                )));
            }

            let pending = runtime
                .pending_wallet_requests
                .remove(request_id)
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "wallet request `{request_id}` not found for apartment `{apartment_name}`"
                    ))
                })?;
            let spend_limit = runtime
                .manifest
                .spend_limits
                .iter()
                .find(|limit| limit.asset_definition == pending.asset_definition)
                .ok_or_else(|| {
                    SoracloudError::conflict(format!(
                        "apartment `{apartment_name}` has no spend limit configured for asset `{}`",
                        pending.asset_definition
                    ))
                })?;

            let day_bucket = wallet_day_bucket(sequence);
            let current_day_spent =
                wallet_day_spent(runtime, &pending.asset_definition, day_bucket);
            let projected_day_spent = current_day_spent
                .checked_add(pending.amount_nanos)
                .ok_or_else(|| {
                    SoracloudError::internal(format!(
                        "wallet daily spend overflow for apartment `{apartment_name}`"
                    ))
                })?;
            if projected_day_spent > spend_limit.max_per_day_nanos.get() {
                return Err(SoracloudError::conflict(format!(
                    "projected daily spend {} exceeds max_per_day_nanos {} for asset `{}`",
                    projected_day_spent,
                    spend_limit.max_per_day_nanos.get(),
                    pending.asset_definition
                )));
            }
            wallet_record_spend(
                runtime,
                &pending.asset_definition,
                day_bucket,
                projected_day_spent,
            );
            touch_agent_runtime_activity(runtime, sequence);

            AgentWalletMutationResponse {
                action: AgentApartmentAction::WalletSpendApproved,
                apartment_name: apartment_name.clone(),
                sequence,
                manifest_hash: runtime.manifest_hash,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                request_id: Some(pending.request_id),
                asset_definition: Some(pending.asset_definition),
                amount_nanos: Some(pending.amount_nanos),
                day_bucket: Some(day_bucket),
                day_spent_nanos: Some(projected_day_spent),
                capability: None,
                reason: None,
                pending_request_count: agent_pending_wallet_request_count(runtime),
                revoked_policy_capability_count: agent_revoked_capability_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };

        let (lease_expires_sequence, restart_count) = state
            .apartments
            .get(&apartment_name)
            .map(|runtime| (runtime.lease_expires_sequence, runtime.restart_count))
            .unwrap_or((sequence, 0));
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::WalletSpendApproved,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: response.request_id.clone(),
            asset_definition: response.asset_definition.clone(),
            amount_nanos: response.amount_nanos,
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
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentWalletMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_message_send(
        &self,
        request: SignedAgentMessageSendRequest,
    ) -> Result<AgentMailboxMutationResponse, SoracloudError> {
        verify_agent_message_send_signature(&request)?;
        let from_apartment = parse_agent_apartment_name(&request.payload.from_apartment)?;
        let to_apartment = parse_agent_apartment_name(&request.payload.to_apartment)?;
        let channel = request.payload.channel.trim();
        if channel.is_empty() {
            return Err(SoracloudError::bad_request("channel must not be empty"));
        }
        let payload = request.payload.payload.trim();
        if payload.is_empty() {
            return Err(SoracloudError::bad_request("payload must not be empty"));
        }
        if payload.len() > AGENT_MAILBOX_MAX_PAYLOAD_BYTES {
            return Err(SoracloudError::bad_request(format!(
                "payload exceeds max mailbox payload bytes ({AGENT_MAILBOX_MAX_PAYLOAD_BYTES})"
            )));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let sender = state.apartments.get(&from_apartment).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{from_apartment}` not found in control-plane runtime"
            ))
        })?;
        if agent_runtime_status_for_sequence(sender, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(SoracloudError::conflict(format!(
                "sender apartment `{from_apartment}` lease expired at sequence {}; renew before messaging",
                sender.lease_expires_sequence
            )));
        }
        if !agent_policy_capability_active(sender, "agent.mailbox.send") {
            return Err(SoracloudError::conflict(format!(
                "apartment `{from_apartment}` does not have active `agent.mailbox.send` capability"
            )));
        }

        let recipient_snapshot = state.apartments.get(&to_apartment).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{to_apartment}` not found in control-plane runtime"
            ))
        })?;
        if agent_runtime_status_for_sequence(recipient_snapshot, sequence)
            == AgentRuntimeStatus::LeaseExpired
        {
            return Err(SoracloudError::conflict(format!(
                "recipient apartment `{to_apartment}` lease expired at sequence {}; renew before messaging",
                recipient_snapshot.lease_expires_sequence
            )));
        }
        if !agent_policy_capability_active(recipient_snapshot, "agent.mailbox.receive") {
            return Err(SoracloudError::conflict(format!(
                "apartment `{to_apartment}` does not have active `agent.mailbox.receive` capability"
            )));
        }

        let message_id = format!("{to_apartment}:mail:{sequence}");
        let payload_hash = Hash::new(payload.as_bytes());
        let response = {
            let recipient = state.apartments.get_mut(&to_apartment).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{to_apartment}` not found in control-plane runtime"
                ))
            })?;
            recipient.mailbox_queue.push(AgentMailboxMessage {
                message_id: message_id.clone(),
                from_apartment: from_apartment.clone(),
                channel: channel.to_owned(),
                payload: payload.to_owned(),
                payload_hash,
                enqueued_sequence: sequence,
            });
            touch_agent_runtime_activity(recipient, sequence);

            AgentMailboxMutationResponse {
                action: AgentApartmentAction::MessageEnqueued,
                apartment_name: to_apartment.clone(),
                sequence,
                message_id: message_id.clone(),
                from_apartment: Some(from_apartment.clone()),
                to_apartment: Some(to_apartment.clone()),
                channel: channel.to_owned(),
                payload_hash,
                status: agent_runtime_status_for_sequence(recipient, sequence.saturating_add(1)),
                pending_message_count: agent_pending_mailbox_message_count(recipient),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };
        if let Some(sender_runtime) = state.apartments.get_mut(&from_apartment) {
            touch_agent_runtime_activity(sender_runtime, sequence);
        }

        let (lease_expires_sequence, manifest_hash, restart_count) = state
            .apartments
            .get(&to_apartment)
            .map(|runtime| {
                (
                    runtime.lease_expires_sequence,
                    runtime.manifest_hash,
                    runtime.restart_count,
                )
            })
            .unwrap_or((sequence, payload_hash, 0));
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::MessageEnqueued,
            apartment_name: to_apartment.clone(),
            status: response.status,
            lease_expires_sequence,
            manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: Some(message_id),
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: None,
            from_apartment: Some(from_apartment),
            to_apartment: Some(to_apartment),
            channel: Some(channel.to_owned()),
            payload_hash: Some(payload_hash),
            artifact_hash: None,
            provenance_hash: None,
            run_id: None,
            run_label: None,
            budget_units: None,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMailboxMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_message_ack(
        &self,
        request: SignedAgentMessageAckRequest,
    ) -> Result<AgentMailboxMutationResponse, SoracloudError> {
        verify_agent_message_ack_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let message_id = request.payload.message_id.trim();
        if message_id.is_empty() {
            return Err(SoracloudError::bad_request("message_id must not be empty"));
        }
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;
        let response = {
            let recipient = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(recipient, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before mailbox actions",
                    recipient.lease_expires_sequence
                )));
            }
            if !agent_policy_capability_active(recipient, "agent.mailbox.receive") {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not have active `agent.mailbox.receive` capability"
                )));
            }
            let message_index = recipient
                .mailbox_queue
                .iter()
                .position(|message| message.message_id == message_id)
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "mailbox message `{message_id}` not found for apartment `{apartment_name}`"
                    ))
                })?;
            let message = recipient.mailbox_queue.remove(message_index);
            touch_agent_runtime_activity(recipient, sequence);

            AgentMailboxMutationResponse {
                action: AgentApartmentAction::MessageAcknowledged,
                apartment_name: apartment_name.clone(),
                sequence,
                message_id: message.message_id,
                from_apartment: Some(message.from_apartment),
                to_apartment: Some(apartment_name.clone()),
                channel: message.channel,
                payload_hash: message.payload_hash,
                status: agent_runtime_status_for_sequence(recipient, sequence.saturating_add(1)),
                pending_message_count: agent_pending_mailbox_message_count(recipient),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };

        let (lease_expires_sequence, manifest_hash, restart_count) = state
            .apartments
            .get(&apartment_name)
            .map(|runtime| {
                (
                    runtime.lease_expires_sequence,
                    runtime.manifest_hash,
                    runtime.restart_count,
                )
            })
            .unwrap_or((sequence, response.payload_hash, 0));
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::MessageAcknowledged,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence,
            manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: Some(response.message_id.clone()),
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: None,
            from_apartment: response.from_apartment.clone(),
            to_apartment: Some(apartment_name),
            channel: Some(response.channel.clone()),
            payload_hash: Some(response.payload_hash),
            artifact_hash: None,
            provenance_hash: None,
            run_id: None,
            run_label: None,
            budget_units: None,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentMailboxMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn agent_mailbox_status(
        &self,
        apartment_name: &str,
    ) -> Result<AgentMailboxStatusResponse, SoracloudError> {
        let apartment_name = parse_agent_apartment_name(apartment_name)?;
        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let sequence = state.next_sequence;
        let runtime = state.apartments.get(&apartment_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in control-plane runtime"
            ))
        })?;
        let messages = runtime
            .mailbox_queue
            .iter()
            .map(AgentMailboxMessageEntry::from_message)
            .collect::<Vec<_>>();
        Ok(AgentMailboxStatusResponse {
            schema_version: state.schema_version,
            apartment_name,
            status: agent_runtime_status_for_sequence(runtime, sequence),
            pending_message_count: u32::try_from(messages.len()).unwrap_or(u32::MAX),
            event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            messages,
        })
    }

    pub(crate) async fn apply_agent_artifact_allow(
        &self,
        request: SignedAgentArtifactAllowRequest,
    ) -> Result<AgentAutonomyMutationResponse, SoracloudError> {
        verify_agent_artifact_allow_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let artifact_hash =
            normalize_agent_hash_like("--artifact-hash", &request.payload.artifact_hash)?;
        let provenance_hash = normalize_optional_agent_hash_like(
            "--provenance-hash",
            request.payload.provenance_hash.as_deref(),
        )?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(runtime, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                    runtime.lease_expires_sequence
                )));
            }
            if !(agent_policy_capability_active(runtime, "governance.audit")
                || agent_policy_capability_active(runtime, "agent.autonomy.allow"))
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not have active `governance.audit` or `agent.autonomy.allow` capability"
                )));
            }
            if runtime
                .artifact_allowlist
                .get(&artifact_hash)
                .is_some_and(|rule| rule.provenance_hash == provenance_hash)
            {
                return Err(SoracloudError::conflict(format!(
                    "artifact `{artifact_hash}` already allowlisted for apartment `{apartment_name}` with the same provenance rule"
                )));
            }
            runtime.artifact_allowlist.insert(
                artifact_hash.clone(),
                AgentArtifactAllowRule {
                    artifact_hash: artifact_hash.clone(),
                    provenance_hash: provenance_hash.clone(),
                    added_sequence: sequence,
                },
            );
            touch_agent_runtime_activity(runtime, sequence);

            AgentAutonomyMutationResponse {
                action: AgentApartmentAction::ArtifactAllowed,
                apartment_name: apartment_name.clone(),
                sequence,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                lease_expires_sequence: runtime.lease_expires_sequence,
                lease_remaining_ticks: agent_lease_remaining_ticks(
                    runtime,
                    sequence.saturating_add(1),
                ),
                manifest_hash: runtime.manifest_hash,
                artifact_hash: artifact_hash.clone(),
                provenance_hash: provenance_hash.clone(),
                run_id: None,
                run_label: None,
                budget_units: None,
                budget_remaining_units: runtime.autonomy_budget_remaining_units,
                allowlist_count: agent_allowlist_count(runtime),
                run_count: agent_run_count(runtime),
                process_generation: runtime.process_generation,
                process_started_sequence: runtime.process_started_sequence,
                last_active_sequence: runtime.last_active_sequence,
                last_checkpoint_sequence: runtime.last_checkpoint_sequence,
                checkpoint_count: runtime.checkpoint_count,
                persistent_state_total_bytes: runtime.persistent_state.total_bytes,
                persistent_state_key_count: agent_persistent_state_key_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };

        let restart_count = state
            .apartments
            .get(&apartment_name)
            .map(|runtime| runtime.restart_count)
            .unwrap_or(0);
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::ArtifactAllowed,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: None,
            from_apartment: None,
            to_apartment: None,
            channel: None,
            payload_hash: None,
            artifact_hash: Some(response.artifact_hash.clone()),
            provenance_hash: response.provenance_hash.clone(),
            run_id: None,
            run_label: None,
            budget_units: None,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentAutonomyMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn apply_agent_autonomy_run(
        &self,
        request: SignedAgentAutonomyRunRequest,
    ) -> Result<AgentAutonomyMutationResponse, SoracloudError> {
        verify_agent_autonomy_run_signature(&request)?;
        let apartment_name = parse_agent_apartment_name(&request.payload.apartment_name)?;
        let artifact_hash =
            normalize_agent_hash_like("--artifact-hash", &request.payload.artifact_hash)?;
        let provenance_hash = normalize_optional_agent_hash_like(
            "--provenance-hash",
            request.payload.provenance_hash.as_deref(),
        )?;
        if request.payload.budget_units == 0 {
            return Err(SoracloudError::bad_request(
                "budget_units must be greater than zero",
            ));
        }
        let run_label = normalize_run_label(&request.payload.run_label)?;
        let signer = request.provenance.signer.to_string();

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();
        let sequence = state.next_sequence;

        let response = {
            let runtime = state.apartments.get_mut(&apartment_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "apartment `{apartment_name}` not found in control-plane runtime"
                ))
            })?;
            if agent_runtime_status_for_sequence(runtime, sequence)
                == AgentRuntimeStatus::LeaseExpired
            {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                    runtime.lease_expires_sequence
                )));
            }
            if !agent_policy_capability_active(runtime, "agent.autonomy.run") {
                return Err(SoracloudError::conflict(format!(
                    "apartment `{apartment_name}` does not have active `agent.autonomy.run` capability"
                )));
            }
            let allow_rule = runtime.artifact_allowlist.get(&artifact_hash).ok_or_else(|| {
                SoracloudError::conflict(format!(
                    "artifact `{artifact_hash}` is not allowlisted for apartment `{apartment_name}`"
                ))
            })?;
            if let Some(expected_provenance) = allow_rule.provenance_hash.as_deref() {
                let provided_provenance = provenance_hash.as_deref().ok_or_else(|| {
                    SoracloudError::conflict(format!(
                        "artifact `{artifact_hash}` requires provenance_hash `{expected_provenance}`"
                    ))
                })?;
                if provided_provenance != expected_provenance {
                    return Err(SoracloudError::conflict(format!(
                        "artifact `{artifact_hash}` provenance mismatch: expected `{expected_provenance}`, got `{provided_provenance}`"
                    )));
                }
            }
            if request.payload.budget_units > runtime.autonomy_budget_remaining_units {
                return Err(SoracloudError::conflict(format!(
                    "requested budget {} exceeds remaining autonomy budget {} for apartment `{apartment_name}`",
                    request.payload.budget_units, runtime.autonomy_budget_remaining_units
                )));
            }
            let run_id = format!("{apartment_name}:autonomy:{sequence}");
            let checkpoint_key = autonomy_checkpoint_key(&apartment_name, &run_id);
            let checkpoint_value_size = autonomy_checkpoint_value_size(
                &artifact_hash,
                provenance_hash.as_deref(),
                &run_label,
                request.payload.budget_units,
            );
            let projected_persistent_total = projected_persistent_state_total_bytes(
                runtime,
                &checkpoint_key,
                checkpoint_value_size,
            )?;
            if projected_persistent_total > runtime.manifest.state_quota_bytes.get() {
                return Err(SoracloudError::conflict(format!(
                    "autonomy checkpoint would exceed apartment `{apartment_name}` state_quota_bytes {}",
                    runtime.manifest.state_quota_bytes
                )));
            }

            runtime.autonomy_budget_remaining_units = runtime
                .autonomy_budget_remaining_units
                .saturating_sub(request.payload.budget_units);
            runtime.autonomy_run_history.push(AgentAutonomyRunRecord {
                run_id: run_id.clone(),
                artifact_hash: artifact_hash.clone(),
                provenance_hash: provenance_hash.clone(),
                budget_units: request.payload.budget_units,
                run_label: run_label.clone(),
                approved_sequence: sequence,
            });
            runtime.persistent_state.total_bytes = projected_persistent_total;
            runtime
                .persistent_state
                .key_sizes
                .insert(checkpoint_key, checkpoint_value_size);
            runtime.last_checkpoint_sequence = Some(sequence);
            runtime.checkpoint_count = runtime.checkpoint_count.saturating_add(1);
            touch_agent_runtime_activity(runtime, sequence);

            AgentAutonomyMutationResponse {
                action: AgentApartmentAction::AutonomyRunApproved,
                apartment_name: apartment_name.clone(),
                sequence,
                status: agent_runtime_status_for_sequence(runtime, sequence.saturating_add(1)),
                lease_expires_sequence: runtime.lease_expires_sequence,
                lease_remaining_ticks: agent_lease_remaining_ticks(
                    runtime,
                    sequence.saturating_add(1),
                ),
                manifest_hash: runtime.manifest_hash,
                artifact_hash: artifact_hash.clone(),
                provenance_hash: provenance_hash.clone(),
                run_id: Some(run_id),
                run_label: Some(run_label.clone()),
                budget_units: Some(request.payload.budget_units),
                budget_remaining_units: runtime.autonomy_budget_remaining_units,
                allowlist_count: agent_allowlist_count(runtime),
                run_count: agent_run_count(runtime),
                process_generation: runtime.process_generation,
                process_started_sequence: runtime.process_started_sequence,
                last_active_sequence: runtime.last_active_sequence,
                last_checkpoint_sequence: runtime.last_checkpoint_sequence,
                checkpoint_count: runtime.checkpoint_count,
                persistent_state_total_bytes: runtime.persistent_state.total_bytes,
                persistent_state_key_count: agent_persistent_state_key_count(runtime),
                audit_event_count: 0,
                signed_by: signer.clone(),
            }
        };

        let restart_count = state
            .apartments
            .get(&apartment_name)
            .map(|runtime| runtime.restart_count)
            .unwrap_or(0);
        state.apartment_audit_log.push(AgentApartmentAuditEvent {
            sequence,
            action: AgentApartmentAction::AutonomyRunApproved,
            apartment_name: apartment_name.clone(),
            status: response.status,
            lease_expires_sequence: response.lease_expires_sequence,
            manifest_hash: response.manifest_hash,
            restart_count,
            signed_by: signer,
            request_id: response.run_id.clone(),
            asset_definition: None,
            amount_nanos: None,
            capability: None,
            reason: None,
            from_apartment: None,
            to_apartment: None,
            channel: None,
            payload_hash: None,
            artifact_hash: Some(response.artifact_hash.clone()),
            provenance_hash: response.provenance_hash.clone(),
            run_id: response.run_id.clone(),
            run_label: response.run_label.clone(),
            budget_units: response.budget_units,
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(AgentAutonomyMutationResponse {
            audit_event_count: u32::try_from(state.apartment_audit_log.len()).unwrap_or(u32::MAX),
            ..response
        })
    }

    pub(crate) async fn agent_autonomy_status(
        &self,
        apartment_name: &str,
    ) -> Result<AgentAutonomyStatusResponse, SoracloudError> {
        let apartment_name = parse_agent_apartment_name(apartment_name)?;
        let state = self.state.read().await;
        ensure_registry_schema(&state)?;
        let sequence = state.next_sequence;
        let runtime = state.apartments.get(&apartment_name).ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in control-plane runtime"
            ))
        })?;

        let recent_runs = runtime
            .autonomy_run_history
            .iter()
            .rev()
            .take(AGENT_AUTONOMY_RECENT_RUN_LIMIT)
            .cloned()
            .collect::<Vec<_>>();
        let allowlist = runtime
            .artifact_allowlist
            .values()
            .map(AgentAutonomyAllowlistEntry::from_rule)
            .collect::<Vec<_>>();

        Ok(AgentAutonomyStatusResponse {
            apartment_name,
            sequence,
            status: agent_runtime_status_for_sequence(runtime, sequence),
            lease_expires_sequence: runtime.lease_expires_sequence,
            lease_remaining_ticks: agent_lease_remaining_ticks(runtime, sequence),
            manifest_hash: runtime.manifest_hash,
            revoked_policy_capability_count: agent_revoked_capability_count(runtime),
            budget_ceiling_units: runtime.autonomy_budget_ceiling_units,
            budget_remaining_units: runtime.autonomy_budget_remaining_units,
            allowlist_count: agent_allowlist_count(runtime),
            run_count: agent_run_count(runtime),
            process_generation: runtime.process_generation,
            process_started_sequence: runtime.process_started_sequence,
            last_active_sequence: runtime.last_active_sequence,
            last_checkpoint_sequence: runtime.last_checkpoint_sequence,
            checkpoint_count: runtime.checkpoint_count,
            persistent_state_total_bytes: runtime.persistent_state.total_bytes,
            persistent_state_key_count: agent_persistent_state_key_count(runtime),
            allowlist,
            recent_runs,
        })
    }

    async fn apply_bundle_mutation(
        &self,
        mode: MutationMode,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        verify_bundle_signature(&request)?;
        request.bundle.validate_for_admission().map_err(|err| {
            SoracloudError::bad_request(format!("deployment bundle failed admission checks: {err}"))
        })?;
        let host_admission = admit_scr_host_bundle(&request.bundle)?;

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let previous_state = state.clone();

        let service_name = request.bundle.service.service_name.to_string();
        let service_version = request.bundle.service.service_version.clone();
        let sequence = state.next_sequence;
        let signer = request.provenance.signer.to_string();
        let previous_version = state
            .services
            .get(&service_name)
            .map(|entry| entry.current_version.clone());
        let action = match (mode, previous_version.is_some()) {
            (MutationMode::Deploy, false) => SoracloudAction::Deploy,
            (MutationMode::Deploy, true) => {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` already deployed; use upgrade instead"
                )));
            }
            (MutationMode::Upgrade, false) => {
                return Err(SoracloudError::not_found(format!(
                    "service `{service_name}` not found; deploy it before upgrading"
                )));
            }
            (MutationMode::Upgrade, true) => SoracloudAction::Upgrade,
        };

        if let Some(existing) = state.services.get(&service_name)
            && existing.current_version == service_version
        {
            return Err(SoracloudError::conflict(format!(
                "service `{service_name}` is already at version `{service_version}`"
            )));
        }

        let container_manifest_hash = request.bundle.container_manifest_hash();
        let service_manifest_hash = request.bundle.service_manifest_hash();

        let mut response_rollout_handle = None;
        let mut response_rollout_stage = None;
        let mut response_rollout_percent = None;
        let revision_count = {
            let entry = state
                .services
                .entry(service_name.clone())
                .or_insert_with(|| RegistryServiceEntry {
                    current_version: service_version.clone(),
                    revisions: Vec::new(),
                    binding_states: BTreeMap::new(),
                    training_jobs: BTreeMap::new(),
                    model_weights: BTreeMap::new(),
                    model_artifacts: BTreeMap::new(),
                    active_rollout: None,
                    last_rollout: None,
                });
            entry.current_version = service_version.clone();
            sync_binding_states(entry, &request.bundle.service.state_bindings);
            let process_generation = entry
                .revisions
                .last()
                .map_or(1, |revision| revision.process_generation.saturating_add(1));
            let revision = RegistryServiceRevision {
                sequence,
                action,
                service_version: service_version.clone(),
                service_manifest_hash,
                container_manifest_hash,
                replicas: request.bundle.service.replicas.get(),
                route_host: request
                    .bundle
                    .service
                    .route
                    .as_ref()
                    .map(|route| route.host.clone()),
                route_path_prefix: request
                    .bundle
                    .service
                    .route
                    .as_ref()
                    .map(|route| route.path_prefix.clone()),
                state_binding_count: u32::try_from(request.bundle.service.state_bindings.len())
                    .unwrap_or(u32::MAX),
                state_bindings: request.bundle.service.state_bindings.clone(),
                allow_model_training: host_admission.allow_model_training,
                runtime: host_admission.runtime,
                allow_wallet_signing: host_admission.allow_wallet_signing,
                allow_state_writes: host_admission.allow_state_writes,
                network: host_admission.network.clone(),
                cpu_millis: host_admission.cpu_millis,
                memory_bytes: host_admission.memory_bytes,
                ephemeral_storage_bytes: host_admission.ephemeral_storage_bytes,
                max_open_files: host_admission.max_open_files,
                max_tasks: host_admission.max_tasks,
                start_grace_secs: host_admission.start_grace_secs,
                stop_grace_secs: host_admission.stop_grace_secs,
                healthcheck_path: host_admission.healthcheck_path.clone(),
                sandbox_profile_hash: host_admission.sandbox_profile_hash,
                process_generation,
                process_started_sequence: sequence,
                signed_by: signer.clone(),
            };
            entry.revisions.push(revision);

            if action == SoracloudAction::Upgrade {
                let canary_percent = request.bundle.service.rollout.canary_percent.min(100);
                let traffic_percent = if canary_percent == 0 {
                    100
                } else {
                    canary_percent
                };
                let rollout_state = RolloutRuntimeState {
                    rollout_handle: rollout_handle(&service_name, sequence),
                    baseline_version: previous_version.clone(),
                    candidate_version: service_version.clone(),
                    canary_percent,
                    traffic_percent,
                    stage: if traffic_percent >= 100 {
                        RolloutStage::Promoted
                    } else {
                        RolloutStage::Canary
                    },
                    health_failures: 0,
                    max_health_failures: request
                        .bundle
                        .service
                        .rollout
                        .automatic_rollback_failures
                        .get(),
                    health_window_secs: request.bundle.service.rollout.health_window_secs.get(),
                    created_sequence: sequence,
                    updated_sequence: sequence,
                };
                response_rollout_handle = Some(rollout_state.rollout_handle.clone());
                response_rollout_stage = Some(rollout_state.stage);
                response_rollout_percent = Some(rollout_state.traffic_percent);
                if rollout_state.stage == RolloutStage::Promoted {
                    entry.active_rollout = None;
                } else {
                    entry.active_rollout = Some(rollout_state.clone());
                }
                entry.last_rollout = Some(rollout_state);
            } else {
                entry.active_rollout = None;
                entry.last_rollout = None;
            }
            u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX)
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action,
            service_name: service_name.clone(),
            from_version: previous_version.clone(),
            to_version: service_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: None,
            state_key: None,
            governance_tx_hash: None,
            rollout_handle: response_rollout_handle.clone(),
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        self.persist_state_or_rollback(&mut state, previous_state)?;

        Ok(MutationResponse {
            action,
            service_name,
            previous_version,
            current_version: service_version,
            sequence,
            service_manifest_hash,
            container_manifest_hash,
            revision_count,
            audit_event_count,
            signed_by: signer,
            rollout_handle: response_rollout_handle,
            rollout_stage: response_rollout_stage,
            rollout_percent: response_rollout_percent,
        })
    }
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

fn sync_binding_states(entry: &mut RegistryServiceEntry, bindings: &[SoraStateBindingV1]) {
    let active_bindings = bindings
        .iter()
        .map(|binding| binding.binding_name.to_string())
        .collect::<BTreeSet<_>>();
    entry
        .binding_states
        .retain(|name, _| active_bindings.contains(name));
    for name in active_bindings {
        entry.binding_states.entry(name).or_default();
    }
}

fn ensure_registry_schema(state: &RegistryState) -> Result<(), SoracloudError> {
    if state.schema_version != REGISTRY_SCHEMA_VERSION {
        return Err(SoracloudError::internal(format!(
            "unsupported registry schema {} (expected {REGISTRY_SCHEMA_VERSION})",
            state.schema_version
        )));
    }
    Ok(())
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

fn normalize_registry_runtime_defaults(state: &mut RegistryState) {
    for runtime in state.apartments.values_mut() {
        normalize_agent_runtime_defaults(runtime);
    }
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
        training_job_id: artifact.training_job_id.clone(),
        weight_artifact_hash: artifact.weight_artifact_hash,
        dataset_ref: artifact.dataset_ref.clone(),
        training_config_hash: artifact.training_config_hash,
        reproducibility_hash: artifact.reproducibility_hash,
        provenance_attestation_hash: artifact.provenance_attestation_hash,
        registered_sequence: artifact.registered_sequence,
        consumed_by_version: artifact.consumed_by_version.clone(),
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
        SoracloudAction::StateMutation => "state_mutation",
        SoracloudAction::FheJobRun => "fhe_job_run",
        SoracloudAction::DecryptionRequest => "decryption_request",
        SoracloudAction::CiphertextQuery => "ciphertext_query",
        SoracloudAction::Rollout => "rollout",
    }
}

fn audit_event_leaf_hash(event: &RegistryAuditEvent) -> Hash {
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
            event.binding_name.as_deref(),
            event.state_key.as_deref(),
            event.governance_tx_hash,
            event.rollout_handle.as_deref(),
            event.policy_name.as_deref(),
            event.policy_snapshot_hash,
            event.jurisdiction_tag.as_deref(),
            event.consent_evidence_hash,
            event.break_glass,
            event.break_glass_reason.as_deref(),
            event.signed_by.as_str(),
        ),
    )))
}

fn audit_anchor_hash(audit_log: &[RegistryAuditEvent], anchor_sequence: u64) -> Hash {
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
    audit_log: &[RegistryAuditEvent],
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
    let payload = norito::to_bytes(&request.bundle).map_err(|err| {
        SoracloudError::internal(format!("failed to encode bundle payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("bundle provenance signature verification failed")
        })?;
    Ok(())
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

fn verify_state_mutation_signature(
    request: &SignedStateMutationRequest,
) -> Result<(), SoracloudError> {
    let payload = norito::to_bytes(&request.payload).map_err(|err| {
        SoracloudError::internal(format!("failed to encode state mutation payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("state mutation provenance signature verification failed")
        })?;
    Ok(())
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
    norito::to_bytes(payload)
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

fn encode_decryption_request_signature_payload(
    payload: &DecryptionRequestPayload,
) -> Result<Vec<u8>, SoracloudError> {
    norito::to_bytes(payload).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode decryption request payload: {err}"
        ))
    })
}

fn encode_ciphertext_query_signature_payload(
    payload: &CiphertextQuerySpecV1,
) -> Result<Vec<u8>, SoracloudError> {
    norito::to_bytes(payload).map_err(|err| {
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
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode agent autonomy run payload: {err}"
        ))
    })
}

pub(crate) async fn handle_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/deploy").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_deploy(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_upgrade(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_rollback(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_rollout(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_state_mutation(request).await {
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

    match app.soracloud_registry.apply_fhe_job_run(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_decryption_request(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_decryption_request(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_ciphertext_query(request).await {
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

    match app
        .soracloud_registry
        .apply_training_job_start(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_training_job_checkpoint(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_training_job_retry(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .training_job_status(&query.service_name, &query.job_id)
        .await
    {
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

    match app
        .soracloud_registry
        .apply_model_weight_register(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_model_weight_promote(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_model_weight_rollback(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .model_weight_status(&query.service_name, &query.model_name)
        .await
    {
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

    match app
        .soracloud_registry
        .apply_model_artifact_register(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .model_artifact_status(&query.service_name, &query.training_job_id)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_agent_deploy(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_agent_lease_renew(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app.soracloud_registry.apply_agent_restart(request).await {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .agent_status(query.apartment_name.as_deref())
        .await
    {
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

    match app
        .soracloud_registry
        .apply_agent_wallet_spend(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_agent_wallet_approve(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_agent_policy_revoke(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_agent_message_send(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .apply_agent_message_ack(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
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

    match app
        .soracloud_registry
        .agent_mailbox_status(&query.apartment_name)
        .await
    {
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

    match app
        .soracloud_registry
        .apply_agent_artifact_allow(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
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

    match app
        .soracloud_registry
        .apply_agent_autonomy_run(request)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
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

    match app
        .soracloud_registry
        .agent_autonomy_status(&query.apartment_name)
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_registry_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<RegistryStatusQuery>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/registry").await {
        return err.into_response();
    }

    let audit_limit = query
        .audit_limit
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_AUDIT_LIMIT)
        .max(1);
    let snapshot = app
        .soracloud_registry
        .snapshot(query.service_name.as_deref(), audit_limit)
        .await;
    JsonBody(snapshot).into_response()
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
    match app
        .soracloud_registry
        .health_compliance_report(
            query.service_name.as_deref(),
            query.jurisdiction_tag.as_deref(),
            limit,
        )
        .await
    {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        fs,
        num::NonZeroU64,
        path::{Path, PathBuf},
    };

    use iroha_crypto::{KeyPair, Signature};
    use iroha_data_model::{
        Encode,
        name::Name,
        soracloud::{
            AgentApartmentManifestV1, CiphertextQueryMetadataLevelV1, CiphertextQuerySpecV1,
            DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheExecutionPolicyV1, FheJobSpecV1,
            FheParamSetV1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SoraContainerManifestV1,
            SoraServiceManifestV1,
        },
    };

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

    fn signed_bundle_request(
        bundle: SoraDeploymentBundleV1,
        key_pair: &KeyPair,
    ) -> SignedBundleRequest {
        let payload = norito::to_bytes(&bundle).expect("encode bundle");
        let signature = Signature::new(key_pair.private_key(), &payload);
        SignedBundleRequest {
            bundle,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
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
        }
    }

    fn signed_state_mutation_request(
        payload: StateMutationRequest,
        key_pair: &KeyPair,
    ) -> SignedStateMutationRequest {
        let encoded = norito::to_bytes(&payload).expect("encode state mutation payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedStateMutationRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
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
        }
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
            asset_definition: "xor#sora".to_owned(),
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
        };
        let encoded = encode_agent_autonomy_run_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.artifact_hash.as_str(),
            payload.provenance_hash.as_deref(),
            payload.budget_units,
            payload.run_label.as_str(),
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
            dataset_ref: "dataset://synthetic/v1".to_owned(),
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
            dataset_ref: "dataset://synthetic/v1".to_owned(),
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

    #[tokio::test]
    async fn deploy_upgrade_rollback_workflow_updates_registry_and_audit_log() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        let deployed = registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");
        assert_eq!(deployed.action, SoracloudAction::Deploy);
        assert_eq!(deployed.current_version, "1.0.0");
        assert_eq!(deployed.audit_event_count, 1);

        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        assert_eq!(upgraded.action, SoracloudAction::Upgrade);
        assert_eq!(upgraded.previous_version.as_deref(), Some("1.0.0"));
        assert_eq!(upgraded.current_version, "1.1.0");
        assert_eq!(upgraded.audit_event_count, 2);

        let rolled_back = registry
            .apply_rollback(signed_rollback_request("web_portal", None, &key_pair))
            .await
            .expect("rollback");
        assert_eq!(rolled_back.action, SoracloudAction::Rollback);
        assert_eq!(rolled_back.current_version, "1.0.0");
        assert_eq!(rolled_back.audit_event_count, 3);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        assert_eq!(snapshot.service_count, 1);
        assert_eq!(snapshot.audit_event_count, 3);
        assert_eq!(snapshot.services[0].current_version, "1.0.0");
        assert_eq!(snapshot.recent_audit_events.len(), 3);

        let state = registry.state.read().await;
        let service = state
            .services
            .get("web_portal")
            .expect("service should remain in registry");
        let generations = service
            .revisions
            .iter()
            .map(|revision| revision.process_generation)
            .collect::<Vec<_>>();
        assert_eq!(generations, vec![1, 2, 3]);
        let started_sequences = service
            .revisions
            .iter()
            .map(|revision| revision.process_started_sequence)
            .collect::<Vec<_>>();
        assert_eq!(
            started_sequences,
            vec![deployed.sequence, upgraded.sequence, rolled_back.sequence]
        );
        let first_hash = service.revisions[0].sandbox_profile_hash;
        assert_eq!(service.revisions[1].sandbox_profile_hash, first_hash);
        assert_eq!(service.revisions[2].sandbox_profile_hash, first_hash);
    }

    #[tokio::test]
    async fn deploy_rejects_invalid_bundle_signature() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let other = KeyPair::random();
        let mut request = signed_bundle_request(fixture_bundle("1.0.0"), &key_pair);
        request.provenance.signature = Signature::new(other.private_key(), b"tampered-payload");

        let err = registry
            .apply_deploy(request)
            .await
            .expect_err("invalid signature must fail");
        assert_eq!(err.kind, SoracloudErrorKind::Unauthorized);
    }

    #[tokio::test]
    async fn deploy_rejects_state_write_capability_mismatch_for_mutable_bindings() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let mut bundle = fixture_bundle("1.0.0");
        bundle.container.capabilities.allow_state_writes = false;
        let first_binding = bundle
            .service
            .state_bindings
            .first_mut()
            .expect("fixture includes at least one binding");
        first_binding.mutability = SoraStateMutabilityV1::ReadWrite;
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();

        let err = registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect_err("deploy must reject mutable bindings when state writes are disabled");
        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("allow_state_writes"),
            "unexpected admission error: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn deploy_rejects_resources_above_scr_caps() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let mut bundle = fixture_bundle("1.0.0");
        bundle.container.resources.cpu_millis =
            std::num::NonZeroU32::new(SCR_HOST_MAX_CPU_MILLIS.saturating_add(1))
                .expect("non-zero CPU");
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();

        let err = registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect_err("deploy must reject resources beyond SCR caps");
        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("resources.cpu_millis exceeds SCR cap"),
            "unexpected resource-admission error: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn state_mutation_tracks_binding_usage_for_current_revision() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let first = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/session/user-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(128),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-1"),
            },
            &key_pair,
        );
        let first_result = registry
            .apply_state_mutation(first)
            .await
            .expect("first upsert");
        assert_eq!(first_result.binding_total_bytes, 128);
        assert_eq!(first_result.binding_key_count, 1);
        assert_eq!(first_result.audit_event_count, 2);

        let second = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/session/user-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(64),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-2"),
            },
            &key_pair,
        );
        let second_result = registry
            .apply_state_mutation(second)
            .await
            .expect("overwrite upsert");
        assert_eq!(second_result.binding_total_bytes, 64);
        assert_eq!(second_result.binding_key_count, 1);
        assert_eq!(second_result.audit_event_count, 3);
    }

    #[tokio::test]
    async fn state_mutation_enforces_append_only_and_prefix_rules() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let first = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "patient_records".to_string(),
                key: "/state/health/patient-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(256),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-3"),
            },
            &key_pair,
        );
        registry
            .apply_state_mutation(first)
            .await
            .expect("append-only first write");

        let overwrite = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "patient_records".to_string(),
                key: "/state/health/patient-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(512),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-4"),
            },
            &key_pair,
        );
        let overwrite_err = registry
            .apply_state_mutation(overwrite)
            .await
            .expect_err("append-only overwrite must fail");
        assert_eq!(overwrite_err.kind, SoracloudErrorKind::Conflict);

        let wrong_prefix = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/other/key".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(32),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-5"),
            },
            &key_pair,
        );
        let prefix_err = registry
            .apply_state_mutation(wrong_prefix)
            .await
            .expect_err("wrong prefix must fail");
        assert_eq!(prefix_err.kind, SoracloudErrorKind::Conflict);
    }

    #[tokio::test]
    async fn fhe_job_run_tracks_ciphertext_binding_usage_and_audit_log() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let job = fixture_fhe_job_spec();
        let expected_operation = job.operation;
        let expected_output_key = job.output_state_key.clone();
        let expected_output_bytes = job.deterministic_output_payload_bytes();
        let expected_commitment = job.deterministic_output_commitment();
        let response = registry
            .apply_fhe_job_run(signed_fhe_job_run_request(
                FheJobRunPayload {
                    service_name: "web_portal".to_owned(),
                    binding_name: "patient_records".to_owned(),
                    job,
                    policy: fixture_fhe_execution_policy(),
                    param_set: fixture_fhe_param_set(),
                    governance_tx_hash: Hash::new(b"fhe-governance-1"),
                },
                &key_pair,
            ))
            .await
            .expect("fhe job run");

        assert_eq!(response.action, SoracloudAction::FheJobRun);
        assert_eq!(response.service_name, "web_portal");
        assert_eq!(response.binding_name, "patient_records");
        assert_eq!(response.operation, expected_operation);
        assert_eq!(response.output_state_key, expected_output_key);
        assert_eq!(response.output_payload_bytes, expected_output_bytes);
        assert_eq!(response.output_commitment, expected_commitment);
        assert_eq!(response.binding_total_bytes, expected_output_bytes);
        assert_eq!(response.binding_key_count, 1);
        assert_eq!(response.current_version, "1.0.0");
        assert_eq!(response.audit_event_count, 2);

        let snapshot = registry.snapshot(Some("web_portal"), 4).await;
        assert_eq!(snapshot.audit_event_count, 2);
        assert_eq!(snapshot.recent_audit_events.len(), 2);
        assert_eq!(
            snapshot.recent_audit_events[0].action,
            SoracloudAction::FheJobRun
        );
    }

    #[tokio::test]
    async fn fhe_job_run_rejects_bindings_that_are_not_fhe_ciphertext() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let err = registry
            .apply_fhe_job_run(signed_fhe_job_run_request(
                FheJobRunPayload {
                    service_name: "web_portal".to_owned(),
                    binding_name: "session_store".to_owned(),
                    job: fixture_fhe_job_spec(),
                    policy: fixture_fhe_execution_policy(),
                    param_set: fixture_fhe_param_set(),
                    governance_tx_hash: Hash::new(b"fhe-governance-2"),
                },
                &key_pair,
            ))
            .await
            .expect_err("non-fhe binding must be rejected");

        assert_eq!(err.kind, SoracloudErrorKind::Conflict);
        assert!(
            err.message.contains("not configured for FHE ciphertexts"),
            "unexpected non-fhe binding error: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn fhe_job_run_rejects_policy_name_mismatch() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut job = fixture_fhe_job_spec();
        job.policy_name = "wrong_policy".parse().expect("valid policy name");
        let err = registry
            .apply_fhe_job_run(signed_fhe_job_run_request(
                FheJobRunPayload {
                    service_name: "web_portal".to_owned(),
                    binding_name: "patient_records".to_owned(),
                    job,
                    policy: fixture_fhe_execution_policy(),
                    param_set: fixture_fhe_param_set(),
                    governance_tx_hash: Hash::new(b"fhe-governance-3"),
                },
                &key_pair,
            ))
            .await
            .expect_err("policy mismatch must be rejected");

        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("policy_name"),
            "unexpected policy mismatch error: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn decryption_request_records_audit_event_for_private_binding() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let request = fixture_decryption_request();
        let policy = fixture_decryption_authority_policy();
        let expected_policy_name = request.policy_name.clone();
        let expected_binding = request.binding_name.clone();
        let expected_request_id = request.request_id.clone();
        let expected_state_key = request.state_key.clone();
        let expected_governance_hash = request.governance_tx_hash;
        let expected_jurisdiction = request.jurisdiction_tag.clone();
        let expected_policy_snapshot_hash = Hash::new(Encode::encode(&policy));
        let expected_consent_hash = request.consent_evidence_hash;

        let response = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy,
                    request,
                },
                &key_pair,
            ))
            .await
            .expect("decryption request");

        assert_eq!(response.action, SoracloudAction::DecryptionRequest);
        assert_eq!(response.service_name, "web_portal");
        assert_eq!(response.policy_name, expected_policy_name);
        assert_eq!(response.binding_name, expected_binding);
        assert_eq!(response.request_id, expected_request_id);
        assert_eq!(response.state_key, expected_state_key);
        assert_eq!(response.jurisdiction_tag, expected_jurisdiction);
        assert_eq!(response.policy_snapshot_hash, expected_policy_snapshot_hash);
        assert_eq!(response.consent_evidence_hash, expected_consent_hash);
        assert_eq!(response.governance_tx_hash, expected_governance_hash);
        assert_eq!(response.audit_event_count, 2);

        let snapshot = registry.snapshot(Some("web_portal"), 4).await;
        assert_eq!(snapshot.audit_event_count, 2);
        assert_eq!(snapshot.recent_audit_events.len(), 2);
        let audit = &snapshot.recent_audit_events[0];
        assert_eq!(audit.action, SoracloudAction::DecryptionRequest);
        assert_eq!(audit.jurisdiction_tag.as_deref(), Some("us_hipaa"));
        assert_eq!(audit.policy_name.as_deref(), Some("phi_threshold_policy"));
        assert_eq!(
            audit.policy_snapshot_hash,
            Some(expected_policy_snapshot_hash)
        );
        assert_eq!(audit.consent_evidence_hash, expected_consent_hash);
        assert_eq!(audit.break_glass, Some(false));
    }

    #[tokio::test]
    async fn decryption_request_rejects_break_glass_when_policy_disallows_it() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut request = fixture_decryption_request();
        request.break_glass = true;
        request.break_glass_reason = Some("critical-care override".to_string());
        let err = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: fixture_decryption_authority_policy(),
                    request,
                },
                &key_pair,
            ))
            .await
            .expect_err("break-glass must fail when policy disallows it");

        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("break_glass"),
            "unexpected break-glass rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn decryption_request_rejects_missing_consent_evidence_when_policy_requires_it() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut request = fixture_decryption_request();
        request.consent_evidence_hash = None;
        let err = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: fixture_decryption_authority_policy(),
                    request,
                },
                &key_pair,
            ))
            .await
            .expect_err("consent evidence is required for non-break-glass request");

        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("consent_evidence_hash"),
            "unexpected consent-evidence rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn decryption_request_rejects_jurisdiction_mismatch() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut request = fixture_decryption_request();
        request.jurisdiction_tag = "eu_gdpr".to_string();
        let err = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: fixture_decryption_authority_policy(),
                    request,
                },
                &key_pair,
            ))
            .await
            .expect_err("jurisdiction mismatch should fail policy admission");

        assert_eq!(err.kind, SoracloudErrorKind::BadRequest);
        assert!(
            err.message.contains("jurisdiction_tag"),
            "unexpected jurisdiction rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn decryption_request_rejects_state_key_outside_binding_prefix() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut request = fixture_decryption_request();
        request.state_key = "/state/session/user-1".to_owned();
        let err = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: fixture_decryption_authority_policy(),
                    request,
                },
                &key_pair,
            ))
            .await
            .expect_err("state key outside binding prefix must be rejected");

        assert_eq!(err.kind, SoracloudErrorKind::Conflict);
        assert!(
            err.message.contains("outside binding prefix"),
            "unexpected binding-prefix rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn health_privacy_matrix_rejects_unauthorized_decryption_signatures() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut signed = signed_decryption_request(
            DecryptionRequestPayload {
                service_name: "web_portal".to_owned(),
                policy: fixture_decryption_authority_policy(),
                request: fixture_decryption_request(),
            },
            &key_pair,
        );
        signed.payload.request.justification = "tampered-justification".to_string();

        let err = registry
            .apply_decryption_request(signed)
            .await
            .expect_err("tampered signed payload must fail signature verification");
        assert_eq!(err.kind, SoracloudErrorKind::Unauthorized);
        assert!(
            err.message.contains("signature verification failed"),
            "unexpected unauthorized-signature rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn health_privacy_matrix_enforces_declared_binding_scope() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut request = fixture_decryption_request();
        request.binding_name = "unknown_binding".parse().expect("valid binding name");
        let err = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: fixture_decryption_authority_policy(),
                    request,
                },
                &key_pair,
            ))
            .await
            .expect_err("undeclared binding should fail least-privilege enforcement");
        assert_eq!(err.kind, SoracloudErrorKind::NotFound);
        assert!(
            err.message.contains("is not declared"),
            "unexpected undeclared-binding rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn health_privacy_matrix_reports_evidence_completeness_gaps() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut policy = fixture_decryption_authority_policy();
        policy.allow_break_glass = true;

        let mut standard_request = fixture_decryption_request();
        standard_request.request_id = "decrypt-req-matrix-1".to_string();
        standard_request.state_key = "/state/health/matrix-patient-1".to_string();
        standard_request.governance_tx_hash = Hash::new(b"health-matrix-gov-1");
        registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: policy.clone(),
                    request: standard_request,
                },
                &key_pair,
            ))
            .await
            .expect("standard request");

        let mut break_glass_request = fixture_decryption_request();
        break_glass_request.request_id = "decrypt-req-matrix-2".to_string();
        break_glass_request.state_key = "/state/health/matrix-patient-2".to_string();
        break_glass_request.break_glass = true;
        break_glass_request.break_glass_reason = Some("critical care override".to_string());
        break_glass_request.consent_evidence_hash = None;
        break_glass_request.governance_tx_hash = Hash::new(b"health-matrix-gov-2");
        registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy,
                    request: break_glass_request,
                },
                &key_pair,
            ))
            .await
            .expect("break-glass request");

        let report = registry
            .health_compliance_report(Some("web_portal"), Some("us_hipaa"), 20)
            .await
            .expect("health compliance report");
        assert_eq!(report.total_access_events, 2);
        assert_eq!(report.break_glass_events, 1);
        assert_eq!(report.consent_evidence_present_events, 1);
        assert_eq!(report.consent_evidence_coverage_bps, 5_000);
    }

    #[tokio::test]
    async fn health_compliance_report_summarizes_access_logs_policy_history_and_attestations() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let policy_v1 = fixture_decryption_authority_policy();
        let policy_hash_v1 = Hash::new(Encode::encode(&policy_v1));

        let mut policy_v2 = policy_v1.clone();
        policy_v2.allow_break_glass = true;
        policy_v2.approver_quorum = 3u16.try_into().expect("non-zero approver quorum");
        let policy_hash_v2 = Hash::new(Encode::encode(&policy_v2));

        let mut policy_eu = policy_v2.clone();
        policy_eu.jurisdiction_tag = "eu_gdpr".to_string();
        policy_eu.require_consent_evidence = false;
        let policy_hash_eu = Hash::new(Encode::encode(&policy_eu));

        let request_v1 = fixture_decryption_request();
        registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: policy_v1,
                    request: request_v1,
                },
                &key_pair,
            ))
            .await
            .expect("first decryption request");

        let mut request_v2_break_glass = fixture_decryption_request();
        request_v2_break_glass.request_id = "decrypt-req-0002".to_string();
        request_v2_break_glass.state_key = "/state/health/patient-2".to_string();
        request_v2_break_glass.break_glass = true;
        request_v2_break_glass.break_glass_reason = Some("emergency trauma review".to_string());
        request_v2_break_glass.consent_evidence_hash = None;
        request_v2_break_glass.governance_tx_hash = Hash::new(b"health-gov-2");
        registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: policy_v2.clone(),
                    request: request_v2_break_glass,
                },
                &key_pair,
            ))
            .await
            .expect("second decryption request");

        let mut request_v2 = fixture_decryption_request();
        request_v2.request_id = "decrypt-req-0003".to_string();
        request_v2.state_key = "/state/health/patient-3".to_string();
        request_v2.governance_tx_hash = Hash::new(b"health-gov-3");
        let response_v2 = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: policy_v2,
                    request: request_v2,
                },
                &key_pair,
            ))
            .await
            .expect("third decryption request");

        let mut request_eu = fixture_decryption_request();
        request_eu.request_id = "decrypt-req-0004".to_string();
        request_eu.state_key = "/state/health/patient-4".to_string();
        request_eu.jurisdiction_tag = "eu_gdpr".to_string();
        request_eu.consent_evidence_hash = None;
        request_eu.governance_tx_hash = Hash::new(b"health-gov-4");
        let response_eu = registry
            .apply_decryption_request(signed_decryption_request(
                DecryptionRequestPayload {
                    service_name: "web_portal".to_owned(),
                    policy: policy_eu,
                    request: request_eu,
                },
                &key_pair,
            ))
            .await
            .expect("fourth decryption request");

        let report = registry
            .health_compliance_report(Some("web_portal"), None, 20)
            .await
            .expect("health compliance report");

        assert_eq!(report.schema_version, HEALTH_COMPLIANCE_REPORT_VERSION_V1);
        assert_eq!(report.service_name.as_deref(), Some("web_portal"));
        assert_eq!(report.total_access_events, 4);
        assert_eq!(report.break_glass_events, 1);
        assert_eq!(report.non_break_glass_events, 3);
        assert_eq!(report.consent_evidence_present_events, 2);
        assert_eq!(report.consent_evidence_coverage_bps, 5_000);
        assert_eq!(report.recent_access_events.len(), 4);
        assert_eq!(
            report.recent_access_events[0].sequence,
            response_eu.sequence
        );
        assert_eq!(
            report.recent_access_events[1].sequence,
            response_v2.sequence
        );

        let us_stats = report
            .jurisdiction_stats
            .iter()
            .find(|entry| entry.jurisdiction_tag == "us_hipaa")
            .expect("us_hipaa stats");
        assert_eq!(us_stats.access_event_count, 3);
        assert_eq!(us_stats.break_glass_event_count, 1);
        let eu_stats = report
            .jurisdiction_stats
            .iter()
            .find(|entry| entry.jurisdiction_tag == "eu_gdpr")
            .expect("eu_gdpr stats");
        assert_eq!(eu_stats.access_event_count, 1);
        assert_eq!(eu_stats.break_glass_event_count, 0);

        assert!(
            report
                .data_flow_attestations
                .iter()
                .any(|entry| entry.service_name == "web_portal"
                    && entry.binding_name == "patient_records"),
            "expected patient_records data-flow attestation"
        );
        assert!(
            report
                .data_flow_attestations
                .iter()
                .any(|entry| entry.service_name == "web_portal"
                    && entry.binding_name == "session_store"),
            "expected session_store data-flow attestation"
        );

        let policy_v2_history = report
            .policy_diff_history
            .iter()
            .find(|entry| entry.policy_snapshot_hash == policy_hash_v2)
            .expect("policy v2 history");
        assert_eq!(policy_v2_history.event_count, 2);
        assert!(
            report
                .policy_diff_history
                .iter()
                .any(|entry| entry.policy_snapshot_hash == policy_hash_v1),
            "expected baseline policy history"
        );
        assert!(
            report
                .policy_diff_history
                .iter()
                .any(|entry| entry.policy_snapshot_hash == policy_hash_eu),
            "expected EU policy history"
        );

        let us_filtered = registry
            .health_compliance_report(Some("web_portal"), Some("us_hipaa"), 1)
            .await
            .expect("filtered health compliance report");
        assert_eq!(us_filtered.total_access_events, 3);
        assert_eq!(us_filtered.break_glass_events, 1);
        assert_eq!(us_filtered.non_break_glass_events, 2);
        assert_eq!(us_filtered.consent_evidence_present_events, 2);
        assert_eq!(us_filtered.recent_access_events.len(), 1);
        assert_eq!(
            us_filtered.recent_access_events[0].sequence,
            response_v2.sequence
        );
        assert_eq!(us_filtered.jurisdiction_stats.len(), 1);
        assert_eq!(
            us_filtered.jurisdiction_stats[0].jurisdiction_tag,
            "us_hipaa"
        );
        assert_eq!(us_filtered.policy_diff_history.len(), 1);
    }

    #[tokio::test]
    async fn ciphertext_query_returns_minimal_metadata_with_inclusion_proofs() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");
        registry
            .apply_fhe_job_run(signed_fhe_job_run_request(
                FheJobRunPayload {
                    service_name: "web_portal".to_owned(),
                    binding_name: "patient_records".to_owned(),
                    job: fixture_fhe_job_spec(),
                    policy: fixture_fhe_execution_policy(),
                    param_set: fixture_fhe_param_set(),
                    governance_tx_hash: Hash::new(b"fhe-governance-query-1"),
                },
                &key_pair,
            ))
            .await
            .expect("fhe job run");

        let response = registry
            .apply_ciphertext_query(signed_ciphertext_query_request(
                fixture_ciphertext_query_spec(),
                &key_pair,
            ))
            .await
            .expect("ciphertext query");
        assert_eq!(response.action, SoracloudAction::CiphertextQuery);
        assert_eq!(response.signed_by, key_pair.public_key().to_string());

        let payload = response.response;
        payload.validate().expect("query response should validate");
        assert_eq!(
            payload.metadata_level,
            CiphertextQueryMetadataLevelV1::Minimal
        );
        assert_eq!(payload.result_count, 1);
        assert!(!payload.truncated);
        assert_eq!(payload.results.len(), 1);
        let row = &payload.results[0];
        assert!(
            row.state_key.is_none(),
            "minimal projection must hide state key"
        );
        assert_eq!(row.encryption, SoraStateEncryptionV1::FheCiphertext);
        assert!(
            row.proof
                .as_ref()
                .is_some_and(|proof| proof.proof_scheme == CIPHERTEXT_QUERY_PROOF_SCHEME_V1),
            "inclusion proof should be attached for proof-enabled queries"
        );
    }

    #[tokio::test]
    async fn ciphertext_query_rejects_plaintext_bindings() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let mut bundle = fixture_bundle("1.0.0");
        let session_binding = bundle
            .service
            .state_bindings
            .iter_mut()
            .find(|binding| binding.binding_name.as_ref() == "session_store")
            .expect("session binding should exist");
        session_binding.encryption = SoraStateEncryptionV1::Plaintext;
        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("deploy");

        let err = registry
            .apply_ciphertext_query(signed_ciphertext_query_request(
                CiphertextQuerySpecV1 {
                    schema_version: fixture_ciphertext_query_spec().schema_version,
                    service_name: "web_portal".parse().expect("valid name"),
                    binding_name: "session_store".parse().expect("valid name"),
                    state_key_prefix: "/state/session".to_owned(),
                    max_results: std::num::NonZeroU16::new(16).expect("nonzero"),
                    metadata_level: CiphertextQueryMetadataLevelV1::Minimal,
                    include_proof: true,
                },
                &key_pair,
            ))
            .await
            .expect_err("plaintext binding should not be queryable via ciphertext interface");
        assert_eq!(err.kind, SoracloudErrorKind::Conflict);
        assert!(
            err.message.contains("plaintext"),
            "unexpected plaintext-binding rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn ciphertext_query_rejects_prefix_outside_binding_scope() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let mut query = fixture_ciphertext_query_spec();
        query.state_key_prefix = "/state/session".to_owned();
        let err = registry
            .apply_ciphertext_query(signed_ciphertext_query_request(query, &key_pair))
            .await
            .expect_err("prefix outside binding scope should be rejected");
        assert_eq!(err.kind, SoracloudErrorKind::Conflict);
        assert!(
            err.message.contains("outside binding prefix"),
            "unexpected binding-scope rejection: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn ciphertext_query_response_is_deterministic_across_registries() {
        let key_pair = KeyPair::random();
        let registry_a = Registry::default();
        let registry_b = Registry::default();

        for registry in [&registry_a, &registry_b] {
            registry
                .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
                .await
                .expect("deploy");
            registry
                .apply_fhe_job_run(signed_fhe_job_run_request(
                    FheJobRunPayload {
                        service_name: "web_portal".to_owned(),
                        binding_name: "patient_records".to_owned(),
                        job: fixture_fhe_job_spec(),
                        policy: fixture_fhe_execution_policy(),
                        param_set: fixture_fhe_param_set(),
                        governance_tx_hash: Hash::new(b"fhe-governance-query-parity"),
                    },
                    &key_pair,
                ))
                .await
                .expect("fhe job run");
        }

        let response_a = registry_a
            .apply_ciphertext_query(signed_ciphertext_query_request(
                fixture_ciphertext_query_spec(),
                &key_pair,
            ))
            .await
            .expect("query a");
        let response_b = registry_b
            .apply_ciphertext_query(signed_ciphertext_query_request(
                fixture_ciphertext_query_spec(),
                &key_pair,
            ))
            .await
            .expect("query b");

        assert_eq!(response_a.response, response_b.response);
        assert_eq!(response_a.signed_by, response_b.signed_by);
    }

    #[tokio::test]
    async fn rollout_canary_advances_and_closes_on_full_promotion() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        let handle = upgraded
            .rollout_handle
            .clone()
            .expect("upgrade should provide rollout handle");
        assert_eq!(upgraded.rollout_stage, Some(RolloutStage::Canary));
        assert_eq!(upgraded.rollout_percent, Some(20));

        let canary = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                true,
                Some(50),
                b"rollout-canary-1",
                &key_pair,
            ))
            .await
            .expect("canary advance");
        assert_eq!(canary.action, SoracloudAction::Rollout);
        assert_eq!(canary.stage, RolloutStage::Canary);
        assert_eq!(canary.traffic_percent, 50);

        let promoted = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                true,
                None,
                b"rollout-promote-2",
                &key_pair,
            ))
            .await
            .expect("promotion");
        assert_eq!(promoted.action, SoracloudAction::Rollout);
        assert_eq!(promoted.stage, RolloutStage::Promoted);
        assert_eq!(promoted.traffic_percent, 100);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        let service = snapshot.services.first().expect("service exists");
        assert!(
            service.active_rollout.is_none(),
            "promoted rollout should close"
        );
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::Promoted)
        );
    }

    #[tokio::test]
    async fn rollout_auto_rolls_back_after_health_failures() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");
        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        let handle = upgraded
            .rollout_handle
            .clone()
            .expect("upgrade should provide rollout handle");

        for index in 0..2 {
            let response = registry
                .apply_rollout(signed_rollout_request(
                    "web_portal",
                    &handle,
                    false,
                    None,
                    format!("rollout-fail-{index}").as_bytes(),
                    &key_pair,
                ))
                .await
                .expect("pre-threshold health failure");
            assert_eq!(response.action, SoracloudAction::Rollout);
            assert_eq!(response.stage, RolloutStage::Canary);
        }

        let rollback = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                false,
                None,
                b"rollout-fail-terminal",
                &key_pair,
            ))
            .await
            .expect("terminal health failure should rollback");
        assert_eq!(rollback.action, SoracloudAction::Rollback);
        assert_eq!(rollback.stage, RolloutStage::RolledBack);
        assert_eq!(rollback.current_version, "1.0.0");
        assert_eq!(rollback.traffic_percent, 0);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        let service = snapshot.services.first().expect("service exists");
        assert_eq!(service.current_version, "1.0.0");
        assert!(service.active_rollout.is_none());
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::RolledBack)
        );
    }

    #[tokio::test]
    async fn agent_autonomy_runtime_enforces_allowlist_budget_and_revocation() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        let deployed = registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest: fixture_agent_manifest(),
                    lease_ticks: 32,
                    autonomy_budget_units: Some(500),
                },
                &key_pair,
            ))
            .await
            .expect("agent deploy");
        assert_eq!(deployed.action, AgentApartmentAction::Deploy);
        assert_eq!(deployed.budget_remaining_units, 500);

        let allow = registry
            .apply_agent_artifact_allow(signed_agent_artifact_allow_request(
                AgentArtifactAllowPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                },
                &key_pair,
            ))
            .await
            .expect("allow artifact");
        assert_eq!(allow.action, AgentApartmentAction::ArtifactAllowed);
        assert_eq!(allow.allowlist_count, 1);
        assert_eq!(allow.budget_remaining_units, 500);

        let run = registry
            .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                AgentAutonomyRunPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                    budget_units: 120,
                    run_label: "nightly-train-step-1".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("autonomy run");
        assert_eq!(run.action, AgentApartmentAction::AutonomyRunApproved);
        assert_eq!(run.run_count, 1);
        assert_eq!(run.budget_remaining_units, 380);
        assert!(
            run.run_id
                .as_deref()
                .is_some_and(|run_id| run_id.contains(":autonomy:"))
        );

        let status = registry
            .agent_autonomy_status("ops_agent")
            .await
            .expect("autonomy status");
        assert_eq!(status.allowlist_count, 1);
        assert_eq!(status.run_count, 1);
        assert_eq!(status.budget_remaining_units, 380);
        assert_eq!(status.recent_runs.len(), 1);
        assert_eq!(status.recent_runs[0].run_label, "nightly-train-step-1");

        let provenance_mismatch = registry
            .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                AgentAutonomyRunPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:WRONG0001#01".to_owned()),
                    budget_units: 1,
                    run_label: "mismatch".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect_err("provenance mismatch must fail");
        assert_eq!(provenance_mismatch.kind, SoracloudErrorKind::Conflict);
        assert!(
            provenance_mismatch.message.contains("provenance mismatch"),
            "unexpected mismatch error: {}",
            provenance_mismatch.message
        );

        let revoke = registry
            .apply_agent_policy_revoke(signed_agent_policy_revoke_request(
                AgentPolicyRevokePayload {
                    apartment_name: "ops_agent".to_owned(),
                    capability: "agent.autonomy.run".to_owned(),
                    reason: Some("manual-review".to_owned()),
                },
                &key_pair,
            ))
            .await
            .expect("policy revoke");
        assert_eq!(revoke.action, AgentApartmentAction::PolicyRevoked);
        assert_eq!(revoke.revoked_policy_capability_count, 1);

        let revoked_run = registry
            .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                AgentAutonomyRunPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                    budget_units: 1,
                    run_label: "revoked".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect_err("run with revoked capability must fail");
        assert_eq!(revoked_run.kind, SoracloudErrorKind::Conflict);
        assert!(
            revoked_run.message.contains("agent.autonomy.run"),
            "unexpected revoked capability error: {}",
            revoked_run.message
        );
    }

    #[tokio::test]
    async fn agent_autonomy_runtime_rejects_actions_after_lease_expiry() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest: fixture_agent_manifest(),
                    lease_ticks: 1,
                    autonomy_budget_units: Some(100),
                },
                &key_pair,
            ))
            .await
            .expect("agent deploy");

        let expired_allow = registry
            .apply_agent_artifact_allow(signed_agent_artifact_allow_request(
                AgentArtifactAllowPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: None,
                },
                &key_pair,
            ))
            .await
            .expect_err("allow should fail after lease expiry");
        assert_eq!(expired_allow.kind, SoracloudErrorKind::Conflict);
        assert!(
            expired_allow.message.contains("lease expired"),
            "unexpected lease-expiry error: {}",
            expired_allow.message
        );

        let status = registry
            .agent_autonomy_status("ops_agent")
            .await
            .expect("status should still resolve");
        assert_eq!(status.status, AgentRuntimeStatus::LeaseExpired);
        assert_eq!(status.lease_remaining_ticks, 0);
    }

    #[tokio::test]
    async fn agent_runtime_lease_renew_restart_and_status_flow() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        let deployed = registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest: fixture_agent_manifest_with_capabilities("ops_agent", &[]),
                    lease_ticks: 1,
                    autonomy_budget_units: Some(250),
                },
                &key_pair,
            ))
            .await
            .expect("agent deploy");
        assert_eq!(deployed.action, AgentApartmentAction::Deploy);

        let expired_restart = registry
            .apply_agent_restart(signed_agent_restart_request(
                AgentRestartPayload {
                    apartment_name: "ops_agent".to_owned(),
                    reason: "expired-lease".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect_err("restart should fail while lease is expired");
        assert_eq!(expired_restart.kind, SoracloudErrorKind::Conflict);
        assert!(
            expired_restart.message.contains("lease expired"),
            "unexpected lease-expiry error: {}",
            expired_restart.message
        );

        let renewed = registry
            .apply_agent_lease_renew(signed_agent_lease_renew_request(
                AgentLeaseRenewPayload {
                    apartment_name: "ops_agent".to_owned(),
                    lease_ticks: 20,
                },
                &key_pair,
            ))
            .await
            .expect("lease renew");
        assert_eq!(renewed.action, AgentApartmentAction::LeaseRenew);
        assert_eq!(renewed.status, AgentRuntimeStatus::Running);
        assert!(renewed.lease_remaining_ticks > 0);

        let restarted = registry
            .apply_agent_restart(signed_agent_restart_request(
                AgentRestartPayload {
                    apartment_name: "ops_agent".to_owned(),
                    reason: "manual-restart".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("restart");
        assert_eq!(restarted.action, AgentApartmentAction::Restart);
        assert_eq!(restarted.restart_count, 1);
        assert_eq!(
            restarted.last_restart_reason.as_deref(),
            Some("manual-restart")
        );
        assert_eq!(restarted.process_generation, 2);
        assert_eq!(restarted.process_started_sequence, restarted.sequence);
        assert_eq!(restarted.last_active_sequence, restarted.sequence);
        assert_eq!(restarted.checkpoint_count, 0);
        assert_eq!(restarted.persistent_state_total_bytes, 0);
        assert_eq!(restarted.persistent_state_key_count, 0);

        let status = registry
            .agent_status(Some("ops_agent"))
            .await
            .expect("agent status");
        assert_eq!(status.apartment_count, 1);
        assert_eq!(status.apartments.len(), 1);
        let apartment = &status.apartments[0];
        assert_eq!(apartment.apartment_name, "ops_agent");
        assert_eq!(apartment.status, AgentRuntimeStatus::Running);
        assert_eq!(apartment.restart_count, 1);
        assert!(
            apartment
                .last_restart_reason
                .as_deref()
                .is_some_and(|reason| reason == "manual-restart")
        );
        assert_eq!(apartment.process_generation, 2);
        assert_eq!(
            apartment.process_started_sequence,
            restarted.process_started_sequence
        );
        assert_eq!(apartment.checkpoint_count, 0);
        assert_eq!(apartment.persistent_state_total_bytes, 0);
        assert_eq!(apartment.persistent_state_key_count, 0);
        assert!(
            apartment.lease_remaining_ticks > 0,
            "lease should be active after renewal"
        );
    }

    #[tokio::test]
    async fn agent_runtime_wallet_and_mailbox_policy_flow() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        let sender_manifest =
            fixture_agent_manifest_with_capabilities("ops_agent", &["agent.mailbox.send"]);
        let mut recipient_manifest =
            fixture_agent_manifest_with_capabilities("worker_agent", &["agent.mailbox.receive"]);
        recipient_manifest
            .policy_capabilities
            .retain(|capability| capability.as_ref() != "agent.mailbox.send");
        recipient_manifest.validate().expect("recipient manifest");

        registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest: sender_manifest,
                    lease_ticks: 64,
                    autonomy_budget_units: Some(500),
                },
                &key_pair,
            ))
            .await
            .expect("sender deploy");
        registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest: recipient_manifest,
                    lease_ticks: 64,
                    autonomy_budget_units: Some(500),
                },
                &key_pair,
            ))
            .await
            .expect("recipient deploy");

        let wallet_request = registry
            .apply_agent_wallet_spend(signed_agent_wallet_spend_request(
                AgentWalletSpendPayload {
                    apartment_name: "ops_agent".to_owned(),
                    asset_definition: "xor#sora".to_owned(),
                    amount_nanos: 1_000_000,
                },
                &key_pair,
            ))
            .await
            .expect("wallet spend request");
        assert_eq!(
            wallet_request.action,
            AgentApartmentAction::WalletSpendRequested
        );
        assert_eq!(wallet_request.pending_request_count, 1);
        let request_id = wallet_request
            .request_id
            .clone()
            .expect("wallet request id must be present");

        let wallet_approve = registry
            .apply_agent_wallet_approve(signed_agent_wallet_approve_request(
                AgentWalletApprovePayload {
                    apartment_name: "ops_agent".to_owned(),
                    request_id: request_id.clone(),
                },
                &key_pair,
            ))
            .await
            .expect("wallet approve");
        assert_eq!(
            wallet_approve.action,
            AgentApartmentAction::WalletSpendApproved
        );
        assert_eq!(wallet_approve.pending_request_count, 0);
        assert_eq!(
            wallet_approve.request_id.as_deref(),
            Some(request_id.as_str())
        );

        let message_send = registry
            .apply_agent_message_send(signed_agent_message_send_request(
                AgentMessageSendPayload {
                    from_apartment: "ops_agent".to_owned(),
                    to_apartment: "worker_agent".to_owned(),
                    channel: "ops.sync".to_owned(),
                    payload: "rotate-key-42".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("message send");
        assert_eq!(message_send.action, AgentApartmentAction::MessageEnqueued);
        assert_eq!(message_send.pending_message_count, 1);
        let message_id = message_send.message_id.clone();

        let mailbox_status = registry
            .agent_mailbox_status("worker_agent")
            .await
            .expect("mailbox status");
        assert_eq!(mailbox_status.pending_message_count, 1);
        assert_eq!(mailbox_status.messages.len(), 1);
        assert_eq!(mailbox_status.messages[0].message_id, message_id);

        let message_ack = registry
            .apply_agent_message_ack(signed_agent_message_ack_request(
                AgentMessageAckPayload {
                    apartment_name: "worker_agent".to_owned(),
                    message_id: message_id.clone(),
                },
                &key_pair,
            ))
            .await
            .expect("message ack");
        assert_eq!(
            message_ack.action,
            AgentApartmentAction::MessageAcknowledged
        );
        assert_eq!(message_ack.pending_message_count, 0);

        let mailbox_status_empty = registry
            .agent_mailbox_status("worker_agent")
            .await
            .expect("mailbox status after ack");
        assert_eq!(mailbox_status_empty.pending_message_count, 0);
        assert!(mailbox_status_empty.messages.is_empty());

        registry
            .apply_agent_policy_revoke(signed_agent_policy_revoke_request(
                AgentPolicyRevokePayload {
                    apartment_name: "worker_agent".to_owned(),
                    capability: "agent.mailbox.receive".to_owned(),
                    reason: Some("maintenance-window".to_owned()),
                },
                &key_pair,
            ))
            .await
            .expect("revoke recipient mailbox capability");

        let rejected_send = registry
            .apply_agent_message_send(signed_agent_message_send_request(
                AgentMessageSendPayload {
                    from_apartment: "ops_agent".to_owned(),
                    to_apartment: "worker_agent".to_owned(),
                    channel: "ops.sync".to_owned(),
                    payload: "rotate-key-43".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect_err("message send should fail after recipient capability revocation");
        assert_eq!(rejected_send.kind, SoracloudErrorKind::Conflict);
        assert!(
            rejected_send.message.contains("agent.mailbox.receive"),
            "unexpected revoked-capability error: {}",
            rejected_send.message
        );
    }

    #[tokio::test]
    async fn training_job_lifecycle_tracks_checkpoints_retries_and_completion() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", true);
        let service_name = bundle.service.service_name.to_string();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("training-capable service deploy");

        let started = registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 6,
                    checkpoint_interval_steps: 2,
                    max_retries: 2,
                    step_compute_units: 10,
                    compute_budget_units: 200,
                    storage_budget_bytes: 500,
                },
                &key_pair,
            ))
            .await
            .expect("training start");
        assert_eq!(started.action, TrainingJobAction::Start);
        assert_eq!(started.status, TrainingJobStatus::Running);
        assert_eq!(started.training_event_count, 1);
        assert_eq!(started.compute_consumed_units, 0);
        assert_eq!(started.storage_consumed_bytes, 0);

        let checkpoint_one = registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 2,
                    checkpoint_size_bytes: 100,
                    metrics_hash: Hash::new(b"checkpoint-1"),
                },
                &key_pair,
            ))
            .await
            .expect("checkpoint one");
        assert_eq!(checkpoint_one.status, TrainingJobStatus::Running);
        assert_eq!(checkpoint_one.completed_steps, 2);
        assert_eq!(checkpoint_one.checkpoint_count, 1);
        assert_eq!(checkpoint_one.compute_consumed_units, 40);
        assert_eq!(checkpoint_one.storage_consumed_bytes, 100);
        assert_eq!(checkpoint_one.training_event_count, 2);

        let retry = registry
            .apply_training_job_retry(signed_training_job_retry_request(
                TrainingJobRetryPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    reason: "gradient divergence at shard 3".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("retry request");
        assert_eq!(retry.action, TrainingJobAction::Retry);
        assert_eq!(retry.status, TrainingJobStatus::RetryPending);
        assert_eq!(retry.retry_count, 1);
        assert_eq!(
            retry.last_failure_reason.as_deref(),
            Some("gradient divergence at shard 3")
        );
        assert_eq!(retry.training_event_count, 3);

        let checkpoint_two = registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 4,
                    checkpoint_size_bytes: 120,
                    metrics_hash: Hash::new(b"checkpoint-2"),
                },
                &key_pair,
            ))
            .await
            .expect("checkpoint two");
        assert_eq!(checkpoint_two.status, TrainingJobStatus::Running);
        assert_eq!(checkpoint_two.completed_steps, 4);
        assert_eq!(checkpoint_two.checkpoint_count, 2);
        assert_eq!(checkpoint_two.compute_consumed_units, 80);
        assert_eq!(checkpoint_two.storage_consumed_bytes, 220);
        assert_eq!(checkpoint_two.retry_count, 1);
        assert!(checkpoint_two.last_failure_reason.is_none());
        assert_eq!(checkpoint_two.training_event_count, 4);

        let completed = registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 6,
                    checkpoint_size_bytes: 140,
                    metrics_hash: Hash::new(b"checkpoint-3"),
                },
                &key_pair,
            ))
            .await
            .expect("checkpoint three");
        assert_eq!(completed.status, TrainingJobStatus::Completed);
        assert_eq!(completed.completed_steps, 6);
        assert_eq!(completed.checkpoint_count, 3);
        assert_eq!(completed.compute_consumed_units, 120);
        assert_eq!(completed.storage_consumed_bytes, 360);
        assert_eq!(completed.training_event_count, 5);

        let status = registry
            .training_job_status(&service_name, "job-1")
            .await
            .expect("training status");
        assert_eq!(status.schema_version, TRAINING_JOB_STATUS_SCHEMA_VERSION_V1);
        assert_eq!(status.job.status, TrainingJobStatus::Completed);
        assert_eq!(status.job.completed_steps, 6);
        assert_eq!(status.job.compute_consumed_units, 120);
        assert_eq!(status.job.compute_remaining_units, 80);
        assert_eq!(status.job.storage_consumed_bytes, 360);
        assert_eq!(status.job.storage_remaining_bytes, 140);
        assert_eq!(status.job.retry_count, 1);
    }

    #[tokio::test]
    async fn training_job_start_rejects_services_without_training_capability() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", false);
        let service_name = bundle.service.service_name.to_string();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("service deploy");

        let error = registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name,
                    model_name: "foundation_model".to_owned(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 4,
                    checkpoint_interval_steps: 2,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 100,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect_err("training start should reject non-training service");
        assert_eq!(error.kind, SoracloudErrorKind::Conflict);
        assert!(
            error.message.contains("does not allow model training"),
            "unexpected rejection: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn model_weight_lifecycle_supports_lineage_promotion_and_rollback() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", true);
        let service_name = bundle.service.service_name.to_string();
        let model_name = "foundation_model".to_owned();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("training-capable service deploy");

        registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 2,
                    checkpoint_interval_steps: 1,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 80,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect("training job 1 start");
        registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 2,
                    checkpoint_size_bytes: 64,
                    metrics_hash: Hash::new(b"job-1-metrics"),
                },
                &key_pair,
            ))
            .await
            .expect("training job 1 complete");

        registry
            .apply_model_artifact_register(signed_model_artifact_register_request(
                ModelArtifactRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    training_job_id: "job-1".to_owned(),
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect("artifact register v1");

        let register_v1 = registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    weight_version: "v1".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    parent_version: None,
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect("register v1");
        assert_eq!(register_v1.action, ModelWeightAction::Register);
        assert_eq!(register_v1.target_version, "v1");
        assert!(register_v1.current_version.is_none());
        assert_eq!(register_v1.version_count, 1);

        let promote_v1 = registry
            .apply_model_weight_promote(signed_model_weight_promote_request(
                ModelWeightPromotePayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    weight_version: "v1".to_owned(),
                    gate_approved: true,
                    gate_report_hash: Hash::new(b"gate-v1"),
                },
                &key_pair,
            ))
            .await
            .expect("promote v1");
        assert_eq!(promote_v1.action, ModelWeightAction::Promote);
        assert_eq!(promote_v1.current_version.as_deref(), Some("v1"));

        registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    job_id: "job-2".to_owned(),
                    worker_group_size: 2,
                    target_steps: 2,
                    checkpoint_interval_steps: 1,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 80,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect("training job 2 start");
        registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-2".to_owned(),
                    completed_step: 2,
                    checkpoint_size_bytes: 64,
                    metrics_hash: Hash::new(b"job-2-metrics"),
                },
                &key_pair,
            ))
            .await
            .expect("training job 2 complete");

        registry
            .apply_model_artifact_register(signed_model_artifact_register_request(
                ModelArtifactRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    training_job_id: "job-2".to_owned(),
                    weight_artifact_hash: Hash::new(b"weight-v2"),
                    dataset_ref: "sorafs://datasets/health/v2".to_owned(),
                    training_config_hash: Hash::new(b"config-v2"),
                    reproducibility_hash: Hash::new(b"repro-v2"),
                    provenance_attestation_hash: Hash::new(b"attest-v2"),
                },
                &key_pair,
            ))
            .await
            .expect("artifact register v2");

        let register_v2 = registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    weight_version: "v2".to_owned(),
                    training_job_id: "job-2".to_owned(),
                    parent_version: Some("v1".to_owned()),
                    weight_artifact_hash: Hash::new(b"weight-v2"),
                    dataset_ref: "sorafs://datasets/health/v2".to_owned(),
                    training_config_hash: Hash::new(b"config-v2"),
                    reproducibility_hash: Hash::new(b"repro-v2"),
                    provenance_attestation_hash: Hash::new(b"attest-v2"),
                },
                &key_pair,
            ))
            .await
            .expect("register v2");
        assert_eq!(register_v2.version_count, 2);
        assert_eq!(register_v2.parent_version.as_deref(), Some("v1"));
        assert_eq!(register_v2.current_version.as_deref(), Some("v1"));

        let promote_v2 = registry
            .apply_model_weight_promote(signed_model_weight_promote_request(
                ModelWeightPromotePayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    weight_version: "v2".to_owned(),
                    gate_approved: true,
                    gate_report_hash: Hash::new(b"gate-v2"),
                },
                &key_pair,
            ))
            .await
            .expect("promote v2");
        assert_eq!(promote_v2.current_version.as_deref(), Some("v2"));

        let rollback = registry
            .apply_model_weight_rollback(signed_model_weight_rollback_request(
                ModelWeightRollbackPayload {
                    service_name: service_name.clone(),
                    model_name: model_name.clone(),
                    target_version: "v1".to_owned(),
                    reason: "regression in validation shard".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("rollback to v1");
        assert_eq!(rollback.action, ModelWeightAction::Rollback);
        assert_eq!(rollback.current_version.as_deref(), Some("v1"));
        assert_eq!(rollback.version_count, 2);

        let status = registry
            .model_weight_status(&service_name, &model_name)
            .await
            .expect("model status");
        assert_eq!(status.schema_version, MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1);
        assert_eq!(status.model.current_version.as_deref(), Some("v1"));
        assert_eq!(status.model.version_count, 2);
        assert_eq!(status.model.versions.len(), 2);
        assert_eq!(status.model.versions[0].weight_version, "v1");
        assert_eq!(status.model.versions[1].weight_version, "v2");
        assert_eq!(
            status.model.versions[1].parent_version.as_deref(),
            Some("v1")
        );
    }

    #[tokio::test]
    async fn model_weight_register_rejects_non_completed_training_jobs() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", true);
        let service_name = bundle.service.service_name.to_string();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("training-capable service deploy");

        registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 2,
                    checkpoint_interval_steps: 1,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 80,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect("training start");

        let error = registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name,
                    model_name: "foundation_model".to_owned(),
                    weight_version: "v1".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    parent_version: None,
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect_err("register should fail for non-completed training job");
        assert_eq!(error.kind, SoracloudErrorKind::Conflict);
        assert!(
            error.message.contains("not completed"),
            "unexpected error: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn secure_artifact_pipeline_requires_artifact_registration_before_weight_register() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", true);
        let service_name = bundle.service.service_name.to_string();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("training-capable service deploy");

        registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 2,
                    checkpoint_interval_steps: 1,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 80,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect("training start");
        registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 2,
                    checkpoint_size_bytes: 64,
                    metrics_hash: Hash::new(b"job-1-metrics"),
                },
                &key_pair,
            ))
            .await
            .expect("training complete");

        let error = registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name,
                    model_name: "foundation_model".to_owned(),
                    weight_version: "v1".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    parent_version: None,
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect_err("register should fail when artifact metadata is missing");
        assert_eq!(error.kind, SoracloudErrorKind::NotFound);
        assert!(
            error.message.contains("artifact metadata"),
            "unexpected error: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn secure_artifact_pipeline_enforces_metadata_match_and_consumption() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let bundle = fixture_bundle_with_training("2026.03.0", true);
        let service_name = bundle.service.service_name.to_string();

        registry
            .apply_deploy(signed_bundle_request(bundle, &key_pair))
            .await
            .expect("training-capable service deploy");

        registry
            .apply_training_job_start(signed_training_job_start_request(
                TrainingJobStartPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    job_id: "job-1".to_owned(),
                    worker_group_size: 2,
                    target_steps: 2,
                    checkpoint_interval_steps: 1,
                    max_retries: 1,
                    step_compute_units: 10,
                    compute_budget_units: 80,
                    storage_budget_bytes: 256,
                },
                &key_pair,
            ))
            .await
            .expect("training start");
        registry
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name: service_name.clone(),
                    job_id: "job-1".to_owned(),
                    completed_step: 2,
                    checkpoint_size_bytes: 64,
                    metrics_hash: Hash::new(b"job-1-metrics"),
                },
                &key_pair,
            ))
            .await
            .expect("training complete");

        registry
            .apply_model_artifact_register(signed_model_artifact_register_request(
                ModelArtifactRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect("artifact register");

        let mismatch = registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    weight_version: "v1".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    parent_version: None,
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v2".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect_err("register should fail on artifact metadata mismatch");
        assert_eq!(mismatch.kind, SoracloudErrorKind::Conflict);
        assert!(
            mismatch.message.contains("dataset_ref mismatch"),
            "unexpected mismatch error: {}",
            mismatch.message
        );

        registry
            .apply_model_weight_register(signed_model_weight_register_request(
                ModelWeightRegisterPayload {
                    service_name: service_name.clone(),
                    model_name: "foundation_model".to_owned(),
                    weight_version: "v1".to_owned(),
                    training_job_id: "job-1".to_owned(),
                    parent_version: None,
                    weight_artifact_hash: Hash::new(b"weight-v1"),
                    dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                    training_config_hash: Hash::new(b"config-v1"),
                    reproducibility_hash: Hash::new(b"repro-v1"),
                    provenance_attestation_hash: Hash::new(b"attest-v1"),
                },
                &key_pair,
            ))
            .await
            .expect("register weight");

        let artifact_status = registry
            .model_artifact_status(&service_name, "job-1")
            .await
            .expect("artifact status");
        assert_eq!(
            artifact_status.schema_version,
            MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1
        );
        assert_eq!(
            artifact_status.artifact.consumed_by_version.as_deref(),
            Some("v1")
        );
    }

    #[tokio::test]
    async fn training_benchmark_throughput_and_checkpoint_recovery_are_deterministic() {
        let key_pair = KeyPair::random();
        let temp_dir = tempfile::tempdir().expect("temporary persistence directory");
        let persistence_path = temp_dir.path().join("registry_state.to");
        let service_name = "web_portal".to_owned();

        {
            let registry = Registry::with_persistence(persistence_path.clone());
            let bundle = fixture_bundle_with_training("2026.03.0", true);
            registry
                .apply_deploy(signed_bundle_request(bundle, &key_pair))
                .await
                .expect("training-capable service deploy");
            registry
                .apply_training_job_start(signed_training_job_start_request(
                    TrainingJobStartPayload {
                        service_name: service_name.clone(),
                        model_name: "foundation_model".to_owned(),
                        job_id: "bench-job".to_owned(),
                        worker_group_size: 2,
                        target_steps: 10,
                        checkpoint_interval_steps: 5,
                        max_retries: 2,
                        step_compute_units: 10,
                        compute_budget_units: 400,
                        storage_budget_bytes: 1024,
                    },
                    &key_pair,
                ))
                .await
                .expect("training start");
            registry
                .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                    TrainingJobCheckpointPayload {
                        service_name: service_name.clone(),
                        job_id: "bench-job".to_owned(),
                        completed_step: 5,
                        checkpoint_size_bytes: 200,
                        metrics_hash: Hash::new(b"bench-metrics-1"),
                    },
                    &key_pair,
                ))
                .await
                .expect("first checkpoint");
        }

        let recovered = Registry::with_persistence(persistence_path);
        let recovered_status = recovered
            .training_job_status(&service_name, "bench-job")
            .await
            .expect("training status after reload");
        assert_eq!(recovered_status.job.completed_steps, 5);
        assert_eq!(recovered_status.job.checkpoint_count, 1);

        recovered
            .apply_training_job_retry(signed_training_job_retry_request(
                TrainingJobRetryPayload {
                    service_name: service_name.clone(),
                    job_id: "bench-job".to_owned(),
                    reason: "benchmark recovery resume".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("retry after reload");

        let completed = recovered
            .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                TrainingJobCheckpointPayload {
                    service_name,
                    job_id: "bench-job".to_owned(),
                    completed_step: 10,
                    checkpoint_size_bytes: 220,
                    metrics_hash: Hash::new(b"bench-metrics-2"),
                },
                &key_pair,
            ))
            .await
            .expect("second checkpoint");
        assert_eq!(completed.status, TrainingJobStatus::Completed);
        assert_eq!(completed.checkpoint_count, 2);
        assert_eq!(completed.retry_count, 1);
        assert_eq!(completed.training_event_count, 4);
        assert_eq!(completed.completed_steps, 10);

        // Deterministic benchmark ratio: steps/event = 10/4.
        let throughput_numerator = u64::from(completed.completed_steps);
        let throughput_denominator = u64::from(completed.training_event_count);
        assert_eq!((throughput_numerator, throughput_denominator), (10, 4));
    }

    #[tokio::test]
    async fn promotion_policy_benchmark_gate_enforcement_is_deterministic() {
        async fn run_once() -> (String, Option<String>, u32) {
            let registry = Registry::default();
            let key_pair = KeyPair::random();
            let bundle = fixture_bundle_with_training("2026.03.0", true);
            let service_name = bundle.service.service_name.to_string();

            registry
                .apply_deploy(signed_bundle_request(bundle, &key_pair))
                .await
                .expect("training-capable service deploy");
            registry
                .apply_training_job_start(signed_training_job_start_request(
                    TrainingJobStartPayload {
                        service_name: service_name.clone(),
                        model_name: "foundation_model".to_owned(),
                        job_id: "job-1".to_owned(),
                        worker_group_size: 2,
                        target_steps: 2,
                        checkpoint_interval_steps: 1,
                        max_retries: 1,
                        step_compute_units: 10,
                        compute_budget_units: 80,
                        storage_budget_bytes: 256,
                    },
                    &key_pair,
                ))
                .await
                .expect("training start");
            registry
                .apply_training_job_checkpoint(signed_training_job_checkpoint_request(
                    TrainingJobCheckpointPayload {
                        service_name: service_name.clone(),
                        job_id: "job-1".to_owned(),
                        completed_step: 2,
                        checkpoint_size_bytes: 64,
                        metrics_hash: Hash::new(b"job-1-metrics"),
                    },
                    &key_pair,
                ))
                .await
                .expect("training complete");
            registry
                .apply_model_artifact_register(signed_model_artifact_register_request(
                    ModelArtifactRegisterPayload {
                        service_name: service_name.clone(),
                        model_name: "foundation_model".to_owned(),
                        training_job_id: "job-1".to_owned(),
                        weight_artifact_hash: Hash::new(b"weight-v1"),
                        dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                        training_config_hash: Hash::new(b"config-v1"),
                        reproducibility_hash: Hash::new(b"repro-v1"),
                        provenance_attestation_hash: Hash::new(b"attest-v1"),
                    },
                    &key_pair,
                ))
                .await
                .expect("artifact register");
            registry
                .apply_model_weight_register(signed_model_weight_register_request(
                    ModelWeightRegisterPayload {
                        service_name: service_name.clone(),
                        model_name: "foundation_model".to_owned(),
                        weight_version: "v1".to_owned(),
                        training_job_id: "job-1".to_owned(),
                        parent_version: None,
                        weight_artifact_hash: Hash::new(b"weight-v1"),
                        dataset_ref: "sorafs://datasets/health/v1".to_owned(),
                        training_config_hash: Hash::new(b"config-v1"),
                        reproducibility_hash: Hash::new(b"repro-v1"),
                        provenance_attestation_hash: Hash::new(b"attest-v1"),
                    },
                    &key_pair,
                ))
                .await
                .expect("weight register");

            let rejected = registry
                .apply_model_weight_promote(signed_model_weight_promote_request(
                    ModelWeightPromotePayload {
                        service_name: service_name.clone(),
                        model_name: "foundation_model".to_owned(),
                        weight_version: "v1".to_owned(),
                        gate_approved: false,
                        gate_report_hash: Hash::new(b"gate-v1-rejected"),
                    },
                    &key_pair,
                ))
                .await
                .expect_err("promotion must reject when gate is false");
            let promoted = registry
                .apply_model_weight_promote(signed_model_weight_promote_request(
                    ModelWeightPromotePayload {
                        service_name,
                        model_name: "foundation_model".to_owned(),
                        weight_version: "v1".to_owned(),
                        gate_approved: true,
                        gate_report_hash: Hash::new(b"gate-v1-approved"),
                    },
                    &key_pair,
                ))
                .await
                .expect("promotion should pass with approved gate");
            (
                rejected.message,
                promoted.current_version,
                promoted.model_event_count,
            )
        }

        let first = run_once().await;
        let second = run_once().await;
        assert_eq!(first, second);
        assert!(
            first.0.contains("gate is not approved"),
            "unexpected rejection reason: {}",
            first.0
        );
        assert_eq!(first.1.as_deref(), Some("v1"));
        assert_eq!(first.2, 2);
    }

    #[tokio::test]
    async fn agent_autonomy_checkpoint_rejects_when_state_quota_is_exceeded() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let mut manifest = fixture_agent_manifest_with_capabilities("ops_agent", &[]);
        manifest.state_quota_bytes = NonZeroU64::new(80).expect("non-zero quota");

        registry
            .apply_agent_deploy(signed_agent_deploy_request(
                AgentDeployPayload {
                    manifest,
                    lease_ticks: 64,
                    autonomy_budget_units: Some(200),
                },
                &key_pair,
            ))
            .await
            .expect("agent deploy");
        registry
            .apply_agent_artifact_allow(signed_agent_artifact_allow_request(
                AgentArtifactAllowPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                },
                &key_pair,
            ))
            .await
            .expect("artifact allow");

        let oversized = registry
            .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                AgentAutonomyRunPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                    budget_units: 10,
                    run_label: "x".repeat(96),
                },
                &key_pair,
            ))
            .await
            .expect_err("autonomy run should fail when state quota would be exceeded");
        assert_eq!(oversized.kind, SoracloudErrorKind::Conflict);
        assert!(
            oversized.message.contains("state_quota_bytes"),
            "unexpected state-quota rejection: {}",
            oversized.message
        );

        let status = registry
            .agent_autonomy_status("ops_agent")
            .await
            .expect("autonomy status");
        assert_eq!(status.run_count, 0);
        assert_eq!(status.checkpoint_count, 0);
        assert_eq!(status.persistent_state_total_bytes, 0);
        assert_eq!(status.persistent_state_key_count, 0);
        assert_eq!(status.budget_remaining_units, 200);
    }

    #[tokio::test]
    async fn agent_runtime_state_persists_across_registry_reload() {
        let key_pair = KeyPair::random();
        let temp_dir = tempfile::tempdir().expect("temporary persistence directory");
        let persistence_path = temp_dir.path().join("registry_state.to");

        let sender_manifest =
            fixture_agent_manifest_with_capabilities("ops_agent", &["agent.mailbox.send"]);
        let mut recipient_manifest =
            fixture_agent_manifest_with_capabilities("worker_agent", &["agent.mailbox.receive"]);
        recipient_manifest
            .policy_capabilities
            .retain(|capability| capability.as_ref() != "agent.mailbox.send");
        recipient_manifest.validate().expect("recipient manifest");

        let wallet_request_id;
        let mailbox_message_id;
        let first_run_persistent_total;
        {
            let registry = Registry::with_persistence(persistence_path.clone());

            registry
                .apply_agent_deploy(signed_agent_deploy_request(
                    AgentDeployPayload {
                        manifest: sender_manifest,
                        lease_ticks: 128,
                        autonomy_budget_units: Some(500),
                    },
                    &key_pair,
                ))
                .await
                .expect("sender deploy");
            registry
                .apply_agent_deploy(signed_agent_deploy_request(
                    AgentDeployPayload {
                        manifest: recipient_manifest,
                        lease_ticks: 128,
                        autonomy_budget_units: Some(250),
                    },
                    &key_pair,
                ))
                .await
                .expect("recipient deploy");

            registry
                .apply_agent_artifact_allow(signed_agent_artifact_allow_request(
                    AgentArtifactAllowPayload {
                        apartment_name: "ops_agent".to_owned(),
                        artifact_hash: "hash:ABCD0123#01".to_owned(),
                        provenance_hash: Some("hash:PROV0001#01".to_owned()),
                    },
                    &key_pair,
                ))
                .await
                .expect("artifact allow");

            let first_run = registry
                .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                    AgentAutonomyRunPayload {
                        apartment_name: "ops_agent".to_owned(),
                        artifact_hash: "hash:ABCD0123#01".to_owned(),
                        provenance_hash: Some("hash:PROV0001#01".to_owned()),
                        budget_units: 120,
                        run_label: "before-reload".to_owned(),
                    },
                    &key_pair,
                ))
                .await
                .expect("autonomy run before reload");
            assert_eq!(first_run.run_count, 1);
            assert_eq!(first_run.budget_remaining_units, 380);
            assert_eq!(first_run.process_generation, 1);
            assert_eq!(first_run.checkpoint_count, 1);
            assert!(
                first_run.persistent_state_total_bytes > 0,
                "autonomy checkpoint should consume persistent state bytes"
            );
            first_run_persistent_total = first_run.persistent_state_total_bytes;
            assert_eq!(first_run.persistent_state_key_count, 1);
            assert_eq!(first_run.last_checkpoint_sequence, Some(first_run.sequence));

            let wallet_request = registry
                .apply_agent_wallet_spend(signed_agent_wallet_spend_request(
                    AgentWalletSpendPayload {
                        apartment_name: "ops_agent".to_owned(),
                        asset_definition: "xor#sora".to_owned(),
                        amount_nanos: 1_000_000,
                    },
                    &key_pair,
                ))
                .await
                .expect("wallet request before reload");
            assert_eq!(wallet_request.pending_request_count, 1);
            wallet_request_id = wallet_request
                .request_id
                .expect("wallet request id before reload");

            let mailbox_send = registry
                .apply_agent_message_send(signed_agent_message_send_request(
                    AgentMessageSendPayload {
                        from_apartment: "ops_agent".to_owned(),
                        to_apartment: "worker_agent".to_owned(),
                        channel: "ops.sync".to_owned(),
                        payload: "rotate-key-42".to_owned(),
                    },
                    &key_pair,
                ))
                .await
                .expect("mailbox send before reload");
            assert_eq!(mailbox_send.pending_message_count, 1);
            mailbox_message_id = mailbox_send.message_id;
        }

        let recovered = Registry::with_persistence(persistence_path);
        let recovered_autonomy = recovered
            .agent_autonomy_status("ops_agent")
            .await
            .expect("autonomy status after reload");
        assert_eq!(recovered_autonomy.run_count, 1);
        assert_eq!(recovered_autonomy.budget_remaining_units, 380);

        let recovered_status = recovered
            .agent_status(Some("ops_agent"))
            .await
            .expect("agent status after reload");
        assert_eq!(recovered_status.apartment_count, 1);
        assert_eq!(recovered_status.apartments.len(), 1);
        let apartment = &recovered_status.apartments[0];
        assert_eq!(apartment.apartment_name, "ops_agent");
        assert_eq!(apartment.pending_wallet_request_count, 1);
        assert_eq!(apartment.process_generation, 1);
        assert_eq!(apartment.checkpoint_count, 1);
        assert!(
            apartment.persistent_state_total_bytes > 0,
            "persistent checkpoint bytes should survive reload"
        );
        assert_eq!(apartment.persistent_state_key_count, 1);
        assert!(apartment.last_checkpoint_sequence.is_some());
        assert!(
            apartment.lease_remaining_ticks > 0,
            "lease should remain active after reload"
        );

        let recovered_mailbox = recovered
            .agent_mailbox_status("worker_agent")
            .await
            .expect("mailbox status after reload");
        assert_eq!(recovered_mailbox.pending_message_count, 1);
        assert_eq!(recovered_mailbox.messages.len(), 1);
        assert_eq!(recovered_mailbox.messages[0].message_id, mailbox_message_id);

        let approved = recovered
            .apply_agent_wallet_approve(signed_agent_wallet_approve_request(
                AgentWalletApprovePayload {
                    apartment_name: "ops_agent".to_owned(),
                    request_id: wallet_request_id,
                },
                &key_pair,
            ))
            .await
            .expect("wallet approve after reload");
        assert_eq!(approved.pending_request_count, 0);

        let acknowledged = recovered
            .apply_agent_message_ack(signed_agent_message_ack_request(
                AgentMessageAckPayload {
                    apartment_name: "worker_agent".to_owned(),
                    message_id: mailbox_message_id,
                },
                &key_pair,
            ))
            .await
            .expect("mailbox ack after reload");
        assert_eq!(acknowledged.pending_message_count, 0);

        let second_run = recovered
            .apply_agent_autonomy_run(signed_agent_autonomy_run_request(
                AgentAutonomyRunPayload {
                    apartment_name: "ops_agent".to_owned(),
                    artifact_hash: "hash:ABCD0123#01".to_owned(),
                    provenance_hash: Some("hash:PROV0001#01".to_owned()),
                    budget_units: 50,
                    run_label: "after-reload".to_owned(),
                },
                &key_pair,
            ))
            .await
            .expect("autonomy run after reload");
        assert_eq!(second_run.run_count, 2);
        assert_eq!(second_run.budget_remaining_units, 330);
        assert_eq!(second_run.process_generation, 1);
        assert_eq!(second_run.checkpoint_count, 2);
        assert!(
            second_run.persistent_state_total_bytes > first_run_persistent_total,
            "second checkpoint should increase persistent state usage"
        );
        assert_eq!(second_run.persistent_state_key_count, 2);
        assert_eq!(
            second_run.last_checkpoint_sequence,
            Some(second_run.sequence)
        );

        let mailbox_after_ack = recovered
            .agent_mailbox_status("worker_agent")
            .await
            .expect("mailbox status after ack");
        assert_eq!(mailbox_after_ack.pending_message_count, 0);
        assert!(mailbox_after_ack.messages.is_empty());
    }
}
