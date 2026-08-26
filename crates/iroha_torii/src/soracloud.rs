//! App-facing Soracloud control-plane shim.
//!
//! This module provides a deterministic control-plane surface for
//! `deploy`/`upgrade`/`rollback` workflows plus SCR host-admission snapshots.
//! Requests must carry signed payloads so admission can verify manifest
//! provenance before mutating authoritative control-plane state. Every
//! top-level POST request schema is strict and excludes inline account
//! authority/private-key fields; caller identity comes only from the verified
//! HTTP signature or witness headers.
//! Node-local autonomy summaries are read through the same 16 MiB V1 ceiling
//! enforced by the runtime writer and from a stable direct file description.
use crate::{JsonBody, NoritoJson, NoritoQuery, SharedAppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{
    Engine as _,
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
    },
};
use futures_util::{StreamExt as _, TryStreamExt as _, stream};
use iroha_core::soracloud_runtime::{
    HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS, HF_GENERATED_AGENT_LEASE_BLOCKS,
    SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_MAX_BYTES_V1,
    SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_JOURNAL_VERSION_V1,
    SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_MAX_SUBMISSION_ATTEMPTS_V1,
    SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentExecutionRequest,
    SoracloudLocalReadKind, SoracloudPrivateUploadedModelExecutionJournalPhaseV1,
    SoracloudPrivateUploadedModelExecutionJournalV1,
    SoracloudPrivateUploadedModelExecutionRequestV1,
    SoracloudPrivateUploadedModelExecutionSubmissionProgressV1, SoracloudRuntimeExecutionError,
    SoracloudRuntimeExecutionErrorKind, SoracloudRuntimeHfSourcePlan,
    SoracloudRuntimeHfSourceStatus, authoritative_soracloud_sequence,
    build_soracloud_hf_generated_agent_manifest, build_soracloud_hf_generated_service_bundle,
    latest_soracloud_sequence, soracloud_hf_generated_source_binding,
    validate_finalized_soracloud_uploaded_model_release,
    validate_soracloud_apartment_autonomy_execution_summary_v1,
};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
#[cfg(test)]
use iroha_data_model::soracloud::SoraServiceExactCurrentRevisionPreconditionV1;
use iroha_data_model::{
    Encode,
    account::AccountId,
    asset::AssetDefinitionId,
    isi::{self, InstructionBox},
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, CIPHERTEXT_QUERY_PROOF_VERSION_V1,
        CIPHERTEXT_QUERY_RESPONSE_VERSION_V1, CiphertextInclusionProofV1,
        CiphertextQueryMetadataLevelV1, CiphertextQueryResponseV1, CiphertextQueryResultItemV1,
        CiphertextQuerySpecV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheJobSpecV1,
        SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
        SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1,
        SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
        SORACLOUD_PRIVATE_OUTPUT_MIN_RETENTION_SECS_V1, SecretEnvelopeEncryptionV1,
        SecretEnvelopeV1, SoraAgentApartmentActionV1, SoraAgentApartmentAuditEventV1,
        SoraAgentApartmentRecordV1, SoraAgentArtifactAllowRuleV1, SoraAgentAutonomyRunRecordV1,
        SoraAgentMailboxMessageV1, SoraAgentRuntimeStatusV1, SoraAppInfraAuditEventV1,
        SoraAppInfraManifestV1, SoraAppInfraMutationPreconditionV1, SoraAppInfraStateV1,
        SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerRuntimeV1,
        SoraDecryptionRequestRecordV1, SoraDeploymentBundleV1, SoraHfBackendFamilyV1,
        SoraHfModelFormatV1, SoraHfPlacementRecordV1, SoraHfResourceProfileV1,
        SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1, SoraHfSharedLeaseMemberStatusV1,
        SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1,
        SoraHfSourceRecordV1, SoraHfSourceStatusV1, SoraLeaseVolumeBindingV1,
        SoraModelArtifactActionV1, SoraModelArtifactAuditEventV1, SoraModelArtifactRecordV1,
        SoraModelHostCapabilityRecordV1, SoraModelProvenanceKindV1, SoraModelRegistryV1,
        SoraModelWeightActionV1, SoraModelWeightAuditEventV1, SoraModelWeightVersionRecordV1,
        SoraNetworkPolicyV1, SoraPrivateModelArtifactRefV1,
        SoraPrivateUploadedModelExecutionReceiptV1, SoraRolloutStageV1, SoraRuntimeExecutionHostV1,
        SoraRuntimeReceiptV1, SoraServiceAuditEventV1, SoraServiceConfigEntryV1,
        SoraServiceConfigMutationV1, SoraServiceDeploymentStateV1, SoraServiceExecutionPlaneV1,
        SoraServiceHandlerClassV1, SoraServiceLeaseReportingEpochRolloverV1,
        SoraServiceLeaseStateV1, SoraServiceLeaseStatusV1, SoraServiceLeaseUsageAuditV1,
        SoraServiceLifecycleActionV1, SoraServiceMutationPreconditionV1, SoraServiceRolloutStateV1,
        SoraServiceSecretEntryV1, SoraServiceSecretMutationV1, SoraStateBindingV1,
        SoraStateEncryptionV1, SoraStateMutabilityV1, SoraStateMutationOperationV1, SoraTlsModeV1,
        SoraTrainingJobActionV1, SoraTrainingJobAuditEventV1, SoraTrainingJobRecordV1,
        SoraTrainingJobStatusV1, SoraUploadedModelBundleV1, SoraUploadedModelEncryptionRecipientV1,
        SoraUploadedModelRuntimeFormatV1, SoracloudFheBootstrapKeyProofV1,
        SoracloudFheFullBootstrapExecutionProofV1, SoracloudFheInputAdmissionProofV1,
        SoracloudFhePolicyReferenceV1, SoracloudFhePublicKeyProofV1,
        derive_hf_shared_lease_pool_id_v1, derive_hf_source_id_v1,
        derive_soracloud_private_model_request_commitment_v1,
        derive_soracloud_private_model_result_commitment_v1,
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1,
        encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_app_infra_provenance_payload,
        encode_bundle_provenance_payload, encode_ciphertext_query_provenance_payload,
        encode_decryption_request_provenance_payload,
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
        encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_set_service_config_provenance_payload,
        encode_set_service_secret_provenance_payload, encode_state_mutation_provenance_payload,
        encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
        hf_shared_lease_max_compute_reservation_fee_v1, is_canonical_hf_commit_oid_v1,
        is_canonical_hf_repo_id_v1,
    },
    sorafs::pin_registry::{
        ManifestDigest, ManifestRootCid, PinManifestRecord, PinStatus,
        SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1, StorageClass,
        derive_sorafs_auto_replication_order_id_v1,
    },
    transaction::SignedTransaction,
};
use iroha_primitives::{
    json::Json,
    numeric::{NumericOperationError, Quantity},
};
use mv::storage::StorageReadOnly;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _},
    num::NonZeroU64,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
#[cfg(test)]
use tokio::sync::RwLock;
mod bounded_public_response;
mod hf_model_info_response;
const CONTROL_PLANE_SCHEMA_VERSION: u16 = 1;
const PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME: &str = "soracloud/public_service_discovery";
const PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1: u16 = 1;
const DEFAULT_AUDIT_LIMIT: usize = 20;
const MAX_AUDIT_LIMIT: usize = 500;
const AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS: u64 = 1_000;
const AGENT_AUTONOMY_RECENT_RUN_LIMIT: usize = 20;
const CIPHERTEXT_QUERY_PROOF_SCHEME_V1: &str = "soracloud.audit_anchor.v1";
const HEALTH_COMPLIANCE_REPORT_VERSION_V1: u16 = 1;
const DEFAULT_HEALTH_COMPLIANCE_LIMIT: usize = 50;
const MAX_HEALTH_COMPLIANCE_LIMIT: usize = 500;
const TRAINING_JOB_STATUS_SCHEMA_VERSION_V1: u16 = 1;
pub(crate) const VERIFIED_ACCOUNT_HEADER: &str = "x-iroha-internal-soracloud-account";
pub(crate) const VERIFIED_SIGNER_HEADER: &str = "x-iroha-internal-soracloud-signer";
pub(crate) const VERIFIED_SIGNERS_HEADER: &str = "x-iroha-internal-soracloud-signers";
const TRAINING_MAX_IDENTIFIER_BYTES: usize = 128;
const MODEL_WEIGHT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const MODEL_ARTIFACT_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const UPLOADED_MODEL_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const HF_SHARED_LEASE_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const HF_MODEL_NAME_MAX_BYTES: usize = 128;
const HF_PROFILE_DNS_MAX_ADDRESSES_V1: usize = 32;
const HF_PROFILE_DNS_MAX_IN_FLIGHT_V1: usize = 8;
const HF_PROFILE_DERIVATION_MAX_IN_FLIGHT_V1: usize = 4;
const HF_PROFILE_HEAD_MAX_IN_FLIGHT_V1: usize = 8;
const HF_PROFILE_DNS_TIMEOUT_V1: Duration = Duration::from_secs(5);
static HF_PROFILE_DNS_GATE_V1: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(HF_PROFILE_DNS_MAX_IN_FLIGHT_V1);
static HF_PROFILE_DERIVATION_GATE_V1: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(HF_PROFILE_DERIVATION_MAX_IN_FLIGHT_V1);
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
#[norito(deny_unknown_fields)]
pub(crate) enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
    ConfigMutation,
    SecretMutation,
    StateMutation,
    FheJobRun,
    FhePolicyRegister,
    FhePolicyRotate,
    FhePolicyRevoke,
    DecryptionRequest,
    CiphertextQuery,
    Rollout,
    LeaseUsage,
    LeaseReportingEpochRollover,
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) enum UploadedModelAction {
    Register,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedBundleRequest {
    pub bundle: SoraDeploymentBundleV1,
    pub initial_service_configs: BTreeMap<String, Json>,
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    pub precondition: SoraServiceMutationPreconditionV1,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAppInfraRequest {
    pub deploy_services: Vec<SignedBundleRequest>,
    pub upgrade_services: Vec<SignedBundleRequest>,
    pub manifest: SoraAppInfraManifestV1,
    pub precondition: SoraAppInfraMutationPreconditionV1,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AppInfraStatusQuery {
    #[norito(default)]
    pub app_name: Option<String>,
    #[norito(default)]
    pub audit_limit: Option<usize>,
}
#[derive(Clone, Debug, JsonSerialize, NoritoSerialize)]
pub(crate) struct AppInfraStatusResponse {
    pub schema_version: u16,
    pub app_count: u32,
    pub audit_event_count: u32,
    pub apps: Vec<SoraAppInfraStateV1>,
    pub recent_audit_events: Vec<SoraAppInfraAuditEventV1>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct RollbackPayload {
    pub service_name: String,
    pub target_version: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) enum StateMutationOperation {
    Upsert,
    Delete,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct StateMutationRequest {
    pub service_name: String,
    pub binding_name: String,
    pub key: String,
    pub operation: StateMutationOperation,
    #[norito(required)]
    pub value_size_bytes: Option<u64>,
    #[norito(required)]
    pub value_payload_hex: Option<String>,
    pub encryption: SoraStateEncryptionV1,
    pub governance_tx_hash: Hash,
    #[norito(required)]
    pub fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedStateMutationRequest {
    pub payload: StateMutationRequest,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigSetRequest {
    pub service_name: String,
    pub config_name: String,
    pub value_json: Json,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedServiceConfigSetRequest {
    pub payload: ServiceConfigSetRequest,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigDeleteRequest {
    pub service_name: String,
    pub config_name: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedServiceConfigDeleteRequest {
    pub payload: ServiceConfigDeleteRequest,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceSecretSetRequest {
    pub service_name: String,
    pub secret_name: String,
    pub secret: SecretEnvelopeV1,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedServiceSecretSetRequest {
    pub payload: ServiceSecretSetRequest,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceSecretDeleteRequest {
    pub service_name: String,
    pub secret_name: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedServiceSecretDeleteRequest {
    pub payload: ServiceSecretDeleteRequest,
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
#[norito(deny_unknown_fields)]
pub(crate) enum RolloutStage {
    Canary,
    Promoted,
    RolledBack,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct RolloutAdvancePayload {
    pub service_name: String,
    pub rollout_handle: String,
    pub healthy: bool,
    #[norito(required)]
    pub promote_to_percent: Option<u8>,
    pub governance_tx_hash: Hash,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedRolloutAdvanceRequest {
    pub payload: RolloutAdvancePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentDeployPayload {
    pub manifest: AgentApartmentManifestV1,
    pub lease_blocks: u64,
    pub autonomy_budget_units: u64,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentDeployRequest {
    pub payload: AgentDeployPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentLeaseRenewPayload {
    pub apartment_name: String,
    pub lease_blocks: u64,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentLeaseRenewRequest {
    pub payload: AgentLeaseRenewPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfDeployPayload {
    pub repo_id: String,
    pub revision: String,
    pub model_name: String,
    pub service_name: String,
    #[norito(required)]
    pub apartment_name: Option<String>,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    pub lease_asset_definition_id: AssetDefinitionId,
    pub base_fee: Quantity,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedHfDeployRequest {
    pub payload: HfDeployPayload,
    pub provenance: ManifestProvenance,
    #[norito(required)]
    pub generated_service_provenance: Option<ManifestProvenance>,
    #[norito(required)]
    pub generated_apartment_provenance: Option<ManifestProvenance>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfLeaseLeavePayload {
    pub repo_id: String,
    pub revision: String,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    #[norito(required)]
    pub service_name: Option<String>,
    #[norito(required)]
    pub apartment_name: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedHfLeaseLeaveRequest {
    pub payload: HfLeaseLeavePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfLeaseRenewPayload {
    pub repo_id: String,
    pub revision: String,
    pub model_name: String,
    pub service_name: String,
    #[norito(required)]
    pub apartment_name: Option<String>,
    pub storage_class: StorageClass,
    pub lease_term_ms: u64,
    pub lease_asset_definition_id: AssetDefinitionId,
    pub base_fee: Quantity,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedHfLeaseRenewRequest {
    pub payload: HfLeaseRenewPayload,
    pub provenance: ManifestProvenance,
    #[norito(required)]
    pub generated_service_provenance: Option<ManifestProvenance>,
    #[norito(required)]
    pub generated_apartment_provenance: Option<ManifestProvenance>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostAdvertisePayload {
    pub capability: SoraModelHostCapabilityRecordV1,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelHostAdvertiseRequest {
    pub payload: ModelHostAdvertisePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostHeartbeatPayload {
    pub validator_account_id: AccountId,
    pub heartbeat_expires_at_ms: u64,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelHostHeartbeatRequest {
    pub payload: ModelHostHeartbeatPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostWithdrawPayload {
    pub validator_account_id: AccountId,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelHostWithdrawRequest {
    pub payload: ModelHostWithdrawPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentRestartPayload {
    pub apartment_name: String,
    pub reason: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentRestartRequest {
    pub payload: AgentRestartPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentPolicyRevokePayload {
    pub apartment_name: String,
    pub capability: String,
    #[norito(required)]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentPolicyRevokeRequest {
    pub payload: AgentPolicyRevokePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentWalletSpendPayload {
    pub apartment_name: String,
    pub asset_definition: String,
    pub amount: Quantity,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentWalletSpendRequest {
    pub payload: AgentWalletSpendPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentWalletApprovePayload {
    pub apartment_name: String,
    pub request_id: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentWalletApproveRequest {
    pub payload: AgentWalletApprovePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMessageSendPayload {
    pub from_apartment: String,
    pub to_apartment: String,
    pub channel: String,
    pub payload: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentMessageSendRequest {
    pub payload: AgentMessageSendPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMessageAckPayload {
    pub apartment_name: String,
    pub message_id: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentMessageAckRequest {
    pub payload: AgentMessageAckPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentArtifactAllowPayload {
    pub apartment_name: String,
    pub artifact_hash: String,
    #[norito(required)]
    pub provenance_hash: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentArtifactAllowRequest {
    pub payload: AgentArtifactAllowPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyRunPayload {
    pub apartment_name: String,
    pub artifact_hash: String,
    #[norito(required)]
    pub provenance_hash: Option<String>,
    pub budget_units: u64,
    pub run_label: String,
    #[norito(required)]
    pub workflow_input_json: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedAgentAutonomyRunRequest {
    pub payload: AgentAutonomyRunPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyFinalizeRequest {
    pub apartment_name: String,
    pub run_id: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct FheJobRunPayload {
    pub service_name: String,
    pub binding_name: String,
    pub job: FheJobSpecV1,
    pub policy_reference: SoracloudFhePolicyReferenceV1,
    #[norito(required)]
    pub public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    #[norito(required)]
    pub bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    pub full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedFheJobRunRequest {
    pub payload: FheJobRunPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct SignedTrainingJobStartRequest {
    pub payload: TrainingJobStartPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrainingJobCheckpointPayload {
    pub service_name: String,
    pub job_id: String,
    pub completed_step: u32,
    pub checkpoint_size_bytes: u64,
    pub metrics_hash: Hash,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedTrainingJobCheckpointRequest {
    pub payload: TrainingJobCheckpointPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrainingJobRetryPayload {
    pub service_name: String,
    pub job_id: String,
    pub reason: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedTrainingJobRetryRequest {
    pub payload: TrainingJobRetryPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightRegisterPayload {
    pub service_name: String,
    pub model_name: String,
    pub weight_version: String,
    pub training_job_id: String,
    #[norito(required)]
    pub parent_version: Option<String>,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelWeightRegisterRequest {
    pub payload: ModelWeightRegisterPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightPromotePayload {
    pub service_name: String,
    pub model_name: String,
    pub weight_version: String,
    pub gate_approved: bool,
    pub gate_report_hash: Hash,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelWeightPromoteRequest {
    pub payload: ModelWeightPromotePayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightRollbackPayload {
    pub service_name: String,
    pub model_name: String,
    pub target_version: String,
    pub reason: String,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelWeightRollbackRequest {
    pub payload: ModelWeightRollbackPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct SignedModelArtifactRegisterRequest {
    pub payload: ModelArtifactRegisterPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct UploadedModelRegisterPayload {
    pub bundle: SoraUploadedModelBundleV1,
    pub model_name: String,
    pub artifact_id: String,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedUploadedModelRegisterRequest {
    pub payload: UploadedModelRegisterPayload,
    pub bundle_provenance: ManifestProvenance,
    pub finalize_provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct DecryptionRequestPayload {
    pub service_name: String,
    pub policy: DecryptionAuthorityPolicyV1,
    pub request: DecryptionRequestV1,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedDecryptionRequest {
    pub payload: DecryptionRequestPayload,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SignedCiphertextQueryRequest {
    pub query: CiphertextQuerySpecV1,
    pub provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) enum ServiceMaterialMutationOperation {
    Upsert,
    Delete,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub config_name: String,
    pub operation: ServiceMaterialMutationOperation,
    pub sequence: u64,
    pub current_version: String,
    pub config_generation: u64,
    pub config_entry_count: u32,
    #[norito(required)]
    pub value_hash: Option<Hash>,
    pub audit_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceSecretMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub secret_name: String,
    pub operation: ServiceMaterialMutationOperation,
    pub sequence: u64,
    pub current_version: String,
    pub secret_generation: u64,
    pub secret_entry_count: u32,
    #[norito(required)]
    pub encryption: Option<SecretEnvelopeEncryptionV1>,
    #[norito(required)]
    pub key_id: Option<String>,
    #[norito(required)]
    pub key_version: Option<u32>,
    #[norito(required)]
    pub commitment: Option<Hash>,
    #[norito(required)]
    pub ciphertext_bytes: Option<u64>,
    pub audit_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigStatusQuery {
    pub service_name: String,
    #[norito(default)]
    pub config_name: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigStatusEntry {
    pub config_name: String,
    pub value_hash: Hash,
    pub value_json: norito::json::Value,
    pub last_update_sequence: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceConfigStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub config_generation: u64,
    pub config_entry_count: u32,
    pub configs: Vec<ServiceConfigStatusEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceSecretStatusQuery {
    pub service_name: String,
    #[norito(default)]
    pub secret_name: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct ServiceSecretStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub secret_generation: u64,
    pub secret_entry_count: u32,
    pub secrets: Vec<ServiceSecretStatusEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct DecryptionRequestResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub policy_name: Name,
    pub request_id: String,
    pub binding_name: Name,
    pub state_key: String,
    pub jurisdiction_tag: String,
    pub policy_snapshot_hash: Hash,
    #[norito(required)]
    pub consent_evidence_hash: Option<Hash>,
    pub break_glass: bool,
    #[norito(required)]
    pub break_glass_reason: Option<String>,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub current_version: String,
    pub audit_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct CiphertextQueryResponse {
    pub action: SoracloudAction,
    pub response: CiphertextQueryResponseV1,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
    #[norito(required)]
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
    #[norito(required)]
    pub latest_metrics_hash: Option<Hash>,
    #[norito(required)]
    pub last_failure_reason: Option<String>,
    pub training_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrainingJobStatusResponse {
    pub schema_version: u16,
    pub job: TrainingJobStatusEntry,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrainingJobStatusEntry {
    pub service_name: String,
    pub model_name: String,
    pub job_id: String,
    pub status: TrainingJobStatus,
    pub worker_group_size: u16,
    pub target_steps: u32,
    pub completed_steps: u32,
    pub checkpoint_interval_steps: u32,
    #[norito(required)]
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
    #[norito(required)]
    pub latest_metrics_hash: Option<Hash>,
    #[norito(required)]
    pub last_failure_reason: Option<String>,
    pub created_sequence: u64,
    pub updated_sequence: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightMutationResponse {
    pub action: ModelWeightAction,
    pub service_name: String,
    pub model_name: String,
    pub target_version: String,
    #[norito(required)]
    pub current_version: Option<String>,
    #[norito(required)]
    pub parent_version: Option<String>,
    pub sequence: u64,
    pub version_count: u32,
    pub model_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightStatusResponse {
    pub schema_version: u16,
    pub model: ModelWeightStatusEntry,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightStatusEntry {
    pub service_name: String,
    pub model_name: String,
    #[norito(required)]
    pub current_version: Option<String>,
    pub version_count: u32,
    pub versions: Vec<ModelWeightVersionEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightVersionEntry {
    pub weight_version: String,
    #[norito(required)]
    pub parent_version: Option<String>,
    pub training_job_id: String,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
    pub registered_sequence: u64,
    #[norito(required)]
    pub promoted_sequence: Option<u64>,
    #[norito(required)]
    pub gate_report_hash: Option<Hash>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelArtifactMutationResponse {
    pub action: ModelArtifactAction,
    pub service_name: String,
    pub model_name: String,
    pub training_job_id: String,
    pub artifact_id: String,
    #[norito(required)]
    pub weight_version: Option<String>,
    pub sequence: u64,
    pub model_artifact_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelArtifactStatusResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub model_name: String,
    pub artifact_count: u32,
    pub artifact: ModelArtifactStatusEntry,
    pub artifacts: Vec<ModelArtifactStatusEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelArtifactStatusEntry {
    pub service_name: String,
    pub model_name: String,
    pub artifact_id: String,
    pub training_job_id: String,
    #[norito(required)]
    pub weight_version: Option<String>,
    pub weight_artifact_hash: Hash,
    pub dataset_ref: String,
    pub training_config_hash: Hash,
    pub reproducibility_hash: Hash,
    pub provenance_attestation_hash: Hash,
    pub registered_sequence: u64,
    #[norito(required)]
    pub consumed_by_version: Option<String>,
    #[norito(required)]
    pub chunk_manifest_root: Option<Hash>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct UploadedModelStatusResponse {
    pub schema_version: u16,
    pub bundle: SoraUploadedModelBundleV1,
    #[norito(required)]
    pub artifact: Option<ModelArtifactStatusEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct UploadedModelEncryptionRecipientResponse {
    pub recipient: SoraUploadedModelEncryptionRecipientV1,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PrivateUploadedModelExecuteRequest {
    pub service_name: String,
    pub service_version: String,
    pub weight_version: String,
    /// Canonical immutable uploaded-model identity; aliases are discovery-only.
    pub model_id: String,
    /// Exact committed bundle root for the selected model release.
    pub bundle_root: Hash,
    /// Exact committed authorization record for releasing the encrypted input.
    pub decryption_request_id: String,
    /// Encrypted input persisted in `SoraFS`; plaintext is never accepted by Torii.
    pub input_artifact: SoraPrivateModelArtifactRefV1,
    /// Exact public key metadata to which the runtime must wrap the encrypted output.
    pub output_recipient: SoraUploadedModelEncryptionRecipientV1,
}
/// Current ledger-submission phase for one private uploaded-model execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrivateUploadedModelSubmissionPhaseV1 {
    /// The exact output pin exists externally but has not reached the required durability quorum.
    AwaitingOutputDurability,
    /// The output-pin transaction is the current durable submission.
    OutputPinSubmitted,
    /// The receipt-only transaction is the current durable submission.
    ReceiptSubmitted,
    /// The exact receipt is committed in authoritative world state.
    Committed,
}
impl PrivateUploadedModelSubmissionPhaseV1 {
    /// Return the stable V1 JSON label.
    const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingOutputDurability => "awaiting_output_durability",
            Self::OutputPinSubmitted => "output_pin_submitted",
            Self::ReceiptSubmitted => "receipt_submitted",
            Self::Committed => "committed",
        }
    }
}
impl core::str::FromStr for PrivateUploadedModelSubmissionPhaseV1 {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "awaiting_output_durability" => Ok(Self::AwaitingOutputDurability),
            "output_pin_submitted" => Ok(Self::OutputPinSubmitted),
            "receipt_submitted" => Ok(Self::ReceiptSubmitted),
            "committed" => Ok(Self::Committed),
            _ => Err("unknown private uploaded-model submission phase"),
        }
    }
}
impl norito::json::FastJsonWrite for PrivateUploadedModelSubmissionPhaseV1 {
    fn write_json(&self, output: &mut String) {
        norito::json::write_json_string(self.as_str(), output);
    }
}
impl norito::json::JsonDeserialize for PrivateUploadedModelSubmissionPhaseV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|error: &'static str| norito::json::Error::Message(error.into()))
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let Some(value) = value.as_str() else {
            return Err(norito::json::Error::Message(
                "private uploaded-model submission phase must be a string".into(),
            ));
        };
        value
            .parse()
            .map_err(|error: &'static str| norito::json::Error::Message(error.into()))
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PrivateUploadedModelExecuteResponse {
    pub schema_version: u16,
    pub status: UploadedModelStatusResponse,
    /// Exact current phase of the durable output-pin-then-receipt state machine.
    pub submission_phase: PrivateUploadedModelSubmissionPhaseV1,
    /// Canonical signed transaction hash for the current submission phase, when one exists.
    #[norito(required)]
    pub transaction_hash: Option<Hash>,
    pub receipt: SoraPrivateUploadedModelExecutionReceiptV1,
    /// Runtime-created and persisted encrypted output. It must exactly equal the receipt binding.
    pub output_artifact: SoraPrivateModelArtifactRefV1,
}

const PRIVATE_EXECUTION_SUBMISSION_CACHE_MAX_ENTRIES: usize = 4_096;
const PRIVATE_EXECUTION_SUBMISSION_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const PRIVATE_EXECUTION_SUBMISSION_PHASE_COUNT_V1: u32 = 2;
const PRIVATE_EXECUTION_RETENTION_SAFETY_SECS_V1: u64 = 60;
const PRIVATE_EXECUTION_INITIAL_RECEIPT_BLOCK_OFFSET_V1: u64 = 3;

fn private_execution_required_retention_margin(
    signed_attempt_lifetime: Duration,
    recovery_interval: Duration,
) -> Duration {
    let per_phase_submission = signed_attempt_lifetime
        .saturating_mul(u32::from(
            SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_MAX_SUBMISSION_ATTEMPTS_V1,
        ))
        .saturating_add(recovery_interval.saturating_mul(2));
    Duration::from_secs(SORACLOUD_PRIVATE_OUTPUT_MIN_RETENTION_SECS_V1)
        .saturating_add(Duration::from_secs(u64::from(
            SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
        )))
        .saturating_add(
            per_phase_submission.saturating_mul(PRIVATE_EXECUTION_SUBMISSION_PHASE_COUNT_V1),
        )
        .saturating_add(Duration::from_secs(
            PRIVATE_EXECUTION_RETENTION_SAFETY_SECS_V1,
        ))
}

fn private_execution_release_candidate_is_authorized(
    release_height: u64,
    requested_ttl_blocks: u32,
    candidate_height: u64,
) -> Option<(u64, bool)> {
    let expires_at_height = release_height.checked_add(u64::from(requested_ttl_blocks))?;
    Some((
        expires_at_height,
        candidate_height >= release_height && candidate_height < expires_at_height,
    ))
}

#[derive(Debug)]
enum PrivateExecutionSubmissionState {
    Executing {
        request_fingerprint: Hash,
    },
    Submitted {
        request_fingerprint: Hash,
        response: PrivateUploadedModelExecuteResponse,
        submitted_at: Instant,
    },
}

/// Node-local coalescing state for expensive deterministic private execution submissions.
///
/// Authoritative committed receipts remain the durable source of truth. This bounded cache only
/// prevents concurrent or pre-commit retries from executing the same released ciphertext twice.
#[derive(Debug, Default)]
pub(crate) struct PrivateExecutionSubmissionTracker {
    entries: parking_lot::Mutex<BTreeMap<(String, String), PrivateExecutionSubmissionState>>,
}

struct PrivateExecutionSubmissionGuard {
    tracker: Arc<PrivateExecutionSubmissionTracker>,
    key: (String, String),
    request_fingerprint: Hash,
    completed: bool,
}

impl PrivateExecutionSubmissionGuard {
    fn complete(mut self, response: PrivateUploadedModelExecuteResponse) {
        self.tracker.entries.lock().insert(
            self.key.clone(),
            PrivateExecutionSubmissionState::Submitted {
                request_fingerprint: self.request_fingerprint,
                response,
                submitted_at: Instant::now(),
            },
        );
        self.completed = true;
    }
}

impl Drop for PrivateExecutionSubmissionGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut entries = self.tracker.entries.lock();
        if matches!(
            entries.get(&self.key),
            Some(PrivateExecutionSubmissionState::Executing { request_fingerprint })
                if *request_fingerprint == self.request_fingerprint
        ) {
            entries.remove(&self.key);
        }
    }
}

enum PrivateExecutionSubmissionClaim {
    Acquired(PrivateExecutionSubmissionGuard),
    Cached(PrivateUploadedModelExecuteResponse),
}
#[derive(Clone, Debug, Default, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PrivateUploadedModelReceiptQuery {
    #[norito(default)]
    pub receipt_id: Option<Hash>,
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub model_id: Option<String>,
    #[norito(default)]
    pub weight_version: Option<String>,
    #[norito(default)]
    pub cursor: Option<String>,
    #[norito(default)]
    pub limit: Option<u32>,
    #[norito(default)]
    pub count_mode: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PrivateUploadedModelReceiptListResponse {
    pub schema_version: u16,
    pub receipts: Vec<SoraPrivateUploadedModelExecutionReceiptV1>,
    #[norito(required)]
    pub total: Option<u32>,
    pub returned_items: u32,
    #[norito(required)]
    pub remaining_items: Option<u32>,
    pub has_more: bool,
    pub count_mode: String,
    #[norito(required)]
    pub continue_cursor: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateUploadedModelReceiptCountMode {
    Bounded,
    Exact,
}
pub(crate) const PRIVATE_UPLOADED_MODEL_RECEIPT_DEFAULT_LIMIT: u32 = 50;
pub(crate) const PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT: u32 = 500;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_MAGIC: [u8; 4] = *b"SPRC";
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_VERSION_V1: u8 = 1;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FRAME_BYTES_V1: usize = 4 + 1 + 32 + 8 + 8 + 32;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_ENCODED_CHARS_V1: usize = 114;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FILTER_DOMAIN_V1: &[u8] =
    b"iroha.soracloud.private-uploaded-model.receipt-cursor-filter.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateUploadedModelReceiptCursorV1 {
    snapshot_sequence: u64,
    after_sequence: u64,
    after_receipt_id: Hash,
}

fn append_private_receipt_cursor_filter_component(frame: &mut Vec<u8>, value: Option<&[u8]>) {
    match value {
        Some(value) => {
            frame.push(1);
            frame.extend_from_slice(
                &u32::try_from(value.len())
                    .expect("bounded private receipt filter length fits u32")
                    .to_be_bytes(),
            );
            frame.extend_from_slice(value);
        }
        None => frame.push(0),
    }
}

fn private_uploaded_model_receipt_filter_digest(
    receipt_id: Option<&Hash>,
    service_name: Option<&str>,
    model_id: Option<&str>,
    weight_version: Option<&str>,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(
        PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FILTER_DOMAIN_V1.len()
            + receipt_id.map_or(0, |_| Hash::LENGTH)
            + service_name.map_or(0, str::len)
            + model_id.map_or(0, str::len)
            + weight_version.map_or(0, str::len)
            + 16,
    );
    frame.extend_from_slice(PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FILTER_DOMAIN_V1);
    append_private_receipt_cursor_filter_component(
        &mut frame,
        receipt_id.map(|hash| {
            let bytes: &[u8; Hash::LENGTH] = hash.as_ref();
            bytes.as_slice()
        }),
    );
    append_private_receipt_cursor_filter_component(&mut frame, service_name.map(str::as_bytes));
    append_private_receipt_cursor_filter_component(&mut frame, model_id.map(str::as_bytes));
    append_private_receipt_cursor_filter_component(&mut frame, weight_version.map(str::as_bytes));
    *Hash::new(frame).as_ref()
}

fn encode_private_uploaded_model_receipt_cursor(
    filter_digest: [u8; 32],
    cursor: PrivateUploadedModelReceiptCursorV1,
) -> String {
    let mut frame = Vec::with_capacity(PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FRAME_BYTES_V1);
    frame.extend_from_slice(&PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_MAGIC);
    frame.push(PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_VERSION_V1);
    frame.extend_from_slice(&filter_digest);
    frame.extend_from_slice(&cursor.snapshot_sequence.to_be_bytes());
    frame.extend_from_slice(&cursor.after_sequence.to_be_bytes());
    frame.extend_from_slice(cursor.after_receipt_id.as_ref());
    BASE64_URL_SAFE_NO_PAD.encode(frame)
}

fn decode_private_uploaded_model_receipt_cursor(
    raw: &str,
    expected_filter_digest: [u8; 32],
) -> Result<PrivateUploadedModelReceiptCursorV1, SoracloudError> {
    let invalid = || {
        SoracloudError::bad_request(
            "private uploaded-model receipt cursor is not a canonical V1 cursor for these filters",
        )
    };
    if raw.len() != PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_ENCODED_CHARS_V1 {
        return Err(invalid());
    }
    let frame = BASE64_URL_SAFE_NO_PAD
        .decode(raw.as_bytes())
        .map_err(|_| invalid())?;
    if BASE64_URL_SAFE_NO_PAD.encode(&frame) != raw
        || frame.len() != PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_FRAME_BYTES_V1
        || frame[..4] != PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_MAGIC
        || frame[4] != PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_VERSION_V1
        || frame[5..37] != expected_filter_digest
    {
        return Err(invalid());
    }
    let snapshot_sequence = u64::from_be_bytes(
        frame[37..45]
            .try_into()
            .expect("validated private receipt cursor snapshot slice"),
    );
    let after_sequence = u64::from_be_bytes(
        frame[45..53]
            .try_into()
            .expect("validated private receipt cursor sequence slice"),
    );
    if after_sequence == 0 || after_sequence > snapshot_sequence {
        return Err(invalid());
    }
    let after_receipt_id = Hash::prehashed(
        frame[53..]
            .try_into()
            .expect("validated private receipt cursor hash slice"),
    );
    Ok(PrivateUploadedModelReceiptCursorV1 {
        snapshot_sequence,
        after_sequence,
        after_receipt_id,
    })
}
fn private_uploaded_model_receipt_limit(raw: Option<u32>) -> Result<usize, SoracloudError> {
    let limit = raw.unwrap_or(PRIVATE_UPLOADED_MODEL_RECEIPT_DEFAULT_LIMIT);
    if !(1..=PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT).contains(&limit) {
        return Err(SoracloudError::bad_request(format!(
            "private uploaded-model receipt limit must be in 1..={PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT}"
        )));
    }
    Ok(usize::try_from(limit).expect("bounded private receipt limit fits usize"))
}
impl PrivateUploadedModelReceiptCountMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Bounded => "bounded",
            Self::Exact => "exact",
        }
    }
}
fn private_uploaded_model_receipt_count_mode(
    raw: Option<&str>,
) -> Result<PrivateUploadedModelReceiptCountMode, SoracloudError> {
    match raw {
        Some("exact") => Ok(PrivateUploadedModelReceiptCountMode::Exact),
        Some("bounded") | None => Ok(PrivateUploadedModelReceiptCountMode::Bounded),
        Some(other) => Err(SoracloudError::bad_request(format!(
            "invalid count_mode `{other}`; expected `bounded` or `exact`"
        ))),
    }
}

#[derive(Clone, Copy)]
struct PrivateUploadedModelReceiptPageSpec<'a> {
    receipt_id: Option<&'a Hash>,
    service_name: Option<&'a str>,
    model_id: Option<&'a str>,
    weight_version: Option<&'a str>,
    filter_digest: [u8; 32],
    cursor: Option<PrivateUploadedModelReceiptCursorV1>,
    limit: usize,
    count_mode: PrivateUploadedModelReceiptCountMode,
    current_sequence: u64,
}

fn paginate_private_uploaded_model_receipts<'a>(
    receipts: impl Iterator<Item = (&'a Hash, &'a SoraPrivateUploadedModelExecutionReceiptV1)>,
    spec: PrivateUploadedModelReceiptPageSpec<'_>,
) -> Result<PrivateUploadedModelReceiptListResponse, SoracloudError> {
    let snapshot_sequence = spec
        .cursor
        .map_or(spec.current_sequence, |cursor| cursor.snapshot_sequence);
    if snapshot_sequence > spec.current_sequence {
        return Err(SoracloudError::conflict(
            "private uploaded-model receipt cursor names a future ledger snapshot",
        ));
    }
    let after_key = spec
        .cursor
        .map(|cursor| (cursor.after_sequence, cursor.after_receipt_id));
    let mut cursor_found = spec.cursor.is_none();
    let mut exact_total = 0_u64;
    let mut exact_suffix = 0_u64;
    let mut page = BTreeMap::new();
    for (receipt_id, receipt) in receipts {
        if receipt.emitted_sequence > snapshot_sequence
            || spec.receipt_id.is_some_and(|filter| filter != receipt_id)
            || spec
                .service_name
                .is_some_and(|filter| filter != receipt.service_name.as_ref())
            || spec
                .model_id
                .is_some_and(|filter| filter != receipt.model_id)
            || spec
                .weight_version
                .is_some_and(|filter| filter != receipt.weight_version)
        {
            continue;
        }
        if spec.count_mode == PrivateUploadedModelReceiptCountMode::Exact {
            exact_total = exact_total.checked_add(1).ok_or_else(|| {
                SoracloudError::internal("private uploaded-model receipt exact total overflows u64")
            })?;
        }
        let key = (receipt.emitted_sequence, *receipt_id);
        if after_key.is_some_and(|after| key == after) {
            cursor_found = true;
            continue;
        }
        if after_key.is_some_and(|after| key < after) {
            continue;
        }
        if spec.count_mode == PrivateUploadedModelReceiptCountMode::Exact {
            exact_suffix = exact_suffix.checked_add(1).ok_or_else(|| {
                SoracloudError::internal(
                    "private uploaded-model receipt exact remaining count overflows u64",
                )
            })?;
        }
        page.insert(key, receipt);
        if page.len() > spec.limit.saturating_add(1) {
            page.pop_last();
        }
    }
    if !cursor_found {
        return Err(SoracloudError::bad_request(
            "private uploaded-model receipt cursor does not name an exact retained receipt",
        ));
    }
    let has_more = if spec.count_mode == PrivateUploadedModelReceiptCountMode::Exact {
        exact_suffix > u64::try_from(spec.limit).expect("bounded receipt limit fits u64")
    } else {
        page.len() > spec.limit
    };
    if page.len() > spec.limit {
        page.pop_last();
    }
    let last_key = page.last_key_value().map(|(key, _)| *key);
    let receipts = page
        .into_values()
        .cloned()
        .collect::<Vec<SoraPrivateUploadedModelExecutionReceiptV1>>();
    let returned_items = u32::try_from(receipts.len())
        .expect("bounded private uploaded-model receipt page length fits u32");
    let (total, remaining_items) = if spec.count_mode == PrivateUploadedModelReceiptCountMode::Exact
    {
        let total = u32::try_from(exact_total).map_err(|_| {
            SoracloudError::internal(
                "private uploaded-model receipt exact total exceeds the V1 u32 response range",
            )
        })?;
        let remaining = exact_suffix
            .checked_sub(u64::from(returned_items))
            .ok_or_else(|| {
                SoracloudError::internal(
                    "private uploaded-model receipt exact remaining count underflows",
                )
            })?;
        let remaining = u32::try_from(remaining).map_err(|_| {
            SoracloudError::internal(
                "private uploaded-model receipt exact remaining count exceeds the V1 u32 response range",
            )
        })?;
        (Some(total), Some(remaining))
    } else {
        (None, None)
    };
    let continue_cursor = if has_more {
        let (after_sequence, after_receipt_id) = last_key
            .expect("a private receipt page with more items must return a continuation key");
        Some(encode_private_uploaded_model_receipt_cursor(
            spec.filter_digest,
            PrivateUploadedModelReceiptCursorV1 {
                snapshot_sequence,
                after_sequence,
                after_receipt_id,
            },
        ))
    } else {
        None
    };
    Ok(PrivateUploadedModelReceiptListResponse {
        schema_version: 1,
        receipts,
        total,
        returned_items,
        remaining_items,
        has_more,
        count_mode: spec.count_mode.label().to_owned(),
        continue_cursor,
    })
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct UploadedModelMutationResponse {
    pub action: UploadedModelAction,
    pub status: UploadedModelStatusResponse,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfSharedLeaseStatusResponse {
    pub schema_version: u16,
    pub source: SoraHfSourceRecordV1,
    #[norito(required)]
    pub runtime_projection: Option<SoracloudRuntimeHfSourcePlan>,
    #[norito(required)]
    pub pool: Option<SoraHfSharedLeasePoolV1>,
    #[norito(required)]
    pub member: Option<SoraHfSharedLeaseMemberV1>,
    #[norito(required)]
    pub placement: Option<SoraHfPlacementRecordV1>,
    #[norito(required)]
    pub latest_audit_event: Option<SoraHfSharedLeaseAuditEventV1>,
    pub audit_event_count: u32,
    pub storage_base_fee: Quantity,
    pub compute_reservation_fee: Quantity,
    pub eligible_host_count: u32,
    pub warm_host_count: u32,
    pub importer_pending: bool,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfSharedLeaseMutationResponse {
    pub schema_version: u16,
    pub action: SoraHfSharedLeaseActionV1,
    pub source: SoraHfSourceRecordV1,
    #[norito(required)]
    pub runtime_projection: Option<SoracloudRuntimeHfSourcePlan>,
    pub pool: SoraHfSharedLeasePoolV1,
    pub member: SoraHfSharedLeaseMemberV1,
    #[norito(required)]
    pub placement: Option<SoraHfPlacementRecordV1>,
    #[norito(required)]
    pub latest_audit_event: Option<SoraHfSharedLeaseAuditEventV1>,
    pub storage_base_fee: Quantity,
    pub compute_reservation_fee: Quantity,
    pub eligible_host_count: u32,
    pub warm_host_count: u32,
    pub importer_pending: bool,
}
#[derive(Clone, Copy, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
#[norito(deny_unknown_fields)]
pub(crate) enum ModelHostMutationAction {
    Advertise,
    Heartbeat,
    Withdraw,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostStatusResponse {
    pub schema_version: u16,
    #[norito(required)]
    pub validator_account_id: Option<AccountId>,
    pub active_host_count: u32,
    pub hosts: Vec<SoraModelHostCapabilityRecordV1>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostMutationResponse {
    pub action: ModelHostMutationAction,
    pub status: ModelHostStatusResponse,
    pub signed_by: String,
}
#[derive(Clone, Debug, Default, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthComplianceReportQuery {
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub jurisdiction_tag: Option<String>,
    #[norito(default)]
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrainingJobStatusQuery {
    pub service_name: String,
    pub job_id: String,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelWeightStatusQuery {
    pub service_name: String,
    pub model_name: String,
}
#[derive(Clone, Debug, Default, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct UploadedModelStatusQuery {
    pub service_name: String,
    pub weight_version: String,
    #[norito(default)]
    pub model_id: Option<String>,
    #[norito(default)]
    pub model_name: Option<String>,
    #[norito(default)]
    pub bundle_root: Option<Hash>,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HfSharedLeaseStatusQuery {
    pub repo_id: String,
    pub revision: String,
    pub storage_class: String,
    pub lease_term_ms: u64,
    #[norito(default)]
    /// Optional account filter as canonical I105 or on-chain account alias.
    pub account_id: Option<String>,
}
#[derive(Clone, Debug, Default, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ModelHostStatusQuery {
    #[norito(default)]
    /// Optional validator filter as canonical I105 or on-chain account alias.
    pub account_id: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyStatusQuery {
    pub apartment_name: String,
}
#[derive(Clone, Debug, Default, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentStatusQuery {
    #[norito(default)]
    pub apartment_name: Option<String>,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMailboxStatusQuery {
    pub apartment_name: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct MutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(required)]
    pub previous_version: Option<String>,
    pub current_version: String,
    pub sequence: u64,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub revision_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(required)]
    pub rollout_handle: Option<String>,
    #[norito(required)]
    pub rollout_stage: Option<RolloutStage>,
    #[norito(required)]
    pub rollout_percent: Option<u8>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ControlPlaneSnapshot {
    pub schema_version: u16,
    pub service_count: u32,
    pub audit_event_count: u32,
    pub services: Vec<ControlPlaneServiceSnapshot>,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostedHttpRouteMatch {
    pub service_name: String,
    pub service_version: String,
    pub request_path: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PublicRouteMatch {
    LocalRead(LocalReadRouteMatch),
    HostedHttp(HostedHttpRouteMatch),
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthComplianceReportResponse {
    pub schema_version: u16,
    #[norito(required)]
    pub service_name: Option<String>,
    #[norito(required)]
    pub jurisdiction_tag: Option<String>,
    pub generated_at_sequence: u64,
    pub total_access_events: u32,
    pub break_glass_events: u32,
    pub non_break_glass_events: u32,
    pub consent_evidence_present_events: u32,
    pub consent_evidence_coverage_bps: u16,
    pub recent_access_events: Vec<HealthAccessAuditEntry>,
    pub jurisdiction_stats: Vec<HealthJurisdictionStat>,
    pub data_flow_attestations: Vec<HealthDataFlowAttestation>,
    pub policy_diff_history: Vec<HealthPolicyDiffEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthAccessAuditEntry {
    pub sequence: u64,
    pub service_name: String,
    pub binding_name: String,
    pub state_key: String,
    pub policy_name: String,
    pub jurisdiction_tag: String,
    #[norito(required)]
    pub consent_evidence_hash: Option<Hash>,
    pub break_glass: bool,
    #[norito(required)]
    pub break_glass_reason: Option<String>,
    pub governance_tx_hash: Hash,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthJurisdictionStat {
    pub jurisdiction_tag: String,
    pub access_event_count: u32,
    pub break_glass_event_count: u32,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthDataFlowAttestation {
    pub service_name: String,
    pub current_version: String,
    pub binding_name: String,
    pub key_prefix: String,
    pub encryption: SoraStateEncryptionV1,
    pub mutability: SoraStateMutabilityV1,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct HealthPolicyDiffEntry {
    pub policy_name: String,
    pub jurisdiction_tag: String,
    pub policy_snapshot_hash: Hash,
    pub first_seen_sequence: u64,
    pub last_seen_sequence: u64,
    pub event_count: u32,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SoracloudPublicServiceDiscoveryV1 {
    pub schema_version: u16,
    pub service_name: String,
    pub service_version: String,
    pub execution_plane: String,
    pub runtime: String,
    pub route_host: String,
    pub path_prefix: String,
    pub base_url: String,
    #[norito(required)]
    pub healthcheck_path: Option<String>,
    #[norito(required)]
    pub healthcheck_url: Option<String>,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub deployment_bundle_hash: Hash,
    pub content_cid: String,
    pub public_discovery_url: String,
    pub public_discovery_cid_host_url: String,
    pub manifest_digest_hex: String,
    #[norito(required)]
    pub manifest_id_hex: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SoracloudPublicServiceDiscoveryRegistryV1 {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub revisions: BTreeMap<String, SoracloudPublicServiceDiscoveryV1>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServicePublicDiscoveryResponse {
    pub schema_version: u16,
    pub service_name: String,
    pub current_version: String,
    pub requested_version: String,
    pub discovery: SoracloudPublicServiceDiscoveryV1,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ControlPlaneServiceLeaseSnapshot {
    pub authoritative_state: SoraServiceLeaseStateV1,
    pub effective_status: SoraServiceLeaseStatusV1,
    pub remaining_runtime_balance: Quantity,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ControlPlaneServiceSnapshot {
    pub service_name: String,
    pub current_version: String,
    pub revision_count: u32,
    pub config_generation: u64,
    pub secret_generation: u64,
    pub config_entry_count: u32,
    pub secret_entry_count: u32,
    #[norito(required)]
    pub service_lease: Option<ControlPlaneServiceLeaseSnapshot>,
    #[norito(required)]
    pub public_discovery_content_cid: Option<String>,
    #[norito(required)]
    pub public_discovery_url: Option<String>,
    #[norito(required)]
    pub public_discovery_cid_host_url: Option<String>,
    #[norito(required)]
    pub latest_revision: Option<ControlPlaneServiceRevision>,
    #[norito(required)]
    pub active_rollout: Option<RolloutRuntimeState>,
    #[norito(required)]
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
#[norito(deny_unknown_fields)]
pub(crate) struct ControlPlaneServiceRevision {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub replicas: u16,
    pub execution_plane: SoraServiceExecutionPlaneV1,
    #[norito(required)]
    pub route_host: Option<String>,
    #[norito(required)]
    pub route_path_prefix: Option<String>,
    #[norito(required)]
    pub route_service_port: Option<u16>,
    #[norito(required)]
    pub route_visibility: Option<String>,
    #[norito(required)]
    pub route_tls_mode: Option<String>,
    #[norito(required)]
    pub base_url: Option<String>,
    #[norito(required)]
    pub healthcheck_url: Option<String>,
    #[norito(required)]
    pub public_discovery_content_cid: Option<String>,
    #[norito(required)]
    pub public_discovery_url: Option<String>,
    #[norito(required)]
    pub public_discovery_cid_host_url: Option<String>,
    pub state_binding_count: u32,
    pub state_bindings: Vec<SoraStateBindingV1>,
    pub lease_volumes: Vec<SoraLeaseVolumeBindingV1>,
    pub allow_model_inference: bool,
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
    #[norito(required)]
    pub healthcheck_path: Option<String>,
    /// Required service-scoped configs declared by the container manifest.
    pub required_config_names: Vec<String>,
    /// Required service-scoped secrets declared by the container manifest.
    pub required_secret_names: Vec<String>,
    /// Explicit config exports declared by the container manifest.
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
#[norito(deny_unknown_fields)]
pub(crate) struct ControlPlaneAuditEvent {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(required)]
    pub from_version: Option<String>,
    pub to_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub process_generation: u64,
    pub config_generation: u64,
    pub secret_generation: u64,
    pub config_snapshot_hash: Hash,
    pub secret_snapshot_hash: Hash,
    #[norito(required)]
    pub binding_name: Option<String>,
    #[norito(required)]
    pub state_key: Option<String>,
    pub config_mutations: Vec<SoraServiceConfigMutationV1>,
    pub secret_mutations: Vec<SoraServiceSecretMutationV1>,
    #[norito(required)]
    pub governance_tx_hash: Option<Hash>,
    #[norito(required)]
    pub rollout_state: Option<RolloutRuntimeState>,
    #[norito(required)]
    pub policy_name: Option<String>,
    #[norito(required)]
    pub policy_snapshot_hash: Option<Hash>,
    #[norito(required)]
    pub jurisdiction_tag: Option<String>,
    #[norito(required)]
    pub consent_evidence_hash: Option<Hash>,
    #[norito(required)]
    pub break_glass: Option<bool>,
    #[norito(required)]
    pub break_glass_reason: Option<String>,
    #[norito(required)]
    pub lease_usage: Option<SoraServiceLeaseUsageAuditV1>,
    #[norito(required)]
    pub service_lease_commitment: Option<Hash>,
    #[norito(required)]
    pub lease_reporting_epoch_rollover: Option<SoraServiceLeaseReportingEpochRolloverV1>,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
struct CiphertextRuntimeRecord {
    encryption: SoraStateEncryptionV1,
    payload_bytes: u64,
    commitment: Hash,
    last_update_sequence: u64,
    governance_tx_hash: Hash,
    source_action: SoracloudAction,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct RolloutRuntimeState {
    pub rollout_handle: String,
    pub baseline_version: String,
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
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_height: u64,
    pub lease_remaining_blocks: u64,
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
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(required)]
    pub capability: Option<String>,
    #[norito(required)]
    pub reason: Option<String>,
    #[norito(required)]
    pub last_restart_sequence: Option<u64>,
    #[norito(required)]
    pub last_restart_reason: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentStatusResponse {
    pub schema_version: u16,
    pub apartment_count: u32,
    pub event_count: u32,
    pub apartments: Vec<AgentApartmentStatusEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentApartmentStatusEntry {
    pub apartment_name: String,
    pub manifest_hash: Hash,
    pub status: AgentRuntimeStatus,
    pub lease_started_height: u64,
    pub lease_expires_height: u64,
    pub lease_remaining_blocks: u64,
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
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub spend_limit_count: u32,
    pub upgrade_policy: iroha_data_model::soracloud::AgentUpgradePolicyV1,
    #[norito(required)]
    pub last_restart_sequence: Option<u64>,
    #[norito(required)]
    pub last_restart_reason: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentWalletMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub manifest_hash: Hash,
    pub status: AgentRuntimeStatus,
    #[norito(required)]
    pub request_id: Option<String>,
    #[norito(required)]
    pub asset_definition: Option<String>,
    #[norito(required)]
    pub amount: Option<Quantity>,
    #[norito(required)]
    pub day_bucket: Option<u64>,
    #[norito(required)]
    pub day_spent: Option<Quantity>,
    #[norito(required)]
    pub capability: Option<String>,
    #[norito(required)]
    pub reason: Option<String>,
    pub pending_request_count: u32,
    pub revoked_policy_capability_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMailboxMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub message_id: String,
    #[norito(required)]
    pub from_apartment: Option<String>,
    #[norito(required)]
    pub to_apartment: Option<String>,
    pub channel: String,
    pub payload_hash: Hash,
    pub status: AgentRuntimeStatus,
    pub pending_message_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMailboxStatusResponse {
    pub schema_version: u16,
    pub apartment_name: String,
    pub status: AgentRuntimeStatus,
    pub pending_message_count: u32,
    pub event_count: u32,
    pub messages: Vec<AgentMailboxMessageEntry>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentMailboxMessageEntry {
    pub message_id: String,
    pub from_apartment: String,
    pub channel: String,
    pub payload: String,
    pub payload_hash: Hash,
    pub enqueued_sequence: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
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
    #[norito(required)]
    pub execution_host: Option<SoraRuntimeExecutionHostV1>,
    #[norito(required)]
    pub mailbox_message_id: Option<Hash>,
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentRuntimeWorkflowStepSummary {
    pub step_index: u32,
    #[norito(required)]
    pub step_id: Option<String>,
    pub request_commitment: Hash,
    pub result_commitment: Hash,
    #[norito(required)]
    pub runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(required)]
    pub content_type: Option<String>,
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    #[norito(required)]
    pub response_text: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyExecutionAuditRecord {
    pub sequence: u64,
    pub succeeded: bool,
    pub result_commitment: Hash,
    #[norito(required)]
    pub service_name: Option<String>,
    #[norito(required)]
    pub service_version: Option<String>,
    #[norito(required)]
    pub handler_name: Option<String>,
    #[norito(required)]
    pub runtime_receipt_id: Option<Hash>,
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    #[norito(required)]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentRuntimeExecutionSummary {
    pub apartment_name: String,
    pub run_id: String,
    #[norito(required)]
    pub service_name: Option<String>,
    #[norito(required)]
    pub service_version: Option<String>,
    #[norito(required)]
    pub handler_name: Option<String>,
    pub succeeded: bool,
    pub result_commitment: Hash,
    pub journal_artifact_hash: Hash,
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    #[norito(required)]
    pub runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    pub workflow_steps: Vec<AgentRuntimeWorkflowStepSummary>,
    #[norito(required)]
    pub content_type: Option<String>,
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    #[norito(required)]
    pub response_text: Option<String>,
    #[norito(required)]
    pub error: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyMutationResponse {
    pub action: AgentApartmentAction,
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_height: u64,
    pub lease_remaining_blocks: u64,
    pub manifest_hash: Hash,
    pub artifact_hash: String,
    #[norito(required)]
    pub provenance_hash: Option<String>,
    #[norito(required)]
    pub run_id: Option<String>,
    #[norito(required)]
    pub run_label: Option<String>,
    #[norito(required)]
    pub workflow_input_json: Option<String>,
    #[norito(required)]
    pub budget_units: Option<u64>,
    pub budget_remaining_units: u64,
    pub allowlist_count: u32,
    pub run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(required)]
    pub runtime_execution: Option<AgentRuntimeExecutionSummary>,
    #[norito(required)]
    pub runtime_execution_error: Option<String>,
    #[norito(required)]
    pub authoritative_runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(required)]
    pub authoritative_runtime_receipt_error: Option<String>,
    #[norito(required)]
    pub authoritative_execution_audit: Option<AgentAutonomyExecutionAuditRecord>,
    #[norito(required)]
    pub authoritative_execution_audit_error: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyStatusResponse {
    pub apartment_name: String,
    pub sequence: u64,
    pub status: AgentRuntimeStatus,
    pub lease_expires_height: u64,
    pub lease_remaining_blocks: u64,
    pub manifest_hash: Hash,
    pub revoked_policy_capability_count: u32,
    pub budget_ceiling_units: u64,
    pub budget_remaining_units: u64,
    pub allowlist_count: u32,
    pub run_count: u32,
    pub process_generation: u64,
    pub process_started_sequence: u64,
    pub last_active_sequence: u64,
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    pub checkpoint_count: u32,
    pub persistent_state_total_bytes: u64,
    pub persistent_state_key_count: u32,
    pub allowlist: Vec<AgentAutonomyAllowlistEntry>,
    pub recent_runs: Vec<AgentAutonomyRunRecord>,
    pub runtime_recent_runs: Vec<AgentRuntimeExecutionSummary>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyAllowlistEntry {
    pub artifact_hash: String,
    #[norito(required)]
    pub provenance_hash: Option<String>,
    pub added_sequence: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct AgentAutonomyRunRecord {
    pub run_id: String,
    pub artifact_hash: String,
    #[norito(required)]
    pub provenance_hash: Option<String>,
    pub budget_units: u64,
    pub run_label: String,
    #[norito(required)]
    pub workflow_input_json: Option<String>,
    pub approved_sequence: u64,
    #[norito(required)]
    pub authoritative_runtime_receipt: Option<AgentRuntimeReceiptRecord>,
    #[norito(required)]
    pub authoritative_execution_audit: Option<AgentAutonomyExecutionAuditRecord>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoracloudErrorKind {
    BadRequest,
    Unauthorized,
    NotFound,
    Conflict,
    Unavailable,
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
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Unavailable,
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
            SoracloudErrorKind::Unavailable => "unavailable",
            SoracloudErrorKind::Internal => "internal",
        }
    }
    fn status(&self) -> StatusCode {
        match self.kind {
            SoracloudErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            SoracloudErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            SoracloudErrorKind::NotFound => StatusCode::NOT_FOUND,
            SoracloudErrorKind::Conflict => StatusCode::CONFLICT,
            SoracloudErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            SoracloudErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
impl std::fmt::Display for SoracloudError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for SoracloudError {}
impl From<NumericOperationError> for SoracloudError {
    fn from(error: NumericOperationError) -> Self {
        Self::internal(format!(
            "failed to project authoritative Soracloud economic state: {error}"
        ))
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
    if let SoraNetworkPolicyV1::Allowlist(entries) = &container.capabilities.network {
        if entries.is_empty() {
            return Err(SoracloudError::bad_request(
                "container capability network allowlist must not be empty",
            ));
        }
        let mut seen = BTreeSet::new();
        for entry in entries {
            let normalized = entry.host.trim();
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
            if !seen.insert(normalized.to_ascii_lowercase()) {
                return Err(SoracloudError::bad_request(
                    "container capability network allowlist must not contain duplicates",
                ));
            }
            if entry.ports.is_empty() {
                return Err(SoracloudError::bad_request(
                    "container capability network allowlist entries must include at least one port",
                ));
            }
            let mut seen_ports = BTreeSet::new();
            for port in &entry.ports {
                if *port == 0 {
                    return Err(SoracloudError::bad_request(
                        "container capability network allowlist contains invalid port 0",
                    ));
                }
                if !seen_ports.insert(*port) {
                    return Err(SoracloudError::bad_request(
                        "container capability network allowlist must not contain duplicate ports per host",
                    ));
                }
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
fn parse_agent_apartment_name(apartment_name: &str) -> Result<String, SoracloudError> {
    let normalized = apartment_name.trim();
    let parsed: Name = normalized
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid apartment_name: {err}")))?;
    Ok(parsed.to_string())
}
fn parse_exact_name(field_name: &'static str, value: &str) -> Result<Name, SoracloudError> {
    if value.trim() != value {
        return Err(SoracloudError::bad_request(format!(
            "{field_name} must not contain leading or trailing whitespace"
        )));
    }
    let parsed: Name = value
        .parse()
        .map_err(|err| SoracloudError::bad_request(format!("invalid {field_name}: {err}")))?;
    if parsed.as_ref() != value {
        return Err(SoracloudError::bad_request(format!(
            "{field_name} must use canonical NFC form"
        )));
    }
    Ok(parsed)
}
fn parse_service_name(service_name: &str) -> Result<Name, SoracloudError> {
    parse_exact_name("service_name", service_name)
}
fn parse_optional_service_name(service_name: Option<&str>) -> Result<Option<Name>, SoracloudError> {
    service_name.map(parse_service_name).transpose()
}
fn parse_training_model_name(model_name: &str) -> Result<String, SoracloudError> {
    let parsed = parse_exact_name("model_name", model_name)?;
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
    if !is_canonical_hf_repo_id_v1(repo_id) {
        return Err(SoracloudError::bad_request(
            "repo_id must be one exact fully-qualified `namespace/repository` identifier",
        ));
    }
    Ok(repo_id.to_owned())
}
fn parse_hf_revision(resolved_revision: &str) -> Result<String, SoracloudError> {
    if !is_canonical_hf_commit_oid_v1(resolved_revision) {
        return Err(SoracloudError::bad_request(
            "resolved_revision must be the full 40-character lowercase hexadecimal commit OID",
        ));
    }
    Ok(resolved_revision.to_owned())
}
fn parse_hf_model_name(model_name: &str) -> Result<String, SoracloudError> {
    normalize_hf_token("model_name", model_name, HF_MODEL_NAME_MAX_BYTES)
}
fn parse_optional_account_id(
    state: &iroha_core::state::State,
    telemetry: &crate::routing::MaybeTelemetry,
    context: &'static str,
    account_id: Option<&str>,
) -> Result<Option<AccountId>, SoracloudError> {
    account_id
        .map(|literal| {
            crate::routing::parse_account_literal_with_state(
                state,
                literal.trim(),
                telemetry,
                context,
            )
            .map(|(account_id, _)| account_id)
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
    derive_hf_source_id_v1(repo_id, resolved_revision)
        .map_err(|error| SoracloudError::bad_request(error.to_string()))
}
fn hf_shared_lease_pool_id(
    source_id: Hash,
    storage_class: StorageClass,
    lease_term_ms: u64,
) -> Result<Hash, SoracloudError> {
    derive_hf_shared_lease_pool_id_v1(source_id, storage_class, lease_term_ms)
        .map_err(|error| SoracloudError::bad_request(error.to_string()))
}
fn hf_profile_http_client(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hostname: &str,
    pinned_addresses: &[SocketAddr],
) -> Result<reqwest::Client, SoracloudError> {
    let builder = reqwest::Client::builder()
        .timeout(config.request_timeout)
        .connect_timeout(HF_PROFILE_DNS_TIMEOUT_V1.min(config.request_timeout))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never());
    let builder = if hostname.parse::<IpAddr>().is_ok() {
        builder
    } else {
        builder.resolve_to_addrs(hostname, pinned_addresses)
    };
    builder.build().map_err(|err| {
        SoracloudError::internal(format!(
            "failed to build Hugging Face profile-derivation client: {err}"
        ))
    })
}
fn normalize_hf_base_url(base_url: &str) -> Result<reqwest::Url, SoracloudError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() || trimmed != base_url {
        return Err(SoracloudError::bad_request(
            "Hugging Face base URL must be non-empty and have no surrounding whitespace",
        ));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme).map_err(|err| {
        SoracloudError::bad_request(format!("invalid Hugging Face base URL: {err}"))
    })?;
    validate_hf_profile_url_transport(&url)?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(SoracloudError::bad_request(
            "Hugging Face base URL must not contain a query or fragment",
        ));
    }
    let normalized_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => path.to_owned(),
    };
    url.set_path(&normalized_path);
    Ok(url)
}
fn validate_hf_profile_url_transport(url: &reqwest::Url) -> Result<(), SoracloudError> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.port() == Some(0)
        || url.fragment().is_some()
    {
        return Err(SoracloudError::bad_request(
            "Hugging Face profile URL must have one credential-free network authority",
        ));
    }
    if url.scheme() == "https" {
        return Ok(());
    }
    #[cfg(test)]
    if url.scheme() == "http"
        && url
            .host_str()
            .and_then(|host| host.parse::<IpAddr>().ok())
            .is_some_and(|address| address.is_loopback())
    {
        return Ok(());
    }
    Err(SoracloudError::bad_request(
        "Hugging Face profile URLs must use HTTPS",
    ))
}
fn hf_profile_url_origin(url: &reqwest::Url) -> Result<String, SoracloudError> {
    validate_hf_profile_url_transport(url)?;
    let origin = url.origin().ascii_serialization();
    if origin == "null" {
        return Err(SoracloudError::bad_request(
            "Hugging Face profile URL must have a tuple origin",
        ));
    }
    Ok(origin)
}
fn hf_profile_allowed_redirect_origins(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
) -> Result<BTreeSet<String>, SoracloudError> {
    if config.import_redirect_allowed_origins.len()
        > iroha_config::parameters::defaults::soracloud_runtime::hf::IMPORT_REDIRECT_ALLOWED_ORIGINS_LIMIT
    {
        return Err(SoracloudError::bad_request(format!(
            "Hugging Face redirect-origin allowlist exceeds its fixed {}-entry bound",
            iroha_config::parameters::defaults::soracloud_runtime::hf::IMPORT_REDIRECT_ALLOWED_ORIGINS_LIMIT
        )));
    }
    let mut allowed = BTreeSet::new();
    for base in [&config.hub_base_url, &config.api_base_url] {
        allowed.insert(hf_profile_url_origin(&normalize_hf_base_url(base)?)?);
    }
    for raw in &config.import_redirect_allowed_origins {
        let url = reqwest::Url::parse(raw).map_err(|err| {
            SoracloudError::bad_request(format!(
                "invalid Hugging Face redirect origin `{raw}`: {err}"
            ))
        })?;
        validate_hf_profile_url_transport(&url)?;
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(SoracloudError::bad_request(format!(
                "Hugging Face redirect allowlist entry `{raw}` must be one exact origin"
            )));
        }
        let origin = hf_profile_url_origin(&url)?;
        if origin != raw.trim_end_matches('/') {
            return Err(SoracloudError::bad_request(format!(
                "Hugging Face redirect allowlist entry `{raw}` is not canonical; expected `{origin}`"
            )));
        }
        allowed.insert(origin);
    }
    Ok(allowed)
}
fn hf_profile_ipv4_is_public(address: Ipv4Addr) -> bool {
    let [first, second, third, _] = address.octets();
    !address.is_private()
        && !address.is_loopback()
        && !address.is_link_local()
        && !address.is_broadcast()
        && !address.is_documentation()
        && !address.is_unspecified()
        && !address.is_multicast()
        && first != 0
        && !(first == 100 && (64..=127).contains(&second))
        && !(first == 192 && second == 0 && third == 0)
        && !(first == 192 && second == 88 && third == 99)
        && !(first == 198 && (18..=19).contains(&second))
        && first < 240
}
fn hf_profile_ipv6_is_public(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return hf_profile_ipv4_is_public(mapped);
    }
    let segments = address.segments();
    let global_unicast = segments[0] & 0xe000 == 0x2000;
    let documentation = (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x3fff && segments[1] & 0xf000 == 0);
    let special_purpose = segments[0] == 0x2001 && segments[1] <= 0x01ff;
    let six_to_four = segments[0] == 0x2002;
    global_unicast
        && !documentation
        && !special_purpose
        && !six_to_four
        && !address.is_loopback()
        && !address.is_unspecified()
        && !address.is_multicast()
}
fn hf_profile_ip_is_allowed(address: IpAddr) -> bool {
    #[cfg(test)]
    if address.is_loopback() {
        return true;
    }
    match address {
        IpAddr::V4(address) => hf_profile_ipv4_is_public(address),
        IpAddr::V6(address) => hf_profile_ipv6_is_public(address),
    }
}
async fn resolve_hf_profile_socket_addrs(
    url: &reqwest::Url,
    timeout: Duration,
) -> Result<Vec<SocketAddr>, SoracloudError> {
    let hostname = url
        .host_str()
        .ok_or_else(|| SoracloudError::bad_request("Hugging Face profile URL has no host"))?
        .to_owned();
    let port = url.port_or_known_default().ok_or_else(|| {
        SoracloudError::bad_request("Hugging Face profile URL has no effective port")
    })?;
    if let Ok(address) = hostname.parse::<IpAddr>() {
        if !hf_profile_ip_is_allowed(address) {
            return Err(SoracloudError::bad_request(
                "Hugging Face profile URL targets a non-public address",
            ));
        }
        return Ok(vec![SocketAddr::new(address, port)]);
    }
    let timeout = timeout.min(HF_PROFILE_DNS_TIMEOUT_V1);
    let permit = tokio::time::timeout(timeout, HF_PROFILE_DNS_GATE_V1.acquire())
        .await
        .map_err(|_| SoracloudError::internal("Hugging Face DNS admission timed out"))?
        .map_err(|_| SoracloudError::internal("Hugging Face DNS admission gate closed"))?;
    let lookup_hostname = hostname.clone();
    let lookup = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut addresses = Vec::new();
        let resolved = (lookup_hostname.as_str(), port)
            .to_socket_addrs()
            .map_err(|err| err.to_string())?;
        for address in resolved {
            if addresses.len() == HF_PROFILE_DNS_MAX_ADDRESSES_V1 {
                return Err(format!(
                    "DNS answer exceeds the fixed {HF_PROFILE_DNS_MAX_ADDRESSES_V1}-address bound"
                ));
            }
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        Ok::<_, String>(addresses)
    });
    let addresses = tokio::time::timeout(timeout, lookup)
        .await
        .map_err(|_| SoracloudError::internal("Hugging Face DNS lookup timed out"))?
        .map_err(|err| SoracloudError::internal(format!("Hugging Face DNS task failed: {err}")))?
        .map_err(|err| {
            SoracloudError::internal(format!("Hugging Face DNS lookup failed: {err}"))
        })?;
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !hf_profile_ip_is_allowed(address.ip()))
    {
        return Err(SoracloudError::bad_request(format!(
            "Hugging Face origin `{hostname}` did not resolve exclusively to bounded public addresses"
        )));
    }
    Ok(addresses)
}
async fn send_hf_profile_request_with_vetted_redirects(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    method: reqwest::Method,
    initial_url: reqwest::Url,
) -> Result<reqwest::Response, SoracloudError> {
    if method != reqwest::Method::GET && method != reqwest::Method::HEAD {
        return Err(SoracloudError::bad_request(
            "Hugging Face profile derivation permits only GET and HEAD",
        ));
    }
    let allowed_origins = hf_profile_allowed_redirect_origins(config)?;
    let mut pinned_addresses = BTreeMap::<(String, u16), Vec<SocketAddr>>::new();
    let mut url = initial_url;
    for redirect_count in
        0..=iroha_config::parameters::defaults::soracloud_runtime::hf::IMPORT_MAX_REDIRECTS
    {
        let origin = hf_profile_url_origin(&url)?;
        if !allowed_origins.contains(&origin) {
            return Err(SoracloudError::bad_request(format!(
                "Hugging Face profile redirect targeted unapproved origin `{origin}`"
            )));
        }
        let hostname = url
            .host_str()
            .ok_or_else(|| SoracloudError::bad_request("Hugging Face profile URL has no host"))?
            .to_owned();
        let port = url.port_or_known_default().ok_or_else(|| {
            SoracloudError::bad_request("Hugging Face profile URL has no effective port")
        })?;
        let address_key = (hostname.clone(), port);
        let addresses = if let Some(addresses) = pinned_addresses.get(&address_key) {
            addresses.clone()
        } else {
            let addresses = resolve_hf_profile_socket_addrs(&url, config.request_timeout).await?;
            pinned_addresses.insert(address_key, addresses.clone());
            addresses
        };
        let client = hf_profile_http_client(config, &hostname, &addresses)?;
        // Build each hop from only method and URL, so no authorization or
        // cookie header can cross a redirect origin boundary.
        let response = client
            .request(method.clone(), url.clone())
            .send()
            .await
            .map_err(|err| {
                SoracloudError::internal(format!(
                    "failed to fetch Hugging Face profile resource from {url}: {err}"
                ))
            })?;
        let status = response.status();
        if !matches!(
            status,
            reqwest::StatusCode::MOVED_PERMANENTLY
                | reqwest::StatusCode::FOUND
                | reqwest::StatusCode::SEE_OTHER
                | reqwest::StatusCode::TEMPORARY_REDIRECT
                | reqwest::StatusCode::PERMANENT_REDIRECT
        ) {
            return Ok(response);
        }
        if redirect_count
            == iroha_config::parameters::defaults::soracloud_runtime::hf::IMPORT_MAX_REDIRECTS
        {
            return Err(SoracloudError::conflict(
                "Hugging Face profile request exceeded its fixed redirect bound",
            ));
        }
        let mut locations = response.headers().get_all(reqwest::header::LOCATION).iter();
        let location = locations
            .next()
            .ok_or_else(|| SoracloudError::conflict("Hugging Face redirect omitted Location"))?
            .to_str()
            .map_err(|_| {
                SoracloudError::conflict("Hugging Face redirect Location is not visible ASCII")
            })?;
        if locations.next().is_some() {
            return Err(SoracloudError::conflict(
                "Hugging Face redirect returned multiple Location headers",
            ));
        }
        url = url.join(location).map_err(|err| {
            SoracloudError::conflict(format!(
                "invalid Hugging Face redirect Location `{location}`: {err}"
            ))
        })?;
    }
    unreachable!("bounded Hugging Face redirect loop always returns or errors")
}
fn hf_model_info_url(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<reqwest::Url, SoracloudError> {
    parse_hf_repo_id(repo_id)?;
    parse_hf_revision(resolved_revision)?;
    let mut url = normalize_hf_base_url(&config.api_base_url)?;
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
    // Immutable LFS SHA-256 and size metadata is present only when the Hub is
    // explicitly asked to expand blob records.
    url.query_pairs_mut().append_pair("blobs", "true");
    Ok(url)
}
fn hf_repo_file_url(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
    file_path: &str,
) -> Result<reqwest::Url, SoracloudError> {
    parse_hf_repo_id(repo_id)?;
    parse_hf_revision(resolved_revision)?;
    let mut url = normalize_hf_base_url(&config.hub_base_url)?;
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
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
    file_path: &str,
    expected_lfs_size: u64,
) -> Result<(), SoracloudError> {
    let file_url = hf_repo_file_url(config, repo_id, resolved_revision, file_path)?;
    let response = send_hf_profile_request_with_vetted_redirects(
        config,
        reqwest::Method::HEAD,
        file_url.clone(),
    )
    .await?;
    if !response.status().is_success() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` returned {}",
            response.status()
        )));
    }
    validate_hf_content_length_headers(
        response.headers(),
        repo_id,
        resolved_revision,
        file_path,
        expected_lfs_size,
    )
}
fn validate_hf_content_length_headers(
    headers: &reqwest::header::HeaderMap,
    repo_id: &str,
    resolved_revision: &str,
    file_path: &str,
    expected_lfs_size: u64,
) -> Result<(), SoracloudError> {
    let mut lengths = headers.get_all(reqwest::header::CONTENT_LENGTH).iter();
    let length = lengths
        .next()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` is missing Content-Length"
            ))
        })?
        .to_str()
        .map_err(|_| {
            SoracloudError::conflict(format!(
                "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` has invalid Content-Length"
            ))
        })?;
    if length.is_empty()
        || (length.len() > 1 && length.starts_with('0'))
        || !length.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` has noncanonical Content-Length"
        )));
    }
    let length = length.parse::<u64>().map_err(|_| {
        SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` has invalid Content-Length"
        ))
    })?;
    if lengths.next().is_some() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` has duplicate Content-Length headers"
        )));
    }
    if length != expected_lfs_size {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face file `{file_path}` for `{repo_id}@{resolved_revision}` reports {length} bytes, but authenticated LFS metadata commits to {expected_lfs_size}"
        )));
    }
    Ok(())
}
fn validate_hf_profile_model_info_identity(
    model_info: &norito::json::Value,
    expected_repo_id: &str,
    expected_commit: &str,
) -> Result<(), SoracloudError> {
    if !is_canonical_hf_repo_id_v1(expected_repo_id) {
        return Err(SoracloudError::bad_request(
            "requested Hugging Face repository identifier is not canonical",
        ));
    }
    let resolved_repo_id = model_info
        .get("modelId")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            SoracloudError::conflict("Hugging Face model-info response omitted string `modelId`")
        })?;
    if !is_canonical_hf_repo_id_v1(resolved_repo_id) {
        return Err(SoracloudError::conflict(
            "Hugging Face model-info `modelId` is not canonical",
        ));
    }
    if resolved_repo_id != expected_repo_id {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model-info repository `{resolved_repo_id}` does not exactly match requested repository `{expected_repo_id}`"
        )));
    }
    if !is_canonical_hf_commit_oid_v1(expected_commit) {
        return Err(SoracloudError::bad_request(
            "requested Hugging Face revision is not a full lowercase commit OID",
        ));
    }
    let resolved_commit = model_info
        .get("sha")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            SoracloudError::conflict("Hugging Face model-info response omitted string `sha`")
        })?;
    if !is_canonical_hf_commit_oid_v1(resolved_commit) {
        return Err(SoracloudError::conflict(
            "Hugging Face model-info `sha` is not a full lowercase commit OID",
        ));
    }
    if resolved_commit != expected_commit {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model-info commit `{resolved_commit}` does not match requested commit `{expected_commit}`"
        )));
    }
    Ok(())
}
async fn derive_hf_resource_profile(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<SoraHfResourceProfileV1, SoracloudError> {
    tokio::time::timeout(
        config.request_timeout,
        async {
            let _permit = HF_PROFILE_DERIVATION_GATE_V1.acquire().await.map_err(|_| {
                SoracloudError::internal("Hugging Face profile admission gate closed")
            })?;
            derive_hf_resource_profile_within_deadline(config, repo_id, resolved_revision).await
        },
    )
    .await
    .map_err(|_| {
        SoracloudError::internal(format!(
            "Hugging Face profile derivation for `{repo_id}@{resolved_revision}` exceeded its fixed end-to-end deadline"
        ))
    })?
}
async fn derive_hf_resource_profile_within_deadline(
    config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<SoraHfResourceProfileV1, SoracloudError> {
    let info_url = hf_model_info_url(config, repo_id, resolved_revision)?;
    let response = send_hf_profile_request_with_vetted_redirects(
        config,
        reqwest::Method::GET,
        info_url.clone(),
    )
    .await?;
    if !response.status().is_success() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info request for `{repo_id}@{resolved_revision}` returned {}",
            response.status()
        )));
    }
    let body = hf_model_info_response::read(response, config, repo_id, resolved_revision).await?;
    let model_info = hf_model_info_response::decode(&body, config, repo_id, resolved_revision)?;
    // The decoded tree owns its strings, so the bounded wire buffer need not
    // remain resident while the provider-controlled sibling list is handled.
    drop(body);
    validate_hf_profile_model_info_identity(&model_info, repo_id, resolved_revision)?;
    let Some(mut weight_selection) = hf_model_info_response::derive_weight_selection(
        &model_info,
        config,
        repo_id,
        resolved_revision,
    )?
    else {
        return Err(SoracloudError::conflict(format!(
            "no supported Hugging Face model weights were found for `{repo_id}@{resolved_revision}`"
        )));
    };
    // Release the decoded metadata before issuing one HEAD request per selected
    // weight file; the immutable LFS contract now owns every required field.
    drop(model_info);
    let selected_weight_file_count = u32::try_from(weight_selection.required_weight_files.len())
        .map_err(|_| SoracloudError::internal("selected HF weight count does not fit u32"))?;
    let required_weight_files = std::mem::take(&mut weight_selection.required_weight_files);
    stream::iter(required_weight_files)
        .map(|weight| async move {
            hf_content_length_bytes(
                config,
                repo_id,
                resolved_revision,
                &weight.path,
                weight.content_length,
            )
            .await
        })
        .buffer_unordered(HF_PROFILE_HEAD_MAX_IN_FLIGHT_V1)
        .try_collect::<Vec<()>>()
        .await?;
    let required_model_bytes = weight_selection.required_model_bytes;
    let disk_cache_bytes_floor = required_model_bytes;
    let ram_bytes_floor = match weight_selection.backend_family {
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
        backend_family: weight_selection.backend_family,
        model_format: weight_selection.model_format,
        selected_weight_file_count,
        weight_selection_commitment: weight_selection.weight_selection_commitment,
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
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload)
        .map_err(|_| SoracloudError::bad_request(signature_error))?;
    Ok(())
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
    let payload = encode_bundle_signature_payload(
        bundle,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &SoraServiceMutationPreconditionV1::ServiceAbsent,
    )?;
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
        HF_GENERATED_AGENT_LEASE_BLOCKS,
        HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS,
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
            precondition: SoraServiceMutationPreconditionV1::ServiceAbsent,
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
            lease_blocks: HF_GENERATED_AGENT_LEASE_BLOCKS,
            autonomy_budget_units: HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS,
            provenance,
        },
    )))
}
fn parse_training_job_id(job_id: &str) -> Result<String, SoracloudError> {
    if job_id.is_empty() {
        return Err(SoracloudError::bad_request("job_id must not be empty"));
    }
    if job_id.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "job_id exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if job_id.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "job_id must not contain control characters",
        ));
    }
    if job_id.chars().any(char::is_whitespace) {
        return Err(SoracloudError::bad_request(
            "job_id must not contain whitespace",
        ));
    }
    if !job_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(SoracloudError::bad_request(
            "job_id must use only ASCII letters, digits, or [-_.:#]",
        ));
    }
    Ok(job_id.to_owned())
}
fn parse_model_weight_version(weight_version: &str) -> Result<String, SoracloudError> {
    if weight_version.is_empty() {
        return Err(SoracloudError::bad_request(
            "weight_version must not be empty",
        ));
    }
    if weight_version.len() > TRAINING_MAX_IDENTIFIER_BYTES {
        return Err(SoracloudError::bad_request(format!(
            "weight_version exceeds max bytes ({TRAINING_MAX_IDENTIFIER_BYTES})"
        )));
    }
    if weight_version.chars().any(char::is_control) {
        return Err(SoracloudError::bad_request(
            "weight_version must not contain control characters",
        ));
    }
    if weight_version.chars().any(char::is_whitespace) {
        return Err(SoracloudError::bad_request(
            "weight_version must not contain whitespace",
        ));
    }
    if !weight_version
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(SoracloudError::bad_request(
            "weight_version must use only ASCII letters, digits, or [-_.:#]",
        ));
    }
    Ok(weight_version.to_owned())
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
        chunk_manifest_root: artifact.chunk_manifest_root,
    }
}
fn wallet_day_bucket(block_timestamp_ms: u64) -> u64 {
    block_timestamp_ms / 86_400_000
}
fn rollout_handle(service_name: &str, sequence: u64) -> String {
    format!("{service_name}:rollout:{sequence}")
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
        SoracloudAction::FhePolicyRegister => "fhe_policy_register",
        SoracloudAction::FhePolicyRotate => "fhe_policy_rotate",
        SoracloudAction::FhePolicyRevoke => "fhe_policy_revoke",
        SoracloudAction::DecryptionRequest => "decryption_request",
        SoracloudAction::CiphertextQuery => "ciphertext_query",
        SoracloudAction::Rollout => "rollout",
        SoracloudAction::LeaseUsage => "lease_usage",
        SoracloudAction::LeaseReportingEpochRollover => "lease_reporting_epoch_rollover",
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
            event.process_generation,
            event.config_generation,
            event.secret_generation,
            event.config_snapshot_hash,
            event.secret_snapshot_hash,
        ),
        (
            (
                event.binding_name.as_deref(),
                event.state_key.as_deref(),
                event.config_mutations.clone(),
                event.secret_mutations.clone(),
                event.governance_tx_hash,
                event.rollout_state.clone(),
            ),
            (
                event.policy_name.as_deref(),
                event.policy_snapshot_hash,
                event.jurisdiction_tag.as_deref(),
                event.consent_evidence_hash,
                event.break_glass,
                event.break_glass_reason.as_deref(),
                event.lease_usage.clone(),
                event.service_lease_commitment,
                event.lease_reporting_epoch_rollover.clone(),
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
        &request.precondition,
    )?;
    verify_signature_for_signer(
        &request.provenance.signature,
        &request.provenance.signer,
        &payload,
    )
    .map_err(|_| SoracloudError::unauthorized("bundle provenance signature verification failed"))?;
    Ok(())
}
fn verify_app_infra_signature(request: &SignedAppInfraRequest) -> Result<(), SoracloudError> {
    request
        .manifest
        .validate()
        .map_err(|err| SoracloudError::bad_request(err.to_string()))?;
    let payload = encode_app_infra_provenance_payload(&request.manifest, &request.precondition)
        .map_err(|err| {
            SoracloudError::internal(format!("failed to encode app infra payload: {err}"))
        })?;
    verify_signature_for_signer(
        &request.provenance.signature,
        &request.provenance.signer,
        &payload,
    )
    .map_err(|_| {
        SoracloudError::unauthorized("app infra provenance signature verification failed")
    })?;
    Ok(())
}

macro_rules! define_provenance_signature_verifiers {
    ($(
        $verify_fn:ident(
            $request_ty:ty,
            $payload_field:ident,
            $encode_fn:ident,
            $failure:literal
        );
    )*) => {
        $(
            fn $verify_fn(request: &$request_ty) -> Result<(), SoracloudError> {
                let payload = $encode_fn(&request.$payload_field)?;
                verify_signature_for_signer(
                    &request.provenance.signature,
                    &request.provenance.signer,
                    &payload,
                )
                .map_err(|_| SoracloudError::unauthorized($failure))?;
                Ok(())
            }
        )*
    };
}

define_provenance_signature_verifiers! {
    verify_rollback_signature(
        SignedRollbackRequest,
        payload,
        encode_rollback_signature_payload,
        "rollback provenance signature verification failed"
    );
    verify_service_config_set_signature(
        SignedServiceConfigSetRequest,
        payload,
        encode_service_config_set_signature_payload,
        "service config provenance signature verification failed"
    );
    verify_service_config_delete_signature(
        SignedServiceConfigDeleteRequest,
        payload,
        encode_service_config_delete_signature_payload,
        "service config delete provenance signature verification failed"
    );
    verify_service_secret_set_signature(
        SignedServiceSecretSetRequest,
        payload,
        encode_service_secret_set_signature_payload,
        "service secret provenance signature verification failed"
    );
    verify_service_secret_delete_signature(
        SignedServiceSecretDeleteRequest,
        payload,
        encode_service_secret_delete_signature_payload,
        "service secret delete provenance signature verification failed"
    );
    verify_state_mutation_signature(
        SignedStateMutationRequest,
        payload,
        encode_state_mutation_signature_payload,
        "state mutation provenance signature verification failed"
    );
    verify_fhe_job_run_signature(
        SignedFheJobRunRequest,
        payload,
        encode_fhe_job_run_signature_payload,
        "fhe job run provenance signature verification failed"
    );
    verify_training_job_start_signature(
        SignedTrainingJobStartRequest,
        payload,
        encode_training_job_start_signature_payload,
        "training job start provenance signature verification failed"
    );
    verify_training_job_checkpoint_signature(
        SignedTrainingJobCheckpointRequest,
        payload,
        encode_training_job_checkpoint_signature_payload,
        "training checkpoint provenance signature verification failed"
    );
    verify_training_job_retry_signature(
        SignedTrainingJobRetryRequest,
        payload,
        encode_training_job_retry_signature_payload,
        "training retry provenance signature verification failed"
    );
    verify_model_weight_register_signature(
        SignedModelWeightRegisterRequest,
        payload,
        encode_model_weight_register_signature_payload,
        "model weight register provenance signature verification failed"
    );
    verify_model_weight_promote_signature(
        SignedModelWeightPromoteRequest,
        payload,
        encode_model_weight_promote_signature_payload,
        "model weight promote provenance signature verification failed"
    );
    verify_model_weight_rollback_signature(
        SignedModelWeightRollbackRequest,
        payload,
        encode_model_weight_rollback_signature_payload,
        "model weight rollback provenance signature verification failed"
    );
    verify_model_artifact_register_signature(
        SignedModelArtifactRegisterRequest,
        payload,
        encode_model_artifact_register_signature_payload,
        "model artifact register provenance signature verification failed"
    );
    verify_decryption_request_signature(
        SignedDecryptionRequest,
        payload,
        encode_decryption_request_signature_payload,
        "decryption request provenance signature verification failed"
    );
    verify_ciphertext_query_signature(
        SignedCiphertextQueryRequest,
        query,
        encode_ciphertext_query_signature_payload,
        "ciphertext query provenance signature verification failed"
    );
    verify_rollout_signature(
        SignedRolloutAdvanceRequest,
        payload,
        encode_rollout_signature_payload,
        "rollout provenance signature verification failed"
    );
    verify_agent_deploy_signature(
        SignedAgentDeployRequest,
        payload,
        encode_agent_deploy_signature_payload,
        "agent deploy provenance signature verification failed"
    );
    verify_agent_lease_renew_signature(
        SignedAgentLeaseRenewRequest,
        payload,
        encode_agent_lease_renew_signature_payload,
        "agent lease renew provenance signature verification failed"
    );
    verify_agent_restart_signature(
        SignedAgentRestartRequest,
        payload,
        encode_agent_restart_signature_payload,
        "agent restart provenance signature verification failed"
    );
    verify_agent_policy_revoke_signature(
        SignedAgentPolicyRevokeRequest,
        payload,
        encode_agent_policy_revoke_signature_payload,
        "agent policy revoke provenance signature verification failed"
    );
    verify_agent_wallet_spend_signature(
        SignedAgentWalletSpendRequest,
        payload,
        encode_agent_wallet_spend_signature_payload,
        "agent wallet spend provenance signature verification failed"
    );
    verify_agent_wallet_approve_signature(
        SignedAgentWalletApproveRequest,
        payload,
        encode_agent_wallet_approve_signature_payload,
        "agent wallet approve provenance signature verification failed"
    );
    verify_agent_message_send_signature(
        SignedAgentMessageSendRequest,
        payload,
        encode_agent_message_send_signature_payload,
        "agent message send provenance signature verification failed"
    );
    verify_agent_message_ack_signature(
        SignedAgentMessageAckRequest,
        payload,
        encode_agent_message_ack_signature_payload,
        "agent message ack provenance signature verification failed"
    );
    verify_agent_artifact_allow_signature(
        SignedAgentArtifactAllowRequest,
        payload,
        encode_agent_artifact_allow_signature_payload,
        "agent artifact allow provenance signature verification failed"
    );
    verify_agent_autonomy_run_signature(
        SignedAgentAutonomyRunRequest,
        payload,
        encode_agent_autonomy_run_signature_payload,
        "agent autonomy run provenance signature verification failed"
    );
    verify_hf_deploy_signature(
        SignedHfDeployRequest,
        payload,
        encode_hf_deploy_signature_payload,
        "hf deploy provenance signature verification failed"
    );
    verify_hf_lease_leave_signature(
        SignedHfLeaseLeaveRequest,
        payload,
        encode_hf_lease_leave_signature_payload,
        "hf shared-lease leave provenance signature verification failed"
    );
    verify_hf_lease_renew_signature(
        SignedHfLeaseRenewRequest,
        payload,
        encode_hf_lease_renew_signature_payload,
        "hf shared-lease renew provenance signature verification failed"
    );
    verify_model_host_advertise_signature(
        SignedModelHostAdvertiseRequest,
        payload,
        encode_model_host_advertise_signature_payload,
        "model host advertise provenance signature verification failed"
    );
    verify_model_host_heartbeat_signature(
        SignedModelHostHeartbeatRequest,
        payload,
        encode_model_host_heartbeat_signature_payload,
        "model host heartbeat provenance signature verification failed"
    );
    verify_model_host_withdraw_signature(
        SignedModelHostWithdrawRequest,
        payload,
        encode_model_host_withdraw_signature_payload,
        "model host withdraw provenance signature verification failed"
    );
}

fn app_service_bundle_instruction(
    request: SignedBundleRequest,
    mode: MutationMode,
    app_signer: &PublicKey,
) -> Result<InstructionBox, SoracloudError> {
    if &request.provenance.signer != app_signer {
        return Err(SoracloudError::unauthorized(
            "app infra service bundle signer must match the app infra provenance signer",
        ));
    }
    verify_bundle_signature(&request)?;
    admit_scr_host_bundle(&request.bundle)?;
    Ok(match mode {
        MutationMode::Deploy => InstructionBox::from(isi::soracloud::DeploySoracloudService {
            bundle: request.bundle,
            initial_service_configs: request.initial_service_configs,
            initial_service_secrets: request.initial_service_secrets,
            precondition: request.precondition,
            provenance: request.provenance,
        }),
        MutationMode::Upgrade => InstructionBox::from(isi::soracloud::UpgradeSoracloudService {
            bundle: request.bundle,
            initial_service_configs: request.initial_service_configs,
            initial_service_secrets: request.initial_service_secrets,
            precondition: request.precondition,
            provenance: request.provenance,
        }),
    })
}
fn encode_bundle_signature_payload(
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    initial_service_secrets: &BTreeMap<String, SecretEnvelopeV1>,
    precondition: &SoraServiceMutationPreconditionV1,
) -> Result<Vec<u8>, SoracloudError> {
    iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
        bundle,
        initial_service_configs,
        initial_service_secrets,
        precondition,
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode bundle payload: {err}")))
}
fn encode_rollback_signature_payload(payload: &RollbackPayload) -> Result<Vec<u8>, SoracloudError> {
    encode_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.target_version.as_str(),
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode rollback payload: {err}")))
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
fn encode_state_mutation_signature_payload(
    payload: &StateMutationRequest,
) -> Result<Vec<u8>, SoracloudError> {
    let (value_size_bytes, payload_commitment) = state_mutation_payload_metadata(payload)?;
    encode_state_mutation_provenance_payload(
        payload.service_name.as_str(),
        payload.binding_name.as_str(),
        payload.key.as_str(),
        state_mutation_operation_label(payload.operation),
        value_size_bytes,
        payload_commitment,
        payload.encryption,
        payload.governance_tx_hash,
        payload.fhe_input_admission_proof.clone(),
    )
    .map_err(|err| {
        SoracloudError::internal(format!("failed to encode state mutation payload: {err}"))
    })
}
fn decode_state_mutation_payload(
    payload: &StateMutationRequest,
) -> Result<Option<Vec<u8>>, SoracloudError> {
    match payload.operation {
        StateMutationOperation::Upsert => {
            let Some(payload_hex) = payload.value_payload_hex.as_deref() else {
                return Err(SoracloudError::bad_request(
                    "value_payload_hex is required for state upserts",
                ));
            };
            let bytes = hex::decode(payload_hex).map_err(|err| {
                SoracloudError::bad_request(format!("invalid value_payload_hex: {err}"))
            })?;
            if bytes.is_empty() {
                return Err(SoracloudError::bad_request(
                    "value_payload_hex must encode a non-empty payload",
                ));
            }
            let actual_size = u64::try_from(bytes.len())
                .map_err(|_| SoracloudError::bad_request("value_payload_hex is too large"))?;
            if let Some(declared_size) = payload.value_size_bytes
                && declared_size != actual_size
            {
                return Err(SoracloudError::bad_request(format!(
                    "value_size_bytes {declared_size} does not match value_payload_hex length {actual_size}",
                )));
            }
            Ok(Some(bytes))
        }
        StateMutationOperation::Delete => {
            if payload.value_size_bytes.is_some() || payload.value_payload_hex.is_some() {
                return Err(SoracloudError::bad_request(
                    "delete mutations must not include value_size_bytes or value_payload_hex",
                ));
            }
            Ok(None)
        }
    }
}
fn state_mutation_payload_metadata(
    payload: &StateMutationRequest,
) -> Result<(Option<u64>, Option<Hash>), SoracloudError> {
    Ok(match decode_state_mutation_payload(payload)? {
        Some(bytes) => (
            Some(
                u64::try_from(bytes.len())
                    .map_err(|_| SoracloudError::bad_request("value_payload_hex is too large"))?,
            ),
            Some(Hash::new(&bytes)),
        ),
        None => (None, None),
    })
}
fn state_mutation_operation_label(operation: StateMutationOperation) -> &'static str {
    match operation {
        StateMutationOperation::Upsert => "upsert",
        StateMutationOperation::Delete => "delete",
    }
}
fn validate_fhe_job_run_proof_attachments(
    payload: &FheJobRunPayload,
) -> Result<(), SoracloudError> {
    payload
        .policy_reference
        .validate()
        .map_err(|err| SoracloudError::bad_request(format!("invalid policy_reference: {err}")))?;
    if let Some(proof) = &payload.public_key_proof {
        proof.validate().map_err(|err| {
            SoracloudError::bad_request(format!("invalid public_key_proof: {err}"))
        })?;
    }
    if let Some(proof) = &payload.bootstrap_key_zero_refresh_proof {
        proof.validate().map_err(|err| {
            SoracloudError::bad_request(format!("invalid bootstrap_key_zero_refresh_proof: {err}"))
        })?;
    }
    for (index, proof) in payload.full_bootstrap_execution_proofs.iter().enumerate() {
        proof.validate().map_err(|err| {
            SoracloudError::bad_request(format!(
                "invalid full_bootstrap_execution_proofs[{index}]: {err}"
            ))
        })?;
    }
    Ok(())
}
fn verify_uploaded_model_register_signature(
    request: &SignedUploadedModelRegisterRequest,
) -> Result<(), SoracloudError> {
    if request.bundle_provenance.signer != request.finalize_provenance.signer {
        return Err(SoracloudError::unauthorized(
            "uploaded model register bundle and finalize provenance signers must match",
        ));
    }
    let bundle_payload = encode_uploaded_model_register_bundle_signature_payload(&request.payload)?;
    verify_signature_for_signer(
        &request.bundle_provenance.signature,
        &request.bundle_provenance.signer,
        &bundle_payload,
    )
    .map_err(|_| {
        SoracloudError::unauthorized(
            "uploaded model register bundle provenance signature verification failed",
        )
    })?;
    let finalize_payload =
        encode_uploaded_model_register_finalize_signature_payload(&request.payload)?;
    verify_signature_for_signer(
        &request.finalize_provenance.signature,
        &request.finalize_provenance.signer,
        &finalize_payload,
    )
    .map_err(|_| {
        SoracloudError::unauthorized(
            "uploaded model register finalize provenance signature verification failed",
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
        payload.policy_reference.clone(),
        payload.public_key_proof.clone(),
        payload.bootstrap_key_zero_refresh_proof.clone(),
        payload.full_bootstrap_execution_proofs.clone(),
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
fn encode_uploaded_model_register_bundle_signature_payload(
    payload: &UploadedModelRegisterPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_bundle_register_provenance_payload(payload.bundle.clone()).map_err(
        |err| {
            SoracloudError::internal(format!(
                "failed to encode uploaded model register bundle payload: {err}"
            ))
        },
    )
}
fn encode_uploaded_model_register_finalize_signature_payload(
    payload: &UploadedModelRegisterPayload,
) -> Result<Vec<u8>, SoracloudError> {
    encode_uploaded_model_finalize_provenance_payload(
        payload.bundle.service_name.as_ref(),
        payload.model_name.as_str(),
        payload.bundle.model_id.as_str(),
        payload.artifact_id.as_str(),
        payload.bundle.weight_version.as_str(),
        payload.bundle.bundle_root,
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode uploaded model register finalize payload: {err}"
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
        payload.lease_blocks,
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
        payload.lease_blocks,
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
        &payload.amount,
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
    let resolved_revision = parse_hf_revision(&payload.revision)?;
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
    if payload.base_fee.is_zero() {
        return Err(SoracloudError::bad_request(
            "base_fee must be greater than zero",
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
        &payload.base_fee,
    )
    .map_err(|err| SoracloudError::internal(format!("failed to encode hf deploy payload: {err}")))
}
fn encode_hf_lease_leave_signature_payload(
    payload: &HfLeaseLeavePayload,
) -> Result<Vec<u8>, SoracloudError> {
    let repo_id = parse_hf_repo_id(&payload.repo_id)?;
    let resolved_revision = parse_hf_revision(&payload.revision)?;
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
    let resolved_revision = parse_hf_revision(&payload.revision)?;
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
    if payload.base_fee.is_zero() {
        return Err(SoracloudError::bad_request(
            "base_fee must be greater than zero",
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
        &payload.base_fee,
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
#[norito(deny_unknown_fields)]
struct SoracloudMutationDraftResponse {
    ok: bool,
    authority: String,
    signed_by: String,
    tx_instructions: Vec<SoracloudTxInstr>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SoracloudTxInstr {
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
) -> Result<(AccountId, PublicKey, Vec<PublicKey>), SoracloudError> {
    let account = headers
        .get(VERIFIED_ACCOUNT_HEADER)
        .ok_or_else(|| {
            SoracloudError::unauthorized(
                "signed request headers are required for Soracloud mutation endpoints",
            )
        })
        .and_then(|value| {
            std::str::from_utf8(value.as_bytes()).map_err(|_| {
                SoracloudError::internal(
                    "failed to decode verified Soracloud account header".to_owned(),
                )
            })
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
    let verified_signers = match headers.get(VERIFIED_SIGNERS_HEADER) {
        Some(value) => {
            let literal = value.to_str().map(str::trim).map_err(|_| {
                SoracloudError::internal(
                    "failed to parse verified Soracloud signer-set header".to_owned(),
                )
            })?;
            let decoded = BASE64_STANDARD.decode(literal).map_err(|_| {
                SoracloudError::internal(
                    "failed to decode verified Soracloud signer-set header".to_owned(),
                )
            })?;
            let signers: Vec<PublicKey> = norito::decode_from_bytes(&decoded).map_err(|_| {
                SoracloudError::internal(
                    "failed to decode verified Soracloud signer-set payload".to_owned(),
                )
            })?;
            if signers.is_empty() {
                return Err(SoracloudError::internal(
                    "verified Soracloud signer-set header must not be empty".to_owned(),
                ));
            }
            signers
        }
        None => vec![signer.clone()],
    };
    if !verified_signers.iter().any(|verified| verified == &signer) {
        return Err(SoracloudError::internal(
            "verified Soracloud signer-set header does not include the primary signer".to_owned(),
        ));
    }
    Ok((account, signer, verified_signers))
}
fn require_soracloud_mutation_signer(
    headers: &HeaderMap,
    provenance: &ManifestProvenance,
) -> Result<SoracloudMutationSigner, SoracloudError> {
    let (authority, _request_signer, verified_signers) =
        verified_soracloud_request_identity(headers)?;
    if !verified_signers
        .iter()
        .any(|verified_signer| verified_signer == &provenance.signer)
    {
        return Err(SoracloudError::unauthorized(
            "mutation provenance signer must match one of the verified request signers",
        ));
    }
    Ok(SoracloudMutationSigner {
        authority,
        request_signer: provenance.signer.clone(),
    })
}
fn require_soracloud_request_signer(
    headers: &HeaderMap,
) -> Result<SoracloudMutationSigner, SoracloudError> {
    let (authority, request_signer, _verified_signers) =
        verified_soracloud_request_identity(headers)?;
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
        SoraServiceLifecycleActionV1::FhePolicyRegister => SoracloudAction::FhePolicyRegister,
        SoraServiceLifecycleActionV1::FhePolicyRotate => SoracloudAction::FhePolicyRotate,
        SoraServiceLifecycleActionV1::FhePolicyRevoke => SoracloudAction::FhePolicyRevoke,
        SoraServiceLifecycleActionV1::DecryptionRequest => SoracloudAction::DecryptionRequest,
        SoraServiceLifecycleActionV1::CiphertextQuery => SoracloudAction::CiphertextQuery,
        SoraServiceLifecycleActionV1::Rollout => SoracloudAction::Rollout,
        SoraServiceLifecycleActionV1::Rollback => SoracloudAction::Rollback,
        SoraServiceLifecycleActionV1::LeaseUsage => SoracloudAction::LeaseUsage,
        SoraServiceLifecycleActionV1::LeaseReportingEpochRollover => {
            SoracloudAction::LeaseReportingEpochRollover
        }
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
        process_generation: event.process_generation,
        config_generation: event.config_generation,
        secret_generation: event.secret_generation,
        config_snapshot_hash: event.config_snapshot_hash,
        secret_snapshot_hash: event.secret_snapshot_hash,
        binding_name: event.binding_name.as_ref().map(ToString::to_string),
        state_key: event.state_key.clone(),
        config_mutations: event.config_mutations.clone(),
        secret_mutations: event.secret_mutations.clone(),
        governance_tx_hash: event.governance_tx_hash,
        rollout_state: event
            .rollout_state
            .as_ref()
            .map(rollout_state_to_runtime_state),
        policy_name: event.policy_name.as_ref().map(ToString::to_string),
        policy_snapshot_hash: event.policy_snapshot_hash,
        jurisdiction_tag: event.jurisdiction_tag.clone(),
        consent_evidence_hash: event.consent_evidence_hash,
        break_glass: event.break_glass,
        break_glass_reason: event.break_glass_reason.clone(),
        lease_usage: event.lease_usage.clone(),
        service_lease_commitment: event.service_lease_commitment,
        lease_reporting_epoch_rollover: event.lease_reporting_epoch_rollover.clone(),
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
fn authoritative_audit_log(
    app: &SharedAppState,
) -> Result<Vec<ControlPlaneAuditEvent>, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let mut audit_log = Vec::new();
    for (sequence, event) in world.soracloud_service_audit_events().iter() {
        if *sequence != event.sequence {
            return Err(SoracloudError::internal(format!(
                "authoritative Soracloud audit event key {sequence} does not bind event sequence {}",
                event.sequence
            )));
        }
        event.validate().map_err(|error| {
            SoracloudError::internal(format!(
                "authoritative Soracloud audit event {sequence} is invalid: {error}"
            ))
        })?;
        audit_log.push(audit_event_to_control_plane_audit_event(event));
    }
    audit_log.sort_by_key(|event| event.sequence);
    Ok(audit_log)
}
fn authoritative_training_job_status_response(
    app: &SharedAppState,
    service_name: &str,
    job_id: &str,
) -> Result<TrainingJobStatusResponse, SoracloudError> {
    let service_name = parse_service_name(service_name)?;
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
    let service_name = parse_service_name(service_name)?;
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
    let artifact = world.soracloud_model_artifacts().iter().find_map(
        |((stored_service, _artifact_id), record)| {
            (stored_service == &service_name
                && record.weight_version.as_deref() == Some(weight_version.as_str())
                && record.source_provenance.as_ref().is_some_and(|provenance| {
                    provenance.kind == SoraModelProvenanceKindV1::UserUpload
                        && provenance.id == model_id
                }))
            .then(|| authoritative_model_artifact_status_entry(&service_name, record))
        },
    );
    Ok(UploadedModelStatusResponse {
        schema_version: UPLOADED_MODEL_STATUS_SCHEMA_VERSION_V1,
        bundle,
        artifact,
    })
}
fn require_active_sorafs_uploaded_model_pin(
    app: &SharedAppState,
    bundle: &SoraUploadedModelBundleV1,
) -> Result<(), SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let Some(pin) = world.pin_manifests().get(&bundle.sorafs_manifest_digest) else {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} for uploaded model `{}` version `{}` is not registered",
            bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
        )));
    };
    require_active_sorafs_uploaded_model_pin_record(pin, bundle)
}
fn require_active_sorafs_uploaded_model_pin_record(
    pin: &PinManifestRecord,
    bundle: &SoraUploadedModelBundleV1,
) -> Result<(), SoracloudError> {
    match pin.status {
        PinStatus::Approved(_) => Ok(()),
        PinStatus::Pending => Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} for uploaded model `{}` version `{}` is not approved",
            bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
        ))),
        PinStatus::Retired(epoch) => Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} for uploaded model `{}` version `{}` retired at epoch {epoch}",
            bundle.sorafs_manifest_digest, bundle.model_id, bundle.weight_version
        ))),
    }?;
    if pin.digest != bundle.sorafs_manifest_digest {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest record digest {:?} does not match uploaded model digest {:?}",
            pin.digest, bundle.sorafs_manifest_digest
        )));
    }
    if pin.content_length != bundle.ciphertext_bytes {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} content_length {} does not match uploaded model ciphertext_bytes {}",
            bundle.sorafs_manifest_digest, pin.content_length, bundle.ciphertext_bytes
        )));
    }
    Ok(())
}
fn require_finalized_uploaded_model_release(
    app: &SharedAppState,
    bundle: &SoraUploadedModelBundleV1,
) -> Result<(), SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    validate_finalized_soracloud_uploaded_model_release(world, bundle)
        .map_err(SoracloudError::conflict)
}
fn require_active_private_model_artifact_pin(
    app: &SharedAppState,
    artifact: &SoraPrivateModelArtifactRefV1,
    minimum_remaining_seconds: u64,
) -> Result<PinManifestRecord, SoracloudError> {
    artifact.validate().map_err(|err| {
        SoracloudError::bad_request(format!("invalid private artifact ref: {err}"))
    })?;
    let state_view = app.state.view();
    let world = state_view.world();
    let Some(pin) = world.pin_manifests().get(&artifact.sorafs_manifest_digest) else {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} for private `{}` artifact is not registered",
            artifact.sorafs_manifest_digest, artifact.artifact_role
        )));
    };
    match pin.status {
        PinStatus::Approved(_) => {}
        PinStatus::Pending => {
            return Err(SoracloudError::conflict(format!(
                "SoraFS manifest {:?} for private `{}` artifact is not approved",
                artifact.sorafs_manifest_digest, artifact.artifact_role
            )));
        }
        PinStatus::Retired(epoch) => {
            return Err(SoracloudError::conflict(format!(
                "SoraFS manifest {:?} for private `{}` artifact retired at epoch {epoch}",
                artifact.sorafs_manifest_digest, artifact.artifact_role
            )));
        }
    }
    if pin.digest != artifact.sorafs_manifest_digest {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest record digest {:?} does not match private artifact digest {:?}",
            pin.digest, artifact.sorafs_manifest_digest
        )));
    }
    if pin.root_cid != artifact.sorafs_root_cid {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} root CID does not match the private artifact content identity",
            artifact.sorafs_manifest_digest
        )));
    }
    if pin.content_length != artifact.ciphertext_bytes {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} content_length {} does not match private artifact ciphertext_bytes {}",
            artifact.sorafs_manifest_digest, pin.content_length, artifact.ciphertext_bytes
        )));
    }
    let wall_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| SoracloudError::unavailable("system clock is before the Unix epoch"))?
        .as_secs();
    let finalized_epoch = app
        .state
        .latest_block_header_fast()
        .map_or(0, |header| header.creation_time_ms / 1_000);
    let candidate_epoch = wall_epoch.max(finalized_epoch).saturating_add(1);
    let required_retention_epoch = candidate_epoch
        .checked_add(minimum_remaining_seconds)
        .ok_or_else(|| SoracloudError::unavailable("private output pin margin overflowed"))?;
    if pin.policy.retention_epoch <= required_retention_epoch {
        return Err(SoracloudError::conflict(format!(
            "SoraFS manifest {:?} for private `{}` artifact expires before the required {minimum_remaining_seconds}-second execution and durable recovery margin",
            artifact.sorafs_manifest_digest, artifact.artifact_role,
        )));
    }
    Ok(pin.clone())
}
fn canonical_private_uploaded_model_decryption_request_id(
    request: &PrivateUploadedModelExecuteRequest,
) -> Result<&str, SoracloudError> {
    let decryption_request_id = request.decryption_request_id.as_str();
    if decryption_request_id.is_empty()
        || decryption_request_id.trim() != decryption_request_id
        || decryption_request_id.chars().any(char::is_control)
    {
        return Err(SoracloudError::bad_request(
            "decryption_request_id must be canonical and non-empty",
        ));
    }
    Ok(decryption_request_id)
}

fn require_private_uploaded_model_release_signer(
    world: &impl WorldReadOnly,
    request: &PrivateUploadedModelExecuteRequest,
    verified_request_signers: &[PublicKey],
) -> Result<SoraDecryptionRequestRecordV1, SoracloudError> {
    let decryption_request_id = canonical_private_uploaded_model_decryption_request_id(request)?;
    let record = world
        .soracloud_decryption_request_records()
        .get(&(request.service_name.clone(), decryption_request_id.to_owned()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "decryption request `{decryption_request_id}` for private execution service `{}` is not committed in authoritative state",
                request.service_name
            ))
        })?;
    record.validate().map_err(|err| {
        SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` for private execution is invalid: {err}"
        ))
    })?;
    if record.service_name.as_ref() != request.service_name.as_str() {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` service `{}` does not match private execution service `{}`",
            record.service_name, request.service_name
        )));
    }
    if record.request.request_id.as_str() != decryption_request_id {
        return Err(SoracloudError::conflict(format!(
            "decryption request record id `{}` does not match lookup id `{decryption_request_id}`",
            record.request.request_id
        )));
    }
    if !verified_request_signers
        .iter()
        .any(|signer| signer == &record.signer)
    {
        return Err(SoracloudError::unauthorized(format!(
            "private execution must be signed by the exact signer that committed decryption request `{decryption_request_id}`"
        )));
    }
    Ok(record)
}

fn require_private_uploaded_model_release_policy(
    app: &SharedAppState,
    bundle: &SoraUploadedModelBundleV1,
    request: &PrivateUploadedModelExecuteRequest,
    verified_request_signers: &[PublicKey],
    blocks_until_receipt: u64,
) -> Result<(), SoracloudError> {
    let decryption_request_id = canonical_private_uploaded_model_decryption_request_id(request)?;
    if request.service_version.is_empty()
        || request.service_version.trim() != request.service_version
        || request.service_version.chars().any(char::is_control)
        || request.service_version.len() > 256
    {
        return Err(SoracloudError::bad_request(
            "service_version must be canonical, non-empty, and at most 256 bytes",
        ));
    }
    request
        .output_recipient
        .validate()
        .map_err(|err| SoracloudError::bad_request(format!("invalid output_recipient: {err}")))?;
    let state_view = app.state.view();
    let world = state_view.world();
    let service_revision = world
        .soracloud_service_revisions()
        .get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        ))
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "service revision `{}` is not retained for private execution service `{}`",
                request.service_version, request.service_name
            ))
        })?;
    if service_revision.service.service_name.as_ref() != request.service_name
        || service_revision.service.service_version != request.service_version
        || !service_revision
            .container
            .capabilities
            .allow_model_inference
    {
        return Err(SoracloudError::conflict(format!(
            "service revision `{}` does not admit uploaded-model inference",
            request.service_version
        )));
    }
    let record =
        require_private_uploaded_model_release_signer(world, request, verified_request_signers)?;
    if record.service_version != request.service_version {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` service version `{}` does not match requested service version `{}`",
            record.service_version, request.service_version
        )));
    }
    if record.request.policy_name.as_ref() != bundle.decryption_policy_ref.as_str() {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` policy `{}` does not match private execution policy `{}`",
            record.request.policy_name, bundle.decryption_policy_ref
        )));
    }
    if record.request.ciphertext_commitment != request.input_artifact.artifact_hash {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` ciphertext commitment does not match private input artifact hash"
        )));
    }
    let event = world
        .soracloud_service_audit_events()
        .get(&record.sequence)
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "decryption request `{decryption_request_id}` is missing its exact audit event"
            ))
        })?;
    event.validate().map_err(|err| {
        SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` audit event is invalid: {err}"
        ))
    })?;
    if event.sequence != record.sequence
        || event.action != SoraServiceLifecycleActionV1::DecryptionRequest
        || event.service_name != record.service_name
        || event.from_version.is_some()
        || event.to_version != record.service_version
        || event.service_manifest_hash != service_revision.service_manifest_hash()
        || event.container_manifest_hash != service_revision.container_manifest_hash()
        || event.governance_tx_hash != Some(record.request.governance_tx_hash)
        || event.binding_name.as_ref() != Some(&record.request.binding_name)
        || event.state_key.as_deref() != Some(record.request.state_key.as_str())
        || event.policy_name.as_ref() != Some(&record.request.policy_name)
        || event.policy_snapshot_hash != Some(record.policy_snapshot_hash())
        || event.jurisdiction_tag.as_deref() != Some(record.request.jurisdiction_tag.as_str())
        || event.consent_evidence_hash != record.request.consent_evidence_hash
        || event.break_glass != Some(record.request.break_glass)
        || event.break_glass_reason.as_deref() != record.request.break_glass_reason.as_deref()
        || event.signer != record.signer
    {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` does not match its authoritative audit event"
        )));
    }
    let candidate_height = u64::try_from(state_view.height())
        .unwrap_or(u64::MAX)
        .checked_add(blocks_until_receipt)
        .ok_or_else(|| SoracloudError::unavailable("receipt candidate height overflowed"))?;
    let (expires_at_height, authorized) = private_execution_release_candidate_is_authorized(
        event.block_height,
        record.request.requested_ttl_blocks.get(),
        candidate_height,
    )
    .ok_or_else(|| {
        SoracloudError::conflict("private execution decryption-request expiry height overflowed")
    })?;
    if !authorized {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{decryption_request_id}` is outside its half-open authorization window [{}..{expires_at_height}) at receipt candidate height {candidate_height}",
            event.block_height
        )));
    }
    Ok(())
}

fn map_private_runtime_error(error: SoracloudRuntimeExecutionError) -> SoracloudError {
    match error.kind {
        SoracloudRuntimeExecutionErrorKind::Unavailable => {
            SoracloudError::unavailable(error.message)
        }
        SoracloudRuntimeExecutionErrorKind::InvalidRequest => {
            SoracloudError::bad_request(error.message)
        }
        SoracloudRuntimeExecutionErrorKind::Internal => SoracloudError::internal(error.message),
    }
}

fn read_admitted_private_model_payload(
    app: &SharedAppState,
    manifest_digest: &ManifestDigest,
    expected_length: u64,
    expected_hash: Option<Hash>,
    label: &str,
) -> Result<Vec<u8>, SoracloudError> {
    if expected_length == 0
        || expected_length
            > u64::try_from(SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1).unwrap_or(u64::MAX)
    {
        return Err(SoracloudError::bad_request(format!(
            "encrypted {label} artifact length must be in 1..={SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1} bytes"
        )));
    }
    let expected_usize = usize::try_from(expected_length).map_err(|_| {
        SoracloudError::bad_request(format!(
            "encrypted {label} artifact length does not fit this host"
        ))
    })?;
    app.sorafs_node()
        .with_admitted_payload_read_lease(manifest_digest.as_bytes(), |lease| {
            if lease.manifest_digest() != manifest_digest.as_bytes()
                || lease.content_length() != expected_length
            {
                return Err(SoracloudError::conflict(format!(
                    "locally admitted encrypted {label} artifact does not match its authoritative manifest binding"
                )));
            }
            let reader = lease.open_reader().map_err(|_| {
                SoracloudError::unavailable(format!(
                    "encrypted {label} artifact is temporarily unreadable from local SoraFS storage"
                ))
            })?;
            let mut limited = reader.take(expected_length.saturating_add(1));
            let mut bytes = Vec::with_capacity(expected_usize);
            limited.read_to_end(&mut bytes).map_err(|_| {
                SoracloudError::unavailable(format!(
                    "encrypted {label} artifact could not be read from local SoraFS storage"
                ))
            })?;
            if bytes.len() != expected_usize {
                return Err(SoracloudError::conflict(format!(
                    "encrypted {label} artifact length changed while held under its SoraFS read lease"
                )));
            }
            if expected_hash.is_some_and(|hash| Hash::new(bytes.as_slice()) != hash) {
                return Err(SoracloudError::conflict(format!(
                    "encrypted {label} artifact bytes do not match the authoritative artifact hash"
                )));
            }
            Ok(bytes)
        })
        .map_err(|_| {
            SoracloudError::unavailable(format!(
                "encrypted {label} artifact is not available in admitted local SoraFS storage"
            ))
        })?
}

struct PrivateOutputManifest {
    artifact: SoraPrivateModelArtifactRefV1,
    manifest: sorafs_manifest::ManifestV1,
    manifest_payload: Vec<u8>,
    plan: sorafs_car::CarBuildPlan,
}

fn build_private_output_manifest(
    encrypted_output: &[u8],
    input_pin: &PinManifestRecord,
) -> Result<PrivateOutputManifest, SoracloudError> {
    if encrypted_output.is_empty()
        || encrypted_output.len() > SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1
    {
        return Err(SoracloudError::internal(
            "private runtime produced an output outside the encrypted artifact byte bound",
        ));
    }
    let plan = sorafs_car::CarBuildPlan::single_file_with_profile(
        encrypted_output,
        sorafs_chunker::ChunkProfile::DEFAULT,
    )
    .map_err(|err| {
        SoracloudError::internal(format!(
            "failed to derive canonical SoraFS plan for encrypted private output: {err}"
        ))
    })?;
    let stats = sorafs_car::CarWriter::new(&plan, encrypted_output)
        .and_then(|writer| writer.write_to(std::io::sink()))
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to derive canonical SoraFS metadata for encrypted private output: {err}"
            ))
        })?;
    let root_cid = stats.root_cids.first().cloned().ok_or_else(|| {
        SoracloudError::internal("private output SoraFS plan did not produce a root CID")
    })?;
    let sorafs_root_cid = ManifestRootCid::try_from_slice(&root_cid).map_err(|err| {
        SoracloudError::internal(format!(
            "private output SoraFS plan produced a non-canonical root CID: {err}"
        ))
    })?;
    let mut car_digest = [0_u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    let storage_class = match input_pin.policy.storage_class {
        StorageClass::Hot => sorafs_manifest::StorageClass::Hot,
        StorageClass::Warm => sorafs_manifest::StorageClass::Warm,
        StorageClass::Cold => sorafs_manifest::StorageClass::Cold,
    };
    let manifest = sorafs_manifest::ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(sorafs_manifest::DagCodecId(stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(
            sorafs_car::compute_por_root(encrypted_output, &plan).map_err(|err| {
                SoracloudError::internal(format!(
                    "failed to derive SoraFS PoR root for encrypted private output: {err}"
                ))
            })?,
        )
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(sorafs_manifest::PinPolicy {
            min_replicas: input_pin.policy.min_replicas,
            storage_class,
            retention_epoch: input_pin.policy.retention_epoch,
        })
        .build()
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to build SoraFS manifest for encrypted private output: {err}"
            ))
        })?;
    let manifest_digest = ManifestDigest::from_manifest(&manifest).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to derive encrypted private output manifest digest: {err}"
        ))
    })?;
    let artifact = SoraPrivateModelArtifactRefV1 {
        schema_version: SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
        sorafs_manifest_digest: manifest_digest,
        sorafs_root_cid,
        artifact_hash: Hash::new(encrypted_output),
        ciphertext_bytes: u64::try_from(encrypted_output.len()).map_err(|_| {
            SoracloudError::internal("encrypted private output length does not fit u64")
        })?,
        artifact_role: "output".to_owned(),
    };
    artifact.validate().map_err(|err| {
        SoracloudError::internal(format!(
            "runtime produced an invalid encrypted output reference: {err}"
        ))
    })?;
    let manifest_payload = manifest.encode().map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode encrypted private output manifest: {err}"
        ))
    })?;
    Ok(PrivateOutputManifest {
        artifact,
        manifest,
        manifest_payload,
        plan,
    })
}

fn ingest_private_output_manifest(
    app: &SharedAppState,
    output: &PrivateOutputManifest,
    encrypted_output: &[u8],
) -> Result<(), SoracloudError> {
    use sorafs_node::{NodeStorageError, store::StorageError};

    let mut reader = encrypted_output;
    match app
        .sorafs_node()
        .ingest_manifest(&output.manifest, &output.plan, &mut reader)
    {
        Ok(_) => Ok(()),
        Err(NodeStorageError::Storage(StorageError::ManifestExists { .. })) => {
            let existing = read_admitted_private_model_payload(
                app,
                &output.artifact.sorafs_manifest_digest,
                output.artifact.ciphertext_bytes,
                Some(output.artifact.artifact_hash),
                "output",
            )?;
            if existing != encrypted_output {
                return Err(SoracloudError::conflict(
                    "existing deterministic private output manifest carries different bytes",
                ));
            }
            Ok(())
        }
        Err(err) => Err(SoracloudError::unavailable(format!(
            "failed to durably ingest encrypted private output into local SoraFS storage: {err}"
        ))),
    }
}

fn committed_private_execution_response(
    app: &SharedAppState,
    status: &UploadedModelStatusResponse,
    request: &PrivateUploadedModelExecuteRequest,
    verified_request_signers: &[PublicKey],
) -> Result<Option<PrivateUploadedModelExecuteResponse>, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let existing = world
        .soracloud_private_uploaded_model_execution_receipts()
        .iter()
        .find_map(|(_receipt_id, receipt)| {
            (receipt.service_name.as_ref() == request.service_name
                && receipt.decryption_request_id == request.decryption_request_id)
                .then(|| receipt.clone())
        });
    let Some(receipt) = existing else {
        return Ok(None);
    };
    // Commitment makes execution idempotent, not public: authenticate before exposing the
    // receipt or allowing the caller to clean up recovery state.
    let _authorized_record =
        require_private_uploaded_model_release_signer(world, request, verified_request_signers)?;
    receipt.validate().map_err(|err| {
        SoracloudError::internal(format!(
            "committed private execution receipt is invalid: {err}"
        ))
    })?;
    let bundle = &status.bundle;
    if receipt.service_version != request.service_version
        || receipt.model_id != bundle.model_id
        || receipt.weight_version != bundle.weight_version
        || receipt.model_manifest_digest != bundle.sorafs_manifest_digest
        || receipt.model_bundle_root != bundle.bundle_root
        || receipt.policy_id != bundle.decryption_policy_ref
        || receipt.input_artifact != request.input_artifact
        || receipt.output_recipient != request.output_recipient
    {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{}` is already consumed by different private execution evidence",
            request.decryption_request_id
        )));
    }
    Ok(Some(PrivateUploadedModelExecuteResponse {
        schema_version: 1,
        status: status.clone(),
        submission_phase: PrivateUploadedModelSubmissionPhaseV1::Committed,
        transaction_hash: None,
        output_artifact: receipt.output_artifact.clone(),
        receipt,
    }))
}

fn claim_private_execution_submission(
    app: &SharedAppState,
    request: &PrivateUploadedModelExecuteRequest,
) -> Result<PrivateExecutionSubmissionClaim, SoracloudError> {
    let key = (
        request.service_name.clone(),
        request.decryption_request_id.clone(),
    );
    let request_fingerprint = Hash::new(request.encode());
    let tracker = Arc::clone(&app.soracloud_private_execution_submissions);
    let now = Instant::now();
    let mut entries = tracker.entries.lock();

    if let Some(state) = entries.get(&key) {
        match state {
            PrivateExecutionSubmissionState::Executing {
                request_fingerprint: executing_fingerprint,
            } => {
                if *executing_fingerprint != request_fingerprint {
                    return Err(SoracloudError::conflict(format!(
                        "decryption request `{}` is already executing with different private evidence",
                        request.decryption_request_id
                    )));
                }
                return Err(SoracloudError::unavailable(format!(
                    "decryption request `{}` is already executing on this node; retry after the current attempt completes",
                    request.decryption_request_id
                )));
            }
            PrivateExecutionSubmissionState::Submitted {
                request_fingerprint: submitted_fingerprint,
                response,
                submitted_at,
            } => {
                let terminal_failure = response.transaction_hash.is_some_and(|hash| {
                    let typed_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(hash);
                    app.pipeline_status_cache
                        .lookup(&typed_hash)
                        .is_some_and(|status| {
                            matches!(
                                status.kind,
                                crate::PipelineStatusKind::Rejected
                                    | crate::PipelineStatusKind::Expired
                            )
                        })
                });
                let expired = now.saturating_duration_since(*submitted_at)
                    >= PRIVATE_EXECUTION_SUBMISSION_CACHE_TTL;
                if !terminal_failure && !expired {
                    if *submitted_fingerprint != request_fingerprint {
                        return Err(SoracloudError::conflict(format!(
                            "decryption request `{}` already has a pending transaction with different private evidence",
                            request.decryption_request_id
                        )));
                    }
                    return Ok(PrivateExecutionSubmissionClaim::Cached(response.clone()));
                }
            }
        }
        entries.remove(&key);
    }

    entries.retain(|_, state| {
        !matches!(
            state,
            PrivateExecutionSubmissionState::Submitted { submitted_at, .. }
                if now.saturating_duration_since(*submitted_at)
                    >= PRIVATE_EXECUTION_SUBMISSION_CACHE_TTL
        )
    });
    if entries.len() >= PRIVATE_EXECUTION_SUBMISSION_CACHE_MAX_ENTRIES {
        return Err(SoracloudError::unavailable(
            "private execution submission cache is saturated; retry after pending transactions settle",
        ));
    }
    entries.insert(
        key.clone(),
        PrivateExecutionSubmissionState::Executing {
            request_fingerprint,
        },
    );
    drop(entries);
    Ok(PrivateExecutionSubmissionClaim::Acquired(
        PrivateExecutionSubmissionGuard {
            tracker,
            key,
            request_fingerprint,
            completed: false,
        },
    ))
}

fn validate_private_execution_journal_for_request(
    entry: &SoracloudPrivateUploadedModelExecutionJournalV1,
    request_fingerprint: Hash,
    status: &UploadedModelStatusResponse,
    request: &PrivateUploadedModelExecuteRequest,
) -> Result<(), SoracloudError> {
    entry.validate().map_err(|error| {
        SoracloudError::internal(format!(
            "durable private execution recovery journal is invalid: {error}"
        ))
    })?;
    if entry.request_fingerprint != request_fingerprint {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{}` already has durable prepared evidence for a different execution request",
            request.decryption_request_id
        )));
    }
    let receipt = &entry.receipt;
    let bundle = &status.bundle;
    if receipt.service_name.as_ref() != request.service_name
        || receipt.service_version != request.service_version
        || receipt.model_id != bundle.model_id
        || receipt.weight_version != bundle.weight_version
        || receipt.model_manifest_digest != bundle.sorafs_manifest_digest
        || receipt.model_bundle_root != bundle.bundle_root
        || receipt.policy_id != bundle.decryption_policy_ref
        || receipt.decryption_request_id != request.decryption_request_id
        || receipt.input_artifact != request.input_artifact
        || receipt.output_recipient != request.output_recipient
    {
        return Err(SoracloudError::conflict(format!(
            "decryption request `{}` has durable prepared evidence that no longer matches its authoritative execution inputs",
            request.decryption_request_id
        )));
    }
    Ok(())
}

fn private_uploaded_model_submission_phase(
    progress: SoracloudPrivateUploadedModelExecutionSubmissionProgressV1,
) -> Result<PrivateUploadedModelSubmissionPhaseV1, SoracloudError> {
    match (progress.phase, progress.transaction_hash.is_some()) {
        (SoracloudPrivateUploadedModelExecutionJournalPhaseV1::OutputPin, false) => {
            Ok(PrivateUploadedModelSubmissionPhaseV1::AwaitingOutputDurability)
        }
        (SoracloudPrivateUploadedModelExecutionJournalPhaseV1::OutputPin, true) => {
            Ok(PrivateUploadedModelSubmissionPhaseV1::OutputPinSubmitted)
        }
        (SoracloudPrivateUploadedModelExecutionJournalPhaseV1::Receipt, true) => {
            Ok(PrivateUploadedModelSubmissionPhaseV1::ReceiptSubmitted)
        }
        (SoracloudPrivateUploadedModelExecutionJournalPhaseV1::Receipt, false) => Err(
            SoracloudError::internal("private execution receipt phase has no durable transaction"),
        ),
    }
}

fn recover_private_execution_submission(
    app: &SharedAppState,
    runtime: &dyn iroha_core::soracloud_runtime::SoracloudRuntimeReadHandle,
    status: &UploadedModelStatusResponse,
    request: &PrivateUploadedModelExecuteRequest,
    request_fingerprint: Hash,
    submission_guard: &mut Option<PrivateExecutionSubmissionGuard>,
) -> Result<Option<(StatusCode, PrivateUploadedModelExecuteResponse)>, SoracloudError> {
    let service_name = status.bundle.service_name.clone();
    let Some(entry) = runtime
        .load_private_uploaded_model_execution_journal(
            &service_name,
            &request.decryption_request_id,
        )
        .map_err(map_private_runtime_error)?
    else {
        return Ok(None);
    };
    validate_private_execution_journal_for_request(&entry, request_fingerprint, status, request)?;

    // A journal is recoverable only while its encrypted output remains durably readable under
    // the exact content-addressed manifest admitted before the journal was published.
    let output_artifact = entry.receipt.output_artifact.clone();
    let _encrypted_output = read_admitted_private_model_payload(
        app,
        &output_artifact.sorafs_manifest_digest,
        output_artifact.ciphertext_bytes,
        Some(output_artifact.artifact_hash),
        "prepared output",
    )?;

    // The runtime owns observation and replacement of the exact phase transaction. Keeping that
    // decision inside the locked durable outbox prevents Torii's pipeline cache from racing a pin
    // commitment or advancing to the receipt before replication quorum is authoritative.
    let progress = runtime
        .advance_private_uploaded_model_execution(
            entry.output_manifest_payload.clone(),
            entry.receipt.clone(),
        )
        .map_err(map_private_runtime_error)?;
    let submission_phase = private_uploaded_model_submission_phase(progress)?;
    let response = PrivateUploadedModelExecuteResponse {
        schema_version: 1,
        status: status.clone(),
        submission_phase,
        transaction_hash: progress.transaction_hash,
        output_artifact,
        receipt: entry.receipt,
    };
    submission_guard
        .take()
        .expect("private execution recovery owns its submission claim")
        .complete(response.clone());
    Ok(Some((StatusCode::ACCEPTED, response)))
}

fn authoritative_private_uploaded_model_execute_response(
    app: &SharedAppState,
    request: PrivateUploadedModelExecuteRequest,
    verified_request_signers: &[PublicKey],
) -> Result<(StatusCode, PrivateUploadedModelExecuteResponse), SoracloudError> {
    let status_query = UploadedModelStatusQuery {
        service_name: request.service_name.clone(),
        weight_version: request.weight_version.clone(),
        model_id: Some(request.model_id.clone()),
        model_name: None,
        bundle_root: Some(request.bundle_root),
    };
    let status = authoritative_uploaded_model_status_from_query(app, &status_query)?;
    if let Some(response) =
        committed_private_execution_response(app, &status, &request, verified_request_signers)?
    {
        if let Some(runtime) = app.soracloud_runtime.as_ref()
            && let Err(error) = runtime.remove_private_uploaded_model_execution_journal(
                &response.receipt.service_name,
                &response.receipt.decryption_request_id,
            )
        {
            iroha_logger::warn!(
                ?error,
                receipt_id = %response.receipt.receipt_id,
                "failed to remove committed private execution recovery journal"
            );
        }
        app.soracloud_private_execution_submissions
            .entries
            .lock()
            .remove(&(
                request.service_name.clone(),
                request.decryption_request_id.clone(),
            ));
        return Ok((StatusCode::OK, response));
    }
    require_finalized_uploaded_model_release(app, &status.bundle)?;
    if status.bundle.runtime_format != SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1
    {
        return Err(SoracloudError::conflict(format!(
            "uploaded model `{}` version `{}` is not admitted for deterministic quantized CPU execution",
            status.bundle.model_id, status.bundle.weight_version
        )));
    }
    require_private_uploaded_model_release_policy(
        app,
        &status.bundle,
        &request,
        verified_request_signers,
        1,
    )?;
    require_active_sorafs_uploaded_model_pin(app, &status.bundle)?;
    let runtime = app.soracloud_runtime.as_ref().ok_or_else(|| {
        SoracloudError::unavailable("qualified Soracloud private runtime is not available")
    })?;
    let request_fingerprint = Hash::new(request.encode());
    let mut submission_guard = match claim_private_execution_submission(app, &request)? {
        PrivateExecutionSubmissionClaim::Acquired(guard) => Some(guard),
        PrivateExecutionSubmissionClaim::Cached(mut response) => {
            let current_entry = runtime
                .load_private_uploaded_model_execution_journal(
                    &response.receipt.service_name,
                    &response.receipt.decryption_request_id,
                )
                .map_err(map_private_runtime_error)?
                .ok_or_else(|| {
                    SoracloudError::unavailable(
                        "private execution journal is temporarily unavailable before receipt commitment",
                    )
                })?;
            validate_private_execution_journal_for_request(
                &current_entry,
                request_fingerprint,
                &status,
                &request,
            )?;
            if current_entry.receipt != response.receipt {
                return Err(SoracloudError::internal(
                    "private execution journal changed the cached receipt evidence",
                ));
            }
            response.submission_phase = private_uploaded_model_submission_phase(
                SoracloudPrivateUploadedModelExecutionSubmissionProgressV1 {
                    phase: current_entry.phase,
                    transaction_hash: current_entry.transaction_hash,
                },
            )?;
            response.transaction_hash = current_entry.transaction_hash;
            return Ok((StatusCode::ACCEPTED, response));
        }
    };
    if let Some(recovered) = recover_private_execution_submission(
        app,
        runtime.as_ref(),
        &status,
        &request,
        request_fingerprint,
        &mut submission_guard,
    )? {
        return Ok(recovered);
    }
    let submission_guard = submission_guard
        .take()
        .expect("fresh private execution retains its submission claim");
    // A fresh request must leave room for the output-pin block, a provider-completion block,
    // and the receipt block. Recovery checks above retain the one-block candidate rule because
    // an existing journal may already be in its receipt phase.
    require_private_uploaded_model_release_policy(
        app,
        &status.bundle,
        &request,
        verified_request_signers,
        PRIVATE_EXECUTION_INITIAL_RECEIPT_BLOCK_OFFSET_V1,
    )?;
    let signed_attempt_lifetime = iroha_data_model::transaction::DEFAULT_TRANSACTION_TIME_TO_LIVE
        .min(app.queue.tx_time_to_live);
    let required_retention_margin = private_execution_required_retention_margin(
        signed_attempt_lifetime,
        runtime.private_execution_recovery_interval(),
    );
    let required_retention_margin_seconds = required_retention_margin
        .as_secs()
        .saturating_add(u64::from(required_retention_margin.subsec_nanos() != 0));
    let _ = require_active_private_model_artifact_pin(
        app,
        &request.input_artifact,
        required_retention_margin_seconds,
    )?;
    let encrypted_model_artifact_bytes = read_admitted_private_model_payload(
        app,
        &status.bundle.sorafs_manifest_digest,
        status.bundle.ciphertext_bytes,
        None,
        "model",
    )?;
    let encrypted_input_artifact_bytes = read_admitted_private_model_payload(
        app,
        &request.input_artifact.sorafs_manifest_digest,
        request.input_artifact.ciphertext_bytes,
        Some(request.input_artifact.artifact_hash),
        "input",
    )?;
    let result = runtime
        .execute_private_uploaded_model(SoracloudPrivateUploadedModelExecutionRequestV1 {
            bundle: status.bundle.clone(),
            service_version: request.service_version.clone(),
            policy_id: status.bundle.decryption_policy_ref.clone(),
            decryption_request_id: request.decryption_request_id.clone(),
            input_artifact: request.input_artifact.clone(),
            output_recipient: request.output_recipient.clone(),
            encrypted_model_artifact_bytes,
            encrypted_input_artifact_bytes,
        })
        .map_err(map_private_runtime_error)?;
    if result.output_recipient != request.output_recipient {
        return Err(SoracloudError::internal(
            "private runtime returned output encryption metadata different from the request",
        ));
    }
    // Inference is intentionally outside consensus and may take long enough to consume the
    // retention margin checked before execution. Refresh the authoritative pin immediately after
    // inference so the output manifest never inherits a stale recovery horizon.
    let input_pin = require_active_private_model_artifact_pin(
        app,
        &request.input_artifact,
        required_retention_margin_seconds,
    )?;
    let output = build_private_output_manifest(
        result.encrypted_output_artifact_bytes.as_slice(),
        &input_pin,
    )?;
    if output.artifact.artifact_hash == request.input_artifact.artifact_hash {
        return Err(SoracloudError::internal(
            "private runtime produced encrypted output identical to the encrypted input",
        ));
    }
    ingest_private_output_manifest(
        app,
        &output,
        result.encrypted_output_artifact_bytes.as_slice(),
    )?;
    // Local ingest can itself be slow. Do not publish durable outbox evidence unless the source
    // pin still covers the complete bounded registration, replication, and receipt-recovery
    // window. Consensus independently enforces the output horizon at receipt commitment.
    require_active_private_model_artifact_pin(
        app,
        &request.input_artifact,
        required_retention_margin_seconds,
    )?;
    // Close the state-change race before enqueueing. Consensus performs this validation again at
    // the exact execution height inside the same transaction as output-pin registration.
    require_private_uploaded_model_release_policy(
        app,
        &status.bundle,
        &request,
        verified_request_signers,
        PRIVATE_EXECUTION_INITIAL_RECEIPT_BLOCK_OFFSET_V1,
    )?;
    let placeholder = Hash::new(b"soracloud-private-receipt-placeholder");
    let mut receipt = SoraPrivateUploadedModelExecutionReceiptV1 {
        schema_version: SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
        network_id: *app.state.network_id_ref(),
        receipt_id: placeholder,
        service_name: status.bundle.service_name.clone(),
        service_version: request.service_version,
        model_id: status.bundle.model_id.clone(),
        weight_version: status.bundle.weight_version.clone(),
        runtime_version: result.runtime_version,
        model_manifest_digest: status.bundle.sorafs_manifest_digest,
        model_bundle_root: status.bundle.bundle_root,
        policy_id: status.bundle.decryption_policy_ref.clone(),
        decryption_request_id: request.decryption_request_id,
        attesting_validator: result.attesting_validator,
        input_artifact: request.input_artifact,
        output_artifact: output.artifact.clone(),
        output_replication_order_id: derive_sorafs_auto_replication_order_id_v1(
            &output.artifact.sorafs_manifest_digest,
        ),
        input_commitment: result.input_commitment,
        output_commitment: result.output_commitment,
        output_recipient: result.output_recipient,
        request_commitment: placeholder,
        result_commitment: placeholder,
        emitted_sequence: 0,
        emitted_block_height: 0,
    };
    receipt.request_commitment = derive_soracloud_private_model_request_commitment_v1(&receipt);
    receipt.result_commitment = derive_soracloud_private_model_result_commitment_v1(&receipt);
    receipt.receipt_id = derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&receipt);
    receipt.validate_submission().map_err(|err| {
        SoracloudError::internal(format!(
            "private runtime produced an invalid receipt submission: {err}"
        ))
    })?;
    let journal = SoracloudPrivateUploadedModelExecutionJournalV1 {
        schema_version: SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_JOURNAL_VERSION_V1,
        service_name: receipt.service_name.clone(),
        decryption_request_id: receipt.decryption_request_id.clone(),
        request_fingerprint,
        output_manifest_payload: output.manifest_payload.clone(),
        receipt: receipt.clone(),
        phase: SoracloudPrivateUploadedModelExecutionJournalPhaseV1::OutputPin,
        transaction_hash: None,
        signed_transaction: None,
        submission_attempt: 0,
    };
    runtime
        .store_private_uploaded_model_execution_journal(journal.clone())
        .map_err(map_private_runtime_error)?;
    let progress = runtime
        .advance_private_uploaded_model_execution(output.manifest_payload, receipt.clone())
        .map_err(map_private_runtime_error)?;
    let submission_phase = private_uploaded_model_submission_phase(progress)?;
    let response = PrivateUploadedModelExecuteResponse {
        schema_version: 1,
        status,
        submission_phase,
        transaction_hash: progress.transaction_hash,
        output_artifact: output.artifact,
        receipt,
    };
    submission_guard.complete(response.clone());
    Ok((StatusCode::ACCEPTED, response))
}
fn authoritative_private_uploaded_model_receipts_response(
    app: &SharedAppState,
    query: PrivateUploadedModelReceiptQuery,
) -> Result<PrivateUploadedModelReceiptListResponse, SoracloudError> {
    let count_mode = private_uploaded_model_receipt_count_mode(query.count_mode.as_deref())?;
    let service_name = query
        .service_name
        .as_deref()
        .map(parse_service_name)
        .transpose()?
        .map(|name| name.to_string());
    let model_id = query
        .model_id
        .as_deref()
        .map(parse_training_job_id)
        .transpose()?;
    let weight_version = query
        .weight_version
        .as_deref()
        .map(parse_model_weight_version)
        .transpose()?;
    let limit = private_uploaded_model_receipt_limit(query.limit)?;
    let state_view = app.state.view();
    let world = state_view.world();
    let filter_digest = private_uploaded_model_receipt_filter_digest(
        query.receipt_id.as_ref(),
        service_name.as_deref(),
        model_id.as_deref(),
        weight_version.as_deref(),
    );
    let cursor = query
        .cursor
        .as_deref()
        .map(|raw| decode_private_uploaded_model_receipt_cursor(raw, filter_digest))
        .transpose()?;
    let current_sequence = latest_soracloud_sequence(world);
    paginate_private_uploaded_model_receipts(
        world
            .soracloud_private_uploaded_model_execution_receipts()
            .iter(),
        PrivateUploadedModelReceiptPageSpec {
            receipt_id: query.receipt_id.as_ref(),
            service_name: service_name.as_deref(),
            model_id: model_id.as_deref(),
            weight_version: weight_version.as_deref(),
            filter_digest,
            cursor,
            limit,
            count_mode,
            current_sequence,
        },
    )
}
fn authoritative_uploaded_model_status_from_query(
    app: &SharedAppState,
    query: &UploadedModelStatusQuery,
) -> Result<UploadedModelStatusResponse, SoracloudError> {
    let service_name = parse_service_name(&query.service_name)?;
    let service_name = service_name.to_string();
    let weight_version = parse_model_weight_version(&query.weight_version)?;
    if query.model_id.is_some() && query.model_name.is_some() {
        return Err(SoracloudError::bad_request(
            "exactly one of model_id or model_name must be provided for uploaded model status",
        ));
    }
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
                    && query.bundle_root.is_none_or(|bundle_root| {
                        record
                            .source_provenance
                            .as_ref()
                            .and_then(|provenance| {
                                world
                                    .soracloud_uploaded_model_bundles()
                                    .get(&(
                                        service_name.clone(),
                                        provenance.id.clone(),
                                        weight_version.clone(),
                                    ))
                            })
                            .is_some_and(|bundle| bundle.bundle_root == bundle_root)
                    }))
                .then(|| record.source_provenance.as_ref().map(|provenance| provenance.id.clone()))
                .flatten()
            })
            .ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "uploaded model status not found for service `{service_name}`, model `{model_name}`, version `{weight_version}`"
                ))
            })?
    };
    authoritative_uploaded_model_status_response(app, &service_name, &model_id, &weight_version)
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
        .filter(|(_sequence, event)| {
            event.pool_id == pool_id
                && account_id.is_none_or(|account_id| event.account_id == *account_id)
        })
        .map(|(_sequence, event)| event.clone())
        .max_by_key(|event| event.sequence);
    let runtime_projection = authoritative_hf_runtime_projection(app, &source_id);
    let storage_base_fee = pool
        .as_ref()
        .map_or_else(Quantity::zero, |pool| pool.base_fee.clone());
    let compute_reservation_fee = placement.as_ref().map_or_else(Quantity::zero, |placement| {
        placement.total_reservation_fee.clone()
    });
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
        storage_base_fee,
        compute_reservation_fee,
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
    if deployment.service_name != service_id {
        return Err(SoracloudError::internal(format!(
            "service `{service_name}` deployment record has substituted identity `{}`",
            deployment.service_name
        )));
    }
    let bundle = world
        .soracloud_service_revisions()
        .get(&(
            deployment.service_name.to_string(),
            deployment.current_service_version.clone(),
        ))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::internal(format!(
                "service `{service_name}` active revision `{}` is missing from authoritative state",
                deployment.current_service_version
            ))
        })?;
    if bundle.service.service_name != deployment.service_name
        || bundle.service.service_version != deployment.current_service_version
        || bundle.service_manifest_hash() != deployment.current_service_manifest_hash
        || bundle.container_manifest_hash() != deployment.current_container_manifest_hash
    {
        return Err(SoracloudError::internal(format!(
            "service `{service_name}` deployment record does not bind active revision `{}`",
            deployment.current_service_version
        )));
    }
    bundle.validate_for_admission().map_err(|error| {
        SoracloudError::internal(format!(
            "service `{service_name}` active revision `{}` is not an admitted V1 bundle: {error}",
            deployment.current_service_version
        ))
    })?;
    Ok((deployment, bundle))
}
fn decode_public_service_discovery_registry(
    entry: &SoraServiceConfigEntryV1,
) -> Result<SoracloudPublicServiceDiscoveryRegistryV1, SoracloudError> {
    if entry.config_name != PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME {
        return Err(SoracloudError::internal(format!(
            "authoritative public discovery config has substituted name `{}`",
            entry.config_name
        )));
    }
    let value_json = entry
        .value_json
        .clone()
        .try_into_any_norito::<norito::json::Value>()
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to decode authoritative public discovery config json: {err}"
            ))
        })?;
    let canonical_bytes = norito::json::to_vec(&value_json).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to encode authoritative public discovery config json: {err}"
        ))
    })?;
    if Hash::new(&canonical_bytes) != entry.value_hash {
        return Err(SoracloudError::internal(
            "authoritative public discovery config hash does not bind its JSON value",
        ));
    }
    let registry: SoracloudPublicServiceDiscoveryRegistryV1 = norito::json::from_value(value_json)
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to parse authoritative public discovery registry: {err}"
            ))
        })?;
    if registry.schema_version != PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1 {
        return Err(SoracloudError::internal(format!(
            "authoritative public discovery registry uses schema version {}; expected {}",
            registry.schema_version, PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1
        )));
    }
    for (version, discovery) in &registry.revisions {
        if discovery.schema_version != PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1
            || discovery.service_name != registry.service_name
            || discovery.service_version != *version
        {
            return Err(SoracloudError::internal(format!(
                "authoritative public discovery revision `{version}` is not bound to its registry identity"
            )));
        }
    }
    Ok(registry)
}
fn authoritative_public_service_discovery_registry(
    deployment: &SoraServiceDeploymentStateV1,
) -> Result<Option<SoracloudPublicServiceDiscoveryRegistryV1>, SoracloudError> {
    let registry = deployment
        .service_configs
        .get(PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME)
        .map(decode_public_service_discovery_registry)
        .transpose()?;
    if let Some(registry) = registry.as_ref()
        && (registry.service_name != deployment.service_name.as_ref()
            || registry.current_version != deployment.current_service_version
            || !registry
                .revisions
                .contains_key(&deployment.current_service_version))
    {
        return Err(SoracloudError::internal(format!(
            "authoritative public discovery registry for service `{}` does not bind its active revision `{}`",
            deployment.service_name, deployment.current_service_version
        )));
    }
    Ok(registry)
}
fn authoritative_public_service_discovery_for_version(
    deployment: &SoraServiceDeploymentStateV1,
    service_version: &str,
) -> Result<Option<SoracloudPublicServiceDiscoveryV1>, SoracloudError> {
    let Some(registry) = authoritative_public_service_discovery_registry(deployment)? else {
        return Ok(None);
    };
    Ok(registry.revisions.get(service_version).cloned())
}
fn validate_public_service_discovery_for_bundle(
    deployment: &SoraServiceDeploymentStateV1,
    bundle: &SoraDeploymentBundleV1,
    discovery: &SoracloudPublicServiceDiscoveryV1,
) -> Result<(), SoracloudError> {
    bundle.validate_for_admission().map_err(|error| {
        SoracloudError::internal(format!(
            "service `{}` public discovery revision `{}` is not an admitted V1 bundle: {error}",
            deployment.service_name, bundle.service.service_version
        ))
    })?;
    admit_scr_host_bundle(bundle).map_err(|error| {
        SoracloudError::internal(format!(
            "service `{}` public discovery revision `{}` fails authoritative SCR admission: {}",
            deployment.service_name, bundle.service.service_version, error.message
        ))
    })?;
    let route = bundle.service.route.as_ref().ok_or_else(|| {
        SoracloudError::internal(format!(
            "service `{}` has public discovery without an admitted route",
            deployment.service_name
        ))
    })?;
    let expected_base_url = bundle_base_url(bundle).ok_or_else(|| {
        SoracloudError::internal(format!(
            "service `{}` active route cannot form its authoritative base URL",
            deployment.service_name
        ))
    })?;
    let expected_healthcheck_url = bundle_healthcheck_url(bundle);
    let expected_bundle_hash = Hash::new(Encode::encode(bundle));
    let fields_match = discovery.schema_version == PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1
        && discovery.service_name == deployment.service_name.as_ref()
        && bundle.service.service_name == deployment.service_name
        && discovery.service_version == bundle.service.service_version
        && discovery.execution_plane == format!("{:?}", bundle.service.execution_plane)
        && discovery.runtime == format!("{:?}", bundle.container.runtime)
        && discovery.route_host == route.host
        && discovery.path_prefix == route.path_prefix
        && discovery.base_url == expected_base_url
        && discovery.healthcheck_path == bundle.container.lifecycle.healthcheck_path
        && discovery.healthcheck_url == expected_healthcheck_url
        && discovery.service_manifest_hash == bundle.service_manifest_hash()
        && discovery.container_manifest_hash == bundle.container_manifest_hash()
        && discovery.deployment_bundle_hash == expected_bundle_hash;
    if !fields_match {
        return Err(SoracloudError::internal(format!(
            "service `{}` authoritative public discovery record does not bind admitted revision `{}`",
            deployment.service_name, bundle.service.service_version
        )));
    }
    for (field, value) in [
        ("content_cid", discovery.content_cid.as_str()),
        (
            "public_discovery_url",
            discovery.public_discovery_url.as_str(),
        ),
        (
            "public_discovery_cid_host_url",
            discovery.public_discovery_cid_host_url.as_str(),
        ),
        (
            "manifest_digest_hex",
            discovery.manifest_digest_hex.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(SoracloudError::internal(format!(
                "service `{}` authoritative public discovery field `{field}` is empty",
                deployment.service_name
            )));
        }
    }
    Ok(())
}
fn bundle_base_url(bundle: &SoraDeploymentBundleV1) -> Option<String> {
    let route = bundle.service.route.as_ref()?;
    let scheme = match route.tls_mode {
        SoraTlsModeV1::Disabled => "http",
        SoraTlsModeV1::Optional | SoraTlsModeV1::Required => "https",
    };
    let mut base_url = reqwest::Url::parse(&format!("{scheme}://{}", route.host)).ok()?;
    let route_root = if route.path_prefix.trim().is_empty() {
        "/".to_owned()
    } else if route.path_prefix.ends_with('/') {
        route.path_prefix.clone()
    } else {
        format!("{}/", route.path_prefix)
    };
    base_url.set_path(&route_root);
    base_url.set_query(None);
    base_url.set_fragment(None);
    Some(base_url.to_string())
}
fn bundle_healthcheck_url(bundle: &SoraDeploymentBundleV1) -> Option<String> {
    let route = bundle.service.route.as_ref()?;
    let healthcheck_path = bundle.container.lifecycle.healthcheck_path.as_ref()?;
    let scheme = match route.tls_mode {
        SoraTlsModeV1::Disabled => "http",
        SoraTlsModeV1::Optional | SoraTlsModeV1::Required => "https",
    };
    let mut url = reqwest::Url::parse(&format!("{scheme}://{}", route.host)).ok()?;
    url.set_path(&join_public_route_paths(
        route.path_prefix.as_str(),
        healthcheck_path,
    ));
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
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
fn authoritative_agent_current_height(app: &SharedAppState) -> u64 {
    u64::try_from(app.state.view().height()).unwrap_or(u64::MAX)
}
fn authoritative_agent_mutation_response(
    app: &SharedAppState,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> AgentMutationResponse {
    let current_height = authoritative_agent_current_height(app);
    AgentMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        status: authoritative_agent_runtime_status_in_current_view(record, current_height),
        lease_expires_height: record.lease_expires_height,
        lease_remaining_blocks: record.lease_expires_height.saturating_sub(current_height),
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
) -> Result<AgentWalletMutationResponse, SoracloudError> {
    let current_height = authoritative_agent_current_height(app);
    let day_bucket = matches!(
        event.action,
        SoraAgentApartmentActionV1::WalletSpendApproved
    )
    .then(|| wallet_day_bucket(event.block_timestamp_ms));
    let day_spent = match (day_bucket, event.asset_definition.as_deref()) {
        (Some(bucket), Some(asset_definition)) => record
            .wallet_daily_spend
            .get(&format!("{asset_definition}:{bucket}"))
            .map(|entry| entry.spent.clone()),
        _ => None,
    };
    Ok(AgentWalletMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        manifest_hash: record.manifest_hash,
        status: authoritative_agent_runtime_status_in_current_view(record, current_height),
        request_id: event.request_id.clone(),
        asset_definition: event.asset_definition.clone(),
        amount: event.amount.clone(),
        day_bucket,
        day_spent,
        capability: event.capability.clone(),
        reason: event.reason.clone(),
        pending_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
        audit_event_count: 0,
        signed_by: event.signer.to_string(),
    })
}
fn authoritative_agent_mailbox_mutation_response(
    app: &SharedAppState,
    apartment_name: &str,
    record: &SoraAgentApartmentRecordV1,
    event: &SoraAgentApartmentAuditEventV1,
) -> Result<AgentMailboxMutationResponse, SoracloudError> {
    let current_height = authoritative_agent_current_height(app);
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
        status: authoritative_agent_runtime_status_in_current_view(record, current_height),
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
    let current_height = authoritative_agent_current_height(app);
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
    let authoritative_execution_audit = match approved_run {
        Some(run) => authoritative_agent_execution_audit_for_run(
            world,
            record.manifest.apartment_name.as_ref(),
            run,
        )?,
        None => None,
    };
    Ok(AgentAutonomyMutationResponse {
        action: authoritative_agent_action(event.action),
        apartment_name: record.manifest.apartment_name.to_string(),
        sequence: event.sequence,
        status: authoritative_agent_runtime_status_in_current_view(record, current_height),
        lease_expires_height: record.lease_expires_height,
        lease_remaining_blocks: record.lease_expires_height.saturating_sub(current_height),
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
    rollout.validate().map_err(|error| {
        SoracloudError::internal(format!(
            "authoritative rollout `{requested_rollout_handle}` for service `{service_name}` is invalid: {error}"
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
                && event
                    .rollout_state
                    .as_ref()
                    .is_some_and(|state| state.rollout_handle == requested_rollout_handle)
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "authoritative rollout audit event for service `{service_name}` was not observed after mutation"
            ))
        })?;
    event.validate().map_err(|error| {
        SoracloudError::internal(format!(
            "authoritative rollout audit event for service `{service_name}` is invalid: {error}"
        ))
    })?;
    let authoritative_governance_tx_hash = event.governance_tx_hash.ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative rollout audit event for service `{service_name}` is missing governance_tx_hash"
        ))
    })?;
    if authoritative_governance_tx_hash != governance_tx_hash {
        return Err(SoracloudError::conflict(format!(
            "authoritative rollout audit event for service `{service_name}` does not bind the requested governance_tx_hash"
        )));
    }
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
        governance_tx_hash: authoritative_governance_tx_hash,
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
            && event.config_mutations.len() == 1
            && event.config_mutations[0].config_name() == config_name
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
            && event.secret_mutations.len() == 1
            && event.secret_mutations[0].secret_name() == secret_name
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
        .map(
            |entry| -> Result<ServiceConfigStatusEntry, SoracloudError> {
                let value_json = entry
                    .value_json
                    .try_into_any_norito::<norito::json::Value>()
                    .map_err(|err| {
                        SoracloudError::internal(format!(
                            "failed to decode authoritative service config json: {err}"
                        ))
                    })?;
                Ok::<_, SoracloudError>(ServiceConfigStatusEntry {
                    config_name: entry.config_name.clone(),
                    value_hash: entry.value_hash,
                    value_json,
                    last_update_sequence: entry.last_update_sequence,
                })
            },
        )
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
fn authoritative_service_public_discovery_response(
    app: &SharedAppState,
    service_name: &str,
    requested_version: Option<&str>,
) -> Result<ServicePublicDiscoveryResponse, SoracloudError> {
    let state_view = app.state.view();
    let world = state_view.world();
    let (deployment, _active_bundle) =
        authoritative_service_deployment_bundle(world, service_name)?;
    let registry =
        authoritative_public_service_discovery_registry(&deployment)?.ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` has no authoritative public discovery record"
            ))
        })?;
    let current_version = deployment.current_service_version.clone();
    let requested_version = requested_version
        .unwrap_or(current_version.as_str())
        .to_owned();
    let discovery = registry
        .revisions
        .get(requested_version.as_str())
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "service `{service_name}` has no public discovery record for revision `{requested_version}`"
            ))
        })?;
    let bundle = world
        .soracloud_service_revisions()
        .get(&(deployment.service_name.to_string(), requested_version.clone()))
        .cloned()
        .ok_or_else(|| {
            SoracloudError::internal(format!(
                "service `{service_name}` public discovery revision `{requested_version}` is missing its admitted bundle"
            ))
        })?;
    validate_public_service_discovery_for_bundle(&deployment, &bundle, &discovery)?;
    Ok(ServicePublicDiscoveryResponse {
        schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
        service_name: deployment.service_name.to_string(),
        current_version,
        requested_version,
        discovery,
    })
}
fn authoritative_fhe_job_mutation_response(
    app: &SharedAppState,
    baseline: &SoracloudAuditBaseline,
    service_name: &str,
    binding_name: &str,
    job: &FheJobSpecV1,
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
        governance_tx_hash: event.governance_tx_hash.ok_or_else(|| {
            SoracloudError::conflict(
                "authoritative FHE audit event is missing governed-material transaction hash",
            )
        })?,
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
    let queued_renewal = if event.action == SoraHfSharedLeaseActionV1::Renew {
        pool.queued_next_window.as_ref().filter(|next_window| {
            next_window.sponsor_account_id == *account_id
                && event.lease_expires_at_ms == next_window.window_expires_at_ms
        })
    } else {
        None
    };
    // A queued renewal has no authoritative next-window placement or compute charge yet. Keep
    // projecting the current placement; the nested queued window exposes its canonical cap.
    let placement = active_placement.clone();
    let storage_base_fee = if event.action == SoraHfSharedLeaseActionV1::Renew {
        pool.queued_next_window
            .as_ref()
            .filter(|next_window| {
                next_window.sponsor_account_id == *account_id
                    && event.lease_expires_at_ms == next_window.window_expires_at_ms
            })
            .map_or_else(
                || pool.base_fee.clone(),
                |next_window| next_window.base_fee.clone(),
            )
    } else {
        pool.base_fee.clone()
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
        storage_base_fee,
        compute_reservation_fee: queued_renewal.map_or_else(
            || {
                placement.as_ref().map_or_else(Quantity::zero, |placement| {
                    placement.total_reservation_fee.clone()
                })
            },
            |_queued_window| Quantity::zero(),
        ),
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
    if record.last_renewed_height <= baseline.agent_apartment_max {
        return Err(SoracloudError::conflict(format!(
            "authoritative lease-renew event for apartment `{apartment_name}` was not observed after mutation"
        )));
    }
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&record.last_renewed_height)
        .cloned()
        .filter(|event| {
            event.action == SoraAgentApartmentActionV1::LeaseRenew
                && event.apartment_name.as_ref() == apartment_name
        })
        .ok_or_else(|| {
            SoracloudError::conflict(format!(
                "lease-renew audit event `{}` for apartment `{apartment_name}` is missing from authoritative state",
                record.last_renewed_height
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
    amount: &Quantity,
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
                && &request.amount == amount
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
                    && event.amount.as_ref() == Some(amount)
            }),
        None => latest_agent_apartment_audit_event_after(world, baseline.agent_apartment_max, |event| {
            event.apartment_name.as_ref() == apartment_name
                && matches!(
                    event.action,
                    SoraAgentApartmentActionV1::WalletSpendRequested
                        | SoraAgentApartmentActionV1::WalletSpendApproved
                )
                && event.asset_definition.as_deref() == Some(asset_definition)
                && event.amount.as_ref() == Some(amount)
        })
        .cloned(),
    }
    .ok_or_else(|| {
        SoracloudError::conflict(format!(
            "authoritative wallet-spend event for apartment `{apartment_name}` asset `{asset_definition}` amount `{amount}` was not observed after mutation"
        ))
    })?;
    let mut response = authoritative_agent_wallet_mutation_response(app, &record, &event)?;
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
    let mut response = authoritative_agent_wallet_mutation_response(app, &record, &event)?;
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
fn authoritative_agent_runtime_status(status: SoraAgentRuntimeStatusV1) -> AgentRuntimeStatus {
    match status {
        SoraAgentRuntimeStatusV1::Running => AgentRuntimeStatus::Running,
        SoraAgentRuntimeStatusV1::LeaseExpired => AgentRuntimeStatus::LeaseExpired,
    }
}
fn authoritative_agent_runtime_status_in_current_view(
    record: &SoraAgentApartmentRecordV1,
    current_height: u64,
) -> AgentRuntimeStatus {
    authoritative_agent_runtime_status(record.runtime_status_at_current_height(current_height))
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
        execution_host: receipt.execution_host.clone(),
        mailbox_message_id: receipt.mailbox_message_id,
        journal_artifact_hash: receipt.journal_artifact_hash,
        checkpoint_artifact_hash: receipt.checkpoint_artifact_hash,
    }
}
fn authoritative_agent_execution_audit_record(
    event: &SoraAgentApartmentAuditEventV1,
) -> Result<AgentAutonomyExecutionAuditRecord, SoracloudError> {
    let succeeded = event.succeeded.ok_or_else(|| {
        SoracloudError::internal(format!(
            "authoritative agent execution audit event {} is missing `succeeded`",
            event.sequence
        ))
    })?;
    let result_commitment = event.result_commitment.ok_or_else(|| {
        SoracloudError::internal(format!(
            "authoritative agent execution audit event {} is missing `result_commitment`",
            event.sequence
        ))
    })?;
    Ok(AgentAutonomyExecutionAuditRecord {
        sequence: event.sequence,
        succeeded,
        result_commitment,
        service_name: event.service_name.clone(),
        service_version: event.service_version.clone(),
        handler_name: event.handler_name.clone(),
        runtime_receipt_id: event.runtime_receipt_id,
        journal_artifact_hash: event.journal_artifact_hash,
        checkpoint_artifact_hash: event.checkpoint_artifact_hash,
        reason: event.reason.clone(),
    })
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
) -> Result<Option<AgentAutonomyExecutionAuditRecord>, SoracloudError> {
    let mut latest = None;
    for (sequence, event) in world.soracloud_agent_apartment_audit_events().iter() {
        if event.action != SoraAgentApartmentActionV1::AutonomyRunExecuted
            || event.apartment_name.as_ref() != apartment_name
            || event.run_id.as_deref() != Some(run.run_id.as_str())
        {
            continue;
        }
        if *sequence != event.sequence {
            return Err(SoracloudError::internal(format!(
                "authoritative agent execution audit key {sequence} does not bind event sequence {}",
                event.sequence
            )));
        }
        event.validate().map_err(|error| {
            SoracloudError::internal(format!(
                "authoritative agent execution audit event {sequence} is invalid: {error}"
            ))
        })?;
        if latest.is_none_or(|current: &SoraAgentApartmentAuditEventV1| {
            event.sequence > current.sequence
        }) {
            latest = Some(event);
        }
    }
    latest
        .map(authoritative_agent_execution_audit_record)
        .transpose()
}
fn authoritative_agent_run_record(
    world: &impl WorldReadOnly,
    apartment_name: &str,
    record: &SoraAgentAutonomyRunRecordV1,
) -> Result<AgentAutonomyRunRecord, SoracloudError> {
    Ok(AgentAutonomyRunRecord {
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
        )?,
    })
}
fn authoritative_agent_runtime_recent_runs(
    app: &SharedAppState,
    apartment_name: &str,
) -> Result<Vec<AgentRuntimeExecutionSummary>, SoracloudError> {
    let Some(runtime) = app.soracloud_runtime.as_ref() else {
        return Ok(Vec::new());
    };
    let state_view = app.state.view();
    let Some(record) = state_view
        .world()
        .soracloud_agent_apartments()
        .get(apartment_name)
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let mut summaries = Vec::new();
    for run in record
        .autonomy_run_history
        .iter()
        .rev()
        .take(AGENT_AUTONOMY_RECENT_RUN_LIMIT)
    {
        if let Some(summary) = read_agent_runtime_execution_summary(
            runtime.state_dir().as_path(),
            apartment_name,
            &run.run_id,
            run.approved_process_generation,
            run.request_commitment,
        )? {
            summaries.push(summary);
        }
    }
    Ok(summaries)
}
#[cfg(unix)]
fn same_agent_runtime_summary_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn same_agent_runtime_summary_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    left.volume_serial_number().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index().is_some()
        && left.file_index() == right.file_index()
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
        && right.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
}
#[cfg(not(any(unix, windows)))]
fn same_agent_runtime_summary_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
fn read_agent_runtime_execution_summary_bytes(path: &FsPath) -> io::Result<Vec<u8>> {
    let named_before = fs::symlink_metadata(path)?;
    if named_before.file_type().is_symlink()
        || !named_before.is_file()
        || named_before.len() > SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_MAX_BYTES_V1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Soracloud autonomy summary is not a bounded direct regular file",
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if !opened_before.is_file() || !same_agent_runtime_summary_file(&named_before, &opened_before) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Soracloud autonomy summary changed while opening",
        ));
    }
    let expected_len = usize::try_from(opened_before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Soracloud autonomy summary length is not addressable",
        )
    })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|error| io::Error::other(format!("reserve bounded summary buffer: {error}")))?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes)?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Soracloud autonomy summary grew beyond its admitted length",
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || !named_after.is_file()
        || !same_agent_runtime_summary_file(&opened_before, &opened_after)
        || !same_agent_runtime_summary_file(&opened_after, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Soracloud autonomy summary changed during bounded read",
        ));
    }
    Ok(bytes)
}
fn read_agent_runtime_execution_summary(
    state_dir: &std::path::Path,
    apartment_name: &str,
    run_id: &str,
    process_generation: u64,
    request_commitment: Hash,
) -> Result<Option<AgentRuntimeExecutionSummary>, SoracloudError> {
    let summary_path = state_dir
        .join("apartments")
        .join(sanitize_runtime_path_component(apartment_name))
        .join("runs")
        .join(sanitize_runtime_path_component(run_id))
        .join("execution_summary.json");
    let summary_bytes = match read_agent_runtime_execution_summary_bytes(&summary_path) {
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
    validate_soracloud_apartment_autonomy_execution_summary_v1(
        &summary,
        apartment_name,
        run_id,
        process_generation,
        request_commitment,
    )
    .map_err(|error| {
        SoracloudError::internal(format!(
            "invalid runtime execution summary {}: {error}",
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
    current_height: u64,
) -> AgentApartmentStatusEntry {
    AgentApartmentStatusEntry {
        apartment_name: apartment_name.to_owned(),
        manifest_hash: record.manifest_hash,
        status: authoritative_agent_runtime_status_in_current_view(record, current_height),
        lease_started_height: record.lease_started_height,
        lease_expires_height: record.lease_expires_height,
        lease_remaining_blocks: record.lease_expires_height.saturating_sub(current_height),
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
    let state_view = app.state.view();
    let world = state_view.world();
    let current_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let mut apartments = world
        .soracloud_agent_apartments()
        .iter()
        .filter(|(apartment_name, _record)| {
            apartment_filter
                .as_ref()
                .is_none_or(|filter| filter.as_str() == apartment_name.as_str())
        })
        .map(|(apartment_name, record)| {
            authoritative_agent_status_entry(apartment_name, record, current_height)
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
    let state_view = app.state.view();
    let world = state_view.world();
    let current_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
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
        status: authoritative_agent_runtime_status_in_current_view(&record, current_height),
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
    let state_view = app.state.view();
    let world = state_view.world();
    let sequence = authoritative_soracloud_sequence(world);
    let current_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let record = world
        .soracloud_agent_apartments()
        .get(&apartment_name)
        .cloned()
        .ok_or_else(|| {
            SoracloudError::not_found(format!(
                "apartment `{apartment_name}` not found in authoritative Soracloud state"
            ))
        })?;
    let runtime_recent_runs =
        authoritative_agent_runtime_recent_runs(app, apartment_name.as_ref())?;
    Ok(AgentAutonomyStatusResponse {
        apartment_name,
        sequence,
        status: authoritative_agent_runtime_status_in_current_view(&record, current_height),
        lease_expires_height: record.lease_expires_height,
        lease_remaining_blocks: record.lease_expires_height.saturating_sub(current_height),
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
            .collect::<Result<Vec<_>, _>>()?,
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
    let audit_log = authoritative_audit_log(app)?;
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
    let audit_log = authoritative_audit_log(app)?;
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
    for event in &access_events {
        for (field, present) in [
            ("binding_name", event.binding_name.is_some()),
            ("state_key", event.state_key.is_some()),
            ("policy_name", event.policy_name.is_some()),
            ("policy_snapshot_hash", event.policy_snapshot_hash.is_some()),
            ("jurisdiction_tag", event.jurisdiction_tag.is_some()),
            ("break_glass", event.break_glass.is_some()),
            ("governance_tx_hash", event.governance_tx_hash.is_some()),
        ] {
            if !present {
                return Err(SoracloudError::internal(format!(
                    "authoritative decryption audit event {} is missing `{field}`",
                    event.sequence
                )));
            }
        }
    }
    let total_access_events = u32::try_from(access_events.len()).map_err(|_| {
        SoracloudError::internal(
            "authoritative decryption audit count exceeds the V1 u32 response range",
        )
    })?;
    let break_glass_events = u32::try_from(
        access_events
            .iter()
            .filter(|event| event.break_glass == Some(true))
            .count(),
    )
    .map_err(|_| {
        SoracloudError::internal(
            "authoritative break-glass audit count exceeds the V1 u32 response range",
        )
    })?;
    let non_break_glass_events = total_access_events
        .checked_sub(break_glass_events)
        .ok_or_else(|| {
            SoracloudError::internal(
                "authoritative break-glass audit count exceeds total access events",
            )
        })?;
    let consent_evidence_present_events = u32::try_from(
        access_events
            .iter()
            .filter(|event| event.consent_evidence_hash.is_some())
            .count(),
    )
    .map_err(|_| {
        SoracloudError::internal(
            "authoritative consent-evidence audit count exceeds the V1 u32 response range",
        )
    })?;
    let consent_evidence_coverage_bps = if total_access_events == 0 {
        0
    } else {
        let numerator = u128::from(consent_evidence_present_events) * 10_000;
        let denominator = u128::from(total_access_events);
        u16::try_from(numerator / denominator).map_err(|_| {
            SoracloudError::internal(
                "authoritative consent-evidence coverage exceeds the V1 u16 response range",
            )
        })?
    };
    let mut recent_access_events = Vec::with_capacity(access_events.len().min(limit));
    for event in access_events.iter().rev().take(limit) {
        recent_access_events.push(HealthAccessAuditEntry {
            sequence: event.sequence,
            service_name: event.service_name.clone(),
            binding_name: event.binding_name.clone().ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost binding_name")
            })?,
            state_key: event.state_key.clone().ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost state_key")
            })?,
            policy_name: event.policy_name.clone().ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost policy_name")
            })?,
            jurisdiction_tag: event.jurisdiction_tag.clone().ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost jurisdiction_tag")
            })?,
            consent_evidence_hash: event.consent_evidence_hash,
            break_glass: event.break_glass.ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost break_glass")
            })?,
            break_glass_reason: event.break_glass_reason.clone(),
            governance_tx_hash: event.governance_tx_hash.ok_or_else(|| {
                SoracloudError::internal("validated decryption event lost governance_tx_hash")
            })?,
            signed_by: event.signed_by.clone(),
        });
    }
    let mut jurisdiction_stats_acc: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for event in &access_events {
        let tag = event.jurisdiction_tag.clone().ok_or_else(|| {
            SoracloudError::internal("validated decryption event lost jurisdiction_tag")
        })?;
        let entry = jurisdiction_stats_acc.entry(tag).or_insert((0, 0));
        entry.0 = entry.0.checked_add(1).ok_or_else(|| {
            SoracloudError::internal(
                "authoritative jurisdiction access count exceeds the V1 u32 response range",
            )
        })?;
        if event.break_glass == Some(true) {
            entry.1 = entry.1.checked_add(1).ok_or_else(|| {
                SoracloudError::internal(
                    "authoritative jurisdiction break-glass count exceeds the V1 u32 response range",
                )
            })?;
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
        let policy_name = event.policy_name.clone().ok_or_else(|| {
            SoracloudError::internal("validated decryption event lost policy_name")
        })?;
        let policy_snapshot_hash = event.policy_snapshot_hash.ok_or_else(|| {
            SoracloudError::internal("validated decryption event lost policy_snapshot_hash")
        })?;
        let jurisdiction = event.jurisdiction_tag.clone().ok_or_else(|| {
            SoracloudError::internal("validated decryption event lost jurisdiction_tag")
        })?;
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
        entry.event_count = entry.event_count.checked_add(1).ok_or_else(|| {
            SoracloudError::internal(
                "authoritative policy history count exceeds the V1 u32 response range",
            )
        })?;
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
        let service_id = service_name.parse::<Name>().map_err(|error| {
            SoracloudError::internal(format!(
                "authoritative health audit references invalid service `{service_name}`: {error}"
            ))
        })?;
        let deployment = world
            .soracloud_service_deployments()
            .get(&service_id)
            .ok_or_else(|| {
                SoracloudError::internal(format!(
                    "authoritative health audit references missing service `{service_name}`"
                ))
            })?;
        let bundle = world
            .soracloud_service_revisions()
            .get(&(
                service_name.clone(),
                deployment.current_service_version.clone(),
            ))
            .ok_or_else(|| {
                SoracloudError::internal(format!(
                    "authoritative health audit service `{service_name}` is missing active revision `{}`",
                    deployment.current_service_version
                ))
            })?;
        if deployment.service_name != service_id
            || bundle.service.service_name != service_id
            || bundle.service.service_version != deployment.current_service_version
            || deployment.current_service_manifest_hash != bundle.service_manifest_hash()
            || deployment.current_container_manifest_hash != bundle.container_manifest_hash()
        {
            return Err(SoracloudError::internal(format!(
                "authoritative health audit service `{service_name}` does not bind its active revision `{}`",
                deployment.current_service_version
            )));
        }
        bundle.validate_for_admission().map_err(|error| {
            SoracloudError::internal(format!(
                "authoritative health audit service `{service_name}` has an invalid active revision: {error}"
            ))
        })?;
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
    public_discovery: Option<&SoracloudPublicServiceDiscoveryV1>,
) -> Result<ControlPlaneServiceRevision, SoracloudError> {
    if deployment.service_name != bundle.service.service_name
        || deployment.current_service_version != bundle.service.service_version
        || deployment.current_service_manifest_hash != bundle.service_manifest_hash()
        || deployment.current_container_manifest_hash != bundle.container_manifest_hash()
    {
        return Err(SoracloudError::internal(format!(
            "service `{}` deployment state does not bind active revision `{}`",
            deployment.service_name, deployment.current_service_version
        )));
    }
    bundle.validate_for_admission().map_err(|error| {
        SoracloudError::internal(format!(
            "service `{}` active revision `{}` is not an admitted V1 bundle: {error}",
            deployment.service_name, deployment.current_service_version
        ))
    })?;
    let host_admission = admit_scr_host_bundle(bundle).map_err(|error| {
        SoracloudError::internal(format!(
            "service `{}` active revision `{}` fails authoritative SCR admission: {}",
            deployment.service_name, deployment.current_service_version, error.message
        ))
    })?;
    let latest_audit = latest_audit.ok_or_else(|| {
        SoracloudError::internal(format!(
            "service `{}` active revision `{}` has no authoritative lifecycle audit event",
            deployment.service_name, deployment.current_service_version
        ))
    })?;
    latest_audit.validate().map_err(|error| {
        SoracloudError::internal(format!(
            "service `{}` active revision `{}` lifecycle audit event is invalid: {error}",
            deployment.service_name, deployment.current_service_version
        ))
    })?;
    if latest_audit.action == SoraServiceLifecycleActionV1::Rollout
        && (latest_audit.governance_tx_hash.is_none() || latest_audit.rollout_state.is_none())
    {
        return Err(SoracloudError::internal(format!(
            "service `{}` active revision `{}` rollout audit event is missing governance or rollout identity",
            deployment.service_name, deployment.current_service_version
        )));
    }
    if !matches!(
        latest_audit.action,
        SoraServiceLifecycleActionV1::Deploy
            | SoraServiceLifecycleActionV1::Upgrade
            | SoraServiceLifecycleActionV1::Rollout
            | SoraServiceLifecycleActionV1::Rollback
    ) || latest_audit.service_name != deployment.service_name
        || latest_audit.to_version != deployment.current_service_version
        || latest_audit.service_manifest_hash != bundle.service_manifest_hash()
        || latest_audit.container_manifest_hash != bundle.container_manifest_hash()
    {
        return Err(SoracloudError::internal(format!(
            "service `{}` active revision `{}` lifecycle audit event does not bind the admitted bundle",
            deployment.service_name, deployment.current_service_version
        )));
    }
    if let Some(public_discovery) = public_discovery {
        validate_public_service_discovery_for_bundle(deployment, bundle, public_discovery)?;
    }
    let route = bundle.service.route.as_ref();
    let state_binding_count = u32::try_from(bundle.service.state_bindings.len()).map_err(|_| {
        SoracloudError::internal(format!(
            "service `{}` active revision `{}` state-binding count exceeds the V1 u32 response range",
            deployment.service_name, deployment.current_service_version
        ))
    })?;
    Ok(ControlPlaneServiceRevision {
        sequence: latest_audit.sequence,
        action: audit_action_to_control_plane_action(latest_audit.action),
        service_version: bundle.service.service_version.clone(),
        service_manifest_hash: bundle.service_manifest_hash(),
        container_manifest_hash: bundle.container_manifest_hash(),
        replicas: bundle.service.replicas.get(),
        execution_plane: bundle.service.execution_plane,
        route_host: route.map(|route| route.host.clone()),
        route_path_prefix: route.map(|route| route.path_prefix.clone()),
        route_service_port: route.map(|route| route.service_port.get()),
        route_visibility: route.map(|route| format!("{:?}", route.visibility)),
        route_tls_mode: route.map(|route| format!("{:?}", route.tls_mode)),
        base_url: bundle_base_url(bundle),
        healthcheck_url: bundle_healthcheck_url(bundle),
        public_discovery_content_cid: public_discovery.map(|entry| entry.content_cid.clone()),
        public_discovery_url: public_discovery.map(|entry| entry.public_discovery_url.clone()),
        public_discovery_cid_host_url: public_discovery
            .map(|entry| entry.public_discovery_cid_host_url.clone()),
        state_binding_count,
        state_bindings: bundle.service.state_bindings.clone(),
        lease_volumes: bundle.service.lease_volumes.clone(),
        allow_model_inference: host_admission.allow_model_inference,
        allow_model_training: host_admission.allow_model_training,
        runtime: host_admission.runtime,
        allow_wallet_signing: host_admission.allow_wallet_signing,
        allow_state_writes: host_admission.allow_state_writes,
        network: host_admission.network,
        cpu_millis: host_admission.cpu_millis,
        memory_bytes: host_admission.memory_bytes,
        ephemeral_storage_bytes: host_admission.ephemeral_storage_bytes,
        max_open_files: host_admission.max_open_files,
        max_tasks: host_admission.max_tasks,
        start_grace_secs: host_admission.start_grace_secs,
        stop_grace_secs: host_admission.stop_grace_secs,
        healthcheck_path: host_admission.healthcheck_path,
        required_config_names: bundle.container.required_config_names.clone(),
        required_secret_names: bundle.container.required_secret_names.clone(),
        config_exports: bundle.container.config_exports.clone(),
        sandbox_profile_hash: host_admission.sandbox_profile_hash,
        process_generation: deployment.process_generation,
        process_started_sequence: deployment.process_started_sequence,
        signed_by: latest_audit.signer.to_string(),
    })
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
        if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::DeterministicService
            || !bundle.container.runtime.is_deterministic()
        {
            continue;
        }
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
fn public_method_supports_handler(
    request_method: &str,
    handler_class: iroha_data_model::soracloud::SoraServiceHandlerClassV1,
) -> bool {
    if request_method.eq_ignore_ascii_case("GET") || request_method.eq_ignore_ascii_case("HEAD") {
        matches!(
            handler_class,
            iroha_data_model::soracloud::SoraServiceHandlerClassV1::Asset
                | iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query
        )
    } else {
        false
    }
}
pub(crate) fn resolve_public_route(
    app: &SharedAppState,
    host: &str,
    request_method: &str,
    request_path: &str,
) -> Option<PublicRouteMatch> {
    let normalized_host = normalize_public_route_host(host);
    if normalized_host.is_empty() {
        return None;
    }
    let normalized_path = normalize_public_route_path(request_path);
    let state_view = app.state.view();
    let world = state_view.world();
    let current_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let mut best_match: Option<(usize, PublicRouteMatch, (String, String, String))> = None;
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
        if bundle.service.execution_plane == SoraServiceExecutionPlaneV1::HttpService {
            if iroha_core::soracloud_runtime::validate_soracloud_deployment_lease_volume_bindings(
                deployment, bundle,
            )
            .is_err()
            {
                continue;
            }
            if !deployment
                .hosted_service_lease_active_at(current_height)
                .unwrap_or(false)
                || deployment
                    .lease_volume_states
                    .iter()
                    .any(|volume| !volume.is_active_at(current_height))
            {
                continue;
            }
            if bundle.container.runtime
                != iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou
            {
                continue;
            }
            let Some(request_path) =
                split_public_handler_path(normalized_path, route.path_prefix.as_str())
            else {
                continue;
            };
            let route_len = route.path_prefix.len();
            let route_match = PublicRouteMatch::HostedHttp(HostedHttpRouteMatch {
                service_name: service_name.clone(),
                service_version: deployment.current_service_version.clone(),
                request_path,
            });
            let sort_key = (
                service_name.clone(),
                deployment.current_service_version.clone(),
                String::new(),
            );
            let replace = best_match
                .as_ref()
                .is_none_or(|(best_len, _, best_sort_key)| {
                    route_len > *best_len || (route_len == *best_len && sort_key < *best_sort_key)
                });
            if replace {
                best_match = Some((route_len, route_match, sort_key));
            }
            continue;
        }
        if !bundle.container.runtime.is_deterministic() {
            continue;
        }
        for handler in &bundle.service.handlers {
            if !public_method_supports_handler(request_method, handler.class) {
                continue;
            }
            let route_match = match handler.class {
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Asset => {
                    let full_route = join_public_route_paths(
                        route.path_prefix.as_str(),
                        handler.route_path.as_deref().unwrap_or("/"),
                    );
                    let Some(handler_path) =
                        split_public_handler_path(normalized_path, &full_route)
                    else {
                        continue;
                    };
                    let route_len = full_route.len();
                    let route_match = PublicRouteMatch::LocalRead(LocalReadRouteMatch {
                        service_name: service_name.clone(),
                        service_version: deployment.current_service_version.clone(),
                        handler_name: handler.handler_name.to_string(),
                        handler_class: SoracloudLocalReadKind::Asset,
                        handler_path,
                    });
                    (route_len, route_match)
                }
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query => {
                    let full_route = join_public_route_paths(
                        route.path_prefix.as_str(),
                        handler.route_path.as_deref().unwrap_or("/"),
                    );
                    let Some(handler_path) =
                        split_public_handler_path(normalized_path, &full_route)
                    else {
                        continue;
                    };
                    let route_len = full_route.len();
                    let route_match = PublicRouteMatch::LocalRead(LocalReadRouteMatch {
                        service_name: service_name.clone(),
                        service_version: deployment.current_service_version.clone(),
                        handler_name: handler.handler_name.to_string(),
                        handler_class: SoracloudLocalReadKind::Query,
                        handler_path,
                    });
                    (route_len, route_match)
                }
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update
                | iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate => {
                    continue;
                }
            };
            let (route_len, route_match) = route_match;
            let sort_key = (
                service_name.clone(),
                deployment.current_service_version.clone(),
                handler.handler_name.to_string(),
            );
            let replace = best_match
                .as_ref()
                .is_none_or(|(best_len, _, best_sort_key)| {
                    route_len > *best_len || (route_len == *best_len && sort_key < *best_sort_key)
                });
            if replace {
                best_match = Some((route_len, route_match, sort_key));
            }
        }
    }
    best_match.map(|(_route_len, route_match, _)| route_match)
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
) -> Result<ControlPlaneSnapshot, SoracloudError> {
    let validated_audit_log = authoritative_audit_log(app)?;
    let state_view = app.state.view();
    let world = state_view.world();
    let current_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
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
            .cloned()
            .ok_or_else(|| {
                SoracloudError::internal(format!(
                    "service `{service_label}` active revision `{}` is missing from authoritative state",
                    deployment.current_service_version
                ))
            })?;
        if &deployment.service_name != service_id
            || deployment.current_service_manifest_hash != current_bundle.service_manifest_hash()
            || deployment.current_container_manifest_hash
                != current_bundle.container_manifest_hash()
        {
            return Err(SoracloudError::internal(format!(
                "service `{service_label}` deployment state does not bind its active revision `{}`",
                deployment.current_service_version
            )));
        }
        iroha_core::soracloud_runtime::validate_soracloud_deployment_lease_volume_bindings(
            deployment,
            &current_bundle,
        )
        .map_err(|message| {
            SoracloudError::internal(format!(
                "service `{service_label}` has invalid authoritative lease-volume state: {message}"
            ))
        })?;
        let current_public_discovery = authoritative_public_service_discovery_for_version(
            deployment,
            deployment.current_service_version.as_str(),
        )?;
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
            .max_by_key(|event| event.sequence)
            .ok_or_else(|| {
                SoracloudError::internal(format!(
                    "service `{service_label}` active revision `{}` has no authoritative lifecycle audit event",
                    deployment.current_service_version
                ))
            })?;
        let accounted_storage_bytes = deployment.accounted_storage_bytes();
        let service_lease = deployment
            .service_lease
            .as_ref()
            .map(|lease| -> Result<_, NumericOperationError> {
                Ok(ControlPlaneServiceLeaseSnapshot {
                    authoritative_state: lease.clone(),
                    effective_status: lease.status_at(current_height, accounted_storage_bytes)?,
                    remaining_runtime_balance: lease
                        .remaining_balance(current_height, accounted_storage_bytes)?,
                })
            })
            .transpose()?;
        services.push(ControlPlaneServiceSnapshot {
            service_name: service_label.clone(),
            current_version: deployment.current_service_version.clone(),
            revision_count: deployment.revision_count,
            config_generation: deployment.config_generation,
            secret_generation: deployment.secret_generation,
            config_entry_count: u32::try_from(deployment.service_configs.len()).map_err(|_| {
                SoracloudError::internal(format!(
                    "service `{service_label}` config count exceeds the V1 u32 response range"
                ))
            })?,
            secret_entry_count: u32::try_from(deployment.service_secrets.len()).map_err(|_| {
                SoracloudError::internal(format!(
                    "service `{service_label}` secret count exceeds the V1 u32 response range"
                ))
            })?,
            service_lease,
            public_discovery_content_cid: current_public_discovery
                .as_ref()
                .map(|entry| entry.content_cid.clone()),
            public_discovery_url: current_public_discovery
                .as_ref()
                .map(|entry| entry.public_discovery_url.clone()),
            public_discovery_cid_host_url: current_public_discovery
                .as_ref()
                .map(|entry| entry.public_discovery_cid_host_url.clone()),
            latest_revision: Some(deployment_bundle_to_control_plane_revision(
                deployment,
                &current_bundle,
                Some(latest_audit),
                current_public_discovery.as_ref(),
            )?),
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
    let validated_audit_event_count = validated_audit_log.len();
    let mut recent_audit_events = validated_audit_log
        .into_iter()
        .filter(|event| service_name.is_none_or(|filter| filter == event.service_name))
        .collect::<Vec<_>>();
    recent_audit_events.sort_by_key(|event| std::cmp::Reverse(event.sequence));
    recent_audit_events.truncate(limit);
    Ok(ControlPlaneSnapshot {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        service_count: u32::try_from(services.len()).map_err(|_| {
            SoracloudError::internal("Soracloud service count exceeds the V1 u32 response range")
        })?,
        audit_event_count: u32::try_from(validated_audit_event_count).map_err(|_| {
            SoracloudError::internal(
                "Soracloud audit event count exceeds the V1 u32 response range",
            )
        })?,
        services,
        recent_audit_events,
    })
}
fn authoritative_app_infra_status_response(
    app: &SharedAppState,
    app_name: Option<&str>,
    audit_limit: usize,
) -> Result<AppInfraStatusResponse, SoracloudError> {
    let app_filter = app_name
        .map(str::parse::<Name>)
        .transpose()
        .map_err(|err| SoracloudError::bad_request(format!("invalid app_name: {err}")))?;
    let state_view = app.state.view();
    let world = state_view.world();
    let mut apps = world
        .soracloud_app_infra_states()
        .iter()
        .filter(|(_name, state)| {
            app_filter
                .as_ref()
                .is_none_or(|filter| filter == &state.app_name)
        })
        .map(|(_name, state)| state.clone())
        .collect::<Vec<_>>();
    apps.sort_by(|left, right| left.app_name.as_ref().cmp(right.app_name.as_ref()));
    if app_filter.is_some() && apps.is_empty() {
        return Err(SoracloudError::not_found(format!(
            "app `{}` not found in authoritative Soracloud app infra state",
            app_name.unwrap_or_default()
        )));
    }
    let limit = audit_limit.max(1).min(MAX_AUDIT_LIMIT);
    let mut recent_audit_events = world
        .soracloud_app_infra_audit_events()
        .iter()
        .filter(|(_sequence, event)| {
            app_filter
                .as_ref()
                .is_none_or(|filter| filter == &event.app_name)
        })
        .map(|(_sequence, event)| event.clone())
        .collect::<Vec<_>>();
    recent_audit_events.sort_by_key(|event| std::cmp::Reverse(event.sequence));
    recent_audit_events.truncate(limit);
    Ok(AppInfraStatusResponse {
        schema_version: CONTROL_PLANE_SCHEMA_VERSION,
        app_count: u32::try_from(apps.len()).unwrap_or(u32::MAX),
        audit_event_count: u32::try_from(world.soracloud_app_infra_audit_events().iter().count())
            .unwrap_or(u32::MAX),
        apps,
        recent_audit_events,
    })
}
pub(crate) async fn handle_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/deploy").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_bundle_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = admit_scr_host_bundle(&request.bundle) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
            precondition: request.precondition,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/upgrade").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_bundle_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = admit_scr_host_bundle(&request.bundle) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
            precondition: request.precondition,
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
pub(crate) async fn handle_app_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAppInfraRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/apps/deploy").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_app_infra_signature(&request) {
        return err.into_response();
    }
    let SignedAppInfraRequest {
        deploy_services,
        upgrade_services,
        manifest,
        precondition,
        provenance,
    } = request;
    let signer = match require_soracloud_mutation_signer(&headers, &provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let app_signer = provenance.signer.clone();
    let app_name = manifest.app_name.to_string();
    let mut instructions = Vec::new();
    for service_request in deploy_services {
        match app_service_bundle_instruction(service_request, MutationMode::Deploy, &app_signer) {
            Ok(instruction) => instructions.push(instruction),
            Err(err) => return err.into_response(),
        }
    }
    for service_request in upgrade_services {
        match app_service_bundle_instruction(service_request, MutationMode::Upgrade, &app_signer) {
            Ok(instruction) => instructions.push(instruction),
            Err(err) => return err.into_response(),
        }
    }
    instructions.push(InstructionBox::from(
        isi::soracloud::DeploySoracloudAppInfra {
            manifest,
            precondition,
            provenance,
        },
    ));
    match submit_confirm_and_respond_instructions(
        &app,
        signer,
        instructions,
        "/v1/soracloud/apps/deploy",
        move |app, _baseline| {
            authoritative_app_infra_status_response(app, Some(&app_name), DEFAULT_AUDIT_LIMIT)
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_app_upgrade(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAppInfraRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/apps/upgrade").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_app_infra_signature(&request) {
        return err.into_response();
    }
    let SignedAppInfraRequest {
        deploy_services,
        upgrade_services,
        manifest,
        precondition,
        provenance,
    } = request;
    let signer = match require_soracloud_mutation_signer(&headers, &provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let app_signer = provenance.signer.clone();
    let app_name = manifest.app_name.to_string();
    let mut instructions = Vec::new();
    for service_request in deploy_services {
        match app_service_bundle_instruction(service_request, MutationMode::Deploy, &app_signer) {
            Ok(instruction) => instructions.push(instruction),
            Err(err) => return err.into_response(),
        }
    }
    for service_request in upgrade_services {
        match app_service_bundle_instruction(service_request, MutationMode::Upgrade, &app_signer) {
            Ok(instruction) => instructions.push(instruction),
            Err(err) => return err.into_response(),
        }
    }
    instructions.push(InstructionBox::from(
        isi::soracloud::UpgradeSoracloudAppInfra {
            manifest,
            precondition,
            provenance,
        },
    ));
    match submit_confirm_and_respond_instructions(
        &app,
        signer,
        instructions,
        "/v1/soracloud/apps/upgrade",
        move |app, _baseline| {
            authoritative_app_infra_status_response(app, Some(&app_name), DEFAULT_AUDIT_LIMIT)
        },
    )
    .await
    {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_app_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<AppInfraStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/apps/status").await
    {
        return err.into_response();
    }
    match authoritative_app_infra_status_response(
        &app,
        query.app_name.as_deref(),
        query.audit_limit.unwrap_or(DEFAULT_AUDIT_LIMIT),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_named_app_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    Path(app_name): Path<String>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<AppInfraStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/apps/{app_name}/status",
    )
    .await
    {
        return err.into_response();
    }
    match authoritative_app_infra_status_response(
        &app,
        Some(&app_name),
        query.audit_limit.unwrap_or(DEFAULT_AUDIT_LIMIT),
    ) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_rollback(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedRollbackRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/rollback").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_rollback_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedRolloutAdvanceRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/rollout").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_rollout_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedStateMutationRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/state/mutate").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_state_mutation_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    let value_payload = match decode_state_mutation_payload(&request.payload) {
        Ok(payload) => payload,
        Err(err) => return err.into_response(),
    };
    let value_size_bytes = value_payload.as_ref().map(|payload| {
        u64::try_from(payload.len()).expect("payload length already validated for u64")
    });
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::MutateSoracloudState {
            service_name,
            binding_name,
            state_key: request.payload.key,
            operation: state_mutation_operation_to_model(request.payload.operation),
            value_size_bytes,
            value_payload,
            encryption: request.payload.encryption,
            governance_tx_hash: request.payload.governance_tx_hash,
            fhe_input_admission_proof: request.payload.fhe_input_admission_proof,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedServiceConfigSetRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/config/set",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_service_config_set_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedServiceConfigDeleteRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/config/delete",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_service_config_delete_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ServiceConfigStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/config/status",
    )
    .await
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
pub(crate) async fn handle_service_public_discovery(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(service_name): Path<String>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/services/{service_name}/public-discovery",
    )
    .await
    {
        return err.into_response();
    }
    let service_name = match parse_service_name(&service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    match authoritative_service_public_discovery_response(&app, service_name.as_ref(), None) {
        Ok(response) => bounded_public_response::json(&response),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_service_revision_public_discovery(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path((service_name, service_version)): Path<(String, String)>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/services/{service_name}/revisions/{service_version}/public-discovery",
    )
    .await
    {
        return err.into_response();
    }
    let service_name = match parse_service_name(&service_name) {
        Ok(service_name) => service_name,
        Err(err) => return err.into_response(),
    };
    let service_version = service_version.trim();
    if service_version.is_empty() {
        return SoracloudError::bad_request("service_version must not be empty").into_response();
    }
    match authoritative_service_public_discovery_response(
        &app,
        service_name.as_ref(),
        Some(service_version),
    ) {
        Ok(response) => bounded_public_response::json(&response),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_service_secret_set(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedServiceSecretSetRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/secret/set",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_service_secret_set_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedServiceSecretDeleteRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/secret/delete",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_service_secret_delete_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ServiceSecretStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/service/secret/status",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedFheJobRunRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/fhe/job/run").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_fhe_job_run_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = validate_fhe_job_run_proof_attachments(&request.payload) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RunSoracloudFheJob {
            service_name,
            binding_name,
            job: request.payload.job,
            policy_reference: request.payload.policy_reference,
            public_key_proof: request.payload.public_key_proof,
            bootstrap_key_zero_refresh_proof: request.payload.bootstrap_key_zero_refresh_proof,
            full_bootstrap_execution_proofs: request.payload.full_bootstrap_execution_proofs,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedDecryptionRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/decrypt/request",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_decryption_request_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedDecryptionRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/health/access/request",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_decryption_request_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedCiphertextQueryRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/ciphertext/query",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedTrainingJobStartRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/training/job/start",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_training_job_start_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedTrainingJobCheckpointRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/training/job/checkpoint",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_training_job_checkpoint_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedTrainingJobRetryRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/training/job/retry",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_training_job_retry_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<TrainingJobStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/training/job/status",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelWeightRegisterRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/weight/register",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_weight_register_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelWeightPromoteRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/weight/promote",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_weight_promote_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelWeightRollbackRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/weight/rollback",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_weight_rollback_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ModelWeightStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/weight/status",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelArtifactRegisterRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/artifact/register",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_artifact_register_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ModelArtifactStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/artifact/status",
    )
    .await
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
pub(crate) async fn handle_uploaded_model_register(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedUploadedModelRegisterRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/upload/register",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_uploaded_model_register_signature(&request) {
        return err.into_response();
    }
    if let Err(err) = require_active_sorafs_uploaded_model_pin(&app, &request.payload.bundle) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.bundle_provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let service_name = request.payload.bundle.service_name.clone();
    let service_label = service_name.to_string();
    let model_id = request.payload.bundle.model_id.clone();
    let weight_version = request.payload.bundle.weight_version.clone();
    let signed_by = request.bundle_provenance.signer.to_string();
    let finalize_payload = request.payload.clone();
    let instructions = vec![
        InstructionBox::from(isi::soracloud::RegisterSoracloudUploadedModelBundle {
            bundle: request.payload.bundle,
            provenance: request.bundle_provenance,
        }),
        InstructionBox::from(isi::soracloud::FinalizeSoracloudUploadedModelBundle {
            service_name,
            model_name: finalize_payload.model_name,
            model_id: finalize_payload.bundle.model_id,
            artifact_id: finalize_payload.artifact_id,
            weight_version: finalize_payload.bundle.weight_version,
            bundle_root: finalize_payload.bundle.bundle_root,
            weight_artifact_hash: finalize_payload.weight_artifact_hash,
            dataset_ref: finalize_payload.dataset_ref,
            training_config_hash: finalize_payload.training_config_hash,
            reproducibility_hash: finalize_payload.reproducibility_hash,
            provenance_attestation_hash: finalize_payload.provenance_attestation_hash,
            provenance: request.finalize_provenance,
        }),
    ];
    match submit_confirm_and_respond_instructions(
        &app,
        signer,
        instructions,
        "/v1/soracloud/model/upload/register",
        move |app, _baseline| {
            authoritative_uploaded_model_status_response(
                app,
                &service_label,
                &model_id,
                &weight_version,
            )
            .map(|status| UploadedModelMutationResponse {
                action: UploadedModelAction::Register,
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
pub(crate) async fn handle_uploaded_model_encryption_recipient(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
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
    bounded_public_response::encryption_recipient(recipient)
}
pub(crate) async fn handle_uploaded_model_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<UploadedModelStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/upload/status",
    )
    .await
    {
        return err.into_response();
    }
    match authoritative_uploaded_model_status_from_query(&app, &query) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_uploaded_model_private_execute(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<PrivateUploadedModelExecuteRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/upload/private/execute",
    )
    .await
    {
        return err.into_response();
    }
    let (_, _, verified_request_signers) = match verified_soracloud_request_identity(&headers) {
        Ok(identity) => identity,
        Err(err) => return err.into_response(),
    };
    let private_execution_permit = match app
        .soracloud_private_execution_inflight
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,
        Err(_) => {
            return SoracloudError::unavailable(
                    "private execution memory capacity is busy; retry after the active request completes",
                )
                .into_response();
        }
    };
    let worker = tokio::task::spawn_blocking(move || {
        let _private_execution_permit = private_execution_permit;
        authoritative_private_uploaded_model_execute_response(
            &app,
            request,
            &verified_request_signers,
        )
    })
    .await;
    match worker {
        Ok(Ok((status, response))) => (status, JsonBody(response)).into_response(),
        Ok(Err(err)) => err.into_response(),
        Err(err) => SoracloudError::internal(format!(
            "private uploaded-model execution worker failed: {err}"
        ))
        .into_response(),
    }
}
pub(crate) async fn handle_uploaded_model_private_receipts(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<PrivateUploadedModelReceiptQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model/upload/private/receipts",
    )
    .await
    {
        return err.into_response();
    }
    match authoritative_private_uploaded_model_receipts_response(&app, query) {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}
pub(crate) async fn handle_hf_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedHfDeployRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/hf/deploy").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_hf_deploy_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_revision(&request.payload.revision) {
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
    let base_fee = request.payload.base_fee.clone();
    let source_id = match hf_source_id(&repo_id, &resolved_revision) {
        Ok(source_id) => source_id,
        Err(err) => return err.into_response(),
    };
    let resource_profile =
        match derive_hf_resource_profile(&app.soracloud_hf_config, &repo_id, &resolved_revision)
            .await
        {
            Ok(resource_profile) => resource_profile,
            Err(err) => return err.into_response(),
        };
    let max_compute_reservation_fee =
        match hf_shared_lease_max_compute_reservation_fee_v1(&resource_profile, lease_term_ms) {
            Ok(max_compute_reservation_fee) => max_compute_reservation_fee,
            Err(err) => {
                return SoracloudError::bad_request(format!(
                    "failed to quote HF compute reservation cap: {err}"
                ))
                .into_response();
            }
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
            base_fee,
            resource_profile: Some(resource_profile),
            max_compute_reservation_fee,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<HfSharedLeaseStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/hf/status").await
    {
        return err.into_response();
    }
    let repo_id = match parse_hf_repo_id(&query.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_revision(&query.revision) {
        Ok(resolved_revision) => resolved_revision,
        Err(err) => return err.into_response(),
    };
    let storage_class = match parse_storage_class_query(&query.storage_class) {
        Ok(storage_class) => storage_class,
        Err(err) => return err.into_response(),
    };
    let account_id = match parse_optional_account_id(
        app.state.as_ref(),
        &app.telemetry,
        "/v1/soracloud/hf/status#account_id",
        query.account_id.as_deref(),
    ) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedHfLeaseLeaveRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/hf/lease/leave",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_hf_lease_leave_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_revision(&request.payload.revision) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedHfLeaseRenewRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/hf/lease/renew",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_hf_lease_renew_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let authority = signer.authority.clone();
    let repo_id = match parse_hf_repo_id(&request.payload.repo_id) {
        Ok(repo_id) => repo_id,
        Err(err) => return err.into_response(),
    };
    let resolved_revision = match parse_hf_revision(&request.payload.revision) {
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
    let base_fee = request.payload.base_fee.clone();
    let source_id = match hf_source_id(&repo_id, &resolved_revision) {
        Ok(source_id) => source_id,
        Err(err) => return err.into_response(),
    };
    let resource_profile =
        match derive_hf_resource_profile(&app.soracloud_hf_config, &repo_id, &resolved_revision)
            .await
        {
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
            base_fee,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ModelHostStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model-host/status",
    )
    .await
    {
        return err.into_response();
    }
    let validator_account_id = match parse_optional_account_id(
        app.state.as_ref(),
        &app.telemetry,
        "/v1/soracloud/model-host/status#account_id",
        query.account_id.as_deref(),
    ) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelHostAdvertiseRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model-host/advertise",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_host_advertise_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelHostHeartbeatRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model-host/heartbeat",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_host_heartbeat_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedModelHostWithdrawRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/model-host/withdraw",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_model_host_withdraw_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentDeployRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/agent/deploy").await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_deploy_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
        Ok(signer) => signer,
        Err(err) => return err.into_response(),
    };
    let autonomy_budget_units = request.payload.autonomy_budget_units;
    let apartment_name = request.payload.manifest.apartment_name.to_string();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::DeploySoracloudAgentApartment {
            manifest: request.payload.manifest,
            lease_blocks: request.payload.lease_blocks,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentLeaseRenewRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/lease/renew",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_lease_renew_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
            lease_blocks: request.payload.lease_blocks,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentRestartRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/restart",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_restart_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<AgentStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) =
        crate::check_access(&app, &headers, Some(remote_ip), "v1/soracloud/agent/status").await
    {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentWalletSpendRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/wallet/spend",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_wallet_spend_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    if request.payload.amount.is_zero() {
        return SoracloudError::bad_request("amount must be greater than zero").into_response();
    }
    let amount = request.payload.amount.clone();
    match submit_confirm_and_respond(
        &app,
        signer,
        InstructionBox::from(isi::soracloud::RequestSoracloudAgentWalletSpend {
            apartment_name,
            asset_definition: request.payload.asset_definition,
            amount: request.payload.amount,
            provenance: request.provenance,
        }),
        "/v1/soracloud/agent/wallet/spend",
        move |app, baseline| {
            authoritative_agent_wallet_request_mutation_response(
                app,
                baseline,
                &apartment_label,
                &asset_definition,
                &amount,
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentWalletApproveRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/wallet/approve",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_wallet_approve_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentPolicyRevokeRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/policy/revoke",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_policy_revoke_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentMessageSendRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/message/send",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_message_send_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentMessageAckRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/message/ack",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_message_ack_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<AgentMailboxStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/mailbox/status",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentArtifactAllowRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/autonomy/allow",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_artifact_allow_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    let approved_process_generation = approved_run.approved_process_generation;
    let approved_request_commitment = approved_run.request_commitment;
    let observed_height = u64::try_from(state_view.height()).unwrap_or(u64::MAX);
    let observed_block_hash = state_view.latest_block_hash().map(Hash::from);
    drop(state_view);
    let request = SoracloudApartmentExecutionRequest {
        observed_height,
        observed_block_hash,
        apartment_name: response.apartment_name.clone(),
        process_generation: record.process_generation,
        operation: format!("autonomy-run:{run_id}"),
        request_commitment: approved_request_commitment,
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
        approved_process_generation,
        approved_request_commitment,
    )
    .map_err(|error| error.message)
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
                emitted_sequence: 0,
                execution_host: runtime_receipt.execution_host.clone(),
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
        if let Some(event) = state_view
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
        {
            let expected_receipt_id = runtime_receipt_id.or_else(|| {
                runtime_execution
                    .runtime_receipt
                    .as_ref()
                    .map(|receipt| receipt.receipt_id)
            });
            if event.succeeded == Some(runtime_execution.succeeded)
                && event.result_commitment == Some(runtime_execution.result_commitment)
                && event.service_name == runtime_execution.service_name
                && event.service_version == runtime_execution.service_version
                && event.handler_name == runtime_execution.handler_name
                && event.runtime_receipt_id == expected_receipt_id
                && event.journal_artifact_hash == Some(runtime_execution.journal_artifact_hash)
                && event.checkpoint_artifact_hash == runtime_execution.checkpoint_artifact_hash
                && event.reason == runtime_execution.error
            {
                return Ok(None);
            }
            return Err(format!(
                "autonomy run `{}` already has a different authoritative execution outcome",
                runtime_execution.run_id
            ));
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<SignedAgentAutonomyRunRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/autonomy/run",
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = verify_agent_autonomy_run_signature(&request) {
        return err.into_response();
    }
    let signer = match require_soracloud_mutation_signer(&headers, &request.provenance) {
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<AgentAutonomyFinalizeRequest>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/autonomy/run",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<AgentAutonomyStatusQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
        "v1/soracloud/agent/autonomy/status",
    )
    .await
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
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<HealthComplianceReportQuery>,
) -> Response {
    let remote_ip = remote.ip();
    if let Err(err) = crate::check_access(
        &app,
        &headers,
        Some(remote_ip),
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
    const TEST_HF_COMMIT_OID: &str = "0123456789abcdef0123456789abcdef01234567";
    use crate::tests_runtime_handlers::{
        mk_app_state_for_tests, mk_app_state_for_tests_with_world,
    };
    use iroha_core::soracloud_runtime::{
        SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
        SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentExecutionRequest,
        SoracloudApartmentExecutionResult, SoracloudLocalReadRequest, SoracloudLocalReadResponse,
        SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
        SoracloudRuntime, SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
        SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
        derive_soracloud_apartment_autonomy_result_commitment_v1,
    };
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        Encode,
        account::{Account, AccountId},
        asset::AssetDefinitionId,
        domain::{Domain, DomainId},
        isi::Grant,
        metadata::Metadata,
        name::Name,
        permission::Permission,
        prelude::Register,
        sns::{NameControllerV1, NameRecordV1},
        soracloud::{
            AgentApartmentManifestV1, CiphertextQueryMetadataLevelV1, CiphertextQuerySpecV1,
            DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheJobSpecV1,
            SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1, SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1, SORA_HF_SOURCE_RECORD_VERSION_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1, SecretEnvelopeEncryptionV1,
            SecretEnvelopeV1, SoraAgentApartmentActionV1, SoraAgentApartmentAuditEventV1,
            SoraAgentAutonomyRunRecordV1, SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1,
            SoraContainerManifestV1, SoraHfSharedLeaseActionV1, SoraHfSharedLeaseAuditEventV1,
            SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1,
            SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1, SoraHfSourceStatusV1,
            SoraModelProvenanceKindV1, SoraModelProvenanceRefV1, SoraRouteTargetV1,
            SoraRouteVisibilityV1, SoraServiceAuditEventV1, SoraServiceConfigEntryV1,
            SoraServiceDeploymentStateV1, SoraServiceHandlerV1, SoraServiceLifecycleActionV1,
            SoraServiceManifestV1, SoraServiceSecretEntryV1, SoraServiceStateEntryV1,
            SoraStateEncryptionV1, SoraTlsModeV1, SoraUploadedModelBundleV1,
            SoraUploadedModelPricingPolicyV1, SoraUploadedModelRuntimeFormatV1,
        },
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestRecord, PinPolicy,
            StorageClass,
        },
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID};
    use std::{
        fs,
        num::{NonZeroU16, NonZeroU32, NonZeroU64},
        path::{Path, PathBuf},
        sync::Arc,
    };
    #[test]
    fn private_execution_submission_phase_is_closed_and_matches_the_durable_journal() {
        let transaction_hash = Hash::new(b"private execution phase transaction");
        for (journal_phase, hash, expected) in [
            (
                SoracloudPrivateUploadedModelExecutionJournalPhaseV1::OutputPin,
                None,
                PrivateUploadedModelSubmissionPhaseV1::AwaitingOutputDurability,
            ),
            (
                SoracloudPrivateUploadedModelExecutionJournalPhaseV1::OutputPin,
                Some(&transaction_hash),
                PrivateUploadedModelSubmissionPhaseV1::OutputPinSubmitted,
            ),
            (
                SoracloudPrivateUploadedModelExecutionJournalPhaseV1::Receipt,
                Some(&transaction_hash),
                PrivateUploadedModelSubmissionPhaseV1::ReceiptSubmitted,
            ),
        ] {
            assert_eq!(
                private_uploaded_model_submission_phase(
                    SoracloudPrivateUploadedModelExecutionSubmissionProgressV1 {
                        phase: journal_phase,
                        transaction_hash: hash.copied(),
                    },
                )
                .expect("valid journal phase"),
                expected
            );
        }
        for phase in [
            PrivateUploadedModelSubmissionPhaseV1::AwaitingOutputDurability,
            PrivateUploadedModelSubmissionPhaseV1::OutputPinSubmitted,
            PrivateUploadedModelSubmissionPhaseV1::ReceiptSubmitted,
            PrivateUploadedModelSubmissionPhaseV1::Committed,
        ] {
            let encoded = norito::json::to_string(&phase).expect("encode phase label");
            assert_eq!(encoded, format!("\"{}\"", phase.as_str()));
            assert_eq!(
                norito::json::from_str::<PrivateUploadedModelSubmissionPhaseV1>(&encoded)
                    .expect("decode phase label"),
                phase
            );
        }
        assert!(
            private_uploaded_model_submission_phase(
                SoracloudPrivateUploadedModelExecutionSubmissionProgressV1 {
                    phase: SoracloudPrivateUploadedModelExecutionJournalPhaseV1::Receipt,
                    transaction_hash: None,
                },
            )
            .is_err(),
            "receipt phase must always identify its durable transaction"
        );
        assert!(
            "submitted"
                .parse::<PrivateUploadedModelSubmissionPhaseV1>()
                .is_err(),
            "the retired aggregate status must not remain accepted"
        );
    }
    #[test]
    fn private_execution_retention_margin_covers_both_phases_replication_and_consensus() {
        let margin = private_execution_required_retention_margin(
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        let per_phase = 10
            * u64::from(SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_MAX_SUBMISSION_ATTEMPTS_V1)
            + 10;
        assert_eq!(
            margin,
            Duration::from_secs(
                SORACLOUD_PRIVATE_OUTPUT_MIN_RETENTION_SECS_V1
                    + u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1)
                    + per_phase * u64::from(PRIVATE_EXECUTION_SUBMISSION_PHASE_COUNT_V1)
                    + PRIVATE_EXECUTION_RETENTION_SAFETY_SECS_V1
            )
        );
    }
    #[test]
    fn private_execution_release_window_is_half_open_for_the_receipt_candidate() {
        assert_eq!(
            private_execution_release_candidate_is_authorized(10, 4, 13),
            Some((14, true))
        );
        assert_eq!(
            private_execution_release_candidate_is_authorized(10, 4, 14),
            Some((14, false))
        );
        assert_eq!(
            private_execution_release_candidate_is_authorized(10, 4, 9),
            Some((14, false))
        );
        assert_eq!(
            private_execution_release_candidate_is_authorized(u64::MAX, 1, u64::MAX),
            None
        );
        assert_eq!(PRIVATE_EXECUTION_INITIAL_RECEIPT_BLOCK_OFFSET_V1, 3);
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn autonomy_summary_reader_accepts_v1_limit_and_rejects_first_overflow_byte() {
        let directory = tempfile::tempdir().expect("temporary autonomy summary directory");
        let path = directory.path().join("execution_summary.json");
        let limit = SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_MAX_BYTES_V1;
        let file = fs::File::create(&path).expect("create exact-bound sparse summary");
        file.set_len(limit)
            .expect("size exact-bound sparse summary");
        drop(file);
        let exact = read_agent_runtime_execution_summary_bytes(&path)
            .expect("an exact-bound direct summary is accepted");
        assert_eq!(u64::try_from(exact.len()).expect("length fits u64"), limit);
        drop(exact);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("reopen summary for overflow fixture");
        file.set_len(limit.saturating_add(1))
            .expect("size overflowing sparse summary");
        drop(file);
        let error = read_agent_runtime_execution_summary_bytes(&path)
            .expect_err("the first byte beyond the V1 summary ceiling must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    fn agent_runtime_receipt_json_fixture() -> AgentRuntimeReceiptRecord {
        AgentRuntimeReceiptRecord {
            receipt_id: Hash::new(b"agent runtime receipt"),
            service_name: "hf_agent_service".to_owned(),
            service_version: "hf.generated.v1".to_owned(),
            handler_name: "infer".to_owned(),
            handler_class: SoraServiceHandlerClassV1::Query,
            request_commitment: Hash::new(b"agent runtime request"),
            result_commitment: Hash::new(b"agent runtime result"),
            certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
            emitted_sequence: 9,
            execution_host: None,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
        }
    }
    fn agent_autonomy_execution_audit_json_fixture() -> AgentAutonomyExecutionAuditRecord {
        AgentAutonomyExecutionAuditRecord {
            sequence: 10,
            succeeded: true,
            result_commitment: Hash::new(b"agent autonomy audit result"),
            service_name: None,
            service_version: None,
            handler_name: None,
            runtime_receipt_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            reason: None,
        }
    }
    fn agent_runtime_execution_summary_json_fixture() -> AgentRuntimeExecutionSummary {
        AgentRuntimeExecutionSummary {
            apartment_name: "ops_agent".to_owned(),
            run_id: "ops_agent:autonomy:9".to_owned(),
            service_name: None,
            service_version: None,
            handler_name: None,
            succeeded: true,
            result_commitment: Hash::new(b"agent autonomy result"),
            journal_artifact_hash: Hash::new(b"agent autonomy journal"),
            checkpoint_artifact_hash: None,
            runtime_receipt: Some(agent_runtime_receipt_json_fixture()),
            workflow_steps: vec![AgentRuntimeWorkflowStepSummary {
                step_index: 0,
                step_id: None,
                request_commitment: Hash::new(b"agent workflow request"),
                result_commitment: Hash::new(b"agent workflow result"),
                runtime_receipt: None,
                content_type: None,
                response_json: None,
                response_text: None,
            }],
            content_type: None,
            response_json: None,
            response_text: None,
            error: None,
        }
    }
    fn json_object_at_mut<'a>(
        value: &'a mut norito::json::Value,
        pointer: &str,
    ) -> &'a mut norito::json::Map {
        let target = if pointer.is_empty() {
            value
        } else {
            value.pointer_mut(pointer).expect("JSON pointer exists")
        };
        target
            .as_object_mut()
            .expect("JSON pointer names an object")
    }
    #[test]
    fn agent_autonomy_mutation_json_graph_requires_exact_v1_fields() {
        let canonical = AgentAutonomyMutationResponse {
            action: AgentApartmentAction::AutonomyRunExecuted,
            apartment_name: "ops_agent".to_owned(),
            sequence: 10,
            status: AgentRuntimeStatus::Running,
            lease_expires_height: 100,
            lease_remaining_blocks: 90,
            manifest_hash: Hash::new(b"agent manifest"),
            artifact_hash: "hash:agent#1".to_owned(),
            provenance_hash: None,
            run_id: Some("ops_agent:autonomy:9".to_owned()),
            run_label: None,
            workflow_input_json: None,
            budget_units: Some(10),
            budget_remaining_units: 90,
            allowlist_count: 1,
            run_count: 1,
            process_generation: 3,
            process_started_sequence: 1,
            last_active_sequence: 10,
            last_checkpoint_sequence: None,
            checkpoint_count: 1,
            persistent_state_total_bytes: 64,
            persistent_state_key_count: 1,
            audit_event_count: 2,
            signed_by: "signer".to_owned(),
            runtime_execution: Some(agent_runtime_execution_summary_json_fixture()),
            runtime_execution_error: None,
            authoritative_runtime_receipt: Some(agent_runtime_receipt_json_fixture()),
            authoritative_runtime_receipt_error: None,
            authoritative_execution_audit: Some(agent_autonomy_execution_audit_json_fixture()),
            authoritative_execution_audit_error: None,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical agent autonomy mutation");
        norito::json::from_value::<AgentAutonomyMutationResponse>(canonical_value.clone())
            .expect("decode canonical agent autonomy mutation");
        for (pointer, required_field) in [
            ("", "provenance_hash"),
            ("", "run_id"),
            ("", "run_label"),
            ("", "workflow_input_json"),
            ("", "budget_units"),
            ("", "last_checkpoint_sequence"),
            ("", "runtime_execution"),
            ("", "runtime_execution_error"),
            ("", "authoritative_runtime_receipt"),
            ("", "authoritative_runtime_receipt_error"),
            ("", "authoritative_execution_audit"),
            ("", "authoritative_execution_audit_error"),
            ("/runtime_execution", "service_name"),
            ("/runtime_execution", "service_version"),
            ("/runtime_execution", "handler_name"),
            ("/runtime_execution", "checkpoint_artifact_hash"),
            ("/runtime_execution", "runtime_receipt"),
            ("/runtime_execution", "workflow_steps"),
            ("/runtime_execution", "content_type"),
            ("/runtime_execution", "response_json"),
            ("/runtime_execution", "response_text"),
            ("/runtime_execution", "error"),
            ("/runtime_execution/runtime_receipt", "execution_host"),
            ("/runtime_execution/runtime_receipt", "mailbox_message_id"),
            (
                "/runtime_execution/runtime_receipt",
                "journal_artifact_hash",
            ),
            (
                "/runtime_execution/runtime_receipt",
                "checkpoint_artifact_hash",
            ),
            ("/runtime_execution/workflow_steps/0", "step_id"),
            ("/runtime_execution/workflow_steps/0", "runtime_receipt"),
            ("/runtime_execution/workflow_steps/0", "content_type"),
            ("/runtime_execution/workflow_steps/0", "response_json"),
            ("/runtime_execution/workflow_steps/0", "response_text"),
            ("/authoritative_execution_audit", "service_name"),
            ("/authoritative_execution_audit", "service_version"),
            ("/authoritative_execution_audit", "handler_name"),
            ("/authoritative_execution_audit", "runtime_receipt_id"),
            ("/authoritative_execution_audit", "journal_artifact_hash"),
            ("/authoritative_execution_audit", "checkpoint_artifact_hash"),
            ("/authoritative_execution_audit", "reason"),
        ] {
            let mut missing = canonical_value.clone();
            assert!(
                json_object_at_mut(&mut missing, pointer)
                    .remove(required_field)
                    .is_some(),
                "fixture must contain {pointer}/{required_field}"
            );
            assert!(
                norito::json::from_value::<AgentAutonomyMutationResponse>(missing).is_err(),
                "agent autonomy graph must require {pointer}/{required_field}"
            );
        }
        for pointer in [
            "",
            "/runtime_execution",
            "/runtime_execution/runtime_receipt",
            "/runtime_execution/workflow_steps/0",
            "/authoritative_execution_audit",
        ] {
            let mut unknown = canonical_value.clone();
            json_object_at_mut(&mut unknown, pointer)
                .insert("legacy_field".to_owned(), norito::json::Value::Null);
            assert!(
                norito::json::from_value::<AgentAutonomyMutationResponse>(unknown).is_err(),
                "agent autonomy graph must reject unknown fields at {pointer}"
            );
        }
    }
    #[test]
    fn agent_autonomy_status_json_graph_requires_exact_v1_fields() {
        let canonical = AgentAutonomyStatusResponse {
            apartment_name: "ops_agent".parse().expect("valid apartment name"),
            sequence: 10,
            status: AgentRuntimeStatus::Running,
            lease_expires_height: 100,
            lease_remaining_blocks: 90,
            manifest_hash: Hash::new(b"agent manifest"),
            revoked_policy_capability_count: 0,
            budget_ceiling_units: 100,
            budget_remaining_units: 90,
            allowlist_count: 1,
            run_count: 1,
            process_generation: 3,
            process_started_sequence: 1,
            last_active_sequence: 10,
            last_checkpoint_sequence: None,
            checkpoint_count: 1,
            persistent_state_total_bytes: 64,
            persistent_state_key_count: 1,
            allowlist: vec![AgentAutonomyAllowlistEntry {
                artifact_hash: "hash:agent#1".to_owned(),
                provenance_hash: None,
                added_sequence: 2,
            }],
            recent_runs: vec![AgentAutonomyRunRecord {
                run_id: "ops_agent:autonomy:9".to_owned(),
                artifact_hash: "hash:agent#1".to_owned(),
                provenance_hash: None,
                budget_units: 10,
                run_label: "fixture".to_owned(),
                workflow_input_json: None,
                approved_sequence: 9,
                authoritative_runtime_receipt: Some(agent_runtime_receipt_json_fixture()),
                authoritative_execution_audit: Some(agent_autonomy_execution_audit_json_fixture()),
            }],
            runtime_recent_runs: vec![agent_runtime_execution_summary_json_fixture()],
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical agent autonomy status");
        norito::json::from_value::<AgentAutonomyStatusResponse>(canonical_value.clone())
            .expect("decode canonical agent autonomy status");
        for (pointer, required_field) in [
            ("", "last_checkpoint_sequence"),
            ("", "allowlist"),
            ("", "recent_runs"),
            ("", "runtime_recent_runs"),
            ("/allowlist/0", "provenance_hash"),
            ("/recent_runs/0", "provenance_hash"),
            ("/recent_runs/0", "workflow_input_json"),
            ("/recent_runs/0", "authoritative_runtime_receipt"),
            ("/recent_runs/0", "authoritative_execution_audit"),
        ] {
            let mut missing = canonical_value.clone();
            assert!(
                json_object_at_mut(&mut missing, pointer)
                    .remove(required_field)
                    .is_some(),
                "fixture must contain {pointer}/{required_field}"
            );
            assert!(
                norito::json::from_value::<AgentAutonomyStatusResponse>(missing).is_err(),
                "agent autonomy status graph must require {pointer}/{required_field}"
            );
        }
        for pointer in ["", "/allowlist/0", "/recent_runs/0"] {
            let mut unknown = canonical_value.clone();
            json_object_at_mut(&mut unknown, pointer)
                .insert("legacy_field".to_owned(), norito::json::Value::Null);
            assert!(
                norito::json::from_value::<AgentAutonomyStatusResponse>(unknown).is_err(),
                "agent autonomy status graph must reject unknown fields at {pointer}"
            );
        }
    }
    #[test]
    fn agent_runtime_summary_reader_binds_v1_path_and_authoritative_run() {
        let directory = tempfile::tempdir().expect("temporary autonomy summary directory");
        let apartment_name = "ops_agent";
        let run_id = "ops_agent:autonomy:9";
        let process_generation = 3;
        let request_commitment = Hash::new(b"authoritative agent autonomy request");
        let summary_path = directory
            .path()
            .join("apartments")
            .join(sanitize_runtime_path_component(apartment_name))
            .join("runs")
            .join(sanitize_runtime_path_component(run_id))
            .join("execution_summary.json");
        fs::create_dir_all(summary_path.parent().expect("summary parent"))
            .expect("create summary parent");
        let mut summary = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: apartment_name.to_owned(),
            run_id: run_id.to_owned(),
            service_name: None,
            service_version: None,
            handler_name: None,
            succeeded: false,
            result_commitment: Hash::new(b"placeholder"),
            checkpoint_artifact_hash: None,
            runtime_receipt: None,
            workflow_steps: Vec::new(),
            content_type: None,
            response_json: None,
            response_text: None,
            error: Some("fixture failure".to_owned()),
        };
        summary.result_commitment = derive_soracloud_apartment_autonomy_result_commitment_v1(
            &summary,
            process_generation,
            request_commitment,
        )
        .expect("derive canonical result commitment");
        fs::write(
            &summary_path,
            norito::json::to_vec(&summary).expect("encode summary"),
        )
        .expect("write summary");
        assert!(
            read_agent_runtime_execution_summary(
                directory.path(),
                apartment_name,
                run_id,
                process_generation,
                request_commitment,
            )
            .expect("read canonical summary")
            .is_some()
        );
        assert!(
            read_agent_runtime_execution_summary(
                directory.path(),
                apartment_name,
                run_id,
                process_generation.saturating_add(1),
                request_commitment,
            )
            .is_err()
        );
        assert!(
            read_agent_runtime_execution_summary(
                directory.path(),
                apartment_name,
                run_id,
                process_generation,
                Hash::new(b"different authoritative request"),
            )
            .is_err()
        );

        let mut wrong_path = summary.clone();
        wrong_path.apartment_name = "other_agent".to_owned();
        wrong_path.result_commitment = derive_soracloud_apartment_autonomy_result_commitment_v1(
            &wrong_path,
            process_generation,
            request_commitment,
        )
        .expect("derive wrong-path commitment");
        fs::write(
            &summary_path,
            norito::json::to_vec(&wrong_path).expect("encode wrong-path summary"),
        )
        .expect("write wrong-path summary");
        assert!(
            read_agent_runtime_execution_summary(
                directory.path(),
                apartment_name,
                run_id,
                process_generation,
                request_commitment,
            )
            .is_err()
        );

        let mut wrong_schema = summary;
        wrong_schema.schema_version =
            SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1.saturating_add(1);
        fs::write(
            &summary_path,
            norito::json::to_vec(&wrong_schema).expect("encode wrong-schema summary"),
        )
        .expect("write wrong-schema summary");
        assert!(
            read_agent_runtime_execution_summary(
                directory.path(),
                apartment_name,
                run_id,
                process_generation,
                request_commitment,
            )
            .is_err()
        );
    }
    fn checked_test_signature(private_key: &iroha_crypto::PrivateKey, payload: &[u8]) -> Signature {
        Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
    }
    fn checked_test_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test fixture key derivation should succeed")
    }
    fn test_runtime() -> std::io::Result<tokio::runtime::Runtime> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    }
    fn required_test_runtime(message: &str) -> tokio::runtime::Runtime {
        test_runtime().expect(message)
    }
    #[test]
    fn fhe_policy_lifecycle_actions_keep_distinct_control_plane_identity() {
        for (source, expected, label) in [
            (
                SoraServiceLifecycleActionV1::FhePolicyRegister,
                SoracloudAction::FhePolicyRegister,
                "fhe_policy_register",
            ),
            (
                SoraServiceLifecycleActionV1::FhePolicyRotate,
                SoracloudAction::FhePolicyRotate,
                "fhe_policy_rotate",
            ),
            (
                SoraServiceLifecycleActionV1::FhePolicyRevoke,
                SoracloudAction::FhePolicyRevoke,
                "fhe_policy_revoke",
            ),
        ] {
            assert_eq!(audit_action_to_control_plane_action(source), expected);
            assert_eq!(soracloud_action_label(expected), label);
        }
    }
    fn checked_test_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} Soracloud fixture key generation should succeed: {err}")
        })
    }
    const SMALL_ORDER_ED25519_SIGNATURE_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_ED25519_SIGNATURE_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    fn signature_with_malformed_ed25519_r(
        signature: &Signature,
        replacement_r: &[u8; 32],
    ) -> Signature {
        let mut payload = signature.payload().to_vec();
        payload[..replacement_r.len()].copy_from_slice(replacement_r);
        Signature::from_bytes(&payload)
    }
    #[test]
    fn checked_test_keypair_uses_fallible_seed_derivation() {
        assert_eq!(checked_test_keypair(0x50).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn soracloud_provenance_helper_rejects_malformed_ed25519_signature_r() {
        let keypair = checked_test_keypair(0x51);
        let payload = b"torii-soracloud-provenance";
        let signature = checked_test_signature(keypair.private_key(), payload);
        verify_signature_for_signer(&signature, keypair.public_key(), payload)
            .expect("valid Soracloud provenance signature should verify");
        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_SIGNATURE_R),
            ("noncanonical", NONCANONICAL_ED25519_SIGNATURE_R),
        ] {
            let malformed_signature =
                signature_with_malformed_ed25519_r(&signature, &replacement_r);
            assert_eq!(
                verify_signature_for_signer(&malformed_signature, keypair.public_key(), payload)
                    .expect_err("malformed Soracloud provenance signature R must fail admission"),
                iroha_crypto::Error::BadSignature,
                "{label} Soracloud provenance signature R was not rejected"
            );
        }
    }
    #[test]
    fn soracloud_provenance_helper_rejects_malformed_mldsa_signature_lengths() {
        let keypair = checked_test_keypair_with_algorithm(Algorithm::MlDsa);
        let payload = b"torii-soracloud-provenance-mldsa";
        let signature = checked_test_signature(keypair.private_key(), payload);
        verify_signature_for_signer(&signature, keypair.public_key(), payload)
            .expect("valid Soracloud ML-DSA provenance signature should verify");
        let valid_signature = signature.payload().to_vec();
        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x63);
                payload
            }),
        ] {
            let malformed_signature = Signature::from_bytes(&replacement_signature);
            assert_eq!(
                verify_signature_for_signer(&malformed_signature, keypair.public_key(), payload)
                    .expect_err("malformed Soracloud ML-DSA signature length must fail admission"),
                iroha_crypto::Error::BadSignature,
                "{label} Soracloud ML-DSA signature length was not rejected"
            );
        }
    }
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
    fn fixture_hosted_http_inrou_bundle(version: &str) -> SoraDeploymentBundleV1 {
        let mut bundle = fixture_bundle(version);
        bundle.container.runtime = iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou;
        bundle.container.entrypoint = "/app/main".to_owned();
        bundle.container.inrou = Some(iroha_data_model::soracloud::SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: iroha_data_model::soracloud::SoraInrouGuestOsV1::DebianSlim,
            guest_images: BTreeMap::from([
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: None,
            ssh_authorized_keys: vec!["ssh-ed25519 test-key torii-tests".to_owned()],
        });
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        bundle.service.artifacts.clear();
        bundle.service.lease_volumes = vec![
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "root_disk".parse().expect("volume name"),
                kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
                storage_class: StorageClass::Warm,
                mount_path: "/".to_owned(),
                max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024)
                    .expect("nonzero volume size"),
            },
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "service_state".parse().expect("volume name"),
                kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: StorageClass::Warm,
                mount_path: "/var/lib/soracloud/service".to_owned(),
                max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero volume size"),
            },
        ];
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        bundle
            .validate_for_admission()
            .expect("hosted HTTP Inrou fixture must satisfy production validation");
        bundle
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
    fn fixture_service_deployment(bundle: &SoraDeploymentBundleV1) -> SoraServiceDeploymentStateV1 {
        SoraServiceDeploymentStateV1 {
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
            fhe_policy_records: BTreeMap::new(),
            service_lease: None,
            lease_volume_states: Vec::new(),
        }
    }
    fn fixture_service_lease_volume_states(
        bundle: &SoraDeploymentBundleV1,
        lease: Option<&iroha_data_model::soracloud::SoraServiceLeaseStateV1>,
    ) -> Vec<iroha_data_model::soracloud::SoraServiceLeaseVolumeStateV1> {
        let Some(lease) = lease else {
            return Vec::new();
        };
        bundle
            .service
            .lease_volumes
            .iter()
            .map(
                |binding| iroha_data_model::soracloud::SoraServiceLeaseVolumeStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
                    volume_name: binding.volume_name.clone(),
                    kind: binding.kind,
                    storage_class: binding.storage_class,
                    mount_path: binding.mount_path.clone(),
                    max_total_bytes: binding.max_total_bytes.get(),
                    lease_started_height: lease.lease_started_height,
                    lease_expires_height: lease.lease_expires_height,
                    authoritative_generation: 1,
                },
            )
            .collect()
    }
    fn fixture_service_deploy_audit_event(
        bundle: &SoraDeploymentBundleV1,
    ) -> SoraServiceAuditEventV1 {
        SoraServiceAuditEventV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
            sequence: 1,
            block_height: 1,
            block_timestamp_ms: 1,
            action: SoraServiceLifecycleActionV1::Deploy,
            service_name: bundle.service.service_name.clone(),
            from_version: None,
            to_version: bundle.service.service_version.clone(),
            service_manifest_hash: bundle.service_manifest_hash(),
            container_manifest_hash: bundle.container_manifest_hash(),
            process_generation: 1,
            config_generation: 0,
            secret_generation: 0,
            config_snapshot_hash:
                iroha_data_model::soracloud::derive_soracloud_service_config_snapshot_hash_v1(
                    &BTreeMap::new(),
                ),
            secret_snapshot_hash:
                iroha_data_model::soracloud::derive_soracloud_service_secret_snapshot_hash_v1(
                    &BTreeMap::new(),
                ),
            governance_tx_hash: None,
            binding_name: None,
            state_key: None,
            config_mutations: Vec::new(),
            secret_mutations: Vec::new(),
            rollout_state: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            lease_usage: None,
            service_lease_commitment: None,
            lease_reporting_epoch_rollover: None,
            signer: checked_test_keypair(0x5A).public_key().clone(),
        }
    }
    fn insert_revision(
        world: &mut iroha_core::state::World,
        bundle: &SoraDeploymentBundleV1,
        service_name: String,
    ) {
        world.soracloud_service_revisions_mut_for_testing().insert(
            (service_name, bundle.service.service_version.clone()),
            bundle.clone(),
        );
    }
    fn install_fixture_service(
        world: &mut iroha_core::state::World,
        bundle: &SoraDeploymentBundleV1,
        service_name: &Name,
    ) {
        insert_revision(world, bundle, service_name.as_ref().to_owned());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name.clone(), fixture_service_deployment(bundle));
    }
    fn fixture_fhe_job_spec() -> FheJobSpecV1 {
        load_json(&workspace_fixture(
            "fixtures/soracloud/fhe_job_spec_v1.json",
        ))
    }
    fn fixture_fhe_policy_reference() -> SoracloudFhePolicyReferenceV1 {
        SoracloudFhePolicyReferenceV1 {
            schema_version: iroha_data_model::soracloud::SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1,
            policy_name: "health_policy".parse().expect("policy name"),
            version: NonZeroU32::new(1).expect("non-zero policy version"),
            material_digest: Hash::new(b"governed-fhe-material-v1"),
        }
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
    fn fixture_private_decryption_audit_event(
        bundle: &SoraDeploymentBundleV1,
        record: &SoraDecryptionRequestRecordV1,
    ) -> SoraServiceAuditEventV1 {
        let mut event = fixture_service_deploy_audit_event(bundle);
        event.sequence = record.sequence;
        event.block_height = 1;
        event.block_timestamp_ms = 1;
        event.action = SoraServiceLifecycleActionV1::DecryptionRequest;
        event.from_version = None;
        event.to_version = record.service_version.clone();
        event.governance_tx_hash = Some(record.request.governance_tx_hash);
        event.binding_name = Some(record.request.binding_name.clone());
        event.state_key = Some(record.request.state_key.clone());
        event.policy_name = Some(record.request.policy_name.clone());
        event.policy_snapshot_hash = Some(record.policy_snapshot_hash());
        event.jurisdiction_tag = Some(record.request.jurisdiction_tag.clone());
        event.consent_evidence_hash = record.request.consent_evidence_hash;
        event.break_glass = Some(record.request.break_glass);
        event.break_glass_reason = record.request.break_glass_reason.clone();
        event.signer = record.signer.clone();
        event
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
    fn signed_rollback_request(
        service_name: &str,
        target_version: &str,
        key_pair: &KeyPair,
    ) -> SignedRollbackRequest {
        let payload = RollbackPayload {
            service_name: service_name.to_string(),
            target_version: target_version.to_owned(),
        };
        let encoded = encode_rollback_signature_payload(&payload).expect("encode rollback payload");
        let signature = checked_test_signature(key_pair.private_key(), &encoded);
        SignedRollbackRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }
    fn verified_request_headers(account: &AccountId, signer: &PublicKey) -> axum::http::HeaderMap {
        verified_request_headers_with_signers(account, signer, std::slice::from_ref(signer))
    }
    fn verified_request_headers_with_signers(
        account: &AccountId,
        signer: &PublicKey,
        verified_signers: &[PublicKey],
    ) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            VERIFIED_ACCOUNT_HEADER,
            axum::http::HeaderValue::from_bytes(account.to_string().as_bytes())
                .expect("valid utf-8 account header"),
        );
        headers.insert(
            VERIFIED_SIGNER_HEADER,
            signer.to_string().parse().expect("valid signer header"),
        );
        headers.insert(
            VERIFIED_SIGNERS_HEADER,
            BASE64_STANDARD
                .encode(norito::to_bytes(&verified_signers.to_vec()).expect("encode signers"))
                .parse()
                .expect("valid signer-set header"),
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
        let payload = encode_bundle_signature_payload(
            bundle,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &SoraServiceMutationPreconditionV1::ServiceAbsent,
        )
        .expect("bundle payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: checked_test_signature(key_pair.private_key(), &payload),
        }
    }
    fn signed_generated_apartment_provenance(
        manifest: &AgentApartmentManifestV1,
        key_pair: &KeyPair,
    ) -> ManifestProvenance {
        let payload = encode_agent_deploy_provenance_payload(
            manifest.clone(),
            HF_GENERATED_AGENT_LEASE_BLOCKS,
            HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS,
        )
        .expect("agent deploy payload");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: checked_test_signature(key_pair.private_key(), &payload),
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
            deployed_sequence: 1,
            lease_started_height: 1,
            lease_expires_height: 100,
            last_renewed_height: 1,
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
            block_height: run.approved_sequence,
            block_timestamp_ms: run.approved_sequence,
            action: SoraAgentApartmentActionV1::AutonomyRunApproved,
            apartment_name: apartment_name.parse().expect("valid apartment name"),
            status: SoraAgentRuntimeStatusV1::Running,
            lease_expires_height: 100,
            manifest_hash,
            restart_count: 0,
            signer: signer.public_key().clone(),
            request_id: Some(run.run_id.clone()),
            asset_definition: None,
            amount: None,
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
    #[test]
    fn agent_execution_audit_projection_fails_closed_without_authoritative_outcome_fields() {
        let signer = checked_test_keypair(0x5B);
        let run = fixture_agent_run_record("ops_agent", "ops_agent:autonomy:1", 1, 1);
        let mut event = fixture_autonomy_approval_event(
            "ops_agent",
            Hash::new(b"agent manifest"),
            &signer,
            &run,
        );
        event.action = SoraAgentApartmentActionV1::AutonomyRunExecuted;
        event.result_commitment = Some(Hash::new(b"execution result"));
        let error = authoritative_agent_execution_audit_record(&event)
            .expect_err("missing authoritative succeeded projection must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Internal);
        assert!(error.message.contains("missing `succeeded`"));

        event.succeeded = Some(false);
        event.result_commitment = None;
        let error = authoritative_agent_execution_audit_record(&event)
            .expect_err("missing authoritative result commitment must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Internal);
        assert!(error.message.contains("missing `result_commitment`"));
    }
    fn attach_test_runtime(app: &mut SharedAppState, state_dir: PathBuf) {
        Arc::get_mut(app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestHfRuntimeHandle {
            snapshot: SoracloudRuntimeSnapshot::default(),
            state_dir,
        }));
    }
    fn seed_domain_name_lease(
        world: &mut iroha_core::state::World,
        owner: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
    ) {
        let selector = iroha_core::sns::selector_for_domain(domain_id).expect("domain selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&selector),
            Encode::encode(&record),
        );
    }
    #[test]
    fn signed_mutation_request_rejects_inline_signing_material() {
        let key_pair = checked_test_keypair(0x60);
        let request = signed_rollback_request("inline-secret-test", "1.0.0", &key_pair);
        let mut payload = norito::json::Map::new();
        payload.insert(
            "service_name".to_owned(),
            norito::json::Value::from(request.payload.service_name),
        );
        payload.insert(
            "target_version".to_owned(),
            norito::json::Value::from(request.payload.target_version),
        );
        let mut fields = norito::json::Map::new();
        fields.insert("payload".to_owned(), norito::json::Value::Object(payload));
        fields.insert(
            "provenance".to_owned(),
            norito::json::to_value(&request.provenance)
                .expect("serialize valid request provenance"),
        );
        fields.insert(
            "authority".to_owned(),
            norito::json::Value::from("inline-authority-is-forbidden"),
        );
        let error =
            norito::json::from_value::<SignedRollbackRequest>(norito::json::Value::Object(fields))
                .expect_err("inline signing material must be rejected during decoding");
        assert!(
            error.to_string().contains("unknown field `authority`"),
            "unexpected inline-authority rejection: {error}"
        );
    }
    #[test]
    fn rollback_payload_requires_explicit_target_version() {
        for payload in [
            r#"{"service_name":"web_portal"}"#,
            r#"{"service_name":"web_portal","target_version":null}"#,
        ] {
            norito::json::from_str::<RollbackPayload>(payload)
                .expect_err("missing or null target_version must be rejected");
        }
    }
    #[test]
    fn rollback_payload_rejects_retired_history_fields() {
        let error = norito::json::from_str::<RollbackPayload>(
            r#"{"service_name":"web_portal","target_version":"1.0.0","previous_service_version":"0.9.0"}"#,
        )
        .expect_err("retired rollback history inference field must be rejected");
        assert!(
            error
                .to_string()
                .contains("unknown field `previous_service_version`"),
            "unexpected retired rollback field rejection: {error}"
        );
    }
    #[test]
    fn signed_bundle_request_requires_explicit_material_maps_and_precondition() {
        let request = SignedBundleRequest {
            bundle: fixture_bundle("1.0.0"),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            precondition: SoraServiceMutationPreconditionV1::ServiceAbsent,
            provenance: signed_rollback_request(
                "web_portal",
                "1.0.0",
                &iroha_test_samples::ALICE_KEYPAIR,
            )
            .provenance,
        };
        let mut fields = norito::json::Map::new();
        fields.insert(
            "bundle".to_owned(),
            norito::json::to_value(&request.bundle).expect("serialize bundle"),
        );
        fields.insert(
            "initial_service_configs".to_owned(),
            norito::json::to_value(&request.initial_service_configs)
                .expect("serialize initial configs"),
        );
        fields.insert(
            "initial_service_secrets".to_owned(),
            norito::json::to_value(&request.initial_service_secrets)
                .expect("serialize initial secrets"),
        );
        fields.insert(
            "precondition".to_owned(),
            norito::json::to_value(&request.precondition).expect("serialize precondition"),
        );
        fields.insert(
            "provenance".to_owned(),
            norito::json::to_value(&request.provenance).expect("serialize provenance"),
        );
        let canonical = norito::json::Value::Object(fields);
        norito::json::from_value::<SignedBundleRequest>(canonical.clone())
            .expect("canonical signed bundle request must decode");

        for field in [
            "initial_service_configs",
            "initial_service_secrets",
            "precondition",
        ] {
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("signed bundle request JSON object")
                    .remove(field)
                    .is_some()
            );
            norito::json::from_value::<SignedBundleRequest>(missing)
                .expect_err("omitted signed bundle field must be rejected");

            let mut null = canonical.clone();
            null.as_object_mut()
                .expect("signed bundle request JSON object")
                .insert(field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<SignedBundleRequest>(null)
                .expect_err("null signed bundle field must be rejected");
        }
    }
    #[test]
    fn signed_app_infra_request_requires_explicit_service_vectors_and_closed_fields() {
        let manifest = SoraAppInfraManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_APP_INFRA_MANIFEST_VERSION_V1,
            app_name: "web_portal".parse().expect("valid app name"),
            app_version: "1.0.0".to_owned(),
            public_url: "https://web-portal.example".to_owned(),
            static_site: None,
            services: Vec::new(),
        };
        let provenance =
            signed_rollback_request("web_portal", "1.0.0", &iroha_test_samples::ALICE_KEYPAIR)
                .provenance;
        let mut fields = norito::json::Map::new();
        fields.insert("deploy_services".to_owned(), norito::json!([]));
        fields.insert("upgrade_services".to_owned(), norito::json!([]));
        fields.insert(
            "manifest".to_owned(),
            norito::json::to_value(&manifest).expect("serialize app manifest"),
        );
        fields.insert(
            "precondition".to_owned(),
            norito::json::to_value(&SoraAppInfraMutationPreconditionV1::AppAbsent)
                .expect("serialize app precondition"),
        );
        fields.insert(
            "provenance".to_owned(),
            norito::json::to_value(&provenance).expect("serialize app provenance"),
        );
        let canonical = norito::json::Value::Object(fields);
        norito::json::from_value::<SignedAppInfraRequest>(canonical.clone())
            .expect("canonical signed app request must decode");

        for field in ["deploy_services", "upgrade_services", "precondition"] {
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("signed app request JSON object")
                    .remove(field)
                    .is_some()
            );
            norito::json::from_value::<SignedAppInfraRequest>(missing)
                .expect_err("signed app request must reject an omitted required field");

            let mut null = canonical.clone();
            null.as_object_mut()
                .expect("signed app request JSON object")
                .insert(field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<SignedAppInfraRequest>(null)
                .expect_err("signed app request must reject a null required field");
        }

        let mut unknown = canonical;
        unknown
            .as_object_mut()
            .expect("signed app request JSON object")
            .insert("retired_v0".to_owned(), norito::json::Value::from(true));
        norito::json::from_value::<SignedAppInfraRequest>(unknown)
            .expect_err("signed app request must reject unknown fields");
    }
    #[test]
    fn service_control_payloads_are_closed_and_state_nulls_are_explicit() {
        macro_rules! assert_closed_value {
            ($value:expr, $ty:ty, $label:literal) => {{
                let value = $value;
                norito::json::from_value::<$ty>(value.clone()).expect(concat!(
                    "canonical ",
                    $label,
                    " must decode"
                ));
                let mut unknown = value;
                unknown
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert("retired_v0".to_owned(), norito::json::Value::from(true));
                norito::json::from_value::<$ty>(unknown)
                    .expect_err(concat!($label, " must reject unknown fields"));
            }};
        }

        let operation =
            norito::json::to_value(&StateMutationOperation::Delete).expect("serialize operation");
        assert_closed_value!(
            operation.clone(),
            StateMutationOperation,
            "state mutation operation"
        );
        let encryption = norito::json::to_value(&SoraStateEncryptionV1::ClientCiphertext)
            .expect("serialize state encryption");
        let governance_tx_hash = Hash::new(b"governance");
        let mut state_fields = norito::json::Map::new();
        state_fields.insert(
            "service_name".to_owned(),
            norito::json::Value::from("web_portal"),
        );
        state_fields.insert(
            "binding_name".to_owned(),
            norito::json::Value::from("private_state"),
        );
        state_fields.insert("key".to_owned(), norito::json::Value::from("/state/1"));
        state_fields.insert("operation".to_owned(), operation);
        state_fields.insert("value_size_bytes".to_owned(), norito::json::Value::Null);
        state_fields.insert("value_payload_hex".to_owned(), norito::json::Value::Null);
        state_fields.insert("encryption".to_owned(), encryption);
        state_fields.insert(
            "governance_tx_hash".to_owned(),
            norito::json::to_value(&governance_tx_hash).expect("serialize governance hash"),
        );
        state_fields.insert(
            "fhe_input_admission_proof".to_owned(),
            norito::json::Value::Null,
        );
        let state = norito::json::Value::Object(state_fields);
        assert_closed_value!(
            state.clone(),
            StateMutationRequest,
            "state mutation payload"
        );
        for field in [
            "value_size_bytes",
            "value_payload_hex",
            "fhe_input_admission_proof",
        ] {
            let mut missing = state.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("state mutation JSON object")
                    .remove(field)
                    .is_some()
            );
            norito::json::from_value::<StateMutationRequest>(missing)
                .expect_err("state mutation payload must reject an omitted nullable key");

            let mut explicit_null = state.clone();
            explicit_null
                .as_object_mut()
                .expect("state mutation JSON object")
                .insert(field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<StateMutationRequest>(explicit_null)
                .expect("state mutation payload must accept an explicit null key");
        }

        assert_closed_value!(
            norito::json!({
                "service_name": "web_portal",
                "config_name": "runtime",
                "value_json": {"workers": 2_u64}
            }),
            ServiceConfigSetRequest,
            "service config set payload"
        );
        assert_closed_value!(
            norito::json!({
                "service_name": "web_portal",
                "config_name": "runtime"
            }),
            ServiceConfigDeleteRequest,
            "service config delete payload"
        );
        let secret = SecretEnvelopeV1 {
            schema_version: iroha_data_model::soracloud::SECRET_ENVELOPE_VERSION_V1,
            encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
            key_id: "kms/test".to_owned(),
            key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
            nonce: vec![1],
            ciphertext: vec![2],
            commitment: Hash::new(b"secret"),
            aad_digest: None,
        };
        let mut secret_set = norito::json::Map::new();
        secret_set.insert(
            "service_name".to_owned(),
            norito::json::Value::from("web_portal"),
        );
        secret_set.insert(
            "secret_name".to_owned(),
            norito::json::Value::from("api_token"),
        );
        secret_set.insert(
            "secret".to_owned(),
            norito::json::to_value(&secret).expect("serialize secret envelope"),
        );
        assert_closed_value!(
            norito::json::Value::Object(secret_set),
            ServiceSecretSetRequest,
            "service secret set payload"
        );
        assert_closed_value!(
            norito::json!({
                "service_name": "web_portal",
                "secret_name": "api_token"
            }),
            ServiceSecretDeleteRequest,
            "service secret delete payload"
        );
    }
    #[test]
    fn signed_soracloud_mutation_graph_rejects_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($($ty:ty),+ $(,)?) => {
                $(
                    let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                        .expect_err(concat!(stringify!($ty), " must reject unknown fields"));
                    assert!(
                        matches!(
                            error,
                            norito::json::Error::UnknownField { ref field }
                                if field == "retired_v0"
                        ),
                        "{} reported the wrong error: {error}",
                        stringify!($ty)
                    );
                )+
            };
        }
        assert_unknown_rejected!(
            SignedBundleRequest,
            SignedAppInfraRequest,
            RollbackPayload,
            SignedRollbackRequest,
            StateMutationOperation,
            StateMutationRequest,
            SignedStateMutationRequest,
            ServiceConfigSetRequest,
            SignedServiceConfigSetRequest,
            ServiceConfigDeleteRequest,
            SignedServiceConfigDeleteRequest,
            ServiceSecretSetRequest,
            SignedServiceSecretSetRequest,
            ServiceSecretDeleteRequest,
            SignedServiceSecretDeleteRequest,
            RolloutAdvancePayload,
            SignedRolloutAdvanceRequest,
            AgentDeployPayload,
            SignedAgentDeployRequest,
            AgentLeaseRenewPayload,
            SignedAgentLeaseRenewRequest,
            HfDeployPayload,
            SignedHfDeployRequest,
            HfLeaseLeavePayload,
            SignedHfLeaseLeaveRequest,
            HfLeaseRenewPayload,
            SignedHfLeaseRenewRequest,
            ModelHostAdvertisePayload,
            SignedModelHostAdvertiseRequest,
            ModelHostHeartbeatPayload,
            SignedModelHostHeartbeatRequest,
            ModelHostWithdrawPayload,
            SignedModelHostWithdrawRequest,
            AgentRestartPayload,
            SignedAgentRestartRequest,
            AgentPolicyRevokePayload,
            SignedAgentPolicyRevokeRequest,
            AgentWalletSpendPayload,
            SignedAgentWalletSpendRequest,
            AgentWalletApprovePayload,
            SignedAgentWalletApproveRequest,
            AgentMessageSendPayload,
            SignedAgentMessageSendRequest,
            AgentMessageAckPayload,
            SignedAgentMessageAckRequest,
            AgentArtifactAllowPayload,
            SignedAgentArtifactAllowRequest,
            AgentAutonomyRunPayload,
            SignedAgentAutonomyRunRequest,
            AgentAutonomyFinalizeRequest,
            FheJobRunPayload,
            SignedFheJobRunRequest,
            TrainingJobStartPayload,
            SignedTrainingJobStartRequest,
            TrainingJobCheckpointPayload,
            SignedTrainingJobCheckpointRequest,
            TrainingJobRetryPayload,
            SignedTrainingJobRetryRequest,
            ModelWeightRegisterPayload,
            SignedModelWeightRegisterRequest,
            ModelWeightPromotePayload,
            SignedModelWeightPromoteRequest,
            ModelWeightRollbackPayload,
            SignedModelWeightRollbackRequest,
            ModelArtifactRegisterPayload,
            SignedModelArtifactRegisterRequest,
            UploadedModelRegisterPayload,
            SignedUploadedModelRegisterRequest,
            DecryptionRequestPayload,
            SignedDecryptionRequest,
            SignedCiphertextQueryRequest,
            PrivateUploadedModelExecuteRequest,
        );
    }
    fn sample_private_model_artifact_ref(role: &str, seed: u8) -> SoraPrivateModelArtifactRefV1 {
        SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([seed; 32]),
            sorafs_root_cid: ManifestRootCid::from_blake3_digest([seed; 32])
                .expect("fixture root CID"),
            artifact_hash: Hash::new([seed; 16]),
            ciphertext_bytes: 128,
            artifact_role: role.to_owned(),
        }
    }
    fn sample_private_uploaded_model_receipt_for_pagination(
        network_id: iroha_data_model::id::NetworkId,
        emitted_sequence: u64,
        receipt_seed: u8,
        service_name: &str,
        model_id: &str,
        weight_version: &str,
    ) -> (Hash, SoraPrivateUploadedModelExecutionReceiptV1) {
        let placeholder = Hash::new([receipt_seed; 32]);
        let mut receipt = SoraPrivateUploadedModelExecutionReceiptV1 {
            schema_version: SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
            network_id,
            receipt_id: placeholder,
            service_name: service_name
                .parse()
                .expect("canonical receipt service name"),
            service_version: "1.0.0".to_owned(),
            model_id: model_id.to_owned(),
            weight_version: weight_version.to_owned(),
            runtime_version:
                iroha_data_model::soracloud::SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_owned(),
            model_manifest_digest: ManifestDigest::new([receipt_seed.wrapping_add(1); 32]),
            model_bundle_root: Hash::new([receipt_seed.wrapping_add(2); 32]),
            policy_id: "private_release".to_owned(),
            decryption_request_id: format!("release-{receipt_seed}"),
            output_recipient: sample_uploaded_model_register_payload()
                .bundle
                .upload_recipient,
            attesting_validator:
                iroha_data_model::soracloud::SoraRuntimeDeterministicValidatorHostV1 {
                    lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                    validator_account_id: ALICE_ID.clone(),
                    peer_id: iroha_data_model::peer::PeerId::from(
                        ALICE_ID.expect_single_signatory().clone(),
                    )
                    .to_string(),
                },
            input_artifact: sample_private_model_artifact_ref(
                "input",
                receipt_seed.wrapping_add(3),
            ),
            output_artifact: sample_private_model_artifact_ref(
                "output",
                receipt_seed.wrapping_add(4),
            ),
            output_replication_order_id: derive_sorafs_auto_replication_order_id_v1(
                &ManifestDigest::new([receipt_seed.wrapping_add(4); 32]),
            ),
            input_commitment: Hash::new([receipt_seed.wrapping_add(5); 32]),
            output_commitment: Hash::new([receipt_seed.wrapping_add(6); 32]),
            request_commitment: placeholder,
            result_commitment: placeholder,
            emitted_sequence,
            emitted_block_height: emitted_sequence,
        };
        receipt.request_commitment = derive_soracloud_private_model_request_commitment_v1(&receipt);
        receipt.result_commitment = derive_soracloud_private_model_result_commitment_v1(&receipt);
        receipt.receipt_id =
            derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&receipt);
        receipt
            .validate()
            .expect("canonical private receipt pagination fixture");
        let receipt_id = receipt.receipt_id;
        (receipt_id, receipt)
    }
    #[test]
    fn signed_soracloud_mutation_graph_requires_explicit_optional_keys() {
        macro_rules! assert_required_nullable {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect(concat!("serialize canonical ", $label));
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("canonical ", $label, " must decode"));
                for field in [$($field),+] {
                    assert!(
                        canonical.get(field).is_some_and(norito::json::Value::is_null),
                        "{} must serialize `{field}` as explicit null",
                        $label
                    );
                    let mut missing = canonical.clone();
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove(field);
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject an omitted nullable key"));

                    let mut explicit_null = canonical.clone();
                    explicit_null
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .insert(field.to_owned(), norito::json::Value::Null);
                    norito::json::from_value::<$ty>(explicit_null)
                        .expect(concat!($label, " must accept explicit null"));
                }
            }};
        }

        let agent_deploy = AgentDeployPayload {
            manifest: fixture_agent_manifest(),
            lease_blocks: 120,
            autonomy_budget_units: 500,
        };
        let mut missing_budget =
            norito::json::to_value(&agent_deploy).expect("serialize agent deploy payload");
        missing_budget
            .as_object_mut()
            .expect("agent deploy payload object")
            .remove("autonomy_budget_units");
        norito::json::from_value::<AgentDeployPayload>(missing_budget)
            .expect_err("agent deployment must not infer an autonomy budget");

        let hf_deploy = HfDeployPayload {
            repo_id: "openai/gpt-oss".to_owned(),
            revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            model_name: "gpt_oss_20b".to_owned(),
            service_name: "vision_portal".to_owned(),
            apartment_name: None,
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            lease_asset_definition_id: hf_shared_lease_asset_definition(),
            base_fee: "0.0000000001".parse().expect("canonical quantity"),
        };
        assert_required_nullable!(
            hf_deploy.clone(),
            HfDeployPayload,
            ["apartment_name"],
            "HF deploy payload"
        );
        assert_required_nullable!(
            HfLeaseLeavePayload {
                repo_id: hf_deploy.repo_id.clone(),
                revision: hf_deploy.revision.clone(),
                storage_class: StorageClass::Warm,
                lease_term_ms: hf_deploy.lease_term_ms,
                service_name: None,
                apartment_name: None,
            },
            HfLeaseLeavePayload,
            ["service_name", "apartment_name"],
            "HF lease-leave payload"
        );
        let hf_renew = HfLeaseRenewPayload {
            repo_id: hf_deploy.repo_id.clone(),
            revision: hf_deploy.revision.clone(),
            model_name: hf_deploy.model_name.clone(),
            service_name: hf_deploy.service_name.clone(),
            apartment_name: None,
            storage_class: hf_deploy.storage_class,
            lease_term_ms: hf_deploy.lease_term_ms,
            lease_asset_definition_id: hf_deploy.lease_asset_definition_id.clone(),
            base_fee: hf_deploy.base_fee.clone(),
        };
        assert_required_nullable!(
            hf_renew.clone(),
            HfLeaseRenewPayload,
            ["apartment_name"],
            "HF lease-renew payload"
        );

        let key_pair = checked_test_keypair(0xE1);
        let provenance = signed_generated_service_provenance(&fixture_bundle("1.0.0"), &key_pair);
        assert_required_nullable!(
            SignedHfDeployRequest {
                payload: hf_deploy,
                provenance: provenance.clone(),
                generated_service_provenance: None,
                generated_apartment_provenance: None,
            },
            SignedHfDeployRequest,
            [
                "generated_service_provenance",
                "generated_apartment_provenance",
            ],
            "signed HF deploy request"
        );
        assert_required_nullable!(
            SignedHfLeaseRenewRequest {
                payload: hf_renew,
                provenance,
                generated_service_provenance: None,
                generated_apartment_provenance: None,
            },
            SignedHfLeaseRenewRequest,
            [
                "generated_service_provenance",
                "generated_apartment_provenance",
            ],
            "signed HF lease-renew request"
        );
        assert_required_nullable!(
            AgentPolicyRevokePayload {
                apartment_name: "ops_agent".to_owned(),
                capability: "agent.autonomy.run".to_owned(),
                reason: None,
            },
            AgentPolicyRevokePayload,
            ["reason"],
            "agent policy-revoke payload"
        );
        assert_required_nullable!(
            AgentArtifactAllowPayload {
                apartment_name: "ops_agent".to_owned(),
                artifact_hash: "hash:ABCD0123#01".to_owned(),
                provenance_hash: None,
            },
            AgentArtifactAllowPayload,
            ["provenance_hash"],
            "agent artifact-allow payload"
        );
        assert_required_nullable!(
            AgentAutonomyRunPayload {
                apartment_name: "ops_agent".to_owned(),
                artifact_hash: "hash:ABCD0123#01".to_owned(),
                provenance_hash: None,
                budget_units: 120,
                run_label: "nightly".to_owned(),
                workflow_input_json: None,
            },
            AgentAutonomyRunPayload,
            ["provenance_hash", "workflow_input_json"],
            "agent autonomy-run payload"
        );
        assert_required_nullable!(
            ModelWeightRegisterPayload {
                service_name: "web_portal".to_owned(),
                model_name: "model-1".to_owned(),
                weight_version: "1.0.0".to_owned(),
                training_job_id: "job-1".to_owned(),
                parent_version: None,
                weight_artifact_hash: Hash::new(b"weight-artifact"),
                dataset_ref: "dataset://synthetic/v2".to_owned(),
                training_config_hash: Hash::new(b"train-config"),
                reproducibility_hash: Hash::new(b"repro"),
                provenance_attestation_hash: Hash::new(b"attestation"),
            },
            ModelWeightRegisterPayload,
            ["parent_version"],
            "model-weight register payload"
        );
    }
    #[test]
    fn private_uploaded_model_execute_request_rejects_plaintext_and_claimed_execution_fields() {
        let request = PrivateUploadedModelExecuteRequest {
            service_name: "private_model_host".to_owned(),
            service_version: "1.0.0".to_owned(),
            weight_version: "v1".to_owned(),
            model_id: "upload-1".to_owned(),
            bundle_root: Hash::new(b"uploaded-model-bundle"),
            decryption_request_id: "decrypt-upload-input".to_owned(),
            input_artifact: sample_private_model_artifact_ref("input", 0xD1),
            output_recipient: sample_uploaded_model_register_payload()
                .bundle
                .upload_recipient,
        };
        let canonical =
            norito::json::to_value(&request).expect("serialize private execute request");
        for retired_field in [
            "model",
            "plaintext_input_i32",
            "output_artifact",
            "execution_host",
            "policy_id",
        ] {
            let mut adversarial = canonical.clone();
            adversarial
                .as_object_mut()
                .expect("private execute request object")
                .insert(retired_field.to_owned(), norito::json::Value::from(true));
            norito::json::from_value::<PrivateUploadedModelExecuteRequest>(adversarial)
                .expect_err("caller-controlled private execution fields must be rejected");
        }
        for required in ["model_id", "bundle_root", "decryption_request_id"] {
            let mut missing_release = canonical.clone();
            missing_release
                .as_object_mut()
                .expect("private execute request object")
                .remove(required);
            norito::json::from_value::<PrivateUploadedModelExecuteRequest>(missing_release)
                .expect_err("private execution must require its immutable release coordinates");
        }
        let mut retired_alias = canonical;
        retired_alias
            .as_object_mut()
            .expect("private execute request object")
            .insert("model_name".to_owned(), "model-1".into());
        norito::json::from_value::<PrivateUploadedModelExecuteRequest>(retired_alias)
            .expect_err("private execution must reject the discovery-only model_name alias");
    }
    #[test]
    fn private_execution_submission_tracker_coalesces_exact_concurrent_retries() {
        let app = mk_app_state_for_tests();
        let request = PrivateUploadedModelExecuteRequest {
            service_name: "private_model_host".to_owned(),
            service_version: "1.0.0".to_owned(),
            weight_version: "v1".to_owned(),
            model_id: "upload-1".to_owned(),
            bundle_root: Hash::new(b"uploaded-model-bundle"),
            decryption_request_id: "decrypt-upload-input".to_owned(),
            input_artifact: sample_private_model_artifact_ref("input", 0xD1),
            output_recipient: sample_uploaded_model_register_payload()
                .bundle
                .upload_recipient,
        };
        let first = match claim_private_execution_submission(&app, &request)
            .expect("first exact private execution must acquire the request key")
        {
            PrivateExecutionSubmissionClaim::Acquired(guard) => guard,
            PrivateExecutionSubmissionClaim::Cached(_) => {
                panic!("a fresh private execution must not be cached")
            }
        };
        let duplicate = claim_private_execution_submission(&app, &request)
            .err()
            .expect("an exact concurrent retry must be coalesced");
        assert_eq!(duplicate.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(duplicate.message.contains("already executing"));
        drop(first);
        assert!(matches!(
            claim_private_execution_submission(&app, &request)
                .expect("a failed execution guard must release its request key"),
            PrivateExecutionSubmissionClaim::Acquired(_)
        ));
    }
    #[test]
    fn rollout_response_mirrors_are_closed_and_require_explicit_baseline() {
        macro_rules! assert_closed {
            ($value:expr, $ty:ty, $label:literal) => {{
                let mut value =
                    norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
                norito::json::from_value::<$ty>(value.clone()).expect(concat!(
                    "canonical ",
                    $label,
                    " must decode"
                ));
                value
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert("retired_v0".to_owned(), norito::json::Value::from(true));
                norito::json::from_value::<$ty>(value)
                    .expect_err(concat!($label, " must reject unknown fields"));
            }};
        }
        assert_closed!(SoracloudAction::Deploy, SoracloudAction, "Soracloud action");
        assert_closed!(RolloutStage::Canary, RolloutStage, "rollout stage");

        let state = RolloutRuntimeState {
            rollout_handle: "web_portal:rollout:2".to_owned(),
            baseline_version: "1.0.0".to_owned(),
            candidate_version: "2.0.0".to_owned(),
            canary_percent: 10,
            traffic_percent: 10,
            stage: RolloutStage::Canary,
            health_failures: 0,
            max_health_failures: 3,
            health_window_secs: 30,
            created_sequence: 1,
            updated_sequence: 1,
        };
        assert_closed!(state.clone(), RolloutRuntimeState, "rollout runtime state");
        let canonical = norito::json::to_value(&state).expect("serialize rollout runtime state");
        assert!(
            canonical
                .get("baseline_version")
                .and_then(norito::json::Value::as_str)
                == Some("1.0.0")
        );
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("rollout runtime state JSON object")
                .remove("baseline_version")
                .is_some()
        );
        norito::json::from_value::<RolloutRuntimeState>(missing)
            .expect_err("rollout runtime state must reject omitted baseline_version");
        let mut explicit_null = canonical;
        explicit_null
            .as_object_mut()
            .expect("rollout runtime state JSON object")
            .insert("baseline_version".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<RolloutRuntimeState>(explicit_null)
            .expect_err("rollout runtime state must reject null baseline_version");
    }
    #[test]
    fn service_control_response_types_reject_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($($ty:ty),+ $(,)?) => {
                $(
                    let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                        .expect_err(concat!(stringify!($ty), " must reject unknown fields"));
                    assert!(
                        matches!(
                            error,
                            norito::json::Error::UnknownField { ref field }
                                if field == "retired_v0"
                        ),
                        "{} reported the wrong error: {error}",
                        stringify!($ty)
                    );
                )+
            };
        }
        assert_unknown_rejected!(
            RolloutResponse,
            StateMutationResponse,
            ServiceMaterialMutationOperation,
            ServiceConfigMutationResponse,
            ServiceSecretMutationResponse,
            ServiceConfigStatusQuery,
            ServiceConfigStatusEntry,
            ServiceConfigStatusResponse,
            ServiceSecretStatusQuery,
            ServiceSecretStatusEntry,
            ServiceSecretStatusResponse,
            FheJobRunResponse,
            DecryptionRequestResponse,
            CiphertextQueryResponse,
            MutationResponse,
            ControlPlaneSnapshot,
            ControlPlaneServiceLeaseSnapshot,
            ControlPlaneServiceSnapshot,
            ControlPlaneServiceRevision,
            ControlPlaneAuditEvent,
        );
    }
    #[test]
    fn soracloud_torii_json_graph_rejects_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($($ty:ty),+ $(,)?) => {
                $(
                    let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                        .expect_err(concat!(stringify!($ty), " must reject unknown fields"));
                    assert!(
                        matches!(
                            error,
                            norito::json::Error::UnknownField { ref field }
                                if field == "retired_v0"
                        ),
                        "{} reported the wrong error: {error}",
                        stringify!($ty)
                    );
                )+
            };
        }

        assert_unknown_rejected!(
            AgentApartmentAction,
            AgentRuntimeStatus,
            TrainingJobAction,
            TrainingJobStatus,
            ModelWeightAction,
            ModelArtifactAction,
            UploadedModelAction,
            AppInfraStatusQuery,
            TrainingJobMutationResponse,
            TrainingJobStatusResponse,
            TrainingJobStatusEntry,
            ModelWeightMutationResponse,
            ModelWeightStatusResponse,
            ModelWeightStatusEntry,
            ModelWeightVersionEntry,
            ModelArtifactMutationResponse,
            ModelArtifactStatusResponse,
            ModelArtifactStatusEntry,
            UploadedModelStatusResponse,
            UploadedModelEncryptionRecipientResponse,
            PrivateUploadedModelExecuteResponse,
            PrivateUploadedModelReceiptQuery,
            PrivateUploadedModelReceiptListResponse,
            UploadedModelMutationResponse,
            HfSharedLeaseStatusResponse,
            HfSharedLeaseMutationResponse,
            ModelHostMutationAction,
            ModelHostStatusResponse,
            ModelHostMutationResponse,
            HealthComplianceReportQuery,
            TrainingJobStatusQuery,
            ModelWeightStatusQuery,
            ModelArtifactStatusQuery,
            UploadedModelStatusQuery,
            HfSharedLeaseStatusQuery,
            ModelHostStatusQuery,
            AgentAutonomyStatusQuery,
            AgentStatusQuery,
            AgentMailboxStatusQuery,
            HealthComplianceReportResponse,
            HealthAccessAuditEntry,
            HealthJurisdictionStat,
            HealthDataFlowAttestation,
            HealthPolicyDiffEntry,
            SoracloudPublicServiceDiscoveryV1,
            SoracloudPublicServiceDiscoveryRegistryV1,
            ServicePublicDiscoveryResponse,
            CiphertextRuntimeRecord,
            AgentMutationResponse,
            AgentStatusResponse,
            AgentApartmentStatusEntry,
            AgentWalletMutationResponse,
            AgentMailboxMutationResponse,
            AgentMailboxStatusResponse,
            AgentMailboxMessageEntry,
            SoracloudMutationDraftResponse,
            SoracloudTxInstr,
        );
    }
    #[test]
    fn soracloud_torii_body_keys_are_explicit_and_query_absence_is_semantic() {
        macro_rules! assert_explicit_body_keys {
            ($value:expr, $ty:ty, nulls = [$($nullable:literal),* $(,)?], collections = [$($collection:literal),* $(,)?], $label:literal) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect(concat!("serialize canonical ", $label));
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("canonical ", $label, " must decode"));
                $(
                    assert!(
                        canonical
                            .get($nullable)
                            .is_some_and(norito::json::Value::is_null),
                        "{} must serialize `{}` as explicit null",
                        $label,
                        $nullable
                    );
                    let mut missing = canonical.clone();
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($nullable);
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject an omitted nullable key"));
                )*
                $(
                    assert!(
                        canonical
                            .get($collection)
                            .and_then(norito::json::Value::as_array)
                            .is_some_and(Vec::is_empty),
                        "{} must serialize `{}` as an explicit empty list",
                        $label,
                        $collection
                    );
                    let mut missing = canonical.clone();
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($collection);
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject an omitted collection key"));
                )*
            }};
        }

        assert_explicit_body_keys!(
            ModelWeightStatusEntry {
                service_name: "model_host".to_owned(),
                model_name: "model-1".to_owned(),
                current_version: None,
                version_count: 0,
                versions: Vec::new(),
            },
            ModelWeightStatusEntry,
            nulls = ["current_version"],
            collections = ["versions"],
            "model-weight status entry"
        );
        assert_explicit_body_keys!(
            HealthComplianceReportResponse {
                schema_version: CONTROL_PLANE_SCHEMA_VERSION,
                service_name: None,
                jurisdiction_tag: None,
                generated_at_sequence: 0,
                total_access_events: 0,
                break_glass_events: 0,
                non_break_glass_events: 0,
                consent_evidence_present_events: 0,
                consent_evidence_coverage_bps: 0,
                recent_access_events: Vec::new(),
                jurisdiction_stats: Vec::new(),
                data_flow_attestations: Vec::new(),
                policy_diff_history: Vec::new(),
            },
            HealthComplianceReportResponse,
            nulls = ["service_name", "jurisdiction_tag"],
            collections = [
                "recent_access_events",
                "jurisdiction_stats",
                "data_flow_attestations",
                "policy_diff_history",
            ],
            "health compliance report"
        );
        assert_explicit_body_keys!(
            AgentWalletMutationResponse {
                action: AgentApartmentAction::PolicyRevoked,
                apartment_name: "ops_agent".to_owned(),
                sequence: 1,
                manifest_hash: Hash::new(b"agent manifest"),
                status: AgentRuntimeStatus::Running,
                request_id: None,
                asset_definition: None,
                amount: None,
                day_bucket: None,
                day_spent: None,
                capability: None,
                reason: None,
                pending_request_count: 0,
                revoked_policy_capability_count: 1,
                audit_event_count: 1,
                signed_by: "signer".to_owned(),
            },
            AgentWalletMutationResponse,
            nulls = [
                "request_id",
                "asset_definition",
                "amount",
                "day_bucket",
                "day_spent",
                "capability",
                "reason",
            ],
            collections = [],
            "agent wallet mutation response"
        );
        assert_explicit_body_keys!(
            PrivateUploadedModelReceiptListResponse {
                schema_version: iroha_data_model::soracloud::SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
                receipts: Vec::new(),
                total: None,
                returned_items: 0,
                remaining_items: None,
                has_more: false,
                count_mode: "bounded".to_owned(),
                continue_cursor: None,
            },
            PrivateUploadedModelReceiptListResponse,
            nulls = ["total", "remaining_items", "continue_cursor"],
            collections = ["receipts"],
            "private uploaded-model receipt list"
        );

        let empty_query = norito::json::from_str::<PrivateUploadedModelReceiptQuery>("{}")
            .expect("an empty private receipt query has semantic filter absence");
        assert!(empty_query.receipt_id.is_none());
        assert!(empty_query.service_name.is_none());
        assert!(empty_query.model_id.is_none());
        assert!(empty_query.weight_version.is_none());
        assert!(empty_query.cursor.is_none());
        assert!(empty_query.limit.is_none());
        assert!(empty_query.count_mode.is_none());

        let partial_query =
            norito::json::from_str::<PrivateUploadedModelReceiptQuery>(r#"{"limit":10}"#)
                .expect("a partial private receipt query must decode present filters only");
        assert_eq!(partial_query.limit, Some(10));
        let unknown =
            norito::json::from_str::<PrivateUploadedModelReceiptQuery>(r#"{"retired_v0":true}"#)
                .expect_err("a private receipt query must reject unknown keys");
        assert!(
            matches!(
                unknown,
                norito::json::Error::UnknownField { ref field } if field == "retired_v0"
            ),
            "unexpected private receipt query error: {unknown}"
        );
        let invalid_count = norito::json::from_str::<PrivateUploadedModelReceiptQuery>(
            r#"{"count_mode":"surprise"}"#,
        )
        .expect("the query decoder leaves semantic count validation to the handler");
        private_uploaded_model_receipt_count_mode(invalid_count.count_mode.as_deref())
            .expect_err("an unknown private receipt count_mode must be rejected");
        assert_eq!(
            private_uploaded_model_receipt_limit(None).expect("default receipt limit"),
            usize::try_from(PRIVATE_UPLOADED_MODEL_RECEIPT_DEFAULT_LIMIT)
                .expect("default receipt limit fits usize")
        );
        assert_eq!(
            private_uploaded_model_receipt_limit(Some(PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT))
                .expect("maximum receipt limit"),
            usize::try_from(PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT)
                .expect("maximum receipt limit fits usize")
        );
        for invalid_limit in [0, PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT + 1] {
            private_uploaded_model_receipt_limit(Some(invalid_limit))
                .expect_err("out-of-range private receipt limit must be rejected");
        }

        let filter_digest = private_uploaded_model_receipt_filter_digest(
            Some(&Hash::new(b"receipt filter")),
            Some("web_portal"),
            Some("upload-1"),
            Some("v1"),
        );
        let cursor = PrivateUploadedModelReceiptCursorV1 {
            snapshot_sequence: 9,
            after_sequence: 7,
            after_receipt_id: Hash::new(b"receipt cursor"),
        };
        let encoded = encode_private_uploaded_model_receipt_cursor(filter_digest, cursor);
        assert_eq!(
            encoded.len(),
            PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_ENCODED_CHARS_V1
        );
        assert_eq!(
            decode_private_uploaded_model_receipt_cursor(&encoded, filter_digest)
                .expect("canonical private receipt cursor"),
            cursor
        );
        decode_private_uploaded_model_receipt_cursor(&format!("{encoded}="), filter_digest)
            .expect_err("a padded private receipt cursor must be rejected before decoding");
        decode_private_uploaded_model_receipt_cursor(&encoded[..encoded.len() - 1], filter_digest)
            .expect_err("a truncated private receipt cursor must be rejected before decoding");
        let mut wrong_magic = encoded.as_bytes().to_vec();
        wrong_magic[0] = if wrong_magic[0] == b'A' { b'B' } else { b'A' };
        decode_private_uploaded_model_receipt_cursor(
            std::str::from_utf8(&wrong_magic).expect("cursor mutation remains ASCII"),
            filter_digest,
        )
        .expect_err("a private receipt cursor with the wrong magic must be rejected");
        decode_private_uploaded_model_receipt_cursor(
            &encoded,
            private_uploaded_model_receipt_filter_digest(None, None, None, None),
        )
        .expect_err("a private receipt cursor must remain bound to its exact filters");
        for invalid_cursor in [
            PrivateUploadedModelReceiptCursorV1 {
                after_sequence: 0,
                ..cursor
            },
            PrivateUploadedModelReceiptCursorV1 {
                snapshot_sequence: cursor.after_sequence - 1,
                ..cursor
            },
        ] {
            let encoded =
                encode_private_uploaded_model_receipt_cursor(filter_digest, invalid_cursor);
            decode_private_uploaded_model_receipt_cursor(&encoded, filter_digest)
                .expect_err("a cursor boundary must be within its non-zero snapshot");
        }
    }
    #[test]
    fn private_uploaded_model_receipt_query_rejects_noncanonical_filters() {
        use iroha_core::state::World;

        let app = mk_app_state_for_tests_with_world(World::default());
        let canonical = PrivateUploadedModelReceiptQuery {
            receipt_id: None,
            service_name: Some("web_portal".to_owned()),
            model_id: Some("upload-1".to_owned()),
            weight_version: Some("v1".to_owned()),
            cursor: None,
            limit: Some(1),
            count_mode: Some("exact".to_owned()),
        };
        let response =
            authoritative_private_uploaded_model_receipts_response(&app, canonical.clone())
                .expect("canonical empty receipt query must succeed");
        assert!(response.receipts.is_empty());
        assert_eq!(response.total, Some(0));
        assert_eq!(response.remaining_items, Some(0));
        assert!(!response.has_more);
        assert!(response.continue_cursor.is_none());

        let assert_bad_request = |query: PrivateUploadedModelReceiptQuery, expected: &str| {
            let error = authoritative_private_uploaded_model_receipts_response(&app, query)
                .expect_err("noncanonical receipt filter must fail");
            assert_eq!(error.status(), StatusCode::BAD_REQUEST);
            assert!(
                error.message.contains(expected),
                "expected `{expected}` in `{}`",
                error.message
            );
        };

        let mut query = canonical.clone();
        query.service_name = Some("cafe\u{301}".to_owned());
        assert_bad_request(query, "service_name must use canonical NFC form");

        let mut query = canonical.clone();
        query.model_id = Some(" upload-1".to_owned());
        assert_bad_request(query, "job_id must not contain whitespace");

        let mut query = canonical;
        query.weight_version = Some("v1 ".to_owned());
        assert_bad_request(query, "weight_version must not contain whitespace");
    }
    #[test]
    fn private_uploaded_model_receipt_pagination_is_snapshot_stable_and_exact() {
        let network_id = iroha_data_model::id::NetworkId::from_genesis_hash(HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"private receipt pagination network"),
        ));
        let mut receipts = vec![
            sample_private_uploaded_model_receipt_for_pagination(
                network_id,
                4,
                0x44,
                "web_portal",
                "upload-1",
                "v1",
            ),
            sample_private_uploaded_model_receipt_for_pagination(
                network_id,
                1,
                0x11,
                "web_portal",
                "upload-1",
                "v1",
            ),
            sample_private_uploaded_model_receipt_for_pagination(
                network_id,
                3,
                0x33,
                "web_portal",
                "upload-1",
                "v1",
            ),
            sample_private_uploaded_model_receipt_for_pagination(
                network_id,
                2,
                0x22,
                "web_portal",
                "upload-1",
                "v1",
            ),
            sample_private_uploaded_model_receipt_for_pagination(
                network_id,
                5,
                0x55,
                "other_service",
                "upload-1",
                "v1",
            ),
        ];
        let filter_digest = private_uploaded_model_receipt_filter_digest(
            None,
            Some("web_portal"),
            Some("upload-1"),
            Some("v1"),
        );
        let page = |receipts: &[(Hash, SoraPrivateUploadedModelExecutionReceiptV1)],
                    cursor: Option<PrivateUploadedModelReceiptCursorV1>,
                    count_mode: PrivateUploadedModelReceiptCountMode,
                    current_sequence: u64| {
            paginate_private_uploaded_model_receipts(
                receipts
                    .iter()
                    .map(|(receipt_id, receipt)| (receipt_id, receipt)),
                PrivateUploadedModelReceiptPageSpec {
                    receipt_id: None,
                    service_name: Some("web_portal"),
                    model_id: Some("upload-1"),
                    weight_version: Some("v1"),
                    filter_digest,
                    cursor,
                    limit: 2,
                    count_mode,
                    current_sequence,
                },
            )
        };

        let first = page(
            &receipts,
            None,
            PrivateUploadedModelReceiptCountMode::Exact,
            5,
        )
        .expect("first exact private receipt page");
        assert_eq!(
            first
                .receipts
                .iter()
                .map(|receipt| receipt.emitted_sequence)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "receipt pages must use canonical emitted-sequence order instead of storage order"
        );
        assert_eq!(first.total, Some(4));
        assert_eq!(first.returned_items, 2);
        assert_eq!(first.remaining_items, Some(2));
        assert!(first.has_more);
        let first_cursor = decode_private_uploaded_model_receipt_cursor(
            first
                .continue_cursor
                .as_deref()
                .expect("a non-terminal page must carry a cursor"),
            filter_digest,
        )
        .expect("decode first-page cursor");
        assert_eq!(first_cursor.snapshot_sequence, 5);
        assert_eq!(first_cursor.after_sequence, 2);
        assert_eq!(first_cursor.after_receipt_id, first.receipts[1].receipt_id);

        receipts.push(sample_private_uploaded_model_receipt_for_pagination(
            network_id,
            6,
            0x66,
            "web_portal",
            "upload-1",
            "v1",
        ));
        let second = page(
            &receipts,
            Some(first_cursor),
            PrivateUploadedModelReceiptCountMode::Exact,
            6,
        )
        .expect("second exact private receipt page");
        assert_eq!(
            second
                .receipts
                .iter()
                .map(|receipt| receipt.emitted_sequence)
                .collect::<Vec<_>>(),
            vec![3, 4],
            "a continuation must exclude receipts appended after its first-page snapshot"
        );
        assert_eq!(second.total, Some(4));
        assert_eq!(second.returned_items, 2);
        assert_eq!(second.remaining_items, Some(0));
        assert!(!second.has_more);
        assert!(second.continue_cursor.is_none());

        let bounded = page(
            &receipts,
            None,
            PrivateUploadedModelReceiptCountMode::Bounded,
            6,
        )
        .expect("bounded private receipt page");
        assert_eq!(bounded.total, None);
        assert_eq!(bounded.remaining_items, None);
        assert_eq!(bounded.returned_items, 2);
        assert!(bounded.has_more);
        assert!(bounded.continue_cursor.is_some());

        let without_boundary = receipts
            .iter()
            .filter(|(_receipt_id, receipt)| receipt.emitted_sequence != 2)
            .cloned()
            .collect::<Vec<_>>();
        let error = page(
            &without_boundary,
            Some(first_cursor),
            PrivateUploadedModelReceiptCountMode::Exact,
            6,
        )
        .expect_err("a cursor boundary must still name an exact retained receipt");
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(error.message.contains("exact retained receipt"));

        let error = page(
            &receipts,
            Some(PrivateUploadedModelReceiptCursorV1 {
                snapshot_sequence: 7,
                ..first_cursor
            }),
            PrivateUploadedModelReceiptCountMode::Exact,
            6,
        )
        .expect_err("a cursor must not name a future snapshot");
        assert_eq!(error.status(), StatusCode::CONFLICT);
    }
    #[test]
    fn service_control_response_keys_are_explicit() {
        macro_rules! assert_required_nullable {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect(concat!("serialize canonical ", $label));
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("canonical ", $label, " must decode"));
                for field in [$($field),+] {
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect(concat!($label, " JSON object"))
                            .remove(field)
                            .is_some()
                    );
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject an omitted nullable key"));

                    let mut explicit_null = canonical.clone();
                    explicit_null
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .insert(field.to_owned(), norito::json::Value::Null);
                    norito::json::from_value::<$ty>(explicit_null)
                        .expect(concat!($label, " must accept an explicit null key"));
                }
            }};
        }
        macro_rules! assert_required_collection {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect(concat!("serialize canonical ", $label));
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("canonical ", $label, " must decode"));
                for field in [$($field),+] {
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect(concat!($label, " JSON object"))
                            .remove(field)
                            .is_some()
                    );
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject an omitted collection key"));

                    let mut null = canonical.clone();
                    null.as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .insert(field.to_owned(), norito::json::Value::Null);
                    norito::json::from_value::<$ty>(null)
                        .expect_err(concat!($label, " must reject a null collection key"));
                }
            }};
        }

        let config_mutation = ServiceConfigMutationResponse {
            action: SoracloudAction::ConfigMutation,
            service_name: "web_portal".to_owned(),
            config_name: "runtime".to_owned(),
            operation: ServiceMaterialMutationOperation::Delete,
            sequence: 1,
            current_version: "1.0.0".to_owned(),
            config_generation: 1,
            config_entry_count: 0,
            value_hash: None,
            audit_event_count: 1,
            signed_by: "signer".to_owned(),
        };
        assert_required_nullable!(
            config_mutation,
            ServiceConfigMutationResponse,
            ["value_hash"],
            "config mutation response"
        );
        let secret_mutation = ServiceSecretMutationResponse {
            action: SoracloudAction::SecretMutation,
            service_name: "web_portal".to_owned(),
            secret_name: "api_token".to_owned(),
            operation: ServiceMaterialMutationOperation::Delete,
            sequence: 1,
            current_version: "1.0.0".to_owned(),
            secret_generation: 1,
            secret_entry_count: 0,
            encryption: None,
            key_id: None,
            key_version: None,
            commitment: None,
            ciphertext_bytes: None,
            audit_event_count: 1,
            signed_by: "signer".to_owned(),
        };
        assert_required_nullable!(
            secret_mutation,
            ServiceSecretMutationResponse,
            [
                "encryption",
                "key_id",
                "key_version",
                "commitment",
                "ciphertext_bytes",
            ],
            "secret mutation response"
        );
        let mutation = MutationResponse {
            action: SoracloudAction::Deploy,
            service_name: "web_portal".to_owned(),
            previous_version: None,
            current_version: "1.0.0".to_owned(),
            sequence: 1,
            service_manifest_hash: Hash::new(b"service"),
            container_manifest_hash: Hash::new(b"container"),
            revision_count: 1,
            audit_event_count: 1,
            signed_by: "signer".to_owned(),
            rollout_handle: None,
            rollout_stage: None,
            rollout_percent: None,
        };
        assert_required_nullable!(
            mutation,
            MutationResponse,
            [
                "previous_version",
                "rollout_handle",
                "rollout_stage",
                "rollout_percent",
            ],
            "service mutation response"
        );

        let bundle = fixture_bundle("1.0.0");
        let deployment = fixture_service_deployment(&bundle);
        let audit = fixture_service_deploy_audit_event(&bundle);
        let revision =
            deployment_bundle_to_control_plane_revision(&deployment, &bundle, Some(&audit), None)
                .expect("valid fixture revision should project");
        assert_required_nullable!(
            revision.clone(),
            ControlPlaneServiceRevision,
            [
                "route_host",
                "route_path_prefix",
                "base_url",
                "healthcheck_url",
                "public_discovery_content_cid",
                "public_discovery_url",
                "public_discovery_cid_host_url",
                "healthcheck_path",
            ],
            "control-plane service revision"
        );
        assert_required_collection!(
            revision,
            ControlPlaneServiceRevision,
            [
                "state_bindings",
                "lease_volumes",
                "required_config_names",
                "required_secret_names",
                "config_exports",
            ],
            "control-plane service revision"
        );

        let service = ControlPlaneServiceSnapshot {
            service_name: "web_portal".to_owned(),
            current_version: "1.0.0".to_owned(),
            revision_count: 1,
            config_generation: 0,
            secret_generation: 0,
            config_entry_count: 0,
            secret_entry_count: 0,
            service_lease: None,
            public_discovery_content_cid: None,
            public_discovery_url: None,
            public_discovery_cid_host_url: None,
            latest_revision: None,
            active_rollout: None,
            last_rollout: None,
        };
        assert_required_nullable!(
            service.clone(),
            ControlPlaneServiceSnapshot,
            [
                "service_lease",
                "public_discovery_content_cid",
                "public_discovery_url",
                "public_discovery_cid_host_url",
                "latest_revision",
                "active_rollout",
                "last_rollout",
            ],
            "control-plane service snapshot"
        );
        let audit = ControlPlaneAuditEvent {
            sequence: 1,
            action: SoracloudAction::Deploy,
            service_name: "web_portal".to_owned(),
            from_version: None,
            to_version: "1.0.0".to_owned(),
            service_manifest_hash: Hash::new(b"service"),
            container_manifest_hash: Hash::new(b"container"),
            process_generation: 1,
            config_generation: 0,
            secret_generation: 0,
            config_snapshot_hash:
                iroha_data_model::soracloud::derive_soracloud_service_config_snapshot_hash_v1(
                    &BTreeMap::new(),
                ),
            secret_snapshot_hash:
                iroha_data_model::soracloud::derive_soracloud_service_secret_snapshot_hash_v1(
                    &BTreeMap::new(),
                ),
            binding_name: None,
            state_key: None,
            config_mutations: Vec::new(),
            secret_mutations: Vec::new(),
            governance_tx_hash: None,
            rollout_state: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            lease_usage: None,
            service_lease_commitment: None,
            lease_reporting_epoch_rollover: None,
            signed_by: "signer".to_owned(),
        };
        assert_required_nullable!(
            audit.clone(),
            ControlPlaneAuditEvent,
            [
                "from_version",
                "binding_name",
                "state_key",
                "governance_tx_hash",
                "rollout_state",
                "policy_name",
                "policy_snapshot_hash",
                "jurisdiction_tag",
                "consent_evidence_hash",
                "break_glass",
                "break_glass_reason",
                "lease_usage",
                "service_lease_commitment",
                "lease_reporting_epoch_rollover",
            ],
            "control-plane audit event"
        );
        let snapshot = ControlPlaneSnapshot {
            schema_version: CONTROL_PLANE_SCHEMA_VERSION,
            service_count: 1,
            audit_event_count: 1,
            services: vec![service],
            recent_audit_events: vec![audit],
        };
        assert_required_collection!(
            snapshot,
            ControlPlaneSnapshot,
            ["services", "recent_audit_events"],
            "control-plane snapshot"
        );

        assert_required_collection!(
            ServiceConfigStatusResponse {
                schema_version: CONTROL_PLANE_SCHEMA_VERSION,
                service_name: "web_portal".to_owned(),
                current_version: "1.0.0".to_owned(),
                config_generation: 0,
                config_entry_count: 0,
                configs: Vec::new(),
            },
            ServiceConfigStatusResponse,
            ["configs"],
            "service config status response"
        );
        assert_required_collection!(
            ServiceSecretStatusResponse {
                schema_version: CONTROL_PLANE_SCHEMA_VERSION,
                service_name: "web_portal".to_owned(),
                current_version: "1.0.0".to_owned(),
                secret_generation: 0,
                secret_entry_count: 0,
                secrets: Vec::new(),
            },
            ServiceSecretStatusResponse,
            ["secrets"],
            "service secret status response"
        );
    }
    #[test]
    fn rollout_advance_payload_rejects_unknown_fields() {
        let governance_tx_hash = Hash::new(b"governance");
        let mut fields = norito::json::Map::new();
        fields.insert(
            "service_name".to_owned(),
            norito::json::Value::from("web_portal"),
        );
        fields.insert(
            "rollout_handle".to_owned(),
            norito::json::Value::from("web_portal:rollout:2"),
        );
        fields.insert("healthy".to_owned(), norito::json::Value::from(true));
        fields.insert(
            "promote_to_percent".to_owned(),
            norito::json::Value::from(100_u64),
        );
        fields.insert(
            "governance_tx_hash".to_owned(),
            norito::json::to_value(&governance_tx_hash).expect("serialize governance hash"),
        );
        let canonical = norito::json::Value::Object(fields);
        norito::json::from_value::<RolloutAdvancePayload>(canonical.clone())
            .expect("canonical rollout advance payload must decode");

        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("rollout advance payload JSON object")
                .remove("promote_to_percent")
                .is_some()
        );
        norito::json::from_value::<RolloutAdvancePayload>(missing)
            .expect_err("omitted rollout promotion target must be rejected");

        let mut null = canonical.clone();
        null.as_object_mut()
            .expect("rollout advance payload JSON object")
            .insert("promote_to_percent".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<RolloutAdvancePayload>(null)
                .expect("explicit null rollout promotion target must decode")
                .promote_to_percent
                .is_none()
        );

        let mut unknown = canonical;
        unknown
            .as_object_mut()
            .expect("rollout advance payload JSON object")
            .insert("retired_v0".to_owned(), norito::json::Value::from(true));
        let error = norito::json::from_value::<RolloutAdvancePayload>(unknown)
            .expect_err("rollout advance payload must reject unknown fields");
        assert!(
            matches!(
                error,
                norito::json::Error::UnknownField { ref field } if field == "retired_v0"
            ),
            "unexpected rollout unknown-field rejection: {error}"
        );
    }
    #[test]
    fn soracloud_post_requests_reject_inline_signing_fields_during_decode() {
        macro_rules! assert_rejects_inline_signing_fields {
            ($($request:ty),+ $(,)?) => {
                $(
                    for field in ["authority", "private_key"] {
                        let json = format!(r#"{{"{field}":"must-not-cross-torii"}}"#);
                        let error = norito::json::from_str::<$request>(&json)
                            .expect_err("retired inline signing field must fail JSON admission");
                        let message = error.to_string();
                        assert!(
                            message.contains("unknown field") && message.contains(field),
                            "{} admitted retired field `{field}`: {message}",
                            stringify!($request),
                        );
                    }
                )+
            };
        }
        assert_rejects_inline_signing_fields!(
            SignedBundleRequest,
            SignedAppInfraRequest,
            SignedRollbackRequest,
            SignedStateMutationRequest,
            SignedServiceConfigSetRequest,
            SignedServiceConfigDeleteRequest,
            SignedServiceSecretSetRequest,
            SignedServiceSecretDeleteRequest,
            SignedRolloutAdvanceRequest,
            SignedAgentDeployRequest,
            SignedAgentLeaseRenewRequest,
            SignedHfDeployRequest,
            SignedHfLeaseLeaveRequest,
            SignedHfLeaseRenewRequest,
            SignedModelHostAdvertiseRequest,
            SignedModelHostHeartbeatRequest,
            SignedModelHostWithdrawRequest,
            SignedAgentRestartRequest,
            SignedAgentPolicyRevokeRequest,
            SignedAgentWalletSpendRequest,
            SignedAgentWalletApproveRequest,
            SignedAgentMessageSendRequest,
            SignedAgentMessageAckRequest,
            SignedAgentArtifactAllowRequest,
            SignedAgentAutonomyRunRequest,
            SignedFheJobRunRequest,
            SignedTrainingJobStartRequest,
            SignedTrainingJobCheckpointRequest,
            SignedTrainingJobRetryRequest,
            SignedModelWeightRegisterRequest,
            SignedModelWeightPromoteRequest,
            SignedModelWeightRollbackRequest,
            SignedModelArtifactRegisterRequest,
            SignedUploadedModelRegisterRequest,
            SignedDecryptionRequest,
            SignedCiphertextQueryRequest,
            PrivateUploadedModelExecuteRequest,
            AgentAutonomyFinalizeRequest,
        );
    }
    #[test]
    fn require_soracloud_mutation_signer_derives_authority_from_verified_headers() {
        let key_pair = checked_test_keypair(0x60);
        let account = AccountId::new(key_pair.public_key().clone());
        let headers = verified_request_headers(&account, key_pair.public_key());
        let provenance = ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: checked_test_signature(key_pair.private_key(), b"mutation"),
        };
        let signer = require_soracloud_mutation_signer(&headers, &provenance)
            .expect("verified headers and matching provenance must identify the mutation signer");
        assert_eq!(signer.authority, account);
        assert_eq!(signer.request_signer, provenance.signer);
    }
    #[test]
    fn app_infra_request_rejects_inline_signing_fields_in_nested_service_bundles() {
        for field in ["authority", "private_key"] {
            let json = format!(r#"{{"deploy_services":[{{"{field}":"must-not-cross-torii"}}]}}"#);
            let error = norito::json::from_str::<SignedAppInfraRequest>(&json)
                .expect_err("nested service bundle signing material must fail JSON admission");
            let message = error.to_string();
            assert!(
                message.contains("unknown field") && message.contains(field),
                "nested retired field `{field}` was not rejected: {message}"
            );
        }
    }
    #[test]
    fn require_soracloud_mutation_signer_binds_provenance_to_request_signer() {
        let request_keypair = checked_test_keypair(0x61);
        let provenance_keypair = checked_test_keypair(0x62);
        let account = AccountId::new(request_keypair.public_key().clone());
        let headers = verified_request_headers(&account, request_keypair.public_key());
        let provenance = ManifestProvenance {
            signer: provenance_keypair.public_key().clone(),
            signature: checked_test_signature(provenance_keypair.private_key(), b"mutation"),
        };
        let error = match require_soracloud_mutation_signer(&headers, &provenance) {
            Ok(_) => panic!("provenance signer mismatch must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
    }
    #[test]
    fn require_soracloud_mutation_signer_accepts_multisig_member_provenance() {
        let primary_request_keypair = checked_test_keypair(0x63);
        let provenance_keypair = checked_test_keypair(0x64);
        let account = AccountId::new(primary_request_keypair.public_key().clone());
        let headers = verified_request_headers_with_signers(
            &account,
            primary_request_keypair.public_key(),
            &[
                primary_request_keypair.public_key().clone(),
                provenance_keypair.public_key().clone(),
            ],
        );
        let provenance = ManifestProvenance {
            signer: provenance_keypair.public_key().clone(),
            signature: checked_test_signature(provenance_keypair.private_key(), b"mutation"),
        };
        let signer = require_soracloud_mutation_signer(&headers, &provenance)
            .expect("multisig member provenance must be accepted");
        assert_eq!(signer.authority, account);
        assert_eq!(signer.request_signer, provenance.signer);
    }
    #[test]
    fn control_plane_snapshot_uses_authoritative_soracloud_state() -> Result<(), eyre::Report> {
        use iroha_core::{smartcontracts::Execute, state::World};
        use iroha_data_model::block::BlockHeader;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let wonderland: iroha_data_model::domain::DomainId =
                DomainId::try_new("wonderland", "universal")?;
            let mut world = World::default();
            seed_domain_name_lease(&mut world, &SAMPLE_GENESIS_ACCOUNT_ID, &wonderland);
            let app = mk_app_state_for_tests_with_world(world);
            let block_header = BlockHeader {
                height: NonZeroU64::new(1).expect("non-zero block height"),
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                npos_effects_hash: None,
                sccp_commitment_root: None,
                execution_context_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            };
            let mut state_block = app.state.block(block_header);
            let mut stx = state_block.transaction();
            Register::domain(Domain::new(wonderland.clone()))
                .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
            Register::account(Account::new(ALICE_ID.clone()))
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
            let initial_service_configs = BTreeMap::from([(
                "ui/theme".to_string(),
                Json::from(norito::json!({
                    "accent": "citrus",
                    "mode": "light",
                })),
            )]);
            let provenance = {
                let payload = encode_bundle_signature_payload(
                    &bundle,
                    &initial_service_configs,
                    &BTreeMap::new(),
                    &SoraServiceMutationPreconditionV1::ServiceAbsent,
                )
                .expect("encode bundle payload");
                ManifestProvenance {
                    signer: ALICE_ID.expect_single_signatory().clone(),
                    signature: checked_test_signature(
                        iroha_test_samples::ALICE_KEYPAIR.private_key(),
                        &payload,
                    ),
                }
            };
            isi::soracloud::DeploySoracloudService {
                bundle,
                initial_service_configs,
                initial_service_secrets: BTreeMap::new(),
                precondition: SoraServiceMutationPreconditionV1::ServiceAbsent,
                provenance,
            }
            .execute(&ALICE_ID, &mut stx)?;
            stx.apply();
            state_block.commit()?;
            let snapshot = control_plane_snapshot(&app, Some("web_portal"), 10)?;
            assert_eq!(snapshot.service_count, 1);
            assert_eq!(snapshot.audit_event_count, 1);
            assert_eq!(snapshot.services[0].current_version, "1.0.0");
            assert_eq!(
                snapshot.services[0]
                    .latest_revision
                    .as_ref()
                    .expect("latest revision")
                    .signed_by,
                ALICE_ID.expect_single_signatory().to_string()
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
    fn control_plane_revision_fails_closed_without_lifecycle_or_scr_admission_evidence() {
        let bundle = fixture_bundle("1.0.0");
        let deployment = fixture_service_deployment(&bundle);
        let error = deployment_bundle_to_control_plane_revision(&deployment, &bundle, None, None)
            .expect_err("a revision without lifecycle audit evidence must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Internal);
        assert!(
            error
                .message
                .contains("no authoritative lifecycle audit event")
        );

        let mut over_cap_bundle = fixture_bundle("1.0.0");
        over_cap_bundle.container.resources.cpu_millis =
            NonZeroU32::new(SCR_HOST_MAX_CPU_MILLIS + 1).expect("non-zero cpu");
        over_cap_bundle.service.container.manifest_hash = over_cap_bundle.container_manifest_hash();
        let over_cap_deployment = fixture_service_deployment(&over_cap_bundle);
        let audit = fixture_service_deploy_audit_event(&over_cap_bundle);
        let error = deployment_bundle_to_control_plane_revision(
            &over_cap_deployment,
            &over_cap_bundle,
            Some(&audit),
            None,
        )
        .expect_err("an active revision that fails SCR admission must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Internal);
        assert!(error.message.contains("fails authoritative SCR admission"));
    }
    #[test]
    fn rollout_projection_fails_closed_on_substituted_governance_hash() {
        use iroha_core::state::World;

        let mut world = World::default();
        let bundle = fixture_bundle("1.0.0");
        let service_name = bundle.service.service_name.clone();
        let rollout_handle = format!("{}:rollout:2", service_name);
        let rollout = SoraServiceRolloutStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: rollout_handle.clone(),
            baseline_version: "0.9.0".to_owned(),
            candidate_version: bundle.service.service_version.clone(),
            canary_percent: 10,
            traffic_percent: 10,
            stage: SoraRolloutStageV1::Canary,
            health_failures: 0,
            max_health_failures: 2,
            health_window_secs: 60,
            created_sequence: 2,
            updated_sequence: 2,
        };
        let mut deployment = fixture_service_deployment(&bundle);
        deployment.active_rollout = Some(rollout.clone());
        deployment.last_rollout = Some(rollout.clone());
        insert_revision(&mut world, &bundle, service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name.clone(), deployment);
        let authoritative_governance = Hash::new(b"authoritative rollout governance");
        let mut audit = fixture_service_deploy_audit_event(&bundle);
        audit.sequence = 2;
        audit.action = SoraServiceLifecycleActionV1::Rollout;
        audit.from_version = Some(bundle.service.service_version.clone());
        audit.governance_tx_hash = Some(authoritative_governance);
        audit.rollout_state = Some(rollout);
        world
            .soracloud_service_audit_events_mut_for_testing()
            .insert(2, audit);
        let app = mk_app_state_for_tests_with_world(world);
        let error = authoritative_rollout_mutation_response(
            &app,
            &SoracloudAuditBaseline {
                service_max: 1,
                ..SoracloudAuditBaseline::default()
            },
            service_name.as_ref(),
            &rollout_handle,
            Hash::new(b"substituted rollout governance"),
        )
        .expect_err("substituted rollout governance hash must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Conflict);
        assert!(
            error
                .message
                .contains("does not bind the requested governance_tx_hash")
        );
    }
    #[tokio::test]
    async fn resolve_public_local_read_route_uses_authoritative_service_route_state() {
        use iroha_core::state::World;
        let mut world = World::new();
        let bundle = fixture_bundle("2026.02.0");
        let service_name = bundle.service.service_name.clone();
        insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name.clone(), fixture_service_deployment(&bundle));
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
    #[tokio::test]
    async fn resolve_public_route_projects_http_service_inrous() {
        use iroha_core::state::World;
        let mut world = World::new();
        let bundle = fixture_hosted_http_inrou_bundle("2026.04.0");
        let service_name = bundle.service.service_name.clone();
        let lease = iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
            status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            quota_class: "taira-open".to_string(),
            deployment_deposit: "1".parse().expect("deployment deposit quantity"),
            prepaid_runtime_balance: "50".parse().expect("prepaid runtime quantity"),
            runtime_price_per_block: "0.00025".parse().expect("runtime price quantity"),
            storage_price_per_gib_block: "0.000025".parse().expect("storage price quantity"),
            egress_price_per_mib: "0.000005".parse().expect("egress price quantity"),
            lease_started_height: 1,
            lease_expires_height: 100,
            reporting_epoch: 1,
            settled_egress_bytes: 0,
            egress_reporter_checkpoints: Vec::new(),
            accounted_egress_bytes: 0,
            last_status_reason: None,
        };
        let lease_volume_states = fixture_service_lease_volume_states(&bundle, Some(&lease));
        let deployment = SoraServiceDeploymentStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
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
            fhe_policy_records: BTreeMap::new(),
            service_lease: Some(lease),
            lease_volume_states,
        };
        insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name.clone(), deployment.clone());
        let mut invalid_deployment = deployment;
        invalid_deployment
            .lease_volume_states
            .pop()
            .expect("hosted deployment has a lease-volume row");
        let mut invalid_volume_world = World::new();
        insert_revision(
            &mut invalid_volume_world,
            &bundle,
            bundle.service.service_name.to_string(),
        );
        invalid_volume_world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name.clone(), invalid_deployment);
        let app = mk_app_state_for_tests_with_world(world);
        let route_match = resolve_public_route(&app, "portal.sora", "GET", "/app/v1/health")
            .expect("http service route");
        match route_match {
            PublicRouteMatch::HostedHttp(route_match) => {
                assert_eq!(route_match.service_name, "web_portal");
                assert_eq!(route_match.service_version, "2026.04.0");
                assert_eq!(route_match.request_path, "/v1/health");
            }
            other => panic!("expected hosted-http route, got {other:?}"),
        }
        assert!(
            resolve_public_route(&app, "wrong.sora", "GET", "/app/v1/health").is_none(),
            "host matching must stay authoritative for hosted http services"
        );
        let invalid_volume_app = mk_app_state_for_tests_with_world(invalid_volume_world);
        assert!(
            resolve_public_route(&invalid_volume_app, "portal.sora", "GET", "/app/v1/health")
                .is_none(),
            "public routing must fail closed when authoritative storage rows do not exactly match the admitted bundle"
        );
    }
    #[tokio::test]
    async fn resolve_public_route_splits_hosted_live_search_from_vault_handlers() {
        use iroha_core::state::World;
        use iroha_data_model::soracloud::SoraMailboxContractV1;
        let mut world = World::new();
        let mut live_bundle = fixture_hosted_http_inrou_bundle("2026.04.0");
        live_bundle.service.service_name = "travel_ops_live".parse().expect("service name");
        live_bundle.service.route = Some(SoraRouteTargetV1 {
            host: "travel.sora".to_owned(),
            path_prefix: "/api/v1".to_owned(),
            service_port: NonZeroU16::new(8787).expect("nonzero literal"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        });
        let mut vault_bundle = fixture_bundle("2026.04.0");
        vault_bundle.service.service_name = "travel_ops_vault".parse().expect("service name");
        vault_bundle.service.route = Some(SoraRouteTargetV1 {
            host: "travel.sora".to_owned(),
            path_prefix: "/api".to_owned(),
            service_port: NonZeroU16::new(8788).expect("nonzero literal"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        });
        vault_bundle.service.handlers = vec![
            SoraServiceHandlerV1 {
                handler_name: "auth_me".parse().expect("handler"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_auth_me".to_owned(),
                route_path: Some("/auth/me".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "saved_searches_get".parse().expect("handler"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_saved_searches".to_owned(),
                route_path: Some("/v1/user/saved-searches".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "saved_searches_put".parse().expect("handler"),
                class: SoraServiceHandlerClassV1::PrivateUpdate,
                entrypoint: "store_saved_search".to_owned(),
                route_path: Some("/v1/user/saved-searches".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::None,
                mailbox: Some(SoraMailboxContractV1 {
                    queue_name: "private_updates".parse().expect("queue"),
                    max_pending_messages: NonZeroU32::new(128).expect("pending"),
                    max_message_bytes: NonZeroU64::new(131_072).expect("bytes"),
                    retention_blocks: NonZeroU32::new(64).expect("retention"),
                }),
            },
        ];
        for bundle in [live_bundle.clone(), vault_bundle.clone()] {
            let service_name = bundle.service.service_name.clone();
            insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
            let service_lease = if bundle.service.execution_plane
                == iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService
            {
                Some(iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
                    status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                    quota_class: "taira-open".to_string(),
                    deployment_deposit: "1".parse().expect("deployment deposit quantity"),
                    prepaid_runtime_balance: "50".parse().expect("prepaid runtime quantity"),
                    runtime_price_per_block: "0.00025".parse().expect("runtime price quantity"),
                    storage_price_per_gib_block: "0.000025"
                        .parse()
                        .expect("storage price quantity"),
                    egress_price_per_mib: "0.000005".parse().expect("egress price quantity"),
                    lease_started_height: 1,
                    lease_expires_height: 100,
                    reporting_epoch: 1,
                    settled_egress_bytes: 0,
                    egress_reporter_checkpoints: Vec::new(),
                    accounted_egress_bytes: 0,
                    last_status_reason: None,
                })
            } else {
                None
            };
            let lease_volume_states =
                fixture_service_lease_volume_states(&bundle, service_lease.as_ref());
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name,
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
                        fhe_policy_records: BTreeMap::new(),
                        service_lease,
                        lease_volume_states,
                    },
                );
        }
        let app = mk_app_state_for_tests_with_world(world);
        let live_route = resolve_public_route(&app, "travel.sora", "POST", "/api/v1/search")
            .expect("hosted live route");
        match live_route {
            PublicRouteMatch::HostedHttp(route_match) => {
                assert_eq!(route_match.service_name, "travel_ops_live");
                assert_eq!(route_match.request_path, "/search");
            }
            other => panic!("expected hosted-http live route, got {other:?}"),
        }
        let auth_route =
            resolve_public_route(&app, "travel.sora", "GET", "/api/auth/me").expect("auth route");
        match auth_route {
            PublicRouteMatch::LocalRead(route_match) => {
                assert_eq!(route_match.handler_name, "auth_me");
            }
            other => panic!("expected local-read auth route, got {other:?}"),
        }
        let saved_search_route =
            resolve_public_route(&app, "travel.sora", "GET", "/api/v1/user/saved-searches")
                .expect("saved searches route");
        match saved_search_route {
            PublicRouteMatch::LocalRead(route_match) => {
                assert_eq!(route_match.handler_name, "saved_searches_get");
            }
            other => panic!("expected local-read saved searches route, got {other:?}"),
        }
    }
    #[tokio::test]
    async fn resolve_public_route_rejects_http_service_when_app_event_expires_lease() {
        use iroha_core::state::World;
        let mut world = World::new();
        let bundle = fixture_hosted_http_inrou_bundle("2026.04.1");
        let service_name = bundle.service.service_name.clone();
        let lease = iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
            status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            quota_class: "taira-open".to_string(),
            deployment_deposit: "1".parse().expect("deployment deposit quantity"),
            prepaid_runtime_balance: "50".parse().expect("prepaid runtime quantity"),
            runtime_price_per_block: "0.00025".parse().expect("runtime price quantity"),
            storage_price_per_gib_block: "0.000025".parse().expect("storage price quantity"),
            egress_price_per_mib: "0.000005".parse().expect("egress price quantity"),
            lease_started_height: 1,
            lease_expires_height: 100,
            reporting_epoch: 1,
            settled_egress_bytes: 0,
            egress_reporter_checkpoints: Vec::new(),
            accounted_egress_bytes: 0,
            last_status_reason: None,
        };
        let lease_volume_states = fixture_service_lease_volume_states(&bundle, Some(&lease));
        insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                service_name,
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
                    fhe_policy_records: BTreeMap::new(),
                    service_lease: Some(lease),
                    lease_volume_states,
                },
            );
        let signer = checked_test_keypair(0x51).public_key().clone();
        world
            .soracloud_app_infra_audit_events_mut_for_testing()
            .insert(
                99,
                iroha_data_model::soracloud::SoraAppInfraAuditEventV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
                    sequence: 99,
                    action: iroha_data_model::soracloud::SoraAppInfraActionV1::Deploy,
                    app_name: "lease_boundary_app".parse().expect("valid app name"),
                    from_version: None,
                    to_version: "1.0.0".to_owned(),
                    app_manifest_hash: Hash::new(b"lease-boundary-app-manifest"),
                    service_count: 1,
                    signer,
                },
            );
        let app = mk_app_state_for_tests_with_world(world);
        assert!(
            resolve_public_route(&app, "portal.sora", "GET", "/app/v1/health").is_none(),
            "an app-infra event at the hosted-lease boundary must fail closed before proxy routing"
        );
    }
    #[tokio::test]
    async fn resolve_public_route_rejects_ledger_only_mailbox_handlers() {
        use iroha_core::state::World;
        let mut world = World::new();
        let bundle = fixture_bundle("2026.02.0");
        let service_name = bundle.service.service_name.clone();
        insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name, fixture_service_deployment(&bundle));
        let app = mk_app_state_for_tests_with_world(world);
        assert!(
            resolve_public_route(&app, "portal.sora", "POST", "/app/update/search").is_none(),
            "update handlers must only execute through ledger-owned mailbox transactions"
        );
        assert!(
            resolve_public_route(&app, "portal.sora", "POST", "/app/private/update/vault")
                .is_none(),
            "private-update handlers must only execute through ledger-owned mailbox transactions"
        );
    }
    #[tokio::test]
    async fn resolve_public_route_prefers_handler_class_for_http_method() {
        use iroha_core::state::World;
        use iroha_data_model::soracloud::SoraMailboxContractV1;
        let mut world = World::new();
        let mut bundle = fixture_bundle("2026.03.0");
        bundle.service.handlers = vec![
            SoraServiceHandlerV1 {
                handler_name: "preferences_get".parse().expect("handler"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_user_preferences".to_owned(),
                route_path: Some("/v1/user/preferences".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "preferences_put".parse().expect("handler"),
                class: SoraServiceHandlerClassV1::PrivateUpdate,
                entrypoint: "store_user_preferences".to_owned(),
                route_path: Some("/v1/user/preferences".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::None,
                mailbox: Some(SoraMailboxContractV1 {
                    queue_name: "private_updates".parse().expect("queue"),
                    max_pending_messages: std::num::NonZeroU32::new(128).expect("pending"),
                    max_message_bytes: std::num::NonZeroU64::new(131_072).expect("bytes"),
                    retention_blocks: std::num::NonZeroU32::new(64).expect("retention"),
                }),
            },
        ];
        let service_name = bundle.service.service_name.clone();
        insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name, fixture_service_deployment(&bundle));
        let app = mk_app_state_for_tests_with_world(world);
        let get_route =
            resolve_public_route(&app, "portal.sora", "GET", "/app/v1/user/preferences")
                .expect("query route");
        match get_route {
            PublicRouteMatch::LocalRead(route_match) => {
                assert_eq!(route_match.handler_name, "preferences_get");
                assert_eq!(route_match.handler_class, SoracloudLocalReadKind::Query);
            }
            other => panic!("expected query route, got {other:?}"),
        }
        assert!(
            resolve_public_route(&app, "portal.sora", "PUT", "/app/v1/user/preferences").is_none(),
            "HTTP method selection must not bypass ledger-owned private-update execution"
        );
    }
    #[test]
    fn authoritative_ciphertext_query_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let binding_name: Name = "patient_records".parse()?;
            let state_key = "/state/health/patient-1".to_string();
            let governance_tx_hash = Hash::new(b"gov-state");
            install_fixture_service(&mut world, &bundle, &service_name);
            world
                .soracloud_service_audit_events_mut_for_testing()
                .insert(
                    1,
                    SoraServiceAuditEventV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                        sequence: 1,
                        block_height: 1,
                        block_timestamp_ms: 1,
                        action: SoraServiceLifecycleActionV1::StateMutation,
                        service_name: service_name.clone(),
                        from_version: None,
                        to_version: bundle.service.service_version.clone(),
                        service_manifest_hash: bundle.service_manifest_hash(),
                        container_manifest_hash: bundle.container_manifest_hash(),
                        process_generation: 1,
                        config_generation: 0,
                        secret_generation: 0,
                        config_snapshot_hash: iroha_data_model::soracloud::derive_soracloud_service_config_snapshot_hash_v1(&BTreeMap::new()),
                        secret_snapshot_hash: iroha_data_model::soracloud::derive_soracloud_service_secret_snapshot_hash_v1(&BTreeMap::new()),
                        governance_tx_hash: Some(governance_tx_hash),
                        binding_name: Some(binding_name.clone()),
                        state_key: Some(state_key.clone()),
                        config_mutations: Vec::new(),
                        secret_mutations: Vec::new(),
                        rollout_state: None,
                        policy_name: None,
                        policy_snapshot_hash: None,
                        jurisdiction_tag: None,
                        consent_evidence_hash: None,
                        break_glass: None,
                        break_glass_reason: None,
                        lease_usage: None,
                        service_lease_commitment: None,
                        lease_reporting_epoch_rollover: None,
                        signer: checked_test_keypair(0x70).public_key().clone(),
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
                        payload: b"ciphertext".to_vec(),
                        payload_bytes: NonZeroU64::new(10).expect("nonzero"),
                        payload_commitment: Hash::new(b"ciphertext"),
                        fhe_public_key_digest: None,
                        fhe_residual_multiple_bound: None,
                        fhe_bound_mode: None,
                        last_update_sequence: 1,
                        governance_tx_hash,
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );
            let query_signer = checked_test_keypair(0x71);
            let app = mk_app_state_for_tests_with_world(world);
            let response = authoritative_ciphertext_query_response(
                &app,
                signed_ciphertext_query_request(fixture_ciphertext_query_spec(), &query_signer),
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let policy = fixture_decryption_authority_policy();
            let policy_snapshot_hash = Hash::new(Encode::encode(&policy));
            install_fixture_service(&mut world, &bundle, &service_name);
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
                            block_height: sequence,
                            block_timestamp_ms: sequence,
                            action: SoraServiceLifecycleActionV1::DecryptionRequest,
                            service_name: service_name.clone(),
                            from_version: None,
                            to_version: bundle.service.service_version.clone(),
                            service_manifest_hash: bundle.service_manifest_hash(),
                            container_manifest_hash: bundle.container_manifest_hash(),
                            process_generation: 1,
                            config_generation: 0,
                            secret_generation: 0,
                            config_snapshot_hash: iroha_data_model::soracloud::derive_soracloud_service_config_snapshot_hash_v1(&BTreeMap::new()),
                            secret_snapshot_hash: iroha_data_model::soracloud::derive_soracloud_service_secret_snapshot_hash_v1(&BTreeMap::new()),
                            governance_tx_hash: Some(Hash::new(Encode::encode(&(
                                "gov-health",
                                sequence,
                            )))),
                            binding_name: Some("patient_records".parse()?),
                            state_key: Some(state_key.to_string()),
                            config_mutations: Vec::new(),
                            secret_mutations: Vec::new(),
                            rollout_state: None,
                            policy_name: Some(policy.policy_name.clone()),
                            policy_snapshot_hash: Some(policy_snapshot_hash),
                            jurisdiction_tag: Some(policy.jurisdiction_tag.clone()),
                            consent_evidence_hash,
                            break_glass: Some(break_glass),
                            break_glass_reason: break_glass
                                .then(|| "emergency override".to_string()),
                            lease_usage: None,
                            service_lease_commitment: None,
                            lease_reporting_epoch_rollover: None,
                            signer: checked_test_keypair(0x70u8.wrapping_add(sequence as u8))
                                .public_key()
                                .clone(),
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
    fn health_compliance_projection_fails_closed_without_authoritative_governance() {
        use iroha_core::state::World;

        let mut world = World::default();
        let bundle = fixture_bundle("1.0.0");
        world
            .soracloud_service_audit_events_mut_for_testing()
            .insert(
            1,
            SoraServiceAuditEventV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence: 1,
                block_height: 1,
                block_timestamp_ms: 1,
                action: SoraServiceLifecycleActionV1::DecryptionRequest,
                service_name: bundle.service.service_name.clone(),
                from_version: None,
                to_version: bundle.service.service_version.clone(),
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                process_generation: 1,
                config_generation: 0,
                secret_generation: 0,
                config_snapshot_hash:
                    iroha_data_model::soracloud::derive_soracloud_service_config_snapshot_hash_v1(
                        &BTreeMap::new(),
                    ),
                secret_snapshot_hash:
                    iroha_data_model::soracloud::derive_soracloud_service_secret_snapshot_hash_v1(
                        &BTreeMap::new(),
                    ),
                governance_tx_hash: None,
                binding_name: Some("patient_records".parse().expect("binding name")),
                state_key: Some("/state/health/patient-1".to_owned()),
                config_mutations: Vec::new(),
                secret_mutations: Vec::new(),
                rollout_state: None,
                policy_name: Some("health_policy".parse().expect("policy name")),
                policy_snapshot_hash: Some(Hash::new(b"health policy")),
                jurisdiction_tag: Some("us_hipaa".to_owned()),
                consent_evidence_hash: None,
                break_glass: Some(false),
                break_glass_reason: None,
                lease_usage: None,
                service_lease_commitment: None,
                lease_reporting_epoch_rollover: None,
                signer: checked_test_keypair(0x74).public_key().clone(),
            },
        );
        let app = mk_app_state_for_tests_with_world(world);
        let error = authoritative_health_compliance_report(&app, None, None, 20)
            .expect_err("missing authoritative governance linkage must fail closed");
        assert_eq!(error.kind, SoracloudErrorKind::Internal);
        assert!(error.message.contains("missing `governance_tx_hash`"));
    }
    #[tokio::test]
    async fn health_compliance_report_rate_limit_keys_transport_remote_when_internal_header_missing()
     {
        let mut app = mk_app_state_for_tests_with_world(Default::default());
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .rate_limiter = crate::limits::RateLimiter::new(Some(1), Some(1));
        let first = handle_health_compliance_report(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([198, 51, 100, 20], 0))),
            NoritoQuery(HealthComplianceReportQuery::default()),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        let second = handle_health_compliance_report(
            State(app),
            HeaderMap::new(),
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([198, 51, 100, 21], 0))),
            NoritoQuery(HealthComplianceReportQuery::default()),
        )
        .await;
        assert_eq!(second.status(), StatusCode::OK);
    }
    #[test]
    fn authoritative_training_job_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            install_fixture_service(&mut world, &bundle, &service_name);
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let config_value = norito::json!({
                "theme": "dark",
                "max_connections": 32
            });
            insert_revision(&mut world, &bundle, service_name.as_ref().to_owned());
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
                    fhe_policy_records: BTreeMap::new(),
                    service_lease: None,
                    lease_volume_states: Vec::new(),
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
    fn authoritative_service_public_discovery_reads_world_state_and_fails_closed_on_substitution()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle("1.0.0");
            let service_name = bundle.service.service_name.clone();
            let discovery = SoracloudPublicServiceDiscoveryV1 {
                schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
                service_name: service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                execution_plane: format!("{:?}", bundle.service.execution_plane),
                runtime: format!("{:?}", bundle.container.runtime),
                route_host: bundle
                    .service
                    .route
                    .as_ref()
                    .expect("fixture route")
                    .host
                    .clone(),
                path_prefix: bundle
                    .service
                    .route
                    .as_ref()
                    .expect("fixture route")
                    .path_prefix
                    .clone(),
                base_url: bundle_base_url(&bundle).expect("fixture base URL"),
                healthcheck_path: bundle.container.lifecycle.healthcheck_path.clone(),
                healthcheck_url: bundle_healthcheck_url(&bundle),
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                deployment_bundle_hash: Hash::new(Encode::encode(&bundle)),
                content_cid: "bafytestpublicdiscovery".to_owned(),
                public_discovery_url:
                    "https://taira.sora.org/sorafs/cid/bafytestpublicdiscovery/index.json"
                        .to_owned(),
                public_discovery_cid_host_url:
                    "https://bafytestpublicdiscovery.sorafs.taira.sora.org/index.json".to_owned(),
                manifest_digest_hex: "de".repeat(32),
                manifest_id_hex: Some("feedface".to_owned()),
            };
            let mut substituted_discovery = discovery.clone();
            substituted_discovery.deployment_bundle_hash = Hash::new(b"substituted bundle");
            let error = validate_public_service_discovery_for_bundle(
                &fixture_service_deployment(&bundle),
                &bundle,
                &substituted_discovery,
            )
            .expect_err("substituted public discovery bundle binding must fail closed");
            assert_eq!(error.kind, SoracloudErrorKind::Internal);
            assert!(error.message.contains("does not bind admitted revision"));
            let historical_bundle = fixture_bundle("0.9.0");
            let mut historical_discovery = discovery.clone();
            historical_discovery.service_version =
                historical_bundle.service.service_version.clone();
            historical_discovery.service_manifest_hash = historical_bundle.service_manifest_hash();
            historical_discovery.container_manifest_hash =
                historical_bundle.container_manifest_hash();
            historical_discovery.deployment_bundle_hash =
                Hash::new(Encode::encode(&historical_bundle));
            historical_discovery.content_cid = "bafytestpublicdiscoveryhistorical".to_owned();
            historical_discovery.public_discovery_url =
                "https://taira.sora.org/sorafs/cid/bafytestpublicdiscoveryhistorical/index.json"
                    .to_owned();
            historical_discovery.public_discovery_cid_host_url =
                "https://bafytestpublicdiscoveryhistorical.sorafs.taira.sora.org/index.json"
                    .to_owned();
            let registry = SoracloudPublicServiceDiscoveryRegistryV1 {
                schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
                service_name: service_name.to_string(),
                current_version: bundle.service.service_version.clone(),
                revisions: BTreeMap::from([
                    (
                        historical_bundle.service.service_version.clone(),
                        historical_discovery,
                    ),
                    (bundle.service.service_version.clone(), discovery.clone()),
                ]),
            };
            let registry_json = Json::new(registry);
            let registry_entry = SoraServiceConfigEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                config_name: PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_string(),
                value_hash: Hash::new(
                    norito::json::to_vec(&registry_json).expect("registry json should encode"),
                ),
                value_json: registry_json,
                last_update_sequence: 21,
            };
            let mut substituted_registry_entry = registry_entry.clone();
            substituted_registry_entry.value_hash = Hash::new(b"substituted registry");
            let error = decode_public_service_discovery_registry(&substituted_registry_entry)
                .expect_err("substituted public discovery registry hash must fail closed");
            assert_eq!(error.kind, SoracloudErrorKind::Internal);
            assert!(error.message.contains("hash does not bind its JSON value"));
            insert_revision(&mut world, &bundle, service_name.as_ref().to_owned());
            insert_revision(
                &mut world,
                &historical_bundle,
                service_name.as_ref().to_owned(),
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
                        revision_count: 2,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 1,
                        secret_generation: 0,
                        service_configs: BTreeMap::from([(
                            PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_string(),
                            registry_entry,
                        )]),
                        service_secrets: BTreeMap::new(),
                        fhe_policy_records: BTreeMap::new(),
                        service_lease: None,
                        lease_volume_states: Vec::new(),
                    },
                );
            world
                .soracloud_service_audit_events_mut_for_testing()
                .insert(1, fixture_service_deploy_audit_event(&bundle));
            let app = mk_app_state_for_tests_with_world(world);
            let response =
                authoritative_service_public_discovery_response(&app, service_name.as_ref(), None)
                    .map_err(|err| {
                        eyre::eyre!("authoritative service public discovery query failed: {err:?}")
                    })?;
            assert_eq!(response.discovery.content_cid, "bafytestpublicdiscovery");
            assert_eq!(
                response.discovery.public_discovery_cid_host_url,
                "https://bafytestpublicdiscovery.sorafs.taira.sora.org/index.json"
            );
            let historical_response = authoritative_service_public_discovery_response(
                &app,
                service_name.as_ref(),
                Some("0.9.0"),
            )
            .map_err(|err| eyre::eyre!("historical public discovery query failed: {err:?}"))?;
            assert_eq!(
                historical_response.discovery.content_cid,
                "bafytestpublicdiscoveryhistorical"
            );
            let snapshot = control_plane_snapshot(&app, Some(service_name.as_ref()), 10)?;
            assert_eq!(snapshot.services.len(), 1);
            assert_eq!(
                snapshot.services[0].public_discovery_content_cid.as_deref(),
                Some("bafytestpublicdiscovery")
            );
            assert_eq!(
                snapshot.services[0]
                    .latest_revision
                    .as_ref()
                    .and_then(|revision| revision.public_discovery_url.as_deref()),
                Some("https://taira.sora.org/sorafs/cid/bafytestpublicdiscovery/index.json")
            );
            Ok(())
        })
    }
    #[test]
    fn authoritative_service_secret_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
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
            insert_revision(&mut world, &bundle, service_name.as_ref().to_owned());
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
                    fhe_policy_records: BTreeMap::new(),
                    service_lease: None,
                    lease_volume_states: Vec::new(),
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            install_fixture_service(&mut world, &bundle, &service_name);
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
                    promoted_by: Some(checked_test_keypair(0x74).public_key().clone()),
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let bundle = fixture_bundle_with_training("1.0.0", true);
            let service_name = bundle.service.service_name.clone();
            install_fixture_service(&mut world, &bundle, &service_name);
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
                    chunk_manifest_root: None,
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let mut world = World::default();
            let service_name: Name = "web_portal".parse().expect("service name");
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
                        runtime_format: SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors,
                        bundle_root,
                        sorafs_manifest_digest:
                            iroha_data_model::sorafs::pin_registry::ManifestDigest::new(
                                [0xA5; 32],
                            ),
                        chunk_count: 1,
                        plaintext_bytes: 16,
                        ciphertext_bytes: 24,
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
                            storage_price: "0.000000001"
                                .parse()
                                .expect("canonical storage price"),
                        },
                        decryption_policy_ref: "policy-1".to_string(),
                    },
                );
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
                    chunk_manifest_root: Some(chunk_manifest_root),
                },
            );
            let app = mk_app_state_for_tests_with_world(world);
            let response =
                authoritative_uploaded_model_status_response(&app, "web_portal", "upload-1", "v1")
                    .map_err(|err| eyre::eyre!("uploaded model status query failed: {err:?}"))?;
            assert_eq!(response.bundle.sorafs_manifest_digest.as_bytes(), &[0xA5; 32]);
            assert_eq!(
                response
                    .artifact
                    .as_ref()
                    .map(|artifact| artifact.artifact_id.as_str()),
                Some("artifact-1")
            );
            let err = authoritative_uploaded_model_status_from_query(
                &app,
                &UploadedModelStatusQuery {
                    service_name: "web_portal".to_string(),
                    weight_version: "v1".to_string(),
                    model_id: Some("upload-1".to_string()),
                    model_name: None,
                    bundle_root: Some(Hash::new(b"wrong-bundle-root")),
                },
            )
            .expect_err("bundle_root mismatch must be rejected");
            assert_eq!(err.status(), StatusCode::CONFLICT);
            assert!(err.message.contains("bundle_root does not match"));
            let err = authoritative_uploaded_model_status_from_query(
                &app,
                &UploadedModelStatusQuery {
                    service_name: "web_portal".to_string(),
                    weight_version: "v1".to_string(),
                    model_id: None,
                    model_name: Some("vision_model".to_string()),
                    bundle_root: Some(Hash::new(b"wrong-bundle-root")),
                },
            )
            .expect_err("model-name status with mismatched bundle_root must be rejected");
            assert_eq!(err.status(), StatusCode::NOT_FOUND);
            assert!(err.message.contains("uploaded model status not found"));
            Ok(())
        })
    }
    #[test]
    fn authoritative_uploaded_model_status_rejects_orphan_user_upload_artifact() {
        use iroha_core::state::World;
        let mut world = World::default();
        let service_name: Name = "web_portal".parse().expect("service name");
        world.soracloud_model_artifacts_mut_for_testing().insert(
            (service_name.as_ref().to_owned(), "artifact-1".to_string()),
            iroha_data_model::soracloud::SoraModelArtifactRecordV1 {
                schema_version: iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                service_name,
                service_version: "1.0.0".to_string(),
                model_name: "vision_model".to_string(),
                artifact_id: "artifact-1".to_string(),
                training_job_id: "artifact-1".to_string(),
                weight_version: Some("v1".to_string()),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::UserUpload,
                    id: "orphan-upload".to_string(),
                }),
                weight_artifact_hash: Hash::new(b"weights"),
                dataset_ref: "hf://repo".to_string(),
                training_config_hash: Hash::new(b"cfg"),
                reproducibility_hash: Hash::new(b"repro"),
                provenance_attestation_hash: Hash::new(b"prov"),
                registered_sequence: 11,
                consumed_by_version: Some("v1".to_string()),
                chunk_manifest_root: Some(Hash::new(b"chunk-manifest-root")),
            },
        );
        let app = mk_app_state_for_tests_with_world(world);
        let err = authoritative_uploaded_model_status_from_query(
            &app,
            &UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: None,
                model_name: Some("vision_model".to_string()),
                bundle_root: None,
            },
        )
        .expect_err("artifact-only user upload status must not be authoritative");
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
        assert!(err.message.contains("orphan-upload"));
    }
    #[test]
    fn authoritative_uploaded_model_status_ignores_non_upload_artifacts() {
        use iroha_core::state::World;
        let payload = sample_uploaded_model_register_payload();
        let service_name = payload.bundle.service_name.clone();
        let mut world = World::default();
        world
            .soracloud_uploaded_model_bundles_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    payload.bundle.model_id.clone(),
                    payload.bundle.weight_version.clone(),
                ),
                payload.bundle.clone(),
            );
        world.soracloud_model_artifacts_mut_for_testing().insert(
            (
                service_name.as_ref().to_owned(),
                "training-artifact".to_string(),
            ),
            iroha_data_model::soracloud::SoraModelArtifactRecordV1 {
                schema_version: iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: "1.0.0".to_string(),
                model_name: payload.model_name.clone(),
                artifact_id: "training-artifact".to_string(),
                training_job_id: payload.bundle.model_id.clone(),
                weight_version: Some(payload.bundle.weight_version.clone()),
                source_provenance: Some(SoraModelProvenanceRefV1 {
                    kind: SoraModelProvenanceKindV1::TrainingJob,
                    id: payload.bundle.model_id.clone(),
                }),
                weight_artifact_hash: Hash::new(b"weights"),
                dataset_ref: "dataset://train".to_string(),
                training_config_hash: Hash::new(b"cfg"),
                reproducibility_hash: Hash::new(b"repro"),
                provenance_attestation_hash: Hash::new(b"prov"),
                registered_sequence: 11,
                consumed_by_version: Some(payload.bundle.weight_version.clone()),
                chunk_manifest_root: None,
            },
        );
        let app = mk_app_state_for_tests_with_world(world);
        let response = authoritative_uploaded_model_status_response(
            &app,
            service_name.as_ref(),
            &payload.bundle.model_id,
            &payload.bundle.weight_version,
        )
        .expect("uploaded bundle status should still resolve");
        assert!(
            response.artifact.is_none(),
            "training-job artifacts must not be projected as uploaded-model artifacts"
        );
    }
    #[test]
    fn authoritative_uploaded_model_status_rejects_malformed_queries() {
        use iroha_core::state::World;
        let app = mk_app_state_for_tests_with_world(World::default());
        let assert_bad_request = |query: UploadedModelStatusQuery, expected: &str| {
            let err = authoritative_uploaded_model_status_from_query(&app, &query)
                .expect_err("malformed uploaded-model status query must fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            assert!(
                err.message.contains(expected),
                "expected `{expected}` in `{}`",
                err.message
            );
        };
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "invalid service_name",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: " web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "service_name must not contain leading or trailing whitespace",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "cafe\u{301}".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "service_name must use canonical NFC form",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1 shadow".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "weight_version must not contain whitespace",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1 ".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "weight_version must not contain whitespace",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some("upload 1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "job_id must not contain whitespace",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some(" upload-1".to_string()),
                model_name: None,
                bundle_root: None,
            },
            "job_id must not contain whitespace",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: None,
                model_name: Some("vision model".to_string()),
                bundle_root: None,
            },
            "invalid model_name",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: None,
                model_name: Some("cafe\u{301}".to_string()),
                bundle_root: None,
            },
            "model_name must use canonical NFC form",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: None,
                model_name: None,
                bundle_root: None,
            },
            "model_id or model_name must be provided",
        );
        assert_bad_request(
            UploadedModelStatusQuery {
                service_name: "web_portal".to_string(),
                weight_version: "v1".to_string(),
                model_id: Some("upload-1".to_string()),
                model_name: Some("vision_model".to_string()),
                bundle_root: None,
            },
            "exactly one of model_id or model_name",
        );
    }
    fn signed_fhe_job_run_request(
        payload: FheJobRunPayload,
        key_pair: &KeyPair,
    ) -> SignedFheJobRunRequest {
        let encoded =
            encode_fhe_job_run_signature_payload(&payload).expect("encode fhe job run payload");
        let signature = checked_test_signature(key_pair.private_key(), &encoded);
        SignedFheJobRunRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }
    fn sample_uploaded_model_register_payload() -> UploadedModelRegisterPayload {
        let public_key_bytes = vec![7u8; 32];
        let wrapped_key_ciphertext = vec![10u8; 48];
        UploadedModelRegisterPayload {
            bundle: SoraUploadedModelBundleV1 {
                schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
                service_name: "web_portal".parse().expect("service name"),
                model_id: "upload-1".to_string(),
                weight_version: "v1".to_string(),
                family: "decoder-only".to_string(),
                modalities: vec!["text".to_string()],
                plaintext_root: Hash::new(b"plaintext-root"),
                runtime_format: SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors,
                bundle_root: Hash::new(b"bundle-root"),
                sorafs_manifest_digest:
                    iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xA5; 32]),
                chunk_count: 1,
                plaintext_bytes: 16,
                ciphertext_bytes: 24,
                chunk_manifest_root: Hash::new(b"chunk-manifest-root"),
                upload_recipient: iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                    schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                    key_id: "soracloud-upload".to_string(),
                    key_version: NonZeroU32::new(1).expect("non-zero key version"),
                    kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                    aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                    public_key_bytes: public_key_bytes.clone(),
                    public_key_fingerprint: Hash::new(public_key_bytes),
                },
                wrapped_bundle_key: iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                    schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                    recipient_key_id: "soracloud-upload".to_string(),
                    recipient_key_version: NonZeroU32::new(1).expect("non-zero key version"),
                    kem: iroha_data_model::soracloud::SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                    aead: iroha_data_model::soracloud::SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                    ephemeral_public_key: vec![8u8; 32],
                    nonce: vec![9u8; 12],
                    wrapped_key_ciphertext: wrapped_key_ciphertext.clone(),
                    ciphertext_hash: Hash::new(wrapped_key_ciphertext),
                    aad_digest: Hash::new(b"wrapped-aad"),
                },
                pricing_policy: SoraUploadedModelPricingPolicyV1 {
                    storage_price: "0.000000001"
                        .parse()
                        .expect("canonical storage price"),
                },
                decryption_policy_ref: "policy-1".to_string(),
            },
            model_name: "vision_model".to_string(),
            artifact_id: "artifact-1".to_string(),
            weight_artifact_hash: Hash::new(b"weights"),
            dataset_ref: "dataset://upload".to_string(),
            training_config_hash: Hash::new(b"cfg"),
            reproducibility_hash: Hash::new(b"repro"),
            provenance_attestation_hash: Hash::new(b"prov"),
        }
    }
    fn sample_uploaded_model_pin_record(
        digest: ManifestDigest,
        content_length: u64,
        status: PinStatus,
    ) -> PinManifestRecord {
        let manifest: sorafs_manifest::ManifestV1 = norito::decode_from_bytes(include_bytes!(
            "../../../fixtures/sorafs_gateway/1.0.0/manifest_v1.to"
        ))
        .expect("decode canonical SoraFS fixture manifest");
        let root_cid = ManifestRootCid::try_from_slice(&manifest.root_cid)
            .expect("fixture manifest root CID must be canonical");
        let policy = PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Warm,
            retention_epoch: u64::MAX,
        };
        let mut record = PinManifestRecord::new(
            digest,
            root_cid,
            ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_string(),
                name: "sf1".to_string(),
                semver: "1.0.0".to_string(),
                multihash_code: 0x1e,
            },
            manifest.chunk_digest_sha3_256,
            manifest.por_root,
            content_length,
            policy,
            ALICE_ID.clone(),
            1,
            None,
            None,
            Metadata::default(),
        );
        match status {
            PinStatus::Pending => {}
            PinStatus::Approved(epoch) => record.approve(epoch, None),
            PinStatus::Retired(epoch) => record.retire(epoch, None),
        }
        record
    }
    fn insert_uploaded_model_finalization_projection(
        world: &mut iroha_core::state::World,
        payload: &UploadedModelRegisterPayload,
        service_version: &str,
    ) {
        let source_provenance = Some(SoraModelProvenanceRefV1 {
            kind: SoraModelProvenanceKindV1::UserUpload,
            id: payload.bundle.model_id.clone(),
        });
        let weight = SoraModelWeightVersionRecordV1 {
            schema_version:
                iroha_data_model::soracloud::SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
            service_name: payload.bundle.service_name.clone(),
            service_version: service_version.to_owned(),
            model_name: payload.model_name.clone(),
            weight_version: payload.bundle.weight_version.clone(),
            parent_version: None,
            training_job_id: String::new(),
            source_provenance: source_provenance.clone(),
            weight_artifact_hash: payload.weight_artifact_hash,
            dataset_ref: payload.dataset_ref.clone(),
            training_config_hash: payload.training_config_hash,
            reproducibility_hash: payload.reproducibility_hash,
            provenance_attestation_hash: payload.provenance_attestation_hash,
            registered_sequence: 7,
            promoted_sequence: None,
            gate_report_hash: None,
            promoted_by: None,
        };
        weight
            .validate()
            .expect("valid uploaded-model weight fixture");
        world
            .soracloud_model_weight_versions_mut_for_testing()
            .insert(
                (
                    payload.bundle.service_name.as_ref().to_owned(),
                    payload.model_name.clone(),
                    payload.bundle.weight_version.clone(),
                ),
                weight,
            );
        let artifact = SoraModelArtifactRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
            service_name: payload.bundle.service_name.clone(),
            service_version: service_version.to_owned(),
            model_name: payload.model_name.clone(),
            artifact_id: payload.artifact_id.clone(),
            training_job_id: payload.artifact_id.clone(),
            weight_version: Some(payload.bundle.weight_version.clone()),
            source_provenance,
            weight_artifact_hash: payload.weight_artifact_hash,
            dataset_ref: payload.dataset_ref.clone(),
            training_config_hash: payload.training_config_hash,
            reproducibility_hash: payload.reproducibility_hash,
            provenance_attestation_hash: payload.provenance_attestation_hash,
            registered_sequence: 8,
            consumed_by_version: Some(payload.bundle.weight_version.clone()),
            chunk_manifest_root: Some(payload.bundle.chunk_manifest_root),
        };
        artifact
            .validate()
            .expect("valid uploaded-model artifact fixture");
        world.soracloud_model_artifacts_mut_for_testing().insert(
            (
                payload.bundle.service_name.as_ref().to_owned(),
                payload.artifact_id.clone(),
            ),
            artifact,
        );
    }
    fn signed_uploaded_model_register_request(
        payload: UploadedModelRegisterPayload,
        key_pair: &KeyPair,
    ) -> SignedUploadedModelRegisterRequest {
        let bundle_encoded = encode_uploaded_model_register_bundle_signature_payload(&payload)
            .expect("encode uploaded model bundle payload");
        let finalize_encoded = encode_uploaded_model_register_finalize_signature_payload(&payload)
            .expect("encode uploaded model finalize payload");
        SignedUploadedModelRegisterRequest {
            payload,
            bundle_provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature: checked_test_signature(key_pair.private_key(), &bundle_encoded),
            },
            finalize_provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature: checked_test_signature(key_pair.private_key(), &finalize_encoded),
            },
        }
    }
    #[test]
    fn uploaded_model_register_pin_gate_rejects_missing_inactive_or_mismatched_pin() {
        use iroha_core::state::World;
        let payload = sample_uploaded_model_register_payload();
        let digest = payload.bundle.sorafs_manifest_digest;
        let missing_app = mk_app_state_for_tests_with_world(World::default());
        let missing_err = require_active_sorafs_uploaded_model_pin(&missing_app, &payload.bundle)
            .expect_err("missing pin must fail before upload registration");
        assert_eq!(missing_err.status(), StatusCode::CONFLICT);
        assert!(missing_err.message.contains("is not registered"));
        for (status, expected_message) in [
            (PinStatus::Pending, "is not approved"),
            (PinStatus::Retired(3), "retired at epoch"),
        ] {
            let record =
                sample_uploaded_model_pin_record(digest, payload.bundle.ciphertext_bytes, status);
            let err = require_active_sorafs_uploaded_model_pin_record(&record, &payload.bundle)
                .expect_err("inactive pin must fail before upload registration");
            assert_eq!(err.status(), StatusCode::CONFLICT);
            assert!(err.message.contains(expected_message));
        }
        let digest_mismatch_record = sample_uploaded_model_pin_record(
            ManifestDigest::new([0xB6; 32]),
            payload.bundle.ciphertext_bytes,
            PinStatus::Approved(1),
        );
        let digest_mismatch_err = require_active_sorafs_uploaded_model_pin_record(
            &digest_mismatch_record,
            &payload.bundle,
        )
        .expect_err("pin digest mismatch must fail before upload registration");
        assert_eq!(digest_mismatch_err.status(), StatusCode::CONFLICT);
        assert!(digest_mismatch_err.message.contains("record digest"));
        let length_mismatch_record = sample_uploaded_model_pin_record(
            digest,
            payload.bundle.ciphertext_bytes + 1,
            PinStatus::Approved(1),
        );
        let length_mismatch_err = require_active_sorafs_uploaded_model_pin_record(
            &length_mismatch_record,
            &payload.bundle,
        )
        .expect_err("pin length mismatch must fail before upload registration");
        assert_eq!(length_mismatch_err.status(), StatusCode::CONFLICT);
        assert!(length_mismatch_err.message.contains("content_length"));
    }
    #[test]
    fn uploaded_model_finalization_gate_requires_exact_weight_artifact_linkage() {
        use iroha_core::state::World;
        let payload = sample_uploaded_model_register_payload();
        let mut exact_world = World::default();
        insert_uploaded_model_finalization_projection(&mut exact_world, &payload, "1.0.0");
        let exact_app = mk_app_state_for_tests_with_world(exact_world);
        require_finalized_uploaded_model_release(&exact_app, &payload.bundle)
            .expect("exact finalization projection must pass preflight");

        let mut mismatched_world = World::default();
        insert_uploaded_model_finalization_projection(&mut mismatched_world, &payload, "1.0.0");
        let artifact_key = (
            payload.bundle.service_name.as_ref().to_owned(),
            payload.artifact_id.clone(),
        );
        let mut mismatched_artifact = mismatched_world
            .soracloud_model_artifacts_mut_for_testing()
            .view()
            .get(&artifact_key)
            .cloned()
            .expect("uploaded-model artifact fixture");
        mismatched_artifact.dataset_ref = "dataset://different".to_string();
        mismatched_world
            .soracloud_model_artifacts_mut_for_testing()
            .insert(artifact_key, mismatched_artifact);
        let mismatched_app = mk_app_state_for_tests_with_world(mismatched_world);
        let error = require_finalized_uploaded_model_release(&mismatched_app, &payload.bundle)
            .expect_err("artifact and weight provenance must match exactly");
        assert_eq!(error.status(), StatusCode::CONFLICT);
        assert!(error.message.contains("does not exactly match"));
    }
    #[test]
    fn private_uploaded_model_execute_rejects_registered_but_unfinalized_bundle() {
        use iroha_core::state::World;
        let mut payload = sample_uploaded_model_register_payload();
        payload.bundle.runtime_format =
            SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1;
        let service_name = payload.bundle.service_name.clone();
        let mut service_revision = fixture_bundle("1.0.0");
        service_revision
            .container
            .capabilities
            .allow_model_inference = true;
        service_revision.service.container.manifest_hash =
            service_revision.container_manifest_hash();
        let input_artifact = SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([0xC1; 32]),
            sorafs_root_cid: sample_uploaded_model_pin_record(
                ManifestDigest::new([0xC1; 32]),
                64,
                PinStatus::Approved(1),
            )
            .root_cid,
            artifact_hash: Hash::new(b"encrypted-input"),
            ciphertext_bytes: 64,
            artifact_role: "input".to_string(),
        };
        let mut world = World::default();
        insert_revision(
            &mut world,
            &service_revision,
            service_name.as_ref().to_owned(),
        );
        world
            .soracloud_uploaded_model_bundles_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    payload.bundle.model_id.clone(),
                    payload.bundle.weight_version.clone(),
                ),
                payload.bundle.clone(),
            );
        world.pin_manifests_mut_for_testing().insert(
            payload.bundle.sorafs_manifest_digest,
            sample_uploaded_model_pin_record(
                payload.bundle.sorafs_manifest_digest,
                payload.bundle.ciphertext_bytes,
                PinStatus::Approved(1),
            ),
        );
        world.pin_manifests_mut_for_testing().insert(
            input_artifact.sorafs_manifest_digest,
            sample_uploaded_model_pin_record(
                input_artifact.sorafs_manifest_digest,
                input_artifact.ciphertext_bytes,
                PinStatus::Approved(1),
            ),
        );
        let app = mk_app_state_for_tests_with_world(world);
        let error = authoritative_private_uploaded_model_execute_response(
            &app,
            PrivateUploadedModelExecuteRequest {
                service_name: service_name.to_string(),
                service_version: service_revision.service.service_version.clone(),
                weight_version: "v1".to_string(),
                model_id: "upload-1".to_string(),
                bundle_root: payload.bundle.bundle_root,
                decryption_request_id: "missing-decryption-release".to_string(),
                input_artifact: input_artifact.clone(),
                output_recipient: payload.bundle.upload_recipient.clone(),
            },
            &[],
        )
        .expect_err("private execution of a registered-only model must fail closed");
        assert_eq!(error.status(), StatusCode::CONFLICT);
        assert!(error.message.contains("has not been finalized"));
    }
    #[test]
    fn private_uploaded_model_execute_binds_committed_decryption_request() {
        use iroha_core::state::World;
        let mut payload = sample_uploaded_model_register_payload();
        payload.bundle.runtime_format =
            SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1;
        let policy = fixture_decryption_authority_policy();
        payload.bundle.decryption_policy_ref = policy.policy_name.to_string();
        let service_name = payload.bundle.service_name.clone();
        let mut service_revision = fixture_bundle("1.0.0");
        service_revision
            .container
            .capabilities
            .allow_model_inference = true;
        service_revision.service.container.manifest_hash =
            service_revision.container_manifest_hash();
        let input_artifact = SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([0xD1; 32]),
            sorafs_root_cid: sample_uploaded_model_pin_record(
                ManifestDigest::new([0xD1; 32]),
                64,
                PinStatus::Approved(1),
            )
            .root_cid,
            artifact_hash: Hash::new(b"encrypted-input-release"),
            ciphertext_bytes: 64,
            artifact_role: "input".to_string(),
        };
        let mut decryption_request = fixture_decryption_request();
        decryption_request.request_id = "decrypt-upload-input".to_string();
        decryption_request.ciphertext_commitment = input_artifact.artifact_hash;
        let signer = checked_test_keypair(0x80);
        let record = SoraDecryptionRequestRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_revision.service.service_version.clone(),
            policy: policy.clone(),
            request: decryption_request.clone(),
            sequence: 11,
            signer: signer.public_key().clone(),
        };
        record
            .validate()
            .expect("fixture decryption record should validate");
        let mut world = World::default();
        insert_revision(
            &mut world,
            &service_revision,
            service_name.as_ref().to_owned(),
        );
        world
            .soracloud_uploaded_model_bundles_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    payload.bundle.model_id.clone(),
                    payload.bundle.weight_version.clone(),
                ),
                payload.bundle.clone(),
            );
        insert_uploaded_model_finalization_projection(
            &mut world,
            &payload,
            &service_revision.service.service_version,
        );
        world.pin_manifests_mut_for_testing().insert(
            payload.bundle.sorafs_manifest_digest,
            sample_uploaded_model_pin_record(
                payload.bundle.sorafs_manifest_digest,
                payload.bundle.ciphertext_bytes,
                PinStatus::Approved(1),
            ),
        );
        world.pin_manifests_mut_for_testing().insert(
            input_artifact.sorafs_manifest_digest,
            sample_uploaded_model_pin_record(
                input_artifact.sorafs_manifest_digest,
                input_artifact.ciphertext_bytes,
                PinStatus::Approved(1),
            ),
        );
        world
            .soracloud_decryption_request_records_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    decryption_request.request_id.clone(),
                ),
                record.clone(),
            );
        world
            .soracloud_service_audit_events_mut_for_testing()
            .insert(
                record.sequence,
                fixture_private_decryption_audit_event(&service_revision, &record),
            );
        let app = mk_app_state_for_tests_with_world(world);
        let make_request =
            |input_artifact: SoraPrivateModelArtifactRefV1| PrivateUploadedModelExecuteRequest {
                service_name: service_name.to_string(),
                service_version: service_revision.service.service_version.clone(),
                weight_version: payload.bundle.weight_version.clone(),
                model_id: payload.bundle.model_id.clone(),
                bundle_root: payload.bundle.bundle_root,
                decryption_request_id: decryption_request.request_id.clone(),
                input_artifact,
                output_recipient: payload.bundle.upload_recipient.clone(),
            };
        let unauthorized_signer = checked_test_keypair(0x81);
        let unauthorized = authoritative_private_uploaded_model_execute_response(
            &app,
            make_request(input_artifact.clone()),
            std::slice::from_ref(unauthorized_signer.public_key()),
        )
        .expect_err("a different signed account must not consume a committed private release");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert!(
            unauthorized.message.contains("exact signer"),
            "unexpected authorization error: {}",
            unauthorized.message
        );
        let unavailable = authoritative_private_uploaded_model_execute_response(
            &app,
            make_request(input_artifact.clone()),
            std::slice::from_ref(signer.public_key()),
        )
        .expect_err("matching release still requires a qualified private runtime");
        assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            unavailable
                .message
                .contains("qualified Soracloud private runtime")
        );
        let mut mismatched_input = input_artifact.clone();
        mismatched_input.artifact_hash = Hash::new(b"different-encrypted-input");
        let mismatch = authoritative_private_uploaded_model_execute_response(
            &app,
            make_request(mismatched_input),
            std::slice::from_ref(signer.public_key()),
        )
        .expect_err("ciphertext commitment mismatch must fail closed");
        assert_eq!(mismatch.status(), StatusCode::CONFLICT);
        assert!(
            mismatch.message.contains("ciphertext commitment"),
            "unexpected error: {}",
            mismatch.message
        );
    }
    #[test]
    fn committed_private_uploaded_model_execute_replay_rejects_wrong_signer_before_cleanup() {
        use iroha_core::state::World;

        let mut payload = sample_uploaded_model_register_payload();
        payload.bundle.runtime_format =
            SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1;
        let policy = fixture_decryption_authority_policy();
        payload.bundle.decryption_policy_ref = policy.policy_name.to_string();
        let service_name = payload.bundle.service_name.clone();
        let input_artifact = SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([0xD2; 32]),
            sorafs_root_cid: ManifestRootCid::from_blake3_digest([0xD2; 32])
                .expect("fixture input root CID"),
            artifact_hash: Hash::new(b"committed-replay-encrypted-input"),
            ciphertext_bytes: 64,
            artifact_role: "input".to_owned(),
        };
        let mut decryption_request = fixture_decryption_request();
        decryption_request.request_id = "decrypt-committed-replay".to_owned();
        decryption_request.ciphertext_commitment = input_artifact.artifact_hash;
        let release_signer = checked_test_keypair(0x82);
        let record = SoraDecryptionRequestRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
            service_name: service_name.clone(),
            service_version: "1.0.0".to_owned(),
            policy,
            request: decryption_request.clone(),
            sequence: 11,
            signer: release_signer.public_key().clone(),
        };
        record
            .validate()
            .expect("fixture decryption record should validate");

        let request = PrivateUploadedModelExecuteRequest {
            service_name: service_name.to_string(),
            service_version: record.service_version.clone(),
            weight_version: payload.bundle.weight_version.clone(),
            model_id: payload.bundle.model_id.clone(),
            bundle_root: payload.bundle.bundle_root,
            decryption_request_id: decryption_request.request_id.clone(),
            input_artifact: input_artifact.clone(),
            output_recipient: payload.bundle.upload_recipient.clone(),
        };
        let (_, mut receipt) = sample_private_uploaded_model_receipt_for_pagination(
            crate::signed_query_test_network_id(),
            12,
            0x92,
            service_name.as_ref(),
            &payload.bundle.model_id,
            &payload.bundle.weight_version,
        );
        receipt.service_version = request.service_version.clone();
        receipt.model_manifest_digest = payload.bundle.sorafs_manifest_digest;
        receipt.model_bundle_root = payload.bundle.bundle_root;
        receipt.policy_id = payload.bundle.decryption_policy_ref.clone();
        receipt.decryption_request_id = request.decryption_request_id.clone();
        receipt.input_artifact = input_artifact;
        receipt.output_recipient = request.output_recipient.clone();
        receipt.request_commitment = derive_soracloud_private_model_request_commitment_v1(&receipt);
        receipt.result_commitment = derive_soracloud_private_model_result_commitment_v1(&receipt);
        receipt.receipt_id =
            derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&receipt);
        receipt
            .validate()
            .expect("committed private execution receipt should validate");

        let mut world = World::default();
        world
            .soracloud_uploaded_model_bundles_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    payload.bundle.model_id.clone(),
                    payload.bundle.weight_version.clone(),
                ),
                payload.bundle,
            );
        world
            .soracloud_decryption_request_records_mut_for_testing()
            .insert(
                (
                    service_name.as_ref().to_owned(),
                    decryption_request.request_id,
                ),
                record,
            );
        world
            .soracloud_private_uploaded_model_execution_receipts_mut_for_testing()
            .insert(receipt.receipt_id, receipt.clone());
        let app = mk_app_state_for_tests_with_world(world);
        let submission_key = (
            request.service_name.clone(),
            request.decryption_request_id.clone(),
        );
        app.soracloud_private_execution_submissions
            .entries
            .lock()
            .insert(
                submission_key.clone(),
                PrivateExecutionSubmissionState::Executing {
                    request_fingerprint: Hash::new(request.encode()),
                },
            );

        let wrong_signer = checked_test_keypair(0x83);
        let unauthorized = authoritative_private_uploaded_model_execute_response(
            &app,
            request.clone(),
            std::slice::from_ref(wrong_signer.public_key()),
        )
        .expect_err("a different signer must not replay a committed private execution");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert!(unauthorized.message.contains("exact signer"));
        assert!(
            app.soracloud_private_execution_submissions
                .entries
                .lock()
                .contains_key(&submission_key),
            "unauthorized replay must not clean up the committed submission state"
        );

        let (status, response) = authoritative_private_uploaded_model_execute_response(
            &app,
            request,
            std::slice::from_ref(release_signer.public_key()),
        )
        .expect("the exact release signer may replay committed output");
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            response.submission_phase,
            PrivateUploadedModelSubmissionPhaseV1::Committed
        );
        assert_eq!(response.receipt.receipt_id, receipt.receipt_id);
        assert!(
            !app.soracloud_private_execution_submissions
                .entries
                .lock()
                .contains_key(&submission_key),
            "authorized replay should clean up the committed submission state"
        );
    }
    #[test]
    fn uploaded_model_register_signature_rejects_signer_mismatch() {
        let key_pair = checked_test_keypair(0x81);
        let other_key_pair = checked_test_keypair(0x82);
        let payload = sample_uploaded_model_register_payload();
        let finalize_encoded = encode_uploaded_model_register_finalize_signature_payload(&payload)
            .expect("encode uploaded model finalize payload");
        let mut request = signed_uploaded_model_register_request(payload, &key_pair);
        request.finalize_provenance = ManifestProvenance {
            signer: other_key_pair.public_key().clone(),
            signature: checked_test_signature(other_key_pair.private_key(), &finalize_encoded),
        };
        let err = verify_uploaded_model_register_signature(&request)
            .expect_err("mismatched upload register signers must fail");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        assert!(err.message.contains("signers must match"));
    }
    #[test]
    fn uploaded_model_register_signature_rejects_swapped_same_signer_provenances() {
        let key_pair = checked_test_keypair(0x83);
        let payload = sample_uploaded_model_register_payload();
        let bundle_encoded = encode_uploaded_model_register_bundle_signature_payload(&payload)
            .expect("encode uploaded model bundle payload");
        let finalize_encoded = encode_uploaded_model_register_finalize_signature_payload(&payload)
            .expect("encode uploaded model finalize payload");
        let request = SignedUploadedModelRegisterRequest {
            payload,
            bundle_provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature: checked_test_signature(key_pair.private_key(), &finalize_encoded),
            },
            finalize_provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature: checked_test_signature(key_pair.private_key(), &bundle_encoded),
            },
        };
        let err = verify_uploaded_model_register_signature(&request)
            .expect_err("swapped upload register provenances must fail");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        assert!(
            err.message
                .contains("bundle provenance signature verification failed")
        );
    }
    #[test]
    fn uploaded_model_register_signature_rejects_tampered_bundle_payload() {
        let key_pair = checked_test_keypair(0x84);
        let payload = sample_uploaded_model_register_payload();
        let mut request = signed_uploaded_model_register_request(payload, &key_pair);
        request.payload.bundle.model_id = "upload-replayed".to_string();
        let err = verify_uploaded_model_register_signature(&request)
            .expect_err("tampered upload bundle payload must fail");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        assert!(
            err.message
                .contains("bundle provenance signature verification failed")
        );
    }
    #[test]
    fn uploaded_model_register_signature_rejects_tampered_finalize_payload() {
        let key_pair = checked_test_keypair(0x85);
        let payload = sample_uploaded_model_register_payload();
        let mut request = signed_uploaded_model_register_request(payload, &key_pair);
        request.payload.dataset_ref = "dataset://replayed".to_string();
        let err = verify_uploaded_model_register_signature(&request)
            .expect_err("tampered upload finalize payload must fail");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        assert!(
            err.message
                .contains("finalize provenance signature verification failed")
        );
    }
    fn signed_ciphertext_query_request(
        query: CiphertextQuerySpecV1,
        key_pair: &KeyPair,
    ) -> SignedCiphertextQueryRequest {
        let encoded = encode_ciphertext_query_signature_payload(&query)
            .expect("encode ciphertext query payload");
        let signature = checked_test_signature(key_pair.private_key(), &encoded);
        SignedCiphertextQueryRequest {
            query,
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
    #[test]
    fn bundle_signature_payload_layout_is_canonical_layout() {
        let bundle = fixture_bundle("1.0.0");
        let precondition = SoraServiceMutationPreconditionV1::ServiceAbsent;
        let encoded = encode_bundle_signature_payload(
            &bundle,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &precondition,
        )
        .expect("encode signature payload");
        let expected =
            iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
                &bundle,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &precondition,
            )
            .expect("encode canonical layout");
        assert_eq!(encoded, expected);
        assert_ne!(
            encoded,
            norito::to_bytes(&bundle).expect("encode legacy layout"),
            "bundle signatures must commit to bundle, inline materials, and the exact mutation precondition",
        );
        let different_precondition = SoraServiceMutationPreconditionV1::ExactCurrentRevision(
            SoraServiceExactCurrentRevisionPreconditionV1 {
                service_version: "0.9.0".to_owned(),
                service_manifest_hash: bundle.service_manifest_hash(),
                container_manifest_hash: bundle.container_manifest_hash(),
                process_generation: 1,
                config_generation: 0,
                secret_generation: 0,
            },
        );
        assert_ne!(
            encoded,
            encode_bundle_signature_payload(
                &bundle,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &different_precondition,
            )
            .expect("encode changed precondition"),
            "changing only the mutation precondition must change the signed payload",
        );
    }
    #[test]
    fn state_mutation_signature_payload_layout_is_canonical_layout() {
        let governance_tx_hash = Hash::new(b"governance");
        let payload = StateMutationRequest {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            key: "/state/private/records/1".to_owned(),
            operation: StateMutationOperation::Upsert,
            value_size_bytes: Some(3),
            value_payload_hex: Some("010203".to_string()),
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: None,
        };
        let payload_commitment = Hash::new([1_u8, 2, 3]);
        let encoded =
            encode_state_mutation_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            payload.key.as_str(),
            "upsert",
            payload.value_size_bytes,
            Some(payload_commitment),
            payload.encryption,
            governance_tx_hash,
            None::<SoracloudFheInputAdmissionProofV1>,
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
            value_payload_hex: None,
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            governance_tx_hash,
            fhe_input_admission_proof: None,
        };
        let encoded =
            encode_state_mutation_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            payload.key.as_str(),
            "delete",
            None::<u64>,
            None::<Hash>,
            payload.encryption,
            governance_tx_hash,
            None::<SoracloudFheInputAdmissionProofV1>,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }
    #[test]
    fn fhe_job_run_signature_payload_binds_exact_governed_reference() {
        let job = fixture_fhe_job_spec();
        let policy_reference = fixture_fhe_policy_reference();
        let payload = FheJobRunPayload {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            job: job.clone(),
            policy_reference: policy_reference.clone(),
            public_key_proof: None,
            bootstrap_key_zero_refresh_proof: None,
            full_bootstrap_execution_proofs: Vec::new(),
        };
        let encoded =
            encode_fhe_job_run_signature_payload(&payload).expect("encode signature payload");
        let expected = encode_fhe_job_run_provenance_payload(
            payload.service_name.as_str(),
            payload.binding_name.as_str(),
            job,
            policy_reference,
            None,
            None,
            Vec::new(),
        )
        .expect("encode canonical payload");
        assert_eq!(encoded, expected);
    }
    #[test]
    fn fhe_job_and_decryption_payloads_are_closed_and_require_explicit_proof_keys() {
        let job = fixture_fhe_job_spec();
        let policy_reference = fixture_fhe_policy_reference();
        let canonical = norito::json!({
            "service_name": "health_portal",
            "binding_name": "private_state",
            "job": (norito::json::to_value(&job).expect("serialize FHE job spec")),
            "policy_reference": (norito::json::to_value(&policy_reference)
                .expect("serialize FHE policy reference")),
            "public_key_proof": null,
            "bootstrap_key_zero_refresh_proof": null,
            "full_bootstrap_execution_proofs": [],
        });
        norito::json::from_value::<FheJobRunPayload>(canonical.clone())
            .expect("canonical FHE job payload must decode");
        for field in [
            "public_key_proof",
            "bootstrap_key_zero_refresh_proof",
            "full_bootstrap_execution_proofs",
        ] {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("FHE job payload object")
                .remove(field);
            norito::json::from_value::<FheJobRunPayload>(missing)
                .expect_err("an omitted FHE proof key must fail");
        }
        let mut null_proof_list = canonical.clone();
        null_proof_list
            .as_object_mut()
            .expect("FHE job payload object")
            .insert(
                "full_bootstrap_execution_proofs".to_owned(),
                norito::json::Value::Null,
            );
        norito::json::from_value::<FheJobRunPayload>(null_proof_list)
            .expect_err("a null FHE execution-proof list must fail");
        let mut unknown_job = canonical;
        unknown_job
            .as_object_mut()
            .expect("FHE job payload object")
            .insert("legacy_proof".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<FheJobRunPayload>(unknown_job)
            .expect_err("an unknown FHE job payload key must fail");

        let mut unknown_decryption = norito::json!({
            "service_name": "health_portal",
            "policy": (norito::json::to_value(&fixture_decryption_authority_policy())
                .expect("serialize decryption policy")),
            "request": (norito::json::to_value(&fixture_decryption_request())
                .expect("serialize decryption request")),
        });
        norito::json::from_value::<DecryptionRequestPayload>(unknown_decryption.clone())
            .expect("canonical decryption payload must decode");
        unknown_decryption
            .as_object_mut()
            .expect("decryption payload object")
            .insert("legacy_request".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<DecryptionRequestPayload>(unknown_decryption)
            .expect_err("an unknown decryption payload key must fail");
    }
    #[test]
    fn fhe_job_run_signature_rejects_policy_reference_substitution() {
        let payload = FheJobRunPayload {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            job: fixture_fhe_job_spec(),
            policy_reference: fixture_fhe_policy_reference(),
            public_key_proof: None,
            bootstrap_key_zero_refresh_proof: None,
            full_bootstrap_execution_proofs: Vec::new(),
        };
        let key_pair = checked_test_keypair(0x90);
        let mut request = signed_fhe_job_run_request(payload, &key_pair);
        request.payload.policy_reference.material_digest =
            Hash::new(b"different-governed-fhe-material");
        let err = verify_fhe_job_run_signature(&request)
            .expect_err("signed jobs must bind the exact governed material digest");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }
    #[test]
    fn fhe_job_run_preflight_validates_reference_and_attachment_syntax_only() {
        let mut payload = FheJobRunPayload {
            service_name: "health_portal".to_owned(),
            binding_name: "private_state".to_owned(),
            job: fixture_fhe_job_spec(),
            policy_reference: fixture_fhe_policy_reference(),
            public_key_proof: None,
            bootstrap_key_zero_refresh_proof: None,
            full_bootstrap_execution_proofs: Vec::new(),
        };
        validate_fhe_job_run_proof_attachments(&payload)
            .expect("state-bound execution material is resolved by the instruction executor");
        payload.policy_reference.schema_version = 0;
        let err = validate_fhe_job_run_proof_attachments(&payload)
            .expect_err("unsupported governed reference versions must fail at Torii");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
        assert!(err.message.contains("invalid policy_reference"));
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
            target_version: "1.0.0".to_owned(),
        };
        let encoded =
            encode_rollback_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.target_version.as_str(),
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
            lease_blocks: 120,
            autonomy_budget_units: 500,
        };
        let encoded =
            encode_agent_deploy_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(manifest, 120u64, 500u64)).expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }
    #[test]
    fn agent_lease_renew_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentLeaseRenewPayload {
            apartment_name: "ops_agent".to_owned(),
            lease_blocks: 120,
        };
        let encoded =
            encode_agent_lease_renew_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(payload.apartment_name.as_str(), payload.lease_blocks))
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
            amount: "0.001".parse().expect("canonical quantity"),
        };
        let encoded = encode_agent_wallet_spend_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.asset_definition.as_str(),
            payload.amount.clone(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }
    // Exact-quantity boundary coverage is kept in an included child so this
    // production route module remains within the repository source budget.
    include!("soracloud/wallet_quantity_tests.rs");
    include!("soracloud/control_plane_lease_tests.rs");
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
            revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            model_name: "gpt_oss_20b".to_owned(),
            service_name: "vision_portal".to_owned(),
            apartment_name: Some("ops_agent".to_owned()),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            lease_asset_definition_id: hf_shared_lease_asset_definition(),
            base_fee: "0.0000000001".parse().expect("sub-nano quantity"),
        };
        let encoded =
            encode_hf_deploy_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt_oss_20b",
            "vision_portal",
            Some("ops_agent"),
            StorageClass::Warm,
            604_800_000_u64,
            hf_shared_lease_asset_definition(),
            payload.base_fee.clone(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
        let canonical_json = String::from_utf8(
            norito::json::to_vec(&payload).expect("serialize exact HF deploy payload"),
        )
        .expect("HF deploy JSON is UTF-8");
        assert!(canonical_json.contains(r#""base_fee":"0.0000000001""#));
        assert!(
            canonical_json.contains(r#""revision":"0123456789abcdef0123456789abcdef01234567""#)
        );
        let missing_revision = canonical_json.replace(
            r#""revision":"0123456789abcdef0123456789abcdef01234567","#,
            "",
        );
        assert!(
            norito::json::from_str::<HfDeployPayload>(&missing_revision).is_err(),
            "HF deploy revision must be explicit"
        );
        assert!(!canonical_json.contains("base_fee_nanos"));
        for hostile in [
            canonical_json.replace(r#""base_fee":"0.0000000001""#, r#""base_fee":1"#),
            canonical_json.replace(r#""base_fee":"0.0000000001""#, r#""base_fee_nanos":1"#),
        ] {
            assert!(
                norito::json::from_str::<HfDeployPayload>(&hostile).is_err(),
                "accepted noncanonical or retired HF base-fee payload `{hostile}`"
            );
        }
    }
    #[test]
    fn hf_revision_parser_requires_immutable_canonical_commit_oid() {
        assert!(parse_hf_revision("0123456789abcdef0123456789abcdef01234567").is_ok());
        for revision in [
            "main",
            "refs/pr/7",
            "0123456789abcdef",
            "0123456789ABCDEF0123456789ABCDEF01234567",
            " 0123456789abcdef0123456789abcdef01234567",
        ] {
            parse_hf_revision(revision)
                .expect_err("mutable or noncanonical revision must be rejected");
        }
    }
    #[test]
    fn hf_lease_leave_signature_payload_layout_is_canonical_tuple() {
        let payload = HfLeaseLeavePayload {
            repo_id: "openai/gpt-oss".to_owned(),
            revision: "1123456789abcdef0123456789abcdef01234567".to_owned(),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            service_name: Some("vision_portal".to_owned()),
            apartment_name: Some("ops_agent".to_owned()),
        };
        let encoded =
            encode_hf_lease_leave_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "1123456789abcdef0123456789abcdef01234567",
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
            revision: "2123456789abcdef0123456789abcdef01234567".to_owned(),
            model_name: "gpt_oss_20b".to_owned(),
            service_name: "vision_portal".to_owned(),
            apartment_name: Some("ops_agent".to_owned()),
            storage_class: StorageClass::Warm,
            lease_term_ms: 604_800_000,
            lease_asset_definition_id: hf_shared_lease_asset_definition(),
            base_fee: "340282366920938463463374607431768211456.0000000001"
                .parse()
                .expect("wide exact quantity"),
        };
        let encoded =
            encode_hf_lease_renew_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            "openai/gpt-oss",
            "2123456789abcdef0123456789abcdef01234567",
            "gpt_oss_20b",
            "vision_portal",
            Some("ops_agent"),
            StorageClass::Warm,
            604_800_000_u64,
            hf_shared_lease_asset_definition(),
            payload.base_fee.clone(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }
    #[test]
    fn ensure_hf_generated_service_instruction_reuses_matching_existing_service()
    -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID)
                .map_err(|err| eyre::eyre!("hf source id failed: {}", err.message))?;
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
                "gpt_oss_20b",
            );
            let mut world = World::default();
            insert_revision(&mut world, &bundle, bundle.service.service_name.to_string());
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    fixture_service_deployment(&bundle),
                );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.expect_single_signatory().clone(),
            };
            let instruction = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
                "gpt_oss_20b",
            );
            let app = mk_app_state_for_tests_with_world(World::default());
            let signer = test_soracloud_mutation_signer(&checked_test_keypair(0xA0));
            let error = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &bundle,
                &source_id,
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let key_pair = checked_test_keypair(0xA1);
            let signer = test_soracloud_mutation_signer(&key_pair);
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
                TEST_HF_COMMIT_OID,
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let signer_keypair = checked_test_keypair(0xA2);
            let provenance_keypair = checked_test_keypair(0xA3);
            let signer = test_soracloud_mutation_signer(&signer_keypair);
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
                TEST_HF_COMMIT_OID,
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let expected_bundle = build_soracloud_hf_generated_service_bundle(
                "web_portal".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
                "gpt_oss_20b",
            );
            let mut existing_bundle = fixture_bundle("1.0.0");
            existing_bundle.service.service_name =
                "web_portal".parse().expect("valid service name");
            existing_bundle.service.container.manifest_hash =
                existing_bundle.container_manifest_hash();
            let mut world = World::default();
            insert_revision(
                &mut world,
                &existing_bundle,
                existing_bundle.service.service_name.to_string(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    existing_bundle.service.service_name.clone(),
                    fixture_service_deployment(&existing_bundle),
                );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.expect_single_signatory().clone(),
            };
            let error = ensure_hf_generated_service_instruction(
                &app,
                &signer,
                &expected_bundle,
                &source_id,
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID)
                .map_err(|err| eyre::eyre!("hf source id failed: {}", err.message))?;
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
                    deployed_sequence: 1,
                    lease_started_height: 1,
                    lease_expires_height: 100,
                    last_renewed_height: 1,
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
                },
            );
            let app = mk_app_state_for_tests_with_world(world);
            let signer = SoracloudMutationSigner {
                authority: ALICE_ID.clone(),
                request_signer: ALICE_ID.expect_single_signatory().clone(),
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
                "gpt_oss_20b",
            );
            let manifest = build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent".parse().expect("valid apartment name"),
                &bundle,
            );
            let app = mk_app_state_for_tests_with_world(World::default());
            let signer = test_soracloud_mutation_signer(&checked_test_keypair(0xA4));
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let key_pair = checked_test_keypair(0xA5);
            let signer = test_soracloud_mutation_signer(&key_pair);
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
        let runtime = required_test_runtime("runtime");
        runtime.block_on(async move {
            let signer_keypair = checked_test_keypair(0xA6);
            let provenance_keypair = checked_test_keypair(0xA7);
            let signer = test_soracloud_mutation_signer(&signer_keypair);
            let source_id = hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID).expect("source id");
            let bundle = build_soracloud_hf_generated_service_bundle(
                "hf_generated_service".parse().expect("valid service name"),
                &source_id.to_string(),
                "openai/gpt-oss",
                TEST_HF_COMMIT_OID,
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
        let deployment = fixture_service_deployment(&bundle);
        let audit = fixture_service_deploy_audit_event(&bundle);
        let revision =
            deployment_bundle_to_control_plane_revision(&deployment, &bundle, Some(&audit), None)
                .expect("admitted fixture revision should project");
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
    fn torii_hf_repo_parser_and_source_id_use_the_shared_canonical_identity() {
        let repo_id =
            parse_hf_repo_id("OpenAI/GPT-OSS").expect("case-sensitive canonical repository ID");
        assert_eq!(repo_id, "OpenAI/GPT-OSS");
        for alias in ["GPT-OSS", "OpenAI//GPT-OSS", "OpenAI/./GPT-OSS"] {
            assert!(parse_hf_repo_id(alias).is_err());
            assert!(hf_source_id(alias, TEST_HF_COMMIT_OID).is_err());
        }
        assert_ne!(
            hf_source_id("OpenAI/GPT-OSS", TEST_HF_COMMIT_OID).expect("uppercase source identity"),
            hf_source_id("openai/GPT-OSS", TEST_HF_COMMIT_OID).expect("lowercase source identity")
        );
    }
    #[test]
    fn hf_profile_model_info_requires_exact_canonical_repo_and_commit() {
        const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
        const REPO: &str = "OpenAI/GPT-OSS";
        validate_hf_profile_model_info_identity(
            &norito::json!({"modelId": REPO, "sha": COMMIT}),
            REPO,
            COMMIT,
        )
        .expect("matching case-sensitive repository and canonical commit");
        validate_hf_profile_model_info_identity(
            &norito::json!({"modelId": "openai/GPT-OSS", "sha": COMMIT}),
            REPO,
            COMMIT,
        )
        .expect_err("provider repository case drift must fail rather than alias");
        for model_info in [
            norito::json!({}),
            norito::json!({"modelId": REPO, "sha": 7}),
            norito::json!({"modelId": REPO, "sha": "main"}),
            norito::json!({"modelId": REPO, "sha": "0123456789ABCDEF0123456789ABCDEF01234567"}),
            norito::json!({"modelId": REPO, "sha": "1123456789abcdef0123456789abcdef01234567"}),
        ] {
            validate_hf_profile_model_info_identity(&model_info, REPO, COMMIT)
                .expect_err("missing, noncanonical, or mismatched provider SHA must fail");
        }
        validate_hf_profile_model_info_identity(
            &norito::json!({"modelId": REPO, "sha": COMMIT}),
            REPO,
            "main",
        )
        .expect_err("mutable requested revision must fail before profile derivation");
    }
    #[test]
    fn hf_profile_urls_require_canonical_identity_and_request_blob_metadata() {
        const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
        let config = iroha_config::parameters::actual::SoracloudRuntimeHuggingFace::default();
        let info =
            hf_model_info_url(&config, "OpenAI/GPT-OSS", COMMIT).expect("canonical model-info URL");
        assert_eq!(info.query(), Some("blobs=true"));
        assert!(hf_model_info_url(&config, "GPT-OSS", COMMIT).is_err());
        assert!(hf_repo_file_url(&config, "owner/../model", COMMIT, "model.gguf").is_err());
    }
    #[test]
    fn hf_profile_head_length_must_exactly_match_authenticated_lfs_size() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_LENGTH,
            reqwest::header::HeaderValue::from_static("8"),
        );
        validate_hf_content_length_headers(
            &headers,
            "owner/model",
            TEST_HF_COMMIT_OID,
            "a.gguf",
            8,
        )
        .expect("exact authenticated length");
        validate_hf_content_length_headers(
            &headers,
            "owner/model",
            TEST_HF_COMMIT_OID,
            "a.gguf",
            7,
        )
        .expect_err("LFS/HEAD mismatch must fail");
        headers.append(
            reqwest::header::CONTENT_LENGTH,
            reqwest::header::HeaderValue::from_static("8"),
        );
        validate_hf_content_length_headers(
            &headers,
            "owner/model",
            TEST_HF_COMMIT_OID,
            "a.gguf",
            8,
        )
        .expect_err("duplicate length headers must fail");
        let mut noncanonical = reqwest::header::HeaderMap::new();
        noncanonical.insert(
            reqwest::header::CONTENT_LENGTH,
            reqwest::header::HeaderValue::from_static("08"),
        );
        validate_hf_content_length_headers(
            &noncanonical,
            "owner/model",
            TEST_HF_COMMIT_OID,
            "a.gguf",
            8,
        )
        .expect_err("alternate decimal spellings must fail");
    }
    #[test]
    fn hf_profile_weight_heads_use_fixed_bounded_concurrency() -> Result<(), eyre::Report> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        const SHARD_COUNT: usize = HF_PROFILE_HEAD_MAX_IN_FLIGHT_V1 + 1;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let siblings = (0..SHARD_COUNT)
                .map(|index| {
                    norito::json!({
                        "rfilename": (format!("shard-{index:02}.gguf")),
                        "lfs": {
                            "sha256": (format!("{:064x}", index + 1)),
                            "size": 1,
                        },
                    })
                })
                .collect::<Vec<_>>();
            let model_info = norito::json!({
                "modelId": "owner/model",
                "sha": TEST_HF_COMMIT_OID,
                "siblings": siblings,
            });
            let model_body = norito::json::to_json(&model_info)?.into_bytes();
            let active_heads = Arc::new(AtomicUsize::new(0));
            let maximum_heads = Arc::new(AtomicUsize::new(0));
            let server_active = Arc::clone(&active_heads);
            let server_maximum = Arc::clone(&maximum_heads);
            let server = tokio::spawn(async move {
                let mut handlers = tokio::task::JoinSet::new();
                for _ in 0..=SHARD_COUNT {
                    let (mut stream, _) = listener.accept().await?;
                    let body = model_body.clone();
                    let active = Arc::clone(&server_active);
                    let maximum = Arc::clone(&server_maximum);
                    handlers.spawn(async move {
                        let mut request = [0_u8; 4_096];
                        let request_bytes = stream.read(&mut request).await?;
                        let request = String::from_utf8_lossy(&request[..request_bytes]);
                        if request.starts_with("GET ") {
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            stream.write_all(header.as_bytes()).await?;
                            stream.write_all(&body).await?;
                        } else {
                            assert!(request.starts_with("HEAD "));
                            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                            maximum.fetch_max(current, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            active.fetch_sub(1, Ordering::SeqCst);
                            stream
                                .write_all(
                                    b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n",
                                )
                                .await?;
                        }
                        Ok::<_, io::Error>(())
                    });
                }
                while let Some(result) = handlers.join_next().await {
                    result??;
                }
                Ok::<_, eyre::Report>(())
            });

            let origin = format!("http://{address}");
            let mut config =
                iroha_config::parameters::actual::SoracloudRuntimeHuggingFace::default();
            config.hub_base_url = origin.clone();
            config.api_base_url = origin;
            config.import_redirect_allowed_origins.clear();
            config.request_timeout = Duration::from_secs(2);
            config.import_max_files = u32::try_from(SHARD_COUNT)?;
            let profile = derive_hf_resource_profile(
                &config,
                "owner/model",
                TEST_HF_COMMIT_OID,
            )
            .await
            .map_err(|error| eyre::eyre!("profile derivation failed: {error:?}"))?;
            assert_eq!(
                profile.selected_weight_file_count,
                u32::try_from(SHARD_COUNT)?
            );
            server.await??;
            assert_eq!(active_heads.load(Ordering::SeqCst), 0);
            assert_eq!(
                maximum_heads.load(Ordering::SeqCst),
                HF_PROFILE_HEAD_MAX_IN_FLIGHT_V1,
                "the ninth shard must wait for one of the fixed eight HEAD slots"
            );
            Ok(())
        })
    }
    #[test]
    fn hf_profile_public_address_policy_rejects_special_purpose_ranges() {
        assert!(hf_profile_ipv4_is_public(Ipv4Addr::new(8, 8, 8, 8)));
        for address in [
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(192, 0, 0, 1),
            Ipv4Addr::new(192, 88, 99, 1),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(203, 0, 113, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ] {
            assert!(
                !hf_profile_ipv4_is_public(address),
                "special-purpose IPv4 address {address} must fail closed"
            );
        }

        assert!(hf_profile_ipv6_is_public(
            "2606:4700:4700::1111".parse().expect("public IPv6")
        ));
        for raw in [
            "::1",
            "fc00::1",
            "fe80::1",
            "2001::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "::ffff:10.0.0.1",
        ] {
            let address = raw.parse().expect("valid special-purpose IPv6");
            assert!(
                !hf_profile_ipv6_is_public(address),
                "special-purpose IPv6 address {address} must fail closed"
            );
        }
        assert!(hf_profile_ipv6_is_public(
            "::ffff:8.8.8.8".parse().expect("public mapped IPv4")
        ));

        // Test-only loopback admission is needed by the bounded local HTTP fixture.
        assert!(hf_profile_ip_is_allowed(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!hf_profile_ip_is_allowed(IpAddr::V4(Ipv4Addr::new(
            10, 0, 0, 1
        ))));
    }
    #[test]
    fn hf_profile_fetch_uses_bounded_manual_redirects_and_rejects_private_targets()
    -> Result<(), eyre::Report> {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                let (mut first, _) = listener.accept().await?;
                let mut request = [0_u8; 2_048];
                let request_bytes = first.read(&mut request).await?;
                let request_text = String::from_utf8_lossy(&request[..request_bytes]);
                assert!(request_text.starts_with("GET /source HTTP/1.1"));
                assert!(
                    !request_text
                        .to_ascii_lowercase()
                        .contains("authorization:")
                );
                first
                    .write_all(
                        b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await?;
                drop(first);
                let (mut second, _) = listener.accept().await?;
                let request_bytes = second.read(&mut request).await?;
                let request_text = String::from_utf8_lossy(&request[..request_bytes]);
                assert!(request_text.starts_with("GET /final HTTP/1.1"));
                assert!(
                    !request_text
                        .to_ascii_lowercase()
                        .contains("authorization:")
                );
                second
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await?;
                Ok::<_, io::Error>(())
            });
            let origin = format!("http://{address}");
            let mut config =
                iroha_config::parameters::actual::SoracloudRuntimeHuggingFace::default();
            config.hub_base_url = origin.clone();
            config.api_base_url = origin.clone();
            config.import_redirect_allowed_origins.clear();
            config.request_timeout = Duration::from_secs(2);
            let response = send_hf_profile_request_with_vetted_redirects(
                &config,
                reqwest::Method::GET,
                reqwest::Url::parse(&format!("{origin}/source"))?,
            )
            .await
            .map_err(|error| eyre::eyre!("profile request failed: {error:?}"))?;
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(response.text().await?, "ok");
            server.await??;

            resolve_hf_profile_socket_addrs(
                &reqwest::Url::parse("https://10.0.0.1/model")?,
                Duration::from_secs(1),
            )
            .await
            .expect_err("private address must be rejected before contact");
            resolve_hf_profile_socket_addrs(
                &reqwest::Url::parse("https://[::ffff:10.0.0.1]/model")?,
                Duration::from_secs(1),
            )
            .await
            .expect_err("IPv4-mapped private address must be rejected before contact");
            Ok(())
        })
    }
    #[test]
    fn authoritative_hf_shared_lease_status_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = TEST_HF_COMMIT_OID;
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
                        base_fee: "0.00001".parse().expect("base fee"),
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
                                base_fee: "0.00002".parse().expect("base fee"),
                                compute_reservation_cap: "0.0000075"
                                    .parse()
                                    .expect("compute reservation cap"),
                                resource_profile:
                                    iroha_data_model::soracloud::SoraHfResourceProfileV1 {
                                        required_model_bytes: 4_096,
                                        backend_family:
                                            iroha_data_model::soracloud::SoraHfBackendFamilyV1::Transformers,
                                        model_format:
                                            iroha_data_model::soracloud::SoraHfModelFormatV1::Safetensors,
                                        selected_weight_file_count: 1,
                                        weight_selection_commitment: Hash::new(
                                            b"queued-hf-weight-selection",
                                        ),
                                        disk_cache_bytes_floor: 8_192,
                                        ram_bytes_floor: 8_192,
                                        vram_bytes_floor: 0,
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
                        total_paid: "0.00001".parse().expect("total paid"),
                        total_refunded: Quantity::zero(),
                        last_charge: "0.00001".parse().expect("last charge"),
                        total_compute_paid: Quantity::zero(),
                        total_compute_refunded: Quantity::zero(),
                        last_compute_charge: Quantity::zero(),
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
                        action: SoraHfSharedLeaseActionV1::Renew,
                        pool_id,
                        source_id,
                        account_id: ALICE_ID.clone(),
                        occurred_at_ms: 10,
                        active_member_count: 1,
                        charged: "0.00002".parse().expect("charged amount"),
                        refunded: Quantity::zero(),
                        lease_expires_at_ms: (lease_term_ms * 2) + 10,
                        failure_reason: None,
                        service_name: Some("vision_portal_v2".to_owned()),
                        apartment_name: Some("ops_agent_v2".to_owned()),
                    },
                );
            world
                .soracloud_hf_shared_lease_audit_events_mut_for_testing()
                .insert(
                    8,
                    SoraHfSharedLeaseAuditEventV1 {
                        schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
                        sequence: 8,
                        action: SoraHfSharedLeaseActionV1::Join,
                        pool_id,
                        source_id,
                        account_id: BOB_ID.clone(),
                        occurred_at_ms: 11,
                        active_member_count: 2,
                        charged: "0.000005".parse().expect("charged amount"),
                        refunded: Quantity::zero(),
                        lease_expires_at_ms: lease_term_ms + 10,
                        failure_reason: None,
                        service_name: Some("concurrent_portal".to_owned()),
                        apartment_name: None,
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
            assert_eq!(response.audit_event_count, 2);
            assert!(response.importer_pending);
            assert!(response.runtime_projection.is_none());
            assert_eq!(
                response
                    .latest_audit_event
                    .as_ref()
                    .expect("audit event")
                    .account_id,
                ALICE_ID.clone()
            );
            assert_eq!(
                response
                    .latest_audit_event
                    .as_ref()
                    .expect("audit event")
                    .sequence,
                7
            );
            let unscoped_response = authoritative_hf_shared_lease_status_response(
                &app,
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                None,
            )
            .map_err(|err| eyre::eyre!("unscoped authoritative hf status failed: {err:?}"))?;
            assert_eq!(
                unscoped_response
                    .latest_audit_event
                    .as_ref()
                    .expect("unscoped latest audit event")
                    .account_id,
                BOB_ID.clone()
            );
            assert_eq!(
                unscoped_response
                    .latest_audit_event
                    .as_ref()
                    .expect("unscoped latest audit event")
                    .sequence,
                8
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
            let mutation_response = authoritative_hf_shared_lease_mutation_response(
                &app,
                &SoracloudAuditBaseline {
                    hf_shared_lease_max: 6,
                    ..SoracloudAuditBaseline::default()
                },
                repo_id,
                resolved_revision,
                storage_class,
                lease_term_ms,
                &ALICE_ID,
                Some("vision_portal_v2"),
                Some("ops_agent_v2"),
            )
            .map_err(|err| eyre::eyre!("queued hf mutation response failed: {err:?}"))?;
            assert_eq!(mutation_response.action, SoraHfSharedLeaseActionV1::Renew);
            assert!(mutation_response.placement.is_none());
            assert!(mutation_response.compute_reservation_fee.is_zero());
            assert_eq!(
                mutation_response.storage_base_fee,
                "0.00002".parse::<Quantity>().expect("queued base fee")
            );
            Ok(())
        })
    }
    #[test]
    fn authoritative_hf_shared_lease_mutation_reads_world_state() -> Result<(), eyre::Report> {
        use iroha_core::state::World;
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = TEST_HF_COMMIT_OID;
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
                        base_fee: "0.00001".parse().expect("base fee"),
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
                        total_paid: "0.000013333".parse().expect("total paid"),
                        total_refunded: Quantity::zero(),
                        last_charge: "0.000003333".parse().expect("last charge"),
                        total_compute_paid: Quantity::zero(),
                        total_compute_refunded: Quantity::zero(),
                        last_compute_charge: Quantity::zero(),
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
                        charged: "0.000003333".parse().expect("charged amount"),
                        refunded: Quantity::zero(),
                        lease_expires_at_ms: lease_term_ms + 10,
                        failure_reason: None,
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let repo_id = "openai/gpt-oss";
            let resolved_revision = TEST_HF_COMMIT_OID;
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
            source_id: hf_source_id("openai/gpt-oss", TEST_HF_COMMIT_OID)
                .expect("canonical HF source ID"),
            repo_id: "openai/gpt-oss".to_owned(),
            resolved_revision: TEST_HF_COMMIT_OID.to_owned(),
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
                execution_host: None,
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
                execution_host: None,
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let apartment_name = "ops_agent";
            let run = fixture_agent_run_record(apartment_name, "ops_agent:autonomy:1", 7, 1);
            let manifest = fixture_agent_manifest();
            let record = fixture_agent_apartment_record(manifest.clone(), run.clone(), 1);
            let approval_signer = checked_test_keypair(0xB0);
            let finalize_signer = checked_test_keypair(0xB1);
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
                crate::loopback_connect_info(),
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
        let runtime = test_runtime()?;
        runtime.block_on(async move {
            let apartment_name = "ops_agent";
            let run = fixture_agent_run_record(apartment_name, "ops_agent:autonomy:2", 9, 1);
            let mut manifest = fixture_agent_manifest();
            manifest.tool_capabilities.clear();
            let record = fixture_agent_apartment_record(manifest, run.clone(), 1);
            let signer = checked_test_keypair(0xB2);
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
        let runtime = test_runtime()?;
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
                    execution_host: None,
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
                    execution_host: None,
                    mailbox_message_id: None,
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
        let runtime = test_runtime()?;
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
                        block_height: 88,
                        block_timestamp_ms: 88,
                        action: SoraAgentApartmentActionV1::AutonomyRunExecuted,
                        apartment_name: "ops_agent".parse().expect("valid apartment name"),
                        status: SoraAgentRuntimeStatusV1::Running,
                        lease_expires_height: 100,
                        manifest_hash: Hash::new(b"agent-manifest"),
                        restart_count: 0,
                        signer: checked_test_keypair(0xB3).public_key().clone(),
                        request_id: Some("ops_agent:autonomy:executed".to_owned()),
                        asset_definition: None,
                        amount: None,
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
                    execution_host: None,
                    mailbox_message_id: None,
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
    include!("soracloud/tests/agent_runtime_status.rs");
}
