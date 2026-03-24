//! Soracloud lifecycle instructions.
//!
//! These instructions move Soracloud service deployment state into the
//! authoritative on-chain world model instead of Torii-local file persistence.

use core::cmp::Ordering;

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1,
        FheExecutionPolicyV1, FheJobSpecV1, FheParamSetV1, SoraDeploymentBundleV1,
        SoraHfResourceProfileV1, SoraModelHostCapabilityRecordV1, SoraModelHostViolationKindV1,
        SoraModelPrivacyModeV1, SoraPrivateCompileProfileV1, SoraPrivateInferenceCheckpointV1,
        SoraPrivateInferenceSessionStatusV1, SoraPrivateInferenceSessionV1, SoraRuntimeReceiptV1,
        SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1, SoraStateEncryptionV1,
        SoraStateMutationOperationV1, SoraUploadedModelBundleV1, SoraUploadedModelChunkV1,
    },
    sorafs::pin_registry::StorageClass,
};

fn encoded_order<T: Encode>(left: &T, right: &T) -> Ordering {
    left.encode().cmp(&right.encode())
}

/// Admit a brand new Soracloud service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeploySoracloudService {
    /// Bundle being admitted.
    pub bundle: SoraDeploymentBundleV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for DeploySoracloudService {}

impl PartialOrd for DeploySoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Admit a new candidate revision for an existing Soracloud service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct UpgradeSoracloudService {
    /// Bundle being admitted as the candidate revision.
    pub bundle: SoraDeploymentBundleV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for UpgradeSoracloudService {}

impl PartialOrd for UpgradeSoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Roll a Soracloud service back to an already admitted revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RollbackSoracloudService {
    /// Service to roll back.
    pub service_name: Name,
    /// Optional target version. When omitted, the latest non-current baseline is used.
    #[norito(default)]
    pub target_version: Option<String>,
    /// Provenance attestation over the rollback payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RollbackSoracloudService {}

impl PartialOrd for RollbackSoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record an ordered Soracloud state mutation against a declared binding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MutateSoracloudState {
    /// Service whose state binding should be mutated.
    pub service_name: Name,
    /// Binding being mutated.
    pub binding_name: Name,
    /// Canonical key under the binding prefix.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Declared payload size for upsert operations.
    #[norito(default)]
    pub value_size_bytes: Option<u64>,
    /// Expected binding encryption mode.
    pub encryption: SoraStateEncryptionV1,
    /// Governance transaction hash attached to the mutation.
    pub governance_tx_hash: Hash,
    /// Provenance attestation over the mutation payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for MutateSoracloudState {}

impl PartialOrd for MutateSoracloudState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record an ordered Soracloud FHE execution result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RunSoracloudFheJob {
    /// Service whose ciphertext state receives the job output.
    pub service_name: Name,
    /// Binding receiving the deterministic ciphertext output.
    pub binding_name: Name,
    /// Deterministic FHE job specification.
    pub job: FheJobSpecV1,
    /// Execution policy snapshot validated for this job.
    pub policy: FheExecutionPolicyV1,
    /// Parameter set validated for this job.
    pub param_set: FheParamSetV1,
    /// Governance transaction hash attached to the job.
    pub governance_tx_hash: Hash,
    /// Provenance attestation over the job payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RunSoracloudFheJob {}

impl PartialOrd for RunSoracloudFheJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record an ordered Soracloud decryption or health-access request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudDecryptionRequest {
    /// Service whose ciphertext state is being requested.
    pub service_name: Name,
    /// Policy snapshot used to validate the request.
    pub policy: DecryptionAuthorityPolicyV1,
    /// Decryption request payload.
    pub request: DecryptionRequestV1,
    /// Provenance attestation over the request payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RecordSoracloudDecryptionRequest {}

impl PartialOrd for RecordSoracloudDecryptionRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Join or create a shared Hugging Face lease window on Soracloud.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JoinSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Normalized model name used for Soracloud bindings.
    pub model_name: String,
    /// Service binding that will reuse the shared lease.
    pub service_name: Name,
    /// Optional apartment binding that will reuse the shared lease.
    #[norito(default)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window price in nanos of `lease_asset_definition_id`.
    pub base_fee_nanos: u128,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(default)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
    /// Provenance attestation over the join payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for JoinSoracloudHfSharedLease {}

impl PartialOrd for JoinSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Leave the current shared Hugging Face lease window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LeaveSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Optional service binding being detached for audit context.
    #[norito(default)]
    pub service_name: Option<Name>,
    /// Optional apartment binding being detached for audit context.
    #[norito(default)]
    pub apartment_name: Option<Name>,
    /// Provenance attestation over the leave payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for LeaveSoracloudHfSharedLease {}

impl PartialOrd for LeaveSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Sponsor a fresh shared Hugging Face lease window after expiry or retirement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RenewSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Normalized model name used for Soracloud bindings.
    pub model_name: String,
    /// Service binding that will reuse the renewed shared lease.
    pub service_name: Name,
    /// Optional apartment binding that will reuse the renewed shared lease.
    #[norito(default)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window price in nanos of `lease_asset_definition_id`.
    pub base_fee_nanos: u128,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(default)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
    /// Provenance attestation over the renew payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RenewSoracloudHfSharedLease {}

impl PartialOrd for RenewSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Advertise validator-host capabilities for authoritative HF placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdvertiseSoracloudModelHost {
    /// Capability advert being published by the validator.
    pub capability: SoraModelHostCapabilityRecordV1,
    /// Provenance attestation over the advert payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AdvertiseSoracloudModelHost {}

impl PartialOrd for AdvertiseSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Refresh the heartbeat TTL for an advertised validator host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct HeartbeatSoracloudModelHost {
    /// Validator account that owns the host advert.
    pub validator_account_id: AccountId,
    /// New heartbeat-expiry timestamp.
    pub heartbeat_expires_at_ms: u64,
    /// Provenance attestation over the heartbeat payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for HeartbeatSoracloudModelHost {}

impl PartialOrd for HeartbeatSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Withdraw an advertised validator host from authoritative HF placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct WithdrawSoracloudModelHost {
    /// Validator account that owns the host advert.
    pub validator_account_id: AccountId,
    /// Provenance attestation over the withdrawal payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for WithdrawSoracloudModelHost {}

impl PartialOrd for WithdrawSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Reconcile expired validator-host adverts against authoritative HF placements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReconcileSoracloudModelHosts;

impl crate::seal::Instruction for ReconcileSoracloudModelHosts {}

impl PartialOrd for ReconcileSoracloudModelHosts {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Report authoritative evidence for a validator-host violation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReportSoracloudModelHostViolation {
    /// Validator responsible for the violation.
    pub validator_account_id: AccountId,
    /// Violation class.
    pub kind: SoraModelHostViolationKindV1,
    /// Implicated placement when the violation is placement-scoped.
    #[norito(default)]
    pub placement_id: Option<Hash>,
    /// Optional explanatory detail attached to the evidence.
    #[norito(default)]
    pub detail: Option<String>,
}

impl crate::seal::Instruction for ReportSoracloudModelHostViolation {}

impl PartialOrd for ReportSoracloudModelHostViolation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Deploy a Soracloud agent apartment into authoritative world state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeploySoracloudAgentApartment {
    /// Apartment manifest being admitted.
    pub manifest: AgentApartmentManifestV1,
    /// Requested lease duration in deterministic ticks.
    pub lease_ticks: u64,
    /// Initial autonomy budget ceiling for the apartment.
    pub autonomy_budget_units: u64,
    /// Provenance attestation over the deploy payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for DeploySoracloudAgentApartment {}

impl PartialOrd for DeploySoracloudAgentApartment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Renew a Soracloud agent apartment lease.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RenewSoracloudAgentLease {
    /// Apartment to renew.
    pub apartment_name: Name,
    /// Requested lease duration in deterministic ticks.
    pub lease_ticks: u64,
    /// Provenance attestation over the renew payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RenewSoracloudAgentLease {}

impl PartialOrd for RenewSoracloudAgentLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Restart a Soracloud agent apartment process.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RestartSoracloudAgentApartment {
    /// Apartment to restart.
    pub apartment_name: Name,
    /// Human-readable restart reason.
    pub reason: String,
    /// Provenance attestation over the restart payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RestartSoracloudAgentApartment {}

impl PartialOrd for RestartSoracloudAgentApartment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Revoke an active Soracloud agent apartment policy capability.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RevokeSoracloudAgentPolicy {
    /// Apartment whose policy should change.
    pub apartment_name: Name,
    /// Capability identifier to revoke.
    pub capability: String,
    /// Optional human-readable reason.
    #[norito(default)]
    pub reason: Option<String>,
    /// Provenance attestation over the revoke payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RevokeSoracloudAgentPolicy {}

impl PartialOrd for RevokeSoracloudAgentPolicy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Submit a policy-gated wallet spend request for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RequestSoracloudAgentWalletSpend {
    /// Apartment initiating the spend.
    pub apartment_name: Name,
    /// Asset definition constrained by apartment policy.
    pub asset_definition: String,
    /// Requested spend amount in nanos.
    pub amount_nanos: u64,
    /// Provenance attestation over the spend payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RequestSoracloudAgentWalletSpend {}

impl PartialOrd for RequestSoracloudAgentWalletSpend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Approve and apply a pending wallet spend request for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ApproveSoracloudAgentWalletSpend {
    /// Apartment owning the pending request.
    pub apartment_name: Name,
    /// Deterministic request identifier to approve.
    pub request_id: String,
    /// Provenance attestation over the approval payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for ApproveSoracloudAgentWalletSpend {}

impl PartialOrd for ApproveSoracloudAgentWalletSpend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Enqueue a deterministic mailbox message between agent apartments.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct EnqueueSoracloudAgentMessage {
    /// Sender apartment.
    pub from_apartment: Name,
    /// Recipient apartment.
    pub to_apartment: Name,
    /// Logical mailbox channel.
    pub channel: String,
    /// Message payload.
    pub payload: String,
    /// Provenance attestation over the message payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for EnqueueSoracloudAgentMessage {}

impl PartialOrd for EnqueueSoracloudAgentMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Acknowledge and consume a queued mailbox message for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AcknowledgeSoracloudAgentMessage {
    /// Apartment consuming the mailbox message.
    pub apartment_name: Name,
    /// Deterministic message identifier to acknowledge.
    pub message_id: String,
    /// Provenance attestation over the acknowledgement payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AcknowledgeSoracloudAgentMessage {}

impl PartialOrd for AcknowledgeSoracloudAgentMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Allowlist an autonomy artifact for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AllowSoracloudAgentAutonomyArtifact {
    /// Apartment receiving the allowlist rule.
    pub apartment_name: Name,
    /// Artifact hash being allowlisted.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(default)]
    pub provenance_hash: Option<String>,
    /// Provenance attestation over the allowlist payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AllowSoracloudAgentAutonomyArtifact {}

impl PartialOrd for AllowSoracloudAgentAutonomyArtifact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Approve a deterministic autonomy run for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RunSoracloudAgentAutonomy {
    /// Apartment owning the run.
    pub apartment_name: Name,
    /// Allowlisted artifact hash being executed.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(default)]
    pub provenance_hash: Option<String>,
    /// Budget units approved for the run.
    pub budget_units: u64,
    /// Human-readable run label.
    pub run_label: String,
    /// Optional canonical JSON body forwarded to the generated HF `/infer` handler.
    #[norito(default)]
    pub workflow_input_json: Option<String>,
    /// Provenance attestation over the autonomy-run payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RunSoracloudAgentAutonomy {}

impl PartialOrd for RunSoracloudAgentAutonomy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Persist an authoritative apartment-level execution audit for a completed autonomy run.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudAgentAutonomyExecution {
    /// Apartment that owns the executed run.
    pub apartment_name: Name,
    /// Stable approved run identifier.
    pub run_id: String,
    /// Process generation that executed the run.
    pub process_generation: u64,
    /// Whether the runtime completed the run successfully.
    pub succeeded: bool,
    /// Deterministic commitment over the execution outcome.
    pub result_commitment: Hash,
    /// Generated service name used for execution, when locally resolved.
    #[norito(default)]
    pub service_name: Option<Name>,
    /// Generated service version used for execution, when locally resolved.
    #[norito(default)]
    pub service_version: Option<String>,
    /// Generated handler used for execution, when locally resolved.
    #[norito(default)]
    pub handler_name: Option<Name>,
    /// Authoritative runtime receipt referenced by the execution, when one exists.
    #[norito(default)]
    pub runtime_receipt_id: Option<Hash>,
    /// Node-local journal artifact hash, when one was persisted.
    #[norito(default)]
    pub journal_artifact_hash: Option<Hash>,
    /// Node-local checkpoint artifact hash, when one was persisted.
    #[norito(default)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Human-readable runtime error, when execution failed.
    #[norito(default)]
    pub error: Option<String>,
}

impl crate::seal::Instruction for RecordSoracloudAgentAutonomyExecution {}

impl PartialOrd for RecordSoracloudAgentAutonomyExecution {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Start a deterministic Soracloud training job.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StartSoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Logical model name targeted by the job.
    pub model_name: String,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Size of the worker group.
    pub worker_group_size: u16,
    /// Total target step count.
    pub target_steps: u32,
    /// Required checkpoint interval.
    pub checkpoint_interval_steps: u32,
    /// Maximum retry count.
    pub max_retries: u8,
    /// Compute units consumed per worker-group step.
    pub step_compute_units: u64,
    /// Total compute budget for the job.
    pub compute_budget_units: u64,
    /// Total storage budget for checkpoints.
    pub storage_budget_bytes: u64,
    /// Provenance attestation over the job payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for StartSoracloudTrainingJob {}

impl PartialOrd for StartSoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record a deterministic Soracloud training checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CheckpointSoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Completed step count after this checkpoint.
    pub completed_step: u32,
    /// Checkpoint artifact size in bytes.
    pub checkpoint_size_bytes: u64,
    /// Metrics artifact hash for the checkpoint.
    pub metrics_hash: Hash,
    /// Provenance attestation over the checkpoint payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for CheckpointSoracloudTrainingJob {}

impl PartialOrd for CheckpointSoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Move a deterministic Soracloud training job into retry-pending state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RetrySoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Normalized retry reason.
    pub reason: String,
    /// Provenance attestation over the retry payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RetrySoracloudTrainingJob {}

impl PartialOrd for RetrySoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Register a deterministic Soracloud model artifact.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterSoracloudModelArtifact {
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Training job that produced the artifact.
    pub training_job_id: String,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the artifact payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RegisterSoracloudModelArtifact {}

impl PartialOrd for RegisterSoracloudModelArtifact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Register a deterministic Soracloud model-weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Version identifier being admitted.
    pub weight_version: String,
    /// Training job that produced this version.
    pub training_job_id: String,
    /// Optional lineage parent version.
    #[norito(default)]
    pub parent_version: Option<String>,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the register payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RegisterSoracloudModelWeight {}

impl PartialOrd for RegisterSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Promote an admitted Soracloud model-weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PromoteSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Version being promoted.
    pub weight_version: String,
    /// Gate result that authorizes promotion.
    pub gate_approved: bool,
    /// Gate report hash attached to the promotion.
    pub gate_report_hash: Hash,
    /// Provenance attestation over the promote payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for PromoteSoracloudModelWeight {}

impl PartialOrd for PromoteSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Roll a Soracloud model registry back to a prior admitted weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RollbackSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Target version to make current.
    pub target_version: String,
    /// Human-readable rollback reason.
    pub reason: String,
    /// Provenance attestation over the rollback payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RollbackSoracloudModelWeight {}

impl PartialOrd for RollbackSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Register an uploaded-model bundle root before encrypted chunks arrive.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterSoracloudUploadedModelBundle {
    /// Deterministic uploaded-model bundle metadata.
    pub bundle: SoraUploadedModelBundleV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RegisterSoracloudUploadedModelBundle {}

impl PartialOrd for RegisterSoracloudUploadedModelBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Append one encrypted uploaded-model chunk into authoritative chain state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AppendSoracloudUploadedModelChunk {
    /// Deterministic encrypted chunk metadata.
    pub chunk: SoraUploadedModelChunkV1,
    /// Provenance attestation over the chunk payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AppendSoracloudUploadedModelChunk {}

impl PartialOrd for AppendSoracloudUploadedModelChunk {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Seal an uploaded-model bundle and publish its artifact metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FinalizeSoracloudUploadedModelBundle {
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Logical model name in the canonical registry plane.
    pub model_name: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Artifact identifier bound to the uploaded model.
    pub artifact_id: String,
    /// Version label pinned by the artifact.
    pub weight_version: String,
    /// Canonical uploaded-model bundle root.
    pub bundle_root: Hash,
    /// Execution privacy mode exposed to apartments.
    pub privacy_mode: SoraModelPrivacyModeV1,
    /// Artifact hash for the uploaded payload.
    pub weight_artifact_hash: Hash,
    /// Dataset reference or upload source label.
    pub dataset_ref: String,
    /// Deterministic training/configuration hash for the upload.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash for the canonical upload bundle.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the finalize payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for FinalizeSoracloudUploadedModelBundle {}

impl PartialOrd for FinalizeSoracloudUploadedModelBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Admit a private compile profile for an uploaded-model bundle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdmitSoracloudPrivateCompileProfile {
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Version label pinned by the bundle.
    pub weight_version: String,
    /// Bundle root receiving the compile profile.
    pub bundle_root: Hash,
    /// Deterministic compile profile being admitted.
    pub compile_profile: SoraPrivateCompileProfileV1,
    /// Provenance attestation over the compile payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AdmitSoracloudPrivateCompileProfile {}

impl PartialOrd for AdmitSoracloudPrivateCompileProfile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Explicitly bind an uploaded model version to an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AllowSoracloudUploadedModel {
    /// Apartment receiving the uploaded-model binding.
    pub apartment_name: Name,
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Logical model name in the canonical registry plane.
    pub model_name: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Artifact identifier backing the binding.
    pub artifact_id: String,
    /// Version label pinned by the apartment.
    pub weight_version: String,
    /// Bundle root admitted for the apartment.
    pub bundle_root: Hash,
    /// Compile profile hash used for the private runtime.
    pub compile_profile_hash: Hash,
    /// Execution privacy mode bound to the apartment.
    pub privacy_mode: SoraModelPrivacyModeV1,
    /// Whether `allow_model_inference` must stay active.
    pub require_model_inference: bool,
    /// Provenance attestation over the allow payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AllowSoracloudUploadedModel {}

impl PartialOrd for AllowSoracloudUploadedModel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Start an authoritative private inference session for an uploaded model.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StartSoracloudPrivateInference {
    /// Session metadata admitted for execution.
    pub session: SoraPrivateInferenceSessionV1,
    /// Provenance attestation over the session payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for StartSoracloudPrivateInference {}

impl PartialOrd for StartSoracloudPrivateInference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record a deterministic checkpoint or output release for a private session.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudPrivateInferenceCheckpoint {
    /// Stable session identifier being updated.
    pub session_id: String,
    /// New authoritative runtime status after the checkpoint is applied.
    pub status: SoraPrivateInferenceSessionStatusV1,
    /// Updated receipt root for the session.
    pub receipt_root: Hash,
    /// Total XOR nanos charged so far.
    pub xor_cost_nanos: u128,
    /// Deterministic checkpoint being published.
    pub checkpoint: SoraPrivateInferenceCheckpointV1,
    /// Provenance attestation over the checkpoint payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for RecordSoracloudPrivateInferenceCheckpoint {}

impl PartialOrd for RecordSoracloudPrivateInferenceCheckpoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Advance or roll back an in-flight Soracloud rollout.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdvanceSoracloudRollout {
    /// Service whose rollout should advance.
    pub service_name: Name,
    /// Deterministic rollout handle.
    pub rollout_handle: String,
    /// Health observation for the current rollout step.
    pub healthy: bool,
    /// Optional promotion target for healthy steps.
    #[norito(default)]
    pub promote_to_percent: Option<u8>,
    /// Governance transaction hash attached to the rollout step.
    pub governance_tx_hash: Hash,
    /// Provenance attestation over the rollout payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for AdvanceSoracloudRollout {}

impl PartialOrd for AdvanceSoracloudRollout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Upsert authoritative node/runtime state for a Soracloud service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SetSoracloudRuntimeState {
    /// Runtime state to persist.
    pub state: SoraServiceRuntimeStateV1,
}

impl crate::seal::Instruction for SetSoracloudRuntimeState {}

impl PartialOrd for SetSoracloudRuntimeState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Persist an ordered Soracloud mailbox message.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudMailboxMessage {
    /// Mailbox message to persist.
    pub message: SoraServiceMailboxMessageV1,
}

impl crate::seal::Instruction for RecordSoracloudMailboxMessage {}

impl PartialOrd for RecordSoracloudMailboxMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Persist an authoritative Soracloud runtime receipt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudRuntimeReceipt {
    /// Runtime receipt to persist.
    pub receipt: SoraRuntimeReceiptV1,
}

impl crate::seal::Instruction for RecordSoracloudRuntimeReceipt {}

impl PartialOrd for RecordSoracloudRuntimeReceipt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
