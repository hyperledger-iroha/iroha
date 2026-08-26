//! Shared Soracloud runtime snapshot types, generated HF manifests, and execution traits.
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
        SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraHfSourceStatusV1, SoraHttpServiceEconomicsV1, SoraInrouGuestIsaV1,
        SoraInrouReplicaHostAvailabilityV1, SoraLeaseVolumeKindV1, SoraLifecycleHooksV1,
        SoraNetworkAllowlistEntryV1, SoraNetworkPolicyV1, SoraResourceLimitsV1,
        SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1, SoraRuntimeReceiptV1,
        SoraServiceDeploymentStateV1, SoraServiceExecutionPlaneV1, SoraServiceHandlerClassV1,
        SoraServiceHandlerV1, SoraServiceHealthStatusV1, SoraServiceLeaseStatusV1,
        SoraServiceMailboxMessageV1, SoraServiceManifestV1, SoraServiceRuntimeStateV1,
        SoraStateEncryptionV1, SoraStateMutationOperationV1, SoraTlsModeV1,
    },
    sorafs::pin_registry::StorageClass,
};
use iroha_primitives::numeric::Quantity;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};
use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::PathBuf,
    sync::Arc,
};
const HF_GENERATED_SERVICE_VERSION_V1: &str = "hf.generated.v1";
const HF_GENERATED_SERVICE_MARKER_ENV: &str = "SORACLOUD_HF_GENERATED";
const HF_GENERATED_SOURCE_ID_ENV: &str = "SORACLOUD_HF_SOURCE_ID";
const HF_GENERATED_REPO_ID_ENV: &str = "SORACLOUD_HF_REPO_ID";
const HF_GENERATED_REVISION_ENV: &str = "SORACLOUD_HF_REVISION";
const HF_GENERATED_MODEL_NAME_ENV: &str = "SORACLOUD_HF_MODEL_NAME";
const HF_GENERATED_ROUTE_SUFFIX: &str = ".hf.soracloud.internal";
const HF_GENERATED_ROUTE_LABEL_PREFIX: char = 'n';
const HF_GENERATED_ROUTE_LABEL_MAX_BYTES: usize = 63;
const HF_GENERATED_ROUTE_HOST_MAX_BYTES: usize = 253;
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
    /// Exact canonical Soracloud model name.
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
fn hf_generated_internal_host(service_name: &Name) -> Result<String, String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    // Lowercase ASCII alphanumerics encode as themselves. Every other UTF-8 byte, including the
    // `-` escape delimiter, encodes as `-xx`; the resulting token stream is therefore injective.
    // A fixed prefix on every deterministically partitioned label keeps escape tokens away from
    // DNS label boundaries without making the partition part of the encoded identity.
    let mut labels = Vec::new();
    let mut label = String::from(HF_GENERATED_ROUTE_LABEL_PREFIX);
    for byte in service_name.as_ref().bytes() {
        let literal = byte.is_ascii_lowercase() || byte.is_ascii_digit();
        let encoded_len = if literal { 1 } else { 3 };
        if label.len() + encoded_len > HF_GENERATED_ROUTE_LABEL_MAX_BYTES {
            labels.push(label);
            label = String::from(HF_GENERATED_ROUTE_LABEL_PREFIX);
        }
        if literal {
            label.push(char::from(byte));
        } else {
            label.push('-');
            label.push(char::from(HEX[usize::from(byte >> 4)]));
            label.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    labels.push(label);
    let host = format!("{}{HF_GENERATED_ROUTE_SUFFIX}", labels.join("."));
    if host.len() > HF_GENERATED_ROUTE_HOST_MAX_BYTES {
        return Err(format!(
            "generated HF service name `{service_name}` cannot be encoded as a DNS host within the {HF_GENERATED_ROUTE_HOST_MAX_BYTES}-byte limit"
        ));
    }
    Ok(host)
}
/// Return the deterministic metadata-only IVM artifact used by generated HF services.
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
        seiyaku_name: "SoracloudHfMetadata".to_owned(),
        compiler_fingerprint: "iroha-soracloud-hf-metadata-v1".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: [hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_METADATA, 0)]
            .into_iter()
            .collect(),
        error_codes: Vec::new(),
        states: Vec::new(),
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
/// Build the canonical inert metadata facade for an HF source.
///
/// The generated V1 bundle is an IVM metadata service. It contains no signed
/// Inrou guest artifact, cannot execute model-controlled code, and cannot make
/// an HF source ready for inference.
///
/// # Errors
/// Returns an error when `service_name` cannot be represented injectively as
/// a canonical DNS host within the protocol's 253-byte host limit.
pub fn build_soracloud_hf_generated_service_bundle(
    service_name: Name,
    source_id: &str,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
) -> Result<SoraDeploymentBundleV1, String> {
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
        bundle_path: "/bundles/hf_generated_metadata.to".to_owned(),
        entrypoint: HF_GENERATED_ENTRYPOINT_METADATA.to_owned(),
        args: Vec::new(),
        env,
        inrou: None,
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        config_exports: Vec::new(),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Isolated,
            allow_state_writes: false,
            allow_model_inference: false,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(500).expect("non-zero cpu budget"),
            memory_bytes: NonZeroU64::new(256 * 1024 * 1024).expect("non-zero memory budget"),
            ephemeral_storage_bytes: NonZeroU64::new(128 * 1024 * 1024)
                .expect("non-zero storage budget"),
            max_open_files_per_process: NonZeroU32::new(256)
                .expect("non-zero per-process open-file cap"),
            max_tasks: NonZeroU16::new(32).expect("non-zero task cap"),
        },
        lifecycle: SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(15).expect("non-zero start grace"),
            stop_grace_secs: NonZeroU32::new(10).expect("non-zero stop grace"),
            healthcheck_path: Some("/healthz".to_owned()),
        },
    };
    let container_manifest_hash = Hash::new(Encode::encode(&container));
    let route_host = hf_generated_internal_host(&service_name)?;
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
        handlers: vec![SoraServiceHandlerV1 {
            handler_name: "metadata".parse().expect("valid literal handler name"),
            class: SoraServiceHandlerClassV1::Query,
            entrypoint: HF_GENERATED_ENTRYPOINT_METADATA.to_owned(),
            route_path: Some("/metadata".to_owned()),
            certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
            mailbox: None,
        }],
        artifacts: Vec::new(),
    };
    Ok(SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    })
}
/// Build the canonical generated agent apartment manifest used for HF-bound agents.
///
/// # Errors
/// Returns an error unless `service_bundle` is a canonical generated-HF bundle
/// with its exact internal route present in the authoritative service
/// manifest. The route is never reconstructed from the service name.
pub fn build_soracloud_hf_generated_agent_manifest(
    apartment_name: Name,
    service_bundle: &SoraDeploymentBundleV1,
) -> Result<AgentApartmentManifestV1, String> {
    let expected_host = hf_generated_internal_host(&service_bundle.service.service_name)?;
    let route = service_bundle.service.route.as_ref().ok_or_else(|| {
        format!(
            "generated HF service `{}` is missing its authoritative internal route",
            service_bundle.service.service_name
        )
    })?;
    if route.host != expected_host
        || route.path_prefix != "/"
        || route.service_port.get() != 8080
        || route.visibility != SoraRouteVisibilityV1::Internal
        || route.tls_mode != SoraTlsModeV1::Disabled
    {
        return Err(format!(
            "generated HF service `{}` must use canonical internal route `{expected_host}:8080/`",
            service_bundle.service.service_name
        ));
    }
    if soracloud_hf_generated_source_binding(service_bundle).is_none() {
        return Err(format!(
            "service `{}` is not a canonical generated-HF bundle",
            service_bundle.service.service_name
        ));
    }
    Ok(AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name,
        container: SoraContainerManifestRefV1 {
            manifest_hash: service_bundle.container_manifest_hash(),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![AgentToolCapabilityV1 {
            tool: "soracloud.hf.metadata".to_owned(),
            max_invocations_per_epoch: NonZeroU32::new(10_000)
                .expect("non-zero metadata invocation limit"),
            allow_network: true,
            allow_filesystem_write: false,
        }],
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
            route.host.clone(),
            [route.service_port.get()],
        )]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    })
}
/// Extract canonical HF source markers from a generated Soracloud service bundle.
#[must_use]
pub fn soracloud_hf_generated_source_binding(
    bundle: &SoraDeploymentBundleV1,
) -> Option<SoracloudHfGeneratedSourceBinding> {
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
    let binding = SoracloudHfGeneratedSourceBinding {
        source_id,
        repo_id,
        resolved_revision,
        model_name,
    };
    let expected = build_soracloud_hf_generated_service_bundle(
        bundle.service.service_name.clone(),
        &binding.source_id,
        &binding.repo_id,
        &binding.resolved_revision,
        &binding.model_name,
    )
    .ok()?;
    (bundle == &expected).then_some(binding)
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
    /// Effective hosted-service lease status at the observed block height.
    #[norito(required)]
    pub service_lease_status: Option<SoraServiceLeaseStatusV1>,
    /// Canonical block height when hosted-service routing/materialization expires.
    #[norito(required)]
    pub lease_expires_height: Option<u64>,
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
    /// Guest ISA profile selected locally for this replica.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Kernel image path for the selected guest ISA inside the hydrated Soracloud bundle.
    pub kernel_image_path: String,
    /// Immutable base root filesystem image path for the selected guest ISA.
    pub rootfs_image_path: String,
    /// Optional initrd image path for the selected guest ISA.
    #[norito(required)]
    pub initrd_image_path: Option<String>,
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
    /// Admission-validated exact guest mount path for this volume.
    pub mount_path: String,
    /// Maximum logical bytes retained for this volume.
    pub max_total_bytes: u64,
    /// Canonical block height that identifies the active economic lease incarnation.
    pub lease_started_height: u64,
    /// Canonical block height when the authoritative volume lease expires.
    pub lease_expires_height: u64,
    /// Monotonic generation of the authoritative lease binding.
    pub authoritative_generation: u64,
    /// Node-local materialization directory used by the current host.
    ///
    /// Every lease disk uses a per-revision/per-replica namespace. Inrou V1 has no shared mutable
    /// filesystem or multi-attach volume path.
    pub local_materialization_dir: String,
}
/// Node-local runtime topology projected for one hosted-HTTP replica slot.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeReplicaPlan {
    /// One-based replica slot within the revision.
    pub replica_slot: u16,
    /// Canonical block height identifying this economic lease incarnation.
    pub lease_started_height: u64,
    /// Transaction-bound incarnation of this slot's host assignment within the active service lease.
    pub placement_incarnation: String,
    /// Whether the sticky assigned host remains eligible to serve this lease incarnation.
    pub host_availability: SoraInrouReplicaHostAvailabilityV1,
    /// Canonical validator account assigned to this replica incarnation.
    pub validator_account_id: String,
    /// Canonical peer identifier assigned to this replica incarnation.
    pub peer_id: String,
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
    /// Exact placement incarnation observed for this local runtime state.
    pub placement_incarnation: String,
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
/// Runtime-manager materialization state for a canonical Hugging Face source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "status", content = "value")]
#[norito(deny_unknown_fields)]
pub enum SoracloudRuntimeHfSourceStatus {
    /// The canonical import has no runtime-visible bindings yet.
    PendingImport,
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
    /// Return the local peer id, when the runtime knows its host identity.
    fn local_peer_id(&self) -> Option<String> {
        None
    }
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
#[derive(Clone, PartialEq, Eq, Encode, Decode)]
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
impl std::fmt::Debug for SoracloudLocalReadRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SoracloudLocalReadRequest")
            .field("observed_height", &self.observed_height)
            .field(
                "has_observed_block_hash",
                &self.observed_block_hash.is_some(),
            )
            .field("service_name", &self.service_name)
            .field("service_version", &self.service_version)
            .field("handler_name", &self.handler_name)
            .field("handler_class", &self.handler_class)
            .field("request_method", &self.request_method)
            .field("request_path_len", &self.request_path.len())
            .field("handler_path_len", &self.handler_path.len())
            .field("has_request_query", &self.request_query.is_some())
            .field("request_header_count", &self.request_headers.len())
            .field("request_body_len", &self.request_body.len())
            .field("request_commitment", &self.request_commitment)
            .finish_non_exhaustive()
    }
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
#[derive(Clone, PartialEq, Eq, Encode, Decode)]
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
impl std::fmt::Debug for SoracloudLocalReadResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SoracloudLocalReadResponse")
            .field("response_bytes_len", &self.response_bytes.len())
            .field("has_content_type", &self.content_type.is_some())
            .field("has_content_encoding", &self.content_encoding.is_some())
            .field("has_cache_control", &self.cache_control.is_some())
            .field("binding_count", &self.bindings.len())
            .field("result_commitment", &self.result_commitment)
            .field("certified_by", &self.certified_by)
            .field("has_runtime_receipt", &self.runtime_receipt.is_some())
            .finish_non_exhaustive()
    }
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
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::sorafs::pin_registry::StorageClass;
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("Soracloud runtime fixture key generation should succeed")
    }
    #[test]
    fn local_read_debug_output_redacts_request_and_response_payloads() {
        let request = SoracloudLocalReadRequest {
            observed_height: 7,
            observed_block_hash: Some(Hash::new(b"block")),
            service_name: "public_service".to_owned(),
            service_version: "1.0.0".to_owned(),
            handler_name: "infer".to_owned(),
            handler_class: SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/public/private-path-marker".to_owned(),
            handler_path: "/private-handler-path-marker".to_owned(),
            request_query: Some("token=private-query-marker".to_owned()),
            request_headers: BTreeMap::from([(
                "authorization".to_owned(),
                "private-header-marker".to_owned(),
            )]),
            request_body: b"private-request-body-marker".to_vec(),
            request_commitment: Hash::new(b"request"),
        };
        let response = SoracloudLocalReadResponse {
            response_bytes: b"private-response-body-marker".to_vec(),
            content_type: Some("private-content-type-marker".to_owned()),
            content_encoding: Some("private-content-encoding-marker".to_owned()),
            cache_control: Some("private-cache-control-marker".to_owned()),
            bindings: vec![SoracloudLocalReadBinding {
                binding_name: Some("private-binding-marker".to_owned()),
                state_key: Some("private-state-key-marker".to_owned()),
                payload_commitment: Some(Hash::new(b"payload")),
                artifact_hash: None,
            }],
            result_commitment: Hash::new(b"response"),
            certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
            runtime_receipt: None,
        };
        let rendered = format!("{request:?}\n{response:?}");
        for secret in [
            "private-path-marker",
            "private-handler-path-marker",
            "private-query-marker",
            "private-header-marker",
            "private-request-body-marker",
            "private-response-body-marker",
            "private-content-type-marker",
            "private-content-encoding-marker",
            "private-cache-control-marker",
            "private-binding-marker",
            "private-state-key-marker",
        ] {
            assert!(
                !rendered.contains(secret),
                "local-read Debug output exposed `{secret}`: {rendered}"
            );
        }
        assert!(rendered.contains("request_body_len: 27"));
        assert!(rendered.contains("response_bytes_len: 28"));
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
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
                kernel_image_path: "guest/vmlinuz".to_owned(),
                rootfs_image_path: "guest/rootfs.ext4".to_owned(),
                initrd_image_path: None,
                root_volume_name: "root".to_owned(),
            }),
            bundle_cache_path: "/runtime/cache/bundle".to_owned(),
            bundle_available_locally: true,
            process_generation: None,
            desired_replica_count: 1,
            local_replica_slots: vec![1],
            local_replicas: vec![SoracloudRuntimeReplicaPlan {
                replica_slot: 1,
                lease_started_height: 1,
                placement_incarnation: Hash::new(b"runtime-placement").to_string(),
                host_availability: SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: "validator".to_owned(),
                peer_id: "peer".to_owned(),
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
            lease_expires_height: None,
            remaining_runtime_balance: None,
            config_entry_count: 0,
            secret_entry_count: 0,
            config_exports: Vec::new(),
            supports_host_read_config: false,
            supports_host_read_secret_envelope: false,
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
            authoritative_status: SoraHfSourceStatusV1::PendingImport,
            runtime_status: SoracloudRuntimeHfSourceStatus::PendingImport,
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
            "lease_expires_height",
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
            "lease_expires_height",
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
        assert!(
            inrou_value
                .get("initrd_image_path")
                .is_some_and(norito::json::Value::is_null),
            "canonical runtime Inrou plan must emit explicit null for initrd_image_path"
        );
        for required_field in ["initrd_image_path"] {
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
            .insert(
                "bootstrap_user_data_path".to_owned(),
                norito::json::Value::Null,
            );
        assert!(
            norito::json::from_value::<SoracloudRuntimeInrouPlan>(unknown_inrou).is_err(),
            "same-version runtime Inrou plans must reject the retired bootstrap overlay field"
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
        let mut missing_host_availability = replica_value.clone();
        missing_host_availability
            .as_object_mut()
            .expect("runtime replica plan JSON object")
            .remove("host_availability");
        assert!(
            norito::json::from_value::<SoracloudRuntimeReplicaPlan>(missing_host_availability)
                .is_err(),
            "same-version runtime replica plans must require host_availability"
        );
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
            lease_started_height: 1,
            lease_expires_height: 100,
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
                placement_incarnation: Hash::new(b"runtime-placement").to_string(),
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
    fn hf_runtime_status_json_rejects_unknown_fields() {
        let mut unknown_status =
            norito::json::to_value(&SoracloudRuntimeHfSourceStatus::PendingImport)
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
    fn generated_hf_service_bundle_is_admissible_and_tagged() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:1111111111111111111111111111111111111111111111111111111111111111#0001",
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt-oss",
        )
        .expect("generated HF fixture host should fit DNS limits");
        bundle
            .validate_for_admission()
            .expect("generated HF bundle should validate");
        assert!(!bundle.container.capabilities.allow_model_inference);
        assert!(!bundle.container.capabilities.allow_state_writes);
        assert_eq!(
            bundle.container.entrypoint,
            HF_GENERATED_ENTRYPOINT_METADATA
        );
        assert_eq!(bundle.service.handlers.len(), 1);
        let metadata_handler = &bundle.service.handlers[0];
        assert_eq!(metadata_handler.handler_name.as_ref(), "metadata");
        assert_eq!(
            metadata_handler.entrypoint,
            HF_GENERATED_ENTRYPOINT_METADATA
        );
        assert_eq!(metadata_handler.route_path.as_deref(), Some("/metadata"));
        assert!(
            bundle
                .service
                .handlers
                .iter()
                .all(|handler| handler.handler_name.as_ref() != "infer")
        );
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
    fn generated_hf_internal_host_encoding_is_injective_and_dns_exact() {
        let underscore: Name = "hf_service".parse().expect("valid underscore service name");
        let hyphen: Name = "hf-service".parse().expect("valid hyphen service name");
        let uppercase: Name = "HFService".parse().expect("valid uppercase service name");
        let unicode: Name = "café".parse().expect("valid NFC service name");
        let host = |name: &Name| {
            hf_generated_internal_host(name).expect("fixture name should fit DNS host limits")
        };

        assert_eq!(host(&underscore), "nhf-5fservice.hf.soracloud.internal");
        assert_eq!(host(&hyphen), "nhf-2dservice.hf.soracloud.internal");
        assert_ne!(
            host(&underscore),
            host(&hyphen),
            "the retired underscore-to-hyphen rewrite was not injective"
        );
        assert_eq!(host(&uppercase), "n-48-46-53ervice.hf.soracloud.internal");
        assert_eq!(host(&unicode), "ncaf-c3-a9.hf.soracloud.internal");

        let long_name: Name = "a".repeat(63).parse().expect("valid long service name");
        let long_host = host(&long_name);
        let generated_labels = long_host
            .strip_suffix(HF_GENERATED_ROUTE_SUFFIX)
            .expect("generated route suffix")
            .split('.')
            .collect::<Vec<_>>();
        assert_eq!(generated_labels.len(), 2);
        assert!(
            generated_labels
                .iter()
                .all(|label| !label.is_empty() && label.len() <= 63)
        );
    }
    #[test]
    fn generated_hf_service_bundle_rejects_name_that_cannot_fit_dns_host() {
        let service_name: Name = "a"
            .repeat(iroha_data_model::name::MAX_NAME_BYTES)
            .parse()
            .expect("maximum-length Name should be valid");
        let error = build_soracloud_hf_generated_service_bundle(
            service_name,
            "hash:1111111111111111111111111111111111111111111111111111111111111111#0001",
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt-oss",
        )
        .expect_err("generated service host must fit the DNS full-name limit");
        assert!(error.contains("253-byte limit"));
    }
    #[test]
    fn generated_hf_source_binding_rejects_complete_structure_drift() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:1111111111111111111111111111111111111111111111111111111111111111#0001",
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt-oss",
        )
        .expect("generated HF fixture host should fit DNS limits");
        let assert_rejected = |candidate: &SoraDeploymentBundleV1| {
            assert!(
                soracloud_hf_generated_source_binding(candidate).is_none(),
                "generated-HF binding accepted noncanonical bundle drift"
            );
        };

        let mut extra_environment = bundle.clone();
        extra_environment
            .container
            .env
            .insert("UNREVIEWED".to_owned(), "1".to_owned());
        assert_rejected(&extra_environment);

        let mut resource_drift = bundle.clone();
        resource_drift.container.resources.cpu_millis =
            NonZeroU32::new(501).expect("non-zero drift fixture");
        assert_rejected(&resource_drift);

        let mut route_drift = bundle.clone();
        route_drift
            .service
            .route
            .as_mut()
            .expect("generated route")
            .path_prefix = "/alternate".to_owned();
        assert_rejected(&route_drift);

        let mut handler_drift = bundle.clone();
        handler_drift.service.handlers[0].route_path = Some("/alternate-metadata".to_owned());
        assert_rejected(&handler_drift);

        let mut artifact_drift = bundle;
        artifact_drift.container.bundle_path = "/bundles/alternate.to".to_owned();
        assert_rejected(&artifact_drift);
    }
    #[test]
    fn generated_hf_agent_manifest_tracks_service_container_and_host() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_agent_service".parse().expect("valid service name"),
            "hash:3333333333333333333333333333333333333333333333333333333333333333#0003",
            "huggingface/smol",
            "2123456789abcdef0123456789abcdef01234567",
            "smol",
        )
        .expect("generated HF fixture host should fit DNS limits");
        let manifest = build_soracloud_hf_generated_agent_manifest(
            "hf_agent".parse().expect("valid apartment name"),
            &bundle,
        )
        .expect("canonical generated service route");
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
                [8080],
            )])
        );
        assert_eq!(manifest.tool_capabilities.len(), 1);
        assert_eq!(manifest.tool_capabilities[0].tool, "soracloud.hf.metadata");
    }
    #[test]
    fn generated_hf_agent_manifest_rejects_missing_or_noncanonical_route() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_agent_service".parse().expect("valid service name"),
            "hash:3333333333333333333333333333333333333333333333333333333333333333#0003",
            "huggingface/smol",
            "2123456789abcdef0123456789abcdef01234567",
            "smol",
        )
        .expect("generated HF fixture host should fit DNS limits");
        let apartment_name: Name = "hf_agent".parse().expect("valid apartment name");

        let mut missing_route = bundle.clone();
        missing_route.service.route = None;
        let error =
            build_soracloud_hf_generated_agent_manifest(apartment_name.clone(), &missing_route)
                .expect_err("a generated agent must not infer a missing route");
        assert!(error.contains("missing its authoritative internal route"));

        let mut noncanonical_route = bundle;
        noncanonical_route
            .service
            .route
            .as_mut()
            .expect("generated route")
            .host = "retired-bridge.internal".to_owned();
        let error =
            build_soracloud_hf_generated_agent_manifest(apartment_name, &noncanonical_route)
                .expect_err("a generated agent must require the canonical route");
        assert!(error.contains("must use canonical internal route"));
    }
}
