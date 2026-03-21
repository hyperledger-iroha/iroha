//! Shared Soracloud runtime snapshot types and execution traits.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::InstructionBox,
    soracloud::{
        SoraAgentRuntimeStatusV1, SoraArtifactKindV1, SoraCertifiedResponsePolicyV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraRuntimeReceiptV1,
        SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHandlerV1,
        SoraServiceHealthStatusV1, SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1,
        SoraStateEncryptionV1, SoraStateMutationOperationV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Distinguishes the local runtime role of a materialized service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "revision_role", content = "value")]
pub enum SoracloudRuntimeRevisionRole {
    /// The currently active deployment revision.
    Active,
    /// A canary candidate revision that must be materialized during rollout.
    CanaryCandidate,
}

/// Node-local mailbox materialization metadata for a handler.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeMailboxPlan {
    /// Stable handler identifier.
    pub handler_name: String,
    /// Stable logical queue name.
    pub queue_name: String,
    /// Maximum retained pending messages.
    pub max_pending_messages: u32,
    /// Maximum message size.
    pub max_message_bytes: u64,
    /// Retention bound for queued messages.
    pub retention_blocks: u32,
}

/// Node-local hydration/materialization metadata for a referenced artifact.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeArtifactPlan {
    /// Artifact class.
    pub kind: SoraArtifactKindV1,
    /// Content-addressed artifact digest.
    pub artifact_hash: String,
    /// Logical artifact path inside the service revision.
    pub artifact_path: String,
    /// Optional consuming handler.
    pub handler_name: Option<String>,
    /// Local cache path where the runtime manager expects the artifact.
    pub local_cache_path: String,
    /// Whether the artifact is already present in the node-local cache.
    pub available_locally: bool,
}

/// Node-local materialization plan for one active service revision.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeServicePlan {
    /// Service identifier.
    pub service_name: String,
    /// Materialized revision/version.
    pub service_version: String,
    /// Whether this revision is the active one or a rollout candidate.
    pub role: SoracloudRuntimeRevisionRole,
    /// Requested traffic percentage for this revision.
    pub traffic_percent: u8,
    /// Runtime target.
    pub runtime: SoraContainerRuntimeV1,
    /// Bundle digest.
    pub bundle_hash: String,
    /// Bundle path declared by the container manifest.
    pub bundle_path: String,
    /// Entrypoint declared by the container manifest.
    pub entrypoint: String,
    /// Node-local cache path for the executable bundle.
    pub bundle_cache_path: String,
    /// Whether the bundle is already present locally.
    pub bundle_available_locally: bool,
    /// Current deployment process generation when known for this revision.
    pub process_generation: Option<u64>,
    /// Current runtime health projection.
    pub health_status: SoraServiceHealthStatusV1,
    /// Current runtime load projection.
    pub load_factor_bps: u16,
    /// Pending mailbox count reported for this revision.
    pub reported_pending_mailbox_messages: u32,
    /// Pending mailbox messages currently stored in authoritative state.
    pub authoritative_pending_mailbox_messages: u32,
    /// Active rollout handle when this revision is part of a canary rollout.
    pub rollout_handle: Option<String>,
    /// Local directory where the revision plan is materialized.
    pub materialization_dir: String,
    /// Declared replicated handler mailboxes.
    pub mailboxes: Vec<SoracloudRuntimeMailboxPlan>,
    /// Referenced artifacts that still need local hydration.
    pub artifacts: Vec<SoracloudRuntimeArtifactPlan>,
}

/// Node-local materialization plan for an active agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeApartmentPlan {
    /// Apartment identifier.
    pub apartment_name: String,
    /// Canonical manifest hash.
    pub manifest_hash: String,
    /// Current runtime status.
    pub status: SoraAgentRuntimeStatusV1,
    /// Current process generation.
    pub process_generation: u64,
    /// Audit sequence when the lease expires.
    pub lease_expires_sequence: u64,
    /// Audit sequence of the most recent observed activity.
    pub last_active_sequence: u64,
    /// Node-local directory where the apartment plan is materialized.
    pub materialization_dir: String,
    /// Number of pending wallet approvals.
    pub pending_wallet_request_count: u32,
    /// Number of queued mailbox messages.
    pub pending_mailbox_message_count: u32,
    /// Remaining autonomy budget.
    pub autonomy_budget_remaining_units: u64,
    /// Number of explicitly approved autonomy artifacts.
    pub approved_artifact_count: u32,
    /// Number of recorded autonomy runs.
    pub autonomy_run_count: u32,
    /// Number of revoked policy capabilities.
    pub revoked_policy_capability_count: u32,
}

/// Persisted snapshot of node-local Soracloud runtime materialization state.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeSnapshot {
    /// Schema version for the local runtime snapshot format.
    pub schema_version: u16,
    /// Height of the authoritative state view used to build this snapshot.
    pub observed_height: u64,
    /// Latest committed block hash at snapshot time, when present.
    pub observed_block_hash: Option<String>,
    /// Materialized active service revisions grouped by service name then version.
    pub services: BTreeMap<String, BTreeMap<String, SoracloudRuntimeServicePlan>>,
    /// Materialized active agent apartments keyed by apartment name.
    pub apartments: BTreeMap<String, SoracloudRuntimeApartmentPlan>,
}

