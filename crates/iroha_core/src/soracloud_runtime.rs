//! Shared Soracloud runtime snapshot types, generated HF manifests, and execution traits.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use crate::state::WorldReadOnly;
use iroha_crypto::Hash;
use iroha_data_model::{
    isi::InstructionBox,
    name::Name,
    smart_contract::manifest::EntryPointKind,
    soracloud::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentToolCapabilityV1,
        AgentUpgradePolicyV1, SORA_CONTAINER_MANIFEST_VERSION_V1,
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
        SoraAgentRuntimeStatusV1, SoraArtifactKindV1, SoraCapabilityPolicyV1,
        SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1, SoraContainerManifestV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraHfPlacementHostAssignmentV1,
        SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
        SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseStatusV1, SoraHfSourceStatusV1,
        SoraLifecycleHooksV1, SoraNetworkPolicyV1, SoraPrivateInferenceCheckpointV1,
        SoraPrivateInferenceSessionStatusV1, SoraResourceLimitsV1, SoraRolloutPolicyV1,
        SoraRouteTargetV1, SoraRouteVisibilityV1, SoraRuntimeReceiptV1,
        SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHandlerV1,
        SoraServiceHealthStatusV1, SoraServiceMailboxMessageV1, SoraServiceManifestV1,
        SoraServiceRuntimeStateV1, SoraStateEncryptionV1, SoraStateMutationOperationV1,
        SoraTlsModeV1, SoraUploadedModelKeyEncapsulationV1, SoraUploadedModelKeyWrapAeadV1,
    },
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

const HF_GENERATED_SERVICE_VERSION_V1: &str = "hf.generated.v1";
const HF_GENERATED_SERVICE_MARKER_ENV: &str = "SORACLOUD_HF_GENERATED";
const HF_GENERATED_SOURCE_ID_ENV: &str = "SORACLOUD_HF_SOURCE_ID";
const HF_GENERATED_REPO_ID_ENV: &str = "SORACLOUD_HF_REPO_ID";
const HF_GENERATED_REVISION_ENV: &str = "SORACLOUD_HF_REVISION";
const HF_GENERATED_MODEL_NAME_ENV: &str = "SORACLOUD_HF_MODEL_NAME";
const HF_GENERATED_ROUTE_SUFFIX: &str = ".hf.soracloud.internal";
const HF_GENERATED_ENTRYPOINT_INFER: &str = "infer";
const HF_GENERATED_ENTRYPOINT_METADATA: &str = "metadata";

/// Canonical Hugging Face source markers embedded into generated Soracloud service bundles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudHfGeneratedSourceBinding {
    /// Stable canonical source identifier.
    pub source_id: String,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision resolved by the control plane.
    pub resolved_revision: String,
    /// Normalized Soracloud model name.
    pub model_name: String,
}

fn hf_generated_entrypoint(name: &str, entry_pc: u64) -> ivm::EmbeddedEntrypointDescriptor {
    ivm::EmbeddedEntrypointDescriptor {
        name: name.to_owned(),
        kind: EntryPointKind::Public,
        permission: None,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc,
    }
}

fn hf_generated_internal_host(service_name: &Name) -> String {
    format!(
        "{}{HF_GENERATED_ROUTE_SUFFIX}",
        service_name.as_ref().replace('_', "-")
    )
}

