//! Shared Soracloud runtime snapshot types, generated HF manifests, and execution traits.
use crate::state::WorldReadOnly;
use iroha_crypto::Hash;
use iroha_data_model::{
    isi::InstructionBox,
    name::Name,
    smart_contract::manifest::EntryPointKind,
    soracloud::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentToolCapabilityV1,
        AgentUpgradePolicyV1, SORA_CONTAINER_MANIFEST_VERSION_V1,
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
        SoraAgentRuntimeStatusV1, SoraArtifactKindV1, SoraCapabilityPolicyV1,
        SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1,
        SoraHfPlacementRecordV1, SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseStatusV1,
        SoraHfSourceStatusV1, SoraHttpServiceEconomicsV1, SoraInrouGuestIsaV1, SoraInrouGuestOsV1,
        SoraLeaseVolumeKindV1, SoraLifecycleHooksV1, SoraNetworkAllowlistEntryV1,
        SoraNetworkPolicyV1, SoraPrivateModelArtifactRefV1,
        SoraPrivateUploadedModelExecutionReceiptV1, SoraResourceLimitsV1, SoraRolloutPolicyV1,
        SoraRouteTargetV1, SoraRouteVisibilityV1, SoraRuntimeReceiptV1,
        SoraServiceDeploymentStateV1, SoraServiceExecutionPlaneV1, SoraServiceHandlerClassV1,
        SoraServiceHandlerV1, SoraServiceHealthStatusV1, SoraServiceLeaseStatusV1,
        SoraServiceMailboxMessageV1, SoraServiceManifestV1, SoraServiceRuntimeStateV1,
        SoraStateEncryptionV1, SoraStateMutationOperationV1, SoraTlsModeV1,
        SoraUploadedModelBundleV1, SoraUploadedModelKeyEncapsulationV1,
        SoraUploadedModelKeyWrapAeadV1, SoraUploadedModelRuntimeFormatV1,
    },
    sorafs::pin_registry::StorageClass,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::PathBuf,
    sync::Arc,
    time::Duration,
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
        kind: EntryPointKind::View,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
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
        max_cycles: iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND.get(),
        abi_version: 1,
    };
    let contract_interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "SoracloudRuntime".to_owned(),
        compiler_fingerprint: "iroha-soracloud-hf-generated".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: [
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_INFER, 0),
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_METADATA, 4),
        ]
        .into_iter()
        .collect(),
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut bytes = metadata.encode();
    bytes.extend_from_slice(&contract_interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
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
        inrou: None,
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        config_exports: Vec::new(),
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
        execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
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
        economics: SoraHttpServiceEconomicsV1::default(),
        state_bindings: Vec::new(),
        lease_volumes: Vec::new(),
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
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            service_host,
            [443],
        )]),
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
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::DeterministicService
        || bundle.container.runtime != SoraContainerRuntimeV1::Ivm
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
#[norito(deny_unknown_fields)]
pub enum SoracloudRuntimeRevisionRole {
    /// The currently active deployment revision.
    Active,
    /// A canary candidate revision that must be materialized during rollout.
    CanaryCandidate,
}
/// Node-local mailbox materialization metadata for a handler.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeArtifactPlan {
    /// Artifact class.
    pub kind: SoraArtifactKindV1,
    /// Content-addressed artifact digest.
    pub artifact_hash: String,
    /// Logical artifact path inside the service revision.
    pub artifact_path: String,
    /// Optional consuming handler.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Local cache path where the runtime manager expects the artifact.
    pub local_cache_path: String,
    /// Whether the artifact is already present in the node-local cache.
    pub available_locally: bool,
}
/// Node-local materialization plan for one active service revision.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
    /// Execution plane selected for this revision.
    pub execution_plane: SoraServiceExecutionPlaneV1,
    /// Bundle digest.
    pub bundle_hash: String,
    /// Bundle path declared by the container manifest.
    pub bundle_path: String,
    /// Entrypoint declared by the container manifest.
    pub entrypoint: String,
    /// Explicit Inrou VM metadata projected for hosted HTTP microVMs.
    #[norito(required)]
    pub inrou: Option<SoracloudRuntimeInrouPlan>,
    /// Node-local cache path for the executable bundle.
    pub bundle_cache_path: String,
    /// Whether the bundle is already present locally.
    pub bundle_available_locally: bool,
    /// Current deployment process generation when known for this revision.
    #[norito(required)]
    pub process_generation: Option<u64>,
    /// Desired replica count declared by the admitted service manifest.
    pub desired_replica_count: u16,
    /// Replica slots this runtime host is projecting locally for the revision.
    pub local_replica_slots: Vec<u16>,
    /// Replica-local runtime topology currently projected on this host for the revision.
    pub local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
    /// Current runtime health projection.
    pub health_status: SoraServiceHealthStatusV1,
    /// Current runtime load projection.
    pub load_factor_bps: u16,
    /// Pending mailbox count reported for this revision.
    pub reported_pending_mailbox_messages: u32,
    /// Pending mailbox messages currently stored in authoritative state.
    pub authoritative_pending_mailbox_messages: u32,
    /// Active rollout handle when this revision is part of a canary rollout.
    #[norito(required)]
    pub rollout_handle: Option<String>,
    /// Monotonic generation of committed service config updates.
    pub config_generation: u64,
    /// Monotonic generation of committed service secret updates.
    pub secret_generation: u64,
    /// Hosted-service quota class when the service uses the HTTP plane.
    #[norito(required)]
    pub quota_class: Option<String>,
    /// Effective hosted-service lease status at the observed sequence.
    #[norito(required)]
    pub service_lease_status: Option<SoraServiceLeaseStatusV1>,
    /// Sequence when hosted-service routing/materialization expires.
    #[norito(required)]
    pub lease_expires_sequence: Option<u64>,
    /// Remaining prepaid runtime balance estimated at snapshot build time.
    #[norito(required)]
    pub remaining_runtime_balance: Option<Quantity>,
    /// Number of committed service config entries projected into runtime materialization.
    pub config_entry_count: u32,
    /// Number of committed service secret entries projected into runtime materialization.
    pub secret_entry_count: u32,
    /// Explicit config exports declared by the admitted container manifest.
    pub config_exports: Vec<SoraConfigExportV1>,
    /// Whether ordinary handlers on this revision can read authoritative config payloads.
    pub supports_host_read_config: bool,
    /// Whether ordinary handlers on this revision can read authoritative secret envelopes.
    pub supports_host_read_secret_envelope: bool,
    /// Whether this revision exposes at least one handler allowed to read raw secret payload bytes.
    pub supports_private_secret_payload_reads: bool,
    /// Local directory where the revision plan is materialized.
    pub materialization_dir: String,
    /// Local directory containing canonical JSON config files for this revision.
    pub config_materialization_dir: String,
    /// Effective launch environment after applying explicit config env exports.
    pub effective_env: BTreeMap<String, String>,
    /// Local file containing the effective launch environment projection.
    pub effective_env_materialization_path: String,
    /// Local directory containing explicit config file exports for this revision.
    pub config_exports_materialization_dir: String,
    /// Local directory containing committed secret-envelope files for this revision.
    pub secret_envelopes_materialization_dir: String,
    /// Lease-backed mutable storage materialized for this revision.
    pub lease_volumes: Vec<SoracloudRuntimeLeaseVolumePlan>,
    /// Declared replicated handler mailboxes.
    pub mailboxes: Vec<SoracloudRuntimeMailboxPlan>,
    /// Referenced artifacts that still need local hydration.
    pub artifacts: Vec<SoracloudRuntimeArtifactPlan>,
}
/// Node-local materialization plan for one Inrou microVM guest.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeInrouPlan {
    /// Guest userspace profile expected by the VM image.
    pub guest_os: SoraInrouGuestOsV1,
    /// Guest ISA profile selected locally for this replica.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Kernel image path for the selected guest ISA inside the hydrated Soracloud bundle.
    pub kernel_image_path: String,
    /// Immutable base root filesystem image path for the selected guest ISA.
    pub rootfs_image_path: String,
    /// Optional initrd image path for the selected guest ISA.
    #[norito(required)]
    pub initrd_image_path: Option<String>,
    /// Optional bootstrap user-data overlay path inside the hydrated Soracloud bundle.
    #[norito(required)]
    pub bootstrap_user_data_path: Option<String>,
    /// SSH public keys injected into the guest bootstrap seed.
    pub ssh_authorized_keys: Vec<String>,
    /// Logical volume identifier used as the authoritative mutable root disk.
    pub root_volume_name: String,
}
/// Node-local materialization plan for one lease-backed service volume.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeLeaseVolumePlan {
    /// Logical volume identifier.
    pub volume_name: String,
    /// Soracloud lease-backed volume kind.
    pub kind: SoraLeaseVolumeKindV1,
    /// Requested Sorafs storage class.
    pub storage_class: StorageClass,
    /// Declared in-runtime mount path.
    pub mount_path: String,
    /// Maximum logical bytes retained for this volume.
    pub max_total_bytes: u64,
    /// Sequence when the authoritative volume lease expires.
    pub lease_expires_sequence: u64,
    /// Monotonic generation of the authoritative lease binding.
    pub authoritative_generation: u64,
    /// Node-local materialization directory used by the current host.
    ///
    /// `PersistentRootLeaseVolume` plans use a per-revision/per-replica namespace, while non-root
    /// service volumes are shared across replicas of the same revision by default.
    pub local_materialization_dir: String,
}
/// Node-local runtime topology projected for one hosted-HTTP replica slot.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeReplicaPlan {
    /// One-based replica slot within the revision.
    pub replica_slot: u16,
    /// Local directory where this replica slot is materialized.
    pub materialization_dir: String,
    /// Current runtime health projection for this replica slot.
    pub health_status: SoraServiceHealthStatusV1,
    /// Loopback listener currently exposed by this replica, when healthy.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Local process identifier when the replica is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Human-readable startup or healthcheck failure detail for the replica, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