impl Default for SoracloudRuntimeSnapshot {
    fn default() -> Self {
        Self {
            schema_version: 1,
            observed_height: 0,
            observed_block_hash: None,
            services: BTreeMap::new(),
            apartments: BTreeMap::new(),
        }
    }
}

/// Read-only Soracloud runtime handle exposed to Torii and other consumers.
pub trait SoracloudRuntimeReadHandle: Send + Sync {
    /// Return the latest node-local runtime materialization snapshot.
    fn snapshot(&self) -> SoracloudRuntimeSnapshot;

    /// Return the local runtime-manager state directory.
    fn state_dir(&self) -> PathBuf;
}

/// Shared Soracloud runtime handle type used across crate boundaries.
pub type SharedSoracloudRuntimeHandle = Arc<dyn SoracloudRuntimeReadHandle>;

/// Coarse execution failure category for embedded Soracloud runtime requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudRuntimeExecutionErrorKind {
    /// The runtime cannot execute the request in the current node process.
    Unavailable,
    /// The request is structurally invalid for the configured runtime surface.
    InvalidRequest,
    /// The runtime hit an internal execution failure.
    Internal,
}

/// Structured error returned by the shared Soracloud runtime execution trait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudRuntimeExecutionError {
    /// High-level error category.
    pub kind: SoracloudRuntimeExecutionErrorKind,
    /// Human-readable detail preserved for logging and deterministic receipts.
    pub message: String,
}

impl SoracloudRuntimeExecutionError {
    /// Construct a new structured runtime execution error.
    #[must_use]
    pub fn new(kind: SoracloudRuntimeExecutionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Deterministic local read class for the Soracloud fast path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudLocalReadKind {
    /// Static asset read bound to committed artifacts.
    Asset,
    /// Read-only query bound to the committed state snapshot.
    Query,
}

/// Shared request envelope for deterministic local Soracloud reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudLocalReadRequest {
    /// Authoritative height used for the local read snapshot.
    pub observed_height: u64,
    /// Latest committed block hash visible to the caller.
    pub observed_block_hash: Option<Hash>,
    /// Service targeted by the read.
    pub service_name: String,
    /// Active service version used for the read.
    pub service_version: String,
    /// Handler servicing the request.
    pub handler_name: String,
    /// Handler class for the request.
    pub handler_class: SoracloudLocalReadKind,
    /// HTTP method or logical read method used to invoke the handler.
    pub request_method: String,
    /// Full request path as received by Torii.
    pub request_path: String,
    /// Request path relative to the matched handler route.
    pub handler_path: String,
    /// Optional raw query string without the leading `?`.
    pub request_query: Option<String>,
    /// Canonicalized request headers made visible to the handler.
    pub request_headers: BTreeMap<String, String>,
    /// Opaque request payload bytes supplied to the handler.
    pub request_body: Vec<u8>,
    /// Deterministic commitment over the request envelope.
    pub request_commitment: Hash,
}

/// Committed artifact/state binding attached to a certified local read response.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudLocalReadBinding {
    /// Binding name when the response is derived from authoritative service state.
    pub binding_name: Option<String>,
    /// State key when the response is derived from a specific state entry.
    pub state_key: Option<String>,
    /// Commitment for the bound state entry, when applicable.
    pub payload_commitment: Option<Hash>,
    /// Bound artifact digest when the response is served from hydrated local content.
    pub artifact_hash: Option<Hash>,
}

/// Shared response envelope for deterministic local Soracloud reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudLocalReadResponse {
    /// Raw response bytes emitted by the runtime.
    pub response_bytes: Vec<u8>,
    /// MIME type of the response payload, when known.
    pub content_type: Option<String>,
    /// Optional content encoding metadata for the response.
    pub content_encoding: Option<String>,
    /// Optional cache-control metadata for the response.
    pub cache_control: Option<String>,
    /// Committed bindings that certify the response payload.
    pub bindings: Vec<SoracloudLocalReadBinding>,
    /// Commitment over the response envelope.
    pub result_commitment: Hash,
    /// Certification mode selected for this read.
    pub certified_by: SoraCertifiedResponsePolicyV1,
    /// Optional receipt emitted for audit-style certifications.
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
}

/// Deterministic state mutation produced by ordered Soracloud execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudDeterministicStateMutation {
    /// Binding mutated by the runtime.
    pub binding_name: String,
    /// Canonical key scoped under the binding prefix.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Encryption contract enforced by the binding.
    pub encryption: SoraStateEncryptionV1,
    /// Declared payload size when the mutation upserts content.
    pub payload_bytes: Option<u64>,
    /// Deterministic commitment over the opaque payload.
    pub payload_commitment: Option<Hash>,
}