/// Return the deterministic shared IVM artifact used by generated HF services.
#[must_use]
pub fn soracloud_hf_generated_service_contract_artifact() -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let contract_interface = ivm::EmbeddedContractInterfaceV1 {
        compiler_fingerprint: "iroha-soracloud-hf-generated".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: [
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_INFER, 0),
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_METADATA, 0),
        ]
        .into_iter()
        .collect(),
    };
    let mut bytes = metadata.encode();
    bytes.extend_from_slice(&contract_interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

/// Lease term used for deterministic HF-generated agent apartments.
pub const HF_GENERATED_AGENT_LEASE_TICKS: u64 = 86_400;

/// Autonomy budget applied to deterministic HF-generated agent apartments.
pub const HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS: u64 = 1_000;

/// Build the canonical generated Soracloud service bundle used for HF-backed deployments.
#[must_use]
pub fn build_soracloud_hf_generated_service_bundle(
    service_name: Name,
    source_id: &str,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
) -> SoraDeploymentBundleV1 {
    let bundle_bytes = soracloud_hf_generated_service_contract_artifact();
    let bundle_hash = Hash::new(&bundle_bytes);
    let mut env = BTreeMap::new();
    env.insert(HF_GENERATED_SERVICE_MARKER_ENV.to_owned(), "1".to_owned());
    env.insert(HF_GENERATED_SOURCE_ID_ENV.to_owned(), source_id.to_owned());
    env.insert(HF_GENERATED_REPO_ID_ENV.to_owned(), repo_id.to_owned());
    env.insert(
        HF_GENERATED_REVISION_ENV.to_owned(),
        resolved_revision.to_owned(),
    );
    env.insert(
        HF_GENERATED_MODEL_NAME_ENV.to_owned(),
        model_name.to_owned(),
    );

    let container = SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash,
        bundle_path: "/bundles/hf_generated_inference.to".to_owned(),
        entrypoint: HF_GENERATED_ENTRYPOINT_INFER.to_owned(),
        args: Vec::new(),
        env,
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Isolated,
            allow_wallet_signing: false,
            allow_state_writes: false,
            allow_model_inference: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(500).expect("non-zero cpu budget"),
            memory_bytes: NonZeroU64::new(256 * 1024 * 1024).expect("non-zero memory budget"),
            ephemeral_storage_bytes: NonZeroU64::new(128 * 1024 * 1024)
                .expect("non-zero storage budget"),
            max_open_files: NonZeroU32::new(256).expect("non-zero open-file cap"),
            max_tasks: NonZeroU16::new(32).expect("non-zero task cap"),
        },
        lifecycle: SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(15).expect("non-zero start grace"),
            stop_grace_secs: NonZeroU32::new(10).expect("non-zero stop grace"),
            healthcheck_path: Some("/healthz".to_owned()),
        },
    };
    let container_manifest_hash = Hash::new(Encode::encode(&container));
    let route_host = hf_generated_internal_host(&service_name);
    let service = SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name,
        service_version: HF_GENERATED_SERVICE_VERSION_V1.to_owned(),
        container: SoraContainerManifestRefV1 {
            manifest_hash: container_manifest_hash,
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(1).expect("non-zero replicas"),
        route: Some(SoraRouteTargetV1 {
            host: route_host,
            path_prefix: "/".to_owned(),
            service_port: NonZeroU16::new(8080).expect("non-zero port"),
            visibility: SoraRouteVisibilityV1::Internal,
            tls_mode: SoraTlsModeV1::Disabled,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 0,
            max_unavailable_replicas: 0,
            health_window_secs: NonZeroU32::new(30).expect("non-zero health window"),
            automatic_rollback_failures: NonZeroU32::new(1).expect("non-zero rollback failures"),
        },
        state_bindings: Vec::new(),
        handlers: vec![
            SoraServiceHandlerV1 {
                handler_name: "infer".parse().expect("valid literal handler name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: HF_GENERATED_ENTRYPOINT_INFER.to_owned(),
                route_path: Some("/infer".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "metadata".parse().expect("valid literal handler name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: HF_GENERATED_ENTRYPOINT_METADATA.to_owned(),
                route_path: Some("/metadata".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
        ],
        artifacts: Vec::new(),
    };

    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
}

/// Build the canonical generated agent apartment manifest used for HF-bound agents.
#[must_use]
pub fn build_soracloud_hf_generated_agent_manifest(
    apartment_name: Name,
    service_bundle: &SoraDeploymentBundleV1,
) -> AgentApartmentManifestV1 {
    let service_host = service_bundle
        .service
        .route
        .as_ref()
        .map(|route| route.host.clone())
        .unwrap_or_else(|| hf_generated_internal_host(&service_bundle.service.service_name));

    AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name,
        container: SoraContainerManifestRefV1 {
            manifest_hash: service_bundle.container_manifest_hash(),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![
            AgentToolCapabilityV1 {
                tool: "soracloud.hf.infer".to_owned(),
                max_invocations_per_epoch: NonZeroU32::new(10_000)
                    .expect("non-zero infer invocation limit"),
                allow_network: true,
                allow_filesystem_write: false,
            },
            AgentToolCapabilityV1 {
                tool: "soracloud.hf.metadata".to_owned(),
                max_invocations_per_epoch: NonZeroU32::new(10_000)
                    .expect("non-zero metadata invocation limit"),
                allow_network: true,
                allow_filesystem_write: false,
            },
        ],
        policy_capabilities: vec![
            "agent.autonomy.allow"
                .parse()
                .expect("valid literal policy capability"),
            "agent.autonomy.run"
                .parse()
                .expect("valid literal policy capability"),
        ],
        spend_limits: Vec::new(),
        state_quota_bytes: NonZeroU64::new(16 * 1024 * 1024).expect("non-zero state quota"),
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![service_host]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    }
}

/// Extract canonical HF source markers from a generated Soracloud service bundle.
#[must_use]
pub fn soracloud_hf_generated_source_binding(
    bundle: &SoraDeploymentBundleV1,
) -> Option<SoracloudHfGeneratedSourceBinding> {
    if bundle.service.service_version != HF_GENERATED_SERVICE_VERSION_V1 {
        return None;
    }
    if bundle.container.runtime != SoraContainerRuntimeV1::Ivm
        || !bundle.container.capabilities.allow_model_inference
        || bundle.container.capabilities.allow_state_writes
        || bundle.container.capabilities.allow_model_training
    {
        return None;
    }
    if bundle.container.bundle_hash != Hash::new(soracloud_hf_generated_service_contract_artifact())
    {
        return None;
    }

    let source_id = bundle
        .container
        .env
        .get(HF_GENERATED_SOURCE_ID_ENV)?
        .clone();
    let repo_id = bundle.container.env.get(HF_GENERATED_REPO_ID_ENV)?.clone();
    let resolved_revision = bundle.container.env.get(HF_GENERATED_REVISION_ENV)?.clone();
    let model_name = bundle
        .container
        .env
        .get(HF_GENERATED_MODEL_NAME_ENV)?
        .clone();
    let marker = bundle.container.env.get(HF_GENERATED_SERVICE_MARKER_ENV)?;
    if marker != "1" {
        return None;
    }

    Some(SoracloudHfGeneratedSourceBinding {
        source_id,
        repo_id,
        resolved_revision,
        model_name,
    })
}

/// Return the synthesized HF bundle bytes when a service bundle is one of the canonical generated
/// HF service bundles.
#[must_use]
pub fn soracloud_hf_generated_bundle_payload_if_applicable(
    bundle: &SoraDeploymentBundleV1,
) -> Option<Vec<u8>> {
    soracloud_hf_generated_source_binding(bundle)
        .map(|_binding| soracloud_hf_generated_service_contract_artifact())
}

/// Resolve the authoritative active HF placement serving a generated service binding.
pub fn resolve_generated_hf_active_placement(
    world: &impl WorldReadOnly,
    service_name: &str,
    source_id: &str,
) -> Result<Option<SoraHfPlacementRecordV1>, String> {
    let mut matching_pool_ids = BTreeSet::new();
    for ((_member_pool_id, _account_id), member) in world.soracloud_hf_shared_lease_members().iter()
    {
        if member.status != SoraHfSharedLeaseMemberStatusV1::Active
            || !member.service_bindings.contains(service_name)
            || member.source_id.to_string() != source_id
        {
            continue;
        }
        let Some(pool) = world.soracloud_hf_shared_lease_pools().get(&member.pool_id) else {
            continue;
        };
        if pool.source_id == member.source_id
            && matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
            )
        {
            matching_pool_ids.insert(member.pool_id.clone());
        }
    }

    if matching_pool_ids.len() > 1 {
        return Err(format!(
            "generated HF service `{service_name}` is bound to multiple active lease pools for source `{source_id}`"
        ));
    }

    let Some(pool_id) = matching_pool_ids.into_iter().next() else {
        return Ok(None);
    };
    let Some(placement) = world.soracloud_hf_placements().get(&pool_id).cloned() else {
        return Err(format!(
            "generated HF service `{service_name}` is missing an authoritative placement for pool `{pool_id}`"
        ));
    };
    Ok(Some(placement))
}

/// Resolve the current authoritative primary host for a generated HF service.
pub fn resolve_generated_hf_primary_assignment(
    world: &impl WorldReadOnly,
    service_name: &str,
    source_id: &str,
) -> Result<Option<SoraHfPlacementHostAssignmentV1>, String> {
    let Some(placement) = resolve_generated_hf_active_placement(world, service_name, source_id)?
    else {
        return Ok(None);
    };
    Ok(placement
        .assigned_hosts
        .iter()
        .find(|assignment| {
            assignment.role == SoraHfPlacementHostRoleV1::Primary
                && assignment.status == SoraHfPlacementHostStatusV1::Warm
        })
        .cloned())
}

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

/// Schema version for [`SoracloudApartmentAutonomyExecutionSummaryV1`].
pub const SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1: u16 = 1;

/// One successful service step executed inside a generated apartment autonomy workflow.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub struct SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
    /// Zero-based workflow step index.
    pub step_index: u32,
    /// Optional stable workflow step identifier.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    /// Deterministic commitment over the local-read request for this step.
    pub request_commitment: Hash,
    /// Deterministic commitment over the step result.
    pub result_commitment: Hash,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
}

/// Node-local execution summary for one approved apartment autonomy run.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub struct SoracloudApartmentAutonomyExecutionSummaryV1 {
    /// Schema version; must equal
    /// [`SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1`].
    pub schema_version: u16,
    /// Apartment that executed the run.
    pub apartment_name: String,
    /// Stable authoritative autonomy-run identifier.
    pub run_id: String,
    /// Bound service name used for execution, when one was resolved locally.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    /// Bound service version used for execution, when one was resolved locally.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    /// Service handler used for execution, when one was resolved locally.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub handler_name: Option<String>,
    /// Whether the runtime obtained a successful inference result.
    pub succeeded: bool,
    /// Deterministic commitment over the execution outcome.
    pub result_commitment: Hash,
    /// Optional raw checkpoint artifact hash produced by the run.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Successful workflow steps executed before the final response was produced.
    #[norito(default)]
    pub workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
    /// Human-readable execution error, when the run failed locally.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Runtime-manager materialization state for a canonical Hugging Face source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "status", content = "value")]
pub enum SoracloudRuntimeHfSourceStatus {
    /// The canonical import has no runtime-visible bindings yet.
    PendingImport,
    /// A shared lease is bound, but no runtime service/apartment is materialized yet.
    PendingDeployment,
    /// Runtime materialization exists, but local artifact hydration is incomplete.
    Hydrating,
    /// The runtime can serve the source from the local materialized snapshot.
    Ready,
    /// The canonical source is blocked on a recorded failure.
    Failed,
    /// The canonical source was retired and should not accept fresh joins.
    Retired,
}

/// Runtime-manager projection for one canonical Hugging Face source.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SoracloudRuntimeHfSourcePlan {
    /// Stable canonical source identifier.
    pub source_id: String,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision resolved by the control plane.
    pub resolved_revision: String,
    /// Normalized Soracloud model name.
    pub model_name: String,
    /// Runtime adapter selected for the source.
    pub adapter_id: String,
    /// Authoritative world-state lifecycle status.
    pub authoritative_status: SoraHfSourceStatusV1,
    /// Runtime-manager materialization status for the source.
    pub runtime_status: SoracloudRuntimeHfSourceStatus,
    /// Number of shared-lease pools currently tracked for the source.
    pub pool_count: u32,
    /// Number of active or draining pools still visible to the runtime manager.
    pub active_pool_count: u32,
    /// Aggregate active shared-lease membership count across tracked pools.
    pub active_member_count: u32,
    /// Number of queued next-window sponsors for the source.
    pub queued_window_count: u32,
    /// Number of distinct bound Soracloud services.
    pub bound_service_count: u32,
    /// Bound Soracloud service names in deterministic order.
    #[norito(default)]
    pub bound_service_names: Vec<String>,
    /// Number of bound services already present in the runtime snapshot.
    pub materialized_service_count: u32,
    /// Materialized service names in deterministic order.
    #[norito(default)]
    pub materialized_service_names: Vec<String>,
    /// Number of materialized services still hydrating their artifacts.
    pub hydrating_service_count: u32,
    /// Number of distinct bound Soracloud apartments.
    pub bound_apartment_count: u32,
    /// Bound apartment names in deterministic order.
    #[norito(default)]
    pub bound_apartment_names: Vec<String>,
    /// Number of bound apartments already present in the runtime snapshot.
    pub materialized_apartment_count: u32,
    /// Materialized apartment names in deterministic order.
    #[norito(default)]
    pub materialized_apartment_names: Vec<String>,
    /// Number of missing bundle cache entries across bound services.
    pub bundle_cache_miss_count: u32,
    /// Number of missing non-bundle artifact cache entries across bound services.
    pub artifact_cache_miss_count: u32,
    /// Latest authoritative failure string, when present.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
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
    /// Runtime-manager Hugging Face source projections keyed by canonical source id.
    #[norito(default)]
    pub hf_sources: BTreeMap<String, SoracloudRuntimeHfSourcePlan>,
}

impl Default for SoracloudRuntimeSnapshot {
    fn default() -> Self {
        Self {
            schema_version: 1,
            observed_height: 0,
            observed_block_hash: None,
            services: BTreeMap::new(),
            apartments: BTreeMap::new(),
            hf_sources: BTreeMap::new(),
        }
    }
}

/// Read-only Soracloud runtime handle exposed to Torii and other consumers.
pub trait SoracloudRuntimeReadHandle: Send + Sync {
    /// Return the latest node-local runtime materialization snapshot.
    fn snapshot(&self) -> SoracloudRuntimeSnapshot;

    /// Return the local runtime-manager state directory.
    fn state_dir(&self) -> PathBuf;

    /// Return the Soracloud-upload recipient advertised for user model uploads, when available.
    fn uploaded_model_encryption_recipient(
        &self,
    ) -> Option<SoracloudUploadedModelEncryptionRecipient> {
        None
    }

    /// Return the local peer id, when the runtime knows its host identity.
    fn local_peer_id(&self) -> Option<String> {
        None
    }

    /// Return the maximum time Torii should wait for an internal Soracloud proxy read.
    fn local_read_proxy_timeout(&self) -> Duration {
        Duration::from_secs(10)
    }

    /// Report a failed generated-HF proxy read targeting the authoritative primary host.
    fn report_generated_hf_proxy_failure(
        &self,
        _request: &SoracloudLocalReadRequest,
        _target_peer_id: &str,
        _error: &SoracloudRuntimeExecutionError,
    ) {
    }

    /// Request authoritative HF host reconciliation after ingress observes routing failure.
    fn request_generated_hf_reconcile(
        &self,
        _request: &SoracloudLocalReadRequest,
        _error: &SoracloudRuntimeExecutionError,
    ) {
    }
}

/// Node-local uploaded-model encryption recipient descriptor exposed by the runtime handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudUploadedModelEncryptionRecipient {
    /// Schema version used for the advertised recipient record.
    pub schema_version: u16,
    /// Stable recipient key identifier.
    pub key_id: String,
    /// Recipient key version under the same `key_id`.
    pub key_version: NonZeroU32,
    /// Key-encapsulation suite expected by the recipient.
    pub kem: SoraUploadedModelKeyEncapsulationV1,
    /// AEAD suite expected by the recipient.
    pub aead: SoraUploadedModelKeyWrapAeadV1,
    /// Raw public key bytes used for upload-time envelope encryption.
    pub public_key_bytes: Vec<u8>,
    /// Commitment over the public key bytes.
    pub public_key_fingerprint: Hash,
}

/// Shared Soracloud runtime handle type used across crate boundaries.
pub type SharedSoracloudRuntimeHandle = Arc<dyn SoracloudRuntimeReadHandle>;

/// Coarse execution failure category for embedded Soracloud runtime requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudRuntimeExecutionErrorKind {
    /// The runtime cannot execute the request in the current node process.
    Unavailable,
    /// The request is structurally invalid for the configured runtime surface.
    InvalidRequest,
    /// The runtime hit an internal execution failure.
    Internal,
}