/// Node-local materialization plan for an active agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
/// Schema version for persisted hosted-HTTP runtime state snapshots.
pub const SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1: u16 = 1;
/// Canonical runtime-state filename written beside hosted-HTTP service materializations.
pub const SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1: &str = "hosted_http_runtime.json";
/// Node-local state projected for one hosted-HTTP replica materialized by the local runtime manager.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostedHttpReplicaRuntimeStateV1 {
    /// One-based replica slot for the revision-local materialization.
    pub replica_slot: u16,
    /// Projected health state of the replica runtime.
    pub health_status: SoraServiceHealthStatusV1,
    /// Base URL for the loopback listener exposed by the replica process, when present.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Child process identifier while the replica is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Human-readable startup or healthcheck failure detail for this replica, when present.
    #[norito(required)]
    pub last_error: Option<String>,
    /// Timestamp when the replica state was last refreshed.
    pub updated_at_ms: u64,
}
/// Node-local state projected for a supervised hosted-HTTP Soracloud service revision.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostedHttpRuntimeStateV1 {
    /// Schema version for this runtime-state document.
    pub schema_version: u16,
    /// Service identifier.
    pub service_name: String,
    /// Materialized revision/version.
    pub service_version: String,
    /// Process generation currently hosted by the local runtime-manager.
    pub process_generation: u64,
    /// Projected health state of the child process.
    pub health_status: SoraServiceHealthStatusV1,
    /// Base URL for the loopback listener exposed by the child process.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Child process identifier while the process is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Total authoritative egress bytes accounted by the local supervisor.
    pub accounted_egress_bytes: u64,
    /// Healthy and unhealthy replica listeners currently materialized on this host for the revision.
    pub replicas: Vec<SoracloudHostedHttpReplicaRuntimeStateV1>,
    /// Human-readable startup or healthcheck failure detail, when present.
    #[norito(required)]
    pub last_error: Option<String>,
    /// Timestamp when the state file was last refreshed.
    pub updated_at_ms: u64,
}
/// Schema version for [`SoracloudApartmentAutonomyExecutionSummaryV1`].
pub const SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1: u16 = 1;
/// Maximum canonical JSON bytes persisted for one V1 apartment autonomy summary.
///
/// Runtime writers and every local control-plane reader share this ceiling so
/// corrupted state cannot turn a status read into an unbounded allocation.
pub const SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_MAX_BYTES_V1: u64 = 16 * 1024 * 1024;
/// One successful service step executed inside a generated apartment autonomy workflow.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
    /// Zero-based workflow step index.
    pub step_index: u32,
    /// Optional stable workflow step identifier.
    #[norito(required)]
    pub step_id: Option<String>,
    /// Deterministic commitment over the local-read request for this step.
    pub request_commitment: Hash,
    /// Deterministic commitment over the step result.
    pub result_commitment: Hash,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(required)]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(required)]
    pub response_text: Option<String>,
}
/// Node-local execution summary for one approved apartment autonomy run.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudApartmentAutonomyExecutionSummaryV1 {
    /// Schema version; must equal [`SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1`].
    pub schema_version: u16,
    /// Apartment that executed the run.
    pub apartment_name: String,
    /// Stable authoritative autonomy-run identifier.
    pub run_id: String,
    /// Bound service name used for execution, when one was resolved locally.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Bound service version used for execution, when one was resolved locally.
    #[norito(required)]
    pub service_version: Option<String>,
    /// Service handler used for execution, when one was resolved locally.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Whether the runtime obtained a successful inference result.
    pub succeeded: bool,
    /// Deterministic commitment over the execution outcome.
    pub result_commitment: Hash,
    /// Optional raw checkpoint artifact hash produced by the run.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(required)]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Successful workflow steps executed before the final response was produced.
    pub workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(required)]
    pub response_text: Option<String>,
    /// Human-readable execution error, when the run failed locally.
    #[norito(required)]
    pub error: Option<String>,
}
/// Error returned when a persisted autonomy execution summary is not bound to
/// the exact authoritative V1 run that owns its storage path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentAutonomySummaryValidationError(String);
impl std::fmt::Display for SoracloudApartmentAutonomySummaryValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for SoracloudApartmentAutonomySummaryValidationError {}
/// Derive the V1 commitment for a node-local apartment autonomy summary.
///
/// `process_generation` and `request_commitment` must come from authoritative
/// apartment state rather than from the node-local summary document. This
/// keeps summary reuse and control-plane reads bound to the approved run. The
/// preimage covers every summary field except the circular
/// `result_commitment` field itself.
pub fn derive_soracloud_apartment_autonomy_result_commitment_v1(
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
    process_generation: u64,
    request_commitment: Hash,
) -> Result<Hash, norito::json::Error> {
    let workflow_steps_commitment = summary
        .workflow_steps
        .iter()
        .map(|step| {
            Ok((
                step.step_index,
                step.step_id.as_deref(),
                step.request_commitment,
                step.result_commitment,
                step.runtime_receipt
                    .as_ref()
                    .map(|receipt| Encode::encode(receipt)),
                step.content_type.as_deref(),
                step.response_text.as_deref(),
                step.response_json
                    .as_ref()
                    .map(norito::json::to_string)
                    .transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, norito::json::Error>>()?;
    let runtime_receipt = summary
        .runtime_receipt
        .as_ref()
        .map(|receipt| Encode::encode(receipt));
    Ok(Hash::new(Encode::encode(&(
        "soracloud.apartment.autonomy.v1",
        (
            summary.schema_version,
            summary.apartment_name.as_str(),
            process_generation,
            summary.run_id.as_str(),
            request_commitment,
        ),
        (
            summary.service_name.as_deref(),
            summary.service_version.as_deref(),
            summary.handler_name.as_deref(),
            summary.succeeded,
        ),
        (
            summary.checkpoint_artifact_hash,
            runtime_receipt,
            workflow_steps_commitment,
        ),
        (
            summary.content_type.as_deref(),
            summary.response_text.as_deref(),
            summary
                .response_json
                .as_ref()
                .map(norito::json::to_string)
                .transpose()?,
            summary.error.as_deref(),
        ),
    ))))
}
/// Validate a persisted V1 apartment autonomy summary against its storage path
/// and the authoritative apartment/run values used to derive its commitment.
pub fn validate_soracloud_apartment_autonomy_execution_summary_v1(
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
    expected_apartment_name: &str,
    expected_run_id: &str,
    process_generation: u64,
    request_commitment: Hash,
) -> Result<(), SoracloudApartmentAutonomySummaryValidationError> {
    if summary.schema_version != SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1 {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary schema version {} does not match V1",
            summary.schema_version
        )));
    }
    if summary.apartment_name != expected_apartment_name {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary apartment `{}` does not match path apartment `{expected_apartment_name}`",
            summary.apartment_name
        )));
    }
    if summary.run_id != expected_run_id {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary run `{}` does not match path run `{expected_run_id}`",
            summary.run_id
        )));
    }
    let expected_result_commitment = derive_soracloud_apartment_autonomy_result_commitment_v1(
        summary,
        process_generation,
        request_commitment,
    )
    .map_err(|error| {
        SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary cannot derive its canonical V1 result commitment: {error}"
        ))
    })?;
    if summary.result_commitment != expected_result_commitment {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary result commitment {} does not match authoritative V1 commitment {expected_result_commitment}",
            summary.result_commitment
        )));
    }
    Ok(())
}
/// Runtime-manager materialization state for a canonical Hugging Face source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "status", content = "value")]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
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
    pub bound_service_names: Vec<String>,
    /// Number of bound services already present in the runtime snapshot.
    pub materialized_service_count: u32,
    /// Materialized service names in deterministic order.
    pub materialized_service_names: Vec<String>,
    /// Number of materialized services still hydrating their artifacts.
    pub hydrating_service_count: u32,
    /// Number of distinct bound Soracloud apartments.
    pub bound_apartment_count: u32,
    /// Bound apartment names in deterministic order.
    pub bound_apartment_names: Vec<String>,
    /// Number of bound apartments already present in the runtime snapshot.
    pub materialized_apartment_count: u32,
    /// Materialized apartment names in deterministic order.
    pub materialized_apartment_names: Vec<String>,
    /// Number of missing bundle cache entries across bound services.
    pub bundle_cache_miss_count: u32,
    /// Number of missing non-bundle artifact cache entries across bound services.
    pub artifact_cache_miss_count: u32,
    /// Latest authoritative failure string, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