/// Shared request envelope for ordered Soracloud mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudOrderedMailboxExecutionRequest {
    /// Authoritative height pinned for the execution.
    pub observed_height: u64,
    /// Latest committed block hash visible to the executor.
    pub observed_block_hash: Option<Hash>,
    /// Deterministic Soracloud execution sequence used for receipts.
    pub execution_sequence: u64,
    /// Current deployment state for the target service.
    pub deployment: SoraServiceDeploymentStateV1,
    /// Admitted active bundle for the target service revision.
    pub bundle: SoraDeploymentBundleV1,
    /// Resolved target handler when it exists in the active bundle.
    pub handler: Option<SoraServiceHandlerV1>,
    /// Mailbox message being delivered through replicated progression.
    pub mailbox_message: SoraServiceMailboxMessageV1,
    /// Latest runtime state observed for the target service.
    pub runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Outstanding mailbox message count before this execution is applied.
    pub authoritative_pending_mailbox_messages: u32,
}

/// Deterministic result of ordered Soracloud mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudOrderedMailboxExecutionResult {
    /// Deterministic state mutations to apply to authoritative service state.
    pub state_mutations: Vec<SoracloudDeterministicStateMutation>,
    /// Cross-service messages emitted by the execution.
    pub outbound_mailbox_messages: Vec<SoraServiceMailboxMessageV1>,
    /// Runtime-state observation to persist after execution.
    pub runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Deterministic runtime receipt for the execution.
    pub runtime_receipt: SoraRuntimeReceiptV1,
}

/// Shared request envelope for deterministic apartment execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentExecutionRequest {
    /// Authoritative height pinned for the apartment execution.
    pub observed_height: u64,
    /// Latest committed block hash visible to the runtime.
    pub observed_block_hash: Option<Hash>,
    /// Apartment targeted by the runtime.
    pub apartment_name: String,
    /// Expected apartment process generation.
    pub process_generation: u64,
    /// Logical apartment operation to execute.
    pub operation: String,
    /// Deterministic commitment over the apartment request.
    pub request_commitment: Hash,
}

/// Shared result for deterministic apartment execution.
#[allow(missing_copy_implementations)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentExecutionResult {
    /// Latest apartment status reported by the runtime.
    pub status: SoraAgentRuntimeStatusV1,
    /// Optional committed checkpoint hash materialized by the operation.
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional committed journal hash materialized by the operation.
    pub journal_artifact_hash: Option<Hash>,
    /// Deterministic commitment over the apartment result.
    pub result_commitment: Hash,
}

/// Shared execution interface for the embedded Soracloud runtime.
pub trait SoracloudRuntime: SoracloudRuntimeReadHandle {
    /// Execute a deterministic local read against the committed runtime snapshot.
    fn execute_local_read(
        &self,
        request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError>;

    /// Execute an ordered mailbox message during replicated state progression.
    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError>;

    /// Execute deterministic apartment work owned by the embedded runtime manager.
    fn execute_apartment(
        &self,
        request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError>;
}

/// Shared Soracloud runtime trait object used by the core replicated execution path.
pub type SharedSoracloudRuntime = Arc<dyn SoracloudRuntime>;

impl SoracloudLocalReadKind {
    /// Return the Soracloud handler class represented by this local read kind.
    #[must_use]
    pub fn handler_class(self) -> SoraServiceHandlerClassV1 {
        match self {
            Self::Asset => SoraServiceHandlerClassV1::Asset,
            Self::Query => SoraServiceHandlerClassV1::Query,
        }
    }
}

impl SoracloudRuntimeExecutionErrorKind {
    /// Stable label used when hashing synthetic failure receipts.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::InvalidRequest => "invalid_request",
            Self::Internal => "internal",
        }
    }
}

impl SoracloudDeterministicStateMutation {
    /// Return `true` when this mutation writes payload bytes into authoritative service state.
    #[must_use]
    pub fn is_upsert(&self) -> bool {
        matches!(self.operation, SoraStateMutationOperationV1::Upsert)
    }
}

impl From<SoracloudLocalReadKind> for SoraServiceHandlerClassV1 {
    fn from(value: SoracloudLocalReadKind) -> Self {
        value.handler_class()
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
/// Bounded runtime write-back instruction set used for internal Soracloud integration points.
pub enum SoracloudRuntimeInstruction {
    /// Persist an updated runtime-state snapshot.
    SetRuntimeState(iroha_data_model::isi::soracloud::SetSoracloudRuntimeState),
    /// Persist an outbound cross-service mailbox message.
    RecordMailboxMessage(iroha_data_model::isi::soracloud::RecordSoracloudMailboxMessage),
    /// Persist an authoritative runtime receipt.
    RecordRuntimeReceipt(iroha_data_model::isi::soracloud::RecordSoracloudRuntimeReceipt),
}

impl SoracloudRuntimeInstruction {
    /// Convert the bounded runtime write-back into a regular instruction box.
    #[must_use]
    pub fn into_instruction_box(self) -> InstructionBox {
        match self {
            Self::SetRuntimeState(isi) => InstructionBox::from(isi),
            Self::RecordMailboxMessage(isi) => InstructionBox::from(isi),
            Self::RecordRuntimeReceipt(isi) => InstructionBox::from(isi),
        }
    }
}