/// Structured error returned by the shared Soracloud runtime execution trait.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudLocalReadKind {
    /// Static asset read bound to committed artifacts.
    Asset,
    /// Read-only query bound to the committed state snapshot.
    Query,
}

/// Shared request envelope for deterministic local Soracloud reads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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

/// Schema version for peer-to-peer Soracloud local-read proxy requests.
pub const SORACLOUD_LOCAL_READ_PROXY_REQUEST_VERSION_V1: u16 = 1;
/// Schema version for peer-to-peer Soracloud local-read proxy responses.
pub const SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1: u16 = 1;

/// Peer-to-peer local-read proxy request sent to the authoritative primary host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadProxyRequestV1 {
    /// Version of the proxy request envelope.
    pub schema_version: u16,
    /// Correlation id chosen by the ingress node.
    pub request_id: Hash,
    /// Canonical local-read request to execute on the primary host.
    pub request: SoracloudLocalReadRequest,
}

/// Outcome for a peer-to-peer local-read proxy request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudLocalReadProxyOutcomeV1 {
    /// Successful local-read execution.
    Ok(SoracloudLocalReadResponse),
    /// Failed local-read execution.
    Err(SoracloudRuntimeExecutionError),
}

/// Peer-to-peer local-read proxy response sent back to the ingress node.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadProxyResponseV1 {
    /// Version of the proxy response envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Execution result returned by the primary host.
    pub outcome: SoracloudLocalReadProxyOutcomeV1,
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