/// Schema version for [`SoracloudRuntimeSnapshot`].
pub const SORACLOUD_RUNTIME_SNAPSHOT_VERSION_V1: u16 = 1;
/// Persisted snapshot of node-local Soracloud runtime materialization state.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeSnapshot {
    /// Schema version for the local runtime snapshot format.
    pub schema_version: u16,
    /// Height of the authoritative state view used to build this snapshot.
    pub observed_height: u64,
    /// Latest committed block hash at snapshot time, when present.
    #[norito(required)]
    pub observed_block_hash: Option<String>,
    /// Peer identity of the runtime host that produced this snapshot, when known.
    #[norito(required)]
    pub local_peer_id: Option<String>,
    /// Materialized active service revisions grouped by service name then version.
    pub services: BTreeMap<String, BTreeMap<String, SoracloudRuntimeServicePlan>>,
    /// Materialized active agent apartments keyed by apartment name.
    pub apartments: BTreeMap<String, SoracloudRuntimeApartmentPlan>,
    /// Runtime-manager Hugging Face source projections keyed by canonical source id.
    pub hf_sources: BTreeMap<String, SoracloudRuntimeHfSourcePlan>,
}
impl Default for SoracloudRuntimeSnapshot {
    fn default() -> Self {
        Self {
            schema_version: SORACLOUD_RUNTIME_SNAPSHOT_VERSION_V1,
            observed_height: 0,
            observed_block_hash: None,
            local_peer_id: None,
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
    /// Report a generated-HF proxy forwarding failure caused by the local assigned host before
    /// the request reached the authoritative primary.
    fn report_generated_hf_local_proxy_failure(
        &self,
        _request: &SoracloudLocalReadRequest,
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
    /// Request authoritative HF host reconciliation after ingress observes an assigned
    /// non-primary host answer a proxy request that targeted a different authoritative primary.
    ///
    /// Runtime implementations may also turn that authority drift into a model-host health
    /// signal for the unexpected responder before enqueuing reconciliation.
    fn request_generated_hf_proxy_responder_reconcile(
        &self,
        _request: &SoracloudLocalReadRequest,
        _responder_peer_id: &str,
        _expected_peer_id: &str,
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
/// Runtime version string for deterministic private uploaded-model execution v1.
pub const SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1: &str = "soracloud.quantized-cpu.v1";
/// Fixed rounding rule used by the v1 quantized CPU runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudQuantizedRoundingV1 {
    /// Arithmetic right shift after adding half the scale, with ties rounded away from zero.
    NearestAwayFromZero,
}
/// Deterministic quantized linear model accepted by the v1 private CPU runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudQuantizedCpuModelV1 {
    /// Number of signed 32-bit integer inputs.
    pub input_len: usize,
    /// Number of signed 32-bit integer outputs.
    pub output_len: usize,
    /// Row-major signed 8-bit weights, `output_len * input_len` entries.
    pub weights_i8: Vec<i8>,
    /// Signed 32-bit output biases.
    pub bias_i32: Vec<i32>,
    /// Non-negative right-shift applied after accumulation.
    pub output_shift: u8,
    /// Saturating lower bound for each output.
    pub output_min: i32,
    /// Saturating upper bound for each output.
    pub output_max: i32,
    /// Explicit rounding mode, fixed for v1.
    pub rounding: SoracloudQuantizedRoundingV1,
}
impl SoracloudQuantizedCpuModelV1 {
    /// Validate the deterministic quantized model shape and arithmetic bounds.
    pub fn validate(&self) -> Result<(), SoracloudRuntimeExecutionError> {
        if self.input_len == 0 || self.output_len == 0 {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model input_len and output_len must be non-zero",
            ));
        }
        if self.weights_i8.len() != self.input_len.saturating_mul(self.output_len) {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model weights must be row-major output_len * input_len",
            ));
        }
        if self.bias_i32.len() != self.output_len {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model bias length must equal output_len",
            ));
        }
        if self.output_shift > 30 {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model output_shift must be <= 30",
            ));
        }
        if self.output_min > self.output_max {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model output_min must be <= output_max",
            ));
        }
        Ok(())
    }
    /// Execute deterministic signed-integer inference on the CPU reference path.
    pub fn evaluate(&self, input: &[i32]) -> Result<Vec<i32>, SoracloudRuntimeExecutionError> {
        self.validate()?;
        if input.len() != self.input_len {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model input length does not match input_len",
            ));
        }
        let mut outputs = Vec::with_capacity(self.output_len);
        for output_index in 0..self.output_len {
            let row_start = output_index
                .checked_mul(self.input_len)
                .expect("validated model dimensions fit usize");
            let mut acc = i64::from(self.bias_i32[output_index]);
            for (input_index, input_value) in input.iter().enumerate() {
                let weight = self.weights_i8[row_start + input_index];
                acc = acc.saturating_add(i64::from(*input_value).saturating_mul(i64::from(weight)));
            }
            let rounded = round_quantized_accumulator(acc, self.output_shift, self.rounding);
            let saturated = rounded.clamp(i64::from(self.output_min), i64::from(self.output_max));
            outputs.push(i32::try_from(saturated).expect("clamped to i32 range"));
        }
        Ok(outputs)
    }
}
fn round_quantized_accumulator(
    value: i64,
    shift: u8,
    rounding: SoracloudQuantizedRoundingV1,
) -> i64 {
    if shift == 0 {
        return value;
    }
    match rounding {
        SoracloudQuantizedRoundingV1::NearestAwayFromZero => {
            let half = 1_i64 << (shift - 1);
            if value >= 0 {
                value.saturating_add(half) >> shift
            } else {
                -((-value).saturating_add(half) >> shift)
            }
        }
    }
}
fn append_private_model_commitment_part<T: Encode>(transcript: &mut Vec<u8>, value: &T) {
    transcript.extend(value.encode());
}
fn private_model_request_commitment(
    bundle: &SoraUploadedModelBundleV1,
    policy_id: &str,
    input_artifact: &SoraPrivateModelArtifactRefV1,
    input_commitment: Hash,
) -> Hash {
    let mut transcript = Vec::new();
    append_private_model_commitment_part(&mut transcript, &bundle.service_name);
    append_private_model_commitment_part(&mut transcript, &bundle.model_id);
    append_private_model_commitment_part(&mut transcript, &bundle.weight_version);
    append_private_model_commitment_part(
        &mut transcript,
        &SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_owned(),
    );
    append_private_model_commitment_part(&mut transcript, &policy_id.to_owned());
    append_private_model_commitment_part(&mut transcript, input_artifact);
    append_private_model_commitment_part(&mut transcript, &input_commitment);
    Hash::new(transcript)
}
fn private_model_result_commitment(
    output_artifact: &SoraPrivateModelArtifactRefV1,
    output_commitment: Hash,
) -> Hash {
    let mut transcript = Vec::new();
    append_private_model_commitment_part(
        &mut transcript,
        &SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_owned(),
    );
    append_private_model_commitment_part(&mut transcript, output_artifact);
    append_private_model_commitment_part(&mut transcript, &output_commitment);
    Hash::new(transcript)
}
/// Input envelope for deterministic private uploaded-model execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateUploadedModelExecutionRequestV1 {
    /// Admitted uploaded model package metadata.
    pub bundle: SoraUploadedModelBundleV1,
    /// Decryption policy approved for this execution.
    pub policy_id: String,
    /// Plaintext input visible only inside the private runtime boundary.
    pub plaintext_input_i32: Vec<i32>,
    /// Persisted encrypted input artifact reference.
    pub input_artifact: SoraPrivateModelArtifactRefV1,
    /// Persisted encrypted output artifact reference.
    pub output_artifact: SoraPrivateModelArtifactRefV1,
    /// Monotonic Soracloud sequence emitted by the execution path.
    pub emitted_sequence: u64,
}
/// Result emitted by the deterministic private uploaded-model CPU runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateUploadedModelExecutionResultV1 {
    /// Plaintext output visible only inside the private runtime boundary.
    pub plaintext_output_i32: Vec<i32>,
    /// Chain-facing receipt containing only commitments and encrypted artifact references.
    pub receipt: SoraPrivateUploadedModelExecutionReceiptV1,
}
/// Execute the deterministic quantized CPU runtime for a private uploaded model.
pub fn execute_private_uploaded_model_quantized_cpu_v1(
    model: &SoracloudQuantizedCpuModelV1,
    request: SoracloudPrivateUploadedModelExecutionRequestV1,
) -> Result<SoracloudPrivateUploadedModelExecutionResultV1, SoracloudRuntimeExecutionError> {
    request.bundle.validate().map_err(|err| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("invalid uploaded model bundle: {err}"),
        )
    })?;
    if request.bundle.runtime_format
        != SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "uploaded model bundle is not admitted for deterministic quantized CPU execution",
        ));
    }
    if request.policy_id != request.bundle.decryption_policy_ref {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "execution policy_id must match the uploaded model decryption policy",
        ));
    }
    request.input_artifact.validate().map_err(|err| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("invalid encrypted input artifact: {err}"),
        )
    })?;
    request.output_artifact.validate().map_err(|err| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("invalid encrypted output artifact: {err}"),
        )
    })?;
    let output = model.evaluate(&request.plaintext_input_i32)?;
    let input_commitment = Hash::new(request.plaintext_input_i32.encode());
    let output_commitment = Hash::new(output.encode());
    let request_commitment = private_model_request_commitment(
        &request.bundle,
        &request.policy_id,
        &request.input_artifact,
        input_commitment,
    );
    let result_commitment =
        private_model_result_commitment(&request.output_artifact, output_commitment);
    let mut receipt = SoraPrivateUploadedModelExecutionReceiptV1 {
        schema_version: SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
        receipt_id: Hash::prehashed([0; 32]),
        service_name: request.bundle.service_name,
        model_id: request.bundle.model_id,
        weight_version: request.bundle.weight_version,
        runtime_version: SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_owned(),
        model_manifest_digest: request.bundle.sorafs_manifest_digest,
        model_bundle_root: request.bundle.bundle_root,
        policy_id: request.policy_id,
        input_artifact: request.input_artifact,
        output_artifact: request.output_artifact,
        input_commitment,
        output_commitment,
        request_commitment,
        result_commitment,
        emitted_sequence: request.emitted_sequence,
    };
    receipt.receipt_id = Hash::new(receipt.encode());
    receipt.validate().map_err(|err| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!("invalid private uploaded model execution receipt: {err}"),
        )
    })?;
    Ok(SoracloudPrivateUploadedModelExecutionResultV1 {
        plaintext_output_i32: output,
        receipt,
    })
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
impl std::fmt::Display for SoracloudRuntimeExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let category = match self.kind {
            SoracloudRuntimeExecutionErrorKind::Unavailable => "unavailable",
            SoracloudRuntimeExecutionErrorKind::InvalidRequest => "invalid request",
            SoracloudRuntimeExecutionErrorKind::Internal => "internal",
        };
        write!(
            formatter,
            "Soracloud runtime {category} error: {}",
            self.message
        )
    }
}
impl std::error::Error for SoracloudRuntimeExecutionError {}
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
#[norito(deny_unknown_fields)]
pub struct SoracloudLocalReadBinding {
    /// Binding name when the response is derived from authoritative service state.
    #[norito(required)]
    pub binding_name: Option<String>,
    /// State key when the response is derived from a specific state entry.
    #[norito(required)]
    pub state_key: Option<String>,
    /// Commitment for the bound state entry, when applicable.
    #[norito(required)]
    pub payload_commitment: Option<Hash>,
    /// Bound artifact digest when the response is served from hydrated local content.
    #[norito(required)]
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
    /// Full payload bytes when the mutation upserts content.
    pub payload: Option<Vec<u8>>,
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
    /// Optional response payload returned by the executed handler.
    pub response_bytes: Vec<u8>,
    /// MIME type associated with `response_bytes`, when known.
    pub content_type: Option<String>,
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State, state::World};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        peer::PeerId,
        soracloud::{
            SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1, SoraHfBackendFamilyV1, SoraHfModelFormatV1,
            SoraHfPlacementHostAssignmentV1, SoraHfPlacementStatusV1, SoraHfResourceProfileV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1,
        },
        sorafs::pin_registry::{ManifestDigest, StorageClass},
    };
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("Soracloud runtime fixture key generation should succeed")
    }
    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }
    fn checked_peer_id() -> PeerId {
        PeerId::from(checked_keypair().public_key().clone())
    }
    fn sample_runtime_service_plan() -> SoracloudRuntimeServicePlan {
        SoracloudRuntimeServicePlan {
            service_name: "service".to_owned(),
            service_version: "1.0.0".to_owned(),
            role: SoracloudRuntimeRevisionRole::Active,
            traffic_percent: 100,
            runtime: SoraContainerRuntimeV1::Inrou,
            execution_plane: SoraServiceExecutionPlaneV1::HttpService,
            bundle_hash: "bundle-hash".to_owned(),
            bundle_path: "sorafs://bundle".to_owned(),
            entrypoint: "/sbin/init".to_owned(),
            inrou: Some(SoracloudRuntimeInrouPlan {
                guest_os: SoraInrouGuestOsV1::DebianSlim,
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
                kernel_image_path: "guest/vmlinuz".to_owned(),
                rootfs_image_path: "guest/rootfs.ext4".to_owned(),
                initrd_image_path: None,
                bootstrap_user_data_path: None,
                ssh_authorized_keys: Vec::new(),
                root_volume_name: "root".to_owned(),
            }),
            bundle_cache_path: "/runtime/cache/bundle".to_owned(),
            bundle_available_locally: true,
            process_generation: None,
            desired_replica_count: 1,
            local_replica_slots: vec![1],
            local_replicas: vec![SoracloudRuntimeReplicaPlan {
                replica_slot: 1,
                materialization_dir: "/runtime/services/service/1.0.0/replica-0001".to_owned(),
                health_status: SoraServiceHealthStatusV1::Unavailable,
                listen_base_url: None,
                pid: None,
                last_error: None,
            }],
            health_status: SoraServiceHealthStatusV1::Unavailable,
            load_factor_bps: 0,
            reported_pending_mailbox_messages: 0,
            authoritative_pending_mailbox_messages: 0,
            rollout_handle: None,
            config_generation: 0,
            secret_generation: 0,
            quota_class: None,
            service_lease_status: None,
            lease_expires_sequence: None,
            remaining_runtime_balance: None,
            config_entry_count: 0,
            secret_entry_count: 0,
            config_exports: Vec::new(),
            supports_host_read_config: false,
            supports_host_read_secret_envelope: false,
            supports_private_secret_payload_reads: false,
            materialization_dir: "/runtime/services/service/1.0.0".to_owned(),
            config_materialization_dir: "/runtime/services/service/1.0.0/config".to_owned(),
            effective_env: BTreeMap::new(),
            effective_env_materialization_path: "/runtime/services/service/1.0.0/env.json"
                .to_owned(),
            config_exports_materialization_dir: "/runtime/services/service/1.0.0/exports"
                .to_owned(),
            secret_envelopes_materialization_dir: "/runtime/services/service/1.0.0/secrets"
                .to_owned(),
            lease_volumes: Vec::new(),
            mailboxes: Vec::new(),
            artifacts: Vec::new(),
        }
    }
    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }
    #[test]
    fn runtime_snapshot_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudRuntimeSnapshot::default();
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical runtime snapshot");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeSnapshot>(canonical_value.clone())
                .expect("decode canonical runtime snapshot"),
            canonical
        );

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("snapshot JSON object")
            .insert("legacy_runtime".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeSnapshot>(unknown).is_err(),
            "same-version snapshots must reject unknown fields"
        );

        for required_field in ["observed_block_hash", "local_peer_id", "hf_sources"] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("snapshot JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeSnapshot>(missing).is_err(),
                "same-version snapshots must require the canonically emitted {required_field} field"
            );
        }
    }

    #[test]
    fn runtime_snapshot_nested_records_require_the_exact_v1_field_set() {
        let apartment = SoracloudRuntimeApartmentPlan {
            apartment_name: "apartment".to_owned(),
            manifest_hash: "manifest-hash".to_owned(),
            status: SoraAgentRuntimeStatusV1::Running,
            process_generation: 1,
            lease_expires_sequence: 100,
            last_active_sequence: 1,
            materialization_dir: "/runtime/apartments/apartment".to_owned(),
            pending_wallet_request_count: 0,
            pending_mailbox_message_count: 0,
            autonomy_budget_remaining_units: 0,
            approved_artifact_count: 0,
            autonomy_run_count: 0,
            revoked_policy_capability_count: 0,
        };
        let mut apartment_value =
            norito::json::to_value(&apartment).expect("encode canonical runtime apartment plan");
        apartment_value
            .as_object_mut()
            .expect("runtime apartment plan JSON object")
            .insert("legacy_status".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeApartmentPlan>(apartment_value).is_err(),
            "same-version runtime apartment plans must reject unknown fields"
        );

        let source = SoracloudRuntimeHfSourcePlan {
            source_id: "source".to_owned(),
            repo_id: "owner/model".to_owned(),
            resolved_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            model_name: "model".to_owned(),
            adapter_id: "adapter".to_owned(),
            authoritative_status: SoraHfSourceStatusV1::Ready,
            runtime_status: SoracloudRuntimeHfSourceStatus::Ready,
            pool_count: 0,
            active_pool_count: 0,
            active_member_count: 0,
            queued_window_count: 0,
            bound_service_count: 0,
            bound_service_names: Vec::new(),
            materialized_service_count: 0,
            materialized_service_names: Vec::new(),
            hydrating_service_count: 0,
            bound_apartment_count: 0,
            bound_apartment_names: Vec::new(),
            materialized_apartment_count: 0,
            materialized_apartment_names: Vec::new(),
            bundle_cache_miss_count: 0,
            artifact_cache_miss_count: 0,
            last_error: None,
        };
        let source_value =
            norito::json::to_value(&source).expect("encode canonical runtime HF source plan");
        assert!(
            source_value
                .get("last_error")
                .is_some_and(norito::json::Value::is_null),
            "canonical runtime HF source plans must emit an explicit null last_error"
        );
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(source_value.clone())
                .expect("decode canonical runtime HF source plan"),
            source
        );
        for required_field in [
            "bound_service_names",
            "materialized_service_names",
            "bound_apartment_names",
            "materialized_apartment_names",
            "last_error",
        ] {
            let mut missing = source_value.clone();
            missing
                .as_object_mut()
                .expect("runtime HF source plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(missing).is_err(),
                "same-version runtime HF source plans must require {required_field}"
            );
        }
        let mut unknown_source = source_value;
        unknown_source
            .as_object_mut()
            .expect("runtime HF source plan JSON object")
            .insert("legacy_source".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(unknown_source).is_err(),
            "same-version runtime HF source plans must reject unknown fields"
        );
    }

    #[test]
    fn runtime_service_plan_json_requires_the_exact_v1_field_set() {
        let canonical = sample_runtime_service_plan();
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical runtime service plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeServicePlan>(canonical_value.clone())
                .expect("decode canonical runtime service plan"),
            canonical
        );
        let canonical_role = canonical.role;
        let canonical_role_value =
            norito::json::to_value(&canonical_role).expect("encode canonical runtime role");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeRevisionRole>(canonical_role_value.clone())
                .expect("decode canonical runtime role"),
            canonical_role
        );
        let mut unknown_role = canonical_role_value;
        unknown_role
            .as_object_mut()
            .expect("runtime role JSON object")
            .insert("legacy_role".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeRevisionRole>(unknown_role).is_err(),
            "same-version runtime roles must reject unknown envelope fields"
        );
        for nullable_field in [
            "process_generation",
            "rollout_handle",
            "quota_class",
            "service_lease_status",
            "lease_expires_sequence",
            "remaining_runtime_balance",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime service plan must emit explicit null for {nullable_field}"
            );
        }

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("runtime service plan JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeServicePlan>(unknown).is_err(),
            "same-version runtime service plans must reject unknown fields"
        );

        for required_field in [
            "inrou",
            "process_generation",
            "desired_replica_count",
            "local_replica_slots",
            "local_replicas",
            "rollout_handle",
            "quota_class",
            "service_lease_status",
            "lease_expires_sequence",
            "remaining_runtime_balance",
            "config_exports",
            "effective_env",
            "effective_env_materialization_path",
            "config_exports_materialization_dir",
            "lease_volumes",
        ] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("runtime service plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeServicePlan>(missing).is_err(),
                "same-version runtime service plans must require {required_field}"
            );
        }

        let inrou = canonical.inrou.as_ref().expect("Inrou plan fixture");
        let inrou_value =
            norito::json::to_value(inrou).expect("encode canonical runtime Inrou plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeInrouPlan>(inrou_value.clone())
                .expect("decode canonical runtime Inrou plan"),
            *inrou
        );
        for nullable_field in ["initrd_image_path", "bootstrap_user_data_path"] {
            assert!(
                inrou_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime Inrou plan must emit explicit null for {nullable_field}"
            );
        }
        for required_field in [
            "initrd_image_path",
            "bootstrap_user_data_path",
            "ssh_authorized_keys",
        ] {
            let mut missing = inrou_value.clone();
            missing
                .as_object_mut()
                .expect("runtime Inrou plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeInrouPlan>(missing).is_err(),
                "same-version runtime Inrou plans must require {required_field}"
            );
        }
        let mut unknown_inrou = inrou_value;
        unknown_inrou
            .as_object_mut()
            .expect("runtime Inrou plan JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeInrouPlan>(unknown_inrou).is_err(),
            "same-version runtime Inrou plans must reject unknown fields"
        );

        let replica = canonical
            .local_replicas
            .first()
            .expect("runtime replica plan fixture");
        let replica_value =
            norito::json::to_value(replica).expect("encode canonical runtime replica plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeReplicaPlan>(replica_value.clone())
                .expect("decode canonical runtime replica plan"),
            *replica
        );
        for required_field in ["listen_base_url", "pid", "last_error"] {
            assert!(
                replica_value
                    .get(required_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime replica plan must emit explicit null for {required_field}"
            );
            let mut missing = replica_value.clone();
            missing
                .as_object_mut()
                .expect("runtime replica plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeReplicaPlan>(missing).is_err(),
                "same-version runtime replica plans must require {required_field}"
            );
        }
        let mut unknown_replica = replica_value;
        unknown_replica
            .as_object_mut()
            .expect("runtime replica plan JSON object")
            .insert("legacy_pid".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeReplicaPlan>(unknown_replica).is_err(),
            "same-version runtime replica plans must reject unknown fields"
        );
    }

    #[test]
    fn runtime_nested_plan_json_requires_the_exact_v1_field_set() {
        let mailbox = SoracloudRuntimeMailboxPlan {
            handler_name: "dispatch".to_owned(),
            queue_name: "dispatch-queue".to_owned(),
            max_pending_messages: 4,
            max_message_bytes: 1024,
            retention_blocks: 16,
        };
        let mut mailbox_value =
            norito::json::to_value(&mailbox).expect("encode canonical runtime mailbox plan");
        mailbox_value
            .as_object_mut()
            .expect("runtime mailbox plan JSON object")
            .insert("legacy_queue".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeMailboxPlan>(mailbox_value).is_err(),
            "same-version runtime mailbox plans must reject unknown fields"
        );

        let artifact = SoracloudRuntimeArtifactPlan {
            kind: SoraArtifactKindV1::Bundle,
            artifact_hash: "artifact-hash".to_owned(),
            artifact_path: "bundle.to".to_owned(),
            handler_name: None,
            local_cache_path: "/runtime/cache/artifact".to_owned(),
            available_locally: false,
        };
        let artifact_value =
            norito::json::to_value(&artifact).expect("encode canonical runtime artifact plan");
        assert!(
            artifact_value
                .get("handler_name")
                .is_some_and(norito::json::Value::is_null),
            "canonical runtime artifact plans must emit an explicit null handler_name"
        );
        let mut missing_handler = artifact_value.clone();
        missing_handler
            .as_object_mut()
            .expect("runtime artifact plan JSON object")
            .remove("handler_name");
        assert!(
            norito::json::from_value::<SoracloudRuntimeArtifactPlan>(missing_handler).is_err(),
            "same-version runtime artifact plans must require handler_name"
        );
        let mut unknown_artifact = artifact_value;
        unknown_artifact
            .as_object_mut()
            .expect("runtime artifact plan JSON object")
            .insert("legacy_handler".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeArtifactPlan>(unknown_artifact).is_err(),
            "same-version runtime artifact plans must reject unknown fields"
        );

        let lease = SoracloudRuntimeLeaseVolumePlan {
            volume_name: "root".to_owned(),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: 1024,
            lease_expires_sequence: 100,
            authoritative_generation: 1,
            local_materialization_dir: "/runtime/volumes/root".to_owned(),
        };
        let mut lease_value =
            norito::json::to_value(&lease).expect("encode canonical runtime lease-volume plan");
        lease_value
            .as_object_mut()
            .expect("runtime lease-volume plan JSON object")
            .insert("legacy_path".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeLeaseVolumePlan>(lease_value).is_err(),
            "same-version runtime lease-volume plans must reject unknown fields"
        );
    }

    #[test]
    fn hosted_http_runtime_state_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudHostedHttpRuntimeStateV1 {
            schema_version: SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1,
            service_name: "service".to_owned(),
            service_version: "1.0.0".to_owned(),
            process_generation: 1,
            health_status: SoraServiceHealthStatusV1::Healthy,
            listen_base_url: None,
            pid: None,
            accounted_egress_bytes: 0,
            replicas: vec![SoracloudHostedHttpReplicaRuntimeStateV1 {
                replica_slot: 1,
                health_status: SoraServiceHealthStatusV1::Healthy,
                listen_base_url: None,
                pid: None,
                last_error: None,
                updated_at_ms: 1,
            }],
            last_error: None,
            updated_at_ms: 1,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical hosted runtime state");
        assert_eq!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(canonical_value.clone())
                .expect("decode canonical hosted runtime state"),
            canonical
        );
        for nullable_field in ["listen_base_url", "pid", "last_error"] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical hosted runtime state must emit explicit null for {nullable_field}"
            );
            assert!(
                canonical_value
                    .pointer(&format!("/replicas/0/{nullable_field}"))
                    .is_some_and(norito::json::Value::is_null),
                "canonical hosted replica state must emit explicit null for {nullable_field}"
            );
        }

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("hosted runtime state JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(unknown).is_err(),
            "same-version hosted runtime state must reject unknown fields"
        );

        let mut unknown_replica = canonical_value.clone();
        unknown_replica
            .pointer_mut("/replicas/0")
            .and_then(norito::json::Value::as_object_mut)
            .expect("hosted replica JSON object")
            .insert("legacy_pid".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(unknown_replica).is_err(),
            "same-version hosted replica state must reject unknown fields"
        );

        for required_field in [
            "listen_base_url",
            "pid",
            "accounted_egress_bytes",
            "replicas",
            "last_error",
        ] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("hosted runtime state JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(missing).is_err(),
                "same-version hosted runtime state must require {required_field}"
            );
        }
        for required_field in ["listen_base_url", "pid", "last_error"] {
            let mut missing = canonical_value.clone();
            missing
                .pointer_mut("/replicas/0")
                .and_then(norito::json::Value::as_object_mut)
                .expect("hosted replica JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(missing).is_err(),
                "same-version hosted replica state must require {required_field}"
            );
        }
    }
    #[test]
    fn apartment_autonomy_summary_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: "ops_agent".to_owned(),
            run_id: "run-1".to_owned(),
            service_name: None,
            service_version: None,
            handler_name: None,
            succeeded: false,
            result_commitment: Hash::new(b"failed autonomy result"),
            checkpoint_artifact_hash: None,
            runtime_receipt: None,
            workflow_steps: vec![SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
                step_index: 0,
                step_id: None,
                request_commitment: Hash::new(b"autonomy request"),
                result_commitment: Hash::new(b"autonomy result"),
                runtime_receipt: None,
                content_type: None,
                response_json: None,
                response_text: None,
            }],
            content_type: None,
            response_json: None,
            response_text: None,
            error: None,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical autonomy summary");
        norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(
            canonical_value.clone(),
        )
        .expect("decode canonical autonomy summary");

        for nullable_field in [
            "service_name",
            "service_version",
            "handler_name",
            "checkpoint_artifact_hash",
            "runtime_receipt",
            "content_type",
            "response_json",
            "response_text",
            "error",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical autonomy summary must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("autonomy summary JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing)
                    .is_err(),
                "same-version autonomy summary must require {nullable_field}"
            );
        }
        for nullable_field in [
            "step_id",
            "runtime_receipt",
            "content_type",
            "response_json",
            "response_text",
        ] {
            assert!(
                canonical_value
                    .pointer(&format!("/workflow_steps/0/{nullable_field}"))
                    .is_some_and(norito::json::Value::is_null),
                "canonical autonomy workflow step must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .pointer_mut("/workflow_steps/0")
                .and_then(norito::json::Value::as_object_mut)
                .expect("autonomy workflow-step JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing)
                    .is_err(),
                "same-version autonomy workflow step must require {nullable_field}"
            );
        }

        let mut missing_steps = canonical_value.clone();
        missing_steps
            .as_object_mut()
            .expect("autonomy summary JSON object")
            .remove("workflow_steps");
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing_steps)
                .is_err(),
            "same-version autonomy summary must require workflow_steps"
        );

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("autonomy summary JSON object")
            .insert("legacy_result".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(unknown)
                .is_err(),
            "same-version autonomy summary must reject unknown fields"
        );

        let mut unknown_step = canonical_value;
        unknown_step
            .pointer_mut("/workflow_steps/0")
            .and_then(norito::json::Value::as_object_mut)
            .expect("autonomy workflow-step JSON object")
            .insert("legacy_result".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(unknown_step)
                .is_err(),
            "same-version autonomy workflow step must reject unknown fields"
        );

        let mut unknown_status = norito::json::to_value(&SoracloudRuntimeHfSourceStatus::Ready)
            .expect("encode canonical HF runtime status");
        unknown_status
            .as_object_mut()
            .expect("HF runtime status JSON object")
            .insert("legacy_status".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeHfSourceStatus>(unknown_status).is_err(),
            "same-version HF runtime status must reject unknown fields"
        );
    }
    #[test]
    fn local_read_binding_json_requires_explicit_nullable_slots_and_rejects_unknown_fields() {
        let canonical = SoracloudLocalReadBinding {
            binding_name: None,
            state_key: None,
            payload_commitment: None,
            artifact_hash: None,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical local-read binding");
        for nullable_field in [
            "binding_name",
            "state_key",
            "payload_commitment",
            "artifact_hash",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical local-read binding must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("local-read binding JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudLocalReadBinding>(missing).is_err(),
                "local-read binding must require {nullable_field}"
            );
        }
        let mut unknown = canonical_value;
        unknown
            .as_object_mut()
            .expect("local-read binding JSON object")
            .insert("legacy_binding".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudLocalReadBinding>(unknown).is_err(),
            "local-read binding must reject unknown nested fields"
        );
    }
    #[test]
    fn apartment_autonomy_summary_validation_binds_schema_path_and_authoritative_commitment() {
        let process_generation = 7;
        let request_commitment = Hash::new(b"authoritative autonomy request");
        let mut summary = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: "ops_agent".to_owned(),
            run_id: "run-7".to_owned(),
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
        .expect("derive canonical autonomy result commitment");
        validate_soracloud_apartment_autonomy_execution_summary_v1(
            &summary,
            "ops_agent",
            "run-7",
            process_generation,
            request_commitment,
        )
        .expect("canonical autonomy summary must validate");

        let mut changed_success = summary.clone();
        changed_success.succeeded = true;
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_success,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind the success outcome"
        );
        let mut changed_service = summary.clone();
        changed_service.service_name = Some("different_service".to_owned());
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_service,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind resolved service identity"
        );
        let mut changed_receipt = summary.clone();
        changed_receipt.runtime_receipt = Some(SoraRuntimeReceiptV1 {
            schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id: Hash::new(b"injected autonomy receipt"),
            service_name: "different_service".parse().expect("valid service name"),
            service_version: "v1".to_owned(),
            handler_name: "infer".parse().expect("valid handler name"),
            handler_class: SoraServiceHandlerClassV1::Query,
            request_commitment: Hash::new(b"injected receipt request"),
            result_commitment: Hash::new(b"injected receipt result"),
            certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
            emitted_sequence: 1,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            placement_id: None,
            selected_validator_account_id: None,
            selected_peer_id: None,
        });
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_receipt,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind the runtime receipt used for authoritative ISIs"
        );
        let mut changed_content_type = summary.clone();
        changed_content_type.content_type = Some("application/json".to_owned());
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_content_type,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind response metadata"
        );

        let mut wrong_schema = summary.clone();
        wrong_schema.schema_version =
            SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1.saturating_add(1);
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &wrong_schema,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "other_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-other",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-7",
                process_generation.saturating_add(1),
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-7",
                process_generation,
                Hash::new(b"different authoritative request"),
            )
            .is_err()
        );
        let mut wrong_result = summary;
        wrong_result.result_commitment = Hash::new(b"different autonomy result");
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &wrong_result,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
    }
    fn sample_private_model_artifact_ref(role: &str, seed: u8) -> SoraPrivateModelArtifactRefV1 {
        SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([seed; 32]),
            artifact_hash: Hash::new([seed; 16]),
            ciphertext_bytes: 128,
            artifact_role: role.to_owned(),
        }
    }
    fn sample_quantized_uploaded_model_bundle() -> SoraUploadedModelBundleV1 {
        let public_key_bytes = vec![7u8; 32];
        let wrapped_key_ciphertext = vec![9u8; 48];
        SoraUploadedModelBundleV1 {
            schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
            service_name: "private_model_host".parse().expect("valid service name"),
            model_id: "upload-quant-v1".to_owned(),
            weight_version: "v1".to_owned(),
            family: "linear-demo".to_owned(),
            modalities: vec!["tabular".to_owned()],
            plaintext_root: Hash::new(b"plaintext model root"),
            runtime_format: SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1,
            bundle_root: Hash::new(b"quantized bundle root"),
            sorafs_manifest_digest: ManifestDigest::new([0xA5; 32]),
            chunk_count: 1,
            plaintext_bytes: 256,
            ciphertext_bytes: 512,
            chunk_manifest_root: Hash::new(b"chunk manifest root"),
            upload_recipient: iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                key_id: "recipient".to_owned(),
                key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                public_key_bytes: public_key_bytes.clone(),
                public_key_fingerprint: Hash::new(public_key_bytes.as_slice()),
            },
            wrapped_bundle_key: iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                recipient_key_id: "recipient".to_owned(),
                recipient_key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                ephemeral_public_key: vec![8u8; 32],
                nonce: vec![5u8; 12],
                wrapped_key_ciphertext: wrapped_key_ciphertext.clone(),
                ciphertext_hash: Hash::new(wrapped_key_ciphertext.as_slice()),
                aad_digest: Hash::new(b"aad"),
            },
            pricing_policy: iroha_data_model::soracloud::SoraUploadedModelPricingPolicyV1 {
                storage_price: "0.000000001"
                    .parse()
                    .expect("canonical uploaded-model storage price"),
            },
            decryption_policy_ref: "policy/v1".to_owned(),
        }
    }
    fn seed_generated_hf_world_with_primary(
        primary_peer_id: &str,
    ) -> (World, String, String, String) {
        let mut world = World::new();
        let service_name: Name = "hf_service".parse().expect("valid service name");
        let service_name_string = service_name.as_ref().to_owned();
        let source_id = Hash::new(b"hf-source");
        let pool_id = Hash::new(b"hf-pool");
        let primary_validator = checked_account_id();
        let member_account = checked_account_id();
        let bundle = build_soracloud_hf_generated_service_bundle(
            service_name.clone(),
            &source_id.to_string(),
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
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
                    config_generation: 0,
                    secret_generation: 0,
                    service_configs: BTreeMap::new(),
                    service_secrets: BTreeMap::new(),
                    fhe_policy_records: BTreeMap::new(),
                    service_name: bundle.service.service_name.clone(),
                    service_lease: None,
                    lease_volume_states: Vec::new(),
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
                    lease_asset_definition_id: AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").expect("domain"),
                        "xor".parse().expect("asset"),
                    ),
                    base_fee: "0.00001".parse().expect("base fee"),
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
                    total_paid: "0.00001".parse().expect("total paid"),
                    total_refunded: Quantity::zero(),
                    last_charge: "0.00001".parse().expect("last charge"),
                    total_compute_paid: "0.000005".parse().expect("total compute paid"),
                    total_compute_refunded: Quantity::zero(),
                    last_compute_charge: "0.000005".parse().expect("last compute charge"),
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
                    selected_weight_file_count: 1,
                    weight_selection_commitment: Hash::new(b"hf-runtime-test-selection"),
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
                total_reservation_fee: "0.000005".parse().expect("total reservation fee"),
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
            "0123456789abcdef0123456789abcdef01234567",
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
        assert_eq!(
            binding.resolved_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(binding.model_name, "gpt-oss");
    }
    #[test]
    fn generated_hf_bundle_payload_matches_declared_bundle_hash() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:2222222222222222222222222222222222222222222222222222222222222222#0002",
            "meta/llama",
            "1123456789abcdef0123456789abcdef01234567",
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
            "2123456789abcdef0123456789abcdef01234567",
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
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                bundle
                    .service
                    .route
                    .as_ref()
                    .expect("generated service route")
                    .host
                    .clone(),
                [443],
            )])
        );
        assert_eq!(manifest.tool_capabilities.len(), 2);
    }
    #[test]
    fn resolve_generated_hf_primary_assignment_returns_warm_primary_for_bound_service() {
        let primary_peer_id = checked_peer_id().to_string();
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
    #[test]
    fn private_uploaded_model_quantized_cpu_runtime_is_deterministic_and_receipted() {
        let model = SoracloudQuantizedCpuModelV1 {
            input_len: 3,
            output_len: 2,
            weights_i8: vec![2, -1, 4, -3, 1, 2],
            bias_i32: vec![3, -4],
            output_shift: 1,
            output_min: -32,
            output_max: 32,
            rounding: SoracloudQuantizedRoundingV1::NearestAwayFromZero,
        };
        let request = SoracloudPrivateUploadedModelExecutionRequestV1 {
            bundle: sample_quantized_uploaded_model_bundle(),
            policy_id: "policy/v1".to_owned(),
            plaintext_input_i32: vec![7, -2, 5],
            input_artifact: sample_private_model_artifact_ref("input", 0x11),
            output_artifact: sample_private_model_artifact_ref("output", 0x22),
            emitted_sequence: 9,
        };
        let first =
            execute_private_uploaded_model_quantized_cpu_v1(&model, request.clone()).expect("run");
        let second =
            execute_private_uploaded_model_quantized_cpu_v1(&model, request).expect("rerun");
        assert_eq!(first.plaintext_output_i32, vec![20, -9]);
        assert_eq!(first, second);
        assert_eq!(
            first.receipt.runtime_version,
            SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1
        );
        assert_eq!(first.receipt.input_artifact.artifact_role, "input");
        assert_eq!(first.receipt.output_artifact.artifact_role, "output");
        assert_ne!(
            first.receipt.input_commitment,
            first.receipt.output_commitment
        );
        first.receipt.validate().expect("receipt validates");
    }
    #[test]
    fn private_uploaded_model_quantized_cpu_runtime_rejects_wrong_format() {
        let model = SoracloudQuantizedCpuModelV1 {
            input_len: 1,
            output_len: 1,
            weights_i8: vec![1],
            bias_i32: vec![0],
            output_shift: 0,
            output_min: i32::MIN,
            output_max: i32::MAX,
            rounding: SoracloudQuantizedRoundingV1::NearestAwayFromZero,
        };
        let mut bundle = sample_quantized_uploaded_model_bundle();
        bundle.runtime_format = SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors;
        let err = execute_private_uploaded_model_quantized_cpu_v1(
            &model,
            SoracloudPrivateUploadedModelExecutionRequestV1 {
                bundle,
                policy_id: "policy/v1".to_owned(),
                plaintext_input_i32: vec![1],
                input_artifact: sample_private_model_artifact_ref("input", 0x11),
                output_artifact: sample_private_model_artifact_ref("output", 0x22),
                emitted_sequence: 1,
            },
        )
        .expect_err("wrong runtime format must fail closed");
        assert_eq!(err.kind, SoracloudRuntimeExecutionErrorKind::InvalidRequest);
    }
    #[test]
    fn runtime_execution_error_has_stable_user_facing_context() {
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "missing canonical step_id",
        );
        assert_eq!(
            error.to_string(),
            "Soracloud runtime invalid request error: missing canonical step_id"
        );
        assert!(std::error::Error::source(&error).is_none());
    }
}