/// Runtime-owned private-inference action executed against authoritative Soracloud state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoracloudPrivateInferenceExecutionAction {
    /// Start deterministic private execution and produce an awaiting-decryption checkpoint.
    Start,
    /// Release output for an already materialized awaiting-decryption checkpoint.
    Release {
        /// Decryption request that authorizes release of the checkpoint output.
        decrypt_request_id: String,
    },
}

/// Shared request envelope for deterministic uploaded-model private inference execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateInferenceExecutionRequest {
    /// Authoritative height pinned for the private execution.
    pub observed_height: u64,
    /// Latest committed block hash visible to the runtime.
    pub observed_block_hash: Option<Hash>,
    /// Apartment targeted by the private execution.
    pub apartment_name: String,
    /// Expected apartment process generation.
    pub process_generation: u64,
    /// Stable private session identifier.
    pub session_id: String,
    /// Requested runtime action.
    pub action: SoracloudPrivateInferenceExecutionAction,
    /// Deterministic commitment over the private-runtime request.
    pub request_commitment: Hash,
}

/// Shared result for deterministic uploaded-model private inference execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateInferenceExecutionResult {
    /// Terminal or intermediate session status produced by the runtime.
    pub status: SoraPrivateInferenceSessionStatusV1,
    /// Updated session receipt root.
    pub receipt_root: Hash,
    /// Updated cumulative XOR nanos charged for the session.
    pub xor_cost_nanos: u128,
    /// Deterministic checkpoint emitted by the runtime.
    pub checkpoint: SoraPrivateInferenceCheckpointV1,
    /// Deterministic commitment over the runtime outcome.
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

    /// Execute deterministic private uploaded-model runtime progression.
    fn execute_private_inference(
        &self,
        request: SoracloudPrivateInferenceExecutionRequest,
    ) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError>;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State, state::World};
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        soracloud::{
            SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1, SoraHfBackendFamilyV1, SoraHfModelFormatV1,
            SoraHfPlacementHostAssignmentV1, SoraHfPlacementStatusV1, SoraHfResourceProfileV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1,
        },
        sorafs::pin_registry::StorageClass,
    };

    fn seed_generated_hf_world_with_primary(
        primary_peer_id: &str,
    ) -> (World, String, String, String) {
        let mut world = World::new();
        let service_name: Name = "hf_service".parse().expect("valid service name");
        let service_name_string = service_name.as_ref().to_owned();
        let source_id = Hash::new(b"hf-source");
        let pool_id = Hash::new(b"hf-pool");
        let primary_validator = AccountId::new(KeyPair::random().public_key().clone());
        let member_account = AccountId::new(KeyPair::random().public_key().clone());
        let bundle = build_soracloud_hf_generated_service_bundle(
            service_name.clone(),
            &source_id.to_string(),
            "openai/gpt-oss",
            "main",
            "gpt-oss",
        );
        let service_version = bundle.service.service_version.clone();

        world.soracloud_service_revisions_mut_for_testing().insert(
            (service_name_string.clone(), service_version.clone()),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                service_name,
                SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                    current_service_version: service_version.clone(),
                    current_service_manifest_hash: bundle.service_manifest_hash(),
                    current_container_manifest_hash: bundle.container_manifest_hash(),
                    revision_count: 1,
                    process_generation: 1,
                    process_started_sequence: 1,
                    active_rollout: None,
                    last_rollout: None,
                    service_name: bundle.service.service_name.clone(),
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
                    storage_class: StorageClass::Warm,
                    lease_asset_definition_id: AssetDefinitionId::new(
                        "wonderland".parse().expect("domain"),
                        "xor".parse().expect("asset"),
                    ),
                    base_fee_nanos: 10_000,
                    lease_term_ms: 60_000,
                    window_started_at_ms: 1,
                    window_expires_at_ms: 60_001,
                    active_member_count: 1,
                    status: SoraHfSharedLeaseStatusV1::Active,
                    queued_next_window: None,
                },
            );
        world
            .soracloud_hf_shared_lease_members_mut_for_testing()
            .insert(
                (pool_id.to_string(), member_account.to_string()),
                SoraHfSharedLeaseMemberV1 {
                    schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                    pool_id,
                    source_id,
                    account_id: member_account,
                    status: SoraHfSharedLeaseMemberStatusV1::Active,
                    joined_at_ms: 1,
                    updated_at_ms: 1,
                    total_paid_nanos: 10_000,
                    total_refunded_nanos: 0,
                    last_charge_nanos: 10_000,
                    total_compute_paid_nanos: 5_000,
                    total_compute_refunded_nanos: 0,
                    last_compute_charge_nanos: 5_000,
                    service_bindings: std::collections::BTreeSet::from([
                        service_name_string.clone()
                    ]),
                    apartment_bindings: std::collections::BTreeSet::new(),
                },
            );
        world.soracloud_hf_placements_mut_for_testing().insert(
            pool_id,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: Hash::new(b"hf-placement"),
                source_id,
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash: Hash::new(b"hf-seed"),
                resource_profile: SoraHfResourceProfileV1 {
                    required_model_bytes: 1024,
                    backend_family: SoraHfBackendFamilyV1::Transformers,
                    model_format: SoraHfModelFormatV1::Safetensors,
                    disk_cache_bytes_floor: 2048,
                    ram_bytes_floor: 2048,
                    vram_bytes_floor: 0,
                },
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: primary_validator,
                    peer_id: primary_peer_id.to_owned(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: "gpu.large".to_owned(),
                }],
                total_reservation_fee_nanos: 5_000,
                last_rebalance_at_ms: 1,
                last_error: None,
            },
        );

        (
            world,
            service_name_string,
            service_version,
            source_id.to_string(),
        )
    }

    #[test]
    fn generated_hf_service_bundle_is_admissible_and_tagged() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:1111111111111111111111111111111111111111111111111111111111111111#0001",
            "openai/gpt-oss",
            "main",
            "gpt-oss",
        );

        bundle
            .validate_for_admission()
            .expect("generated HF bundle should validate");
        assert!(bundle.container.capabilities.allow_model_inference);
        assert!(!bundle.container.capabilities.allow_state_writes);
        assert_eq!(
            bundle.service.service_version,
            HF_GENERATED_SERVICE_VERSION_V1
        );
        assert_eq!(
            bundle
                .service
                .route
                .as_ref()
                .expect("generated route")
                .visibility,
            SoraRouteVisibilityV1::Internal
        );

        let binding = soracloud_hf_generated_source_binding(&bundle)
            .expect("generated bundle should expose HF markers");
        assert_eq!(binding.repo_id, "openai/gpt-oss");
        assert_eq!(binding.resolved_revision, "main");
        assert_eq!(binding.model_name, "gpt-oss");
    }

    #[test]
    fn generated_hf_bundle_payload_matches_declared_bundle_hash() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:2222222222222222222222222222222222222222222222222222222222222222#0002",
            "meta/llama",
            "1234abcd",
            "llama",
        );

        let payload = soracloud_hf_generated_bundle_payload_if_applicable(&bundle)
            .expect("generated HF bundle should synthesize payload");
        assert_eq!(Hash::new(&payload), bundle.container.bundle_hash);
    }

    #[test]
    fn generated_hf_agent_manifest_tracks_service_container_and_host() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_agent_service".parse().expect("valid service name"),
            "hash:3333333333333333333333333333333333333333333333333333333333333333#0003",
            "huggingface/smol",
            "rev-a",
            "smol",
        );
        let manifest = build_soracloud_hf_generated_agent_manifest(
            "hf_agent".parse().expect("valid apartment name"),
            &bundle,
        );

        manifest
            .validate()
            .expect("generated HF apartment manifest should validate");
        assert_eq!(
            manifest.container.manifest_hash,
            bundle.container_manifest_hash()
        );
        assert_eq!(
            manifest.network_egress,
            SoraNetworkPolicyV1::Allowlist(vec![
                bundle
                    .service
                    .route
                    .as_ref()
                    .expect("generated service route")
                    .host
                    .clone()
            ])
        );
        assert_eq!(manifest.tool_capabilities.len(), 2);
    }

    #[test]
    fn resolve_generated_hf_primary_assignment_returns_warm_primary_for_bound_service() {
        let primary_peer_id =
            iroha_data_model::peer::PeerId::from(KeyPair::random().public_key().clone())
                .to_string();
        let (world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query_handle);
        let view = state.view();

        let primary =
            resolve_generated_hf_primary_assignment(view.world(), &service_name, &source_id)
                .expect("primary lookup should succeed")
                .expect("generated service should resolve a primary assignment");

        assert_eq!(primary.peer_id, primary_peer_id);
        assert_eq!(primary.role, SoraHfPlacementHostRoleV1::Primary);
        assert_eq!(primary.status, SoraHfPlacementHostStatusV1::Warm);
    }
}
