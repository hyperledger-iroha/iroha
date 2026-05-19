//! Embedded Soracloud runtime-manager reconciliation for `irohad`.
//!
//! This subsystem continuously projects authoritative Soracloud world state
//! into a node-local materialization plan and now serves deterministic local
//! reads/apartment observations directly from the committed snapshot plus the
//! hydrated artifact cache. Soracloud runtime v1 runs IVM handlers directly
//! and supervises hosted HTTP revisions (`Inrou`) as loopback services.
//!
//! Ordered mailbox execution and public query local reads now run admitted IVM
//! bundles directly through the Soracloud host surface while asset local reads
//! still resolve from the committed snapshot plus hydrated artifact cache.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs, io,
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    str::FromStr,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use eyre::WrapErr;
use iroha_core::soracloud_runtime::{
    SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
    SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1, SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1,
    SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentAutonomyWorkflowStepSummaryV1,
    SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
    SoracloudHostedHttpReplicaRuntimeStateV1, SoracloudHostedHttpRuntimeStateV1,
    SoracloudLocalReadRequest, SoracloudLocalReadResponse, SoracloudOrderedMailboxExecutionRequest,
    SoracloudOrderedMailboxExecutionResult, SoracloudRuntime, SoracloudRuntimeApartmentPlan,
    SoracloudRuntimeArtifactPlan,
    SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
    SoracloudRuntimeHfSourcePlan, SoracloudRuntimeHfSourceStatus, SoracloudRuntimeInrouPlan,
    SoracloudRuntimeLeaseVolumePlan, SoracloudRuntimeMailboxPlan, SoracloudRuntimeReadHandle,
    SoracloudRuntimeReplicaPlan, SoracloudRuntimeRevisionRole, SoracloudRuntimeServicePlan,
    SoracloudRuntimeSnapshot, SoracloudUploadedModelEncryptionRecipient,
    soracloud_hf_generated_bundle_payload_if_applicable, soracloud_hf_generated_source_binding,
};
use iroha_core::state::{State, StateView, WorldReadOnly};
use iroha_core::{queue::Queue, tx::AcceptedTransaction};
use iroha_crypto::{Hash, KeyPair};
#[cfg(test)]
use iroha_data_model::soracloud::SoraNetworkAllowlistEntryV1;
use iroha_data_model::{
    ChainId, Encode,
    account::AccountId,
    asset::{AssetDefinitionAlias, AssetDefinitionId},
    isi::{self, InstructionBox},
    metadata::Metadata,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1, SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
        SORA_RUNTIME_RECEIPT_VERSION_V1, SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1, SORACLOUD_HOST_RESPONSE_VERSION_V1,
        SoraAgentApartmentRecordV1, SoraAgentRuntimeStatusV1, SoraArtifactKindV1,
        SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1, SoraConfigExportTargetV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraHfPlacementHostAssignmentV1,
        SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
        SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseStatusV1, SoraHfSourceStatusV1,
        SoraInrouGuestImageV1, SoraInrouGuestIsaV1, SoraInrouHostCapabilityRecordV1,
        SoraInrouReplicaPlacementV1, SoraInrouReplicaRuntimeStateV1, SoraInrouRuntimeBackendV1,
        SoraLeaseVolumeKindV1, SoraModelHostViolationKindV1, SoraNetworkPolicyV1,
        SoraRouteVisibilityV1, SoraRuntimeReceiptV1, SoraServiceDeploymentStateV1,
        SoraServiceHandlerClassV1, SoraServiceHandlerV1,
        SoraServiceHealthStatusV1, SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1,
        SoraServiceRuntimeStateV1, SoraServiceStateEntryV1, SoraStateBindingV1,
        SoraStateMutationOperationV1,
        SoraUploadedModelKeyEncapsulationV1, SoraUploadedModelKeyWrapAeadV1,
        SoracloudAppendJournalResponseV1, SoracloudEgressFetchRequestV1,
        SoracloudEgressFetchResponseV1, SoracloudEmitMailboxMessageRequestV1,
        SoracloudEmitMailboxMessageResponseV1, SoracloudEmitStateMutationRequestV1,
        SoracloudEmitStateMutationResponseV1, SoracloudHostOperationV1,
        SoracloudHostRequestEnvelopeV1, SoracloudHostRequestPayloadV1,
        SoracloudHostResponseEnvelopeV1, SoracloudHostResponsePayloadV1,
        SoracloudPublishCheckpointResponseV1, SoracloudReadCommittedStateResponseV1,
        SoracloudReadConfigResponseV1, SoracloudReadCredentialResponseV1,
        SoracloudReadSecretEnvelopeResponseV1, SoracloudReadSecretResponseV1,
        encode_inrou_host_advertise_provenance_payload,
        encode_model_host_heartbeat_provenance_payload,
    },
    sorafs::pin_registry::ManifestDigest,
    transaction::TransactionBuilder,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_primitives::json::Json;
use iroha_torii::sorafs::{
    EndpointKind, ProviderAdvertCache, ReplicationOrderV1, TransportProtocol,
    api::{StorageManifestResponseDto, StorageStoredFileDto},
};
use ivm::{
    CoreHost, IVM, IVMHost, PointerType, VMError,
    syscalls::{
        self as ivm_syscalls, SYSCALL_SORACLOUD_APPEND_JOURNAL, SYSCALL_SORACLOUD_EGRESS_FETCH,
        SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE, SYSCALL_SORACLOUD_EMIT_STATE_MUTATION,
        SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT, SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
        SYSCALL_SORACLOUD_READ_CONFIG, SYSCALL_SORACLOUD_READ_CREDENTIAL,
        SYSCALL_SORACLOUD_READ_SECRET, SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE,
    },
    verify_contract_artifact,
};
use mv::storage::StorageReadOnly;
use parking_lot::{Mutex, RwLock};
use sorafs_node::store::StoredManifest;
use tokio::{sync::RwLock as AsyncRwLock, task::JoinHandle};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

const SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_VERSION_V1: u32 = 1;
const SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_DIR: &str = "uploaded_model_keys";
const SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_FILE: &str = "x25519_v1.bin";
const MODEL_HOST_VIOLATION_REPORT_COOLDOWN_MS: u64 = 30_000;
const GENERATED_HF_RECONCILE_REQUEST_COOLDOWN_MS: u64 = 30_000;
const INROU_HOST_ADVERT_ATTEMPT_COOLDOWN_MS: u64 = 10_000;
const INROU_HOST_HEARTBEAT_TTL_FLOOR_MS: u64 = 300_000;
// Avoid rewriting authoritative adverts just to push the same heartbeat expiry forward.
const INROU_HOST_HEARTBEAT_REFRESH_MARGIN_FLOOR_MS: u64 = 60_000;
const INROU_PLACEMENT_RECONCILE_ATTEMPT_COOLDOWN_MS: u64 = 10_000;
const SORACLOUD_LOCAL_READ_MAX_SNAPSHOT_LAG_BLOCKS: u64 = 64;
const INROU_PORTABLE_START_GRACE_FLOOR: Duration = Duration::from_secs(180);
const INROU_PORTABLE_BUNDLE_METADATA_PATH: &str = "/soracloud/bundle.tgz";
const INROU_PORTABLE_BUNDLE_METADATA_MEMBER: &str = "soracloud/bundle.tgz";
const INROU_PORTABLE_BUNDLE_GUEST_ROOT: &str = "/var/lib/soracloud/materialization/bundle";

/// Runtime-manager configuration derived from the explicit Soracloud runtime settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SoracloudRuntimeManagerConfig {
    /// Whether runtime production posture checks are enabled.
    pub production_mode: bool,
    /// Root directory for local runtime materialization state.
    pub state_dir: PathBuf,
    /// Reconciliation cadence against authoritative state.
    pub reconcile_interval: Duration,
    /// Reserved concurrency budget for future hydration workers.
    pub hydration_concurrency: NonZeroUsize,
    /// Configured artifact-cache budgets for the embedded runtime manager.
    pub cache_budgets: iroha_config::parameters::actual::SoracloudRuntimeCacheBudgets,
    /// Mutable `Inrou` microVM hosting limits.
    pub inrou: iroha_config::parameters::actual::SoracloudRuntimeInrou,
    /// Runtime-originated transaction submission settings.
    pub submission: iroha_config::parameters::actual::SoracloudRuntimeSubmission,
    /// Outbound egress policy for embedded runtimes.
    pub egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
    /// Hugging Face importer and inference bridge settings.
    pub hf: iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    /// Local validator account used to enforce authoritative HF placement assignments.
    pub local_validator_account_id: Option<AccountId>,
    /// Local peer identifier used to confirm authoritative HF placement assignments.
    pub local_peer_id: Option<String>,
}

/// Internal sink used by the runtime manager to enqueue authoritative Soracloud mutations.
pub(crate) trait SoracloudRuntimeMutationSink: Send + Sync {
    /// Submit one authoritative Soracloud instruction through the normal transaction pipeline.
    fn submit_instruction(
        &self,
        instruction: InstructionBox,
        endpoint: &'static str,
    ) -> eyre::Result<()>;

    /// Submit an authoritative model-host heartbeat using the sink's configured validator authority.
    fn submit_model_host_heartbeat(
        &self,
        validator_account_id: &AccountId,
        heartbeat_expires_at_ms: u64,
    ) -> eyre::Result<()>;

    /// Submit or refresh the authoritative Inrou host advert using the sink's configured validator authority.
    fn submit_inrou_host_capability(
        &self,
        capability: &SoraInrouHostCapabilityRecordV1,
    ) -> eyre::Result<()>;

    /// Submit an authoritative Inrou placement reconciliation request.
    fn submit_inrou_placement_reconcile(&self) -> eyre::Result<()> {
        self.submit_instruction(
            InstructionBox::from(isi::soracloud::ReconcileSoracloudInrouPlacements),
            "/internal/soracloud/runtime/inrou-placement-reconcile",
        )
    }
}

/// Queue-backed mutation sink used by `irohad` to report runtime-originated Soracloud health events.
#[derive(Clone)]
pub(crate) struct QueuedSoracloudRuntimeMutationSink {
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
    authority: AccountId,
    key_pair: KeyPair,
    gas_asset_id: Option<String>,
}

impl QueuedSoracloudRuntimeMutationSink {
    /// Construct the queue-backed mutation sink using the node's validator authority.
    pub(crate) fn new(
        chain_id: Arc<ChainId>,
        queue: Arc<Queue>,
        state: Arc<State>,
        authority: AccountId,
        key_pair: KeyPair,
        gas_asset_id: Option<String>,
    ) -> Self {
        Self {
            chain_id,
            queue,
            state,
            authority,
            key_pair,
            gas_asset_id,
        }
    }
}

impl SoracloudRuntimeMutationSink for QueuedSoracloudRuntimeMutationSink {
    fn submit_instruction(
        &self,
        instruction: InstructionBox,
        endpoint: &'static str,
    ) -> eyre::Result<()> {
        let tx = TransactionBuilder::new((*self.chain_id).clone(), self.authority.clone())
            .with_instructions([instruction])
            .with_metadata(soracloud_runtime_submission_metadata(
                &self.state,
                self.gas_asset_id.as_deref(),
            ))
            .sign(self.key_pair.private_key());
        let (max_clock_drift, transaction_params, crypto) = {
            let world = self.state.world_view();
            let params = world.parameters();
            (
                params.sumeragi().max_clock_drift(),
                params.transaction(),
                self.state.crypto(),
            )
        };
        let accepted = AcceptedTransaction::accept(
            tx,
            &self.chain_id,
            max_clock_drift,
            transaction_params,
            crypto.as_ref(),
        )
        .wrap_err_with(|| format!("accept internal Soracloud runtime mutation at `{endpoint}`"))?;
        self.queue
            .push_with_lane_with_state(accepted, self.state.as_ref())
            .map(|_| ())
            .map_err(|failure| {
                eyre::eyre!(
                    "enqueue internal Soracloud runtime mutation at `{endpoint}`: {}",
                    failure.err
                )
            })
    }

    fn submit_model_host_heartbeat(
        &self,
        validator_account_id: &AccountId,
        heartbeat_expires_at_ms: u64,
    ) -> eyre::Result<()> {
        if *validator_account_id != self.authority {
            eyre::bail!(
                "runtime model-host heartbeat validator `{validator_account_id}` does not match sink authority `{}`",
                self.authority
            );
        }
        let payload = encode_model_host_heartbeat_provenance_payload(
            validator_account_id,
            heartbeat_expires_at_ms,
        )
        .wrap_err("encode runtime model-host heartbeat provenance payload")?;
        let instruction = InstructionBox::from(isi::soracloud::HeartbeatSoracloudModelHost {
            validator_account_id: validator_account_id.clone(),
            heartbeat_expires_at_ms,
            provenance: ManifestProvenance {
                signer: self.key_pair.public_key().clone(),
                signature: iroha_crypto::Signature::new(self.key_pair.private_key(), &payload),
            },
        });
        self.submit_instruction(
            instruction,
            "/internal/soracloud/runtime/model-host-heartbeat",
        )
    }

    fn submit_inrou_host_capability(
        &self,
        capability: &SoraInrouHostCapabilityRecordV1,
    ) -> eyre::Result<()> {
        if capability.validator_account_id != self.authority {
            eyre::bail!(
                "runtime Inrou host advert validator `{}` does not match sink authority `{}`",
                capability.validator_account_id,
                self.authority
            );
        }
        let payload = encode_inrou_host_advertise_provenance_payload(capability)
            .wrap_err("encode runtime Inrou host advert provenance payload")?;
        let instruction = InstructionBox::from(isi::soracloud::AdvertiseSoracloudInrouHost {
            capability: capability.clone(),
            provenance: ManifestProvenance {
                signer: self.key_pair.public_key().clone(),
                signature: iroha_crypto::Signature::new(self.key_pair.private_key(), &payload),
            },
        });
        self.submit_instruction(instruction, "/internal/soracloud/runtime/inrou-host-advert")
    }
}

fn soracloud_runtime_submission_metadata(
    state: &State,
    gas_asset_id: Option<&str>,
) -> Metadata {
    let mut metadata = Metadata::default();
    if let Some(asset_id) = gas_asset_id
        .map(str::trim)
        .filter(|asset_id| !asset_id.is_empty())
        .map(|asset_id| canonicalize_or_preserve_runtime_gas_asset_id(state, asset_id.to_owned()))
    {
        let gas_asset_key =
            Name::from_str("gas_asset_id").expect("static metadata key `gas_asset_id`");
        metadata.insert(gas_asset_key, Json::new(asset_id));
    }

    metadata
}

fn canonicalize_or_preserve_runtime_gas_asset_id(state: &State, asset_id: String) -> String {
    let trimmed = asset_id.trim();
    if trimmed.is_empty() {
        return asset_id;
    }

    let world = state.world_view();
    if let Ok(definition_id) = trimmed.parse::<AssetDefinitionId>() {
        if world.asset_definition(&definition_id).is_ok() {
            return definition_id.to_string();
        }
        return trimmed.to_owned();
    }

    if let Ok(alias) = trimmed.parse::<AssetDefinitionAlias>() {
        let now_ms = state
            .latest_block_header_fast()
            .map(|header| header.creation_time_ms)
            .unwrap_or(0);
        if let Some(definition_id) = world.asset_definition_id_by_alias_at(&alias, now_ms) {
            return definition_id.to_string();
        }
    }

    iroha_logger::warn!(
        asset = %trimmed,
        "failed to canonicalize Soracloud runtime gas asset id; preserving configured value"
    );
    trimmed.to_owned()
}

fn current_host_inrou_guest_isa() -> SoraInrouGuestIsaV1 {
    #[cfg(target_arch = "x86_64")]
    {
        return SoraInrouGuestIsaV1::X8664;
    }

    #[cfg(target_arch = "aarch64")]
    {
        return SoraInrouGuestIsaV1::Aarch64;
    }

    #[allow(unreachable_code)]
    SoraInrouGuestIsaV1::Aarch64
}

fn portable_vm_guest_machine_profile(
    guest_isa: SoraInrouGuestIsaV1,
) -> PortableVmGuestMachineProfile {
    match guest_isa {
        SoraInrouGuestIsaV1::X8664 => PortableVmGuestMachineProfile {
            emulator_candidates: &["qemu-system-x86_64"],
            machine_type: "q35",
            serial_console: "ttyS0",
            root_label: "rootfs-x86_64",
            block_device: "virtio-blk-pci",
            net_device: "virtio-net-pci",
        },
        SoraInrouGuestIsaV1::Aarch64 => PortableVmGuestMachineProfile {
            emulator_candidates: &["qemu-system-aarch64"],
            machine_type: "virt",
            serial_console: "ttyAMA0",
            root_label: "rootfs-aarch64",
            block_device: "virtio-blk-device",
            net_device: "virtio-net-device",
        },
    }
}

fn default_portable_vm_accel() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "hvf"
    }
    #[cfg(target_os = "windows")]
    {
        "whpx"
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        #[cfg(target_os = "linux")]
        if Path::new("/dev/kvm").exists() {
            return "kvm";
        }
        "tcg"
    }
}

fn portable_vm_accel() -> eyre::Result<String> {
    portable_vm_accel_from(std::env::var("IROHA_INROU_PORTABLE_ACCEL").ok().as_deref())
}

fn portable_vm_accel_from(configured: Option<&str>) -> eyre::Result<String> {
    let configured = configured.unwrap_or("auto").trim().to_owned();
    match configured.as_str() {
        "" | "auto" => Ok(default_portable_vm_accel().to_owned()),
        "tcg" | "kvm" | "hvf" | "whpx" => Ok(configured),
        other => eyre::bail!(
            "unsupported IROHA_INROU_PORTABLE_ACCEL `{other}`; expected one of auto, tcg, kvm, hvf, whpx"
        ),
    }
}

fn portable_vm_backend_is_available() -> bool {
    let profile = portable_vm_guest_machine_profile(current_host_inrou_guest_isa());
    resolve_executable_candidates(profile.emulator_candidates).is_some()
        && resolve_inrou_qemu_img_executable().is_some()
}

fn firecracker_kvm_backend_is_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new("/dev/kvm").exists()
            && resolve_executable_on_path("firecracker").is_some()
            && resolve_executable_on_path("ip").is_some()
            && resolve_executable_on_path("iptables").is_some()
            && resolve_inrou_mke2fs_executable().is_some()
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn supported_inrou_backends_for_host() -> BTreeSet<SoraInrouRuntimeBackendV1> {
    let mut backends = BTreeSet::new();
    if portable_vm_backend_is_available() {
        backends.insert(SoraInrouRuntimeBackendV1::PortableVm);
    }
    if firecracker_kvm_backend_is_available() {
        backends.insert(SoraInrouRuntimeBackendV1::FirecrackerKvm);
    }
    backends
}

fn default_zero_capacity_inrou_backends() -> BTreeSet<SoraInrouRuntimeBackendV1> {
    BTreeSet::from([SoraInrouRuntimeBackendV1::PortableVm])
}

fn inrou_host_platform_supports_local_materialization() -> bool {
    !supported_inrou_backends_for_host().is_empty()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModelHostViolationReportKey {
    validator_account_id: AccountId,
    kind: SoraModelHostViolationKindV1,
    placement_id: Option<Hash>,
}

impl ModelHostViolationReportKey {
    fn kind_sort_key(kind: SoraModelHostViolationKindV1) -> u8 {
        match kind {
            SoraModelHostViolationKindV1::WarmupNoShow => 0,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss => 1,
            SoraModelHostViolationKindV1::AdvertContradiction => 2,
        }
    }
}

impl PartialOrd for ModelHostViolationReportKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModelHostViolationReportKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.validator_account_id
            .cmp(&other.validator_account_id)
            .then_with(|| Self::kind_sort_key(self.kind).cmp(&Self::kind_sort_key(other.kind)))
            .then_with(|| self.placement_id.cmp(&other.placement_id))
    }
}

struct SoracloudModelHostViolationReporter {
    mutation_sink: Option<Arc<dyn SoracloudRuntimeMutationSink>>,
    recent_attempts_ms: Mutex<BTreeMap<ModelHostViolationReportKey, u64>>,
}

impl SoracloudModelHostViolationReporter {
    fn disabled() -> Arc<Self> {
        Arc::new(Self {
            mutation_sink: None,
            recent_attempts_ms: Mutex::new(BTreeMap::new()),
        })
    }

    fn with_mutation_sink(mutation_sink: Arc<dyn SoracloudRuntimeMutationSink>) -> Arc<Self> {
        Arc::new(Self {
            mutation_sink: Some(mutation_sink),
            recent_attempts_ms: Mutex::new(BTreeMap::new()),
        })
    }

    fn report(
        &self,
        view: &StateView<'_>,
        validator_account_id: &AccountId,
        kind: SoraModelHostViolationKindV1,
        placement_id: Option<Hash>,
        detail: Option<String>,
    ) {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };
        let key = ModelHostViolationReportKey {
            validator_account_id: validator_account_id.clone(),
            kind,
            placement_id,
        };
        if self.authoritative_evidence_exists(view, &key) || !self.cooldown_allows_attempt(&key) {
            return;
        }
        let log_key = key.clone();
        let instruction = InstructionBox::from(isi::soracloud::ReportSoracloudModelHostViolation {
            validator_account_id: key.validator_account_id.clone(),
            kind: key.kind,
            placement_id: key.placement_id,
            detail,
        });
        if let Err(error) = mutation_sink.submit_instruction(
            instruction,
            "/internal/soracloud/runtime/model-host-violation",
        ) {
            iroha_logger::warn!(
                ?error,
                validator_account_id = %log_key.validator_account_id,
                kind = ?log_key.kind,
                placement_id = ?log_key.placement_id,
                "failed to submit Soracloud model-host violation from runtime health"
            );
            return;
        }
        if let Err(error) = mutation_sink.submit_instruction(
            InstructionBox::from(isi::soracloud::ReconcileSoracloudModelHosts),
            "/internal/soracloud/runtime/model-host-reconcile",
        ) {
            iroha_logger::warn!(
                ?error,
                validator_account_id = %log_key.validator_account_id,
                kind = ?log_key.kind,
                placement_id = ?log_key.placement_id,
                "failed to submit Soracloud model-host reconcile after runtime health report"
            );
        }
    }

    fn authoritative_evidence_exists(
        &self,
        view: &StateView<'_>,
        key: &ModelHostViolationReportKey,
    ) -> bool {
        view.world()
            .soracloud_model_host_violation_evidence()
            .iter()
            .any(|(_evidence_id, record)| {
                record.validator_account_id == key.validator_account_id
                    && record.kind == key.kind
                    && record.placement_id == key.placement_id
            })
    }

    fn cooldown_allows_attempt(&self, key: &ModelHostViolationReportKey) -> bool {
        let now_ms = soracloud_runtime_observed_at_ms();
        let mut recent_attempts_ms = self.recent_attempts_ms.lock();
        if let Some(last_attempt_ms) = recent_attempts_ms.get(key)
            && now_ms.saturating_sub(*last_attempt_ms) < MODEL_HOST_VIOLATION_REPORT_COOLDOWN_MS
        {
            return false;
        }
        recent_attempts_ms.insert(key.clone(), now_ms);
        true
    }
}

impl SoracloudRuntimeManagerConfig {
    /// Build a runtime-manager configuration from the parsed Soracloud runtime settings.
    #[must_use]
    pub fn from_runtime_config(
        config: &iroha_config::parameters::actual::SoracloudRuntime,
    ) -> Self {
        config.assert_production_posture();
        Self {
            production_mode: config.production_mode,
            state_dir: config.state_dir.clone(),
            reconcile_interval: config.reconcile_interval,
            hydration_concurrency: config.hydration_concurrency,
            cache_budgets: config.cache_budgets.clone(),
            inrou: config.inrou,
            submission: config.submission.clone(),
            egress: config.egress.clone(),
            hf: config.hf.clone(),
            local_validator_account_id: None,
            local_peer_id: None,
        }
    }

    /// Attach the local host identity used for placement-aware HF execution.
    #[must_use]
    pub fn with_local_host_identity(
        mut self,
        validator_account_id: AccountId,
        peer_id: impl Into<String>,
    ) -> Self {
        self.local_validator_account_id = Some(validator_account_id);
        self.local_peer_id = Some(peer_id.into());
        self
    }
}

/// Executable handle to the embedded Soracloud runtime manager.
#[derive(Clone)]
pub struct SoracloudRuntimeManagerHandle {
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    config: Arc<SoracloudRuntimeManagerConfig>,
    state_dir: Arc<PathBuf>,
    state: Arc<State>,
    hf_local_workers: SharedHfLocalRunnerWorkers,
    host_violation_reporter: Arc<SoracloudModelHostViolationReporter>,
    mutation_sink: Option<Arc<dyn SoracloudRuntimeMutationSink>>,
    generated_hf_reconcile_attempts_ms: Arc<Mutex<BTreeMap<Hash, u64>>>,
}

impl SoracloudRuntimeManagerHandle {
    /// Return the latest materialization snapshot.
    #[must_use]
    pub fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        self.snapshot.read().clone()
    }

    /// Return the runtime-manager state directory.
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.state_dir.as_ref().clone()
    }

    fn report_generated_hf_proxy_failure(
        &self,
        request: &SoracloudLocalReadRequest,
        target_peer_id: &str,
        error: &SoracloudRuntimeExecutionError,
    ) {
        if request.handler_name != "infer"
            || error.kind == SoracloudRuntimeExecutionErrorKind::InvalidRequest
        {
            return;
        }
        let view = self.state.view();
        let Some(bundle) = view.world().soracloud_service_revisions().get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        )) else {
            return;
        };
        let Some(binding) = soracloud_hf_generated_source_binding(bundle) else {
            return;
        };
        let Ok(Some(placement)) = resolve_active_hf_placement_for_service(
            &view,
            request.service_name.as_str(),
            &binding.source_id,
        ) else {
            return;
        };
        let Some(primary_assignment) = placement.assigned_hosts.iter().find(|assignment| {
            assignment.role == SoraHfPlacementHostRoleV1::Primary
                && assignment.status == SoraHfPlacementHostStatusV1::Warm
                && assignment.peer_id == target_peer_id
        }) else {
            return;
        };
        self.host_violation_reporter.report(
            &view,
            &primary_assignment.validator_account_id,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
            Some(placement.placement_id),
            Some(format!(
                "proxied HF inference for service `{}` targeting primary peer `{target_peer_id}` failed: {}",
                request.service_name, error.message
            )),
        );
    }

    fn report_generated_hf_local_proxy_failure(
        &self,
        request: &SoracloudLocalReadRequest,
        error: &SoracloudRuntimeExecutionError,
    ) {
        if request.handler_name != "infer"
            || error.kind == SoracloudRuntimeExecutionErrorKind::InvalidRequest
        {
            return;
        }
        let view = self.state.view();
        let Some(bundle) = view.world().soracloud_service_revisions().get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        )) else {
            return;
        };
        let Some(binding) = soracloud_hf_generated_source_binding(bundle) else {
            return;
        };
        let Ok(Some(placement)) = resolve_active_hf_placement_for_service(
            &view,
            request.service_name.as_str(),
            &binding.source_id,
        ) else {
            return;
        };
        let Some(local_assignment) = placement
            .assigned_hosts
            .iter()
            .find(|assignment| hf_assignment_matches_local_host(&self.config, assignment))
        else {
            return;
        };
        let kind = match local_assignment.status {
            SoraHfPlacementHostStatusV1::Warm => {
                SoraModelHostViolationKindV1::AssignedHeartbeatMiss
            }
            SoraHfPlacementHostStatusV1::Warming => SoraModelHostViolationKindV1::WarmupNoShow,
            SoraHfPlacementHostStatusV1::Unavailable | SoraHfPlacementHostStatusV1::Retired => {
                return;
            }
        };
        self.host_violation_reporter.report(
            &view,
            &local_assignment.validator_account_id,
            kind,
            Some(placement.placement_id),
            Some(format!(
                "local assigned {} peer `{}` failed to forward generated-HF proxy traffic for service `{}` before reaching the authoritative primary: {}",
                match local_assignment.role {
                    SoraHfPlacementHostRoleV1::Primary => "primary",
                    SoraHfPlacementHostRoleV1::Replica => "replica",
                },
                local_assignment.peer_id,
                request.service_name,
                error.message
            )),
        );
    }

    fn report_local_generated_hf_authority_failure(
        &self,
        view: &StateView<'_>,
        placement: &SoraHfPlacementRecordV1,
        request: &SoracloudLocalReadRequest,
        error: &SoracloudRuntimeExecutionError,
    ) -> bool {
        let Some(local_assignment) = placement
            .assigned_hosts
            .iter()
            .find(|assignment| hf_assignment_matches_local_host(&self.config, assignment))
        else {
            return false;
        };
        if local_assignment.role != SoraHfPlacementHostRoleV1::Primary {
            return false;
        }

        let kind = match local_assignment.status {
            SoraHfPlacementHostStatusV1::Warm => {
                SoraModelHostViolationKindV1::AssignedHeartbeatMiss
            }
            SoraHfPlacementHostStatusV1::Warming => SoraModelHostViolationKindV1::WarmupNoShow,
            SoraHfPlacementHostStatusV1::Unavailable | SoraHfPlacementHostStatusV1::Retired => {
                return false;
            }
        };
        self.host_violation_reporter.report(
            view,
            &local_assignment.validator_account_id,
            kind,
            Some(placement.placement_id),
            Some(format!(
                "local authoritative primary peer `{}` rejected generated-HF proxy execution for service `{}`: {}",
                local_assignment.peer_id, request.service_name, error.message
            )),
        );
        true
    }

    fn request_generated_hf_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        error: &SoracloudRuntimeExecutionError,
    ) {
        if request.handler_name != "infer"
            || error.kind != SoracloudRuntimeExecutionErrorKind::Unavailable
        {
            return;
        }
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };
        let view = self.state.view();
        let Some(bundle) = view.world().soracloud_service_revisions().get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        )) else {
            return;
        };
        let Some(binding) = soracloud_hf_generated_source_binding(bundle) else {
            return;
        };
        let Ok(Some(placement)) = resolve_active_hf_placement_for_service(
            &view,
            request.service_name.as_str(),
            &binding.source_id,
        ) else {
            return;
        };
        if self.report_local_generated_hf_authority_failure(&view, &placement, request, error) {
            return;
        }
        let has_warm_primary =
            iroha_core::soracloud_runtime::resolve_generated_hf_primary_assignment(
                view.world(),
                request.service_name.as_str(),
                &binding.source_id,
            )
            .ok()
            .flatten()
            .is_some();
        let local_assignment_present = placement
            .assigned_hosts
            .iter()
            .any(|assignment| hf_assignment_matches_local_host(&self.config, assignment));
        if has_warm_primary && !local_assignment_present {
            return;
        }
        if !self.generated_hf_reconcile_attempt_allowed(placement.placement_id) {
            return;
        }
        if let Err(submit_error) = mutation_sink.submit_instruction(
            InstructionBox::from(isi::soracloud::ReconcileSoracloudModelHosts),
            "/internal/soracloud/runtime/model-host-reconcile-hint",
        ) {
            iroha_logger::warn!(
                ?submit_error,
                placement_id = %placement.placement_id,
                service_name = %request.service_name,
                service_version = %request.service_version,
                "failed to enqueue Soracloud model-host reconcile after generated-HF routing failure"
            );
        }
    }

    fn request_generated_hf_proxy_responder_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        responder_peer_id: &str,
        expected_peer_id: &str,
    ) {
        if request.handler_name != "infer" || responder_peer_id == expected_peer_id {
            return;
        }
        if self.mutation_sink.is_none() {
            return;
        }
        let view = self.state.view();
        let Some(bundle) = view.world().soracloud_service_revisions().get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        )) else {
            return;
        };
        let Some(binding) = soracloud_hf_generated_source_binding(bundle) else {
            return;
        };
        let Ok(Some(placement)) = resolve_active_hf_placement_for_service(
            &view,
            request.service_name.as_str(),
            &binding.source_id,
        ) else {
            return;
        };
        let Some(unexpected_assignment) = placement.assigned_hosts.iter().find(|assignment| {
            assignment.peer_id == responder_peer_id
                && assignment.status != SoraHfPlacementHostStatusV1::Retired
        }) else {
            return;
        };
        let kind = match unexpected_assignment.status {
            SoraHfPlacementHostStatusV1::Warm | SoraHfPlacementHostStatusV1::Unavailable => {
                SoraModelHostViolationKindV1::AssignedHeartbeatMiss
            }
            SoraHfPlacementHostStatusV1::Warming => SoraModelHostViolationKindV1::WarmupNoShow,
            SoraHfPlacementHostStatusV1::Retired => return,
        };
        self.host_violation_reporter.report(
            &view,
            &unexpected_assignment.validator_account_id,
            kind,
            Some(placement.placement_id),
            Some(format!(
                "assigned {} peer `{}` answered a generated-HF proxy response for service `{}` even though authoritative primary peer is `{expected_peer_id}`",
                match unexpected_assignment.role {
                    SoraHfPlacementHostRoleV1::Primary => "primary",
                    SoraHfPlacementHostRoleV1::Replica => "replica",
                },
                responder_peer_id,
                request.service_name,
            )),
        );
    }

    fn generated_hf_reconcile_attempt_allowed(&self, placement_id: Hash) -> bool {
        let now_ms = soracloud_runtime_observed_at_ms();
        let mut attempts = self.generated_hf_reconcile_attempts_ms.lock();
        if let Some(previous_attempt_ms) = attempts.get(&placement_id)
            && now_ms.saturating_sub(*previous_attempt_ms)
                < GENERATED_HF_RECONCILE_REQUEST_COOLDOWN_MS
        {
            return false;
        }
        attempts.insert(placement_id, now_ms);
        true
    }
}

fn uploaded_model_encryption_key_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_DIR)
}

fn uploaded_model_encryption_key_path(state_dir: &Path) -> PathBuf {
    uploaded_model_encryption_key_dir(state_dir).join(SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_FILE)
}

fn load_or_create_uploaded_model_encryption_secret(state_dir: &Path) -> io::Result<[u8; 32]> {
    let key_dir = uploaded_model_encryption_key_dir(state_dir);
    fs::create_dir_all(&key_dir)?;
    let key_path = uploaded_model_encryption_key_path(state_dir);
    match fs::read(&key_path) {
        Ok(bytes) => bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "uploaded model encryption key at `{}` must be exactly 32 bytes",
                    key_path.display()
                ),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let secret = X25519StaticSecret::from(rand::random::<[u8; 32]>());
            let bytes = secret.to_bytes();
            fs::write(&key_path, bytes)?;
            Ok(bytes)
        }
        Err(error) => Err(error),
    }
}

fn load_or_create_uploaded_model_encryption_recipient(
    state_dir: &Path,
) -> io::Result<SoracloudUploadedModelEncryptionRecipient> {
    let secret_bytes = load_or_create_uploaded_model_encryption_secret(state_dir)?;
    let secret = X25519StaticSecret::from(secret_bytes);
    let public_key_bytes = X25519PublicKey::from(&secret).to_bytes().to_vec();
    let public_key_fingerprint = Hash::new(public_key_bytes.as_slice());
    Ok(SoracloudUploadedModelEncryptionRecipient {
        schema_version: SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        key_id: format!(
            "soracloud-upload-x25519:{}",
            hex::encode(&public_key_bytes[..8])
        ),
        key_version: std::num::NonZeroU32::new(SORACLOUD_UPLOADED_MODEL_UPLOAD_KEY_VERSION_V1)
            .expect("non-zero upload key version"),
        kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
        aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
        public_key_bytes,
        public_key_fingerprint,
    })
}

impl SoracloudRuntimeReadHandle for SoracloudRuntimeManagerHandle {
    fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        SoracloudRuntimeManagerHandle::snapshot(self)
    }

    fn state_dir(&self) -> PathBuf {
        SoracloudRuntimeManagerHandle::state_dir(self)
    }

    fn uploaded_model_encryption_recipient(
        &self,
    ) -> Option<SoracloudUploadedModelEncryptionRecipient> {
        load_or_create_uploaded_model_encryption_recipient(self.state_dir.as_ref().as_path()).ok()
    }

    fn local_peer_id(&self) -> Option<String> {
        self.config.local_peer_id.clone()
    }

    fn local_read_proxy_timeout(&self) -> Duration {
        self.config.hf.request_timeout
    }

    fn report_generated_hf_proxy_failure(
        &self,
        request: &SoracloudLocalReadRequest,
        target_peer_id: &str,
        error: &SoracloudRuntimeExecutionError,
    ) {
        Self::report_generated_hf_proxy_failure(self, request, target_peer_id, error);
    }

    fn report_generated_hf_local_proxy_failure(
        &self,
        request: &SoracloudLocalReadRequest,
        error: &SoracloudRuntimeExecutionError,
    ) {
        Self::report_generated_hf_local_proxy_failure(self, request, error);
    }

    fn request_generated_hf_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        error: &SoracloudRuntimeExecutionError,
    ) {
        Self::request_generated_hf_reconcile(self, request, error);
    }

    fn request_generated_hf_proxy_responder_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        responder_peer_id: &str,
        expected_peer_id: &str,
    ) {
        Self::request_generated_hf_proxy_responder_reconcile(
            self,
            request,
            responder_peer_id,
            expected_peer_id,
        );
    }
}

impl SoracloudRuntime for SoracloudRuntimeManagerHandle {
    fn execute_local_read(
        &self,
        request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
        let view = self.state.view();
        let snapshot = self.snapshot();
        validate_local_runtime_snapshot(&view, &snapshot, &request)?;
        let context = resolve_local_read_context(&view, &request, &self.config)?;

        match request.handler_class {
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset => {
                execute_asset_local_read(&request, &context, self.state_dir.as_ref())
            }
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query => {
                execute_query_local_read(
                    &view,
                    &request,
                    &context,
                    self.state_dir.as_ref(),
                    &self.config.egress,
                    &self.config.hf,
                    &self.hf_local_workers,
                    &self.host_violation_reporter,
                )
            }
        }
    }

    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        if request.handler.is_none() {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "missing_handler",
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        if let Err(message) = ensure_ivm_runtime(
            request.bundle.service.execution_plane,
            request.bundle.container.runtime,
            request.deployment.service_name.as_ref(),
            &request.deployment.current_service_version,
        ) {
            return Ok(deterministic_mailbox_failure_result_with_message(
                request,
                "invalid_runtime",
                message,
                SoraServiceHealthStatusV1::Degraded,
            ));
        }

        let bundle_cache_path = self
            .state_dir
            .join("artifacts")
            .join(hash_cache_name(request.bundle.container.bundle_hash));
        let bundle_bytes = match read_and_verify_cached_artifact(
            &bundle_cache_path,
            request.bundle.container.bundle_hash,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result_with_message(
                    request,
                    "bundle_unavailable",
                    error.message,
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };

        let verified = match verify_contract_artifact(&bundle_bytes) {
            Ok(verified) => verified,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result_with_message(
                    request,
                    "invalid_bundle",
                    error.to_string(),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };
        let Some(entrypoint) = verified
            .contract_interface
            .entrypoints
            .iter()
            .find(|entrypoint| {
                request
                    .handler
                    .as_ref()
                    .is_some_and(|handler| entrypoint.name == handler.entrypoint)
            })
        else {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "missing_entrypoint",
                SoraServiceHealthStatusV1::Degraded,
            ));
        };

        let mailbox_payload_tlv =
            match mailbox_payload_tlv_bytes(&request.mailbox_message.payload_bytes) {
                Ok(tlv_bytes) => tlv_bytes,
                Err(error) => {
                    return Ok(deterministic_mailbox_failure_result(
                        request,
                        vm_error_label(&error),
                        SoraServiceHealthStatusV1::Degraded,
                    ));
                }
            };
        let public_inputs = match ordered_mailbox_public_inputs(
            &mailbox_payload_tlv,
            request.execution_sequence,
            request.observed_height,
        ) {
            Ok(public_inputs) => public_inputs,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result(
                    request,
                    vm_error_label(&error),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };

        let committed_entries = collect_committed_service_state_entries(
            &self.state.view(),
            request.deployment.service_name.as_ref(),
        );
        let host = SoracloudIvmHost::new(
            request.clone(),
            self.state_dir(),
            self.config.egress.clone(),
            committed_entries,
        )
        .with_public_inputs(public_inputs);
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);
        if let Err(error) = vm.load_program(&bundle_bytes) {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        let entry_pc = u64::try_from(verified.code_offset.saturating_sub(verified.header_len))
            .unwrap_or(u64::MAX)
            .saturating_add(entrypoint.entry_pc);
        if let Err(error) = vm.set_program_counter(entry_pc) {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        match vm.alloc_input_tlv(&mailbox_payload_tlv) {
            Ok(ptr) => vm.set_register(10, ptr),
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result(
                    request,
                    vm_error_label(&error),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };
        vm.set_register(11, request.execution_sequence);
        vm.set_register(12, request.observed_height);
        if let Err(error) = vm.run() {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        let (response_bytes, content_type) = match decode_ordered_mailbox_vm_output(&vm, &request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result_with_message(
                    request,
                    "invalid_response",
                    error.message,
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };
        let Some(host) = vm
            .host_mut_any()
            .and_then(|host| host.downcast_mut::<SoracloudIvmHost>())
        else {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "host_unavailable",
                SoraServiceHealthStatusV1::Degraded,
            ));
        };
        match std::mem::replace(
            host,
            SoracloudIvmHost::new(
                request.clone(),
                self.state_dir(),
                self.config.egress.clone(),
                BTreeMap::new(),
            ),
        )
        .into_execution_result(response_bytes, content_type)
        {
            Ok(result) => Ok(result),
            Err(error) => Ok(deterministic_mailbox_failure_result_with_message(
                request,
                "materialization_failure",
                error.message,
                SoraServiceHealthStatusV1::Degraded,
            )),
        }
    }

    fn execute_apartment(
        &self,
        request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
        let view = self.state.view();
        let snapshot = self.snapshot();
        validate_apartment_snapshot(&view, &snapshot, &request)?;
        let Some(record) = view
            .world()
            .soracloud_agent_apartments()
            .get(&request.apartment_name)
        else {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("unknown Soracloud apartment `{}`", request.apartment_name),
            ));
        };
        if record.process_generation != request.process_generation {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "apartment `{}` process generation {} does not match committed generation {}",
                    request.apartment_name, request.process_generation, record.process_generation
                ),
            ));
        }

        if let Some(run_id) = parse_apartment_autonomy_run_id(&request.operation).map(str::to_owned)
        {
            return execute_apartment_autonomy_run(self, &view, record, request, &run_id);
        }

        Ok(SoracloudApartmentExecutionResult {
            status: record.status,
            checkpoint_artifact_hash: None,
            journal_artifact_hash: None,
            result_commitment: apartment_result_commitment(
                &request.apartment_name,
                request.process_generation,
                &request.operation,
                request.request_commitment,
                record.status,
            ),
        })
    }

}

#[derive(Clone, Debug)]
struct StagedRuntimeArtifact {
    artifact_path: String,
    bytes: Vec<u8>,
    artifact_hash: Hash,
}

struct SoracloudIvmHost {
    request: SoracloudOrderedMailboxExecutionRequest,
    state_dir: PathBuf,
    egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
    core_host: CoreHost,
    public_inputs: BTreeMap<Name, Vec<u8>>,
    committed_entries: BTreeMap<(String, String), SoraServiceStateEntryV1>,
    binding_totals: BTreeMap<String, u64>,
    observed_local_read_bindings:
        BTreeMap<(String, String), iroha_core::soracloud_runtime::SoracloudLocalReadBinding>,
    staged_state_mutations: Vec<iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation>,
    staged_outbound_mailbox_messages: Vec<SoraServiceMailboxMessageV1>,
    staged_journal: Option<StagedRuntimeArtifact>,
    staged_checkpoint: Option<StagedRuntimeArtifact>,
    egress_requests: u32,
    egress_bytes: u64,
}

impl SoracloudIvmHost {
    fn new(
        request: SoracloudOrderedMailboxExecutionRequest,
        state_dir: PathBuf,
        egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
        committed_entries: BTreeMap<(String, String), SoraServiceStateEntryV1>,
    ) -> Self {
        let mut binding_totals = BTreeMap::new();
        for entry in committed_entries.values() {
            let total = binding_totals
                .entry(entry.binding_name.to_string())
                .or_insert(0u64);
            *total = total.saturating_add(entry.payload_bytes.get());
        }
        Self {
            request,
            state_dir,
            egress,
            core_host: CoreHost::new(),
            public_inputs: BTreeMap::new(),
            committed_entries,
            binding_totals,
            observed_local_read_bindings: BTreeMap::new(),
            staged_state_mutations: Vec::new(),
            staged_outbound_mailbox_messages: Vec::new(),
            staged_journal: None,
            staged_checkpoint: None,
            egress_requests: 0,
            egress_bytes: 0,
        }
    }

    fn with_public_inputs(mut self, public_inputs: BTreeMap<Name, Vec<u8>>) -> Self {
        self.public_inputs = public_inputs;
        self
    }

    fn handler_class(&self) -> SoraServiceHandlerClassV1 {
        self.request
            .handler
            .as_ref()
            .map(|handler| handler.class)
            .unwrap_or(SoraServiceHandlerClassV1::Update)
    }

    fn service_name(&self) -> &Name {
        &self.request.deployment.service_name
    }

    fn service_version(&self) -> &str {
        &self.request.deployment.current_service_version
    }

    fn require_private_runtime(&self, syscall: u32) -> Result<(), VMError> {
        if self.handler_class() == SoraServiceHandlerClassV1::PrivateUpdate {
            Ok(())
        } else {
            Err(VMError::metered_not_implemented(
                ivm::gas::G_SORACLOUD,
                syscall,
            ))
        }
    }

    fn require_mutating_runtime(&self, _syscall: u32) -> Result<(), VMError> {
        match self.handler_class() {
            SoraServiceHandlerClassV1::Update | SoraServiceHandlerClassV1::PrivateUpdate => Ok(()),
            _ => Err(VMError::metered(
                ivm::gas::G_SORACLOUD,
                VMError::PermissionDenied,
            )),
        }
    }

    fn read_request_payload(
        &self,
        vm: &mut IVM,
        expected_operation: SoracloudHostOperationV1,
        syscall: u32,
    ) -> Result<(SoracloudHostRequestPayloadV1, usize), VMError> {
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .map_err(|err| VMError::metered(ivm::gas::G_SORACLOUD, err))?;
        let request_bytes = tlv.payload.len();
        let request_gas = ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0);
        if tlv.type_id != PointerType::SoracloudRequest {
            return Err(VMError::metered(
                request_gas,
                VMError::AbiTypeNotAllowed {
                    abi: vm.abi_version(),
                    type_id: tlv.type_id_raw(),
                },
            ));
        }
        let envelope = norito::decode_from_bytes::<SoracloudHostRequestEnvelopeV1>(tlv.payload)
            .map_err(|_| VMError::metered(request_gas, VMError::NoritoInvalid))?;
        envelope
            .validate()
            .map_err(|_| VMError::metered(request_gas, VMError::NoritoInvalid))?;
        if envelope.operation != expected_operation {
            return Err(VMError::metered_not_implemented(request_gas, syscall));
        }
        Ok((envelope.payload, request_bytes))
    }

    fn write_response(
        &self,
        vm: &mut IVM,
        operation: SoracloudHostOperationV1,
        payload: SoracloudHostResponsePayloadV1,
        request_bytes: usize,
    ) -> Result<u64, VMError> {
        let envelope = SoracloudHostResponseEnvelopeV1 {
            schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
            operation,
            payload,
        };
        let request_gas = ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0);
        envelope
            .validate()
            .map_err(|_| VMError::metered(request_gas, VMError::NoritoInvalid))?;
        let payload_bytes = norito::to_bytes(&envelope)
            .map_err(|_| VMError::metered(request_gas, VMError::NoritoInvalid))?;
        let gas =
            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, payload_bytes.len());
        let tlv = make_pointer_tlv(PointerType::SoracloudResponse, &payload_bytes);
        let ptr = vm
            .alloc_input_tlv(&tlv)
            .map_err(|err| VMError::metered(gas, err))?;
        vm.set_register(10, ptr);
        Ok(gas)
    }

    fn binding(&self, binding_name: &Name) -> Result<&SoraStateBindingV1, VMError> {
        self.request
            .bundle
            .service
            .state_bindings
            .iter()
            .find(|binding| binding.binding_name == *binding_name)
            .ok_or(VMError::PermissionDenied)
    }

    fn state_entry_key(binding_name: &Name, state_key: &str) -> (String, String) {
        (binding_name.to_string(), state_key.to_owned())
    }

    fn current_entry_size(&self, binding_name: &Name, state_key: &str) -> u64 {
        let key = Self::state_entry_key(binding_name, state_key);
        self.committed_entries
            .get(&key)
            .map(|entry| entry.payload_bytes.get())
            .unwrap_or(0)
    }

    fn stage_state_mutation(
        &mut self,
        request: SoracloudEmitStateMutationRequestV1,
    ) -> Result<SoracloudEmitStateMutationResponseV1, VMError> {
        self.require_mutating_runtime(SYSCALL_SORACLOUD_EMIT_STATE_MUTATION)?;
        if !self
            .request
            .bundle
            .container
            .capabilities
            .allow_state_writes
        {
            return Err(VMError::PermissionDenied);
        }
        if request.state_key.trim().is_empty() || !request.state_key.starts_with('/') {
            return Err(VMError::PermissionDenied);
        }
        let binding = self.binding(&request.binding_name)?.clone();
        if !request.state_key.starts_with(&binding.key_prefix) {
            return Err(VMError::PermissionDenied);
        }
        if binding.encryption != request.encryption {
            return Err(VMError::PermissionDenied);
        }
        let binding_name = request.binding_name.to_string();
        let current_size = self.current_entry_size(&request.binding_name, &request.state_key);
        match request.operation {
            SoraStateMutationOperationV1::Upsert => {
                let Some(payload) = request.payload.as_ref() else {
                    return Err(VMError::NoritoInvalid);
                };
                let payload_bytes =
                    u64::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
                if payload_bytes == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                if request
                    .payload_bytes
                    .is_some_and(|declared| declared != payload_bytes)
                {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_commitment = Hash::new(payload);
                if request
                    .payload_commitment
                    .is_some_and(|declared| declared != payload_commitment)
                {
                    return Err(VMError::NoritoInvalid);
                }
                if payload_bytes > binding.max_item_bytes.get() {
                    return Err(VMError::PermissionDenied);
                }
                if !matches!(
                    binding.mutability,
                    iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly
                        | iroha_data_model::soracloud::SoraStateMutabilityV1::ReadWrite
                ) {
                    return Err(VMError::PermissionDenied);
                }
                if binding.mutability
                    == iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly
                    && current_size > 0
                {
                    return Err(VMError::PermissionDenied);
                }
                let current_total = self.binding_totals.get(&binding_name).copied().unwrap_or(0);
                let next_total = current_total
                    .saturating_sub(current_size)
                    .saturating_add(payload_bytes);
                if next_total > binding.max_total_bytes.get() {
                    return Err(VMError::PermissionDenied);
                }
                self.binding_totals.insert(binding_name.clone(), next_total);
                self.committed_entries.insert(
                    Self::state_entry_key(&request.binding_name, &request.state_key),
                    SoraServiceStateEntryV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                        service_name: self.request.deployment.service_name.clone(),
                        service_version: self.request.deployment.current_service_version.clone(),
                        binding_name: request.binding_name.clone(),
                        state_key: request.state_key.clone(),
                        encryption: request.encryption,
                        payload: payload.clone(),
                        payload_bytes: std::num::NonZeroU64::new(payload_bytes)
                            .ok_or(VMError::NoritoInvalid)?,
                        payload_commitment,
                        last_update_sequence: self.request.execution_sequence,
                        governance_tx_hash: self.request.mailbox_message.payload_commitment,
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );
            }
            SoraStateMutationOperationV1::Delete => {
                if request.payload_bytes.is_some()
                    || request.payload.is_some()
                    || request.payload_commitment.is_some()
                {
                    return Err(VMError::NoritoInvalid);
                }
                if binding.mutability
                    != iroha_data_model::soracloud::SoraStateMutabilityV1::ReadWrite
                {
                    return Err(VMError::PermissionDenied);
                }
                let current_total = self.binding_totals.get(&binding_name).copied().unwrap_or(0);
                self.binding_totals.insert(
                    binding_name.clone(),
                    current_total.saturating_sub(current_size),
                );
                self.committed_entries.remove(&Self::state_entry_key(
                    &request.binding_name,
                    &request.state_key,
                ));
            }
        }

        let mutation_payload_bytes = request
            .payload
            .as_ref()
            .and_then(|payload| u64::try_from(payload.len()).ok());
        let mutation_payload_commitment = request.payload.as_ref().map(Hash::new);
        let mutation = iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation {
            binding_name,
            state_key: request.state_key.clone(),
            operation: request.operation,
            encryption: request.encryption,
            payload_bytes: mutation_payload_bytes,
            payload: request.payload.clone(),
            payload_commitment: mutation_payload_commitment,
        };
        let mutation_commitment = Hash::new(Encode::encode(&(
            "soracloud.host.state-mutation.v1",
            self.request.mailbox_message.message_id,
            mutation.binding_name.as_str(),
            mutation.state_key.as_str(),
            mutation.operation,
            mutation.encryption,
            mutation.payload_bytes,
            mutation.payload_commitment,
            u64::try_from(self.staged_state_mutations.len()).unwrap_or(u64::MAX),
        )));
        self.staged_state_mutations.push(mutation);
        Ok(SoracloudEmitStateMutationResponseV1 {
            mutation_commitment,
        })
    }

    fn stage_outbound_mailbox_message(
        &mut self,
        request: SoracloudEmitMailboxMessageRequestV1,
    ) -> Result<SoracloudEmitMailboxMessageResponseV1, VMError> {
        self.require_mutating_runtime(SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE)?;
        let payload_commitment = Hash::new(&request.payload_bytes);
        let message_id = Hash::new(Encode::encode(&(
            "soracloud.host.mailbox.v1",
            self.request.mailbox_message.message_id,
            self.request.deployment.service_name.as_ref(),
            self.request.mailbox_message.to_handler.as_ref(),
            request.to_service.as_ref(),
            request.to_handler.as_ref(),
            payload_commitment,
            request.available_after_sequence,
            request.expires_at_sequence,
            u64::try_from(self.staged_outbound_mailbox_messages.len()).unwrap_or(u64::MAX),
        )));
        self.staged_outbound_mailbox_messages
            .push(SoraServiceMailboxMessageV1 {
                schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
                message_id,
                from_service: self.request.deployment.service_name.clone(),
                from_handler: self.request.mailbox_message.to_handler.clone(),
                to_service: request.to_service,
                to_handler: request.to_handler,
                payload_bytes: request.payload_bytes,
                payload_commitment,
                enqueue_sequence: self.request.execution_sequence,
                available_after_sequence: request
                    .available_after_sequence
                    .max(self.request.execution_sequence),
                expires_at_sequence: request.expires_at_sequence,
            });
        Ok(SoracloudEmitMailboxMessageResponseV1 {
            message_id,
            payload_commitment,
        })
    }

    fn stage_artifact(
        slot: &mut Option<StagedRuntimeArtifact>,
        request: String,
        bytes: Vec<u8>,
    ) -> Hash {
        let artifact_hash = Hash::new(&bytes);
        *slot = Some(StagedRuntimeArtifact {
            artifact_path: request,
            bytes,
            artifact_hash,
        });
        artifact_hash
    }

    fn read_material(&self, root_name: &str, key: &str) -> Result<Option<Vec<u8>>, VMError> {
        if root_name == "configs" {
            if let Some(config) = self.request.deployment.service_configs.get(key) {
                return Ok(Some(config.value_json.get().as_bytes().to_vec()));
            }
        }
        if root_name == "secrets" {
            if let Some(secret) = self.request.deployment.service_secrets.get(key) {
                return Ok(Some(secret.envelope.ciphertext.clone()));
            }
        }
        let relative = sanitized_relative_material_path(key)?;
        let path = self
            .state_dir
            .join(root_name)
            .join(sanitize_path_component(self.service_name().as_ref()))
            .join(sanitize_path_component(self.service_version()))
            .join(relative);
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(VMError::PermissionDenied),
        }
    }

    fn read_service_config(
        &self,
        config_name: &str,
    ) -> Result<SoracloudReadConfigResponseV1, VMError> {
        let payload_bytes = self.read_material("configs", config_name)?;
        Ok(SoracloudReadConfigResponseV1 {
            found: payload_bytes.is_some(),
            payload_bytes: payload_bytes.unwrap_or_default(),
        })
    }

    fn read_service_secret_envelope(
        &self,
        secret_name: &str,
    ) -> SoracloudReadSecretEnvelopeResponseV1 {
        SoracloudReadSecretEnvelopeResponseV1 {
            envelope: self
                .request
                .deployment
                .service_secrets
                .get(secret_name)
                .map(|entry| entry.envelope.clone()),
        }
    }

    fn read_public_input(&self, vm: &mut IVM) -> Result<u64, VMError> {
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != PointerType::Name {
            return Err(VMError::NoritoInvalid);
        }
        let name: Name =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        let Some(bytes) = self.public_inputs.get(&name) else {
            return Err(VMError::PermissionDenied);
        };
        let dst = vm.alloc_input_tlv(bytes)?;
        vm.set_register(10, dst);
        Ok(0)
    }

    fn host_network_allows(&self, host: &str, port: u16) -> bool {
        let container_policy: &SoraCapabilityPolicyV1 = &self.request.bundle.container.capabilities;
        if !container_policy.network.allows_host_port(host, port) {
            return false;
        }
        if self.egress.default_allow {
            true
        } else {
            self.egress
                .allowed_hosts
                .iter()
                .any(|allowed| allowed == host)
        }
    }

    fn egress_fetch(
        &mut self,
        request: SoracloudEgressFetchRequestV1,
    ) -> Result<SoracloudEgressFetchResponseV1, VMError> {
        self.require_private_runtime(SYSCALL_SORACLOUD_EGRESS_FETCH)?;
        let Some(expected_hash) = request.expected_hash else {
            return Err(VMError::PermissionDenied);
        };
        let Some((host, port)) = url_host_port(&request.url) else {
            return Err(VMError::PermissionDenied);
        };
        if !self.host_network_allows(&host, port) {
            return Err(VMError::PermissionDenied);
        }
        let max_requests = self
            .egress
            .rate_per_minute
            .map(|value| value.get())
            .unwrap_or(u32::MAX);
        if self.egress_requests >= max_requests {
            return Err(VMError::PermissionDenied);
        }
        let remaining_budget = self
            .egress
            .max_bytes_per_minute
            .map(|value| value.get())
            .unwrap_or(u64::MAX)
            .saturating_sub(self.egress_bytes);
        let response_cap = remaining_budget.min(request.max_bytes);
        if response_cap == 0 {
            return Err(VMError::PermissionDenied);
        }
        let response = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| VMError::PermissionDenied)?
            .get(&request.url)
            .send()
            .map_err(|_| VMError::PermissionDenied)?;
        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let body = response
            .bytes()
            .map_err(|_| VMError::PermissionDenied)?
            .to_vec();
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > response_cap {
            return Err(VMError::PermissionDenied);
        }
        let body_hash = Hash::new(&body);
        if body_hash != expected_hash {
            return Err(VMError::PermissionDenied);
        }
        self.egress_requests = self.egress_requests.saturating_add(1);
        self.egress_bytes = self
            .egress_bytes
            .saturating_add(u64::try_from(body.len()).unwrap_or(u64::MAX));
        Ok(SoracloudEgressFetchResponseV1 {
            status_code,
            content_type,
            body,
            body_hash,
        })
    }

    fn into_execution_result(
        self,
        response_bytes: Vec<u8>,
        content_type: Option<String>,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        let handler_class = self.handler_class();
        let journal_artifact_hash = persist_staged_runtime_artifact(
            self.state_dir.join("journals"),
            self.staged_journal.as_ref(),
        )?;
        let checkpoint_artifact_hash = persist_staged_runtime_artifact(
            self.state_dir.join("checkpoints"),
            self.staged_checkpoint.as_ref(),
        )?;
        let runtime_state = updated_runtime_state_with_outbound_mailbox(
            self.request.runtime_state.clone(),
            &self.request,
            SoraServiceHealthStatusV1::Healthy,
            &self.staged_outbound_mailbox_messages,
        );
        let result_commitment = authoritative_mailbox_result_commitment(
            &self.request,
            &self.staged_state_mutations,
            &self.staged_outbound_mailbox_messages,
            &response_bytes,
            content_type.as_deref(),
            &runtime_state,
            journal_artifact_hash,
            checkpoint_artifact_hash,
        );
        let receipt_id = Hash::new(Encode::encode(&(
            "soracloud:runtime-receipt:v1",
            self.request.mailbox_message.message_id,
            self.request.deployment.service_name.as_ref(),
            self.request.deployment.current_service_version.as_str(),
            self.request.mailbox_message.to_handler.as_ref(),
            self.request.execution_sequence,
            result_commitment,
        )));
        Ok(SoracloudOrderedMailboxExecutionResult {
            state_mutations: self.staged_state_mutations,
            outbound_mailbox_messages: self.staged_outbound_mailbox_messages,
            response_bytes,
            content_type,
            runtime_state: Some(runtime_state),
            runtime_receipt: SoraRuntimeReceiptV1 {
                schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id,
                service_name: self.request.deployment.service_name,
                service_version: self.request.deployment.current_service_version,
                handler_name: self.request.mailbox_message.to_handler.clone(),
                handler_class,
                request_commitment: self.request.mailbox_message.payload_commitment,
                result_commitment,
                certified_by: SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: self.request.execution_sequence,
                mailbox_message_id: Some(self.request.mailbox_message.message_id),
                journal_artifact_hash,
                checkpoint_artifact_hash,
                placement_id: None,
                selected_validator_account_id: None,
                selected_peer_id: None,
            },
        })
    }

    fn local_read_bindings(&self) -> Vec<iroha_core::soracloud_runtime::SoracloudLocalReadBinding> {
        self.observed_local_read_bindings
            .values()
            .cloned()
            .collect::<Vec<_>>()
    }

    fn has_local_read_side_effects(&self) -> bool {
        !self.staged_state_mutations.is_empty()
            || !self.staged_outbound_mailbox_messages.is_empty()
            || self.staged_journal.is_some()
            || self.staged_checkpoint.is_some()
    }
}

impl IVMHost for SoracloudIvmHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            ivm_syscalls::SYSCALL_GET_PUBLIC_INPUT => self.read_public_input(vm),
            SYSCALL_SORACLOUD_READ_COMMITTED_STATE => {
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::ReadCommittedState,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::ReadCommittedState(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let entry = self
                    .committed_entries
                    .get(&Self::state_entry_key(
                        &request.binding_name,
                        &request.state_key,
                    ))
                    .cloned();
                if let Some(entry) = entry.as_ref() {
                    self.observed_local_read_bindings.insert(
                        Self::state_entry_key(&request.binding_name, &request.state_key),
                        state_entry_binding(entry),
                    );
                }
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCommittedState,
                    SoracloudHostResponsePayloadV1::ReadCommittedState(
                        SoracloudReadCommittedStateResponseV1 { entry },
                    ),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => {
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::EmitStateMutation,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let response = self.stage_state_mutation(request).map_err(|err| {
                    VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        err.into_unmetered(),
                    )
                })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitStateMutation,
                    SoracloudHostResponsePayloadV1::EmitStateMutation(response),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => {
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::EmitMailboxMessage,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::EmitMailboxMessage(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let response = self
                    .stage_outbound_mailbox_message(request)
                    .map_err(|err| {
                        VMError::metered(
                            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                            err.into_unmetered(),
                        )
                    })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitMailboxMessage,
                    SoracloudHostResponsePayloadV1::EmitMailboxMessage(response),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_APPEND_JOURNAL => {
                let (payload, request_bytes) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::AppendJournal, number)?;
                let SoracloudHostRequestPayloadV1::AppendJournal(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                self.require_mutating_runtime(number).map_err(|err| {
                    VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        err,
                    )
                })?;
                let artifact_hash = Self::stage_artifact(
                    &mut self.staged_journal,
                    request.artifact_path,
                    request.payload_bytes,
                );
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::AppendJournal,
                    SoracloudHostResponsePayloadV1::AppendJournal(
                        SoracloudAppendJournalResponseV1 { artifact_hash },
                    ),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => {
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::PublishCheckpoint,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::PublishCheckpoint(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                self.require_mutating_runtime(number).map_err(|err| {
                    VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        err,
                    )
                })?;
                let artifact_hash = Self::stage_artifact(
                    &mut self.staged_checkpoint,
                    request.artifact_path,
                    request.payload_bytes,
                );
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::PublishCheckpoint,
                    SoracloudHostResponsePayloadV1::PublishCheckpoint(
                        SoracloudPublishCheckpointResponseV1 { artifact_hash },
                    ),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_READ_CONFIG => {
                let (payload, request_bytes) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::ReadConfig, number)?;
                let SoracloudHostRequestPayloadV1::ReadConfig(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let response = self
                    .read_service_config(&request.config_name)
                    .map_err(|err| {
                        VMError::metered(
                            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                            err,
                        )
                    })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadConfig,
                    SoracloudHostResponsePayloadV1::ReadConfig(response),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => {
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::ReadSecretEnvelope,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::ReadSecretEnvelope(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let response = self.read_service_secret_envelope(&request.secret_name);
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadSecretEnvelope,
                    SoracloudHostResponsePayloadV1::ReadSecretEnvelope(response),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_READ_SECRET => {
                self.require_private_runtime(number)?;
                let (payload, request_bytes) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::ReadSecret, number)?;
                let SoracloudHostRequestPayloadV1::ReadSecret(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let payload_bytes = self
                    .read_material("secrets", &request.secret_name)
                    .map_err(|err| {
                        VMError::metered(
                            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                            err,
                        )
                    })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadSecret,
                    SoracloudHostResponsePayloadV1::ReadSecret(SoracloudReadSecretResponseV1 {
                        found: payload_bytes.is_some(),
                        payload_bytes: payload_bytes.unwrap_or_default(),
                    }),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_READ_CREDENTIAL => {
                self.require_private_runtime(number)?;
                let (payload, request_bytes) = self.read_request_payload(
                    vm,
                    SoracloudHostOperationV1::ReadCredential,
                    number,
                )?;
                let SoracloudHostRequestPayloadV1::ReadCredential(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let payload_bytes = self
                    .read_material("credentials", &request.credential_name)
                    .map_err(|err| {
                        VMError::metered(
                            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                            err,
                        )
                    })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCredential,
                    SoracloudHostResponsePayloadV1::ReadCredential(
                        SoracloudReadCredentialResponseV1 {
                            found: payload_bytes.is_some(),
                            payload_bytes: payload_bytes.unwrap_or_default(),
                        },
                    ),
                    request_bytes,
                )
            }
            SYSCALL_SORACLOUD_EGRESS_FETCH => {
                let (payload, request_bytes) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::EgressFetch, number)?;
                let SoracloudHostRequestPayloadV1::EgressFetch(request) = payload else {
                    return Err(VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        VMError::NoritoInvalid,
                    ));
                };
                let response = self.egress_fetch(request).map_err(|err| {
                    VMError::metered(
                        ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, request_bytes, 0),
                        err.into_unmetered(),
                    )
                })?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EgressFetch,
                    SoracloudHostResponsePayloadV1::EgressFetch(response),
                    request_bytes,
                )
            }
            _ => self.core_host.syscall(number, vm),
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any
    where
        Self: 'static,
    {
        self
    }
}

#[derive(Clone)]
struct ResolvedLocalReadContext {
    deployment: SoraServiceDeploymentStateV1,
    bundle: SoraDeploymentBundleV1,
    handler: SoraServiceHandlerV1,
    hf_execution_host: Option<ResolvedHfPlacementExecutionHost>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedHfPlacementExecutionHost {
    placement_id: Hash,
    validator_account_id: AccountId,
    peer_id: String,
    role: SoraHfPlacementHostRoleV1,
    status: SoraHfPlacementHostStatusV1,
}

const HF_LOCAL_IMPORT_SCHEMA_VERSION_V1: u16 = 1;
const HF_LOCAL_RUNNER_REQUEST_SCHEMA_VERSION_V1: u16 = 1;
const HF_LOCAL_RUNNER_SCRIPT_V1: &str = include_str!("../resources/soracloud_hf_local_runner.py");
const APARTMENT_AUTONOMY_OPERATION_PREFIX_V1: &str = "autonomy-run:";
const APARTMENT_AUTONOMY_HANDLER_NAME_V1: &str = "infer";
const APARTMENT_AUTONOMY_HANDLER_PATH_V1: &str = "/infer";
const APARTMENT_AUTONOMY_SUMMARY_FILE_V1: &str = "execution_summary.json";
const APARTMENT_AUTONOMY_CHECKPOINT_FILE_V1: &str = "checkpoint.bin";
const APARTMENT_AUTONOMY_WORKFLOW_VERSION_V1: u64 = 1;
const HF_ALLOW_BRIDGE_FALLBACK_HEADER_V1: &str = "x-soracloud-hf-allow-bridge-fallback";

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct HfImportedFileV1 {
    path: String,
    content_length: u64,
    payload_hash: String,
    local_path: String,
}

#[derive(Clone, Debug)]
struct ApartmentAutonomyWorkflowStepSpec {
    step_index: u32,
    step_id: Option<String>,
    request: norito::json::Value,
    allow_bridge_fallback: bool,
}

#[derive(Clone, Debug)]
struct ApartmentAutonomyWorkflowExecutionError {
    message: String,
    workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct HfLocalImportManifestV1 {
    schema_version: u16,
    source_id: String,
    repo_id: String,
    requested_revision: String,
    resolved_commit: Option<String>,
    model_name: String,
    adapter_id: String,
    pipeline_tag: Option<String>,
    library_name: Option<String>,
    #[norito(default)]
    tags: Vec<String>,
    imported_at_ms: u64,
    #[norito(default)]
    imported_files: Vec<HfImportedFileV1>,
    #[norito(default)]
    skipped_files: Vec<String>,
    #[norito(default)]
    raw_model_info_path: Option<String>,
    #[norito(default)]
    import_error: Option<String>,
}

type SharedHfLocalRunnerWorkers = Arc<Mutex<BTreeMap<String, Arc<Mutex<HfLocalRunnerWorker>>>>>;
type SharedHostedHttpWorkers =
    Arc<Mutex<BTreeMap<(String, String, u16), Arc<Mutex<HostedHttpWorker>>>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct HfLocalRunnerWorkerCacheKey {
    source_id: String,
    repo_id: String,
    resolved_revision: String,
    model_name: String,
    adapter_id: String,
    pipeline_tag: Option<String>,
    library_name: Option<String>,
    imported_at_ms: u64,
    source_files_dir: PathBuf,
    runner_program: String,
    runner_script_path: PathBuf,
    runner_script_revision: String,
}

struct HfLocalRunnerWorker {
    cache_key: HfLocalRunnerWorkerCacheKey,
    child: std::process::Child,
    stdin: ChildStdin,
    stdout_rx: mpsc::Receiver<io::Result<Vec<u8>>>,
    stdout_reader: Option<thread::JoinHandle<()>>,
    stderr_log_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HostedHttpWorkerCacheKey {
    runtime: SoraContainerRuntimeV1,
    backend: Option<SoraInrouRuntimeBackendV1>,
    guest_isa: Option<SoraInrouGuestIsaV1>,
    service_name: String,
    service_version: String,
    replica_slot: u16,
    bundle_hash: String,
    bundle_path: String,
    entrypoint: String,
    process_generation: u64,
    args: Vec<String>,
    effective_env: BTreeMap<String, String>,
    healthcheck_path: Option<String>,
    service_data_dir: PathBuf,
}

struct HostedHttpWorker {
    cache_key: HostedHttpWorkerCacheKey,
    child: std::process::Child,
    listen_base_url: String,
    egress_accounting_offset_bytes: u64,
    attachment: Option<HostedHttpWorkerAttachment>,
    stderr_log_path: PathBuf,
}

enum HostedHttpWorkerAttachment {
    #[allow(dead_code)]
    FirecrackerKvm(InrouTapNetworkAttachment),
    PortableVm(PortableVmAttachment),
}

struct PortableVmAttachment {
    metadata_server: Option<PortableVmMetadataServer>,
}

struct PortableVmMetadataServer {
    bind_addr: SocketAddr,
    shutdown_tx: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct InrouTapNetworkAttachment {
    ip_binary: PathBuf,
    iptables_binary: PathBuf,
    exportfs_binary: Option<PathBuf>,
    tap_name: String,
    host_ip: String,
    guest_ip: String,
    guest_mac: String,
    firewall_plan: InrouTapFirewallPlan,
    installed_firewall_rules: Vec<Vec<String>>,
    installed_nfs_exports: Vec<InrouNfsExport>,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum InrouTapFirewallPlan {
    Open,
    Isolated,
    Allowlist(Vec<InrouTapResolvedAllowlistEndpoint>),
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct InrouTapResolvedAllowlistEndpoint {
    host: String,
    address: Ipv4Addr,
    port: u16,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouTapFirewallRuleSpec {
    args: Vec<String>,
    context: &'static str,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouNfsExport {
    guest_client: String,
    export_path_on_host: PathBuf,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl InrouTapFirewallPlan {
    fn installs_masquerade_rule(&self) -> bool {
        matches!(self, Self::Open | Self::Allowlist(_))
    }

    fn installs_return_rule(&self) -> bool {
        matches!(self, Self::Open | Self::Allowlist(_))
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct HfGeneratedMetadataResponse {
    schema_version: u16,
    source_id: String,
    repo_id: String,
    requested_revision: String,
    resolved_commit: Option<String>,
    model_name: String,
    adapter_id: String,
    pipeline_tag: Option<String>,
    library_name: Option<String>,
    #[norito(default)]
    tags: Vec<String>,
    imported: bool,
    imported_at_ms: Option<u64>,
    imported_file_count: u32,
    imported_total_bytes: u64,
    #[norito(default)]
    imported_files: Vec<HfImportedFileV1>,
    #[norito(default)]
    skipped_files: Vec<String>,
    #[norito(default)]
    import_error: Option<String>,
    inference_local_enabled: bool,
    inference_bridge_enabled: bool,
}

/// Embedded `irohad` Soracloud runtime-manager actor.
pub(crate) struct SoracloudRuntimeManager {
    config: SoracloudRuntimeManagerConfig,
    state: Arc<State>,
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    hf_local_workers: SharedHfLocalRunnerWorkers,
    hosted_http_workers: SharedHostedHttpWorkers,
    host_violation_reporter: Arc<SoracloudModelHostViolationReporter>,
    mutation_sink: Option<Arc<dyn SoracloudRuntimeMutationSink>>,
    last_model_host_heartbeat_attempt_ms: Mutex<Option<u64>>,
    last_inrou_host_advert_attempt_ms: Mutex<Option<u64>>,
    last_inrou_placement_reconcile_attempt_ms: Mutex<Option<u64>>,
    last_runtime_state_submission_commitments: Mutex<BTreeMap<(String, String, u16), Hash>>,
    last_service_lease_usage_submission_bytes: Mutex<BTreeMap<(String, String), u64>>,
    sorafs_node: Option<sorafs_node::NodeHandle>,
    sorafs_provider_cache: Option<Arc<AsyncRwLock<ProviderAdvertCache>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationSource {
    manifest_digest_hex: String,
    manifest_cid_hex: String,
    chunker_handle: Option<String>,
    provider_ids: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationPlan {
    manifest_id_hex: String,
    chunker_handle: String,
    content_length: u64,
    chunks: Vec<RemoteHydrationChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationChunk {
    offset: u64,
    length: u32,
    digest_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SorafsHydratedFileLayout {
    path: Vec<String>,
    offset: u64,
    size: u64,
}

fn stable_inrou_runtime_state_submission_view(
    state: &SoraInrouReplicaRuntimeStateV1,
) -> SoraInrouReplicaRuntimeStateV1 {
    let mut stable_state = state.clone();
    stable_state.updated_at_ms = 0;
    stable_state
}

fn inrou_runtime_state_submission_commitment(state: &SoraInrouReplicaRuntimeStateV1) -> Hash {
    Hash::new(Encode::encode(&stable_inrou_runtime_state_submission_view(
        state,
    )))
}

fn inrou_runtime_state_matches_authoritative_snapshot(
    authoritative: &SoraInrouReplicaRuntimeStateV1,
    desired: &SoraInrouReplicaRuntimeStateV1,
) -> bool {
    stable_inrou_runtime_state_submission_view(authoritative)
        == stable_inrou_runtime_state_submission_view(desired)
}

impl SoracloudRuntimeManager {
    /// Construct the runtime manager for the supplied node state.
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, state: Arc<State>) -> Self {
        Self {
            config,
            state,
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
            hf_local_workers: Arc::new(Mutex::new(BTreeMap::new())),
            hosted_http_workers: Arc::new(Mutex::new(BTreeMap::new())),
            host_violation_reporter: SoracloudModelHostViolationReporter::disabled(),
            mutation_sink: None,
            last_model_host_heartbeat_attempt_ms: Mutex::new(None),
            last_inrou_host_advert_attempt_ms: Mutex::new(None),
            last_inrou_placement_reconcile_attempt_ms: Mutex::new(None),
            last_runtime_state_submission_commitments: Mutex::new(BTreeMap::new()),
            last_service_lease_usage_submission_bytes: Mutex::new(BTreeMap::new()),
            sorafs_node: None,
            sorafs_provider_cache: None,
        }
    }

    /// Attach the authoritative mutation sink used for runtime-originated Soracloud health reports.
    #[must_use]
    pub(crate) fn with_mutation_sink(
        mut self,
        mutation_sink: Arc<dyn SoracloudRuntimeMutationSink>,
    ) -> Self {
        self.host_violation_reporter =
            SoracloudModelHostViolationReporter::with_mutation_sink(Arc::clone(&mutation_sink));
        self.mutation_sink = Some(mutation_sink);
        self
    }

    /// Attach the embedded SoraFS storage handle used for authoritative hydration.
    #[must_use]
    pub fn with_sorafs_node(mut self, sorafs_node: sorafs_node::NodeHandle) -> Self {
        self.sorafs_node = Some(sorafs_node);
        self
    }

    /// Attach the shared SoraFS provider-discovery cache used for remote hydration.
    #[must_use]
    pub fn with_sorafs_provider_cache(
        mut self,
        sorafs_provider_cache: Arc<AsyncRwLock<ProviderAdvertCache>>,
    ) -> Self {
        self.sorafs_provider_cache = Some(sorafs_provider_cache);
        self
    }

    /// Start the background reconciliation loop.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> (SoracloudRuntimeManagerHandle, Child) {
        let manager = Arc::new(self);
        if let Err(error) = manager.restore_persisted_snapshot() {
            iroha_logger::warn!(
                ?error,
                state_dir = %manager.config.state_dir.display(),
                "failed to restore persisted Soracloud runtime-manager snapshot"
            );
        }
        if let Err(error) = Arc::clone(&manager).run_startup_reconcile() {
            iroha_logger::warn!(
                ?error,
                state_dir = %manager.config.state_dir.display(),
                "initial Soracloud runtime-manager reconciliation failed"
            );
        }
        let handle = SoracloudRuntimeManagerHandle {
            snapshot: Arc::clone(&manager.snapshot),
            config: Arc::new(manager.config.clone()),
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state: Arc::clone(&manager.state),
            hf_local_workers: Arc::clone(&manager.hf_local_workers),
            host_violation_reporter: Arc::clone(&manager.host_violation_reporter),
            mutation_sink: manager.mutation_sink.as_ref().map(Arc::clone),
            generated_hf_reconcile_attempts_ms: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let task = Arc::clone(&manager).spawn_reconcile_task(shutdown_signal);
        (
            handle,
            Child::new(task, OnShutdown::Wait(Duration::from_secs(1))),
        )
    }

    fn run_startup_reconcile(self: Arc<Self>) -> eyre::Result<()> {
        std::thread::Builder::new()
            .name("soracloud-runtime-startup-reconcile".to_owned())
            .spawn(move || self.reconcile_once())
            .wrap_err("spawn Soracloud startup reconcile thread")?
            .join()
            .map_err(|panic| {
                eyre::eyre!("Soracloud startup reconcile thread panicked: {panic:?}")
            })?
    }

    fn spawn_reconcile_task(self: Arc<Self>, shutdown_signal: ShutdownSignal) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(self.config.reconcile_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let manager = Arc::clone(&self);
                        match tokio::task::spawn_blocking(move || manager.reconcile_once()).await {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                iroha_logger::warn!(
                                    ?error,
                                    state_dir = %self.config.state_dir.display(),
                                    "Soracloud runtime-manager reconciliation failed"
                                );
                            }
                            Err(error) => {
                                iroha_logger::warn!(
                                    ?error,
                                    state_dir = %self.config.state_dir.display(),
                                    "Soracloud runtime-manager reconciliation task panicked"
                                );
                            }
                        }
                    }
                    () = shutdown_signal.receive() => {
                        iroha_logger::debug!("Soracloud runtime manager is being shut down.");
                        break;
                    }
                    else => break,
                }
            }
        })
    }

    /// Reconcile the node-local materialization plan against authoritative state once.
    pub(crate) fn reconcile_once(&self) -> eyre::Result<()> {
        fs::create_dir_all(self.services_root())
            .wrap_err_with(|| format!("create {}", self.services_root().display()))?;
        fs::create_dir_all(self.apartments_root())
            .wrap_err_with(|| format!("create {}", self.apartments_root().display()))?;
        fs::create_dir_all(self.artifacts_root())
            .wrap_err_with(|| format!("create {}", self.artifacts_root().display()))?;
        fs::create_dir_all(self.journals_root())
            .wrap_err_with(|| format!("create {}", self.journals_root().display()))?;
        fs::create_dir_all(self.checkpoints_root())
            .wrap_err_with(|| format!("create {}", self.checkpoints_root().display()))?;
        fs::create_dir_all(self.secrets_root())
            .wrap_err_with(|| format!("create {}", self.secrets_root().display()))?;
        fs::create_dir_all(self.credentials_root())
            .wrap_err_with(|| format!("create {}", self.credentials_root().display()))?;
        fs::create_dir_all(self.hf_sources_root())
            .wrap_err_with(|| format!("create {}", self.hf_sources_root().display()))?;
        fs::create_dir_all(self.service_data_root())
            .wrap_err_with(|| format!("create {}", self.service_data_root().display()))?;

        let (
            bundle_registry,
            initial_snapshot,
            inrou_host_capability_refresh,
            inrou_placement_reconcile_needed,
        ) = {
            let view = self.state.view();
            self.report_local_model_host_advert_contradictions(&view);
            let bundle_registry = collect_service_revision_registry(&view);
            let inrou_host_capability_refresh =
                self.local_inrou_host_capability_refresh_candidate(&view);
            let inrou_placement_reconcile_needed =
                self.inrou_placement_reconcile_needed(&view, &bundle_registry);
            let initial_snapshot = build_runtime_snapshot(
                &view,
                &bundle_registry,
                &self.config.state_dir,
                self.artifacts_root(),
                self.config.local_validator_account_id.as_ref(),
                self.config.local_peer_id.as_deref(),
                self.config.inrou.enabled && !self.config.inrou.proxy_only,
            )?;
            (
                bundle_registry,
                initial_snapshot,
                inrou_host_capability_refresh,
                inrou_placement_reconcile_needed,
            )
        };
        self.refresh_local_inrou_host_capability_if_needed(inrou_host_capability_refresh);
        self.request_inrou_placement_reconcile_if_needed(inrou_placement_reconcile_needed);

        {
            let view = self.state.view();
            self.write_service_materializations(&initial_snapshot, &bundle_registry, &view)?;
            self.write_apartment_materializations(&initial_snapshot, &view)?;
        }
        self.prune_stale_service_materializations(&initial_snapshot)?;
        self.prune_stale_secret_materializations(&initial_snapshot)?;
        self.prune_stale_apartment_materializations(&initial_snapshot)?;
        {
            let view = self.state.view();
            self.import_hf_sources(&view, &initial_snapshot)?;
        }
        {
            let view = self.state.view();
            self.probe_local_hf_execution_hosts(&view, &initial_snapshot);
        }
        {
            let view = self.state.view();
            self.hydrate_missing_artifacts(&view, &initial_snapshot, &bundle_registry)?;
        }
        {
            let view = self.state.view();
            self.reconcile_hosted_http_workers(&view, &initial_snapshot, &bundle_registry)?;
        }
        {
            let view = self.state.view();
            self.enforce_cache_budgets(&view, &initial_snapshot)?;
        }
        let snapshot = {
            let view = self.state.view();
            build_runtime_snapshot(
                &view,
                &bundle_registry,
                &self.config.state_dir,
                self.artifacts_root(),
                self.config.local_validator_account_id.as_ref(),
                self.config.local_peer_id.as_deref(),
                self.config.inrou.enabled && !self.config.inrou.proxy_only,
            )?
        };
        self.prune_stale_hf_local_workers(&snapshot);
        {
            let view = self.state.view();
            self.submit_http_service_runtime_state_updates(&view, &snapshot, &bundle_registry);
        }
        write_json_atomic(
            &self.config.state_dir.join("runtime_snapshot.json"),
            &snapshot,
        )?;
        *self.snapshot.write() = snapshot;
        Ok(())
    }

    fn submit_http_service_runtime_state_updates(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };

        let desired_keys = snapshot
            .services
            .iter()
            .flat_map(|(service_name, versions)| {
                versions.iter().flat_map(move |(service_version, plan)| {
                    (plan.runtime == SoraContainerRuntimeV1::Inrou)
                        .then_some(
                            plan.local_replicas
                                .iter()
                                .map(|replica| {
                                    (
                                        service_name.clone(),
                                        service_version.clone(),
                                        replica.replica_slot,
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                        .unwrap_or_default()
                })
            })
            .collect::<BTreeSet<_>>();

        let clearable_keys = match (
            self.config.local_validator_account_id.as_ref(),
            self.config.local_peer_id.as_deref(),
        ) {
            (Some(local_validator_account_id), Some(local_peer_id)) => view
                .world()
                .soracloud_inrou_replica_runtime()
                .iter()
                .filter_map(|((service_name, service_version, replica_slot), state)| {
                    if &state.validator_account_id != local_validator_account_id
                        || state.peer_id != local_peer_id
                    {
                        return None;
                    }
                    let replica_slot = replica_slot.parse::<u16>().ok()?;
                    let key = (service_name.clone(), service_version.clone(), replica_slot);
                    (!desired_keys.contains(&key)).then_some(key)
                })
                .collect::<BTreeSet<_>>(),
            _ => BTreeSet::new(),
        };

        let tracked_keys = desired_keys
            .iter()
            .cloned()
            .chain(clearable_keys.iter().cloned())
            .collect::<BTreeSet<_>>();
        self.last_runtime_state_submission_commitments
            .lock()
            .retain(|key, _commitment| tracked_keys.contains(key));

        for (service_name, service_version, replica_slot) in clearable_keys {
            let commitment = Hash::new(Encode::encode(&(
                "soracloud:inrou-runtime-clear:v1",
                service_name.as_str(),
                service_version.as_str(),
                replica_slot,
            )));
            let key = (service_name.clone(), service_version.clone(), replica_slot);
            if self
                .last_runtime_state_submission_commitments
                .lock()
                .get(&key)
                .is_some_and(|previous| *previous == commitment)
            {
                continue;
            }
            let service_name_id = match Name::from_str(&service_name) {
                Ok(name) => name,
                Err(error) => {
                    iroha_logger::warn!(
                        ?error,
                        service_name = %service_name,
                        service_version = %service_version,
                        replica_slot,
                        "failed to parse Soracloud service name while clearing authoritative Inrou replica runtime state"
                    );
                    continue;
                }
            };
            let instruction =
                InstructionBox::from(isi::soracloud::ClearSoracloudInrouReplicaRuntimeState {
                    service_name: service_name_id,
                    service_version: service_version.clone(),
                    replica_slot,
                });
            if let Err(error) = mutation_sink.submit_instruction(
                instruction,
                "/internal/soracloud/runtime/inrou-replica-runtime-state-clear",
            ) {
                iroha_logger::warn!(
                    ?error,
                    service_name = %service_name,
                    service_version = %service_version,
                    replica_slot,
                    "failed to clear authoritative Inrou replica runtime state from embedded runtime manager"
                );
                continue;
            }
            self.last_runtime_state_submission_commitments
                .lock()
                .insert(key, commitment);
        }

        for (service_name, versions) in &snapshot.services {
            for (service_version, plan) in versions {
                if plan.runtime != SoraContainerRuntimeV1::Inrou || plan.local_replicas.is_empty() {
                    continue;
                }
                let Some(bundle) =
                    bundle_registry.get(&(service_name.clone(), service_version.clone()))
                else {
                    continue;
                };
                let service_name_id = match Name::from_str(service_name) {
                    Ok(name) => name,
                    Err(error) => {
                        iroha_logger::warn!(
                            ?error,
                            service_name = %service_name,
                            service_version = %service_version,
                            "failed to parse Soracloud service name while submitting authoritative Inrou replica runtime state"
                        );
                        continue;
                    }
                };

                for replica in &plan.local_replicas {
                    let Some(assignment) =
                        view.world()
                            .soracloud_inrou_service_placements()
                            .get(&(service_name.clone(), service_version.clone()))
                            .and_then(|record| {
                                record.placements.iter().find(|placement| {
                                    placement.replica_slot == replica.replica_slot
                                })
                            })
                    else {
                        continue;
                    };
                    let authoritative_state =
                        view.world().soracloud_inrou_replica_runtime().get(&(
                            service_name.clone(),
                            service_version.clone(),
                            replica.replica_slot.to_string(),
                        ));
                    let desired_state = SoraInrouReplicaRuntimeStateV1 {
                        schema_version: SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
                        service_name: service_name_id.clone(),
                        service_version: service_version.clone(),
                        replica_slot: replica.replica_slot,
                        validator_account_id: assignment.validator_account_id.clone(),
                        peer_id: assignment.peer_id.clone(),
                        selected_backend: assignment.selected_backend,
                        selected_guest_isa: assignment.selected_guest_isa,
                        health_status: replica.health_status,
                        load_factor_bps: plan.load_factor_bps,
                        materialized_bundle_hash: bundle.container.bundle_hash,
                        accounted_egress_bytes: 0,
                        pending_mailbox_message_count: 0,
                        last_receipt_id: authoritative_state
                            .and_then(|state| state.last_receipt_id),
                        updated_at_ms: soracloud_runtime_observed_at_ms(),
                        last_error: replica.last_error.clone(),
                    };
                    if authoritative_state.is_some_and(|state| {
                        inrou_runtime_state_matches_authoritative_snapshot(state, &desired_state)
                    }) {
                        self.last_runtime_state_submission_commitments
                            .lock()
                            .remove(&(
                                service_name.clone(),
                                service_version.clone(),
                                replica.replica_slot,
                            ));
                        continue;
                    }

                    let commitment = inrou_runtime_state_submission_commitment(&desired_state);
                    let key = (
                        service_name.clone(),
                        service_version.clone(),
                        replica.replica_slot,
                    );
                    if authoritative_state.is_some()
                        && self
                            .last_runtime_state_submission_commitments
                            .lock()
                            .get(&key)
                            .is_some_and(|previous| *previous == commitment)
                    {
                        continue;
                    }

                    let instruction = InstructionBox::from(
                        isi::soracloud::SetSoracloudInrouReplicaRuntimeState {
                            state: desired_state,
                        },
                    );
                    if let Err(error) = mutation_sink.submit_instruction(
                        instruction,
                        "/internal/soracloud/runtime/inrou-replica-runtime-state",
                    ) {
                        iroha_logger::warn!(
                            ?error,
                            service_name = %service_name,
                            service_version = %service_version,
                            replica_slot = replica.replica_slot,
                            "failed to submit authoritative Inrou replica runtime state update from embedded runtime manager"
                        );
                        continue;
                    }
                    self.last_runtime_state_submission_commitments
                        .lock()
                        .insert(key, commitment);
                }
            }
        }
    }

    fn authoritative_service_lease_egress_bytes(
        &self,
        view: &StateView<'_>,
        service_name: &str,
        service_version: &str,
    ) -> Option<u64> {
        let service_name = Name::from_str(service_name).ok()?;
        view.world()
            .soracloud_service_deployments()
            .get(&service_name)
            .filter(|deployment| deployment.current_service_version == service_version)
            .and_then(|deployment| deployment.service_lease.as_ref())
            .map(|lease| lease.accounted_egress_bytes)
    }

    fn submit_http_service_lease_usage_update(
        &self,
        view: &StateView<'_>,
        service_name: &str,
        service_version: &str,
        accounted_egress_bytes: u64,
    ) {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };
        let key = (service_name.to_owned(), service_version.to_owned());
        let authoritative_accounted_egress_bytes =
            self.authoritative_service_lease_egress_bytes(view, service_name, service_version);
        if authoritative_accounted_egress_bytes == Some(accounted_egress_bytes) {
            self.last_service_lease_usage_submission_bytes
                .lock()
                .remove(&key);
            return;
        }
        if self
            .last_service_lease_usage_submission_bytes
            .lock()
            .get(&key)
            .is_some_and(|previous| *previous == accounted_egress_bytes)
        {
            return;
        }

        let service_name_id = match Name::from_str(service_name) {
            Ok(name) => name,
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    service_name = %service_name,
                    service_version = %service_version,
                    "failed to parse Soracloud service name while submitting authoritative lease usage"
                );
                return;
            }
        };
        let instruction = InstructionBox::from(isi::soracloud::ReportSoracloudServiceLeaseUsage {
            service_name: service_name_id,
            active_service_version: service_version.to_owned(),
            accounted_egress_bytes,
        });
        if let Err(error) = mutation_sink.submit_instruction(
            instruction,
            "/internal/soracloud/runtime/service-lease-usage",
        ) {
            iroha_logger::warn!(
                ?error,
                service_name = %service_name,
                service_version = %service_version,
                accounted_egress_bytes,
                "failed to submit authoritative Soracloud service lease usage update from embedded runtime manager"
            );
            return;
        }
        self.last_service_lease_usage_submission_bytes
            .lock()
            .insert(key, accounted_egress_bytes);
    }

    fn report_local_model_host_advert_contradictions(&self, view: &StateView<'_>) {
        let Some(validator_account_id) = self.config.local_validator_account_id.as_ref() else {
            return;
        };
        let Some(local_peer_id) = self.config.local_peer_id.as_deref() else {
            return;
        };
        let Some(capability) = view
            .world()
            .soracloud_model_host_capabilities()
            .get(validator_account_id)
        else {
            return;
        };
        if capability.peer_id == local_peer_id {
            return;
        }
        self.host_violation_reporter.report(
            view,
            validator_account_id,
            SoraModelHostViolationKindV1::AdvertContradiction,
            None,
            Some(format!(
                "local runtime peer id `{local_peer_id}` does not match the authoritative model-host advert peer id `{}` for validator `{validator_account_id}`",
                capability.peer_id
            )),
        );
    }

    fn build_local_inrou_host_capability_record(
        &self,
        now_ms: u64,
    ) -> Option<(SoraInrouHostCapabilityRecordV1, bool)> {
        if !self.config.inrou.enabled {
            return None;
        }
        let validator_account_id = self.config.local_validator_account_id.as_ref()?;
        let peer_id = self.config.local_peer_id.as_deref()?;
        let discovered_backends = supported_inrou_backends_for_host();
        let auto_proxy_only = !self.config.inrou.proxy_only && discovered_backends.is_empty();
        let proxy_only = self.config.inrou.proxy_only || auto_proxy_only;
        let supported_backends = if discovered_backends.is_empty() {
            default_zero_capacity_inrou_backends()
        } else {
            discovered_backends
        };
        let supported_guest_isas = BTreeSet::from([current_host_inrou_guest_isa()]);
        let desired_expiry_ms = desired_inrou_host_heartbeat_expiry_ms(now_ms, &self.config);

        Some((
            SoraInrouHostCapabilityRecordV1 {
                schema_version: SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                validator_account_id: validator_account_id.clone(),
                peer_id: peer_id.to_owned(),
                supported_backends,
                supported_guest_isas,
                max_hosted_replica_capacity: if proxy_only {
                    0
                } else {
                    u16::try_from(self.config.inrou.max_concurrent_vms.get()).unwrap_or(u16::MAX)
                },
                max_cpu_millis: if proxy_only { 0 } else { u32::MAX },
                max_memory_bytes: if proxy_only { 0 } else { u64::MAX },
                max_storage_bytes: if proxy_only { 0 } else { u64::MAX },
                proxy_only,
                geography_tags: Default::default(),
                observed_latency_ms: None,
                advertised_at_ms: now_ms,
                heartbeat_expires_at_ms: desired_expiry_ms,
            },
            auto_proxy_only,
        ))
    }

    fn local_inrou_host_advert_attempt_allowed(&self, now_ms: u64) -> bool {
        let mut last_attempt_ms = self.last_inrou_host_advert_attempt_ms.lock();
        if let Some(previous_attempt_ms) = *last_attempt_ms
            && now_ms.saturating_sub(previous_attempt_ms) < INROU_HOST_ADVERT_ATTEMPT_COOLDOWN_MS
        {
            return false;
        }
        *last_attempt_ms = Some(now_ms);
        true
    }

    fn local_inrou_placement_reconcile_attempt_allowed(&self, now_ms: u64) -> bool {
        let mut last_attempt_ms = self.last_inrou_placement_reconcile_attempt_ms.lock();
        if let Some(previous_attempt_ms) = *last_attempt_ms
            && now_ms.saturating_sub(previous_attempt_ms)
                < INROU_PLACEMENT_RECONCILE_ATTEMPT_COOLDOWN_MS
        {
            return false;
        }
        *last_attempt_ms = Some(now_ms);
        true
    }

    fn local_inrou_host_capability_refresh_candidate(
        &self,
        view: &StateView<'_>,
    ) -> Option<(SoraInrouHostCapabilityRecordV1, bool)> {
        let now_ms = soracloud_runtime_observed_at_ms();
        let Some((desired, auto_proxy_only)) =
            self.build_local_inrou_host_capability_record(now_ms)
        else {
            return None;
        };
        let authoritative = view
            .world()
            .soracloud_inrou_host_capabilities()
            .get(&desired.validator_account_id);
        let needs_refresh =
            inrou_host_capability_refresh_needed(authoritative, &desired, now_ms, &self.config);
        if !needs_refresh {
            return None;
        }
        Some((desired, auto_proxy_only))
    }

    fn refresh_local_inrou_host_capability_if_needed(
        &self,
        refresh: Option<(SoraInrouHostCapabilityRecordV1, bool)>,
    ) {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };
        let Some((desired, auto_proxy_only)) = refresh else {
            return;
        };
        let now_ms = soracloud_runtime_observed_at_ms();
        if !self.local_inrou_host_advert_attempt_allowed(now_ms) {
            return;
        }
        if !should_submit_local_inrou_host_capability(auto_proxy_only) {
            iroha_logger::warn!(
                validator_account_id = %desired.validator_account_id,
                peer_id = %desired.peer_id,
                "Inrou backend support is unavailable on this host; suppressing zero-capacity host advert"
            );
            return;
        }
        if let Err(error) = mutation_sink.submit_inrou_host_capability(&desired) {
            iroha_logger::warn!(
                ?error,
                validator_account_id = %desired.validator_account_id,
                peer_id = %desired.peer_id,
                "failed to submit authoritative Inrou host capability advert from embedded runtime manager"
            );
        }
    }

    fn inrou_placement_reconcile_needed(
        &self,
        view: &StateView<'_>,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) -> bool {
        let world = view.world();
        let current_sequence = current_soracloud_service_sequence(world);
        let now_ms = soracloud_runtime_observed_at_ms();
        let mut desired_records = BTreeMap::<(String, String), u16>::new();

        for (service_name, deployment) in world.soracloud_service_deployments().iter() {
            if !deployment.hosted_service_lease_active_at(current_sequence) {
                continue;
            }

            let mut active_versions = vec![deployment.current_service_version.clone()];
            if let Some(rollout) = deployment.active_rollout.as_ref()
                && rollout.traffic_percent > 0
                && rollout.candidate_version != deployment.current_service_version
            {
                active_versions.push(rollout.candidate_version.clone());
            }

            for service_version in active_versions {
                let Some(bundle) = bundle_registry
                    .get(&(service_name.as_ref().to_owned(), service_version.clone()))
                else {
                    continue;
                };
                if bundle.container.runtime != SoraContainerRuntimeV1::Inrou
                    || bundle.service.execution_plane
                        != iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService
                {
                    continue;
                }

                let key = (service_name.as_ref().to_owned(), service_version.clone());
                desired_records.insert(key.clone(), bundle.service.replicas.get());
                let Some(record) = world.soracloud_inrou_service_placements().get(&key) else {
                    return true;
                };
                if record.desired_replica_count != bundle.service.replicas.get()
                    || record.placements.len() > usize::from(record.desired_replica_count)
                {
                    return true;
                }
                for placement in &record.placements {
                    let Some(capability) = world
                        .soracloud_inrou_host_capabilities()
                        .get(&placement.validator_account_id)
                    else {
                        return true;
                    };
                    if !capability.can_host_replicas_at(now_ms)
                        || capability.peer_id != placement.peer_id
                        || !capability
                            .supported_backends
                            .contains(&placement.selected_backend)
                        || !capability
                            .supported_guest_isas
                            .contains(&placement.selected_guest_isa)
                    {
                        return true;
                    }
                }
            }
        }

        world
            .soracloud_inrou_service_placements()
            .iter()
            .any(|(key, record)| {
                desired_records
                    .get(key)
                    .is_none_or(|desired_replica_count| {
                        *desired_replica_count != record.desired_replica_count
                    })
            })
    }

    fn request_inrou_placement_reconcile_if_needed(&self, needed: bool) {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return;
        };
        if !needed {
            return;
        }
        let now_ms = soracloud_runtime_observed_at_ms();
        if !self.local_inrou_placement_reconcile_attempt_allowed(now_ms) {
            return;
        }
        if let Err(error) = mutation_sink.submit_inrou_placement_reconcile() {
            iroha_logger::warn!(
                ?error,
                "failed to submit authoritative Inrou placement reconciliation request from embedded runtime manager"
            );
        }
    }

    fn runtime_snapshot_path(&self) -> PathBuf {
        self.config.state_dir.join("runtime_snapshot.json")
    }

    fn restore_persisted_snapshot(&self) -> eyre::Result<bool> {
        let path = self.runtime_snapshot_path();
        let Some(snapshot) = read_json_optional::<SoracloudRuntimeSnapshot>(&path)
            .wrap_err_with(|| format!("read {}", path.display()))?
        else {
            return Ok(false);
        };
        *self.snapshot.write() = snapshot;
        Ok(true)
    }

    fn prune_stale_hf_local_workers(&self, snapshot: &SoracloudRuntimeSnapshot) {
        let active_sources = snapshot.hf_sources.keys().cloned().collect::<BTreeSet<_>>();
        let stale_workers = {
            let mut workers = self.hf_local_workers.lock();
            let stale_source_ids = workers
                .keys()
                .filter(|source_id| !active_sources.contains(*source_id))
                .cloned()
                .collect::<Vec<_>>();
            stale_source_ids
                .into_iter()
                .filter_map(|source_id| workers.remove(&source_id))
                .collect::<Vec<_>>()
        };
        for worker in stale_workers {
            worker.lock().stop();
        }
    }

    fn desired_hosted_http_worker_keys(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        _bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) -> BTreeSet<(String, String, u16)> {
        snapshot
            .services
            .iter()
            .flat_map(|(service_name, versions)| {
                versions.iter().filter_map(|(service_version, plan)| {
                    (plan.runtime == SoraContainerRuntimeV1::Inrou
                        && plan.process_generation.is_some())
                    .then_some((service_name.clone(), service_version.clone()))
                })
            })
            .flat_map(|(service_name, service_version)| {
                snapshot
                    .services
                    .get(&service_name)
                    .and_then(|versions| versions.get(&service_version))
                    .into_iter()
                    .flat_map(move |plan| {
                        let service_name = service_name.clone();
                        let service_version = service_version.clone();
                        plan.local_replica_slots
                            .iter()
                            .copied()
                            .map(move |replica_slot| {
                                (service_name.clone(), service_version.clone(), replica_slot)
                            })
                    })
            })
            .collect()
    }

    fn reconcile_hosted_http_workers(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) -> eyre::Result<()> {
        let desired_keys = self.desired_hosted_http_worker_keys(snapshot, bundle_registry);
        let desired_revision_keys = desired_keys
            .iter()
            .map(|(service_name, service_version, _replica_slot)| {
                (service_name.clone(), service_version.clone())
            })
            .collect::<BTreeSet<_>>();
        self.last_service_lease_usage_submission_bytes
            .lock()
            .retain(|key, _accounted_egress_bytes| desired_revision_keys.contains(key));
        let stale_workers = {
            let mut workers = self.hosted_http_workers.lock();
            let stale_keys = workers
                .keys()
                .filter(|key| !desired_keys.contains(*key))
                .cloned()
                .collect::<Vec<_>>();
            stale_keys
                .into_iter()
                .filter_map(|key| workers.remove(&key))
                .collect::<Vec<_>>()
        };
        for worker in stale_workers {
            worker.lock().stop();
        }

        let max_inrou_instances = self.hosted_http_concurrency_limit();
        let mut running_processes = {
            let workers = self.hosted_http_workers.lock();
            workers.len()
        };

        for (service_name, versions) in &snapshot.services {
            let mut hosted_http_versions = versions
                .iter()
                .filter(|(_service_version, plan)| {
                    plan.runtime == SoraContainerRuntimeV1::Inrou
                        && plan.process_generation.is_some()
                })
                .collect::<Vec<_>>();
            hosted_http_versions.sort_by_key(|(_service_version, plan)| match plan.role {
                SoracloudRuntimeRevisionRole::Active => 0u8,
                SoracloudRuntimeRevisionRole::CanaryCandidate => 1u8,
            });

            for (service_version, plan) in hosted_http_versions {
                let Some(bundle) =
                    bundle_registry.get(&(service_name.clone(), service_version.clone()))
                else {
                    continue;
                };
                let runtime_label = "inrou";
                let process_generation = plan.process_generation.unwrap_or_default();
                let service_data_dir =
                    build_native_service_data_dir(&self.config.state_dir, service_name);
                let authoritative_lease_egress_bytes = self
                    .authoritative_service_lease_egress_bytes(view, service_name, service_version)
                    .unwrap_or_default();
                let revision_lease_accounting_offset_bytes = authoritative_lease_egress_bytes.max(
                    self.last_service_lease_usage_submission_bytes
                        .lock()
                        .get(&(service_name.clone(), service_version.clone()))
                        .copied()
                        .unwrap_or_default(),
                );
                let mut replica_runtime_states = Vec::with_capacity(plan.local_replica_slots.len());
                let mut replica_accounted_egress_bytes =
                    Vec::with_capacity(plan.local_replica_slots.len());

                for replica_slot in plan.local_replica_slots.iter().copied() {
                    let replica_plan = project_hosted_http_replica_plan(plan, replica_slot);
                    let cache_key = HostedHttpWorkerCacheKey {
                        runtime: bundle.container.runtime,
                        backend: plan.inrou.as_ref().map(|inrou| inrou.selected_backend),
                        guest_isa: plan.inrou.as_ref().map(|inrou| inrou.selected_guest_isa),
                        service_name: service_name.clone(),
                        service_version: service_version.clone(),
                        replica_slot,
                        bundle_hash: plan.bundle_hash.clone(),
                        bundle_path: plan.bundle_path.clone(),
                        entrypoint: plan.entrypoint.clone(),
                        process_generation,
                        args: bundle.container.args.clone(),
                        effective_env: replica_plan.effective_env.clone(),
                        healthcheck_path: bundle.container.lifecycle.healthcheck_path.clone(),
                        service_data_dir: service_data_dir.clone(),
                    };
                    let key = (service_name.clone(), service_version.clone(), replica_slot);
                    let existing_worker = {
                        let workers = self.hosted_http_workers.lock();
                        workers.get(&key).cloned()
                    };

                    if let Some(worker) = existing_worker {
                        let mut guard = worker.lock();
                        let current_accounted_egress_bytes = guard.accounted_egress_bytes();
                        let exited = match guard.try_wait() {
                            Ok(Some(status)) => {
                                Some(format!("{runtime_label} exited with status {status}"))
                            }
                            Ok(None) => None,
                            Err(error) => {
                                Some(format!("failed to poll {runtime_label} status: {error}"))
                            }
                        };
                        let same_cache_key = guard.cache_key == cache_key;
                        if exited.is_none() && same_cache_key {
                            let health = probe_hosted_http_health(
                                &guard.listen_base_url,
                                guard.cache_key.healthcheck_path.as_deref(),
                            );
                            let (health_status, last_error) = match health {
                                Ok(()) => (SoraServiceHealthStatusV1::Healthy, None),
                                Err(error) => (
                                    SoraServiceHealthStatusV1::Degraded,
                                    Some(runtime_error_summary(&error)),
                                ),
                            };
                            let accounted_egress_bytes = current_accounted_egress_bytes
                                .unwrap_or(revision_lease_accounting_offset_bytes);
                            replica_runtime_states.push(persist_hosted_http_replica_runtime_state(
                                &PathBuf::from(&replica_plan.materialization_dir),
                                service_name,
                                service_version,
                                process_generation,
                                replica_slot,
                                health_status,
                                Some(&guard.listen_base_url),
                                guard.pid(),
                                accounted_egress_bytes,
                                last_error,
                            )?);
                            replica_accounted_egress_bytes.push(accounted_egress_bytes);
                            continue;
                        }
                        let accounted_egress_bytes = current_accounted_egress_bytes
                            .unwrap_or(revision_lease_accounting_offset_bytes);
                        guard.stop();
                        drop(guard);
                        let removed = self.hosted_http_workers.lock().remove(&key);
                        if removed.is_some() && running_processes > 0 {
                            running_processes = running_processes.saturating_sub(1);
                        }
                        if !replica_plan.bundle_available_locally {
                            replica_runtime_states.push(persist_hosted_http_replica_runtime_state(
                                &PathBuf::from(&replica_plan.materialization_dir),
                                service_name,
                                service_version,
                                process_generation,
                                replica_slot,
                                SoraServiceHealthStatusV1::Hydrating,
                                None,
                                None,
                                accounted_egress_bytes,
                                Some(format!("{runtime_label} bundle is still hydrating")),
                            )?);
                            replica_accounted_egress_bytes.push(accounted_egress_bytes);
                            continue;
                        }
                        if running_processes >= max_inrou_instances {
                            replica_runtime_states.push(
                                persist_hosted_http_replica_runtime_state(
                                    &PathBuf::from(&replica_plan.materialization_dir),
                                    service_name,
                                    service_version,
                                    process_generation,
                                    replica_slot,
                                    SoraServiceHealthStatusV1::Degraded,
                                    None,
                                    None,
                                    accounted_egress_bytes,
                                    Some(format!(
                                        "{runtime_label} concurrency limit {max_inrou_instances} is already exhausted"
                                    )),
                                )?,
                            );
                            replica_accounted_egress_bytes.push(accounted_egress_bytes);
                            continue;
                        }
                    } else if !replica_plan.bundle_available_locally {
                        replica_runtime_states.push(persist_hosted_http_replica_runtime_state(
                            &PathBuf::from(&replica_plan.materialization_dir),
                            service_name,
                            service_version,
                            process_generation,
                            replica_slot,
                            SoraServiceHealthStatusV1::Hydrating,
                            None,
                            None,
                            revision_lease_accounting_offset_bytes,
                            Some(format!("{runtime_label} bundle is still hydrating")),
                        )?);
                        replica_accounted_egress_bytes.push(revision_lease_accounting_offset_bytes);
                        continue;
                    } else if running_processes >= max_inrou_instances {
                        replica_runtime_states.push(persist_hosted_http_replica_runtime_state(
                            &PathBuf::from(&replica_plan.materialization_dir),
                            service_name,
                            service_version,
                            process_generation,
                            replica_slot,
                            SoraServiceHealthStatusV1::Degraded,
                            None,
                            None,
                            revision_lease_accounting_offset_bytes,
                            Some(format!(
                                "{runtime_label} concurrency limit {max_inrou_instances} is already exhausted"
                            )),
                        )?);
                        replica_accounted_egress_bytes.push(revision_lease_accounting_offset_bytes);
                        continue;
                    }

                    let worker = match self
                        .start_hosted_http_worker(
                            &replica_plan,
                            bundle,
                            cache_key.clone(),
                            revision_lease_accounting_offset_bytes,
                        )
                        .wrap_err_with(|| {
                            format!(
                                "start {runtime_label} Soracloud service `{service_name}` revision `{service_version}` replica {replica_slot}"
                            )
                        }) {
                        Ok(worker) => worker,
                        Err(error) => {
                            iroha_logger::warn!(
                                ?error,
                                service_name = %service_name,
                                service_version = %service_version,
                                replica_slot,
                                runtime = runtime_label,
                                "failed to start hosted Soracloud HTTP service replica"
                            );
                            replica_runtime_states.push(
                                persist_hosted_http_replica_runtime_state(
                                    &PathBuf::from(&replica_plan.materialization_dir),
                                    service_name,
                                    service_version,
                                    process_generation,
                                    replica_slot,
                                    SoraServiceHealthStatusV1::Degraded,
                                    None,
                                    None,
                                    revision_lease_accounting_offset_bytes,
                                    Some(runtime_error_summary(&error)),
                                )?,
                            );
                            replica_accounted_egress_bytes
                                .push(revision_lease_accounting_offset_bytes);
                            continue;
                        }
                    };
                    let accounted_egress_bytes = worker
                        .accounted_egress_bytes()
                        .unwrap_or(revision_lease_accounting_offset_bytes);
                    replica_runtime_states.push(persist_hosted_http_replica_runtime_state(
                        &PathBuf::from(&replica_plan.materialization_dir),
                        service_name,
                        service_version,
                        process_generation,
                        replica_slot,
                        SoraServiceHealthStatusV1::Healthy,
                        Some(&worker.listen_base_url),
                        worker.pid(),
                        accounted_egress_bytes,
                        None,
                    )?);
                    replica_accounted_egress_bytes.push(accounted_egress_bytes);
                    self.hosted_http_workers
                        .lock()
                        .insert(key, Arc::new(Mutex::new(worker)));
                    running_processes = running_processes.saturating_add(1);
                }

                let accounted_egress_bytes = aggregate_hosted_http_revision_accounted_egress_bytes(
                    revision_lease_accounting_offset_bytes,
                    &replica_accounted_egress_bytes,
                );
                let revision_listen_base_url =
                    aggregate_hosted_http_revision_listener(&replica_runtime_states)
                        .map(ToOwned::to_owned);
                let revision_pid = aggregate_hosted_http_revision_pid(&replica_runtime_states);
                let revision_last_error =
                    aggregate_hosted_http_revision_last_error(&replica_runtime_states);
                write_hosted_http_runtime_state(
                    &PathBuf::from(&plan.materialization_dir),
                    service_name,
                    service_version,
                    process_generation,
                    aggregate_hosted_http_revision_health_status(&replica_runtime_states),
                    revision_listen_base_url.as_deref(),
                    revision_pid,
                    accounted_egress_bytes,
                    revision_last_error,
                    replica_runtime_states,
                )?;
                self.submit_http_service_lease_usage_update(
                    view,
                    service_name,
                    service_version,
                    accounted_egress_bytes,
                );
            }
        }

        Ok(())
    }

    fn start_hosted_http_worker(
        &self,
        plan: &SoracloudRuntimeServicePlan,
        bundle: &SoraDeploymentBundleV1,
        cache_key: HostedHttpWorkerCacheKey,
        egress_accounting_offset_bytes: u64,
    ) -> eyre::Result<HostedHttpWorker> {
        if cache_key.runtime != SoraContainerRuntimeV1::Inrou {
            eyre::bail!(
                "unsupported hosted HTTP runtime {:?}; Soracloud hosted HTTP services must use `Inrou`",
                cache_key.runtime
            );
        }
        self.start_inrou_worker(plan, bundle, cache_key, egress_accounting_offset_bytes)
    }

    fn start_inrou_worker(
        &self,
        plan: &SoracloudRuntimeServicePlan,
        bundle: &SoraDeploymentBundleV1,
        cache_key: HostedHttpWorkerCacheKey,
        egress_accounting_offset_bytes: u64,
    ) -> eyre::Result<HostedHttpWorker> {
        let selected_backend = plan
            .inrou
            .as_ref()
            .map(|inrou| inrou.selected_backend)
            .ok_or_else(|| eyre::eyre!("Inrou runtime requires a local runtime Inrou plan"))?;
        match selected_backend {
            SoraInrouRuntimeBackendV1::FirecrackerKvm => {
                #[cfg(target_os = "linux")]
                {
                    self.start_inrou_worker_linux(
                        plan,
                        bundle,
                        cache_key,
                        egress_accounting_offset_bytes,
                    )
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = (plan, bundle, cache_key, egress_accounting_offset_bytes);
                    eyre::bail!("Inrou Firecracker/KVM execution is only available on Linux hosts");
                }
            }
            SoraInrouRuntimeBackendV1::PortableVm => self.start_inrou_worker_portable(
                plan,
                bundle,
                cache_key,
                egress_accounting_offset_bytes,
            ),
        }
    }

    fn start_inrou_worker_portable(
        &self,
        plan: &SoracloudRuntimeServicePlan,
        bundle: &SoraDeploymentBundleV1,
        cache_key: HostedHttpWorkerCacheKey,
        egress_accounting_offset_bytes: u64,
    ) -> eyre::Result<HostedHttpWorker> {
        let firewall_plan = inrou_tap_firewall_plan(&bundle.container.capabilities.network)?;
        let inrou = plan
            .inrou
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Inrou runtime requires a local runtime Inrou plan"))?;
        let profile = portable_vm_guest_machine_profile(inrou.selected_guest_isa);
        let qemu = resolve_executable_candidates(profile.emulator_candidates).ok_or_else(|| {
            eyre::eyre!(
                "Inrou PortableVm execution requires one of {:?} on PATH",
                profile.emulator_candidates
            )
        })?;
        let qemu_img = resolve_inrou_qemu_img_executable()
            .ok_or_else(|| eyre::eyre!("Inrou PortableVm execution requires `qemu-img` on PATH"))?;

        let materialization_dir = PathBuf::from(&plan.materialization_dir);
        fs::create_dir_all(&materialization_dir)
            .wrap_err_with(|| format!("create {}", materialization_dir.display()))?;
        for volume in &plan.lease_volumes {
            let volume_dir = PathBuf::from(&volume.local_materialization_dir);
            fs::create_dir_all(&volume_dir)
                .wrap_err_with(|| format!("create {}", volume_dir.display()))?;
        }

        let bundle_root = materialization_dir.join("inrou_bundle");
        ensure_native_bundle_extracted(
            &PathBuf::from(&plan.bundle_cache_path),
            &plan.bundle_hash,
            &bundle_root,
        )?;
        self.hydrate_published_inrou_guest_image_artifact(
            &bundle_root,
            bundle,
            inrou.selected_guest_isa,
        )?;
        ensure_inrou_entrypoint_present(&bundle_root, &bundle.container.entrypoint)?;
        let kernel_image_path =
            resolve_inrou_bundle_member_path(&bundle_root, &inrou.kernel_image_path)?;
        let base_rootfs_image_path =
            resolve_inrou_bundle_member_path(&bundle_root, &inrou.rootfs_image_path)?;
        let initrd_image_path = inrou
            .initrd_image_path
            .as_ref()
            .map(|path| resolve_inrou_bundle_member_path(&bundle_root, path))
            .transpose()?;
        let bootstrap_user_data = match inrou.bootstrap_user_data_path.as_ref() {
            Some(path) => Some(
                fs::read_to_string(resolve_inrou_bundle_member_path(&bundle_root, path)?)
                    .wrap_err("read Inrou bootstrap user-data overlay")?,
            ),
            None => None,
        };

        let guest_port = bundle
            .service
            .route
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Inrou service requires a route"))?
            .service_port
            .get();
        let stdout_log_path = materialization_dir.join("inrou.stdout.log");
        let stderr_log_path = materialization_dir.join("inrou.stderr.log");
        let console_log_path = materialization_dir.join("inrou.console.log");
        let stdout = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&stdout_log_path)
            .wrap_err_with(|| format!("open {}", stdout_log_path.display()))?;
        let stderr = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&stderr_log_path)
            .wrap_err_with(|| format!("open {}", stderr_log_path.display()))?;

        let (root_disk_path, root_disk_format) = match plan
            .lease_volumes
            .iter()
            .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
        {
            Some(root_volume) => (
                ensure_inrou_portable_root_disk(&qemu_img, &base_rootfs_image_path, root_volume)
                    .wrap_err("prepare Inrou mutable PortableVm root disk")?,
                "qcow2",
            ),
            None => eyre::bail!("Inrou runtime requires one PersistentRootLeaseVolume"),
        };
        let lease_disks = ensure_inrou_portable_lease_disks(&qemu_img, plan)
            .wrap_err("prepare PortableVm lease disks")?;
        let shared_filesystem_mounts = build_inrou_portable_shared_filesystem_mounts(&lease_disks);
        let network_plan = build_portable_vm_network_plan(guest_port, &firewall_plan)
            .wrap_err("prepare PortableVm user-mode networking")?;
        let hosts_overlay =
            build_portable_vm_allowlist_hosts_overlay(&network_plan.allowlist_hosts);
        let network_config = build_inrou_portable_network_config();
        let user_data = build_inrou_user_data(
            plan,
            &cache_key,
            guest_port,
            &shared_filesystem_mounts,
            bootstrap_user_data.as_deref(),
            hosts_overlay.as_deref(),
            Some(&plan.bundle_hash),
        );
        let cloud_init_root = write_inrou_cloud_init_documents(
            &materialization_dir,
            &cache_key,
            &network_config,
            &user_data,
        )
        .wrap_err("write PortableVm cloud-init documents")?;
        stage_portable_vm_metadata_bundle(
            &cloud_init_root,
            &PathBuf::from(&plan.bundle_cache_path),
        )
        .wrap_err("stage PortableVm app bundle for metadata download")?;
        let metadata_server = start_portable_vm_metadata_server(&cloud_init_root)
            .wrap_err("start PortableVm cloud-init metadata server")?;
        let datasource_base_url = metadata_server.datasource_base_url();
        let mut portable_attachment = PortableVmAttachment {
            metadata_server: Some(metadata_server),
        };
        let memory_mib = portable_vm_memory_mib(&bundle.container.resources);
        let machine_arg = format!(
            "{},accel={},memory-backend=vmmem",
            profile.machine_type,
            portable_vm_accel()?
        );

        let mut command = Command::new(&qemu);
        command
            .arg("-object")
            .arg(format!(
                "memory-backend-ram,id=vmmem,size={}M,share=on",
                memory_mib
            ))
            .arg("-machine")
            .arg(machine_arg)
            .arg("-cpu")
            .arg("max")
            .arg("-smp")
            .arg(portable_vm_vcpu_count(&bundle.container.resources).to_string())
            .arg("-kernel")
            .arg(&kernel_image_path);
        if let Some(initrd_image_path) = initrd_image_path.as_ref() {
            command.arg("-initrd").arg(initrd_image_path);
        }
        command
            .arg("-append")
            .arg(portable_vm_kernel_cmdline(profile, &datasource_base_url))
            .arg("-nodefaults")
            .arg("-no-reboot")
            .arg("-display")
            .arg("none")
            .arg("-monitor")
            .arg("none")
            .arg("-serial")
            .arg(format!("file:{}", console_log_path.display()))
            .arg("-netdev")
            .arg(&network_plan.netdev)
            .arg("-device")
            .arg(format!("{},netdev=net0", profile.net_device))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));

        append_portable_vm_drive(
            &mut command,
            profile,
            "rootfs",
            &root_disk_path,
            root_disk_format,
            false,
            true,
        );
        for (index, disk) in lease_disks.iter().enumerate() {
            append_portable_vm_drive_with_serial(
                &mut command,
                profile,
                &format!("lease{index}"),
                &disk.image_path,
                disk.image_format,
                false,
                true,
                Some(&disk.device_serial),
            );
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                portable_attachment.cleanup();
                return Err(error)
                    .wrap_err_with(|| format!("spawn Inrou PortableVm via {}", qemu.display()));
            }
        };
        let started_at = std::time::Instant::now();
        let startup_grace = self
            .config
            .inrou
            .start_grace
            .max(Duration::from_secs(u64::from(
                bundle.container.lifecycle.start_grace_secs.get(),
            )))
            .max(INROU_PORTABLE_START_GRACE_FLOOR);
        loop {
            if let Some(status) = child
                .try_wait()
                .wrap_err("poll Inrou PortableVm process during startup")?
            {
                let stderr = stderr_log_excerpt(&stderr_log_path);
                let console = stderr_log_excerpt(&console_log_path);
                portable_attachment.cleanup();
                eyre::bail!(
                    "Inrou PortableVm process exited during startup with status {status}{}{}",
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(": {stderr}")
                    },
                    if console.is_empty() {
                        String::new()
                    } else {
                        format!("\nserial console:\n{console}")
                    },
                );
            }
            match probe_hosted_http_health(
                &network_plan.listen_base_url,
                bundle.container.lifecycle.healthcheck_path.as_deref(),
            ) {
                Ok(()) => {
                    return Ok(HostedHttpWorker::new(
                        cache_key,
                        child,
                        network_plan.listen_base_url,
                        egress_accounting_offset_bytes,
                        Some(HostedHttpWorkerAttachment::PortableVm(portable_attachment)),
                        stderr_log_path,
                    ));
                }
                Err(error) if started_at.elapsed() < startup_grace => {
                    let _ = error;
                    thread::sleep(Duration::from_millis(250));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    portable_attachment.cleanup();
                    let stderr = stderr_log_excerpt(&stderr_log_path);
                    eyre::bail!(
                        "Inrou PortableVm failed healthcheck during startup: {}{}",
                        error,
                        if stderr.is_empty() {
                            String::new()
                        } else {
                            format!(": {stderr}")
                        }
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn start_inrou_worker_linux(
        &self,
        plan: &SoracloudRuntimeServicePlan,
        bundle: &SoraDeploymentBundleV1,
        cache_key: HostedHttpWorkerCacheKey,
        egress_accounting_offset_bytes: u64,
    ) -> eyre::Result<HostedHttpWorker> {
        let firewall_plan = inrou_tap_firewall_plan(&bundle.container.capabilities.network)?;
        let inrou = plan
            .inrou
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Inrou runtime requires a local runtime Inrou plan"))?;
        let materialization_dir = PathBuf::from(&plan.materialization_dir);
        fs::create_dir_all(&materialization_dir)
            .wrap_err_with(|| format!("create {}", materialization_dir.display()))?;
        for volume in &plan.lease_volumes {
            let volume_dir = PathBuf::from(&volume.local_materialization_dir);
            fs::create_dir_all(&volume_dir)
                .wrap_err_with(|| format!("create {}", volume_dir.display()))?;
        }

        let bundle_root = materialization_dir.join("inrou_bundle");
        ensure_native_bundle_extracted(
            &PathBuf::from(&plan.bundle_cache_path),
            &plan.bundle_hash,
            &bundle_root,
        )?;
        self.hydrate_published_inrou_guest_image_artifact(
            &bundle_root,
            bundle,
            inrou.selected_guest_isa,
        )?;
        ensure_inrou_entrypoint_present(&bundle_root, &bundle.container.entrypoint)?;
        let firecracker = resolve_executable_on_path("firecracker")
            .ok_or_else(|| eyre::eyre!("Inrou runtime requires `firecracker` on PATH"))?;
        let mke2fs = resolve_inrou_mke2fs_executable()
            .ok_or_else(|| eyre::eyre!("Inrou runtime requires `mke2fs` or `mkfs.ext4` on PATH"))?;
        let kernel_image_path =
            resolve_inrou_bundle_member_path(&bundle_root, &inrou.kernel_image_path)?;
        let base_rootfs_image_path =
            resolve_inrou_bundle_member_path(&bundle_root, &inrou.rootfs_image_path)?;
        let initrd_image_path = inrou
            .initrd_image_path
            .as_ref()
            .map(|path| resolve_inrou_bundle_member_path(&bundle_root, path))
            .transpose()?;
        let bootstrap_user_data = match inrou.bootstrap_user_data_path.as_ref() {
            Some(path) => Some(
                fs::read_to_string(resolve_inrou_bundle_member_path(&bundle_root, path)?)
                    .wrap_err("read Inrou bootstrap user-data overlay")?,
            ),
            None => None,
        };
        let guest_port = bundle
            .service
            .route
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Inrou service requires a route"))?
            .service_port
            .get();
        let stdout_log_path = materialization_dir.join("inrou.stdout.log");
        let stderr_log_path = materialization_dir.join("inrou.stderr.log");
        let stdout = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&stdout_log_path)
            .wrap_err_with(|| format!("open {}", stdout_log_path.display()))?;
        let stderr = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&stderr_log_path)
            .wrap_err_with(|| format!("open {}", stderr_log_path.display()))?;
        let mut network_attachment =
            setup_inrou_tap_network(&cache_key, &materialization_dir, firewall_plan)
                .wrap_err("setup Inrou tap network")?;
        let startup_paths = (|| -> eyre::Result<(PathBuf, PathBuf)> {
            let root_disk_path = match plan
                .lease_volumes
                .iter()
                .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
            {
                Some(root_volume) => ensure_inrou_root_disk(&base_rootfs_image_path, root_volume)
                    .wrap_err("prepare Inrou mutable root disk")?,
                None => eyre::bail!("Inrou runtime requires one PersistentRootLeaseVolume"),
            };
            let leasefs_exports =
                ensure_inrou_leasefs_exports(plan).wrap_err("prepare LeaseFs exports")?;
            let shared_filesystem_mounts =
                ensure_inrou_shared_filesystem_mounts(&leasefs_exports, &mut network_attachment)
                    .wrap_err("prepare Inrou shared lease exports")?;
            let seed_image_path = build_inrou_bootstrap_seed(
                &mke2fs,
                &materialization_dir,
                plan,
                &cache_key,
                guest_port,
                &network_attachment,
                &shared_filesystem_mounts,
                bootstrap_user_data.as_deref(),
            )
            .wrap_err("build Inrou bootstrap seed image")?;
            let firecracker_config_path = write_inrou_firecracker_config(
                &materialization_dir,
                &kernel_image_path,
                initrd_image_path.as_deref(),
                &root_disk_path,
                &seed_image_path,
                &network_attachment,
                bundle.container.resources,
            )
            .wrap_err("write Inrou Firecracker config")?;
            let api_socket_path = materialization_dir.join("firecracker.sock");
            if api_socket_path.exists() {
                fs::remove_file(&api_socket_path)
                    .wrap_err_with(|| format!("remove stale {}", api_socket_path.display()))?;
            }
            Ok((firecracker_config_path, api_socket_path))
        })();
        let (firecracker_config_path, api_socket_path) = match startup_paths {
            Ok(paths) => paths,
            Err(error) => {
                network_attachment.cleanup();
                return Err(error);
            }
        };

        let mut command = Command::new(&firecracker);
        command
            .arg("--api-sock")
            .arg(&api_socket_path)
            .arg("--config-file")
            .arg(&firecracker_config_path)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let mut child = command
            .spawn()
            .wrap_err_with(|| format!("spawn Inrou microVM via {}", firecracker.display()))?;
        let listen_base_url = format!("http://{}:{guest_port}", network_attachment.guest_ip);
        let started_at = std::time::Instant::now();
        loop {
            if let Some(status) = child
                .try_wait()
                .wrap_err("poll Inrou Firecracker process during startup")?
            {
                let stderr = stderr_log_excerpt(&stderr_log_path);
                network_attachment.cleanup();
                eyre::bail!(
                    "Inrou Firecracker process exited during startup with status {status}{}",
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(": {stderr}")
                    }
                );
            }
            match probe_hosted_http_health(
                &listen_base_url,
                bundle.container.lifecycle.healthcheck_path.as_deref(),
            ) {
                Ok(()) => {
                    return Ok(HostedHttpWorker::new(
                        cache_key,
                        child,
                        listen_base_url,
                        egress_accounting_offset_bytes,
                        Some(HostedHttpWorkerAttachment::FirecrackerKvm(
                            network_attachment,
                        )),
                        stderr_log_path,
                    ));
                }
                Err(error)
                    if started_at.elapsed()
                        < self
                            .config
                            .inrou
                            .start_grace
                            .max(Duration::from_secs(u64::from(
                                bundle.container.lifecycle.start_grace_secs.get(),
                            ))) =>
                {
                    let _ = error;
                    thread::sleep(Duration::from_millis(250));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    network_attachment.cleanup();
                    let stderr = stderr_log_excerpt(&stderr_log_path);
                    eyre::bail!(
                        "Inrou microVM failed healthcheck during startup: {}{}",
                        error,
                        if stderr.is_empty() {
                            String::new()
                        } else {
                            format!(": {stderr}")
                        }
                    );
                }
            }
        }
    }

    fn services_root(&self) -> PathBuf {
        self.config.state_dir.join("services")
    }

    fn apartments_root(&self) -> PathBuf {
        self.config.state_dir.join("apartments")
    }

    fn hosted_http_concurrency_limit(&self) -> usize {
        if !self.config.inrou.enabled || self.config.inrou.proxy_only {
            0
        } else {
            self.config.inrou.max_concurrent_vms.get()
        }
    }

    fn artifacts_root(&self) -> PathBuf {
        self.config.state_dir.join("artifacts")
    }

    fn journals_root(&self) -> PathBuf {
        self.config.state_dir.join("journals")
    }

    fn checkpoints_root(&self) -> PathBuf {
        self.config.state_dir.join("checkpoints")
    }

    fn secrets_root(&self) -> PathBuf {
        self.config.state_dir.join("secrets")
    }

    fn credentials_root(&self) -> PathBuf {
        self.config.state_dir.join("credentials")
    }

    fn hf_sources_root(&self) -> PathBuf {
        self.config.state_dir.join("hf_sources")
    }

    fn service_data_root(&self) -> PathBuf {
        self.config.state_dir.join("service_data")
    }

    fn hf_source_root(&self, source_id: &str) -> PathBuf {
        self.hf_sources_root()
            .join(sanitize_path_component(source_id))
    }

    fn write_service_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
        view: &StateView<'_>,
    ) -> eyre::Result<()> {
        for (service_name, versions) in &snapshot.services {
            let service_dir_name = sanitize_path_component(service_name);
            let service_root = self.services_root().join(service_dir_name);
            fs::create_dir_all(&service_root)
                .wrap_err_with(|| format!("create {}", service_root.display()))?;
            for (service_version, plan) in versions {
                let version_dir = service_root.join(sanitize_path_component(service_version));
                fs::create_dir_all(&version_dir)
                    .wrap_err_with(|| format!("create {}", version_dir.display()))?;
                write_json_atomic(&version_dir.join("runtime_plan.json"), plan)?;
                for replica_slot in &plan.local_replica_slots {
                    let replica_plan = project_hosted_http_replica_plan(plan, *replica_slot);
                    let replica_dir = PathBuf::from(&replica_plan.materialization_dir);
                    fs::create_dir_all(&replica_dir)
                        .wrap_err_with(|| format!("create {}", replica_dir.display()))?;
                    write_json_atomic(&replica_dir.join("runtime_plan.json"), &replica_plan)?;
                }
                let bundle = bundle_registry
                    .get(&(service_name.clone(), service_version.clone()))
                    .ok_or_else(|| {
                        eyre::eyre!(
                            "runtime snapshot references missing admitted bundle for service `{service_name}` revision `{service_version}`"
                        )
                    })?;
                write_json_atomic(&version_dir.join("deployment_bundle.json"), bundle)?;
                let deployment = view
                    .world()
                    .soracloud_service_deployments()
                    .get(&bundle.service.service_name)
                    .ok_or_else(|| {
                        eyre::eyre!(
                            "runtime snapshot references missing deployment state for service `{}`",
                            bundle.service.service_name
                        )
                    })?;
                write_service_config_materializations(
                    &version_dir,
                    &PathBuf::from(&plan.config_materialization_dir),
                    &PathBuf::from(&plan.config_exports_materialization_dir),
                    &PathBuf::from(&plan.effective_env_materialization_path),
                    plan,
                    deployment,
                )?;
                write_service_secret_materializations(
                    &version_dir,
                    &PathBuf::from(&plan.secret_envelopes_materialization_dir),
                    &PathBuf::from(&plan.secret_payload_materialization_dir),
                    deployment,
                )?;
            }
        }
        Ok(())
    }

    fn write_apartment_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        view: &StateView<'_>,
    ) -> eyre::Result<()> {
        for (apartment_name, plan) in &snapshot.apartments {
            let apartment_root = self
                .apartments_root()
                .join(sanitize_path_component(apartment_name));
            fs::create_dir_all(&apartment_root)
                .wrap_err_with(|| format!("create {}", apartment_root.display()))?;
            write_json_atomic(&apartment_root.join("runtime_plan.json"), plan)?;
            if let Some(record) = view
                .world()
                .soracloud_agent_apartments()
                .get(apartment_name)
            {
                write_json_atomic(
                    &apartment_root.join("apartment_manifest.json"),
                    &record.manifest,
                )?;
            }
        }
        Ok(())
    }

    fn prune_stale_service_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired: BTreeMap<String, BTreeSet<String>> = snapshot
            .services
            .iter()
            .map(|(service_name, versions)| {
                let desired_versions = versions
                    .keys()
                    .map(|version| sanitize_path_component(version))
                    .collect();
                (sanitize_path_component(service_name), desired_versions)
            })
            .collect();

        prune_nested_directory_tree(self.services_root().as_path(), &desired)?;
        Ok(())
    }

    fn prune_stale_secret_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired: BTreeMap<String, BTreeSet<String>> = snapshot
            .services
            .iter()
            .map(|(service_name, versions)| {
                let desired_versions = versions
                    .keys()
                    .map(|version| sanitize_path_component(version))
                    .collect();
                (sanitize_path_component(service_name), desired_versions)
            })
            .collect();

        prune_nested_directory_tree(self.secrets_root().as_path(), &desired)?;
        Ok(())
    }

    fn prune_stale_apartment_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired: BTreeSet<String> = snapshot
            .apartments
            .keys()
            .map(|name| sanitize_path_component(name))
            .collect();
        prune_flat_directory_tree(self.apartments_root().as_path(), &desired)?;
        Ok(())
    }

    fn import_hf_sources(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired_sources = snapshot
            .hf_sources
            .iter()
            .filter(|(_source_id, plan)| {
                plan.active_pool_count > 0
                    || plan.bound_service_count > 0
                    || plan.bound_apartment_count > 0
            })
            .filter(|(source_id, _plan)| {
                self.local_hf_source_assignment_allowed(view, source_id.as_str())
            })
            .map(|(source_id, _plan)| sanitize_path_component(source_id))
            .collect::<BTreeSet<_>>();
        prune_flat_directory_tree(self.hf_sources_root().as_path(), &desired_sources)?;
        if desired_sources.is_empty() {
            return Ok(());
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(self.config.hf.request_timeout)
            .build()
            .wrap_err("build Hugging Face importer HTTP client")?;

        for (source_hash, source) in view.world().soracloud_hf_sources().iter() {
            let source_id = source_hash.to_string();
            if !snapshot.hf_sources.contains_key(&source_id)
                || !desired_sources.contains(&sanitize_path_component(&source_id))
                || matches!(
                    source.status,
                    SoraHfSourceStatusV1::Failed | SoraHfSourceStatusV1::Retired
                )
            {
                continue;
            }

            if let Err(error) = self.import_one_hf_source(&client, &source_id, source) {
                iroha_logger::warn!(
                    ?error,
                    source_id = %source_id,
                    repo_id = %source.repo_id,
                    revision = %source.resolved_revision,
                    "Soracloud HF source import failed"
                );
                self.write_hf_import_error_manifest(
                    &source_id,
                    source,
                    error.to_string().as_str(),
                )?;
                self.report_local_hf_warmup_failure(view, &source_id, error.to_string());
            }
        }

        Ok(())
    }

    fn local_hf_source_assignment_allowed(&self, view: &StateView<'_>, source_id: &str) -> bool {
        if !hf_local_host_identity_is_configured(&self.config) {
            return true;
        }
        !local_hf_source_execution_hosts(view, source_id, &self.config).is_empty()
    }

    fn report_local_hf_warmup_failure(
        &self,
        view: &StateView<'_>,
        source_id: &str,
        error_message: String,
    ) {
        for host in local_hf_source_execution_hosts(view, source_id, &self.config) {
            if host.status != SoraHfPlacementHostStatusV1::Warming {
                continue;
            }
            self.host_violation_reporter.report(
                view,
                &host.validator_account_id,
                SoraModelHostViolationKindV1::WarmupNoShow,
                Some(host.placement_id),
                Some(format!(
                    "local HF warmup for source `{source_id}` failed before readiness: {error_message}"
                )),
            );
        }
    }

    fn probe_local_hf_execution_hosts(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
    ) {
        let mut submitted_local_heartbeat = false;
        for (source_hash, source) in view.world().soracloud_hf_sources().iter() {
            let source_id = source_hash.to_string();
            if !snapshot.hf_sources.contains_key(&source_id) {
                continue;
            }
            let hosts = local_hf_source_execution_hosts(view, &source_id, &self.config);
            if hosts.is_empty() {
                continue;
            }
            if let Err(error) = probe_hf_local_runner_for_source(
                &self.config.state_dir,
                &self.config.hf,
                &self.hf_local_workers,
                &source_id,
                source,
            ) {
                iroha_logger::warn!(
                    ?error,
                    source_id = %source_id,
                    repo_id = %source.repo_id,
                    revision = %source.resolved_revision,
                    "Soracloud HF local worker probe failed"
                );
                self.report_local_hf_execution_probe_failure(
                    view,
                    &source_id,
                    &hosts,
                    &error.message,
                );
            } else if !submitted_local_heartbeat
                && self.refresh_local_model_host_warmth_if_needed(view, &hosts)
            {
                submitted_local_heartbeat = true;
            }
        }
    }

    fn refresh_local_model_host_warmth_if_needed(
        &self,
        view: &StateView<'_>,
        hosts: &[ResolvedHfPlacementExecutionHost],
    ) -> bool {
        let Some(mutation_sink) = self.mutation_sink.as_ref() else {
            return false;
        };
        let Some(validator_account_id) = self.config.local_validator_account_id.as_ref() else {
            return false;
        };
        let Some(capability) = view
            .world()
            .soracloud_model_host_capabilities()
            .get(validator_account_id)
        else {
            return false;
        };
        let now_ms = soracloud_runtime_observed_at_ms();
        if !capability.is_active_at(now_ms) {
            return false;
        }
        let desired_expiry_ms = desired_model_host_heartbeat_expiry_ms(now_ms, &self.config);
        let needs_status_promotion = hosts
            .iter()
            .any(|host| host.status == SoraHfPlacementHostStatusV1::Warming);
        let needs_expiry_refresh = capability.heartbeat_expires_at_ms < desired_expiry_ms;
        if !(needs_status_promotion || needs_expiry_refresh)
            || !self.local_model_host_heartbeat_attempt_allowed(now_ms)
        {
            return false;
        }
        if let Err(error) =
            mutation_sink.submit_model_host_heartbeat(validator_account_id, desired_expiry_ms)
        {
            iroha_logger::warn!(
                ?error,
                validator_account_id = %validator_account_id,
                heartbeat_expires_at_ms = desired_expiry_ms,
                "failed to submit Soracloud model-host heartbeat from runtime health"
            );
            return false;
        }
        true
    }

    fn local_model_host_heartbeat_attempt_allowed(&self, now_ms: u64) -> bool {
        let cooldown_ms =
            u64::try_from(self.config.reconcile_interval.as_millis()).unwrap_or(u64::MAX);
        let mut last_attempt_ms = self.last_model_host_heartbeat_attempt_ms.lock();
        if let Some(previous_attempt_ms) = *last_attempt_ms
            && now_ms.saturating_sub(previous_attempt_ms) < cooldown_ms
        {
            return false;
        }
        *last_attempt_ms = Some(now_ms);
        true
    }

    fn report_local_hf_execution_probe_failure(
        &self,
        view: &StateView<'_>,
        source_id: &str,
        hosts: &[ResolvedHfPlacementExecutionHost],
        error_message: &str,
    ) {
        for host in hosts {
            let host_role = match host.role {
                SoraHfPlacementHostRoleV1::Primary => "primary",
                SoraHfPlacementHostRoleV1::Replica => "replica",
            };
            let (kind, failure_class) = match host.status {
                SoraHfPlacementHostStatusV1::Warming => {
                    (SoraModelHostViolationKindV1::WarmupNoShow, "warming")
                }
                SoraHfPlacementHostStatusV1::Warm => {
                    (SoraModelHostViolationKindV1::AssignedHeartbeatMiss, "warm")
                }
                _ => continue,
            };
            self.host_violation_reporter.report(
                view,
                &host.validator_account_id,
                kind,
                Some(host.placement_id),
                Some(format!(
                    "local HF runtime health probe for source `{source_id}` failed on the assigned {failure_class} {host_role} host: {error_message}"
                )),
            );
        }
    }

    fn import_one_hf_source(
        &self,
        client: &reqwest::blocking::Client,
        source_id: &str,
        source: &iroha_data_model::soracloud::SoraHfSourceRecordV1,
    ) -> eyre::Result<()> {
        let source_root = self.hf_source_root(source_id);
        let manifest_path = source_root.join("import_manifest.json");
        if let Some(existing) = read_json_optional::<HfLocalImportManifestV1>(&manifest_path)
            .wrap_err_with(|| format!("read {}", manifest_path.display()))?
            && existing.source_id == source_id
            && existing.repo_id == source.repo_id
            && existing.requested_revision == source.resolved_revision
            && existing.model_name == source.model_name
            && existing.adapter_id == source.adapter_id
            && existing.import_error.is_none()
        {
            return Ok(());
        }

        fs::create_dir_all(source_root.join("files"))
            .wrap_err_with(|| format!("create {}", source_root.join("files").display()))?;

        let info_url = hf_model_info_url(
            &self.config.hf.api_base_url,
            &source.repo_id,
            &source.resolved_revision,
        )?;
        let response = client
            .get(info_url.clone())
            .send()
            .wrap_err_with(|| format!("fetch Hugging Face model info from {info_url}"))?;
        if !response.status().is_success() {
            eyre::bail!(
                "HF model info request for `{}` revision `{}` returned {}",
                source.repo_id,
                source.resolved_revision,
                response.status()
            );
        }
        let model_info_bytes = response
            .bytes()
            .wrap_err_with(|| format!("read Hugging Face model info response from {info_url}"))?
            .to_vec();
        let model_info: norito::json::Value =
            norito::json::from_slice(&model_info_bytes).wrap_err("decode HF model info JSON")?;
        let raw_model_info_path = source_root.join("model_info.json");
        write_bytes_atomic(&raw_model_info_path, &model_info_bytes)
            .wrap_err_with(|| format!("write {}", raw_model_info_path.display()))?;

        let resolved_commit = model_info
            .get("sha")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned);
        let pipeline_tag = model_info
            .get("pipeline_tag")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned);
        let library_name = model_info
            .get("library_name")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned);
        let tags = model_info
            .get("tags")
            .and_then(norito::json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(norito::json::Value::as_str)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        let mut imported_files = Vec::new();
        let mut skipped_files = Vec::new();
        let mut imported_total_bytes = 0_u64;
        let mut sibling_paths = model_info
            .get("siblings")
            .and_then(norito::json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("rfilename").and_then(norito::json::Value::as_str))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        sibling_paths.sort();
        sibling_paths.dedup();

        for path in sibling_paths {
            if !hf_import_file_selected(&path, &self.config.hf.import_file_allowlist) {
                continue;
            }
            if imported_files.len()
                >= usize::try_from(self.config.hf.import_max_files).unwrap_or(usize::MAX)
            {
                skipped_files.push(format!("{path} (skipped: file limit reached)"));
                continue;
            }

            let file_url = hf_repo_file_url(
                &self.config.hf.hub_base_url,
                &source.repo_id,
                &source.resolved_revision,
                &path,
            )?;
            let head = client
                .head(file_url.clone())
                .send()
                .wrap_err_with(|| format!("query HF file headers from {file_url}"))?;
            if !head.status().is_success() {
                skipped_files.push(format!("{path} (skipped: HEAD returned {})", head.status()));
                continue;
            }
            let Some(content_length) = head
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
            else {
                skipped_files.push(format!("{path} (skipped: missing Content-Length)"));
                continue;
            };
            if content_length > self.config.hf.import_max_file_bytes {
                skipped_files.push(format!(
                    "{path} (skipped: {content_length} bytes exceeds per-file cap {})",
                    self.config.hf.import_max_file_bytes
                ));
                continue;
            }
            let next_total = imported_total_bytes.saturating_add(content_length);
            if next_total > self.config.hf.import_max_total_bytes {
                skipped_files.push(format!(
                    "{path} (skipped: aggregate import cap {} bytes reached)",
                    self.config.hf.import_max_total_bytes
                ));
                continue;
            }

            let body = client
                .get(file_url.clone())
                .send()
                .wrap_err_with(|| format!("download HF file from {file_url}"))?
                .bytes()
                .wrap_err_with(|| format!("read HF file response from {file_url}"))?
                .to_vec();
            let actual_len = u64::try_from(body.len()).unwrap_or(u64::MAX);
            if actual_len != content_length {
                skipped_files.push(format!(
                    "{path} (skipped: body length {actual_len} bytes did not match HEAD length {content_length})"
                ));
                continue;
            }

            let relative_path = sanitized_relative_material_path(&path)
                .map_err(|error| eyre::eyre!("sanitize HF repo path `{path}`: {error}"))?;
            let local_path = source_root.join("files").join(&relative_path);
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .wrap_err_with(|| format!("create {}", parent.display()))?;
            }
            write_bytes_atomic(&local_path, &body)
                .wrap_err_with(|| format!("write {}", local_path.display()))?;
            imported_total_bytes = next_total;
            imported_files.push(HfImportedFileV1 {
                path,
                content_length: actual_len,
                payload_hash: Hash::new(&body).to_string(),
                local_path: local_path.display().to_string(),
            });
        }

        let manifest = HfLocalImportManifestV1 {
            schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
            source_id: source_id.to_owned(),
            repo_id: source.repo_id.clone(),
            requested_revision: source.resolved_revision.clone(),
            resolved_commit,
            model_name: source.model_name.clone(),
            adapter_id: source.adapter_id.clone(),
            pipeline_tag,
            library_name,
            tags,
            imported_at_ms: source.updated_at_ms,
            imported_files,
            skipped_files,
            raw_model_info_path: Some(raw_model_info_path.display().to_string()),
            import_error: None,
        };
        write_json_atomic(&manifest_path, &manifest)
            .wrap_err_with(|| format!("write {}", manifest_path.display()))?;
        Ok(())
    }

    fn write_hf_import_error_manifest(
        &self,
        source_id: &str,
        source: &iroha_data_model::soracloud::SoraHfSourceRecordV1,
        error: &str,
    ) -> eyre::Result<()> {
        let source_root = self.hf_source_root(source_id);
        fs::create_dir_all(&source_root)
            .wrap_err_with(|| format!("create {}", source_root.display()))?;
        let manifest = HfLocalImportManifestV1 {
            schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
            source_id: source_id.to_owned(),
            repo_id: source.repo_id.clone(),
            requested_revision: source.resolved_revision.clone(),
            resolved_commit: None,
            model_name: source.model_name.clone(),
            adapter_id: source.adapter_id.clone(),
            pipeline_tag: None,
            library_name: None,
            tags: Vec::new(),
            imported_at_ms: source.updated_at_ms,
            imported_files: Vec::new(),
            skipped_files: Vec::new(),
            raw_model_info_path: None,
            import_error: Some(error.to_owned()),
        };
        let manifest_path = source_root.join("import_manifest.json");
        write_json_atomic(&manifest_path, &manifest)
            .wrap_err_with(|| format!("write {}", manifest_path.display()))?;
        Ok(())
    }

    fn hydrate_missing_artifacts(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) -> eyre::Result<()> {
        let stored_manifests = if let Some(sorafs_node) = self.sorafs_node.as_ref() {
            if sorafs_node.is_enabled() {
                sorafs_node
                    .stored_manifests()
                    .wrap_err("list stored SoraFS manifests for Soracloud hydration")?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let remote_sources = collect_remote_hydration_sources(view, &self.state);
        let mut missing = BTreeMap::<String, (Hash, String)>::new();
        let mut hydrated_payloads = BTreeMap::<Hash, Option<Vec<u8>>>::new();
        for (service_name, versions) in &snapshot.services {
            for (service_version, plan) in versions {
                if let Some(bundle) =
                    bundle_registry.get(&(service_name.clone(), service_version.clone()))
                    && let Some(payload) =
                        soracloud_hf_generated_bundle_payload_if_applicable(bundle)
                {
                    hydrated_payloads
                        .entry(bundle.container.bundle_hash)
                        .or_insert(Some(payload));
                }
                for artifact in &plan.artifacts {
                    if artifact.available_locally {
                        continue;
                    }
                    let artifact_hash =
                        Hash::from_str(&artifact.artifact_hash).wrap_err_with(|| {
                            format!("parse Soracloud artifact hash `{}`", artifact.artifact_hash)
                        })?;
                    missing
                        .entry(artifact.local_cache_path.clone())
                        .or_insert((artifact_hash, artifact.artifact_path.clone()));
                }
            }
        }

        for (local_cache_path, (artifact_hash, artifact_path)) in missing {
            let cache_path = PathBuf::from(&local_cache_path);
            if cache_path.exists() {
                continue;
            }
            let payload = if let Some(cached) = hydrated_payloads.get(&artifact_hash) {
                cached.clone()
            } else {
                let resolved = self.read_committed_sorafs_payload(
                    view,
                    &stored_manifests,
                    &remote_sources,
                    artifact_hash,
                )?;
                hydrated_payloads.insert(artifact_hash, resolved.clone());
                resolved
            };
            let Some(payload) = payload else {
                continue;
            };
            write_bytes_atomic(&cache_path, &payload).wrap_err_with(|| {
                format!(
                    "persist hydrated Soracloud artifact `{artifact_path}` at {}",
                    cache_path.display()
                )
            })?;
        }

        Ok(())
    }

    fn read_committed_sorafs_payload(
        &self,
        _view: &StateView<'_>,
        stored_manifests: &[StoredManifest],
        remote_sources: &[RemoteHydrationSource],
        expected_hash: Hash,
    ) -> eyre::Result<Option<Vec<u8>>> {
        if let Some(sorafs_node) = self.sorafs_node.as_ref() {
            for manifest in stored_manifests {
                let Ok(content_length) = usize::try_from(manifest.content_length()) else {
                    iroha_logger::warn!(
                        manifest_id = %manifest.manifest_id(),
                        content_length = manifest.content_length(),
                        "skipping Soracloud hydration candidate with oversized SoraFS payload"
                    );
                    continue;
                };
                let payload =
                    match sorafs_node.read_payload_range(manifest.manifest_id(), 0, content_length)
                    {
                        Ok(payload) => payload,
                        Err(error) => {
                            iroha_logger::warn!(
                                ?error,
                                manifest_id = %manifest.manifest_id(),
                                "failed to read committed SoraFS payload during Soracloud hydration"
                            );
                            continue;
                        }
                    };
                if Hash::new(&payload) == expected_hash {
                    return Ok(Some(payload));
                }
            }
        }
        if let Some(payload) =
            self.read_committed_remote_sorafs_payload(remote_sources, expected_hash)?
        {
            return Ok(Some(payload));
        }
        Ok(None)
    }

    fn read_committed_remote_sorafs_payload(
        &self,
        remote_sources: &[RemoteHydrationSource],
        expected_hash: Hash,
    ) -> eyre::Result<Option<Vec<u8>>> {
        let Some(_cache) = self.sorafs_provider_cache.as_ref() else {
            return Ok(None);
        };
        if remote_sources.is_empty() {
            return Ok(None);
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("build Soracloud remote hydration HTTP client")?;

        for source in remote_sources {
            for provider_id in &source.provider_ids {
                let Some(base_url) = self.remote_provider_base_url(provider_id) else {
                    continue;
                };
                let Some(manifest) =
                    self.fetch_remote_manifest_metadata(&client, &base_url, source)
                else {
                    continue;
                };
                if !manifest
                    .manifest_digest_hex
                    .eq_ignore_ascii_case(&source.manifest_digest_hex)
                {
                    continue;
                }
                if let Some(expected_chunker) = source.chunker_handle.as_ref()
                    && !manifest
                        .chunk_profile_handle
                        .eq_ignore_ascii_case(expected_chunker)
                {
                    continue;
                }

                let Some(plan) =
                    self.fetch_remote_hydration_plan(&client, &base_url, &source.manifest_cid_hex)
                else {
                    continue;
                };
                if !plan
                    .chunker_handle
                    .eq_ignore_ascii_case(&manifest.chunk_profile_handle)
                {
                    continue;
                }

                let client_id = "soracloud-runtime-hydration";
                let nonce =
                    remote_hydration_nonce(&source.manifest_cid_hex, provider_id, expected_hash);
                let Some(stream_token) = self.fetch_remote_stream_token(
                    &client,
                    &base_url,
                    &source.manifest_cid_hex,
                    provider_id,
                    &plan,
                    client_id,
                    &nonce,
                ) else {
                    continue;
                };

                let Ok(capacity) = usize::try_from(plan.content_length) else {
                    iroha_logger::warn!(
                        manifest_digest = %source.manifest_digest_hex,
                        manifest_cid = %source.manifest_cid_hex,
                        provider_id_hex = %hex::encode(provider_id),
                        content_length = plan.content_length,
                        "skipping remote Soracloud hydration candidate with oversized payload"
                    );
                    continue;
                };

                let mut payload = Vec::with_capacity(capacity);
                let mut cursor = 0_u64;
                let mut fetch_failed = false;
                for chunk in &plan.chunks {
                    if chunk.offset != cursor {
                        fetch_failed = true;
                        break;
                    }
                    let Some(bytes) = self.fetch_remote_chunk(
                        &client,
                        &base_url,
                        &plan,
                        chunk,
                        &stream_token,
                        client_id,
                        &nonce,
                    ) else {
                        fetch_failed = true;
                        break;
                    };
                    if bytes.len() != usize::try_from(chunk.length).unwrap_or(usize::MAX) {
                        fetch_failed = true;
                        break;
                    }
                    cursor = cursor.saturating_add(bytes.len() as u64);
                    payload.extend_from_slice(&bytes);
                }
                if fetch_failed || cursor != plan.content_length {
                    continue;
                }
                if Hash::new(&payload) == expected_hash {
                    return Ok(Some(payload));
                }
            }
        }

        Ok(None)
    }

    fn read_committed_sorafs_directory_payload_by_digest(
        &self,
        view: &StateView<'_>,
        remote_sources: &[RemoteHydrationSource],
        manifest_digest: [u8; 32],
    ) -> eyre::Result<Option<(Vec<u8>, Vec<SorafsHydratedFileLayout>)>> {
        if let Some(sorafs_node) = self.sorafs_node.as_ref()
            && sorafs_node.is_enabled()
            && let Ok(manifest) = sorafs_node.manifest_metadata_by_digest(&manifest_digest)
        {
            let content_length =
                usize::try_from(manifest.content_length()).wrap_err_with(|| {
                    format!(
                        "convert committed SoraFS manifest {} content length to usize",
                        hex::encode(manifest_digest)
                    )
                })?;
            let payload = sorafs_node
                .read_payload_range(manifest.manifest_id(), 0, content_length)
                .wrap_err_with(|| {
                    format!(
                        "read committed SoraFS payload for manifest {}",
                        hex::encode(manifest_digest)
                    )
                })?;
            let files = manifest
                .files()
                .iter()
                .map(|file| SorafsHydratedFileLayout {
                    path: file.path.clone(),
                    offset: file.offset,
                    size: file.size,
                })
                .collect();
            return Ok(Some((payload, files)));
        }

        if !manifest_is_committed(view, &self.state, &manifest_digest) {
            return Ok(None);
        }

        self.read_committed_remote_sorafs_directory_payload(
            remote_sources,
            &hex::encode(manifest_digest),
        )
    }

    fn read_committed_remote_sorafs_directory_payload(
        &self,
        remote_sources: &[RemoteHydrationSource],
        manifest_digest_hex: &str,
    ) -> eyre::Result<Option<(Vec<u8>, Vec<SorafsHydratedFileLayout>)>> {
        let Some(_cache) = self.sorafs_provider_cache.as_ref() else {
            return Ok(None);
        };
        if remote_sources.is_empty() {
            return Ok(None);
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("build Soracloud remote directory hydration HTTP client")?;

        for source in remote_sources {
            if !source
                .manifest_digest_hex
                .eq_ignore_ascii_case(manifest_digest_hex)
            {
                continue;
            }
            for provider_id in &source.provider_ids {
                let Some(base_url) = self.remote_provider_base_url(provider_id) else {
                    continue;
                };
                let Some(manifest) =
                    self.fetch_remote_manifest_metadata(&client, &base_url, source)
                else {
                    continue;
                };
                if !manifest
                    .manifest_digest_hex
                    .eq_ignore_ascii_case(&source.manifest_digest_hex)
                {
                    continue;
                }
                if let Some(expected_chunker) = source.chunker_handle.as_ref()
                    && !manifest
                        .chunk_profile_handle
                        .eq_ignore_ascii_case(expected_chunker)
                {
                    continue;
                }

                let Some(plan) =
                    self.fetch_remote_hydration_plan(&client, &base_url, &source.manifest_cid_hex)
                else {
                    continue;
                };
                if !plan
                    .chunker_handle
                    .eq_ignore_ascii_case(&manifest.chunk_profile_handle)
                {
                    continue;
                }

                let client_id = "soracloud-runtime-directory-hydration";
                let nonce = remote_hydration_nonce(
                    &source.manifest_cid_hex,
                    provider_id,
                    Hash::new(manifest.manifest_digest_hex.as_bytes()),
                );
                let Some(stream_token) = self.fetch_remote_stream_token(
                    &client,
                    &base_url,
                    &source.manifest_cid_hex,
                    provider_id,
                    &plan,
                    client_id,
                    &nonce,
                ) else {
                    continue;
                };

                let Ok(capacity) = usize::try_from(plan.content_length) else {
                    continue;
                };
                let mut payload = Vec::with_capacity(capacity);
                let mut cursor = 0_u64;
                let mut fetch_failed = false;
                for chunk in &plan.chunks {
                    if chunk.offset != cursor {
                        fetch_failed = true;
                        break;
                    }
                    let Some(bytes) = self.fetch_remote_chunk(
                        &client,
                        &base_url,
                        &plan,
                        chunk,
                        &stream_token,
                        client_id,
                        &nonce,
                    ) else {
                        fetch_failed = true;
                        break;
                    };
                    if bytes.len() != usize::try_from(chunk.length).unwrap_or(usize::MAX) {
                        fetch_failed = true;
                        break;
                    }
                    cursor = cursor.saturating_add(bytes.len() as u64);
                    payload.extend_from_slice(&bytes);
                }
                if fetch_failed || cursor != plan.content_length {
                    continue;
                }
                let payload_digest_hex = hex::encode(blake3::hash(&payload).as_bytes());
                if !payload_digest_hex.eq_ignore_ascii_case(&manifest.payload_digest_hex) {
                    continue;
                }
                let files = manifest
                    .files
                    .iter()
                    .map(storage_file_dto_layout)
                    .collect::<eyre::Result<Vec<_>>>()?;
                return Ok(Some((payload, files)));
            }
        }

        Ok(None)
    }

    fn hydrate_published_inrou_guest_image_artifact(
        &self,
        bundle_root: &Path,
        bundle: &SoraDeploymentBundleV1,
        selected_guest_isa: SoraInrouGuestIsaV1,
    ) -> eyre::Result<()> {
        let Some(inrou) = bundle.container.inrou.as_ref() else {
            return Ok(());
        };
        let Some(image) = inrou.guest_images.get(&selected_guest_isa) else {
            return Ok(());
        };
        let Some(artifact) = image.published_artifact.as_ref() else {
            return Ok(());
        };

        let required_paths = [
            Some(image.kernel_image_path.as_str()),
            Some(image.rootfs_image_path.as_str()),
            image.initrd_image_path.as_deref(),
        ];
        if required_paths
            .iter()
            .flatten()
            .all(|path| bundle_root.join(strip_leading_slashes(path)).is_file())
        {
            return Ok(());
        }

        let manifest_digest = parse_sorafs_manifest_digest_hex(&artifact.manifest_digest_hex)?;
        let view = self.state.view();
        let remote_sources = collect_remote_hydration_sources(&view, &self.state);
        let hydrated = self.read_committed_sorafs_directory_payload_by_digest(
            &view,
            &remote_sources,
            manifest_digest,
        )?;
        drop(view);

        if let Some((payload, files)) = hydrated {
            let inrou_root = bundle_root.join("inrou");
            materialize_sorafs_payload_files(&payload, &files, &inrou_root).wrap_err_with(
                || {
                    format!(
                        "hydrate published Inrou guest-image artifact {} into {}",
                        artifact.manifest_digest_hex,
                        inrou_root.display()
                    )
                },
            )?;
            return Ok(());
        }

        if let Some(host_paths) = inrou_guest_image_host_paths_from_env(selected_guest_isa, image)?
        {
            iroha_logger::warn!(
                guest_isa = selected_guest_isa.as_str(),
                source = %host_paths.source,
                manifest_digest = %artifact.manifest_digest_hex,
                "published Inrou guest-image artifact is unavailable in SoraFS; hydrating from explicit host-local asset paths"
            );
            materialize_inrou_guest_image_from_host_paths(bundle_root, image, &host_paths)?;
            return Ok(());
        }

        Err(eyre::eyre!(
            "published Inrou guest-image artifact {} for {} is not available in local or remote SoraFS storage",
            artifact.manifest_digest_hex,
            selected_guest_isa.as_str()
        ))
    }

    fn remote_provider_base_url(&self, provider_id: &[u8; 32]) -> Option<reqwest::Url> {
        let cache = self.sorafs_provider_cache.as_ref()?;
        let guard = cache.try_read().ok()?;
        let record = guard.record_by_provider(provider_id)?;
        let advert = record.advert();
        let supports_torii_http_range =
            advert.body.transport_hints.as_ref().map_or(true, |hints| {
                hints
                    .iter()
                    .any(|hint| hint.protocol == TransportProtocol::ToriiHttpRange)
            });
        if !supports_torii_http_range {
            return None;
        }
        let endpoint = advert
            .body
            .endpoints
            .iter()
            .find(|endpoint| endpoint.kind == EndpointKind::Torii)?;
        normalize_provider_base_url(&endpoint.host_pattern)
    }

    fn fetch_remote_manifest_metadata(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        source: &RemoteHydrationSource,
    ) -> Option<StorageManifestResponseDto> {
        let url = match base_url.join(&format!(
            "v1/sorafs/storage/manifest/{}",
            source.manifest_cid_hex
        )) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_cid = %source.manifest_cid_hex,
                    "failed to build remote Soracloud hydration manifest URL"
                );
                return None;
            }
        };
        let response = match client.get(url.clone()).send() {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration manifest request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration manifest body"
                );
                return None;
            }
        };
        match norito::json::from_slice::<StorageManifestResponseDto>(&body) {
            Ok(dto) => Some(dto),
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to decode remote Soracloud hydration manifest response"
                );
                None
            }
        }
    }

    fn fetch_remote_hydration_plan(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        manifest_id_hex: &str,
    ) -> Option<RemoteHydrationPlan> {
        let url = match base_url.join(&format!("v1/sorafs/storage/plan/{manifest_id_hex}")) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_id = %manifest_id_hex,
                    "failed to build remote Soracloud hydration plan URL"
                );
                return None;
            }
        };
        let response = match client.get(url.clone()).send() {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration plan request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration plan body"
                );
                return None;
            }
        };
        parse_remote_hydration_plan(manifest_id_hex, &body)
            .inspect_err(|error| {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to decode remote Soracloud hydration plan response"
                );
            })
            .ok()
    }

    fn fetch_remote_stream_token(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        manifest_id_hex: &str,
        provider_id: &[u8; 32],
        plan: &RemoteHydrationPlan,
        client_id: &str,
        nonce: &str,
    ) -> Option<String> {
        let url = match base_url.join("v1/sorafs/storage/token") {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    "failed to build remote Soracloud hydration token URL"
                );
                return None;
            }
        };
        let max_chunk_len = plan
            .chunks
            .iter()
            .map(|chunk| u64::from(chunk.length))
            .max()
            .unwrap_or(0);
        let mut request_body = norito::json::native::Map::new();
        request_body.insert(
            "manifest_id_hex".into(),
            norito::json::Value::from(manifest_id_hex),
        );
        request_body.insert(
            "provider_id_hex".into(),
            norito::json::Value::from(hex::encode(provider_id)),
        );
        request_body.insert("ttl_secs".into(), norito::json::Value::from(60_u64));
        request_body.insert("max_streams".into(), norito::json::Value::from(1_u16));
        request_body.insert(
            "rate_limit_bytes".into(),
            norito::json::Value::from(max_chunk_len.max(1)),
        );
        request_body.insert(
            "requests_per_minute".into(),
            norito::json::Value::from(
                u32::try_from(plan.chunks.len().saturating_add(8)).unwrap_or(u32::MAX),
            ),
        );
        let request_body = norito::json::Value::Object(request_body);
        let request_body = match norito::json::to_vec(&request_body) {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to encode remote Soracloud hydration token request"
                );
                return None;
            }
        };
        let response = match client
            .post(url.clone())
            .header("X-SoraFS-Client", client_id)
            .header("X-SoraFS-Nonce", nonce)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request_body)
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration token request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration token body"
                );
                return None;
            }
        };
        let value: norito::json::Value = match norito::json::from_slice(&body) {
            Ok(value) => value,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to decode remote Soracloud hydration token response"
                );
                return None;
            }
        };
        value
            .get("token_base64")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn fetch_remote_chunk(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        plan: &RemoteHydrationPlan,
        chunk: &RemoteHydrationChunk,
        stream_token: &str,
        client_id: &str,
        nonce: &str,
    ) -> Option<Vec<u8>> {
        let url = match base_url.join(&format!(
            "v1/sorafs/storage/chunk/{}/{}",
            plan.manifest_id_hex, chunk.digest_hex
        )) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_id = %plan.manifest_id_hex,
                    chunk_digest = %chunk.digest_hex,
                    "failed to build remote Soracloud hydration chunk URL"
                );
                return None;
            }
        };
        let response = match client
            .get(url.clone())
            .header("X-SoraFS-Stream-Token", stream_token)
            .header("X-SoraFS-Chunker", &plan.chunker_handle)
            .header("X-SoraFS-Client", client_id)
            .header("X-SoraFS-Nonce", nonce)
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration chunk request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        match response.bytes() {
            Ok(bytes) => Some(bytes.to_vec()),
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration chunk body"
                );
                None
            }
        }
    }

    fn enforce_cache_budgets(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let artifact_observations = collect_artifact_cache_observations(view, snapshot);
        let artifact_candidates = collect_artifact_cache_candidates(
            self.artifacts_root().as_path(),
            &artifact_observations,
        )?;
        let journal_sequences = collect_runtime_receipt_artifact_sequences(view, |receipt| {
            receipt.journal_artifact_hash
        });
        let checkpoint_sequences = collect_runtime_receipt_artifact_sequences(view, |receipt| {
            receipt.checkpoint_artifact_hash
        });

        prune_cache_bucket(
            artifact_candidates.bundle,
            self.config.cache_budgets.bundle_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.static_asset,
            self.config.cache_budgets.static_asset_bytes.get(),
        )?;
        let mut journal_candidates = artifact_candidates.journal;
        journal_candidates.extend(collect_fixed_bucket_candidates(
            self.journals_root().as_path(),
            "journals",
            &journal_sequences,
        )?);
        prune_cache_bucket(
            journal_candidates,
            self.config.cache_budgets.journal_bytes.get(),
        )?;
        let mut checkpoint_candidates = artifact_candidates.checkpoint;
        checkpoint_candidates.extend(collect_fixed_bucket_candidates(
            self.checkpoints_root().as_path(),
            "checkpoints",
            &checkpoint_sequences,
        )?);
        prune_cache_bucket(
            checkpoint_candidates,
            self.config.cache_budgets.checkpoint_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.model_artifact,
            self.config.cache_budgets.model_artifact_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.model_weight,
            self.config.cache_budgets.model_weight_bytes.get(),
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheObservationMetadata {
    bucket: RuntimeCacheBucket,
    observation_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeCacheBucket {
    Bundle,
    StaticAsset,
    Journal,
    Checkpoint,
    ModelArtifact,
    ModelWeight,
}

impl RuntimeCacheBucket {
    const fn priority(self) -> u8 {
        match self {
            Self::Bundle => 5,
            Self::ModelWeight => 4,
            Self::ModelArtifact => 3,
            Self::Journal => 2,
            Self::Checkpoint => 2,
            Self::StaticAsset => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CachePruneCandidate {
    path: PathBuf,
    stable_key: String,
    bytes: u64,
    observation_sequence: u64,
}

#[derive(Default)]
struct ArtifactCacheCandidates {
    bundle: Vec<CachePruneCandidate>,
    static_asset: Vec<CachePruneCandidate>,
    journal: Vec<CachePruneCandidate>,
    checkpoint: Vec<CachePruneCandidate>,
    model_artifact: Vec<CachePruneCandidate>,
    model_weight: Vec<CachePruneCandidate>,
}

impl ArtifactCacheCandidates {
    fn bucket_mut(&mut self, bucket: RuntimeCacheBucket) -> &mut Vec<CachePruneCandidate> {
        match bucket {
            RuntimeCacheBucket::Bundle => &mut self.bundle,
            RuntimeCacheBucket::StaticAsset => &mut self.static_asset,
            RuntimeCacheBucket::Journal => &mut self.journal,
            RuntimeCacheBucket::Checkpoint => &mut self.checkpoint,
            RuntimeCacheBucket::ModelArtifact => &mut self.model_artifact,
            RuntimeCacheBucket::ModelWeight => &mut self.model_weight,
        }
    }
}

fn collect_artifact_cache_observations(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
) -> BTreeMap<String, CacheObservationMetadata> {
    let world = view.world();
    let mut observations = BTreeMap::new();

    for (service_name, deployment) in world.soracloud_service_deployments().iter() {
        let service_name = service_name.to_string();
        let Some(versions) = snapshot.services.get(&service_name) else {
            continue;
        };
        for (_service_version, plan) in versions {
            let observation_sequence = match plan.role {
                SoracloudRuntimeRevisionRole::Active => deployment.process_started_sequence,
                SoracloudRuntimeRevisionRole::CanaryCandidate => deployment
                    .active_rollout
                    .as_ref()
                    .map_or(deployment.process_started_sequence, |rollout| {
                        rollout.updated_sequence
                    }),
            };
            for artifact in &plan.artifacts {
                upsert_cache_observation(
                    &mut observations,
                    sanitize_path_component(&artifact.artifact_hash),
                    runtime_cache_bucket_for_kind(artifact.kind),
                    observation_sequence,
                );
            }
        }
    }

    for (_, record) in world.soracloud_model_weight_versions().iter() {
        upsert_cache_observation(
            &mut observations,
            hash_cache_name(record.weight_artifact_hash),
            RuntimeCacheBucket::ModelWeight,
            record
                .promoted_sequence
                .unwrap_or(record.registered_sequence),
        );
    }

    for (_, record) in world.soracloud_model_artifacts().iter() {
        upsert_cache_observation(
            &mut observations,
            hash_cache_name(record.weight_artifact_hash),
            RuntimeCacheBucket::ModelArtifact,
            record.registered_sequence,
        );
    }

    observations
}

fn runtime_cache_bucket_for_kind(kind: SoraArtifactKindV1) -> RuntimeCacheBucket {
    match kind {
        SoraArtifactKindV1::Bundle => RuntimeCacheBucket::Bundle,
        SoraArtifactKindV1::StaticAsset => RuntimeCacheBucket::StaticAsset,
        SoraArtifactKindV1::Journal => RuntimeCacheBucket::Journal,
        SoraArtifactKindV1::Checkpoint => RuntimeCacheBucket::Checkpoint,
        SoraArtifactKindV1::ModelArtifact => RuntimeCacheBucket::ModelArtifact,
        SoraArtifactKindV1::ModelWeights => RuntimeCacheBucket::ModelWeight,
    }
}

fn upsert_cache_observation(
    observations: &mut BTreeMap<String, CacheObservationMetadata>,
    key: String,
    bucket: RuntimeCacheBucket,
    observation_sequence: u64,
) {
    match observations.entry(key) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let existing = entry.get_mut();
            existing.observation_sequence = existing.observation_sequence.max(observation_sequence);
            if bucket.priority() > existing.bucket.priority() {
                existing.bucket = bucket;
            }
        }
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(CacheObservationMetadata {
                bucket,
                observation_sequence,
            });
        }
    }
}

fn collect_artifact_cache_candidates(
    root: &Path,
    observations: &BTreeMap<String, CacheObservationMetadata>,
) -> eyre::Result<ArtifactCacheCandidates> {
    let mut candidates = ArtifactCacheCandidates::default();
    if !root.exists() {
        return Ok(candidates);
    }

    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let observation =
            observations
                .get(&file_name)
                .copied()
                .unwrap_or(CacheObservationMetadata {
                    bucket: RuntimeCacheBucket::StaticAsset,
                    observation_sequence: 0,
                });
        candidates
            .bucket_mut(observation.bucket)
            .push(CachePruneCandidate {
                path: entry.path(),
                stable_key: format!("artifacts/{file_name}"),
                bytes: entry.metadata()?.len(),
                observation_sequence: observation.observation_sequence,
            });
    }

    Ok(candidates)
}

fn collect_runtime_receipt_artifact_sequences(
    view: &StateView<'_>,
    select_hash: impl Fn(&SoraRuntimeReceiptV1) -> Option<Hash>,
) -> BTreeMap<String, u64> {
    let mut sequences = BTreeMap::new();
    for (_, receipt) in view.world().soracloud_runtime_receipts().iter() {
        let Some(hash) = select_hash(receipt) else {
            continue;
        };
        let key = hash_cache_name(hash);
        sequences
            .entry(key)
            .and_modify(|sequence: &mut u64| {
                *sequence = (*sequence).max(receipt.emitted_sequence);
            })
            .or_insert(receipt.emitted_sequence);
    }
    sequences
}

fn collect_fixed_bucket_candidates(
    root: &Path,
    bucket_name: &str,
    observation_sequences: &BTreeMap<String, u64>,
) -> eyre::Result<Vec<CachePruneCandidate>> {
    let mut candidates = Vec::new();
    if !root.exists() {
        return Ok(candidates);
    }

    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        candidates.push(CachePruneCandidate {
            path: entry.path(),
            stable_key: format!("{bucket_name}/{file_name}"),
            bytes: entry.metadata()?.len(),
            observation_sequence: observation_sequences.get(&file_name).copied().unwrap_or(0),
        });
    }

    Ok(candidates)
}

fn prune_cache_bucket(
    mut candidates: Vec<CachePruneCandidate>,
    budget_bytes: u64,
) -> eyre::Result<()> {
    let mut retained_bytes = candidates.iter().fold(0u64, |total, candidate| {
        total.saturating_add(candidate.bytes)
    });
    if retained_bytes <= budget_bytes {
        return Ok(());
    }

    candidates.sort_by(|left, right| {
        left.observation_sequence
            .cmp(&right.observation_sequence)
            .then_with(|| left.stable_key.cmp(&right.stable_key))
    });

    for candidate in candidates {
        if retained_bytes <= budget_bytes {
            break;
        }
        fs::remove_file(&candidate.path).wrap_err_with(|| {
            format!("prune Soracloud runtime cache {}", candidate.path.display())
        })?;
        retained_bytes = retained_bytes.saturating_sub(candidate.bytes);
    }

    Ok(())
}

fn execute_asset_local_read(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let Some(artifact) =
        resolve_asset_artifact(&context.bundle, &context.handler, &request.handler_path)
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "asset handler `{}` on service `{}` cannot resolve request path `{}`",
                context.handler.handler_name, request.service_name, request.handler_path
            ),
        ));
    };
    let cache_path = state_dir
        .join("artifacts")
        .join(hash_cache_name(artifact.artifact_hash));
    let response_bytes = read_and_verify_cached_artifact(&cache_path, artifact.artifact_hash)?;
    let result_commitment = asset_result_commitment(artifact.artifact_hash, &response_bytes);
    let runtime_receipt = match context.handler.certified_response {
        SoraCertifiedResponsePolicyV1::AuditReceipt => Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
            context.hf_execution_host.as_ref(),
        )),
        SoraCertifiedResponsePolicyV1::StateCommitment => None,
        SoraCertifiedResponsePolicyV1::None => {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "asset handler `{}` cannot serve an uncertified fast-path response",
                    context.handler.handler_name
                ),
            ));
        }
    };

    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type: Some(content_type_for_path(&artifact.artifact_path).to_owned()),
        content_encoding: None,
        cache_control: Some("public, max-age=60".to_owned()),
        bindings: vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
            binding_name: None,
            state_key: None,
            payload_commitment: None,
            artifact_hash: Some(artifact.artifact_hash),
        }],
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt,
    })
}

fn execute_query_local_read(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
    egress: &iroha_config::parameters::actual::SoracloudRuntimeEgress,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    host_violation_reporter: &Arc<SoracloudModelHostViolationReporter>,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    if let Some(binding) =
        iroha_core::soracloud_runtime::soracloud_hf_generated_source_binding(&context.bundle)
    {
        return execute_generated_hf_local_read(
            view,
            request,
            context,
            state_dir,
            hf_config,
            hf_local_workers,
            host_violation_reporter,
            &binding,
        );
    }

    let bundle_cache_path = state_dir
        .join("artifacts")
        .join(hash_cache_name(context.bundle.container.bundle_hash));
    let bundle_bytes =
        read_and_verify_cached_artifact(&bundle_cache_path, context.bundle.container.bundle_hash)?;
    let verified = verify_contract_artifact(&bundle_bytes).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "verify Soracloud query bundle for service `{}` revision `{}`: {error}",
                request.service_name, request.service_version,
            ),
        )
    })?;
    let Some(entrypoint) = verified
        .contract_interface
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == context.handler.entrypoint)
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "query handler `{}` on service `{}` revision `{}` is missing entrypoint `{}`",
                context.handler.handler_name,
                request.service_name,
                request.service_version,
                context.handler.entrypoint,
            ),
        ));
    };

    let body_tlv = local_read_request_body_tlv_bytes(request).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "encode Soracloud query body for service `{}` handler `{}`: {}",
                request.service_name,
                request.handler_name,
                vm_error_label(&error),
            ),
        )
    })?;
    let metadata_tlv = local_read_request_metadata_tlv_bytes(request)?;
    let public_inputs = local_read_public_inputs(&body_tlv, &metadata_tlv, request.observed_height)
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "prepare Soracloud query public inputs for service `{}` handler `{}`: {}",
                    request.service_name,
                    request.handler_name,
                    vm_error_label(&error),
                ),
            )
        })?;

    let committed_entries =
        collect_committed_service_state_entries(view, request.service_name.as_str());
    let host = SoracloudIvmHost::new(
        local_read_execution_request(request, context),
        state_dir.to_path_buf(),
        egress.clone(),
        committed_entries,
    )
    .with_public_inputs(public_inputs);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&bundle_bytes).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "load Soracloud query bundle for service `{}` revision `{}`: {}",
                request.service_name,
                request.service_version,
                vm_error_label(&error),
            ),
        )
    })?;
    let entry_pc = u64::try_from(verified.code_offset.saturating_sub(verified.header_len))
        .unwrap_or(u64::MAX)
        .saturating_add(entrypoint.entry_pc);
    vm.set_program_counter(entry_pc).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "position Soracloud query bundle entrypoint `{}` for service `{}` revision `{}`: {}",
                context.handler.entrypoint,
                request.service_name,
                request.service_version,
                vm_error_label(&error),
            ),
        )
    })?;

    let body_ptr = vm.alloc_input_tlv(&body_tlv).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "stage Soracloud query body for service `{}` handler `{}`: {}",
                request.service_name,
                request.handler_name,
                vm_error_label(&error),
            ),
        )
    })?;
    let metadata_ptr = vm.alloc_input_tlv(&metadata_tlv).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "stage Soracloud query metadata for service `{}` handler `{}`: {}",
                request.service_name,
                request.handler_name,
                vm_error_label(&error),
            ),
        )
    })?;
    vm.set_register(10, body_ptr);
    vm.set_register(11, metadata_ptr);
    vm.set_register(12, request.observed_height);
    vm.run().map_err(|error| {
        let error_label = vm_error_label(&error);
        let error_detail = vm
            .last_diagnostic()
            .and_then(|diagnostic| diagnostic.context.syscall)
            .map_or_else(
                || error_label.to_owned(),
                |syscall| format!("{error_label}(0x{syscall:02x})"),
            );
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "execute Soracloud query handler `{}` on service `{}` revision `{}`: {}",
                context.handler.handler_name,
                request.service_name,
                request.service_version,
                error_detail,
            ),
        )
    })?;

    let (response_bytes, content_type) = decode_local_read_vm_output(&vm, request, context)?;
    let Some(host) = vm
        .host_mut_any()
        .and_then(|host| host.downcast_mut::<SoracloudIvmHost>())
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "local Soracloud query host for service `{}` handler `{}` is unavailable after execution",
                request.service_name, request.handler_name
            ),
        ));
    };
    if host.has_local_read_side_effects() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "query handler `{}` on service `{}` attempted to mutate Soracloud runtime state during a public local read",
                context.handler.handler_name, request.service_name
            ),
        ));
    }
    let bindings = host.local_read_bindings();
    let result_commitment = Hash::new(&response_bytes);
    let runtime_receipt = match context.handler.certified_response {
        SoraCertifiedResponsePolicyV1::AuditReceipt => Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
            context.hf_execution_host.as_ref(),
        )),
        SoraCertifiedResponsePolicyV1::StateCommitment => None,
        SoraCertifiedResponsePolicyV1::None => {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "query handler `{}` cannot serve an uncertified fast-path response",
                    context.handler.handler_name
                ),
            ));
        }
    };

    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type,
        content_encoding: None,
        cache_control: Some("no-store".to_owned()),
        bindings,
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt,
    })
}

fn local_read_execution_request(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
) -> SoracloudOrderedMailboxExecutionRequest {
    let execution_sequence = next_authoritative_observation_sequence_from_view(
        request.service_name.as_str(),
        request.observed_height,
    );
    let mailbox_message = SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: Hash::new(Encode::encode(&(
            "soracloud:local-read:v1",
            request.service_name.as_str(),
            request.service_version.as_str(),
            request.handler_name.as_str(),
            request.request_commitment,
        ))),
        from_service: context.deployment.service_name.clone(),
        from_handler: context.handler.handler_name.clone(),
        to_service: context.deployment.service_name.clone(),
        to_handler: context.handler.handler_name.clone(),
        payload_bytes: request.request_body.clone(),
        payload_commitment: request.request_commitment,
        enqueue_sequence: execution_sequence,
        available_after_sequence: execution_sequence,
        expires_at_sequence: None,
    };
    SoracloudOrderedMailboxExecutionRequest {
        observed_height: request.observed_height,
        observed_block_hash: request.observed_block_hash,
        execution_sequence,
        deployment: context.deployment.clone(),
        bundle: context.bundle.clone(),
        handler: Some(context.handler.clone()),
        mailbox_message,
        runtime_state: None,
        authoritative_pending_mailbox_messages: 0,
    }
}

fn local_read_request_body_tlv_bytes(
    request: &SoracloudLocalReadRequest,
) -> Result<Vec<u8>, VMError> {
    mailbox_payload_tlv_bytes(&request.request_body)
}

fn local_read_request_metadata_tlv_bytes(
    request: &SoracloudLocalReadRequest,
) -> Result<Vec<u8>, SoracloudRuntimeExecutionError> {
    let request_headers = request
        .request_headers
        .iter()
        .map(|(key, value)| (key.clone(), norito::json::Value::from(value.clone())))
        .collect::<norito::json::Map>();
    let mut metadata = norito::json::Map::new();
    metadata.insert(
        "schema_version".to_owned(),
        norito::json::Value::from(u64::from(1_u16)),
    );
    metadata.insert(
        "observed_height".to_owned(),
        norito::json::Value::from(request.observed_height),
    );
    metadata.insert(
        "observed_block_hash".to_owned(),
        request
            .observed_block_hash
            .map(|hash| norito::json::Value::from(hash.to_string()))
            .unwrap_or(norito::json::Value::Null),
    );
    metadata.insert(
        "service_name".to_owned(),
        norito::json::Value::from(request.service_name.clone()),
    );
    metadata.insert(
        "service_version".to_owned(),
        norito::json::Value::from(request.service_version.clone()),
    );
    metadata.insert(
        "handler_name".to_owned(),
        norito::json::Value::from(request.handler_name.clone()),
    );
    metadata.insert(
        "request_method".to_owned(),
        norito::json::Value::from(request.request_method.clone()),
    );
    metadata.insert(
        "request_path".to_owned(),
        norito::json::Value::from(request.request_path.clone()),
    );
    metadata.insert(
        "handler_path".to_owned(),
        norito::json::Value::from(request.handler_path.clone()),
    );
    metadata.insert(
        "request_query".to_owned(),
        request
            .request_query
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    metadata.insert(
        "request_headers".to_owned(),
        norito::json::Value::Object(request_headers),
    );
    metadata.insert(
        "request_commitment".to_owned(),
        norito::json::Value::from(request.request_commitment.to_string()),
    );
    metadata.insert(
        "request_body_bytes".to_owned(),
        norito::json::Value::from(u64::try_from(request.request_body.len()).unwrap_or(u64::MAX)),
    );
    metadata.insert(
        "request_body_is_tlv".to_owned(),
        norito::json::Value::from(
            ivm::pointer_abi::validate_tlv_bytes(&request.request_body).is_ok(),
        ),
    );
    let metadata_value = norito::json::Value::Object(metadata);
    let metadata_json = Json::from_norito_value_ref(&metadata_value).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "serialize Soracloud query metadata JSON for service `{}` handler `{}`: {error}",
                request.service_name, request.handler_name
            ),
        )
    })?;
    let metadata_bytes = norito::to_bytes(&metadata_json).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "serialize Soracloud query metadata for service `{}` handler `{}`: {error}",
                request.service_name, request.handler_name
            ),
        )
    })?;
    Ok(make_pointer_tlv(PointerType::Json, &metadata_bytes))
}

fn public_input_name(name: &str) -> Result<Name, VMError> {
    Name::from_str(name).map_err(|_| VMError::NoritoInvalid)
}

fn public_input_int_tlv(value: u64) -> Result<Vec<u8>, VMError> {
    let value = i64::try_from(value).unwrap_or(i64::MAX);
    let bytes = norito::to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?;
    Ok(make_pointer_tlv(PointerType::NoritoBytes, &bytes))
}

fn insert_public_input_aliases(
    inputs: &mut BTreeMap<Name, Vec<u8>>,
    aliases: &[&str],
    value: &[u8],
) -> Result<(), VMError> {
    for alias in aliases {
        inputs.insert(public_input_name(alias)?, value.to_vec());
    }
    Ok(())
}

fn pointer_tlv_payload(tlv_bytes: &[u8]) -> &[u8] {
    ivm::pointer_abi::validate_tlv_bytes(tlv_bytes)
        .map(|tlv| tlv.payload)
        .unwrap_or(tlv_bytes)
}

fn json_value_from_tlv(tlv_bytes: &[u8]) -> Result<norito::json::Value, VMError> {
    let tlv = ivm::pointer_abi::validate_tlv_bytes(tlv_bytes).map_err(|_| VMError::DecodeError)?;
    if tlv.type_id != PointerType::Json {
        return Err(VMError::DecodeError);
    }
    match norito::decode_from_bytes::<Json>(tlv.payload) {
        Ok(json) => json
            .try_into_any_norito::<norito::json::Value>()
            .map_err(|_| VMError::DecodeError),
        Err(_) => {
            let value = norito::json::from_slice::<norito::json::Value>(tlv.payload)
                .map_err(|_| VMError::DecodeError)?;
            Ok(value)
        }
    }
}

fn json_pointer_response_payload(payload: &[u8]) -> Vec<u8> {
    norito::decode_from_bytes::<Json>(payload)
        .map_or_else(|_| payload.to_vec(), |json| json.get().as_bytes().to_vec())
}

fn trigger_event_json_tlv(fields: norito::json::Map) -> Result<Vec<u8>, VMError> {
    let value = norito::json::Value::Object(fields);
    let json = Json::from_norito_value_ref(&value).map_err(|_| VMError::DecodeError)?;
    let bytes = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
    Ok(make_pointer_tlv(PointerType::Json, &bytes))
}

fn local_read_public_inputs(
    body_tlv: &[u8],
    metadata_tlv: &[u8],
    observed_height: u64,
) -> Result<BTreeMap<Name, Vec<u8>>, VMError> {
    let mut inputs = BTreeMap::new();
    let mut trigger_event = norito::json::Map::new();
    trigger_event.insert(
        "_request_body".to_owned(),
        norito::json::Value::from(hex::encode(pointer_tlv_payload(body_tlv))),
    );
    trigger_event.insert(
        "_request_meta".to_owned(),
        json_value_from_tlv(metadata_tlv)?,
    );
    trigger_event.insert(
        "observed_height".to_owned(),
        norito::json::Value::from(observed_height),
    );
    let trigger_event_tlv = trigger_event_json_tlv(trigger_event)?;
    insert_public_input_aliases(
        &mut inputs,
        &["trigger_event_json", "event", "entrypoint_payload"],
        &trigger_event_tlv,
    )?;
    insert_public_input_aliases(
        &mut inputs,
        &[
            "_request_body",
            "request_body",
            "body",
            "payload",
            "arg0",
            "param0",
        ],
        body_tlv,
    )?;
    insert_public_input_aliases(
        &mut inputs,
        &[
            "_request_meta",
            "request_meta",
            "metadata",
            "meta",
            "arg1",
            "param1",
        ],
        metadata_tlv,
    )?;
    let observed_height_tlv = public_input_int_tlv(observed_height)?;
    insert_public_input_aliases(
        &mut inputs,
        &["observed_height", "height", "arg2", "param2"],
        &observed_height_tlv,
    )?;
    Ok(inputs)
}

fn ordered_mailbox_public_inputs(
    payload_tlv: &[u8],
    execution_sequence: u64,
    observed_height: u64,
) -> Result<BTreeMap<Name, Vec<u8>>, VMError> {
    let mut inputs = BTreeMap::new();
    let mut trigger_event = norito::json::Map::new();
    trigger_event.insert(
        "_request_body".to_owned(),
        norito::json::Value::from(hex::encode(pointer_tlv_payload(payload_tlv))),
    );
    trigger_event.insert(
        "execution_sequence".to_owned(),
        norito::json::Value::from(execution_sequence),
    );
    trigger_event.insert(
        "observed_height".to_owned(),
        norito::json::Value::from(observed_height),
    );
    let trigger_event_tlv = trigger_event_json_tlv(trigger_event)?;
    insert_public_input_aliases(
        &mut inputs,
        &["trigger_event_json", "event", "entrypoint_payload"],
        &trigger_event_tlv,
    )?;
    insert_public_input_aliases(
        &mut inputs,
        &[
            "_request_body",
            "request_body",
            "body",
            "payload",
            "arg0",
            "param0",
        ],
        payload_tlv,
    )?;
    let execution_sequence_tlv = public_input_int_tlv(execution_sequence)?;
    insert_public_input_aliases(
        &mut inputs,
        &["execution_sequence", "sequence", "arg1", "param1"],
        &execution_sequence_tlv,
    )?;
    let observed_height_tlv = public_input_int_tlv(observed_height)?;
    insert_public_input_aliases(
        &mut inputs,
        &["observed_height", "height", "arg2", "param2"],
        &observed_height_tlv,
    )?;
    Ok(inputs)
}

fn decode_local_read_vm_output(
    vm: &IVM,
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
) -> Result<(Vec<u8>, Option<String>), SoracloudRuntimeExecutionError> {
    decode_vm_output(
        vm,
        "query",
        context.handler.handler_name.as_ref(),
        request.service_name.as_str(),
        request.service_version.as_str(),
    )
}

fn decode_ordered_mailbox_vm_output(
    vm: &IVM,
    request: &SoracloudOrderedMailboxExecutionRequest,
) -> Result<(Vec<u8>, Option<String>), SoracloudRuntimeExecutionError> {
    let handler_name = request
        .handler
        .as_ref()
        .map(|handler| handler.handler_name.as_ref())
        .unwrap_or_else(|| request.mailbox_message.to_handler.as_ref());
    let execution_kind = match request
        .handler
        .as_ref()
        .map(|handler| handler.class)
        .unwrap_or(SoraServiceHandlerClassV1::Update)
    {
        SoraServiceHandlerClassV1::Update => "update",
        SoraServiceHandlerClassV1::PrivateUpdate => "private update",
        SoraServiceHandlerClassV1::Query => "query",
        SoraServiceHandlerClassV1::Asset => "asset",
    };
    decode_vm_output(
        vm,
        execution_kind,
        handler_name,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
    )
}

fn decode_vm_output(
    vm: &IVM,
    execution_kind: &str,
    handler_name: &str,
    service_name: &str,
    service_version: &str,
) -> Result<(Vec<u8>, Option<String>), SoracloudRuntimeExecutionError> {
    let response_ptr = vm.register(10);
    if response_ptr == 0 {
        return Ok((Vec::new(), None));
    }
    let tlv = vm.memory.validate_tlv(response_ptr).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "{execution_kind} handler `{handler_name}` on service `{service_name}` revision `{service_version}` returned an invalid pointer: {}",
                vm_error_label(&error),
            ),
        )
    })?;
    let (response_bytes, content_type) = match tlv.type_id {
        PointerType::Json => (
            json_pointer_response_payload(tlv.payload),
            Some("application/json".to_owned()),
        ),
        PointerType::Blob => (
            tlv.payload.to_vec(),
            Some("application/octet-stream".to_owned()),
        ),
        PointerType::NoritoBytes => (
            tlv.payload.to_vec(),
            Some("application/x-norito".to_owned()),
        ),
        other => {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "{execution_kind} handler `{handler_name}` on service `{service_name}` revision `{service_version}` returned unsupported pointer type {:?}",
                    other,
                ),
            ));
        }
    };
    Ok((response_bytes, content_type))
}

fn execute_generated_hf_local_read(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    host_violation_reporter: &Arc<SoracloudModelHostViolationReporter>,
    binding: &iroha_core::soracloud_runtime::SoracloudHfGeneratedSourceBinding,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    match context.handler.handler_name.as_ref() {
        "metadata" => execute_generated_hf_metadata_local_read(
            request, context, state_dir, hf_config, binding,
        ),
        "infer" => execute_generated_hf_infer_local_read(
            view,
            request,
            context,
            state_dir,
            hf_config,
            hf_local_workers,
            host_violation_reporter,
            binding,
        ),
        other => Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("unsupported generated HF handler `{other}`"),
        )),
    }
}

fn execute_generated_hf_metadata_local_read(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    binding: &iroha_core::soracloud_runtime::SoracloudHfGeneratedSourceBinding,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    ensure_generated_hf_execution_host_ready(
        context.hf_execution_host.as_ref(),
        false,
        request.service_name.as_str(),
        &binding.source_id,
    )?;
    let import_manifest =
        read_hf_import_manifest(state_dir, &binding.source_id).map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "read generated HF import manifest for source `{}`: {error}",
                    binding.source_id
                ),
            )
        })?;
    let imported_total_bytes = import_manifest.as_ref().map_or(0, |manifest| {
        manifest.imported_files.iter().fold(0_u64, |total, file| {
            total.saturating_add(file.content_length)
        })
    });
    let response = HfGeneratedMetadataResponse {
        schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
        source_id: binding.source_id.clone(),
        repo_id: binding.repo_id.clone(),
        requested_revision: binding.resolved_revision.clone(),
        resolved_commit: import_manifest
            .as_ref()
            .and_then(|manifest| manifest.resolved_commit.clone()),
        model_name: binding.model_name.clone(),
        adapter_id: import_manifest
            .as_ref()
            .map(|manifest| manifest.adapter_id.clone())
            .unwrap_or_else(|| "hf.shared.v1".to_owned()),
        pipeline_tag: import_manifest
            .as_ref()
            .and_then(|manifest| manifest.pipeline_tag.clone()),
        library_name: import_manifest
            .as_ref()
            .and_then(|manifest| manifest.library_name.clone()),
        tags: import_manifest
            .as_ref()
            .map(|manifest| manifest.tags.clone())
            .unwrap_or_default(),
        imported: import_manifest.is_some(),
        imported_at_ms: import_manifest
            .as_ref()
            .map(|manifest| manifest.imported_at_ms),
        imported_file_count: import_manifest
            .as_ref()
            .map(|manifest| u32::try_from(manifest.imported_files.len()).unwrap_or(u32::MAX))
            .unwrap_or(0),
        imported_total_bytes,
        imported_files: import_manifest
            .as_ref()
            .map(|manifest| manifest.imported_files.clone())
            .unwrap_or_default(),
        skipped_files: import_manifest
            .as_ref()
            .map(|manifest| manifest.skipped_files.clone())
            .unwrap_or_default(),
        import_error: import_manifest
            .as_ref()
            .and_then(|manifest| manifest.import_error.clone()),
        inference_local_enabled: hf_config.local_execution_enabled,
        inference_bridge_enabled: hf_config
            .inference_token
            .as_ref()
            .is_some_and(|token| !token.trim().is_empty()),
    };
    let response_bytes = norito::json::to_vec(&response).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!("serialize generated HF metadata response: {error}"),
        )
    })?;
    let result_commitment = Hash::new(&response_bytes);
    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type: Some("application/json".to_owned()),
        content_encoding: None,
        cache_control: Some("no-store".to_owned()),
        bindings: Vec::new(),
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt: Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
            context.hf_execution_host.as_ref(),
        )),
    })
}

fn execute_generated_hf_infer_local_read(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    host_violation_reporter: &Arc<SoracloudModelHostViolationReporter>,
    binding: &iroha_core::soracloud_runtime::SoracloudHfGeneratedSourceBinding,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    if !request.request_method.eq_ignore_ascii_case("POST") {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "generated HF `/infer` only supports POST requests",
        ));
    }
    ensure_generated_hf_execution_host_ready(
        context.hf_execution_host.as_ref(),
        true,
        request.service_name.as_str(),
        &binding.source_id,
    )?;
    let bridge_fallback_opt_in = request
        .request_headers
        .get(HF_ALLOW_BRIDGE_FALLBACK_HEADER_V1)
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
        });
    let local_error = if hf_config.local_execution_enabled {
        match execute_generated_hf_local_runner(
            request,
            context,
            state_dir,
            hf_config,
            hf_local_workers,
            binding,
        ) {
            Ok(response) => return Ok(response),
            Err(error) => {
                report_generated_hf_runtime_execution_failure(
                    host_violation_reporter,
                    view,
                    context.hf_execution_host.as_ref(),
                    binding.source_id.as_str(),
                    &error,
                );
                Some(error)
            }
        }
    } else {
        None
    };

    let bridge_response = if hf_config.allow_inference_bridge_fallback && bridge_fallback_opt_in {
        Some(execute_generated_hf_inference_bridge_local_read(
            request, context, hf_config, binding,
        ))
    } else {
        None
    };

    match (local_error, bridge_response) {
        (_, Some(Ok(response))) => Ok(response),
        (Some(local_error), Some(Err(bridge_error))) => Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF local execution for source `{}` failed: {}; bridge fallback also failed: {}",
                binding.source_id, local_error.message, bridge_error.message
            ),
        )),
        (Some(local_error), None) => Err(local_error),
        (None, Some(Err(bridge_error))) => Err(bridge_error),
        (None, None) => Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF inference for source `{}` has no enabled runtime backend",
                binding.source_id
            ),
        )),
    }
}

fn execute_generated_hf_local_runner(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    binding: &iroha_core::soracloud_runtime::SoracloudHfGeneratedSourceBinding,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let Some(import_manifest) = read_hf_import_manifest(state_dir, &binding.source_id).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "read generated HF import manifest for source `{}` before local execution: {error}",
                binding.source_id
            ),
        )
    })? else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF source `{}` is not imported into the local cache yet",
                binding.source_id
            ),
        ));
    };
    if let Some(import_error) = import_manifest.import_error.as_ref() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF source `{}` is blocked by a local import error: {import_error}",
                binding.source_id
            ),
        ));
    }

    let request_body = if request.request_body.is_empty() {
        norito::json::Value::Object(norito::json::Map::new())
    } else {
        norito::json::from_slice::<norito::json::Value>(&request.request_body).map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("generated HF local inference expects a JSON request body: {error}"),
            )
        })?
    };

    let runner_script_path = ensure_hf_local_runner_script(state_dir).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "materialize embedded HF local runner for source `{}`: {error}",
                binding.source_id
            ),
        )
    })?;
    let source_files_dir = hf_local_source_files_root(state_dir, &binding.source_id);
    let mut runner_request = norito::json::Map::new();
    runner_request.insert(
        "schema_version".to_owned(),
        norito::json::Value::from(HF_LOCAL_RUNNER_REQUEST_SCHEMA_VERSION_V1),
    );
    runner_request.insert(
        "source_id".to_owned(),
        norito::json::Value::from(binding.source_id.clone()),
    );
    runner_request.insert(
        "repo_id".to_owned(),
        norito::json::Value::from(binding.repo_id.clone()),
    );
    runner_request.insert(
        "resolved_revision".to_owned(),
        norito::json::Value::from(binding.resolved_revision.clone()),
    );
    runner_request.insert(
        "model_name".to_owned(),
        norito::json::Value::from(binding.model_name.clone()),
    );
    runner_request.insert(
        "adapter_id".to_owned(),
        norito::json::Value::from(import_manifest.adapter_id.clone()),
    );
    runner_request.insert(
        "pipeline_tag".to_owned(),
        import_manifest
            .pipeline_tag
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    runner_request.insert(
        "library_name".to_owned(),
        import_manifest
            .library_name
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    runner_request.insert(
        "source_files_dir".to_owned(),
        norito::json::Value::from(source_files_dir.display().to_string()),
    );
    runner_request.insert(
        "request_method".to_owned(),
        norito::json::Value::from(request.request_method.clone()),
    );
    runner_request.insert(
        "request_path".to_owned(),
        norito::json::Value::from(request.request_path.clone()),
    );
    runner_request.insert(
        "request_query".to_owned(),
        request
            .request_query
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    let request_headers = request
        .request_headers
        .iter()
        .map(|(key, value)| (key.clone(), norito::json::Value::from(value.clone())))
        .collect::<norito::json::Map>();
    runner_request.insert(
        "request_headers".to_owned(),
        norito::json::Value::Object(request_headers),
    );
    runner_request.insert("request_body".to_owned(), request_body);
    let runner_request = norito::json::Value::Object(runner_request);
    let runner_request_bytes = norito::json::to_vec(&runner_request).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "serialize local HF runner request for source `{}`: {error}",
                binding.source_id
            ),
        )
    })?;
    let worker_cache_key = HfLocalRunnerWorkerCacheKey {
        source_id: binding.source_id.clone(),
        repo_id: binding.repo_id.clone(),
        resolved_revision: binding.resolved_revision.clone(),
        model_name: binding.model_name.clone(),
        adapter_id: import_manifest.adapter_id.clone(),
        pipeline_tag: import_manifest.pipeline_tag.clone(),
        library_name: import_manifest.library_name.clone(),
        imported_at_ms: import_manifest.imported_at_ms,
        source_files_dir: source_files_dir.clone(),
        runner_program: hf_config.local_runner_program.trim().to_owned(),
        runner_script_path,
        runner_script_revision: Hash::new(HF_LOCAL_RUNNER_SCRIPT_V1.as_bytes()).to_string(),
    };
    let output = execute_hf_local_runner_request(
        hf_local_workers,
        worker_cache_key,
        hf_config.local_runner_timeout,
        &runner_request_bytes,
    )?;
    let runner_response: norito::json::Value =
        norito::json::from_slice(&output).map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "decode local HF runner response for source `{}` as JSON: {error}",
                    binding.source_id
                ),
            )
        })?;
    let ok = runner_response
        .get("ok")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    if !ok {
        let message = runner_response
            .get("error")
            .and_then(norito::json::Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or("local HF runner failed without an error message");
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF local execution for source `{}` failed: {message}",
                binding.source_id
            ),
        ));
    }
    let response_json = runner_response
        .get("response_json")
        .cloned()
        .ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "local HF runner for source `{}` did not return `response_json`",
                    binding.source_id
                ),
            )
        })?;
    let response_bytes = norito::json::to_vec(&response_json).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "serialize local HF runner JSON response for source `{}`: {error}",
                binding.source_id
            ),
        )
    })?;
    let result_commitment = Hash::new(&response_bytes);
    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type: runner_response
            .get("content_type")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| Some("application/json".to_owned())),
        content_encoding: None,
        cache_control: Some("no-store".to_owned()),
        bindings: Vec::new(),
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt: Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
            context.hf_execution_host.as_ref(),
        )),
    })
}

fn probe_hf_local_runner_for_source(
    state_dir: &Path,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    source_id: &str,
    source: &iroha_data_model::soracloud::SoraHfSourceRecordV1,
) -> Result<(), SoracloudRuntimeExecutionError> {
    if !hf_config.local_execution_enabled {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF local execution for source `{source_id}` requires `soracloud_runtime.hf.local_execution_enabled = true`"
            ),
        ));
    }
    let Some(import_manifest) = read_hf_import_manifest(state_dir, source_id).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "read generated HF import manifest for source `{source_id}` before local worker probe: {error}"
            ),
        )
    })? else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF source `{source_id}` is not imported into the local cache yet"
            ),
        ));
    };
    if let Some(import_error) = import_manifest.import_error.as_ref() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF source `{source_id}` is blocked by a local import error: {import_error}"
            ),
        ));
    }

    let runner_script_path = ensure_hf_local_runner_script(state_dir).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!("materialize embedded HF local runner for source `{source_id}`: {error}"),
        )
    })?;
    let source_files_dir = hf_local_source_files_root(state_dir, source_id);
    let mut runner_request = norito::json::Map::new();
    runner_request.insert(
        "schema_version".to_owned(),
        norito::json::Value::from(HF_LOCAL_RUNNER_REQUEST_SCHEMA_VERSION_V1),
    );
    runner_request.insert(
        "source_id".to_owned(),
        norito::json::Value::from(source_id.to_owned()),
    );
    runner_request.insert(
        "repo_id".to_owned(),
        norito::json::Value::from(source.repo_id.clone()),
    );
    runner_request.insert(
        "resolved_revision".to_owned(),
        norito::json::Value::from(source.resolved_revision.clone()),
    );
    runner_request.insert(
        "model_name".to_owned(),
        norito::json::Value::from(source.model_name.clone()),
    );
    runner_request.insert(
        "adapter_id".to_owned(),
        norito::json::Value::from(import_manifest.adapter_id.clone()),
    );
    runner_request.insert(
        "pipeline_tag".to_owned(),
        import_manifest
            .pipeline_tag
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    runner_request.insert(
        "library_name".to_owned(),
        import_manifest
            .library_name
            .clone()
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );
    runner_request.insert(
        "source_files_dir".to_owned(),
        norito::json::Value::from(source_files_dir.display().to_string()),
    );
    runner_request.insert(
        "request_method".to_owned(),
        norito::json::Value::from("GET"),
    );
    runner_request.insert(
        "request_path".to_owned(),
        norito::json::Value::from("/health"),
    );
    runner_request.insert("request_query".to_owned(), norito::json::Value::Null);
    runner_request.insert(
        "request_headers".to_owned(),
        norito::json::Value::Object(norito::json::Map::new()),
    );
    runner_request.insert(
        "request_body".to_owned(),
        norito::json::Value::Object(norito::json::Map::new()),
    );
    runner_request.insert("probe_only".to_owned(), norito::json::Value::Bool(true));
    let runner_request = norito::json::Value::Object(runner_request);
    let runner_request_bytes = norito::json::to_vec(&runner_request).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!("serialize local HF worker probe request for source `{source_id}`: {error}"),
        )
    })?;
    let worker_cache_key = HfLocalRunnerWorkerCacheKey {
        source_id: source_id.to_owned(),
        repo_id: source.repo_id.clone(),
        resolved_revision: source.resolved_revision.clone(),
        model_name: source.model_name.clone(),
        adapter_id: import_manifest.adapter_id.clone(),
        pipeline_tag: import_manifest.pipeline_tag.clone(),
        library_name: import_manifest.library_name.clone(),
        imported_at_ms: import_manifest.imported_at_ms,
        source_files_dir,
        runner_program: hf_config.local_runner_program.trim().to_owned(),
        runner_script_path,
        runner_script_revision: Hash::new(HF_LOCAL_RUNNER_SCRIPT_V1.as_bytes()).to_string(),
    };
    let output = execute_hf_local_runner_request(
        hf_local_workers,
        worker_cache_key,
        hf_config.local_runner_timeout,
        &runner_request_bytes,
    )?;
    let runner_response: norito::json::Value =
        norito::json::from_slice(&output).map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!(
                    "decode local HF worker probe response for source `{source_id}` as JSON: {error}"
                ),
            )
        })?;
    let ok = runner_response
        .get("ok")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    if !ok {
        let message = runner_response
            .get("error")
            .and_then(norito::json::Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or("local HF worker probe failed without an error message");
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!("generated HF local worker probe for source `{source_id}` failed: {message}"),
        ));
    }
    Ok(())
}

fn execute_generated_hf_inference_bridge_local_read(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    hf_config: &iroha_config::parameters::actual::SoracloudRuntimeHuggingFace,
    binding: &iroha_core::soracloud_runtime::SoracloudHfGeneratedSourceBinding,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let Some(token) = hf_config
        .inference_token
        .as_ref()
        .map(|token| token.trim())
        .filter(|token| !token.is_empty())
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF inference for source `{}` requires `soracloud_runtime.hf.inference_token`",
                binding.source_id
            ),
        ));
    };
    let mut url =
        hf_inference_url(&hf_config.inference_base_url, &binding.repo_id).map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("build generated HF inference URL: {error}"),
            )
        })?;
    url.set_query(request.request_query.as_deref());
    let client = reqwest::blocking::Client::builder()
        .timeout(hf_config.request_timeout)
        .build()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("build generated HF inference HTTP client: {error}"),
            )
        })?;
    let mut builder = client
        .post(url.clone())
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    if let Some(content_type) = request.request_headers.get("content-type") {
        builder = builder.header(reqwest::header::CONTENT_TYPE, content_type);
    } else {
        builder = builder.header(reqwest::header::CONTENT_TYPE, "application/json");
    }
    if let Some(accept) = request.request_headers.get("accept") {
        builder = builder.header(reqwest::header::ACCEPT, accept);
    }
    let response = builder
        .body(request.request_body.clone())
        .send()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!("forward generated HF inference request to {url}: {error}"),
            )
        })?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let content_encoding = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let response_bytes = response.bytes().map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!("read generated HF inference response from {url}: {error}"),
        )
    })?;
    if !status.is_success() {
        let detail = String::from_utf8_lossy(&response_bytes).into_owned();
        return Err(SoracloudRuntimeExecutionError::new(
            if status.is_client_error() {
                SoracloudRuntimeExecutionErrorKind::InvalidRequest
            } else {
                SoracloudRuntimeExecutionErrorKind::Unavailable
            },
            format!(
                "generated HF inference request for `{}` failed with {}: {}",
                binding.repo_id, status, detail
            ),
        ));
    }
    let response_bytes = response_bytes.to_vec();
    let result_commitment = Hash::new(&response_bytes);
    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type,
        content_encoding,
        cache_control: Some("no-store".to_owned()),
        bindings: Vec::new(),
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt: Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
            context.hf_execution_host.as_ref(),
        )),
    })
}

fn validate_local_runtime_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudLocalReadRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height
        || request.observed_block_hash != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "local read snapshot is stale: request observed height/hash {:?}/{:?}, committed {:?}/{:?}",
                request.observed_height,
                request.observed_block_hash,
                committed_height,
                committed_block_hash
            ),
        ));
    }
    let snapshot_block_hash = parse_snapshot_hash(snapshot.observed_block_hash.as_deref())?;
    if !local_read_snapshot_covers_committed_state(
        snapshot.observed_height,
        snapshot_block_hash,
        committed_height,
        committed_block_hash,
    ) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "runtime-manager hydration is behind committed state for service `{}`",
                request.service_name
            ),
        ));
    }
    let Some(service_versions) = snapshot.services.get(&request.service_name) else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` is not materialized in the node-local runtime snapshot",
                request.service_name
            ),
        ));
    };
    let Some(plan) = service_versions.get(&request.service_version) else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` revision `{}` is not materialized locally",
                request.service_name, request.service_version
            ),
        ));
    };
    if !plan.bundle_available_locally {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` revision `{}` is not hydrated locally",
                request.service_name, request.service_version
            ),
        ));
    }
    Ok(())
}

fn local_read_snapshot_covers_committed_state(
    snapshot_height: u64,
    snapshot_block_hash: Option<Hash>,
    committed_height: u64,
    committed_block_hash: Option<Hash>,
) -> bool {
    if snapshot_height == committed_height {
        return snapshot_block_hash == committed_block_hash;
    }
    if snapshot_height > committed_height {
        return false;
    }
    committed_height.saturating_sub(snapshot_height) <= SORACLOUD_LOCAL_READ_MAX_SNAPSHOT_LAG_BLOCKS
}

fn validate_apartment_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudApartmentExecutionRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height
        || request.observed_block_hash != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "apartment execution snapshot is stale: request observed height/hash {:?}/{:?}, committed {:?}/{:?}",
                request.observed_height,
                request.observed_block_hash,
                committed_height,
                committed_block_hash
            ),
        ));
    }
    if snapshot.observed_height != committed_height
        || parse_snapshot_hash(snapshot.observed_block_hash.as_deref())? != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "runtime-manager apartment snapshot is behind committed state for `{}`",
                request.apartment_name
            ),
        ));
    }
    if !snapshot.apartments.contains_key(&request.apartment_name) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "apartment `{}` is not materialized in the node-local runtime snapshot",
                request.apartment_name
            ),
        ));
    }
    Ok(())
}

fn parse_apartment_autonomy_run_id(operation: &str) -> Option<&str> {
    let run_id = operation.strip_prefix(APARTMENT_AUTONOMY_OPERATION_PREFIX_V1)?;
    (!run_id.trim().is_empty()).then_some(run_id)
}

fn apartment_declares_hf_infer(record: &SoraAgentApartmentRecordV1) -> bool {
    record
        .manifest
        .tool_capabilities
        .iter()
        .any(|capability| capability.tool == "soracloud.hf.infer")
}

fn execute_apartment_autonomy_run(
    handle: &SoracloudRuntimeManagerHandle,
    view: &StateView<'_>,
    record: &SoraAgentApartmentRecordV1,
    request: SoracloudApartmentExecutionRequest,
    run_id: &str,
) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
    if !apartment_declares_hf_infer(record) {
        return Ok(SoracloudApartmentExecutionResult {
            status: record.status,
            checkpoint_artifact_hash: None,
            journal_artifact_hash: None,
            result_commitment: apartment_result_commitment(
                &request.apartment_name,
                request.process_generation,
                &request.operation,
                request.request_commitment,
                record.status,
            ),
        });
    }

    if let Some((summary, journal_hash)) = read_apartment_autonomy_execution_summary(
        &handle.state_dir,
        &request.apartment_name,
        run_id,
    )? && summary.succeeded
    {
        return Ok(apartment_execution_result_from_summary(
            record.status,
            summary,
            journal_hash,
        ));
    }

    let run = record
        .autonomy_run_history
        .iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "apartment `{}` does not have approved autonomy run `{run_id}`",
                    request.apartment_name
                ),
            )
        })?;

    let resolved_service = resolve_generated_hf_apartment_service(view, record);
    let summary = if let Some((service_name, service_version)) = resolved_service {
        match execute_apartment_autonomy_service_request(
            handle,
            &request,
            run,
            &service_name,
            &service_version,
        ) {
            Ok((response, workflow_steps)) => successful_apartment_autonomy_summary(
                &request.apartment_name,
                run_id,
                &service_name,
                &service_version,
                response,
                request.process_generation,
                request.request_commitment,
                workflow_steps,
            ),
            Err(error) => failed_apartment_autonomy_summary(
                &request.apartment_name,
                run_id,
                Some(service_name),
                Some(service_version),
                error.message,
                request.process_generation,
                request.request_commitment,
                error.workflow_steps,
            ),
        }
    } else {
        failed_apartment_autonomy_summary(
            &request.apartment_name,
            run_id,
            None,
            None,
            "generated HF apartment does not have a locally resolved bound inference service"
                .to_owned(),
            request.process_generation,
            request.request_commitment,
            Vec::new(),
        )
    };

    let (summary, journal_hash) =
        persist_apartment_autonomy_execution_summary(&handle.state_dir, &summary)?;
    Ok(apartment_execution_result_from_summary(
        record.status,
        summary,
        journal_hash,
    ))
}

fn resolve_generated_hf_apartment_service(
    view: &StateView<'_>,
    record: &SoraAgentApartmentRecordV1,
) -> Option<(String, String)> {
    let SoraNetworkPolicyV1::Allowlist(allowed_hosts) = &record.manifest.network_egress else {
        return None;
    };
    let world = view.world();
    world
        .soracloud_service_deployments()
        .iter()
        .find_map(|(service_name, deployment)| {
            let service_label = service_name.to_string();
            let bundle = world.soracloud_service_revisions().get(&(
                service_label.clone(),
                deployment.current_service_version.clone(),
            ))?;
            let route = bundle.service.route.as_ref()?;
            if !allowed_hosts
                .iter()
                .any(|allowed| allowed.matches_host(&route.host))
            {
                return None;
            }
            if bundle.container_manifest_hash() != record.manifest.container.manifest_hash {
                return None;
            }
            soracloud_hf_generated_source_binding(bundle)?;
            Some((service_label, deployment.current_service_version.clone()))
        })
}

fn apartment_autonomy_local_read_request_with_value(
    request: &SoracloudApartmentExecutionRequest,
    service_name: &str,
    service_version: &str,
    request_value: &norito::json::Value,
    allow_bridge_fallback: bool,
) -> SoracloudLocalReadRequest {
    let request_body = norito::json::to_vec(request_value)
        .expect("Soracloud apartment request JSON encoding should be infallible");
    let request_headers = BTreeMap::from([
        ("accept".to_owned(), "application/json".to_owned()),
        ("content-type".to_owned(), "application/json".to_owned()),
        (
            HF_ALLOW_BRIDGE_FALLBACK_HEADER_V1.to_owned(),
            if allow_bridge_fallback {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        ),
    ]);
    let mut local_read = SoracloudLocalReadRequest {
        observed_height: request.observed_height,
        observed_block_hash: request.observed_block_hash,
        service_name: service_name.to_owned(),
        service_version: service_version.to_owned(),
        handler_name: APARTMENT_AUTONOMY_HANDLER_NAME_V1.to_owned(),
        handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
        request_method: "POST".to_owned(),
        request_path: APARTMENT_AUTONOMY_HANDLER_PATH_V1.to_owned(),
        handler_path: APARTMENT_AUTONOMY_HANDLER_PATH_V1.to_owned(),
        request_query: None,
        request_headers,
        request_body,
        request_commitment: Hash::new(b""),
    };
    local_read.request_commitment = apartment_autonomy_local_read_request_commitment(&local_read);
    local_read
}

fn apartment_autonomy_request_value(
    apartment_name: &str,
    run: &iroha_data_model::soracloud::SoraAgentAutonomyRunRecordV1,
) -> Result<norito::json::Value, norito::json::Error> {
    if let Some(workflow_input_json) = run.workflow_input_json.as_deref() {
        return norito::json::from_str::<norito::json::Value>(workflow_input_json);
    }
    let mut parameters = norito::json::Map::new();
    parameters.insert(
        "artifact_hash".to_owned(),
        norito::json::Value::String(run.artifact_hash.clone()),
    );
    if let Some(provenance_hash) = run.provenance_hash.as_ref() {
        parameters.insert(
            "provenance_hash".to_owned(),
            norito::json::Value::String(provenance_hash.clone()),
        );
    }
    parameters.insert(
        "budget_units".to_owned(),
        norito::json::Value::from(run.budget_units),
    );
    parameters.insert(
        "run_id".to_owned(),
        norito::json::Value::String(run.run_id.clone()),
    );
    parameters.insert(
        "apartment_name".to_owned(),
        norito::json::Value::String(apartment_name.to_owned()),
    );

    let mut payload = norito::json::Map::new();
    payload.insert(
        "inputs".to_owned(),
        norito::json::Value::String(run.run_label.clone()),
    );
    payload.insert(
        "parameters".to_owned(),
        norito::json::Value::Object(parameters),
    );
    Ok(norito::json::Value::Object(payload))
}

fn parse_apartment_autonomy_workflow_spec(
    request_value: &norito::json::Value,
) -> Result<Option<Vec<ApartmentAutonomyWorkflowStepSpec>>, SoracloudRuntimeExecutionError> {
    let Some(object) = request_value.as_object() else {
        return Ok(None);
    };
    if !object.contains_key("workflow_version") && !object.contains_key("steps") {
        return Ok(None);
    }
    let workflow_version = object
        .get("workflow_version")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "apartment autonomy workflow requires integer `workflow_version`",
            )
        })?;
    if workflow_version != APARTMENT_AUTONOMY_WORKFLOW_VERSION_V1 {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "unsupported apartment autonomy workflow_version `{workflow_version}`; expected {APARTMENT_AUTONOMY_WORKFLOW_VERSION_V1}"
            ),
        ));
    }
    let steps = object
        .get("steps")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "apartment autonomy workflow requires `steps` to be a JSON array",
            )
        })?;
    if steps.is_empty() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "apartment autonomy workflow requires at least one step",
        ));
    }
    let mut seen_step_ids = BTreeSet::new();
    let mut parsed = Vec::with_capacity(steps.len());
    for (step_index, step) in steps.iter().enumerate() {
        let step_object = step.as_object().ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("workflow step {step_index} must be a JSON object"),
            )
        })?;
        let step_id = match step_object.get("step_id") {
            Some(value) => {
                let step_id = value.as_str().ok_or_else(|| {
                    SoracloudRuntimeExecutionError::new(
                        SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                        format!("workflow step {step_index} field `step_id` must be a string"),
                    )
                })?;
                let normalized = step_id.trim();
                if normalized.is_empty() {
                    return Err(SoracloudRuntimeExecutionError::new(
                        SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                        format!("workflow step {step_index} field `step_id` must not be empty"),
                    ));
                }
                if !seen_step_ids.insert(normalized.to_owned()) {
                    return Err(SoracloudRuntimeExecutionError::new(
                        SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                        format!("workflow step_id `{normalized}` is duplicated"),
                    ));
                }
                Some(normalized.to_owned())
            }
            None => None,
        };
        let request = step_object.get("request").cloned().ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("workflow step {step_index} must define `request`"),
            )
        })?;
        let allow_bridge_fallback = step_object
            .get("allow_bridge_fallback")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        parsed.push(ApartmentAutonomyWorkflowStepSpec {
            step_index: u32::try_from(step_index).unwrap_or(u32::MAX),
            step_id,
            request,
            allow_bridge_fallback,
        });
    }
    Ok(Some(parsed))
}

fn resolve_apartment_autonomy_workflow_placeholder(
    placeholder: &str,
    apartment_name: &str,
    run: &iroha_data_model::soracloud::SoraAgentAutonomyRunRecordV1,
    workflow_steps: &[SoracloudApartmentAutonomyWorkflowStepSummaryV1],
) -> Result<norito::json::Value, SoracloudRuntimeExecutionError> {
    fn workflow_step_text(
        step: &SoracloudApartmentAutonomyWorkflowStepSummaryV1,
    ) -> Option<String> {
        step.response_json
            .as_ref()
            .and_then(|value| value.get("text"))
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| step.response_text.clone())
    }

    let parts = placeholder.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        ["run", "apartment_name"] => Ok(norito::json::Value::String(apartment_name.to_owned())),
        ["run", "run_id"] => Ok(norito::json::Value::String(run.run_id.clone())),
        ["run", "run_label"] => Ok(norito::json::Value::String(run.run_label.clone())),
        ["run", "artifact_hash"] => Ok(norito::json::Value::String(run.artifact_hash.clone())),
        ["run", "provenance_hash"] => Ok(run
            .provenance_hash
            .clone()
            .map(norito::json::Value::String)
            .unwrap_or(norito::json::Value::Null)),
        ["run", "budget_units"] => Ok(norito::json::Value::from(run.budget_units)),
        ["previous", "text"] => workflow_steps
            .last()
            .and_then(workflow_step_text)
            .map(norito::json::Value::String)
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        ["previous", "json"] => workflow_steps
            .last()
            .and_then(|step| step.response_json.clone())
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        ["previous", "result_commitment"] => workflow_steps
            .last()
            .map(|step| norito::json::Value::String(step.result_commitment.to_string()))
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        ["steps", step_id, "text"] => workflow_steps
            .iter()
            .find(|step| step.step_id.as_deref() == Some(*step_id))
            .and_then(workflow_step_text)
            .map(norito::json::Value::String)
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        ["steps", step_id, "json"] => workflow_steps
            .iter()
            .find(|step| step.step_id.as_deref() == Some(*step_id))
            .and_then(|step| step.response_json.clone())
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        ["steps", step_id, "result_commitment"] => workflow_steps
            .iter()
            .find(|step| step.step_id.as_deref() == Some(*step_id))
            .map(|step| norito::json::Value::String(step.result_commitment.to_string()))
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                    format!("workflow placeholder `{placeholder}` is unavailable"),
                )
            }),
        _ => Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("unsupported workflow placeholder `{placeholder}`"),
        )),
    }
}

fn resolve_apartment_autonomy_workflow_template(
    value: &norito::json::Value,
    apartment_name: &str,
    run: &iroha_data_model::soracloud::SoraAgentAutonomyRunRecordV1,
    workflow_steps: &[SoracloudApartmentAutonomyWorkflowStepSummaryV1],
) -> Result<norito::json::Value, SoracloudRuntimeExecutionError> {
    match value {
        norito::json::Value::String(raw)
            if raw.starts_with("${") && raw.ends_with('}') && raw.len() > 3 =>
        {
            resolve_apartment_autonomy_workflow_placeholder(
                &raw[2..raw.len() - 1],
                apartment_name,
                run,
                workflow_steps,
            )
        }
        norito::json::Value::Array(items) => Ok(norito::json::Value::Array(
            items
                .iter()
                .map(|item| {
                    resolve_apartment_autonomy_workflow_template(
                        item,
                        apartment_name,
                        run,
                        workflow_steps,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        norito::json::Value::Object(object) => {
            let mut resolved = norito::json::Map::new();
            for (key, item) in object {
                resolved.insert(
                    key.clone(),
                    resolve_apartment_autonomy_workflow_template(
                        item,
                        apartment_name,
                        run,
                        workflow_steps,
                    )?,
                );
            }
            Ok(norito::json::Value::Object(resolved))
        }
        _ => Ok(value.clone()),
    }
}

fn apartment_autonomy_workflow_response_json(
    apartment_name: &str,
    run_id: &str,
    workflow_steps: &[SoracloudApartmentAutonomyWorkflowStepSummaryV1],
) -> norito::json::Value {
    let final_response = workflow_steps
        .last()
        .map_or(norito::json::Value::Null, |step| {
            step.response_json
                .clone()
                .or_else(|| {
                    step.response_text
                        .as_ref()
                        .map(|text| norito::json::Value::String(text.clone()))
                })
                .unwrap_or(norito::json::Value::Null)
        });
    let steps = workflow_steps
        .iter()
        .map(|step| {
            let mut entry = norito::json::Map::new();
            entry.insert(
                "step_index".to_owned(),
                norito::json::Value::from(step.step_index),
            );
            entry.insert(
                "request_commitment".to_owned(),
                norito::json::Value::String(step.request_commitment.to_string()),
            );
            entry.insert(
                "result_commitment".to_owned(),
                norito::json::Value::String(step.result_commitment.to_string()),
            );
            entry.insert(
                "step_id".to_owned(),
                step.step_id
                    .clone()
                    .map(norito::json::Value::String)
                    .unwrap_or(norito::json::Value::Null),
            );
            entry.insert(
                "content_type".to_owned(),
                step.content_type
                    .clone()
                    .map(norito::json::Value::String)
                    .unwrap_or(norito::json::Value::Null),
            );
            entry.insert(
                "response_json".to_owned(),
                step.response_json
                    .clone()
                    .unwrap_or(norito::json::Value::Null),
            );
            entry.insert(
                "response_text".to_owned(),
                step.response_text
                    .clone()
                    .map(norito::json::Value::String)
                    .unwrap_or(norito::json::Value::Null),
            );
            if let Some(runtime_receipt) = step.runtime_receipt.as_ref() {
                entry.insert(
                    "runtime_receipt_id".to_owned(),
                    norito::json::Value::String(runtime_receipt.receipt_id.to_string()),
                );
            }
            norito::json::Value::Object(entry)
        })
        .collect::<Vec<_>>();

    let mut payload = norito::json::Map::new();
    payload.insert(
        "workflow_version".to_owned(),
        norito::json::Value::from(APARTMENT_AUTONOMY_WORKFLOW_VERSION_V1),
    );
    payload.insert(
        "apartment_name".to_owned(),
        norito::json::Value::String(apartment_name.to_owned()),
    );
    payload.insert(
        "run_id".to_owned(),
        norito::json::Value::String(run_id.to_owned()),
    );
    payload.insert(
        "step_count".to_owned(),
        norito::json::Value::from(u64::try_from(workflow_steps.len()).unwrap_or(u64::MAX)),
    );
    payload.insert("steps".to_owned(), norito::json::Value::Array(steps));
    payload.insert("final_response".to_owned(), final_response);
    norito::json::Value::Object(payload)
}

fn execute_apartment_autonomy_service_request(
    handle: &SoracloudRuntimeManagerHandle,
    request: &SoracloudApartmentExecutionRequest,
    run: &iroha_data_model::soracloud::SoraAgentAutonomyRunRecordV1,
    service_name: &str,
    service_version: &str,
) -> Result<
    (
        SoracloudLocalReadResponse,
        Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
    ),
    ApartmentAutonomyWorkflowExecutionError,
> {
    let request_value =
        apartment_autonomy_request_value(&request.apartment_name, run).map_err(|error| {
            ApartmentAutonomyWorkflowExecutionError {
                message: format!(
                    "failed to decode autonomy request body for apartment `{}` run `{}`: {error}",
                    request.apartment_name, run.run_id
                ),
                workflow_steps: Vec::new(),
            }
        })?;
    let Some(workflow_steps) =
        parse_apartment_autonomy_workflow_spec(&request_value).map_err(|error| {
            ApartmentAutonomyWorkflowExecutionError {
                message: error.message,
                workflow_steps: Vec::new(),
            }
        })?
    else {
        let local_read_request = apartment_autonomy_local_read_request_with_value(
            request,
            service_name,
            service_version,
            &request_value,
            false,
        );
        return handle
            .execute_local_read(local_read_request)
            .map(|response| (response, Vec::new()))
            .map_err(|error| ApartmentAutonomyWorkflowExecutionError {
                message: error.message,
                workflow_steps: Vec::new(),
            });
    };

    let mut executed_steps = Vec::with_capacity(workflow_steps.len());
    let mut final_response: Option<SoracloudLocalReadResponse> = None;
    for step in workflow_steps {
        let resolved_request = resolve_apartment_autonomy_workflow_template(
            &step.request,
            &request.apartment_name,
            run,
            &executed_steps,
        )
        .map_err(|error| ApartmentAutonomyWorkflowExecutionError {
            message: format!(
                "workflow step {}{} template resolution failed: {}",
                step.step_index,
                step.step_id
                    .as_ref()
                    .map(|step_id| format!(" (`{step_id}`)"))
                    .unwrap_or_default(),
                error.message
            ),
            workflow_steps: executed_steps.clone(),
        })?;
        let local_read_request = apartment_autonomy_local_read_request_with_value(
            request,
            service_name,
            service_version,
            &resolved_request,
            step.allow_bridge_fallback,
        );
        let response = handle
            .execute_local_read(local_read_request.clone())
            .map_err(|error| ApartmentAutonomyWorkflowExecutionError {
                message: format!(
                    "workflow step {}{} failed: {}",
                    step.step_index,
                    step.step_id
                        .as_ref()
                        .map(|step_id| format!(" (`{step_id}`)"))
                        .unwrap_or_default(),
                    error.message
                ),
                workflow_steps: executed_steps.clone(),
            })?;
        let (response_json, response_text) = decode_apartment_autonomy_response_body(
            response.content_type.as_deref(),
            &response.response_bytes,
        );
        executed_steps.push(SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
            step_index: step.step_index,
            step_id: step.step_id,
            request_commitment: local_read_request.request_commitment,
            result_commitment: response.result_commitment,
            runtime_receipt: response.runtime_receipt.clone(),
            content_type: response.content_type.clone(),
            response_json,
            response_text,
        });
        final_response = Some(response);
    }
    let final_response = final_response.expect("workflow steps are not empty");
    let response_json = apartment_autonomy_workflow_response_json(
        &request.apartment_name,
        &run.run_id,
        &executed_steps,
    );
    let response_bytes = norito::json::to_vec(&response_json).map_err(|error| {
        ApartmentAutonomyWorkflowExecutionError {
            message: format!(
                "serialize workflow response for apartment `{}` run `{}`: {error}",
                request.apartment_name, run.run_id
            ),
            workflow_steps: executed_steps.clone(),
        }
    })?;
    Ok((
        SoracloudLocalReadResponse {
            response_bytes: response_bytes.clone(),
            content_type: Some("application/json".to_owned()),
            content_encoding: None,
            cache_control: Some("no-store".to_owned()),
            bindings: Vec::new(),
            result_commitment: Hash::new(&response_bytes),
            certified_by: final_response.certified_by,
            runtime_receipt: final_response.runtime_receipt,
        },
        executed_steps,
    ))
}

fn apartment_autonomy_local_read_request_commitment(request: &SoracloudLocalReadRequest) -> Hash {
    Hash::new(
        norito::to_bytes(&(
            request.observed_height,
            request.observed_block_hash,
            request.service_name.as_str(),
            request.service_version.as_str(),
            request.handler_name.as_str(),
            request.handler_class.handler_class(),
            request.request_method.as_str(),
            request.request_path.as_str(),
            request.handler_path.as_str(),
            request.request_query.clone(),
            request.request_headers.clone(),
            request.request_body.clone(),
        ))
        .expect("Soracloud apartment local-read commitment encoding should be infallible"),
    )
}

fn successful_apartment_autonomy_summary(
    apartment_name: &str,
    run_id: &str,
    service_name: &str,
    service_version: &str,
    response: SoracloudLocalReadResponse,
    process_generation: u64,
    request_commitment: Hash,
    workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
) -> SoracloudApartmentAutonomyExecutionSummaryV1 {
    let checkpoint_artifact_hash = Some(Hash::new(&response.response_bytes));
    let (response_json, response_text) = decode_apartment_autonomy_response_body(
        response.content_type.as_deref(),
        &response.response_bytes,
    );
    let runtime_receipt = response.runtime_receipt.clone();
    SoracloudApartmentAutonomyExecutionSummaryV1 {
        schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
        apartment_name: apartment_name.to_owned(),
        run_id: run_id.to_owned(),
        service_name: Some(service_name.to_owned()),
        service_version: Some(service_version.to_owned()),
        handler_name: Some(APARTMENT_AUTONOMY_HANDLER_NAME_V1.to_owned()),
        succeeded: true,
        result_commitment: apartment_autonomy_result_commitment(
            apartment_name,
            process_generation,
            run_id,
            request_commitment,
            checkpoint_artifact_hash,
            response_text.as_deref(),
            response_json.as_ref(),
            &workflow_steps,
            None,
        ),
        checkpoint_artifact_hash,
        runtime_receipt,
        workflow_steps,
        content_type: response.content_type,
        response_json,
        response_text,
        error: None,
    }
}

fn failed_apartment_autonomy_summary(
    apartment_name: &str,
    run_id: &str,
    service_name: Option<String>,
    service_version: Option<String>,
    error: String,
    process_generation: u64,
    request_commitment: Hash,
    workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
) -> SoracloudApartmentAutonomyExecutionSummaryV1 {
    SoracloudApartmentAutonomyExecutionSummaryV1 {
        schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
        apartment_name: apartment_name.to_owned(),
        run_id: run_id.to_owned(),
        service_name,
        service_version,
        handler_name: Some(APARTMENT_AUTONOMY_HANDLER_NAME_V1.to_owned()),
        succeeded: false,
        result_commitment: apartment_autonomy_result_commitment(
            apartment_name,
            process_generation,
            run_id,
            request_commitment,
            None,
            None,
            None,
            &workflow_steps,
            Some(error.as_str()),
        ),
        checkpoint_artifact_hash: None,
        runtime_receipt: None,
        workflow_steps,
        content_type: None,
        response_json: None,
        response_text: None,
        error: Some(error),
    }
}

fn decode_apartment_autonomy_response_body(
    content_type: Option<&str>,
    response_bytes: &[u8],
) -> (Option<norito::json::Value>, Option<String>) {
    let response_text = std::str::from_utf8(response_bytes)
        .ok()
        .map(ToOwned::to_owned);
    if content_type.is_some_and(|content_type| {
        content_type
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    }) {
        if let Ok(value) = norito::json::from_slice(response_bytes) {
            return (Some(value), response_text);
        }
    }
    (None, response_text)
}

fn apartment_autonomy_result_commitment(
    apartment_name: &str,
    process_generation: u64,
    run_id: &str,
    request_commitment: Hash,
    checkpoint_artifact_hash: Option<Hash>,
    response_text: Option<&str>,
    response_json: Option<&norito::json::Value>,
    workflow_steps: &[SoracloudApartmentAutonomyWorkflowStepSummaryV1],
    error: Option<&str>,
) -> Hash {
    let workflow_steps_commitment = workflow_steps
        .iter()
        .map(|step| {
            (
                step.step_index,
                step.step_id.as_deref(),
                step.request_commitment,
                step.result_commitment,
                step.content_type.as_deref(),
                step.response_text.as_deref(),
                step.response_json
                    .as_ref()
                    .map(norito::json::to_string)
                    .transpose()
                    .ok()
                    .flatten(),
            )
        })
        .collect::<Vec<_>>();
    Hash::new(Encode::encode(&(
        "soracloud.apartment.autonomy.v1",
        apartment_name,
        process_generation,
        run_id,
        request_commitment,
        checkpoint_artifact_hash,
        response_text,
        response_json
            .map(norito::json::to_string)
            .transpose()
            .ok()
            .flatten(),
        workflow_steps_commitment,
        error,
    )))
}

fn read_apartment_autonomy_execution_summary(
    state_dir: &Path,
    apartment_name: &str,
    run_id: &str,
) -> Result<
    Option<(SoracloudApartmentAutonomyExecutionSummaryV1, Hash)>,
    SoracloudRuntimeExecutionError,
> {
    let summary_path = apartment_autonomy_summary_path(state_dir, apartment_name, run_id);
    let Some(summary_bytes) = fs::read(&summary_path)
        .ok()
        .filter(|bytes| !bytes.is_empty())
    else {
        return Ok(None);
    };
    let summary =
        norito::json::from_slice::<SoracloudApartmentAutonomyExecutionSummaryV1>(&summary_bytes)
            .map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "failed to decode autonomy execution summary at {}: {error}",
                        summary_path.display()
                    ),
                )
            })?;
    Ok(Some((summary, Hash::new(&summary_bytes))))
}

fn persist_apartment_autonomy_execution_summary(
    state_dir: &Path,
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
) -> Result<(SoracloudApartmentAutonomyExecutionSummaryV1, Hash), SoracloudRuntimeExecutionError> {
    let run_root = apartment_autonomy_run_root(state_dir, &summary.apartment_name, &summary.run_id);
    fs::create_dir_all(&run_root).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "failed to create autonomy run root {}: {error}",
                run_root.display()
            ),
        )
    })?;

    if summary.succeeded {
        if let Some(checkpoint_hash) = summary.checkpoint_artifact_hash {
            let checkpoint_path = apartment_autonomy_checkpoint_path(
                state_dir,
                &summary.apartment_name,
                &summary.run_id,
            );
            let checkpoint_bytes = if let Some(response_text) = summary.response_text.as_ref() {
                response_text.as_bytes().to_vec()
            } else if let Some(response_json) = summary.response_json.as_ref() {
                norito::json::to_vec(response_json).map_err(|error| {
                    SoracloudRuntimeExecutionError::new(
                        SoracloudRuntimeExecutionErrorKind::Internal,
                        format!(
                            "failed to encode autonomy checkpoint JSON for apartment `{}` run `{}`: {error}",
                            summary.apartment_name, summary.run_id
                        ),
                    )
                })?
            } else {
                Vec::new()
            };
            if Hash::new(&checkpoint_bytes) != checkpoint_hash {
                return Err(SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "autonomy checkpoint bytes hash mismatch for apartment `{}` run `{}`",
                        summary.apartment_name, summary.run_id
                    ),
                ));
            }
            write_bytes_atomic(&checkpoint_path, &checkpoint_bytes).map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "failed to persist autonomy checkpoint {}: {error}",
                        checkpoint_path.display()
                    ),
                )
            })?;
        }
    } else {
        let checkpoint_path =
            apartment_autonomy_checkpoint_path(state_dir, &summary.apartment_name, &summary.run_id);
        if checkpoint_path.exists() {
            fs::remove_file(&checkpoint_path).map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "failed to clear stale autonomy checkpoint {}: {error}",
                        checkpoint_path.display()
                    ),
                )
            })?;
        }
    }

    let summary_path =
        apartment_autonomy_summary_path(state_dir, &summary.apartment_name, &summary.run_id);
    let summary_bytes = norito::json::to_vec_pretty(summary).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "failed to encode autonomy execution summary for apartment `{}` run `{}`: {error}",
                summary.apartment_name, summary.run_id
            ),
        )
    })?;
    write_bytes_atomic(&summary_path, &summary_bytes).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "failed to persist autonomy execution summary {}: {error}",
                summary_path.display()
            ),
        )
    })?;
    Ok((summary.clone(), Hash::new(&summary_bytes)))
}

fn apartment_execution_result_from_summary(
    status: SoraAgentRuntimeStatusV1,
    summary: SoracloudApartmentAutonomyExecutionSummaryV1,
    journal_hash: Hash,
) -> SoracloudApartmentExecutionResult {
    SoracloudApartmentExecutionResult {
        status,
        checkpoint_artifact_hash: summary.checkpoint_artifact_hash,
        journal_artifact_hash: Some(journal_hash),
        result_commitment: summary.result_commitment,
    }
}

fn apartment_autonomy_run_root(state_dir: &Path, apartment_name: &str, run_id: &str) -> PathBuf {
    state_dir
        .join("apartments")
        .join(sanitize_path_component(apartment_name))
        .join("runs")
        .join(sanitize_path_component(run_id))
}

fn apartment_autonomy_summary_path(
    state_dir: &Path,
    apartment_name: &str,
    run_id: &str,
) -> PathBuf {
    apartment_autonomy_run_root(state_dir, apartment_name, run_id)
        .join(APARTMENT_AUTONOMY_SUMMARY_FILE_V1)
}

fn apartment_autonomy_checkpoint_path(
    state_dir: &Path,
    apartment_name: &str,
    run_id: &str,
) -> PathBuf {
    apartment_autonomy_run_root(state_dir, apartment_name, run_id)
        .join(APARTMENT_AUTONOMY_CHECKPOINT_FILE_V1)
}

fn resolve_local_read_context(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
    config: &SoracloudRuntimeManagerConfig,
) -> Result<ResolvedLocalReadContext, SoracloudRuntimeExecutionError> {
    let service_id: Name = request.service_name.parse().map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "invalid Soracloud service name `{}`: {error}",
                request.service_name
            ),
        )
    })?;
    let Some(deployment) = view
        .world()
        .soracloud_service_deployments()
        .get(&service_id)
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("unknown Soracloud service `{}`", request.service_name),
        ));
    };
    if deployment.current_service_version != request.service_version {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` active version `{}` does not match requested local-read version `{}`",
                request.service_name, deployment.current_service_version, request.service_version
            ),
        ));
    }
    let Some(bundle) = view
        .world()
        .soracloud_service_revisions()
        .get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        ))
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "missing admitted Soracloud revision `{}` for service `{}`",
                request.service_version, request.service_name
            ),
        ));
    };
    ensure_ivm_runtime(
        bundle.service.execution_plane,
        bundle.container.runtime,
        request.service_name.as_str(),
        request.service_version.as_str(),
    )
    .map_err(|message| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            message,
        )
    })?;
    let route = bundle.service.route.as_ref().ok_or_else(|| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "service `{}` revision `{}` does not expose a public local-read route",
                request.service_name, request.service_version
            ),
        )
    })?;
    if route.visibility != SoraRouteVisibilityV1::Public {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "service `{}` revision `{}` local-read route is not public",
                request.service_name, request.service_version
            ),
        ));
    }
    let Some(handler) = bundle
        .service
        .handlers
        .iter()
        .find(|handler| {
            handler.handler_name.as_ref() == request.handler_name
                && handler.class == request.handler_class.handler_class()
        })
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "service `{}` revision `{}` does not expose handler `{}` for {:?}",
                request.service_name,
                request.service_version,
                request.handler_name,
                request.handler_class
            ),
        ));
    };
    if !matches!(
        handler.class,
        SoraServiceHandlerClassV1::Asset | SoraServiceHandlerClassV1::Query
    ) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "service `{}` revision `{}` handler `{}` is not publicly routable",
                request.service_name, request.service_version, request.handler_name
            ),
        ));
    }
    let hf_execution_host = if let Some(binding) = soracloud_hf_generated_source_binding(&bundle) {
        resolve_local_hf_execution_host(
            view,
            request.service_name.as_str(),
            &binding.source_id,
            config,
        )?
    } else {
        None
    };

    Ok(ResolvedLocalReadContext {
        deployment,
        bundle,
        handler,
        hf_execution_host,
    })
}

fn hf_local_host_identity_is_configured(config: &SoracloudRuntimeManagerConfig) -> bool {
    config.local_validator_account_id.is_some() || config.local_peer_id.is_some()
}

fn hf_assignment_matches_local_host(
    config: &SoracloudRuntimeManagerConfig,
    assignment: &SoraHfPlacementHostAssignmentV1,
) -> bool {
    if !hf_local_host_identity_is_configured(config) {
        return false;
    }
    config
        .local_validator_account_id
        .as_ref()
        .is_none_or(|validator_account_id| assignment.validator_account_id == *validator_account_id)
        && config
            .local_peer_id
            .as_deref()
            .is_none_or(|peer_id| assignment.peer_id == peer_id)
}

fn local_hf_source_execution_hosts(
    view: &StateView<'_>,
    source_id: &str,
    config: &SoracloudRuntimeManagerConfig,
) -> Vec<ResolvedHfPlacementExecutionHost> {
    if !hf_local_host_identity_is_configured(config) {
        return Vec::new();
    }
    view.world()
        .soracloud_hf_placements()
        .iter()
        .filter(|(pool_id, placement)| {
            placement.source_id.to_string() == source_id
                && placement.status != iroha_data_model::soracloud::SoraHfPlacementStatusV1::Retired
                && view
                    .world()
                    .soracloud_hf_shared_lease_pools()
                    .get(pool_id)
                    .is_some_and(|pool| {
                        matches!(
                            pool.status,
                            SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
                        )
                    })
        })
        .flat_map(|(_pool_id, placement)| {
            placement.assigned_hosts.iter().filter_map(|assignment| {
                if !hf_assignment_matches_local_host(config, assignment)
                    || matches!(
                        assignment.status,
                        SoraHfPlacementHostStatusV1::Unavailable
                            | SoraHfPlacementHostStatusV1::Retired
                    )
                {
                    return None;
                }
                Some(ResolvedHfPlacementExecutionHost {
                    placement_id: placement.placement_id,
                    validator_account_id: assignment.validator_account_id.clone(),
                    peer_id: assignment.peer_id.clone(),
                    role: assignment.role,
                    status: assignment.status,
                })
            })
        })
        .collect()
}

fn resolve_active_hf_placement_for_service(
    view: &StateView<'_>,
    service_name: &str,
    source_id: &str,
) -> Result<Option<SoraHfPlacementRecordV1>, SoracloudRuntimeExecutionError> {
    iroha_core::soracloud_runtime::resolve_generated_hf_active_placement(
        view.world(),
        service_name,
        source_id,
    )
    .map_err(|message| {
        SoracloudRuntimeExecutionError::new(SoracloudRuntimeExecutionErrorKind::Internal, message)
    })
}

fn resolve_local_hf_execution_host(
    view: &StateView<'_>,
    service_name: &str,
    source_id: &str,
    config: &SoracloudRuntimeManagerConfig,
) -> Result<Option<ResolvedHfPlacementExecutionHost>, SoracloudRuntimeExecutionError> {
    if !hf_local_host_identity_is_configured(config) {
        return Ok(None);
    }
    let Some(placement) = resolve_active_hf_placement_for_service(view, service_name, source_id)?
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF service `{service_name}` has no active placement for source `{source_id}`"
            ),
        ));
    };
    let Some(assignment) = placement
        .assigned_hosts
        .iter()
        .find(|assignment| hf_assignment_matches_local_host(config, assignment))
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF service `{service_name}` source `{source_id}` is not assigned to this validator host"
            ),
        ));
    };
    Ok(Some(ResolvedHfPlacementExecutionHost {
        placement_id: placement.placement_id,
        validator_account_id: assignment.validator_account_id.clone(),
        peer_id: assignment.peer_id.clone(),
        role: assignment.role,
        status: assignment.status,
    }))
}

fn ensure_generated_hf_execution_host_ready(
    host: Option<&ResolvedHfPlacementExecutionHost>,
    require_primary: bool,
    service_name: &str,
    source_id: &str,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let Some(host) = host else {
        return Ok(());
    };
    if matches!(
        host.status,
        SoraHfPlacementHostStatusV1::Unavailable | SoraHfPlacementHostStatusV1::Retired
    ) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF service `{service_name}` source `{source_id}` is assigned locally but the placement host is not currently available"
            ),
        ));
    }
    if require_primary && host.role != SoraHfPlacementHostRoleV1::Primary {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF service `{service_name}` source `{source_id}` is assigned locally as a replica; proxy-to-primary routing is still required"
            ),
        ));
    }
    if require_primary && host.status != SoraHfPlacementHostStatusV1::Warm {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "generated HF service `{service_name}` source `{source_id}` is not warm on the local primary host yet"
            ),
        ));
    }
    Ok(())
}

fn report_generated_hf_runtime_execution_failure(
    host_violation_reporter: &Arc<SoracloudModelHostViolationReporter>,
    view: &StateView<'_>,
    host: Option<&ResolvedHfPlacementExecutionHost>,
    source_id: &str,
    error: &SoracloudRuntimeExecutionError,
) {
    let Some(host) = host else {
        return;
    };
    if host.role != SoraHfPlacementHostRoleV1::Primary
        || host.status != SoraHfPlacementHostStatusV1::Warm
        || error.kind == SoracloudRuntimeExecutionErrorKind::InvalidRequest
    {
        return;
    }
    host_violation_reporter.report(
        view,
        &host.validator_account_id,
        SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
        Some(host.placement_id),
        Some(format!(
            "local HF execution for source `{source_id}` failed on the assigned primary host: {}",
            error.message
        )),
    );
}

fn resolve_asset_artifact<'a>(
    bundle: &'a SoraDeploymentBundleV1,
    handler: &SoraServiceHandlerV1,
    handler_path: &str,
) -> Option<&'a iroha_data_model::soracloud::SoraArtifactRefV1> {
    let normalized_handler_path = if handler_path.is_empty() {
        "/"
    } else {
        handler_path
    };
    let mut candidates = bundle
        .service
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == SoraArtifactKindV1::StaticAsset
                && artifact
                    .handler_name
                    .as_ref()
                    .is_some_and(|name| name == &handler.handler_name)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.artifact_path.cmp(&right.artifact_path));
    if normalized_handler_path == "/" {
        return candidates
            .iter()
            .copied()
            .find(|artifact| artifact.artifact_path.ends_with("/index.html"))
            .or_else(|| candidates.into_iter().next());
    }

    candidates
        .iter()
        .copied()
        .find(|artifact| artifact.artifact_path == normalized_handler_path)
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|artifact| artifact.artifact_path.ends_with(normalized_handler_path))
        })
}

fn read_and_verify_cached_artifact(
    cache_path: &Path,
    expected_hash: Hash,
) -> Result<Vec<u8>, SoracloudRuntimeExecutionError> {
    let response_bytes = fs::read(cache_path).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "read hydrated Soracloud artifact cache {}: {error}",
                cache_path.display()
            ),
        )
    })?;
    let actual_hash = Hash::new(&response_bytes);
    if actual_hash != expected_hash {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "hydrated Soracloud artifact cache {} failed hash verification: expected {}, found {}",
                cache_path.display(),
                expected_hash,
                actual_hash
            ),
        ));
    }
    Ok(response_bytes)
}

fn asset_result_commitment(artifact_hash: Hash, response_bytes: &[u8]) -> Hash {
    let mut payload = Vec::with_capacity(Hash::LENGTH + response_bytes.len());
    payload.extend_from_slice(artifact_hash.as_ref());
    payload.extend_from_slice(response_bytes);
    Hash::new(payload)
}

fn state_entry_binding(
    entry: &SoraServiceStateEntryV1,
) -> iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
    iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
        binding_name: Some(entry.binding_name.to_string()),
        state_key: Some(entry.state_key.clone()),
        payload_commitment: Some(entry.payload_commitment),
        artifact_hash: None,
    }
}

fn local_read_receipt(
    request: &SoracloudLocalReadRequest,
    deployment: &SoraServiceDeploymentStateV1,
    handler: &SoraServiceHandlerV1,
    result_commitment: Hash,
    certified_by: SoraCertifiedResponsePolicyV1,
    mailbox_message_id: Option<Hash>,
    placement_host: Option<&ResolvedHfPlacementExecutionHost>,
) -> SoraRuntimeReceiptV1 {
    let placement_id = placement_host.map(|host| host.placement_id);
    let selected_validator_account_id =
        placement_host.map(|host| host.validator_account_id.clone());
    let selected_peer_id = placement_host.map(|host| host.peer_id.clone());
    let emitted_sequence = next_authoritative_observation_sequence_from_view(
        deployment.service_name.as_ref(),
        request.observed_height,
    );
    SoraRuntimeReceiptV1 {
        schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: Hash::new(Encode::encode(&(
            "soracloud:local-read",
            deployment.service_name.as_ref(),
            deployment.current_service_version.as_str(),
            handler.handler_name.as_ref(),
            request.request_commitment,
            result_commitment,
            certified_by,
            placement_id,
            selected_validator_account_id.clone(),
            selected_peer_id.clone(),
        ))),
        service_name: deployment.service_name.clone(),
        service_version: deployment.current_service_version.clone(),
        handler_name: handler.handler_name.clone(),
        handler_class: handler.class,
        request_commitment: request.request_commitment,
        result_commitment,
        certified_by,
        emitted_sequence,
        mailbox_message_id,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
        placement_id,
        selected_validator_account_id,
        selected_peer_id,
    }
}

fn next_authoritative_observation_sequence_from_view(
    _service_name: &str,
    observed_height: u64,
) -> u64 {
    observed_height.max(1)
}

fn soracloud_runtime_observed_at_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1)
        .max(1)
}

fn desired_model_host_heartbeat_expiry_ms(
    now_ms: u64,
    config: &SoracloudRuntimeManagerConfig,
) -> u64 {
    let ttl_ms = u64::try_from(config.hf.model_host_heartbeat_ttl.as_millis()).unwrap_or(u64::MAX);
    now_ms.saturating_add(ttl_ms.max(1))
}

fn desired_inrou_host_heartbeat_expiry_ms(
    now_ms: u64,
    config: &SoracloudRuntimeManagerConfig,
) -> u64 {
    now_ms.saturating_add(inrou_host_heartbeat_ttl_ms(config))
}

fn inrou_host_heartbeat_ttl_ms(config: &SoracloudRuntimeManagerConfig) -> u64 {
    let interval_ms = u64::try_from(config.reconcile_interval.as_millis()).unwrap_or(u64::MAX);
    interval_ms
        .saturating_mul(4)
        .max(INROU_HOST_HEARTBEAT_TTL_FLOOR_MS)
}

fn inrou_host_heartbeat_refresh_margin_ms(config: &SoracloudRuntimeManagerConfig) -> u64 {
    let interval_ms = u64::try_from(config.reconcile_interval.as_millis()).unwrap_or(u64::MAX);
    inrou_host_heartbeat_ttl_ms(config).min(
        interval_ms
            .saturating_mul(2)
            .max(INROU_HOST_HEARTBEAT_REFRESH_MARGIN_FLOOR_MS),
    )
}

fn inrou_host_heartbeat_refresh_due(
    existing: &SoraInrouHostCapabilityRecordV1,
    now_ms: u64,
    config: &SoracloudRuntimeManagerConfig,
) -> bool {
    existing.heartbeat_expires_at_ms
        <= now_ms.saturating_add(inrou_host_heartbeat_refresh_margin_ms(config))
}

fn should_submit_local_inrou_host_capability(auto_proxy_only: bool) -> bool {
    !auto_proxy_only
}

fn inrou_host_capability_matches(
    existing: &SoraInrouHostCapabilityRecordV1,
    desired: &SoraInrouHostCapabilityRecordV1,
) -> bool {
    existing.validator_account_id == desired.validator_account_id
        && existing.peer_id == desired.peer_id
        && existing.supported_backends == desired.supported_backends
        && existing.supported_guest_isas == desired.supported_guest_isas
        && existing.max_hosted_replica_capacity == desired.max_hosted_replica_capacity
        && existing.max_cpu_millis == desired.max_cpu_millis
        && existing.max_memory_bytes == desired.max_memory_bytes
        && existing.max_storage_bytes == desired.max_storage_bytes
        && existing.proxy_only == desired.proxy_only
}

fn inrou_host_capability_refresh_needed(
    existing: Option<&SoraInrouHostCapabilityRecordV1>,
    desired: &SoraInrouHostCapabilityRecordV1,
    now_ms: u64,
    config: &SoracloudRuntimeManagerConfig,
) -> bool {
    existing.is_none_or(|existing| {
        !inrou_host_capability_matches(existing, desired)
            || !existing.is_active_at(now_ms)
            || inrou_host_heartbeat_refresh_due(existing, now_ms, config)
    })
}

fn apartment_result_commitment(
    apartment_name: &str,
    process_generation: u64,
    operation: &str,
    request_commitment: Hash,
    status: iroha_data_model::soracloud::SoraAgentRuntimeStatusV1,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud:apartment",
        apartment_name,
        process_generation,
        operation,
        request_commitment,
        status,
    )))
}

fn committed_height(view: &StateView<'_>) -> u64 {
    u64::try_from(view.height()).unwrap_or(u64::MAX)
}

fn committed_block_hash(view: &StateView<'_>) -> Option<Hash> {
    view.latest_block_hash().map(Hash::from)
}

fn parse_snapshot_hash(
    snapshot_hash: Option<&str>,
) -> Result<Option<Hash>, SoracloudRuntimeExecutionError> {
    snapshot_hash
        .map(Hash::from_str)
        .transpose()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("invalid Soracloud runtime snapshot block hash: {error}"),
            )
        })
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("css") => "text/css; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("mjs") => "application/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("xml") => "application/xml",
        _ => "application/octet-stream",
    }
}

fn current_soracloud_service_sequence(world: &impl WorldReadOnly) -> u64 {
    world
        .soracloud_service_audit_events()
        .iter()
        .map(|(sequence, _event)| *sequence)
        .max()
        .unwrap_or(0)
}

fn build_lease_volume_plans(
    bundle: &SoraDeploymentBundleV1,
    deployment: &SoraServiceDeploymentStateV1,
    state_dir: &Path,
    service_name: &str,
    service_version: &str,
) -> Vec<SoracloudRuntimeLeaseVolumePlan> {
    bundle
        .service
        .lease_volumes
        .iter()
        .map(|volume| {
            let authoritative = deployment
                .lease_volume_states
                .iter()
                .find(|state| state.volume_name == volume.volume_name);
            let local_materialization_dir = build_hosted_http_service_volume_dir(
                state_dir,
                service_name,
                service_version,
                authoritative.map_or(volume.kind, |state| state.kind),
                volume.volume_name.as_ref(),
            );
            SoracloudRuntimeLeaseVolumePlan {
                volume_name: volume.volume_name.to_string(),
                kind: authoritative.map_or(volume.kind, |state| state.kind),
                storage_class: authoritative
                    .map_or(volume.storage_class, |state| state.storage_class),
                mount_path: authoritative.map_or_else(
                    || volume.mount_path.clone(),
                    |state| state.mount_path.clone(),
                ),
                max_total_bytes: authoritative
                    .map_or(volume.max_total_bytes.get(), |state| state.max_total_bytes),
                lease_expires_sequence: authoritative.map_or(
                    deployment
                        .service_lease
                        .as_ref()
                        .map_or(u64::MAX, |lease| lease.lease_expires_sequence),
                    |state| state.lease_expires_sequence,
                ),
                authoritative_generation: authoritative
                    .map_or(1, |state| state.authoritative_generation),
                local_materialization_dir: local_materialization_dir.display().to_string(),
            }
        })
        .collect()
}

fn local_inrou_replica_placements(
    world: &impl WorldReadOnly,
    service_name: &str,
    service_version: &str,
    local_validator_account_id: Option<&AccountId>,
    local_peer_id: Option<&str>,
) -> Vec<SoraInrouReplicaPlacementV1> {
    let Some(local_validator_account_id) = local_validator_account_id else {
        return Vec::new();
    };
    let Some(local_peer_id) = local_peer_id else {
        return Vec::new();
    };
    let Some(record) = world
        .soracloud_inrou_service_placements()
        .get(&(service_name.to_owned(), service_version.to_owned()))
    else {
        return Vec::new();
    };

    let mut placements = record
        .placements
        .iter()
        .filter(|placement| {
            &placement.validator_account_id == local_validator_account_id
                && placement.peer_id == local_peer_id
        })
        .cloned()
        .collect::<Vec<_>>();
    placements.sort_by_key(|placement| placement.replica_slot);
    placements
}

fn build_inrou_runtime_plan(
    bundle: &SoraDeploymentBundleV1,
    local_assignment: Option<&SoraInrouReplicaPlacementV1>,
) -> Option<SoracloudRuntimeInrouPlan> {
    let inrou = bundle.container.inrou.as_ref()?;
    let root_volume = bundle
        .service
        .lease_volumes
        .iter()
        .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)?;
    let selected_backend = local_assignment?.selected_backend;
    let selected_guest_isa = local_assignment?.selected_guest_isa;
    let (selected_guest_isa, guest_image) = match inrou.guest_images.get(&selected_guest_isa) {
        Some(guest_image) => (selected_guest_isa, guest_image),
        None => (
            SoraInrouGuestIsaV1::Aarch64,
            inrou.guest_images.get(&SoraInrouGuestIsaV1::Aarch64)?,
        ),
    };

    Some(SoracloudRuntimeInrouPlan {
        guest_os: inrou.guest_os,
        selected_backend,
        selected_guest_isa,
        kernel_image_path: guest_image.kernel_image_path.clone(),
        rootfs_image_path: guest_image.rootfs_image_path.clone(),
        initrd_image_path: guest_image.initrd_image_path.clone(),
        bootstrap_user_data_path: inrou.bootstrap_user_data_path.clone(),
        ssh_authorized_keys: inrou.ssh_authorized_keys.clone(),
        root_volume_name: root_volume.volume_name.to_string(),
    })
}

fn build_runtime_snapshot(
    view: &StateView<'_>,
    bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    state_dir: &Path,
    artifacts_root: PathBuf,
    local_validator_account_id: Option<&AccountId>,
    local_peer_id: Option<&str>,
    local_inrou_hosting_enabled: bool,
) -> eyre::Result<SoracloudRuntimeSnapshot> {
    let mut services = BTreeMap::new();
    let world = view.world();
    let current_sequence = current_soracloud_service_sequence(world);

    for (service_name, deployment) in world.soracloud_service_deployments().iter() {
        let service_name_key = service_name.clone();
        let service_name = service_name_key.to_string();
        let versions = collect_active_versions(deployment);
        let runtime_state = world.soracloud_service_runtime().get(&service_name_key);
        let authoritative_pending = authoritative_mailbox_counts(
            world.soracloud_mailbox_messages(),
            world.soracloud_runtime_receipts(),
        );

        let mut version_plans = BTreeMap::new();
        for (service_version, role, traffic_percent) in versions {
            let bundle = bundle_registry
                .get(&(service_name.clone(), service_version.clone()))
                .ok_or_else(|| {
                    eyre::eyre!(
                        "deployment for service `{service_name}` references missing admitted revision `{service_version}`"
                    )
                })?;
            let is_runtime_active = runtime_state
                .as_ref()
                .is_some_and(|state| state.active_service_version == service_version);
            let service_dir = state_dir
                .join("services")
                .join(sanitize_path_component(&service_name))
                .join(sanitize_path_component(&service_version));
            let config_dir = service_dir.join("configs");
            let config_exports_dir = service_dir.join("config_exports");
            let effective_env_path = service_dir.join("effective_env.json");
            let secret_envelopes_dir = service_dir.join("secret_envelopes");
            let secret_payload_dir = state_dir
                .join("secrets")
                .join(sanitize_path_component(&service_name))
                .join(sanitize_path_component(&service_version));
            let service_data_dir = build_native_service_data_dir(state_dir, &service_name);
            let bundle_cache_path =
                artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
            let active_runtime_state = runtime_state
                .as_ref()
                .filter(|state| state.active_service_version == service_version);
            let artifact_plans = build_artifact_plans(bundle, &artifacts_root);
            let supports_private_secret_payload_reads = bundle
                .service
                .handlers
                .iter()
                .any(|handler| handler.class == SoraServiceHandlerClassV1::PrivateUpdate);
            let hydration_complete = artifact_plans
                .iter()
                .all(|artifact| artifact.available_locally);
            let lease_volumes = build_lease_volume_plans(
                bundle,
                deployment,
                state_dir,
                &service_name,
                &service_version,
            );
            let hosted_http_lease_active = bundle.service.execution_plane
                != iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService
                || (deployment.hosted_service_lease_active_at(current_sequence)
                    && lease_volumes
                        .iter()
                        .all(|volume| current_sequence < volume.lease_expires_sequence));
            let local_inrou_assignments = if local_inrou_hosting_enabled
                && bundle.container.runtime == SoraContainerRuntimeV1::Inrou
            {
                local_inrou_replica_placements(
                    world,
                    &service_name,
                    &service_version,
                    local_validator_account_id,
                    local_peer_id,
                )
            } else {
                Vec::new()
            };
            let hosted_http_runtime_state = if bundle.container.runtime
                == SoraContainerRuntimeV1::Inrou
                && !local_inrou_assignments.is_empty()
            {
                match read_hosted_http_runtime_state(&service_dir) {
                    Ok(state) => state,
                    Err(error) => {
                        iroha_logger::warn!(
                            ?error,
                            service_name = %service_name,
                            service_version = %service_version,
                            "failed to read hosted-HTTP runtime state while building Soracloud snapshot"
                        );
                        None
                    }
                }
            } else {
                None
            };
            let mut effective_env = build_effective_service_environment(bundle, deployment)?;
            if bundle.container.runtime == SoraContainerRuntimeV1::Inrou {
                effective_env.insert(
                    "SORACLOUD_SERVICE_DATA_DIR".to_owned(),
                    "/var/lib/soracloud/service".to_owned(),
                );
                effective_env.insert(
                    "SORACLOUD_SERVICE_MATERIALIZATION_DIR".to_owned(),
                    "/var/lib/soracloud/materialization".to_owned(),
                );
            } else {
                effective_env.insert(
                    "SORACLOUD_SERVICE_DATA_DIR".to_owned(),
                    service_data_dir.display().to_string(),
                );
                effective_env.insert(
                    "SORACLOUD_SERVICE_MATERIALIZATION_DIR".to_owned(),
                    service_dir.display().to_string(),
                );
            }
            if let Some(lease) = deployment.service_lease.as_ref() {
                effective_env.insert(
                    "SORACLOUD_SERVICE_LEASE_EXPIRES_SEQUENCE".to_owned(),
                    lease.lease_expires_sequence.to_string(),
                );
                effective_env.insert(
                    "SORACLOUD_SERVICE_PREPAID_BALANCE_NANOS".to_owned(),
                    lease.prepaid_runtime_balance_nanos.to_string(),
                );
                effective_env.insert(
                    "SORACLOUD_SERVICE_QUOTA_CLASS".to_owned(),
                    lease.quota_class.clone(),
                );
                effective_env.insert(
                    "SORACLOUD_SERVICE_REMAINING_BALANCE_NANOS".to_owned(),
                    deployment
                        .hosted_service_remaining_balance_nanos(current_sequence)
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            for volume in &lease_volumes {
                let env_suffix = sanitize_env_var_component(&volume.volume_name);
                effective_env.insert(
                    format!("SORACLOUD_LEASE_VOLUME_{env_suffix}_DIR"),
                    if bundle.container.runtime == SoraContainerRuntimeV1::Inrou {
                        volume.mount_path.clone()
                    } else {
                        volume.local_materialization_dir.clone()
                    },
                );
                effective_env.insert(
                    format!("SORACLOUD_LEASE_VOLUME_{env_suffix}_MOUNT_PATH"),
                    volume.mount_path.clone(),
                );
            }
            let local_replicas = if bundle.container.runtime == SoraContainerRuntimeV1::Inrou {
                build_hosted_http_local_replica_plans(
                    &service_dir,
                    &local_inrou_assignments,
                    hosted_http_runtime_state.as_ref(),
                    hydration_complete,
                    hosted_http_lease_active,
                )
            } else {
                Vec::new()
            };
            let hosts_inrou_locally =
                hosted_http_lease_active && !local_inrou_assignments.is_empty();
            let plan = SoracloudRuntimeServicePlan {
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                role,
                traffic_percent,
                runtime: bundle.container.runtime,
                execution_plane: bundle.service.execution_plane,
                bundle_hash: bundle.container.bundle_hash.to_string(),
                bundle_path: bundle.container.bundle_path.clone(),
                entrypoint: bundle.container.entrypoint.clone(),
                inrou: build_inrou_runtime_plan(bundle, local_inrou_assignments.first()),
                bundle_cache_path: bundle_cache_path.display().to_string(),
                bundle_available_locally: bundle_cache_path.exists(),
                process_generation: match bundle.container.runtime {
                    iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm => {
                        is_runtime_active.then_some(deployment.process_generation)
                    }
                    SoraContainerRuntimeV1::Inrou => {
                        hosts_inrou_locally.then_some(deployment.process_generation)
                    }
                },
                desired_replica_count: bundle.service.replicas.get(),
                local_replica_slots: local_replicas
                    .iter()
                    .map(|replica| replica.replica_slot)
                    .collect(),
                local_replicas,
                health_status: if !hydration_complete {
                    SoraServiceHealthStatusV1::Hydrating
                } else {
                    match bundle.container.runtime {
                        iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm => {
                            hydrated_ivm_service_health_status(
                                active_runtime_state.copied(),
                                bundle.service.execution_plane,
                                bundle.container.runtime,
                                &service_name,
                                &service_version,
                            )
                        }
                        SoraContainerRuntimeV1::Inrou => {
                            if !hosted_http_lease_active {
                                SoraServiceHealthStatusV1::Degraded
                            } else if !hosts_inrou_locally {
                                SoraServiceHealthStatusV1::Unavailable
                            } else {
                                hosted_http_runtime_state
                                    .as_ref()
                                    .filter(|state| {
                                        state.process_generation == deployment.process_generation
                                    })
                                    .map_or(SoraServiceHealthStatusV1::Degraded, |state| {
                                        state.health_status
                                    })
                            }
                        }
                    }
                },
                load_factor_bps: active_runtime_state.map_or(0, |state| state.load_factor_bps),
                reported_pending_mailbox_messages: active_runtime_state
                    .map_or(0, |state| state.pending_mailbox_message_count),
                authoritative_pending_mailbox_messages: authoritative_pending
                    .get(&service_name)
                    .copied()
                    .unwrap_or_default(),
                rollout_handle: deployment
                    .active_rollout
                    .as_ref()
                    .map(|rollout| rollout.rollout_handle.clone()),
                config_generation: deployment.config_generation,
                secret_generation: deployment.secret_generation,
                quota_class: deployment
                    .service_lease
                    .as_ref()
                    .map(|lease| lease.quota_class.clone()),
                service_lease_status: deployment.hosted_service_lease_status_at(current_sequence),
                lease_expires_sequence: deployment
                    .service_lease
                    .as_ref()
                    .map(|lease| lease.lease_expires_sequence),
                remaining_runtime_balance_nanos: deployment
                    .hosted_service_remaining_balance_nanos(current_sequence),
                config_entry_count: u32::try_from(deployment.service_configs.len())
                    .unwrap_or(u32::MAX),
                secret_entry_count: u32::try_from(deployment.service_secrets.len())
                    .unwrap_or(u32::MAX),
                config_exports: bundle.container.config_exports.clone(),
                supports_host_read_config: true,
                supports_host_read_secret_envelope: true,
                supports_private_secret_payload_reads,
                materialization_dir: service_dir.display().to_string(),
                config_materialization_dir: config_dir.display().to_string(),
                effective_env,
                effective_env_materialization_path: effective_env_path.display().to_string(),
                config_exports_materialization_dir: config_exports_dir.display().to_string(),
                secret_envelopes_materialization_dir: secret_envelopes_dir.display().to_string(),
                secret_payload_materialization_dir: secret_payload_dir.display().to_string(),
                lease_volumes,
                mailboxes: bundle
                    .service
                    .handlers
                    .iter()
                    .filter_map(|handler| {
                        handler
                            .mailbox
                            .as_ref()
                            .map(|mailbox| SoracloudRuntimeMailboxPlan {
                                handler_name: handler.handler_name.to_string(),
                                queue_name: mailbox.queue_name.to_string(),
                                max_pending_messages: mailbox.max_pending_messages.get(),
                                max_message_bytes: mailbox.max_message_bytes.get(),
                                retention_blocks: mailbox.retention_blocks.get(),
                            })
                    })
                    .collect(),
                artifacts: artifact_plans,
            };
            version_plans.insert(service_version, plan);
        }
        services.insert(service_name, version_plans);
    }

    let apartments = world
        .soracloud_agent_apartments()
        .iter()
        .map(|(apartment_name, record)| {
            (
                apartment_name.clone(),
                build_apartment_plan(apartment_name, record, state_dir),
            )
        })
        .collect();
    let hf_sources = build_hf_source_plans(world, &services, &apartments, state_dir);

    Ok(SoracloudRuntimeSnapshot {
        schema_version: SoracloudRuntimeSnapshot::default().schema_version,
        observed_height: u64::try_from(view.height()).unwrap_or(u64::MAX),
        observed_block_hash: view.latest_block_hash().map(|hash| hash.to_string()),
        local_peer_id: local_peer_id.map(ToOwned::to_owned),
        services,
        apartments,
        hf_sources,
    })
}

fn build_hf_source_plans(
    world: &impl WorldReadOnly,
    services: &BTreeMap<String, BTreeMap<String, SoracloudRuntimeServicePlan>>,
    apartments: &BTreeMap<String, SoracloudRuntimeApartmentPlan>,
    state_dir: &Path,
) -> BTreeMap<String, SoracloudRuntimeHfSourcePlan> {
    let mut plans = BTreeMap::new();

    for (source_id, source) in world.soracloud_hf_sources().iter() {
        let source_id_string = source_id.to_string();
        let import_manifest = match read_hf_import_manifest(state_dir, &source_id_string) {
            Ok(manifest) => manifest,
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    source_id = %source_id_string,
                    "failed to read local HF import manifest while building runtime snapshot"
                );
                None
            }
        };
        let pool_records = world
            .soracloud_hf_shared_lease_pools()
            .iter()
            .filter_map(|(_pool_id, pool)| (pool.source_id == *source_id).then_some(pool))
            .collect::<Vec<_>>();

        let pool_count = u32::try_from(pool_records.len()).unwrap_or(u32::MAX);
        let active_pool_count = u32::try_from(
            pool_records
                .iter()
                .filter(|pool| {
                    matches!(
                        pool.status,
                        SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
                    )
                })
                .count(),
        )
        .unwrap_or(u32::MAX);
        let mut active_member_count = 0_u32;
        let mut queued_window_count = 0_u32;
        let mut bound_service_names = BTreeSet::new();
        let mut bound_apartment_names = BTreeSet::new();

        for pool in &pool_records {
            active_member_count = active_member_count.saturating_add(pool.active_member_count);
            if let Some(next_window) = pool.queued_next_window.as_ref() {
                queued_window_count = queued_window_count.saturating_add(1);
                bound_service_names.insert(next_window.service_name.to_string());
                if let Some(apartment_name) = next_window.apartment_name.as_ref() {
                    bound_apartment_names.insert(apartment_name.to_string());
                }
            }

            let pool_key = pool.pool_id.to_string();
            for ((member_pool_id, _account_id), member) in
                world.soracloud_hf_shared_lease_members().iter()
            {
                if member_pool_id != &pool_key
                    || member.status != SoraHfSharedLeaseMemberStatusV1::Active
                {
                    continue;
                }
                bound_service_names.extend(member.service_bindings.iter().cloned());
                bound_apartment_names.extend(member.apartment_bindings.iter().cloned());
            }
        }

        let bound_service_names = bound_service_names.into_iter().collect::<Vec<_>>();
        let bound_apartment_names = bound_apartment_names.into_iter().collect::<Vec<_>>();
        let mut materialized_service_names = Vec::new();
        let mut materialized_apartment_names = Vec::new();
        let mut hydrating_service_count = 0_u32;
        let mut bundle_cache_miss_count = 0_u32;
        let mut artifact_cache_miss_count = 0_u32;

        for service_name in &bound_service_names {
            let Some(version_plans) = services.get(service_name) else {
                continue;
            };
            materialized_service_names.push(service_name.clone());

            let mut service_hydrating = false;
            for plan in version_plans.values() {
                if !plan.bundle_available_locally {
                    service_hydrating = true;
                    bundle_cache_miss_count = bundle_cache_miss_count.saturating_add(1);
                }
                for artifact in &plan.artifacts {
                    if artifact.available_locally {
                        continue;
                    }
                    service_hydrating = true;
                    artifact_cache_miss_count = artifact_cache_miss_count.saturating_add(1);
                }
            }
            if service_hydrating {
                hydrating_service_count = hydrating_service_count.saturating_add(1);
            }
        }

        for apartment_name in &bound_apartment_names {
            if apartments.contains_key(apartment_name) {
                materialized_apartment_names.push(apartment_name.clone());
            }
        }

        let bound_service_count = u32::try_from(bound_service_names.len()).unwrap_or(u32::MAX);
        let materialized_service_count =
            u32::try_from(materialized_service_names.len()).unwrap_or(u32::MAX);
        let bound_apartment_count = u32::try_from(bound_apartment_names.len()).unwrap_or(u32::MAX);
        let materialized_apartment_count =
            u32::try_from(materialized_apartment_names.len()).unwrap_or(u32::MAX);
        let import_complete = import_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.import_error.is_none());
        let import_failed = import_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.import_error.is_some());

        let runtime_status = derive_hf_runtime_status(
            source.status,
            import_complete,
            import_failed,
            bound_service_count,
            materialized_service_count,
            hydrating_service_count,
            bound_apartment_count,
            materialized_apartment_count,
            bundle_cache_miss_count,
            artifact_cache_miss_count,
        );

        plans.insert(
            source_id_string,
            SoracloudRuntimeHfSourcePlan {
                source_id: source_id.to_string(),
                repo_id: source.repo_id.clone(),
                resolved_revision: source.resolved_revision.clone(),
                model_name: source.model_name.clone(),
                adapter_id: source.adapter_id.clone(),
                authoritative_status: source.status,
                runtime_status,
                pool_count,
                active_pool_count,
                active_member_count,
                queued_window_count,
                bound_service_count,
                bound_service_names,
                materialized_service_count,
                materialized_service_names,
                hydrating_service_count,
                bound_apartment_count,
                bound_apartment_names,
                materialized_apartment_count,
                materialized_apartment_names,
                bundle_cache_miss_count,
                artifact_cache_miss_count,
                last_error: import_manifest
                    .as_ref()
                    .and_then(|manifest| manifest.import_error.clone())
                    .or_else(|| source.last_error.clone()),
            },
        );
    }

    plans
}

fn derive_hf_runtime_status(
    authoritative_status: SoraHfSourceStatusV1,
    import_complete: bool,
    import_failed: bool,
    bound_service_count: u32,
    materialized_service_count: u32,
    hydrating_service_count: u32,
    bound_apartment_count: u32,
    materialized_apartment_count: u32,
    bundle_cache_miss_count: u32,
    artifact_cache_miss_count: u32,
) -> SoracloudRuntimeHfSourceStatus {
    match authoritative_status {
        SoraHfSourceStatusV1::Failed => SoracloudRuntimeHfSourceStatus::Failed,
        SoraHfSourceStatusV1::Retired => SoracloudRuntimeHfSourceStatus::Retired,
        SoraHfSourceStatusV1::PendingImport | SoraHfSourceStatusV1::Ready => {
            if import_failed {
                return SoracloudRuntimeHfSourceStatus::Failed;
            }
            let has_runtime_bindings = bound_service_count > 0 || bound_apartment_count > 0;
            let deployment_missing = materialized_service_count < bound_service_count
                || materialized_apartment_count < bound_apartment_count;
            let hydration_missing = hydrating_service_count > 0
                || bundle_cache_miss_count > 0
                || artifact_cache_miss_count > 0;

            if !import_complete {
                SoracloudRuntimeHfSourceStatus::PendingImport
            } else if !has_runtime_bindings {
                if authoritative_status == SoraHfSourceStatusV1::Ready {
                    SoracloudRuntimeHfSourceStatus::Ready
                } else {
                    SoracloudRuntimeHfSourceStatus::PendingDeployment
                }
            } else if deployment_missing {
                SoracloudRuntimeHfSourceStatus::PendingDeployment
            } else if hydration_missing {
                SoracloudRuntimeHfSourceStatus::Hydrating
            } else {
                SoracloudRuntimeHfSourceStatus::Ready
            }
        }
    }
}

fn build_apartment_plan(
    apartment_name: &str,
    record: &SoraAgentApartmentRecordV1,
    state_dir: &Path,
) -> SoracloudRuntimeApartmentPlan {
    let apartment_root = state_dir
        .join("apartments")
        .join(sanitize_path_component(apartment_name));
    SoracloudRuntimeApartmentPlan {
        apartment_name: apartment_name.to_string(),
        manifest_hash: record.manifest_hash.to_string(),
        status: record.status,
        process_generation: record.process_generation,
        lease_expires_sequence: record.lease_expires_sequence,
        last_active_sequence: record.last_active_sequence,
        materialization_dir: apartment_root.display().to_string(),
        pending_wallet_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        pending_mailbox_message_count: u32::try_from(record.mailbox_queue.len())
            .unwrap_or(u32::MAX),
        autonomy_budget_remaining_units: record.autonomy_budget_remaining_units,
        approved_artifact_count: u32::try_from(record.artifact_allowlist.len()).unwrap_or(u32::MAX),
        autonomy_run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
    }
}

fn build_artifact_plans(
    bundle: &SoraDeploymentBundleV1,
    artifacts_root: &Path,
) -> Vec<SoracloudRuntimeArtifactPlan> {
    let mut artifacts = Vec::with_capacity(bundle.service.artifacts.len().saturating_add(1));
    let bundle_cache_path = artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
    artifacts.push(SoracloudRuntimeArtifactPlan {
        kind: SoraArtifactKindV1::Bundle,
        artifact_hash: bundle.container.bundle_hash.to_string(),
        artifact_path: bundle.container.bundle_path.clone(),
        handler_name: None,
        local_cache_path: bundle_cache_path.display().to_string(),
        available_locally: bundle_cache_path.exists(),
    });
    artifacts.extend(bundle.service.artifacts.iter().map(|artifact| {
        let cache_path = artifacts_root.join(hash_cache_name(artifact.artifact_hash));
        SoracloudRuntimeArtifactPlan {
            kind: artifact.kind,
            artifact_hash: artifact.artifact_hash.to_string(),
            artifact_path: artifact.artifact_path.clone(),
            handler_name: artifact.handler_name.as_ref().map(ToString::to_string),
            local_cache_path: cache_path.display().to_string(),
            available_locally: cache_path.exists(),
        }
    }));
    artifacts
}

fn collect_service_revision_registry(
    view: &StateView<'_>,
) -> BTreeMap<(String, String), SoraDeploymentBundleV1> {
    view.world()
        .soracloud_service_revisions()
        .iter()
        .map(|((service_name, service_version), bundle)| {
            (
                (service_name.clone(), service_version.clone()),
                bundle.clone(),
            )
        })
        .collect()
}

fn collect_active_versions(
    deployment: &SoraServiceDeploymentStateV1,
) -> Vec<(String, SoracloudRuntimeRevisionRole, u8)> {
    let mut versions = Vec::new();
    if let Some(rollout) = deployment.active_rollout.as_ref() {
        let traffic_percent = rollout.traffic_percent.min(100);
        let baseline_percent = 100u8.saturating_sub(traffic_percent);
        versions.push((
            deployment.current_service_version.clone(),
            SoracloudRuntimeRevisionRole::Active,
            baseline_percent,
        ));
        if rollout.candidate_version != deployment.current_service_version {
            versions.push((
                rollout.candidate_version.clone(),
                SoracloudRuntimeRevisionRole::CanaryCandidate,
                traffic_percent,
            ));
        }
    } else {
        versions.push((
            deployment.current_service_version.clone(),
            SoracloudRuntimeRevisionRole::Active,
            100,
        ));
    }
    versions
}

fn hydrated_ivm_service_health_status(
    runtime_state: Option<&SoraServiceRuntimeStateV1>,
    execution_plane: iroha_data_model::soracloud::SoraServiceExecutionPlaneV1,
    runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1,
    service_name: &str,
    service_version: &str,
) -> SoraServiceHealthStatusV1 {
    if let Some(state) = runtime_state {
        return state.health_status;
    }
    if ensure_ivm_runtime(execution_plane, runtime, service_name, service_version).is_ok() {
        SoraServiceHealthStatusV1::Healthy
    } else {
        SoraServiceHealthStatusV1::Degraded
    }
}

fn authoritative_mailbox_counts(
    messages: &impl StorageReadOnly<Hash, SoraServiceMailboxMessageV1>,
    receipts: &impl StorageReadOnly<Hash, SoraRuntimeReceiptV1>,
) -> BTreeMap<String, u32> {
    let consumed: BTreeSet<Hash> = receipts
        .iter()
        .filter_map(|(_receipt_id, receipt)| receipt.mailbox_message_id)
        .collect();
    let mut counts = BTreeMap::new();
    for (_, message) in messages.iter() {
        if consumed.contains(&message.message_id) {
            continue;
        }
        let entry = counts.entry(message.to_service.to_string()).or_insert(0u32);
        *entry = entry.saturating_add(1);
    }
    counts
}

fn collect_committed_service_state_entries(
    view: &StateView<'_>,
    service_name: &str,
) -> BTreeMap<(String, String), SoraServiceStateEntryV1> {
    view.world()
        .soracloud_service_state_entries()
        .iter()
        .filter(|((_service, _binding, _key), entry)| entry.service_name.as_ref() == service_name)
        .map(|((_service, binding, key), entry)| ((binding.clone(), key.clone()), entry.clone()))
        .collect()
}

fn deterministic_mailbox_failure_result(
    request: SoracloudOrderedMailboxExecutionRequest,
    outcome_label: &str,
    health_status: SoraServiceHealthStatusV1,
) -> SoracloudOrderedMailboxExecutionResult {
    deterministic_mailbox_failure_result_with_message(
        request,
        outcome_label,
        outcome_label.to_owned(),
        health_status,
    )
}

fn deterministic_mailbox_failure_result_with_message(
    request: SoracloudOrderedMailboxExecutionRequest,
    outcome_label: &str,
    detail: String,
    health_status: SoraServiceHealthStatusV1,
) -> SoracloudOrderedMailboxExecutionResult {
    let result_commitment = Hash::new(Encode::encode(&(
        "soracloud:runtime-failure:v1",
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
        request.mailbox_message.to_handler.as_ref(),
        request.execution_sequence,
        outcome_label,
        detail,
    )));
    let receipt_id = mailbox_receipt_id(
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        &request.deployment.current_service_version,
        request.execution_sequence,
        outcome_label,
    );
    SoracloudOrderedMailboxExecutionResult {
        state_mutations: Vec::new(),
        outbound_mailbox_messages: Vec::new(),
        response_bytes: Vec::new(),
        content_type: None,
        runtime_state: Some(updated_runtime_state_with_outbound_mailbox(
            request.runtime_state.clone(),
            &request,
            health_status,
            &[],
        )),
        runtime_receipt: SoraRuntimeReceiptV1 {
            schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id,
            service_name: request.deployment.service_name,
            service_version: request.deployment.current_service_version,
            handler_name: request.mailbox_message.to_handler.clone(),
            handler_class: request
                .handler
                .as_ref()
                .map(|handler| handler.class)
                .unwrap_or(SoraServiceHandlerClassV1::Update),
            request_commitment: request.mailbox_message.payload_commitment,
            result_commitment,
            certified_by: SoraCertifiedResponsePolicyV1::None,
            emitted_sequence: request.execution_sequence,
            mailbox_message_id: Some(request.mailbox_message.message_id),
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            placement_id: None,
            selected_validator_account_id: None,
            selected_peer_id: None,
        },
    }
}

fn mailbox_receipt_id(
    message_id: Hash,
    service_name: &str,
    service_version: &str,
    execution_sequence: u64,
    outcome_label: &str,
) -> Hash {
    Hash::new(
        format!(
            "soracloud:runtime-receipt:{message_id}:{service_name}:{service_version}:{execution_sequence}:{outcome_label}"
        )
        .as_bytes(),
    )
}

fn synthetic_runtime_state(
    request: &SoracloudOrderedMailboxExecutionRequest,
    health_status: SoraServiceHealthStatusV1,
) -> iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
    iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        service_name: request.deployment.service_name.clone(),
        active_service_version: request.deployment.current_service_version.clone(),
        health_status,
        load_factor_bps: 0,
        materialized_bundle_hash: request.bundle.container.bundle_hash,
        rollout_handle: request
            .deployment
            .active_rollout
            .as_ref()
            .map(|rollout| rollout.rollout_handle.clone()),
        pending_mailbox_message_count: request.authoritative_pending_mailbox_messages,
        last_receipt_id: None,
    }
}

fn updated_runtime_state_with_outbound_mailbox(
    runtime_state: Option<iroha_data_model::soracloud::SoraServiceRuntimeStateV1>,
    request: &SoracloudOrderedMailboxExecutionRequest,
    health_status: SoraServiceHealthStatusV1,
    outbound_mailbox_messages: &[SoraServiceMailboxMessageV1],
) -> iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
    let mut runtime_state =
        runtime_state.unwrap_or_else(|| synthetic_runtime_state(request, health_status));
    let self_requeued = outbound_mailbox_messages
        .iter()
        .filter(|message| message.to_service == request.deployment.service_name)
        .count();
    runtime_state.health_status = health_status;
    runtime_state.pending_mailbox_message_count = request
        .authoritative_pending_mailbox_messages
        .saturating_sub(1)
        .saturating_add(u32::try_from(self_requeued).unwrap_or(u32::MAX));
    runtime_state
}

fn ensure_ivm_runtime(
    execution_plane: iroha_data_model::soracloud::SoraServiceExecutionPlaneV1,
    runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1,
    service_name: &str,
    service_version: &str,
) -> Result<(), String> {
    if execution_plane
        != iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService
    {
        return Err(format!(
            "service `{service_name}` revision `{service_version}` targets unsupported Soracloud execution plane `{:?}`; deterministic local reads and mailbox execution require `DeterministicService`",
            execution_plane
        ));
    }
    match runtime {
        iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm => Ok(()),
        other => Err(format!(
            "service `{service_name}` revision `{service_version}` targets unsupported Soracloud runtime `{:?}`; deterministic local reads and mailbox execution require `Ivm`",
            other
        )),
    }
}

fn authoritative_mailbox_result_commitment(
    request: &SoracloudOrderedMailboxExecutionRequest,
    state_mutations: &[iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation],
    outbound_mailbox_messages: &[SoraServiceMailboxMessageV1],
    response_bytes: &[u8],
    content_type: Option<&str>,
    runtime_state: &SoraServiceRuntimeStateV1,
    journal_artifact_hash: Option<Hash>,
    checkpoint_artifact_hash: Option<Hash>,
) -> Hash {
    let mutation_fingerprints = state_mutations
        .iter()
        .map(|mutation| {
            (
                mutation.binding_name.as_str(),
                mutation.state_key.as_str(),
                mutation.operation,
                mutation.encryption,
                mutation.payload_bytes,
                mutation.payload_commitment,
            )
        })
        .collect::<Vec<_>>();
    let outbound_fingerprints = outbound_mailbox_messages
        .iter()
        .map(|message| {
            (
                message.message_id,
                message.from_service.as_ref(),
                message.from_handler.as_ref(),
                message.to_service.as_ref(),
                message.to_handler.as_ref(),
                message.payload_commitment,
                message.available_after_sequence,
                message.expires_at_sequence,
            )
        })
        .collect::<Vec<_>>();
    let response_fingerprint = (
        content_type,
        Hash::new(response_bytes),
        runtime_state.clone(),
        journal_artifact_hash,
        checkpoint_artifact_hash,
    );
    Hash::new(Encode::encode(&(
        "soracloud:runtime-result:v1",
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
        request.mailbox_message.to_handler.as_ref(),
        request.execution_sequence,
        mutation_fingerprints,
        outbound_fingerprints,
        response_fingerprint,
    )))
}

fn mailbox_payload_tlv_bytes(payload_bytes: &[u8]) -> Result<Vec<u8>, VMError> {
    if payload_bytes.is_empty() {
        return Ok(make_pointer_tlv(PointerType::Blob, &[]));
    }
    if ivm::pointer_abi::validate_tlv_bytes(payload_bytes).is_ok() {
        return Ok(payload_bytes.to_vec());
    }
    Ok(make_pointer_tlv(PointerType::Blob, payload_bytes))
}

fn make_pointer_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(u32::try_from(payload.len()).unwrap_or(u32::MAX)).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}

fn vm_error_label(error: &VMError) -> &'static str {
    match error.as_unmetered() {
        VMError::OutOfGas => "out_of_gas",
        VMError::OutOfMemory => "out_of_memory",
        VMError::MemoryAccessViolation { .. } => "memory_access_violation",
        VMError::MisalignedAccess { .. } => "misaligned_access",
        VMError::MemoryOutOfBounds => "memory_out_of_bounds",
        VMError::UnalignedAccess => "unaligned_access",
        VMError::MemoryPermissionDenied => "memory_permission_denied",
        VMError::DecodeError => "decode_error",
        VMError::InvalidOpcode(_) => "invalid_opcode",
        VMError::UnknownSyscall(_) => "unknown_syscall",
        VMError::HostUnavailable => "host_unavailable",
        VMError::NotImplemented { .. } => "not_implemented",
        VMError::AssertionFailed => "assertion_failed",
        VMError::ExceededMaxCycles => "exceeded_max_cycles",
        VMError::InvalidMetadata => "invalid_metadata",
        VMError::InvalidVectorLength { .. } => "invalid_vector_length",
        VMError::MissingHalt => "missing_halt",
        VMError::VectorExtensionDisabled => "vector_disabled",
        VMError::ZkExtensionDisabled => "zk_disabled",
        VMError::NullifierAlreadyUsed => "nullifier_used",
        VMError::PermissionDenied => "permission_denied",
        VMError::PrivacyViolation => "privacy_violation",
        VMError::RegisterOutOfBounds => "register_out_of_bounds",
        VMError::HTMAbort => "htm_abort",
        VMError::NoritoInvalid => "norito_invalid",
        VMError::AbiTypeNotAllowed { .. } => "abi_type_not_allowed",
        VMError::AmxBudgetExceeded { .. } => "amx_budget_exceeded",
        VMError::Metered { .. } => unreachable!("as_unmetered peels metered wrappers"),
    }
}

fn persist_staged_runtime_artifact(
    root: PathBuf,
    artifact: Option<&StagedRuntimeArtifact>,
) -> Result<Option<Hash>, SoracloudRuntimeExecutionError> {
    let Some(artifact) = artifact else {
        return Ok(None);
    };
    fs::create_dir_all(&root).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "create Soracloud runtime artifact root {}: {error}",
                root.display()
            ),
        )
    })?;
    let path = root.join(hash_cache_name(artifact.artifact_hash));
    fs::write(&path, &artifact.bytes).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "persist Soracloud runtime artifact `{}` at {}: {error}",
                artifact.artifact_path,
                path.display()
            ),
        )
    })?;
    Ok(Some(artifact.artifact_hash))
}

fn sanitized_relative_material_path(key: &str) -> Result<PathBuf, VMError> {
    if key.trim().is_empty() {
        return Err(VMError::PermissionDenied);
    }
    let mut path = PathBuf::new();
    for component in key.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(VMError::PermissionDenied);
        }
        path.push(sanitize_path_component(component));
    }
    Ok(path)
}

fn url_host_port(url: &str) -> Option<(String, u16)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    Some((
        parsed.host_str()?.to_owned(),
        parsed.port_or_known_default()?,
    ))
}

fn hash_cache_name(hash: Hash) -> String {
    sanitize_path_component(&hash.to_string())
}

fn normalize_provider_base_url(raw: &str) -> Option<reqwest::Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('*') {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme).ok()?;
    let normalized_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => format!("{path}/"),
    };
    url.set_path(&normalized_path);
    Some(url)
}

fn normalize_hf_base_url(raw: &str) -> eyre::Result<reqwest::Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        eyre::bail!("empty Hugging Face base URL");
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme).wrap_err("parse Hugging Face base URL")?;
    let normalized_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => path.to_owned(),
    };
    url.set_path(&normalized_path);
    Ok(url)
}

fn hf_model_info_url(
    api_base_url: &str,
    repo_id: &str,
    requested_revision: &str,
) -> eyre::Result<reqwest::Url> {
    let mut url = normalize_hf_base_url(api_base_url)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre::eyre!("Hugging Face API base URL cannot be a base"))?;
        for component in ["models"]
            .into_iter()
            .chain(repo_id.split('/'))
            .chain(["revision", requested_revision].into_iter())
        {
            segments.push(component);
        }
    }
    Ok(url)
}

fn hf_repo_file_url(
    hub_base_url: &str,
    repo_id: &str,
    requested_revision: &str,
    file_path: &str,
) -> eyre::Result<reqwest::Url> {
    let mut url = normalize_hf_base_url(hub_base_url)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre::eyre!("Hugging Face Hub base URL cannot be a base"))?;
        for component in repo_id
            .split('/')
            .chain(["resolve", requested_revision].into_iter())
            .chain(file_path.split('/'))
        {
            segments.push(component);
        }
    }
    Ok(url)
}

fn hf_inference_url(inference_base_url: &str, repo_id: &str) -> eyre::Result<reqwest::Url> {
    let mut url = normalize_hf_base_url(inference_base_url)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre::eyre!("HF inference base URL cannot be a base"))?;
        for component in repo_id.split('/') {
            segments.push(component);
        }
    }
    Ok(url)
}

fn hf_import_file_selected(path: &str, allowlist: &[String]) -> bool {
    let normalized_path = path.trim().to_ascii_lowercase();
    allowlist.iter().any(|pattern| {
        if let Some(suffix) = pattern.strip_prefix("*.") {
            normalized_path.ends_with(&format!(".{suffix}"))
        } else {
            normalized_path == *pattern
        }
    })
}

fn read_hf_import_manifest(
    state_dir: &Path,
    source_id: &str,
) -> io::Result<Option<HfLocalImportManifestV1>> {
    let path = hf_local_import_manifest_path(state_dir, source_id);
    read_json_optional(&path)
}

fn hf_local_source_root(state_dir: &Path, source_id: &str) -> PathBuf {
    state_dir
        .join("hf_sources")
        .join(sanitize_path_component(source_id))
}

fn hf_local_source_files_root(state_dir: &Path, source_id: &str) -> PathBuf {
    hf_local_source_root(state_dir, source_id).join("files")
}

fn hf_local_import_manifest_path(state_dir: &Path, source_id: &str) -> PathBuf {
    hf_local_source_root(state_dir, source_id).join("import_manifest.json")
}

fn hf_local_runner_script_path(state_dir: &Path) -> PathBuf {
    state_dir
        .join("hf_runtime")
        .join("soracloud_hf_local_runner.py")
}

fn hf_local_runner_stderr_log_path(state_dir: &Path, source_id: &str) -> PathBuf {
    state_dir
        .join("hf_runtime")
        .join("workers")
        .join(format!("{}.stderr.log", sanitize_path_component(source_id)))
}

fn ensure_hf_local_runner_script(state_dir: &Path) -> io::Result<PathBuf> {
    let path = hf_local_runner_script_path(state_dir);
    match fs::read_to_string(&path) {
        Ok(current) if current == HF_LOCAL_RUNNER_SCRIPT_V1 => Ok(path),
        Ok(_) | Err(_) => {
            write_bytes_atomic(&path, HF_LOCAL_RUNNER_SCRIPT_V1.as_bytes())?;
            Ok(path)
        }
    }
}

fn execute_hf_local_runner_request(
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    cache_key: HfLocalRunnerWorkerCacheKey,
    timeout: Duration,
    request_payload: &[u8],
) -> Result<Vec<u8>, SoracloudRuntimeExecutionError> {
    for attempt in 0..2 {
        let worker = ensure_hf_local_runner_worker(hf_local_workers, &cache_key)?;
        let mut worker_guard = worker.lock();
        match worker_guard.request(timeout, request_payload) {
            Ok(output) => return Ok(output),
            Err(error) => {
                worker_guard.stop();
                drop(worker_guard);
                remove_hf_local_runner_worker_if_same(
                    hf_local_workers,
                    &cache_key.source_id,
                    &worker,
                );
                if attempt == 0 {
                    continue;
                }
                return Err(error);
            }
        }
    }
    unreachable!("resident HF local runner retries are bounded")
}

fn ensure_hf_local_runner_worker(
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    cache_key: &HfLocalRunnerWorkerCacheKey,
) -> Result<Arc<Mutex<HfLocalRunnerWorker>>, SoracloudRuntimeExecutionError> {
    loop {
        let existing = {
            let workers = hf_local_workers.lock();
            workers.get(&cache_key.source_id).cloned()
        };
        if let Some(existing) = existing {
            let mut worker = existing.lock();
            let is_compatible = worker.cache_key == *cache_key;
            let is_running = worker.is_running()?;
            drop(worker);
            if is_compatible && is_running {
                return Ok(existing);
            }
            remove_hf_local_runner_worker_if_same(
                hf_local_workers,
                &cache_key.source_id,
                &existing,
            );
            let mut worker = existing.lock();
            worker.stop();
            drop(worker);
            continue;
        }

        let candidate = Arc::new(Mutex::new(HfLocalRunnerWorker::spawn(cache_key.clone())?));
        let mut workers = hf_local_workers.lock();
        match workers.entry(cache_key.source_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Arc::clone(&candidate));
                return Ok(candidate);
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                drop(workers);
                candidate.lock().stop();
            }
        }
    }
}

fn remove_hf_local_runner_worker_if_same(
    hf_local_workers: &SharedHfLocalRunnerWorkers,
    source_id: &str,
    worker: &Arc<Mutex<HfLocalRunnerWorker>>,
) {
    let mut workers = hf_local_workers.lock();
    let should_remove = workers
        .get(source_id)
        .is_some_and(|current| Arc::ptr_eq(current, worker));
    if should_remove {
        workers.remove(source_id);
    }
}

fn stderr_log_excerpt(path: &Path) -> String {
    let Ok(contents) = fs::read_to_string(path) else {
        return String::new();
    };
    let mut tail = contents
        .lines()
        .rev()
        .take(6)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if tail.is_empty() {
        return String::new();
    }
    tail.reverse();
    tail.join(" | ")
}

impl HfLocalRunnerWorker {
    fn spawn(
        cache_key: HfLocalRunnerWorkerCacheKey,
    ) -> Result<Self, SoracloudRuntimeExecutionError> {
        let program = cache_key.runner_program.trim();
        if program.is_empty() {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "generated HF local execution requires a non-empty `soracloud_runtime.hf.local_runner_program`",
            ));
        }

        let state_dir = cache_key
            .runner_script_path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "local HF runner script path `{}` must live under the runtime state directory",
                        cache_key.runner_script_path.display()
                    ),
                )
            })?;
        let stderr_log_path = hf_local_runner_stderr_log_path(state_dir, &cache_key.source_id);
        if let Some(parent) = stderr_log_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "create resident HF worker log directory `{}`: {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        let stderr_log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log_path)
            .map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "open resident HF worker stderr log `{}`: {error}",
                        stderr_log_path.display()
                    ),
                )
            })?;

        let mut child = Command::new(program)
            .arg(&cache_key.runner_script_path)
            .arg("--server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr_log))
            .spawn()
            .map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "spawn resident HF local runner `{program}` for script {}: {error}",
                        cache_key.runner_script_path.display()
                    ),
                )
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                "resident HF local runner stdin pipe is unavailable",
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                "resident HF local runner stdout pipe is unavailable",
            )
        })?;
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let source_id = cache_key.source_id.clone();
        let stdout_reader = thread::Builder::new()
            .name(format!("hf-runner-{}", sanitize_path_component(&source_id)))
            .spawn(move || {
                use io::BufRead as _;

                let mut reader = io::BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let payload = line.trim_end_matches(['\r', '\n']).as_bytes().to_vec();
                            if stdout_tx.send(Ok(payload)).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = stdout_tx.send(Err(error));
                            break;
                        }
                    }
                }
            })
            .map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Internal,
                    format!(
                        "spawn resident HF local runner stdout reader for source `{}`: {error}",
                        cache_key.source_id
                    ),
                )
            })?;

        Ok(Self {
            cache_key,
            child,
            stdin,
            stdout_rx,
            stdout_reader: Some(stdout_reader),
            stderr_log_path,
        })
    }

    fn is_running(&mut self) -> Result<bool, SoracloudRuntimeExecutionError> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "poll resident HF local runner for source `{}`: {error}",
                        self.cache_key.source_id
                    ),
                )
            })
    }

    fn request(
        &mut self,
        timeout: Duration,
        request_payload: &[u8],
    ) -> Result<Vec<u8>, SoracloudRuntimeExecutionError> {
        if !self.is_running()? {
            let stderr = stderr_log_excerpt(&self.stderr_log_path);
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "resident HF local runner for source `{}` is no longer running{}",
                    self.cache_key.source_id,
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(": {stderr}")
                    }
                ),
            ));
        }

        {
            use io::Write as _;

            self.stdin.write_all(request_payload).map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "write resident HF local runner request for source `{}`: {error}",
                        self.cache_key.source_id
                    ),
                )
            })?;
            self.stdin.write_all(b"\n").map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "frame resident HF local runner request for source `{}`: {error}",
                        self.cache_key.source_id
                    ),
                )
            })?;
            self.stdin.flush().map_err(|error| {
                SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "flush resident HF local runner request for source `{}`: {error}",
                        self.cache_key.source_id
                    ),
                )
            })?;
        }

        match self.stdout_rx.recv_timeout(timeout) {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(error)) => Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "read resident HF local runner response for source `{}`: {error}",
                    self.cache_key.source_id
                ),
            )),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "resident HF local runner for source `{}` exceeded timeout of {} ms",
                    self.cache_key.source_id,
                    timeout.as_millis()
                ),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let stderr = stderr_log_excerpt(&self.stderr_log_path);
                Err(SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    format!(
                        "resident HF local runner for source `{}` exited before returning a response{}",
                        self.cache_key.source_id,
                        if stderr.is_empty() {
                            String::new()
                        } else {
                            format!(": {stderr}")
                        }
                    ),
                ))
            }
        }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(stdout_reader) = self.stdout_reader.take() {
            let _ = stdout_reader.join();
        }
    }
}

impl Drop for HfLocalRunnerWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

impl HostedHttpWorkerAttachment {
    fn cleanup(&mut self) {
        match self {
            Self::FirecrackerKvm(attachment) => attachment.cleanup(),
            Self::PortableVm(attachment) => attachment.cleanup(),
        }
    }

    fn accounted_egress_bytes(&self) -> io::Result<Option<u64>> {
        match self {
            Self::FirecrackerKvm(attachment) => attachment.accounted_egress_bytes().map(Some),
            Self::PortableVm(attachment) => attachment.accounted_egress_bytes(),
        }
    }
}

impl PortableVmAttachment {
    fn cleanup(&mut self) {
        if let Some(mut metadata_server) = self.metadata_server.take() {
            metadata_server.cleanup();
        }
    }

    fn accounted_egress_bytes(&self) -> io::Result<Option<u64>> {
        Ok(None)
    }
}

impl PortableVmMetadataServer {
    fn datasource_base_url(&self) -> String {
        portable_vm_metadata_base_url(self.bind_addr.port())
    }

    fn cleanup(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let _ = std::net::TcpStream::connect(self.bind_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl HostedHttpWorker {
    fn new(
        cache_key: HostedHttpWorkerCacheKey,
        child: std::process::Child,
        listen_base_url: String,
        egress_accounting_offset_bytes: u64,
        attachment: Option<HostedHttpWorkerAttachment>,
        stderr_log_path: PathBuf,
    ) -> Self {
        Self {
            cache_key,
            child,
            listen_base_url,
            egress_accounting_offset_bytes,
            attachment,
            stderr_log_path,
        }
    }

    fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }

    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    fn accounted_egress_bytes(&self) -> Option<u64> {
        if self.cache_key.runtime != SoraContainerRuntimeV1::Inrou {
            return None;
        }
        self.attachment
            .as_ref()
            .and_then(|attachment| attachment.accounted_egress_bytes().ok().flatten())
            .map(|tx_bytes| self.egress_accounting_offset_bytes.saturating_add(tx_bytes))
    }

    fn stop(&mut self) {
        let _ = &self.stderr_log_path;
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(mut attachment) = self.attachment.take() {
            attachment.cleanup();
        }
    }
}

impl Drop for HostedHttpWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn hosted_http_runtime_state_path(materialization_dir: &Path) -> PathBuf {
    materialization_dir.join(SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1)
}

fn write_hosted_http_runtime_state(
    materialization_dir: &Path,
    service_name: &str,
    service_version: &str,
    process_generation: u64,
    health_status: SoraServiceHealthStatusV1,
    listen_base_url: Option<&str>,
    pid: Option<u32>,
    accounted_egress_bytes: u64,
    last_error: Option<String>,
    replicas: Vec<SoracloudHostedHttpReplicaRuntimeStateV1>,
) -> eyre::Result<()> {
    let runtime_state = SoracloudHostedHttpRuntimeStateV1 {
        schema_version: SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1,
        service_name: service_name.to_owned(),
        service_version: service_version.to_owned(),
        process_generation,
        health_status,
        listen_base_url: listen_base_url.map(ToOwned::to_owned),
        pid,
        accounted_egress_bytes,
        replicas,
        last_error,
        updated_at_ms: soracloud_runtime_observed_at_ms(),
    };
    write_hosted_http_runtime_state_document(materialization_dir, &runtime_state)
}

fn read_hosted_http_runtime_state(
    materialization_dir: &Path,
) -> io::Result<Option<SoracloudHostedHttpRuntimeStateV1>> {
    read_json_optional(&hosted_http_runtime_state_path(materialization_dir))
}

fn build_native_service_data_dir(state_dir: &Path, service_name: &str) -> PathBuf {
    state_dir
        .join("service_data")
        .join(sanitize_path_component(service_name))
}

fn write_hosted_http_runtime_state_document(
    materialization_dir: &Path,
    runtime_state: &SoracloudHostedHttpRuntimeStateV1,
) -> eyre::Result<()> {
    write_json_atomic(
        &hosted_http_runtime_state_path(materialization_dir),
        runtime_state,
    )
    .map_err(eyre::Report::from)
}

fn runtime_error_summary(error: &eyre::Report) -> String {
    const MAX_RUNTIME_ERROR_BYTES: usize = 4096;

    let mut parts = Vec::new();
    for cause in error.chain() {
        let text = cause.to_string();
        let text = text.trim();
        if text.is_empty() || parts.iter().any(|existing| existing == text) {
            continue;
        }
        parts.push(text.to_owned());
    }

    let mut summary = if parts.is_empty() {
        error.to_string()
    } else {
        parts.join(": ")
    };
    if summary.len() > MAX_RUNTIME_ERROR_BYTES {
        summary.truncate(MAX_RUNTIME_ERROR_BYTES);
        summary.push_str("...");
    }
    summary
}

fn persist_hosted_http_replica_runtime_state(
    materialization_dir: &Path,
    service_name: &str,
    service_version: &str,
    process_generation: u64,
    replica_slot: u16,
    health_status: SoraServiceHealthStatusV1,
    listen_base_url: Option<&str>,
    pid: Option<u32>,
    accounted_egress_bytes: u64,
    last_error: Option<String>,
) -> eyre::Result<SoracloudHostedHttpReplicaRuntimeStateV1> {
    let updated_at_ms = soracloud_runtime_observed_at_ms();
    let replica_runtime_state = SoracloudHostedHttpReplicaRuntimeStateV1 {
        replica_slot,
        health_status,
        listen_base_url: listen_base_url.map(ToOwned::to_owned),
        pid,
        last_error: last_error.clone(),
        updated_at_ms,
    };
    write_hosted_http_runtime_state_document(
        materialization_dir,
        &SoracloudHostedHttpRuntimeStateV1 {
            schema_version: SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1,
            service_name: service_name.to_owned(),
            service_version: service_version.to_owned(),
            process_generation,
            health_status,
            listen_base_url: listen_base_url.map(ToOwned::to_owned),
            pid,
            accounted_egress_bytes,
            replicas: vec![replica_runtime_state.clone()],
            last_error,
            updated_at_ms,
        },
    )?;
    Ok(replica_runtime_state)
}

fn aggregate_hosted_http_revision_health_status(
    replicas: &[SoracloudHostedHttpReplicaRuntimeStateV1],
) -> SoraServiceHealthStatusV1 {
    if replicas
        .iter()
        .any(|replica| replica.health_status == SoraServiceHealthStatusV1::Healthy)
    {
        return SoraServiceHealthStatusV1::Healthy;
    }
    if replicas
        .iter()
        .any(|replica| replica.health_status == SoraServiceHealthStatusV1::Hydrating)
    {
        return SoraServiceHealthStatusV1::Hydrating;
    }
    if replicas
        .iter()
        .any(|replica| replica.health_status == SoraServiceHealthStatusV1::Degraded)
    {
        return SoraServiceHealthStatusV1::Degraded;
    }
    replicas
        .first()
        .map_or(SoraServiceHealthStatusV1::Degraded, |replica| {
            replica.health_status
        })
}

fn aggregate_hosted_http_revision_listener(
    replicas: &[SoracloudHostedHttpReplicaRuntimeStateV1],
) -> Option<&str> {
    replicas
        .iter()
        .find(|replica| {
            replica.health_status == SoraServiceHealthStatusV1::Healthy
                && replica.listen_base_url.is_some()
        })
        .and_then(|replica| replica.listen_base_url.as_deref())
}

fn aggregate_hosted_http_revision_pid(
    replicas: &[SoracloudHostedHttpReplicaRuntimeStateV1],
) -> Option<u32> {
    replicas
        .iter()
        .find(|replica| {
            replica.health_status == SoraServiceHealthStatusV1::Healthy && replica.pid.is_some()
        })
        .and_then(|replica| replica.pid)
}

fn aggregate_hosted_http_revision_last_error(
    replicas: &[SoracloudHostedHttpReplicaRuntimeStateV1],
) -> Option<String> {
    replicas
        .iter()
        .find_map(|replica| replica.last_error.clone())
}

fn aggregate_hosted_http_revision_accounted_egress_bytes(
    revision_lease_accounting_offset_bytes: u64,
    replica_accounted_egress_bytes: &[u64],
) -> u64 {
    revision_lease_accounting_offset_bytes.saturating_add(
        replica_accounted_egress_bytes
            .iter()
            .map(|accounted| accounted.saturating_sub(revision_lease_accounting_offset_bytes))
            .sum::<u64>(),
    )
}

fn hosted_http_replica_slot_dir_name(replica_slot: u16) -> String {
    format!("replica-{replica_slot:04}")
}

fn hosted_http_replica_materialization_dir(service_dir: &Path, replica_slot: u16) -> PathBuf {
    service_dir
        .join("replicas")
        .join(hosted_http_replica_slot_dir_name(replica_slot))
}

fn build_hosted_http_local_replica_plans(
    service_dir: &Path,
    placements: &[SoraInrouReplicaPlacementV1],
    runtime_state: Option<&SoracloudHostedHttpRuntimeStateV1>,
    hydration_complete: bool,
    hosted_http_lease_active: bool,
) -> Vec<SoracloudRuntimeReplicaPlan> {
    if !hosted_http_lease_active || placements.is_empty() {
        return Vec::new();
    }

    let observed_replicas = runtime_state
        .map(|state| {
            state
                .replicas
                .iter()
                .cloned()
                .map(|replica| (replica.replica_slot, replica))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let default_health = if !hydration_complete {
        SoraServiceHealthStatusV1::Hydrating
    } else {
        SoraServiceHealthStatusV1::Degraded
    };

    placements
        .iter()
        .map(|placement| {
            let replica_slot = placement.replica_slot;
            let materialization_dir =
                hosted_http_replica_materialization_dir(service_dir, replica_slot);
            let observed = observed_replicas.get(&replica_slot);
            SoracloudRuntimeReplicaPlan {
                replica_slot,
                materialization_dir: materialization_dir.display().to_string(),
                health_status: observed.map_or(default_health, |replica| replica.health_status),
                listen_base_url: observed.and_then(|replica| replica.listen_base_url.clone()),
                pid: observed.and_then(|replica| replica.pid),
                last_error: observed.and_then(|replica| replica.last_error.clone()),
            }
        })
        .collect()
}

fn project_hosted_http_replica_plan(
    plan: &SoracloudRuntimeServicePlan,
    replica_slot: u16,
) -> SoracloudRuntimeServicePlan {
    let mut replica_plan = plan.clone();
    let replica_materialization_dir = hosted_http_replica_materialization_dir(
        &PathBuf::from(&plan.materialization_dir),
        replica_slot,
    );
    replica_plan.materialization_dir = replica_materialization_dir.display().to_string();
    replica_plan.local_replica_slots = vec![replica_slot];
    replica_plan.local_replicas = plan
        .local_replicas
        .iter()
        .find(|replica| replica.replica_slot == replica_slot)
        .cloned()
        .map(|replica| vec![replica])
        .unwrap_or_else(|| {
            vec![SoracloudRuntimeReplicaPlan {
                replica_slot,
                materialization_dir: replica_plan.materialization_dir.clone(),
                health_status: plan.health_status,
                listen_base_url: None,
                pid: None,
                last_error: None,
            }]
        });
    replica_plan.effective_env.insert(
        "SORACLOUD_REPLICA_SLOT".to_owned(),
        replica_slot.to_string(),
    );
    for volume in &mut replica_plan.lease_volumes {
        if volume.kind.is_per_replica() {
            volume.local_materialization_dir = hosted_http_per_replica_volume_materialization_dir(
                &PathBuf::from(&volume.local_materialization_dir),
                replica_slot,
            )
            .display()
            .to_string();
        }
    }
    replica_plan
}

fn hosted_http_per_replica_volume_materialization_dir(
    volume_dir: &Path,
    replica_slot: u16,
) -> PathBuf {
    let replica_dir_name = hosted_http_replica_slot_dir_name(replica_slot);
    match (volume_dir.parent(), volume_dir.file_name()) {
        (Some(parent), Some(volume_name)) => parent.join(replica_dir_name).join(volume_name),
        _ => volume_dir.join(replica_dir_name),
    }
}

fn build_hosted_http_service_volume_dir(
    state_dir: &Path,
    service_name: &str,
    service_version: &str,
    volume_kind: SoraLeaseVolumeKindV1,
    volume_name: &str,
) -> PathBuf {
    build_native_service_data_dir(state_dir, service_name)
        .join("revisions")
        .join(sanitize_path_component(service_version))
        .join("volumes")
        .join(if volume_kind.is_per_replica() {
            "per-replica"
        } else {
            "shared"
        })
        .join(sanitize_path_component(volume_name))
}

struct InrouSharedFilesystemMount {
    mount_path: String,
    kind: InrouSharedFilesystemMountKind,
}

enum InrouSharedFilesystemMountKind {
    #[allow(dead_code)]
    Nfs {
        guest_mount_source: String,
        mount_options: String,
    },
    BlockDevice {
        device_serial: String,
        filesystem_type: String,
        mount_options: String,
    },
}

#[allow(dead_code)]
struct InrouLeaseFsExport {
    mount_path: String,
    host_path: PathBuf,
}

struct PortableVmLeaseDisk {
    mount_path: String,
    image_path: PathBuf,
    image_format: &'static str,
    device_serial: String,
    filesystem_type: String,
    mount_options: String,
}

struct PortableVmNetworkPlan {
    netdev: String,
    listen_base_url: String,
    allowlist_hosts: Vec<(String, Ipv4Addr)>,
}

#[derive(Clone, Copy)]
struct PortableVmGuestMachineProfile {
    emulator_candidates: &'static [&'static str],
    machine_type: &'static str,
    serial_console: &'static str,
    root_label: &'static str,
    block_device: &'static str,
    net_device: &'static str,
}

#[cfg(target_os = "linux")]
impl InrouTapNetworkAttachment {
    fn cleanup(&mut self) {
        let delete_link = ["link", "del", "dev", self.tap_name.as_str()];

        if let Some(exportfs_binary) = self.exportfs_binary.as_ref() {
            for export in self.installed_nfs_exports.iter().rev() {
                let export_spec = format!(
                    "{}:{}",
                    export.guest_client,
                    export.export_path_on_host.display()
                );
                let _ = Command::new(exportfs_binary)
                    .args(["-u", export_spec.as_str()])
                    .status();
            }
        }
        for args in self.installed_firewall_rules.iter().rev() {
            let delete_args = inrou_tap_delete_rule_args(args);
            let _ = Command::new(&self.iptables_binary)
                .args(&delete_args)
                .status();
        }
        let _ = Command::new(&self.ip_binary).args(delete_link).status();
    }

    fn accounted_egress_bytes(&self) -> io::Result<u64> {
        if !self.firewall_plan.installs_masquerade_rule() {
            return Ok(0);
        }
        let path = Path::new("/sys/class/net")
            .join(&self.tap_name)
            .join("statistics")
            .join("rx_bytes");
        let contents = fs::read_to_string(&path)?;
        contents.trim().parse::<u64>().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("parse rx_bytes from {}: {error}", path.display()),
            )
        })
    }
}

#[cfg(not(target_os = "linux"))]
impl InrouTapNetworkAttachment {
    fn cleanup(&mut self) {}

    fn accounted_egress_bytes(&self) -> io::Result<u64> {
        Ok(0)
    }
}

#[allow(dead_code)]
fn resolve_inrou_mke2fs_executable() -> Option<PathBuf> {
    resolve_executable_candidates(&["mke2fs", "mkfs.ext4"]).or_else(|| {
        known_host_executable_paths("mkfs.ext4")
            .into_iter()
            .find(|candidate| candidate.is_file())
    })
}

fn resolve_inrou_qemu_img_executable() -> Option<PathBuf> {
    resolve_executable_candidates(&["qemu-img"])
}

#[cfg(target_os = "linux")]
fn resolve_inrou_exportfs_executable() -> Option<PathBuf> {
    resolve_executable_on_path("exportfs")
}

#[cfg(target_os = "linux")]
fn resolve_inrou_rpc_nfsd_executable() -> Option<PathBuf> {
    resolve_executable_on_path("rpc.nfsd")
}

#[allow(dead_code)]
fn resolve_inrou_mount_executable() -> Option<PathBuf> {
    resolve_executable_on_path("mount")
}

#[allow(dead_code)]
fn resolve_inrou_chown_executable() -> Option<PathBuf> {
    resolve_executable_on_path("chown")
}

fn resolve_inrou_bundle_member_path(
    bundle_root: &Path,
    declared_path: &str,
) -> eyre::Result<PathBuf> {
    let bundle_root = fs::canonicalize(bundle_root)
        .wrap_err_with(|| format!("canonicalize {}", bundle_root.display()))?;
    let candidate = bundle_root.join(strip_leading_slashes(declared_path));
    let canonical = fs::canonicalize(&candidate)
        .wrap_err_with(|| format!("canonicalize Inrou bundle member {}", candidate.display()))?;
    if !canonical.starts_with(&bundle_root) {
        eyre::bail!(
            "Inrou bundle member `{declared_path}` resolves outside {}",
            bundle_root.display()
        );
    }
    if !canonical.is_file() {
        eyre::bail!(
            "Inrou bundle member `{declared_path}` must resolve to a regular file under {}",
            bundle_root.display()
        );
    }
    Ok(canonical)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn ensure_inrou_root_disk(
    base_rootfs_image_path: &Path,
    root_volume: &SoracloudRuntimeLeaseVolumePlan,
) -> eyre::Result<PathBuf> {
    let root_volume_dir = PathBuf::from(&root_volume.local_materialization_dir);
    fs::create_dir_all(&root_volume_dir)
        .wrap_err_with(|| format!("create {}", root_volume_dir.display()))?;
    let root_disk_path = root_volume_dir.join("rootfs.ext4");
    if root_disk_path.exists() {
        return Ok(root_disk_path);
    }

    let base_size = fs::metadata(base_rootfs_image_path)
        .wrap_err_with(|| format!("stat {}", base_rootfs_image_path.display()))?
        .len();
    if base_size > root_volume.max_total_bytes {
        eyre::bail!(
            "Inrou base rootfs {} exceeds root lease budget {} bytes for volume `{}`",
            base_rootfs_image_path.display(),
            root_volume.max_total_bytes,
            root_volume.volume_name
        );
    }
    fs::copy(base_rootfs_image_path, &root_disk_path).wrap_err_with(|| {
        format!(
            "copy immutable Inrou rootfs {} into {}",
            base_rootfs_image_path.display(),
            root_disk_path.display()
        )
    })?;
    Ok(root_disk_path)
}

fn ensure_inrou_portable_root_disk(
    qemu_img: &Path,
    base_rootfs_image_path: &Path,
    root_volume: &SoracloudRuntimeLeaseVolumePlan,
) -> eyre::Result<PathBuf> {
    let root_volume_dir = PathBuf::from(&root_volume.local_materialization_dir);
    fs::create_dir_all(&root_volume_dir)
        .wrap_err_with(|| format!("create {}", root_volume_dir.display()))?;
    let root_disk_path = root_volume_dir.join("rootfs.qcow2");
    if root_disk_path.exists() {
        return Ok(root_disk_path);
    }

    let base_size = fs::metadata(base_rootfs_image_path)
        .wrap_err_with(|| format!("stat {}", base_rootfs_image_path.display()))?
        .len();
    if base_size > root_volume.max_total_bytes {
        eyre::bail!(
            "Inrou base rootfs {} exceeds root lease budget {} bytes for volume `{}`",
            base_rootfs_image_path.display(),
            root_volume.max_total_bytes,
            root_volume.volume_name
        );
    }

    run_host_command(
        qemu_img,
        &[
            "create",
            "-q",
            "-f",
            "qcow2",
            "-F",
            "raw",
            "-b",
            base_rootfs_image_path
                .to_str()
                .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
            root_disk_path
                .to_str()
                .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
            &root_volume.max_total_bytes.to_string(),
        ],
    )
    .wrap_err_with(|| {
        format!(
            "create PortableVm root overlay {} with backing file {}",
            root_disk_path.display(),
            base_rootfs_image_path.display()
        )
    })?;
    Ok(root_disk_path)
}

#[cfg(target_os = "linux")]
fn ensure_inrou_nfs_server_running(
    mount_binary: &Path,
    rpc_nfsd_binary: &Path,
) -> eyre::Result<()> {
    let threads_path = Path::new("/proc/fs/nfsd/threads");
    if !threads_path.exists() {
        run_host_command(mount_binary, &["-t", "nfsd", "nfsd", "/proc/fs/nfsd"])
            .wrap_err("mount nfsd pseudo-filesystem for Inrou shared storage")?;
    }
    if !threads_path.exists() {
        eyre::bail!("/proc/fs/nfsd/threads is unavailable after mounting nfsd");
    }
    let current_threads = fs::read_to_string(threads_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(0);
    if current_threads > 0 {
        return Ok(());
    }
    run_host_command(rpc_nfsd_binary, &["8"]).wrap_err("start kernel nfsd threads for Inrou")?;
    let current_threads = fs::read_to_string(threads_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(0);
    if current_threads == 0 {
        eyre::bail!("rpc.nfsd did not report any running threads after startup");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_inrou_shared_filesystem_mounts(
    leasefs_exports: &[InrouLeaseFsExport],
    network_attachment: &mut InrouTapNetworkAttachment,
) -> eyre::Result<Vec<InrouSharedFilesystemMount>> {
    if leasefs_exports.is_empty() {
        return Ok(Vec::new());
    }
    let Some(exportfs_binary) = network_attachment.exportfs_binary.clone() else {
        eyre::bail!("Inrou shared service volumes require `exportfs` on PATH");
    };
    let rpc_nfsd_binary = resolve_inrou_rpc_nfsd_executable()
        .ok_or_else(|| eyre::eyre!("Inrou shared service volumes require `rpc.nfsd` on PATH"))?;
    let mount_binary = resolve_inrou_mount_executable()
        .ok_or_else(|| eyre::eyre!("Inrou shared service volumes require `mount` on PATH"))?;
    let chown_binary = resolve_inrou_chown_executable()
        .ok_or_else(|| eyre::eyre!("Inrou shared service volumes require `chown` on PATH"))?;
    ensure_inrou_nfs_server_running(&mount_binary, &rpc_nfsd_binary)?;

    let mut mounts = Vec::new();
    for export in leasefs_exports {
        let volume_dir = &export.host_path;
        fs::set_permissions(volume_dir, fs::Permissions::from_mode(0o770))
            .wrap_err_with(|| format!("chmod {}", volume_dir.display()))?;
        run_host_command(
            &chown_binary,
            &[
                "1000:1000",
                volume_dir
                    .to_str()
                    .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
            ],
        )
        .wrap_err_with(|| format!("chown shared Inrou volume {}", volume_dir.display()))?;
        let export_options = "rw,sync,no_subtree_check,insecure";
        let export_spec = format!("{}:{}", network_attachment.guest_ip, volume_dir.display());
        run_host_command(
            &exportfs_binary,
            &["-i", "-o", export_options, export_spec.as_str()],
        )
        .wrap_err_with(|| format!("export shared Inrou volume {}", volume_dir.display()))?;
        network_attachment
            .installed_nfs_exports
            .push(InrouNfsExport {
                guest_client: network_attachment.guest_ip.clone(),
                export_path_on_host: volume_dir.clone(),
            });
        mounts.push(InrouSharedFilesystemMount {
            mount_path: export.mount_path.clone(),
            kind: InrouSharedFilesystemMountKind::Nfs {
                guest_mount_source: format!(
                    "{}:{}",
                    network_attachment.host_ip,
                    volume_dir.display()
                ),
                mount_options: "rw,hard,nofail,proto=tcp,port=2049,vers=4".to_owned(),
            },
        });
    }
    Ok(mounts)
}

#[allow(dead_code)]
fn ensure_inrou_leasefs_exports(
    plan: &SoracloudRuntimeServicePlan,
) -> eyre::Result<Vec<InrouLeaseFsExport>> {
    let mut exports = Vec::new();
    for volume in plan
        .lease_volumes
        .iter()
        .filter(|volume| volume.kind != SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
    {
        let volume_dir = PathBuf::from(&volume.local_materialization_dir);
        fs::create_dir_all(&volume_dir)
            .wrap_err_with(|| format!("create {}", volume_dir.display()))?;
        #[cfg(unix)]
        fs::set_permissions(&volume_dir, fs::Permissions::from_mode(0o777))
            .wrap_err_with(|| format!("chmod {}", volume_dir.display()))?;

        exports.push(InrouLeaseFsExport {
            mount_path: volume.mount_path.clone(),
            host_path: volume_dir,
        });
    }
    Ok(exports)
}

fn ensure_inrou_portable_lease_disks(
    qemu_img: &Path,
    plan: &SoracloudRuntimeServicePlan,
) -> eyre::Result<Vec<PortableVmLeaseDisk>> {
    let mut disks = Vec::new();
    for volume in plan
        .lease_volumes
        .iter()
        .filter(|volume| volume.kind != SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
    {
        let volume_dir = PathBuf::from(&volume.local_materialization_dir);
        fs::create_dir_all(&volume_dir)
            .wrap_err_with(|| format!("create {}", volume_dir.display()))?;
        let image_path = volume_dir.join("lease.raw");
        if !image_path.exists() {
            run_host_command(
                qemu_img,
                &[
                    "create",
                    "-q",
                    "-f",
                    "raw",
                    image_path
                        .to_str()
                        .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
                    &volume.max_total_bytes.to_string(),
                ],
            )
            .wrap_err_with(|| {
                format!(
                    "create PortableVm lease disk {} for volume `{}`",
                    image_path.display(),
                    volume.volume_name
                )
            })?;
        }

        disks.push(PortableVmLeaseDisk {
            mount_path: volume.mount_path.clone(),
            image_path,
            image_format: "raw",
            device_serial: portable_vm_block_device_serial(&volume.volume_name),
            filesystem_type: "ext4".to_owned(),
            mount_options: "rw,nofail".to_owned(),
        });
    }
    Ok(disks)
}

fn build_inrou_portable_shared_filesystem_mounts(
    lease_disks: &[PortableVmLeaseDisk],
) -> Vec<InrouSharedFilesystemMount> {
    lease_disks
        .iter()
        .map(|disk| InrouSharedFilesystemMount {
            mount_path: disk.mount_path.clone(),
            kind: InrouSharedFilesystemMountKind::BlockDevice {
                device_serial: disk.device_serial.clone(),
                filesystem_type: disk.filesystem_type.clone(),
                mount_options: disk.mount_options.clone(),
            },
        })
        .collect()
}

fn portable_vm_block_device_serial(volume_name: &str) -> String {
    let sanitized = sanitize_path_component(volume_name);
    format!("sora-{sanitized}").chars().take(20).collect()
}

fn build_portable_vm_network_plan(
    guest_port: u16,
    firewall_plan: &InrouTapFirewallPlan,
) -> eyre::Result<PortableVmNetworkPlan> {
    let host_port = reserve_loopback_tcp_port()?;
    let mut netdev_parts = vec![
        "user".to_owned(),
        "id=net0".to_owned(),
        "ipv6=off".to_owned(),
        format!("hostfwd=tcp:127.0.0.1:{host_port}-:{guest_port}"),
    ];
    let mut allowlist_hosts = Vec::new();
    match firewall_plan {
        InrouTapFirewallPlan::Open => {}
        InrouTapFirewallPlan::Isolated => {
            netdev_parts.push("restrict=on".to_owned());
        }
        InrouTapFirewallPlan::Allowlist(endpoints) => {
            netdev_parts.push("restrict=on".to_owned());
            for endpoint in endpoints {
                netdev_parts.push(format!(
                    "guestfwd=tcp:{}:{}-tcp:{}:{}",
                    endpoint.address, endpoint.port, endpoint.address, endpoint.port
                ));
                allowlist_hosts.push((endpoint.host.clone(), endpoint.address));
            }
            allowlist_hosts.sort();
            allowlist_hosts.dedup();
        }
    }

    Ok(PortableVmNetworkPlan {
        netdev: netdev_parts.join(","),
        listen_base_url: format!("http://127.0.0.1:{host_port}"),
        allowlist_hosts,
    })
}

fn reserve_loopback_tcp_port() -> eyre::Result<u16> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .wrap_err("bind loopback TCP port for PortableVm host forwarding")?;
    listener
        .local_addr()
        .map(|address| address.port())
        .wrap_err("query PortableVm loopback forwarding port")
}

fn write_inrou_cloud_init_documents(
    materialization_dir: &Path,
    cache_key: &HostedHttpWorkerCacheKey,
    network_config: &str,
    user_data: &str,
) -> eyre::Result<PathBuf> {
    let seed_root = materialization_dir.join("inrou_cloud_init");
    reset_directory(&seed_root).wrap_err_with(|| format!("reset {}", seed_root.display()))?;
    fs::create_dir_all(&seed_root).wrap_err_with(|| format!("create {}", seed_root.display()))?;

    let metadata = format!(
        "instance-id: {}\nlocal-hostname: {}\n",
        sanitize_path_component(&format!(
            "{}-{}-{}-{}",
            cache_key.service_name,
            cache_key.service_version,
            cache_key.process_generation,
            cache_key.replica_slot
        )),
        sanitize_path_component(&format!(
            "inrou-{}-{}",
            cache_key.service_name, cache_key.replica_slot
        ))
    );

    write_bytes_atomic(&seed_root.join("meta-data"), metadata.as_bytes())
        .wrap_err("write Inrou cloud-init meta-data")?;
    write_bytes_atomic(&seed_root.join("network-config"), network_config.as_bytes())
        .wrap_err("write Inrou cloud-init network-config")?;
    write_bytes_atomic(&seed_root.join("user-data"), user_data.as_bytes())
        .wrap_err("write Inrou cloud-init user-data")?;
    Ok(seed_root)
}

fn start_portable_vm_metadata_server(seed_root: &Path) -> eyre::Result<PortableVmMetadataServer> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .wrap_err("bind PortableVm cloud-init metadata server")?;
    listener
        .set_nonblocking(true)
        .wrap_err("configure PortableVm metadata listener as nonblocking")?;
    let bind_addr = listener
        .local_addr()
        .wrap_err("query PortableVm metadata listener address")?;
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let seed_root = seed_root.to_path_buf();
    let thread = thread::Builder::new()
        .name("inrou-portable-metadata".to_owned())
        .spawn(move || {
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = serve_portable_vm_metadata_request(&mut stream, &seed_root);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if shutdown_rx.try_recv().is_ok() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        })
        .wrap_err("spawn PortableVm metadata server thread")?;

    Ok(PortableVmMetadataServer {
        bind_addr,
        shutdown_tx: Some(shutdown_tx),
        thread: Some(thread),
    })
}

fn serve_portable_vm_metadata_request(
    stream: &mut std::net::TcpStream,
    seed_root: &Path,
) -> io::Result<()> {
    use io::{Read as _, Write as _};

    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut request = [0_u8; 4096];
    let read = stream.read(&mut request)?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&request[..read]);
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    if method != "GET" {
        stream.write_all(
            b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(());
    }
    let (file_path, content_type) = match path {
        "/meta-data" => (seed_root.join("meta-data"), "text/plain; charset=utf-8"),
        "/network-config" => (
            seed_root.join("network-config"),
            "text/plain; charset=utf-8",
        ),
        "/user-data" => (seed_root.join("user-data"), "text/plain; charset=utf-8"),
        INROU_PORTABLE_BUNDLE_METADATA_PATH => (
            seed_root.join(INROU_PORTABLE_BUNDLE_METADATA_MEMBER),
            "application/gzip",
        ),
        _ => {
            stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;
            return Ok(());
        }
    };
    let body = fs::read(file_path)?;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(&body)?;
    Ok(())
}

fn stage_portable_vm_metadata_bundle(
    seed_root: &Path,
    bundle_cache_path: &Path,
) -> eyre::Result<()> {
    let bundle_path = seed_root.join(INROU_PORTABLE_BUNDLE_METADATA_MEMBER);
    fs::create_dir_all(
        bundle_path
            .parent()
            .ok_or_else(|| eyre::eyre!("path must have parent: {}", bundle_path.display()))?,
    )
    .wrap_err_with(|| format!("create parent for {}", bundle_path.display()))?;
    fs::copy(bundle_cache_path, &bundle_path).wrap_err_with(|| {
        format!(
            "copy {} to {}",
            bundle_cache_path.display(),
            bundle_path.display()
        )
    })?;
    Ok(())
}

fn portable_vm_metadata_base_url(port: u16) -> String {
    format!("http://10.0.2.2:{port}/")
}

fn build_portable_vm_allowlist_hosts_overlay(
    allowlist_hosts: &[(String, Ipv4Addr)],
) -> Option<String> {
    let lines = allowlist_hosts
        .iter()
        .filter(|(host, _address)| host.parse::<IpAddr>().is_err())
        .map(|(host, address)| format!("{address} {host}"))
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| {
        let mut overlay = lines.join("\n");
        overlay.push('\n');
        overlay
    })
}

fn portable_vm_kernel_cmdline(
    profile: PortableVmGuestMachineProfile,
    datasource_base_url: &str,
) -> String {
    format!(
        "console={} root=LABEL={} rw rootwait rootfstype=ext4 panic=1 ds=nocloud-net;s={}",
        profile.serial_console, profile.root_label, datasource_base_url
    )
}

fn portable_vm_vcpu_count(resources: &iroha_data_model::soracloud::SoraResourceLimitsV1) -> u32 {
    u32::from(resources.cpu_millis.get()).div_ceil(1_000).max(1)
}

fn portable_vm_memory_mib(resources: &iroha_data_model::soracloud::SoraResourceLimitsV1) -> u64 {
    resources.memory_bytes.get().div_ceil(1024 * 1024).max(128)
}

fn append_portable_vm_drive(
    command: &mut Command,
    profile: PortableVmGuestMachineProfile,
    drive_id: &str,
    file_path: &Path,
    format: &str,
    read_only: bool,
    discard_on_unmap: bool,
) {
    append_portable_vm_drive_with_serial(
        command,
        profile,
        drive_id,
        file_path,
        format,
        read_only,
        discard_on_unmap,
        None,
    );
}

fn append_portable_vm_drive_with_serial(
    command: &mut Command,
    profile: PortableVmGuestMachineProfile,
    drive_id: &str,
    file_path: &Path,
    format: &str,
    read_only: bool,
    discard_on_unmap: bool,
    serial: Option<&str>,
) {
    let mut device = format!("{},drive={drive_id}", profile.block_device);
    if let Some(serial) = serial {
        device.push_str(",serial=");
        device.push_str(serial);
    }
    command
        .arg("-drive")
        .arg(format!(
            "if=none,id={drive_id},format={format},readonly={},discard={},file={}",
            if read_only { "on" } else { "off" },
            if discard_on_unmap { "unmap" } else { "ignore" },
            file_path.display(),
        ))
        .arg("-device")
        .arg(device);
}

#[cfg(target_os = "linux")]
fn build_inrou_bootstrap_seed(
    mke2fs: &Path,
    materialization_dir: &Path,
    plan: &SoracloudRuntimeServicePlan,
    cache_key: &HostedHttpWorkerCacheKey,
    guest_port: u16,
    network_attachment: &InrouTapNetworkAttachment,
    shared_filesystem_mounts: &[InrouSharedFilesystemMount],
    bootstrap_user_data_overlay: Option<&str>,
) -> eyre::Result<PathBuf> {
    let network_config = build_inrou_network_config(network_attachment);
    let user_data = build_inrou_user_data(
        plan,
        cache_key,
        guest_port,
        shared_filesystem_mounts,
        bootstrap_user_data_overlay,
        None,
        None,
    );
    build_inrou_bootstrap_seed_from_documents(
        mke2fs,
        materialization_dir,
        cache_key,
        &network_config,
        &user_data,
    )
}

#[allow(dead_code)]
fn build_inrou_bootstrap_seed_from_documents(
    mke2fs: &Path,
    materialization_dir: &Path,
    cache_key: &HostedHttpWorkerCacheKey,
    network_config: &str,
    user_data: &str,
) -> eyre::Result<PathBuf> {
    let seed_root = write_inrou_cloud_init_documents(
        materialization_dir,
        cache_key,
        network_config,
        user_data,
    )?;

    let seed_image_path = materialization_dir.join("inrou_cloud_init.ext4");
    create_populated_ext4_image(
        mke2fs,
        &seed_image_path,
        &seed_root,
        16 * 1024 * 1024,
        "cidata",
    )
    .wrap_err("build Inrou cloud-init ext4 image")?;
    Ok(seed_image_path)
}

#[cfg(target_os = "linux")]
fn json_string_literal(value: impl AsRef<str>) -> String {
    norito::json::to_string(&norito::json!(value.as_ref())).expect("valid json string")
}

#[cfg(target_os = "linux")]
fn write_inrou_firecracker_config(
    materialization_dir: &Path,
    kernel_image_path: &Path,
    initrd_image_path: Option<&Path>,
    root_disk_path: &Path,
    seed_image_path: &Path,
    network_attachment: &InrouTapNetworkAttachment,
    resources: iroha_data_model::soracloud::SoraResourceLimitsV1,
) -> eyre::Result<PathBuf> {
    let vcpu_count = u32::from(resources.cpu_millis.get()).div_ceil(1_000).max(1);
    let mem_size_mib = resources.memory_bytes.get().div_ceil(1024 * 1024).max(128);
    let mut drives = Vec::new();
    drives.push(format!(
        "{{\"drive_id\":\"rootfs\",\"path_on_host\":{},\"is_root_device\":true,\"is_read_only\":false}}",
        json_string_literal(root_disk_path.display().to_string()),
    ));
    drives.push(format!(
        "{{\"drive_id\":\"seed\",\"path_on_host\":{},\"is_root_device\":false,\"is_read_only\":true}}",
        json_string_literal(seed_image_path.display().to_string()),
    ));
    let initrd_fragment = initrd_image_path.map_or_else(String::new, |initrd| {
        format!(
            ",\"initrd_path\":{}",
            json_string_literal(initrd.display().to_string())
        )
    });
    let config = format!(
        concat!(
            "{{",
            "\"boot-source\":{{\"kernel_image_path\":{},\"boot_args\":\"console=ttyS0 reboot=k panic=1 pci=off\"{}}},",
            "\"drives\":[{}],",
            "\"machine-config\":{{\"vcpu_count\":{},\"mem_size_mib\":{},\"smt\":false}},",
            "\"network-interfaces\":[{{\"iface_id\":\"eth0\",\"host_dev_name\":{},\"guest_mac\":{}}}]",
            "}}"
        ),
        json_string_literal(kernel_image_path.display().to_string()),
        initrd_fragment,
        drives.join(","),
        vcpu_count,
        mem_size_mib,
        json_string_literal(&network_attachment.tap_name),
        json_string_literal(&network_attachment.guest_mac),
    );
    let config_path = materialization_dir.join("firecracker-config.json");
    write_bytes_atomic(&config_path, config.as_bytes())
        .wrap_err_with(|| format!("write {}", config_path.display()))?;
    Ok(config_path)
}

#[cfg(target_os = "linux")]
fn setup_inrou_tap_network(
    cache_key: &HostedHttpWorkerCacheKey,
    _materialization_dir: &Path,
    firewall_plan: InrouTapFirewallPlan,
) -> eyre::Result<InrouTapNetworkAttachment> {
    if !inrou_ip_forward_enabled()? {
        eyre::bail!(
            "Inrou microVM networking requires host IPv4 forwarding to be enabled (`/proc/sys/net/ipv4/ip_forward = 1`)"
        );
    }
    let ip_binary = resolve_executable_on_path("ip")
        .ok_or_else(|| eyre::eyre!("Inrou runtime requires `ip` on PATH"))?;
    let iptables_binary = resolve_executable_on_path("iptables")
        .ok_or_else(|| eyre::eyre!("Inrou runtime requires `iptables` on PATH"))?;
    let exportfs_binary = resolve_executable_on_path("exportfs");
    let mut attachment = derive_inrou_tap_network_attachment(
        cache_key,
        ip_binary,
        iptables_binary,
        exportfs_binary,
        firewall_plan,
    );
    let host_cidr = format!("{}/30", attachment.host_ip);
    let guest_cidr = format!("{}/32", attachment.guest_ip);

    let link_path = Path::new("/sys/class/net").join(&attachment.tap_name);
    if link_path.exists() {
        let _ = Command::new(&attachment.ip_binary)
            .args(["link", "del", "dev", attachment.tap_name.as_str()])
            .status();
    }

    run_host_command(
        &attachment.ip_binary,
        &[
            "tuntap",
            "add",
            "dev",
            attachment.tap_name.as_str(),
            "mode",
            "tap",
        ],
    )
    .wrap_err("create Inrou tap device")?;
    run_host_command(
        &attachment.ip_binary,
        &[
            "addr",
            "add",
            host_cidr.as_str(),
            "dev",
            attachment.tap_name.as_str(),
        ],
    )
    .wrap_err("assign Inrou tap host address")?;
    run_host_command(
        &attachment.ip_binary,
        &["link", "set", "dev", attachment.tap_name.as_str(), "up"],
    )
    .wrap_err("bring Inrou tap device up")?;
    if let Err(error) = install_inrou_tap_firewall_rules(&mut attachment, guest_cidr.as_str()) {
        attachment.cleanup();
        return Err(error);
    }

    Ok(attachment)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn inrou_tap_firewall_plan(
    network_policy: &SoraNetworkPolicyV1,
) -> eyre::Result<InrouTapFirewallPlan> {
    match network_policy {
        SoraNetworkPolicyV1::Open => Ok(InrouTapFirewallPlan::Open),
        SoraNetworkPolicyV1::Isolated => Ok(InrouTapFirewallPlan::Isolated),
        SoraNetworkPolicyV1::Allowlist(allowed_hosts) => Ok(InrouTapFirewallPlan::Allowlist(
            resolve_inrou_allowlist_endpoints(allowed_hosts)?,
        )),
    }
}

#[cfg(target_os = "linux")]
fn install_inrou_tap_firewall_rules(
    attachment: &mut InrouTapNetworkAttachment,
    guest_cidr: &str,
) -> eyre::Result<()> {
    for rule in planned_inrou_tap_firewall_rules(
        attachment.tap_name.as_str(),
        guest_cidr,
        &attachment.firewall_plan,
    ) {
        install_inrou_iptables_rule(attachment, rule.args, rule.context)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_inrou_iptables_rule(
    attachment: &mut InrouTapNetworkAttachment,
    args: Vec<String>,
    context: &'static str,
) -> eyre::Result<()> {
    run_host_command_strings(&attachment.iptables_binary, &args).wrap_err(context)?;
    attachment.installed_firewall_rules.push(args);
    Ok(())
}

#[allow(dead_code)]
fn run_host_command_strings(program: &Path, args: &[String]) -> eyre::Result<()> {
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_host_command(program, &borrowed)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn inrou_tap_delete_rule_args(args: &[String]) -> Vec<String> {
    let mut delete_args = args.to_vec();
    if let Some(index) = delete_args
        .iter()
        .position(|arg| arg == "-A" || arg == "-I")
    {
        delete_args[index] = "-D".to_owned();
        if delete_args
            .get(index + 2)
            .is_some_and(|value| value.chars().all(|ch| ch.is_ascii_digit()))
        {
            delete_args.remove(index + 2);
        }
    }
    delete_args
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn planned_inrou_tap_firewall_rules(
    tap_name: &str,
    guest_cidr: &str,
    firewall_plan: &InrouTapFirewallPlan,
) -> Vec<InrouTapFirewallRuleSpec> {
    let mut rules = Vec::new();
    rules.push(InrouTapFirewallRuleSpec {
        args: vec![
            "-I".to_owned(),
            "INPUT".to_owned(),
            "1".to_owned(),
            "-i".to_owned(),
            tap_name.to_owned(),
            "-p".to_owned(),
            "tcp".to_owned(),
            "--dport".to_owned(),
            "2049".to_owned(),
            "-j".to_owned(),
            "ACCEPT".to_owned(),
        ],
        context: "install Inrou shared-storage ingress rule",
    });
    if firewall_plan.installs_masquerade_rule() {
        rules.push(InrouTapFirewallRuleSpec {
            args: vec![
                "-t".to_owned(),
                "nat".to_owned(),
                "-I".to_owned(),
                "POSTROUTING".to_owned(),
                "1".to_owned(),
                "-s".to_owned(),
                guest_cidr.to_owned(),
                "!".to_owned(),
                "-o".to_owned(),
                tap_name.to_owned(),
                "-j".to_owned(),
                "MASQUERADE".to_owned(),
            ],
            context: "install Inrou egress masquerade rule",
        });
    }
    rules.push(InrouTapFirewallRuleSpec {
        args: vec![
            "-I".to_owned(),
            "INPUT".to_owned(),
            "1".to_owned(),
            "-i".to_owned(),
            tap_name.to_owned(),
            "-m".to_owned(),
            "conntrack".to_owned(),
            "--ctstate".to_owned(),
            "RELATED,ESTABLISHED".to_owned(),
            "-j".to_owned(),
            "ACCEPT".to_owned(),
        ],
        context: "install Inrou established host-input rule",
    });
    rules.push(InrouTapFirewallRuleSpec {
        args: vec![
            "-I".to_owned(),
            "INPUT".to_owned(),
            "3".to_owned(),
            "-i".to_owned(),
            tap_name.to_owned(),
            "-j".to_owned(),
            "DROP".to_owned(),
        ],
        context: "install Inrou host-input default-drop rule",
    });
    if firewall_plan.installs_return_rule() {
        rules.push(InrouTapFirewallRuleSpec {
            args: vec![
                "-I".to_owned(),
                "FORWARD".to_owned(),
                "1".to_owned(),
                "-o".to_owned(),
                tap_name.to_owned(),
                "-m".to_owned(),
                "conntrack".to_owned(),
                "--ctstate".to_owned(),
                "RELATED,ESTABLISHED".to_owned(),
                "-j".to_owned(),
                "ACCEPT".to_owned(),
            ],
            context: "install Inrou return-traffic rule",
        });
    }
    match firewall_plan {
        InrouTapFirewallPlan::Open => rules.push(InrouTapFirewallRuleSpec {
            args: vec![
                "-I".to_owned(),
                "FORWARD".to_owned(),
                "1".to_owned(),
                "-i".to_owned(),
                tap_name.to_owned(),
                "-j".to_owned(),
                "ACCEPT".to_owned(),
            ],
            context: "install Inrou forward-out rule",
        }),
        InrouTapFirewallPlan::Isolated => rules.push(InrouTapFirewallRuleSpec {
            args: vec![
                "-I".to_owned(),
                "FORWARD".to_owned(),
                "1".to_owned(),
                "-i".to_owned(),
                tap_name.to_owned(),
                "-j".to_owned(),
                "DROP".to_owned(),
            ],
            context: "install Inrou isolated forward-drop rule",
        }),
        InrouTapFirewallPlan::Allowlist(endpoints) => {
            rules.push(InrouTapFirewallRuleSpec {
                args: vec![
                    "-I".to_owned(),
                    "FORWARD".to_owned(),
                    "1".to_owned(),
                    "-i".to_owned(),
                    tap_name.to_owned(),
                    "-j".to_owned(),
                    "DROP".to_owned(),
                ],
                context: "install Inrou allowlist default-drop rule",
            });
            for endpoint in endpoints {
                rules.push(InrouTapFirewallRuleSpec {
                    args: vec![
                        "-I".to_owned(),
                        "FORWARD".to_owned(),
                        "1".to_owned(),
                        "-i".to_owned(),
                        tap_name.to_owned(),
                        "-p".to_owned(),
                        "tcp".to_owned(),
                        "-d".to_owned(),
                        endpoint.address.to_string(),
                        "--dport".to_owned(),
                        endpoint.port.to_string(),
                        "-j".to_owned(),
                        "ACCEPT".to_owned(),
                    ],
                    context: "install Inrou allowlist forward rule",
                });
            }
        }
    }
    rules
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn resolve_inrou_allowlist_endpoints(
    entries: &[iroha_data_model::soracloud::SoraNetworkAllowlistEntryV1],
) -> eyre::Result<Vec<InrouTapResolvedAllowlistEndpoint>> {
    let mut resolved = BTreeSet::new();
    for entry in entries {
        let host = entry.host.trim();
        let mut host_endpoints = BTreeSet::new();
        if let Ok(address) = host.parse::<IpAddr>() {
            if let IpAddr::V4(address) = address {
                for port in &entry.ports {
                    host_endpoints.insert(InrouTapResolvedAllowlistEndpoint {
                        host: host.to_owned(),
                        address,
                        port: *port,
                    });
                }
            }
        } else {
            for port in &entry.ports {
                let addresses = (host, *port).to_socket_addrs().wrap_err_with(|| {
                    format!("resolve allowlist host `{host}` on port `{port}`")
                })?;
                for address in addresses {
                    if let SocketAddr::V4(address) = address {
                        host_endpoints.insert(InrouTapResolvedAllowlistEndpoint {
                            host: host.to_owned(),
                            address: *address.ip(),
                            port: address.port(),
                        });
                    }
                }
            }
        }
        if host_endpoints.is_empty() {
            eyre::bail!(
                "Inrou hosted HTTP allowlist host `{host}` resolved to no IPv4 endpoints for declared ports {:?}",
                entry.ports
            );
        }
        resolved.extend(host_endpoints);
    }
    Ok(resolved.into_iter().collect())
}

#[cfg(target_os = "linux")]
fn derive_inrou_tap_network_attachment(
    cache_key: &HostedHttpWorkerCacheKey,
    ip_binary: PathBuf,
    iptables_binary: PathBuf,
    exportfs_binary: Option<PathBuf>,
    firewall_plan: InrouTapFirewallPlan,
) -> InrouTapNetworkAttachment {
    let fingerprint = Hash::new(
        format!(
            "{}:{}:{}:{}:{}",
            cache_key.service_name,
            cache_key.service_version,
            cache_key.process_generation,
            cache_key.replica_slot,
            cache_key.bundle_hash
        )
        .as_bytes(),
    );
    let bytes = fingerprint.as_ref();
    let tap_name = format!("ir{}", hex::encode(&bytes[..6]));
    let third_octet = bytes[6].max(1);
    let network_base = (bytes[7] & 0b1111_1100).clamp(4, 248);
    let host_ip = format!("172.31.{third_octet}.{}", network_base + 1);
    let guest_ip = format!("172.31.{third_octet}.{}", network_base + 2);
    let guest_mac = format!(
        "06:fc:{:02x}:{:02x}:{:02x}:{:02x}",
        bytes[8], bytes[9], bytes[10], bytes[11]
    );

    InrouTapNetworkAttachment {
        ip_binary,
        iptables_binary,
        exportfs_binary,
        tap_name: tap_name.chars().take(15).collect(),
        host_ip,
        guest_ip,
        guest_mac,
        firewall_plan,
        installed_firewall_rules: Vec::new(),
        installed_nfs_exports: Vec::new(),
    }
}

#[cfg(target_os = "linux")]
fn inrou_ip_forward_enabled() -> io::Result<bool> {
    Ok(fs::read_to_string("/proc/sys/net/ipv4/ip_forward")?.trim() == "1")
}

fn run_host_command(program: &Path, args: &[&str]) -> eyre::Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .wrap_err_with(|| format!("spawn {} {}", program.display(), args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    eyre::bail!(
        "{} {} failed with status {}{}{}",
        program.display(),
        args.join(" "),
        output.status,
        if output.stdout.is_empty() {
            String::new()
        } else {
            format!(" stdout={}", String::from_utf8_lossy(&output.stdout).trim())
        },
        if output.stderr.is_empty() {
            String::new()
        } else {
            format!(" stderr={}", String::from_utf8_lossy(&output.stderr).trim())
        }
    );
}

#[allow(dead_code)]
fn create_populated_ext4_image(
    mke2fs: &Path,
    image_path: &Path,
    source_dir: &Path,
    size_bytes: u64,
    label: &str,
) -> eyre::Result<()> {
    fs::create_dir_all(
        image_path
            .parent()
            .ok_or_else(|| eyre::eyre!("path must have parent: {}", image_path.display()))?,
    )?;
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(image_path)
        .wrap_err_with(|| format!("create {}", image_path.display()))?;
    file.set_len(size_bytes)
        .wrap_err_with(|| format!("resize {}", image_path.display()))?;
    run_host_command(
        mke2fs,
        &[
            "-q",
            "-t",
            "ext4",
            "-F",
            "-L",
            label,
            "-d",
            source_dir
                .to_str()
                .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
            image_path
                .to_str()
                .ok_or_else(|| eyre::eyre!("non-utf8 path"))?,
        ],
    )
}

#[cfg(target_os = "linux")]
fn build_inrou_network_config(network_attachment: &InrouTapNetworkAttachment) -> String {
    let nameservers = collect_host_nameservers();
    let nameserver_yaml = nameservers
        .iter()
        .map(|value| yaml_single_quote(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        concat!(
            "version: 2\n",
            "ethernets:\n",
            "  eth0:\n",
            "    dhcp4: false\n",
            "    addresses:\n",
            "      - {}/30\n",
            "    routes:\n",
            "      - to: 0.0.0.0/0\n",
            "        via: {}\n",
            "    nameservers:\n",
            "      addresses: [{}]\n"
        ),
        network_attachment.guest_ip, network_attachment.host_ip, nameserver_yaml
    )
}

fn build_inrou_portable_network_config() -> String {
    String::from(concat!(
        "version: 2\n",
        "ethernets:\n",
        "  inrou0:\n",
        "    match:\n",
        "      name: \"e*\"\n",
        "    dhcp4: true\n",
        "    dhcp6: false\n"
    ))
}

fn build_inrou_user_data(
    plan: &SoracloudRuntimeServicePlan,
    cache_key: &HostedHttpWorkerCacheKey,
    guest_port: u16,
    shared_filesystem_mounts: &[InrouSharedFilesystemMount],
    bootstrap_user_data_overlay: Option<&str>,
    allowlist_hosts_overlay: Option<&str>,
    portable_bundle_hash: Option<&str>,
) -> String {
    let mut prepare_script = String::from("#!/bin/sh\nset -eu\n");
    prepare_script.push_str(
        "if [ -w /dev/console ]; then exec >>/dev/console 2>&1; fi\n\
         echo 'Inrou prepare: starting'\n",
    );
    prepare_script
        .push_str("mkdir -p /var/lib/soracloud/service /var/lib/soracloud/materialization\n");
    if let Some(bundle_hash) = portable_bundle_hash {
        let guest_entrypoint = format!(
            "{}/{}",
            INROU_PORTABLE_BUNDLE_GUEST_ROOT,
            strip_leading_slashes(&cache_key.entrypoint)
        );
        prepare_script.push_str("bundle_root=");
        prepare_script.push_str(&shell_single_quote(INROU_PORTABLE_BUNDLE_GUEST_ROOT));
        prepare_script.push('\n');
        prepare_script
            .push_str("bundle_marker='/var/lib/soracloud/materialization/.bundle_hash'\n");
        prepare_script.push_str("bundle_entrypoint=");
        prepare_script.push_str(&shell_single_quote(&guest_entrypoint));
        prepare_script.push('\n');
        prepare_script.push_str("if [ \"$(cat \"$bundle_marker\" 2>/dev/null || true)\" != ");
        prepare_script.push_str(&shell_single_quote(bundle_hash));
        prepare_script.push_str(" ] || [ ! -x \"$bundle_entrypoint\" ]; then\n");
        prepare_script.push_str("  if ! command -v python3 >/dev/null 2>&1; then\n");
        prepare_script.push_str(
            "    echo 'Inrou PortableVm bundle materialization requires python3 in the guest image' >&2\n",
        );
        prepare_script.push_str("    exit 1\n");
        prepare_script.push_str("  fi\n");
        prepare_script.push_str(
            "  datasource_url=$(sed -n 's/.*ds=nocloud-net;s=\\([^ ]*\\).*/\\1/p' /proc/cmdline | head -n 1)\n",
        );
        prepare_script.push_str("  if [ -z \"$datasource_url\" ]; then\n");
        prepare_script.push_str(
            "    echo 'Inrou PortableVm metadata datasource URL not found in /proc/cmdline' >&2\n",
        );
        prepare_script.push_str("    exit 1\n");
        prepare_script.push_str("  fi\n");
        prepare_script.push_str("  bundle_tmp=$(mktemp /tmp/soracloud-bundle.XXXXXX.tgz)\n");
        prepare_script.push_str("  python3 - \"${datasource_url%/}");
        prepare_script.push_str(INROU_PORTABLE_BUNDLE_METADATA_PATH);
        prepare_script.push_str("\" \"$bundle_tmp\" <<'PY'\n");
        prepare_script.push_str("import sys\n");
        prepare_script.push_str("import urllib.request\n");
        prepare_script.push_str("url, dest = sys.argv[1], sys.argv[2]\n");
        prepare_script.push_str("with urllib.request.urlopen(url, timeout=30) as response:\n");
        prepare_script.push_str("    data = response.read()\n");
        prepare_script.push_str("with open(dest, 'wb') as handle:\n");
        prepare_script.push_str("    handle.write(data)\n");
        prepare_script.push_str("PY\n");
        prepare_script.push_str("  rm -rf \"$bundle_root\"\n");
        prepare_script.push_str("  mkdir -p \"$bundle_root\"\n");
        prepare_script.push_str("  tar -xzf \"$bundle_tmp\" -C \"$bundle_root\"\n");
        prepare_script.push_str("  chown -R inrou:inrou \"$bundle_root\"\n");
        prepare_script.push_str("  printf '%s\\n' ");
        prepare_script.push_str(&shell_single_quote(bundle_hash));
        prepare_script.push_str(" > \"$bundle_marker\"\n");
        prepare_script.push_str("  rm -f \"$bundle_tmp\"\n");
        prepare_script.push_str("fi\n");
    }
    if allowlist_hosts_overlay.is_some() {
        prepare_script.push_str("if [ -f /etc/soracloud/allowlist-hosts ]; then\n");
        prepare_script.push_str("  cp /etc/hosts /tmp/soracloud-hosts\n");
        prepare_script.push_str(
            "  while IFS= read -r line; do grep -qxF \"$line\" /tmp/soracloud-hosts || echo \"$line\" >> /tmp/soracloud-hosts; done < /etc/soracloud/allowlist-hosts\n",
        );
        prepare_script.push_str("  cat /tmp/soracloud-hosts > /etc/hosts\n");
        prepare_script.push_str("  rm -f /tmp/soracloud-hosts\n");
        prepare_script.push_str("fi\n");
    }
    if !shared_filesystem_mounts.is_empty() {
        for mount in shared_filesystem_mounts {
            prepare_script.push_str("mkdir -p ");
            prepare_script.push_str(&shell_single_quote(&mount.mount_path));
            prepare_script.push('\n');
            prepare_script.push_str("if ! mountpoint -q ");
            prepare_script.push_str(&shell_single_quote(&mount.mount_path));
            prepare_script.push_str("; then\n");
            match &mount.kind {
                InrouSharedFilesystemMountKind::Nfs {
                    guest_mount_source,
                    mount_options,
                } => {
                    prepare_script.push_str("  if ! command -v mount.nfs >/dev/null 2>&1; then\n");
                    prepare_script.push_str(
                        "    echo 'Inrou shared service storage requires mount.nfs in the guest image' >&2\n",
                    );
                    prepare_script.push_str("    exit 1\n");
                    prepare_script.push_str("  fi\n");
                    prepare_script.push_str("  mount -t nfs -o ");
                    prepare_script.push_str(&shell_single_quote(mount_options));
                    prepare_script.push(' ');
                    prepare_script.push_str(&shell_single_quote(guest_mount_source));
                    prepare_script.push(' ');
                    prepare_script.push_str(&shell_single_quote(&mount.mount_path));
                    prepare_script.push('\n');
                }
                InrouSharedFilesystemMountKind::BlockDevice {
                    device_serial,
                    filesystem_type,
                    mount_options,
                } => {
                    prepare_script.push_str("  device_path=");
                    prepare_script.push_str(&shell_single_quote(&format!(
                        "/dev/disk/by-id/virtio-{device_serial}"
                    )));
                    prepare_script.push('\n');
                    prepare_script.push_str("  attempt=0\n");
                    prepare_script.push_str(
                        "  while [ ! -b \"$device_path\" ] && [ \"$attempt\" -lt 50 ]; do\n",
                    );
                    prepare_script.push_str("    attempt=$((attempt + 1))\n");
                    prepare_script.push_str("    sleep 0.2\n");
                    prepare_script.push_str("  done\n");
                    prepare_script.push_str("  if [ ! -b \"$device_path\" ]; then\n");
                    prepare_script.push_str(
                        "    echo \"Inrou PortableVm volume device not found: $device_path\" >&2\n",
                    );
                    prepare_script.push_str("    exit 1\n");
                    prepare_script.push_str("  fi\n");
                    prepare_script.push_str("  if ! command -v blkid >/dev/null 2>&1; then\n");
                    prepare_script.push_str(
                        "    echo 'Inrou PortableVm block volumes require blkid in the guest image' >&2\n",
                    );
                    prepare_script.push_str("    exit 1\n");
                    prepare_script.push_str("  fi\n");
                    prepare_script
                        .push_str("  if ! blkid \"$device_path\" >/dev/null 2>&1; then\n");
                    prepare_script
                        .push_str("    if ! command -v mkfs.ext4 >/dev/null 2>&1; then\n");
                    prepare_script.push_str(
                        "      echo 'Inrou PortableVm block volumes require mkfs.ext4 in the guest image' >&2\n",
                    );
                    prepare_script.push_str("      exit 1\n");
                    prepare_script.push_str("    fi\n");
                    prepare_script.push_str("    mkfs.ext4 -F \"$device_path\"\n");
                    prepare_script.push_str("  fi\n");
                    prepare_script.push_str("  mount -t ");
                    prepare_script.push_str(&shell_single_quote(filesystem_type));
                    prepare_script.push_str(" -o ");
                    prepare_script.push_str(&shell_single_quote(mount_options));
                    prepare_script.push(' ');
                    prepare_script.push_str("\"$device_path\"");
                    prepare_script.push(' ');
                    prepare_script.push_str(&shell_single_quote(&mount.mount_path));
                    prepare_script.push('\n');
                }
            }
            prepare_script.push_str("fi\n");
            prepare_script.push_str("chown inrou:inrou ");
            prepare_script.push_str(&shell_single_quote(&mount.mount_path));
            prepare_script.push_str(" 2>/dev/null || true\n");
            prepare_script.push_str("chmod 0775 ");
            prepare_script.push_str(&shell_single_quote(&mount.mount_path));
            prepare_script.push_str(" 2>/dev/null || true\n");
        }
    }
    prepare_script.push_str("if command -v python3 >/dev/null 2>&1; then\n");
    prepare_script.push_str("  python3 - ");
    prepare_script.push_str(&shell_single_quote(&guest_port.to_string()));
    prepare_script.push_str(" <<'PY' || true\n");
    prepare_script.push_str("import os\n");
    prepare_script.push_str("import signal\n");
    prepare_script.push_str("import sys\n");
    prepare_script.push_str("import time\n");
    prepare_script.push_str("port = int(sys.argv[1])\n");
    prepare_script.push_str("def listener_inodes():\n");
    prepare_script.push_str("    found = set()\n");
    prepare_script.push_str("    for table in ('/proc/net/tcp', '/proc/net/tcp6'):\n");
    prepare_script.push_str("        try:\n");
    prepare_script
        .push_str("            lines = open(table, encoding='ascii').read().splitlines()[1:]\n");
    prepare_script.push_str("        except OSError:\n");
    prepare_script.push_str("            continue\n");
    prepare_script.push_str("        for line in lines:\n");
    prepare_script.push_str("            cols = line.split()\n");
    prepare_script.push_str("            if len(cols) < 10 or cols[3] != '0A':\n");
    prepare_script.push_str("                continue\n");
    prepare_script.push_str("            try:\n");
    prepare_script.push_str("                local_port = int(cols[1].rsplit(':', 1)[1], 16)\n");
    prepare_script.push_str("            except ValueError:\n");
    prepare_script.push_str("                continue\n");
    prepare_script.push_str("            if local_port == port:\n");
    prepare_script.push_str("                found.add(cols[9])\n");
    prepare_script.push_str("    return found\n");
    prepare_script.push_str("def pids_for(inodes):\n");
    prepare_script.push_str("    pids = set()\n");
    prepare_script.push_str("    for pid in os.listdir('/proc'):\n");
    prepare_script.push_str("        if not pid.isdigit():\n");
    prepare_script.push_str("            continue\n");
    prepare_script.push_str("        fd_dir = f'/proc/{pid}/fd'\n");
    prepare_script.push_str("        try:\n");
    prepare_script.push_str("            fds = os.listdir(fd_dir)\n");
    prepare_script.push_str("        except OSError:\n");
    prepare_script.push_str("            continue\n");
    prepare_script.push_str("        for fd in fds:\n");
    prepare_script.push_str("            try:\n");
    prepare_script.push_str("                target = os.readlink(f'{fd_dir}/{fd}')\n");
    prepare_script.push_str("            except OSError:\n");
    prepare_script.push_str("                continue\n");
    prepare_script
        .push_str("            if target.startswith('socket:[') and target[8:-1] in inodes:\n");
    prepare_script.push_str("                pids.add(int(pid))\n");
    prepare_script.push_str("                break\n");
    prepare_script.push_str("    return pids\n");
    prepare_script.push_str("for sig in (signal.SIGTERM, signal.SIGKILL):\n");
    prepare_script.push_str("    victims = pids_for(listener_inodes())\n");
    prepare_script.push_str("    if not victims:\n");
    prepare_script.push_str("        break\n");
    prepare_script.push_str("    for pid in victims:\n");
    prepare_script.push_str("        if pid == 1:\n");
    prepare_script.push_str("            continue\n");
    prepare_script.push_str("        try:\n");
    prepare_script.push_str(
        "            comm = open(f'/proc/{pid}/comm', encoding='utf-8').read().strip()\n",
    );
    prepare_script.push_str("        except OSError:\n");
    prepare_script.push_str("            comm = 'unknown'\n");
    prepare_script.push_str("        print(f'Inrou prepare: terminating {comm} pid {pid} on port {port}', flush=True)\n");
    prepare_script.push_str("        try:\n");
    prepare_script.push_str("            os.kill(pid, sig)\n");
    prepare_script.push_str("        except ProcessLookupError:\n");
    prepare_script.push_str("            pass\n");
    prepare_script.push_str("    time.sleep(0.5)\n");
    prepare_script.push_str("PY\n");
    prepare_script.push_str("fi\n");
    prepare_script.push_str("echo 'Inrou prepare: completed'\n");

    let mut launcher_script = String::from("#!/bin/sh\nset -eu\n");
    launcher_script.push_str(
        "if [ -w /dev/console ]; then exec >>/dev/console 2>&1; fi\n\
         echo 'Inrou launcher: starting'\n",
    );
    launcher_script
        .push_str("mkdir -p /var/lib/soracloud/service /var/lib/soracloud/materialization\n");
    for (key, value) in &cache_key.effective_env {
        launcher_script.push_str("export ");
        launcher_script.push_str(key);
        launcher_script.push('=');
        launcher_script.push_str(&shell_single_quote(value));
        launcher_script.push('\n');
    }
    launcher_script.push_str("export PORT=");
    launcher_script.push_str(&shell_single_quote(&guest_port.to_string()));
    launcher_script.push('\n');
    let launcher_entrypoint = portable_bundle_hash.map_or_else(
        || cache_key.entrypoint.clone(),
        |_| {
            format!(
                "{}/{}",
                INROU_PORTABLE_BUNDLE_GUEST_ROOT,
                strip_leading_slashes(&cache_key.entrypoint)
            )
        },
    );
    launcher_script.push_str("exec ");
    launcher_script.push_str(&shell_single_quote(&launcher_entrypoint));
    for arg in &cache_key.args {
        launcher_script.push(' ');
        launcher_script.push_str(&shell_single_quote(arg));
    }
    launcher_script.push('\n');

    let mut service_unit = String::new();
    service_unit.push_str("[Unit]\n");
    service_unit.push_str("Description=Soracloud Inrou service\n");
    service_unit.push_str("After=network-online.target\n");
    service_unit.push_str("Wants=network-online.target\n\n");
    service_unit.push_str("[Service]\n");
    service_unit.push_str("PermissionsStartOnly=true\n");
    service_unit.push_str("Type=simple\n");
    service_unit.push_str("User=inrou\n");
    service_unit.push_str("Group=inrou\n");
    service_unit.push_str("Restart=always\n");
    service_unit.push_str("RestartSec=2\n");
    service_unit.push_str("StandardOutput=journal+console\n");
    service_unit.push_str("StandardError=journal+console\n");
    service_unit.push_str("ExecStartPre=/usr/local/bin/inrou-prepare.sh\n");
    service_unit.push_str("ExecStart=/usr/local/bin/inrou-launch.sh\n\n");
    service_unit.push_str("[Install]\n");
    service_unit.push_str("WantedBy=multi-user.target\n");

    let mut user_data = String::from("#cloud-config\n");
    user_data.push_str("ssh_pwauth: false\n");
    user_data.push_str("users:\n");
    user_data.push_str("  - name: inrou\n");
    user_data.push_str("    gecos: Inrou Tenant\n");
    user_data.push_str("    uid: 1000\n");
    user_data.push_str("    groups: [sudo]\n");
    user_data.push_str("    sudo: [\"ALL=(ALL) NOPASSWD:ALL\"]\n");
    user_data.push_str("    shell: /bin/bash\n");
    user_data.push_str("    lock_passwd: true\n");
    user_data.push_str("    ssh_authorized_keys:\n");
    for key in plan
        .inrou
        .as_ref()
        .map(|inrou| inrou.ssh_authorized_keys.as_slice())
        .unwrap_or(&[])
    {
        user_data.push_str("      - ");
        user_data.push_str(&yaml_single_quote(key));
        user_data.push('\n');
    }
    user_data.push_str("write_files:\n");
    user_data.push_str("  - path: /usr/local/bin/inrou-prepare.sh\n");
    user_data.push_str("    owner: root:root\n");
    user_data.push_str("    permissions: '0755'\n");
    user_data.push_str("    content: |\n");
    user_data.push_str(&yaml_block_literal(&prepare_script, 6));
    user_data.push_str("  - path: /usr/local/bin/inrou-launch.sh\n");
    user_data.push_str("    owner: root:root\n");
    user_data.push_str("    permissions: '0755'\n");
    user_data.push_str("    content: |\n");
    user_data.push_str(&yaml_block_literal(&launcher_script, 6));
    user_data.push_str("  - path: /etc/systemd/system/inrou-app.service\n");
    user_data.push_str("    owner: root:root\n");
    user_data.push_str("    permissions: '0644'\n");
    user_data.push_str("    content: |\n");
    user_data.push_str(&yaml_block_literal(&service_unit, 6));
    if let Some(hosts_overlay) = allowlist_hosts_overlay {
        user_data.push_str("  - path: /etc/soracloud/allowlist-hosts\n");
        user_data.push_str("    owner: root:root\n");
        user_data.push_str("    permissions: '0644'\n");
        user_data.push_str("    content: |\n");
        user_data.push_str(&yaml_block_literal(hosts_overlay, 6));
    }
    user_data.push_str("runcmd:\n");
    user_data
        .push_str("  - mkdir -p /var/lib/soracloud/service /var/lib/soracloud/materialization\n");
    user_data.push_str("  - systemctl daemon-reload\n");
    user_data.push_str("  - systemctl enable --now inrou-app.service\n");
    if let Some(overlay) = bootstrap_user_data_overlay {
        user_data.push('\n');
        user_data.push_str(overlay);
        if !overlay.ends_with('\n') {
            user_data.push('\n');
        }
    }
    user_data
}

fn yaml_block_literal(contents: &str, indent: usize) -> String {
    let padding = " ".repeat(indent);
    let mut output = String::new();
    for line in contents.lines() {
        output.push_str(&padding);
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn yaml_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "linux")]
fn collect_host_nameservers() -> Vec<String> {
    let mut nameservers = fs::read_to_string("/etc/resolv.conf")
        .ok()
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if let Some(value) = line.strip_prefix("nameserver ") {
                        let value = value.trim();
                        if value.is_empty() {
                            None
                        } else {
                            Some(value.to_owned())
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if nameservers.is_empty() {
        nameservers.push("1.1.1.1".to_owned());
        nameservers.push("8.8.8.8".to_owned());
    }
    nameservers.truncate(3);
    nameservers
}

fn sanitize_env_var_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_uppercase());
        } else {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        "VOLUME".to_owned()
    } else {
        sanitized
    }
}

fn strip_leading_slashes(path: &str) -> &str {
    path.trim_start_matches('/')
}

#[derive(Debug)]
struct InrouGuestImageHostPaths {
    kernel: PathBuf,
    rootfs: PathBuf,
    initrd: Option<PathBuf>,
    source: String,
}

fn inrou_guest_image_host_paths_from_env(
    selected_guest_isa: SoraInrouGuestIsaV1,
    image: &SoraInrouGuestImageV1,
) -> eyre::Result<Option<InrouGuestImageHostPaths>> {
    let isa_prefix = match selected_guest_isa {
        SoraInrouGuestIsaV1::X8664 => "X86_64",
        SoraInrouGuestIsaV1::Aarch64 => "AARCH64",
    };
    let mut prefixes = vec![
        format!("IROHA_INROU_{isa_prefix}"),
        format!("HAYAHI_TAIRA_INROU_{isa_prefix}"),
    ];
    if selected_guest_isa == current_host_inrou_guest_isa() {
        prefixes.push("IROHA_INROU_PORTABLE".to_owned());
    }

    for prefix in prefixes {
        let kernel_var = format!("{prefix}_KERNEL_IMAGE");
        let rootfs_var = format!("{prefix}_ROOTFS_IMAGE");
        let initrd_var = format!("{prefix}_INITRD_IMAGE");
        let configured = [&kernel_var, &rootfs_var, &initrd_var]
            .iter()
            .any(|name| std::env::var_os(name).is_some());
        if !configured {
            continue;
        }

        let kernel = required_inrou_guest_image_env_path(&kernel_var)?;
        let rootfs = required_inrou_guest_image_env_path(&rootfs_var)?;
        let initrd = if image.initrd_image_path.is_some() {
            Some(required_inrou_guest_image_env_path(&initrd_var)?)
        } else {
            optional_inrou_guest_image_env_path(&initrd_var)?
        };

        return Ok(Some(InrouGuestImageHostPaths {
            kernel,
            rootfs,
            initrd,
            source: prefix,
        }));
    }

    Ok(None)
}

fn required_inrou_guest_image_env_path(name: &str) -> eyre::Result<PathBuf> {
    optional_inrou_guest_image_env_path(name)?
        .ok_or_else(|| eyre::eyre!("required Inrou guest asset env var {name} is not set"))
}

fn optional_inrou_guest_image_env_path(name: &str) -> eyre::Result<Option<PathBuf>> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_file() {
        eyre::bail!(
            "Inrou guest asset env var {name} points to missing file {}",
            path.display()
        );
    }
    Ok(Some(path))
}

fn materialize_inrou_guest_image_from_host_paths(
    bundle_root: &Path,
    image: &SoraInrouGuestImageV1,
    host_paths: &InrouGuestImageHostPaths,
) -> eyre::Result<()> {
    copy_inrou_guest_image_member(
        &host_paths.kernel,
        bundle_root,
        &image.kernel_image_path,
        "kernel",
    )?;
    copy_inrou_guest_image_member(
        &host_paths.rootfs,
        bundle_root,
        &image.rootfs_image_path,
        "rootfs",
    )?;
    if let Some((source, destination)) = host_paths
        .initrd
        .as_ref()
        .zip(image.initrd_image_path.as_deref())
    {
        copy_inrou_guest_image_member(source, bundle_root, destination, "initrd")?;
    }
    Ok(())
}

fn copy_inrou_guest_image_member(
    source: &Path,
    bundle_root: &Path,
    declared_path: &str,
    label: &str,
) -> eyre::Result<()> {
    let destination = bundle_root.join(strip_leading_slashes(declared_path));
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    fs::copy(source, &destination).wrap_err_with(|| {
        format!(
            "copy Inrou {label} image {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn ensure_native_bundle_extracted(
    bundle_cache_path: &Path,
    bundle_hash: &str,
    bundle_root: &Path,
) -> eyre::Result<()> {
    let stamp_path = bundle_root.join(".bundle_hash");
    if read_json_optional::<String>(&stamp_path)
        .ok()
        .flatten()
        .is_some_and(|value| value == bundle_hash)
    {
        return Ok(());
    }

    reset_directory(bundle_root).wrap_err_with(|| format!("reset {}", bundle_root.display()))?;
    let tar = resolve_executable_on_path("tar").unwrap_or_else(|| PathBuf::from("tar"));
    let result = Command::new(&tar)
        .arg("-xzf")
        .arg(bundle_cache_path)
        .arg("-C")
        .arg(bundle_root)
        .status()
        .wrap_err_with(|| {
            format!(
                "spawn {} for {}",
                tar.display(),
                bundle_cache_path.display()
            )
        })?;
    if !result.success() {
        eyre::bail!(
            "extract native bundle {} into {} failed with status {result}",
            bundle_cache_path.display(),
            bundle_root.display(),
        );
    }
    write_json_atomic(&stamp_path, &bundle_hash.to_owned())
        .wrap_err_with(|| format!("write {}", stamp_path.display()))?;
    Ok(())
}

fn ensure_inrou_entrypoint_present(bundle_root: &Path, entrypoint: &str) -> eyre::Result<()> {
    let entrypoint_path = bundle_root.join(strip_leading_slashes(entrypoint));
    if !entrypoint_path.exists() {
        eyre::bail!(
            "Inrou bundle does not contain entrypoint `{}` under {}",
            entrypoint,
            bundle_root.display(),
        );
    }
    #[cfg(unix)]
    {
        let metadata = fs::metadata(&entrypoint_path)
            .wrap_err_with(|| format!("stat {}", entrypoint_path.display()))?;
        if !metadata.is_file() {
            eyre::bail!(
                "Inrou entrypoint `{}` must resolve to a regular file under {}",
                entrypoint,
                bundle_root.display(),
            );
        }
        if metadata.permissions().mode() & 0o111 == 0 {
            eyre::bail!(
                "Inrou entrypoint `{}` is not executable inside {}",
                entrypoint,
                bundle_root.display(),
            );
        }
    }
    Ok(())
}

fn resolve_executable_on_path(program: &str) -> Option<PathBuf> {
    if program.contains(std::path::MAIN_SEPARATOR) {
        let candidate = PathBuf::from(program);
        return candidate.exists().then_some(candidate);
    }
    let mut search_roots = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    search_roots.extend(known_host_executable_directories());
    search_roots.sort();
    search_roots.dedup();

    for directory in search_roots {
        for candidate_name in executable_candidate_names(program) {
            let candidate = directory.join(candidate_name);
            if is_resolved_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_executable_candidates(programs: &[&str]) -> Option<PathBuf> {
    programs
        .iter()
        .find_map(|program| resolve_executable_on_path(program))
}

fn executable_candidate_names(program: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let path = Path::new(program);
        let has_extension = path.extension().is_some();
        let mut names = vec![program.to_owned()];
        if !has_extension {
            let pathext =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
            for extension in pathext.split(';').filter(|value| !value.is_empty()) {
                names.push(format!("{program}{extension}"));
            }
        }
        names
    }
    #[cfg(not(windows))]
    {
        vec![program.to_owned()]
    }
}

fn known_host_executable_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    #[cfg(not(windows))]
    {
        directories.push(PathBuf::from("/opt/homebrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    }

    #[cfg(windows)]
    {
        for env_var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(env_var) {
                directories.push(PathBuf::from(root).join("qemu"));
                directories.push(PathBuf::from(root).join("QEMU"));
            }
        }
        directories.push(PathBuf::from(r"C:\Program Files\qemu"));
        directories.push(PathBuf::from(r"C:\Program Files\QEMU"));
        directories.push(PathBuf::from(r"C:\Program Files (x86)\qemu"));
        directories.push(PathBuf::from(r"C:\Program Files (x86)\QEMU"));
        directories.push(PathBuf::from(r"C:\msys64\ucrt64\bin"));
        directories.push(PathBuf::from(r"C:\msys64\mingw64\bin"));
    }

    if let Some(android_sdk_root) = std::env::var_os("ANDROID_SDK_ROOT") {
        directories.push(PathBuf::from(android_sdk_root.clone()).join("emulator/bin64"));
        directories.push(PathBuf::from(android_sdk_root).join("emulator"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join("Library/Android/sdk/emulator"));
        directories.push(home.join("Library/Android/sdk/emulator/bin64"));
    }
    directories
}

#[allow(dead_code)]
fn known_host_executable_paths(program: &str) -> Vec<PathBuf> {
    known_host_executable_directories()
        .into_iter()
        .flat_map(|directory| {
            executable_candidate_names(program)
                .into_iter()
                .map(move |candidate| directory.join(candidate))
        })
        .collect()
}

fn is_resolved_executable(candidate: &Path) -> bool {
    if !candidate.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        return fs::metadata(candidate)
            .ok()
            .is_some_and(|metadata| metadata.permissions().mode() & 0o111 != 0);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[allow(dead_code)]
fn append_ro_bind_try(command: &mut Command, host_path: &str) {
    if Path::new(host_path).exists() {
        command.arg("--ro-bind");
        command.arg(host_path);
        command.arg(host_path);
    }
}

fn probe_hosted_http_health(
    listen_base_url: &str,
    healthcheck_path: Option<&str>,
) -> eyre::Result<()> {
    let Some(path) = healthcheck_path else {
        return Ok(());
    };
    let request_path = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    let url = format!("{listen_base_url}{request_path}");
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .wrap_err("build hosted-HTTP healthcheck client")?
        .get(&url)
        .send()
        .wrap_err_with(|| format!("probe hosted-HTTP healthcheck {url}"))?;
    if !response.status().is_success() {
        eyre::bail!(
            "hosted-HTTP healthcheck {url} returned {}",
            response.status()
        );
    }
    Ok(())
}

#[cfg(test)]
fn fetch_hosted_http_text(listen_base_url: &str, path: &str) -> eyre::Result<String> {
    let request_path = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    let url = format!("{listen_base_url}{request_path}");
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .wrap_err("build hosted-HTTP client")?
        .get(&url)
        .send()
        .wrap_err_with(|| format!("fetch hosted-HTTP response {url}"))?;
    if !response.status().is_success() {
        eyre::bail!("hosted-HTTP request {url} returned {}", response.status());
    }
    response
        .text()
        .wrap_err_with(|| format!("read hosted-HTTP response body {url}"))
}

fn remote_hydration_nonce(
    manifest_cid_hex: &str,
    provider_id: &[u8; 32],
    expected_hash: Hash,
) -> String {
    let manifest_prefix = manifest_cid_hex
        .chars()
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    let provider_prefix = hex::encode(provider_id).chars().take(8).collect::<String>();
    let hash_prefix = expected_hash
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("sc-{manifest_prefix}-{provider_prefix}-{hash_prefix}")
}

fn parse_remote_hydration_plan(
    manifest_id_hex: &str,
    body: &[u8],
) -> eyre::Result<RemoteHydrationPlan> {
    let value: norito::json::Value =
        norito::json::from_slice(body).wrap_err("decode remote plan response as JSON")?;
    let plan = value
        .get("plan")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `plan` object"))?;
    let chunker_handle = plan
        .get("chunk_profile_handle")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `chunk_profile_handle`"))?
        .to_owned();
    let content_length = plan
        .get("content_length")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `content_length`"))?;
    let chunks_value = plan
        .get("chunks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `chunks` array"))?;
    if chunks_value.is_empty() && content_length != 0 {
        return Err(eyre::eyre!(
            "remote plan response returned zero chunks for non-empty payload"
        ));
    }

    let mut chunks = Vec::with_capacity(chunks_value.len());
    for (index, chunk) in chunks_value.iter().enumerate() {
        let chunk = chunk
            .as_object()
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} is not an object"))?;
        let offset = chunk
            .get("offset")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `offset`"))?;
        let length = chunk
            .get("length")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `length`"))?;
        let digest_hex = chunk
            .get("digest_blake3")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `digest_blake3`"))?
            .to_ascii_lowercase();
        let digest_bytes = hex::decode(&digest_hex)
            .wrap_err_with(|| format!("decode remote plan chunk {index} digest"))?;
        if digest_bytes.len() != 32 {
            return Err(eyre::eyre!(
                "remote plan chunk {index} digest must decode to 32 bytes"
            ));
        }
        let length = u32::try_from(length)
            .wrap_err_with(|| format!("convert remote plan chunk {index} length to u32"))?;
        chunks.push(RemoteHydrationChunk {
            offset,
            length,
            digest_hex,
        });
    }

    Ok(RemoteHydrationPlan {
        manifest_id_hex: manifest_id_hex.to_owned(),
        chunker_handle,
        content_length,
        chunks,
    })
}

fn parse_sorafs_manifest_digest_hex(raw: &str) -> eyre::Result<[u8; 32]> {
    let bytes = hex::decode(raw.trim()).wrap_err("decode SoraFS manifest digest hex")?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
        eyre::eyre!(
            "SoraFS manifest digest hex must decode to 32 bytes, got {}",
            bytes.len()
        )
    })
}

fn storage_file_dto_layout(file: &StorageStoredFileDto) -> eyre::Result<SorafsHydratedFileLayout> {
    Ok(SorafsHydratedFileLayout {
        path: file.path.clone(),
        offset: file.offset,
        size: file.size,
    })
}

fn materialize_sorafs_payload_files(
    payload: &[u8],
    files: &[SorafsHydratedFileLayout],
    target_root: &Path,
) -> eyre::Result<()> {
    if files.is_empty() {
        eyre::bail!("published SoraFS directory artifact did not declare any files");
    }

    for file in files {
        let start = usize::try_from(file.offset).wrap_err_with(|| {
            format!(
                "convert published SoraFS file `{}` offset to usize",
                file.path.join("/")
            )
        })?;
        let size = usize::try_from(file.size).wrap_err_with(|| {
            format!(
                "convert published SoraFS file `{}` size to usize",
                file.path.join("/")
            )
        })?;
        let end = start.checked_add(size).ok_or_else(|| {
            eyre::eyre!(
                "published SoraFS file `{}` range overflows host usize",
                file.path.join("/")
            )
        })?;
        if end > payload.len() {
            eyre::bail!(
                "published SoraFS file `{}` range {}..{} exceeds payload length {}",
                file.path.join("/"),
                start,
                end,
                payload.len()
            );
        }
        let target = sorafs_hydrated_file_target(target_root, &file.path)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
        }
        write_bytes_atomic(&target, &payload[start..end])
            .wrap_err_with(|| format!("write {}", target.display()))?;
    }
    Ok(())
}

fn sorafs_hydrated_file_target(root: &Path, components: &[String]) -> eyre::Result<PathBuf> {
    if components.is_empty() {
        eyre::bail!("published SoraFS directory artifact file path must not be empty");
    }
    let mut path = root.to_path_buf();
    for component in components {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains('/')
            || component.contains('\\')
        {
            eyre::bail!(
                "published SoraFS directory artifact contains unsafe path component `{component}`"
            );
        }
        path.push(component);
    }
    Ok(path)
}

fn collect_remote_hydration_sources(
    view: &StateView<'_>,
    state: &State,
) -> Vec<RemoteHydrationSource> {
    let mut sources = BTreeMap::<(u8, u64, String, String), RemoteHydrationSource>::new();
    for (_order_id, record) in view.world().replication_orders().iter() {
        if !manifest_is_committed(view, state, record.manifest_digest.as_bytes()) {
            continue;
        }
        let order = match norito::decode_from_bytes::<ReplicationOrderV1>(&record.canonical_order) {
            Ok(order) => order,
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    manifest_digest = %hex::encode(record.manifest_digest.as_bytes()),
                    "failed to decode canonical SoraFS replication order during Soracloud hydration"
                );
                continue;
            }
        };
        if order.manifest_cid.is_empty() || order.providers.is_empty() {
            continue;
        }

        let manifest_digest_hex = hex::encode(record.manifest_digest.as_bytes());
        let manifest_cid_hex = hex::encode(&order.manifest_cid);
        let chunker_handle = view
            .world()
            .pin_manifests()
            .get(&record.manifest_digest)
            .map(|manifest| manifest.chunker.to_handle());
        let status_rank = match record.status {
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Completed(_) => 0,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Pending => 1,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Expired(_) => 2,
        };
        let key = (
            status_rank,
            record.issued_epoch,
            manifest_digest_hex.clone(),
            manifest_cid_hex.clone(),
        );
        let entry = sources.entry(key).or_insert_with(|| RemoteHydrationSource {
            manifest_digest_hex,
            manifest_cid_hex,
            chunker_handle,
            provider_ids: Vec::new(),
        });
        for provider_id in order.providers {
            if !entry.provider_ids.contains(&provider_id) {
                entry.provider_ids.push(provider_id);
            }
        }
        entry.provider_ids.sort();
    }

    sources.into_values().collect()
}

fn manifest_is_committed(view: &StateView<'_>, state: &State, manifest_digest: &[u8; 32]) -> bool {
    let digest = ManifestDigest::new(*manifest_digest);
    let has_active_pin = view
        .world()
        .pin_manifests()
        .get(&digest)
        .is_some_and(|record| record.status.is_active());
    has_active_pin || state.find_da_commitment_by_manifest(&digest).is_some()
}

fn sanitize_path_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect()
}

fn build_effective_service_environment(
    bundle: &SoraDeploymentBundleV1,
    deployment: &SoraServiceDeploymentStateV1,
) -> eyre::Result<BTreeMap<String, String>> {
    let mut effective_env = bundle.container.env.clone();
    for export in &bundle.container.config_exports {
        let entry = deployment
            .service_configs
            .get(export.config_name())
            .ok_or_else(|| {
                eyre::eyre!(
                    "service `{}` revision `{}` config export references missing authoritative config `{}`",
                    bundle.service.service_name,
                    bundle.service.service_version,
                    export.config_name()
                )
            })?;
        if let SoraConfigExportTargetV1::Env(var_name) = &export.target {
            effective_env.insert(var_name.clone(), entry.value_json.get().clone());
        }
    }
    Ok(effective_env)
}

fn sanitized_relative_export_path(relative_path: &str) -> Result<PathBuf, ()> {
    if relative_path.is_empty()
        || relative_path.starts_with('/')
        || relative_path.ends_with('/')
        || relative_path.contains('\\')
    {
        return Err(());
    }
    let mut path = PathBuf::new();
    for component in relative_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(());
        }
        path.push(sanitize_path_component(component));
    }
    Ok(path)
}

fn prune_nested_directory_tree(
    root: &Path,
    desired: &BTreeMap<String, BTreeSet<String>>,
) -> eyre::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for service_entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let service_entry = service_entry?;
        if !service_entry.file_type()?.is_dir() {
            continue;
        }
        let service_name = service_entry.file_name().to_string_lossy().into_owned();
        let service_path = service_entry.path();
        let Some(desired_versions) = desired.get(&service_name) else {
            fs::remove_dir_all(&service_path)
                .wrap_err_with(|| format!("remove stale {}", service_path.display()))?;
            continue;
        };
        for version_entry in fs::read_dir(&service_path)
            .wrap_err_with(|| format!("read {}", service_path.display()))?
        {
            let version_entry = version_entry?;
            if !version_entry.file_type()?.is_dir() {
                continue;
            }
            let version_name = version_entry.file_name().to_string_lossy().into_owned();
            if !desired_versions.contains(&version_name) {
                let version_path = version_entry.path();
                fs::remove_dir_all(&version_path)
                    .wrap_err_with(|| format!("remove stale {}", version_path.display()))?;
            }
        }
        let mut remaining = fs::read_dir(&service_path)?;
        if remaining.next().is_none() {
            fs::remove_dir_all(&service_path)
                .wrap_err_with(|| format!("remove empty {}", service_path.display()))?;
        }
    }
    Ok(())
}

fn prune_flat_directory_tree(root: &Path, desired: &BTreeSet<String>) -> eyre::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !desired.contains(&name) {
            let path = entry.path();
            fs::remove_dir_all(&path)
                .wrap_err_with(|| format!("remove stale {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_json_atomic<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must have a parent"))?;
    fs::create_dir_all(parent)?;
    let payload = norito::json::to_json(value)
        .map_err(|error| io::Error::other(format!("serialize json: {error}")))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, payload)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must have a parent"))?;
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn reset_directory(root: &Path) -> io::Result<()> {
    match fs::remove_dir_all(root) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(root)?;
    Ok(())
}

fn write_service_config_materializations(
    version_dir: &Path,
    config_root: &Path,
    config_exports_root: &Path,
    effective_env_path: &Path,
    plan: &SoracloudRuntimeServicePlan,
    deployment: &SoraServiceDeploymentStateV1,
) -> eyre::Result<()> {
    reset_directory(config_root).wrap_err_with(|| format!("reset {}", config_root.display()))?;
    reset_directory(config_exports_root)
        .wrap_err_with(|| format!("reset {}", config_exports_root.display()))?;
    write_json_atomic(
        &version_dir.join("service_configs.json"),
        &deployment.service_configs,
    )
    .wrap_err_with(|| {
        format!(
            "write {}",
            version_dir.join("service_configs.json").display()
        )
    })?;
    write_json_atomic(effective_env_path, &plan.effective_env)
        .wrap_err_with(|| format!("write {}", effective_env_path.display()))?;

    for (config_name, entry) in &deployment.service_configs {
        let relative_path = sanitized_relative_material_path(config_name).map_err(|_| {
            eyre::eyre!("invalid authoritative service config name `{config_name}`")
        })?;
        write_bytes_atomic(
            &config_root.join(relative_path),
            entry.value_json.get().as_bytes(),
        )
        .wrap_err_with(|| {
            format!(
                "write materialized config `{config_name}` under {}",
                config_root.display()
            )
        })?;
    }
    for export in &plan.config_exports {
        let SoraConfigExportTargetV1::File(relative_path) = &export.target else {
            continue;
        };
        let entry = deployment
            .service_configs
            .get(export.config_name())
            .ok_or_else(|| {
                eyre::eyre!(
                    "service config export `{}` references missing authoritative config `{}`",
                    export.target_identifier(),
                    export.config_name()
                )
            })?;
        let relative_path = sanitized_relative_export_path(relative_path).map_err(|_| {
            eyre::eyre!(
                "invalid service config export file path `{relative_path}` for config `{}`",
                export.config_name()
            )
        })?;
        write_bytes_atomic(
            &config_exports_root.join(relative_path),
            entry.value_json.get().as_bytes(),
        )
        .wrap_err_with(|| {
            format!(
                "write exported config `{}` under {}",
                export.config_name(),
                config_exports_root.display()
            )
        })?;
    }
    Ok(())
}

fn write_service_secret_materializations(
    version_dir: &Path,
    secret_envelopes_root: &Path,
    secret_payload_root: &Path,
    deployment: &SoraServiceDeploymentStateV1,
) -> eyre::Result<()> {
    reset_directory(secret_envelopes_root)
        .wrap_err_with(|| format!("reset {}", secret_envelopes_root.display()))?;
    reset_directory(secret_payload_root)
        .wrap_err_with(|| format!("reset {}", secret_payload_root.display()))?;
    write_json_atomic(
        &version_dir.join("service_secret_envelopes.json"),
        &deployment.service_secrets,
    )
    .wrap_err_with(|| {
        format!(
            "write {}",
            version_dir.join("service_secret_envelopes.json").display()
        )
    })?;

    for (secret_name, entry) in &deployment.service_secrets {
        let relative_path = sanitized_relative_material_path(secret_name).map_err(|_| {
            eyre::eyre!("invalid authoritative service secret name `{secret_name}`")
        })?;
        write_json_atomic(&secret_envelopes_root.join(&relative_path), entry).wrap_err_with(
            || {
                format!(
                    "write materialized secret envelope `{secret_name}` under {}",
                    secret_envelopes_root.display()
                )
            },
        )?;
        write_bytes_atomic(
            &secret_payload_root.join(relative_path),
            &entry.envelope.ciphertext,
        )
        .wrap_err_with(|| {
            format!(
                "write materialized secret payload `{secret_name}` under {}",
                secret_payload_root.display()
            )
        })?;
    }
    Ok(())
}

fn read_json_optional<T>(path: &Path) -> io::Result<Option<T>>
where
    T: norito::json::JsonDeserialize,
{
    let payload = match fs::read(path) {
        Ok(payload) => payload,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if payload.is_empty() {
        return Ok(None);
    }
    norito::json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::other(format!("deserialize json: {error}")))
}

#[cfg(test)]
mod tests {
    //! Tests for the embedded Soracloud runtime manager.

    use super::*;
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        num::NonZeroU64,
        sync::{Arc, Mutex, mpsc},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use eyre::Result;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_crypto::{Algorithm, PrivateKey, PublicKey, Signature};
    use iroha_data_model::asset::AssetDefinitionId;
    use iroha_data_model::{
        block::BlockHeader,
        metadata::Metadata,
        smart_contract::manifest::EntryPointKind,
        soracloud::{
            AgentApartmentManifestV1, SECRET_ENVELOPE_VERSION_V1,
            SORA_AGENT_APARTMENT_RECORD_VERSION_V1, SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            SORA_HF_SOURCE_RECORD_VERSION_V1, SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            SORA_SERVICE_ROLLOUT_STATE_VERSION_V1, SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            SecretEnvelopeEncryptionV1, SecretEnvelopeV1, SoraAgentArtifactAllowRuleV1,
            SoraAgentAutonomyRunRecordV1, SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1,
            SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraHfBackendFamilyV1,
            SoraHfModelFormatV1, SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1,
            SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1, SoraHfPlacementStatusV1,
            SoraHfResourceProfileV1, SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseMemberV1,
            SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1, SoraHfSourceRecordV1,
            SoraHfSourceStatusV1, SoraInrouGuestImageV1, SoraModelHostCapabilityRecordV1,
            SoraRolloutStageV1, SoraRouteVisibilityV1, SoraServiceConfigEntryV1,
            SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHealthStatusV1,
            SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1, SoraServiceRuntimeStateV1,
            SoraServiceSecretEntryV1,
        },
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, PinFeePayment, PinManifestRecord, PinPolicy,
            ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus,
        },
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
    use iroha_torii::sorafs::AdmissionRegistry;
    use serial_test::serial;
    use sorafs_car::CarBuildPlan;
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{
        AdvertEndpoint, AvailabilityTier, BLAKE3_256_MULTIHASH_CODE, CapabilityTlv, CapabilityType,
        CouncilSignature, DagCodecId, EndpointAdmissionV1, EndpointAttestationKind,
        EndpointAttestationV1, EndpointMetadata, EndpointMetadataKey, ManifestBuilder,
        PROVIDER_ADMISSION_ENVELOPE_VERSION_V1, PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
        PROVIDER_ADVERT_VERSION_V1, PathDiversityPolicy, PinPolicy as ManifestPinPolicy,
        ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdvertBodyV1,
        ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints, RendezvousTopic, SignatureAlgorithm,
        StakePointer, StreamBudgetV1, TransportHintV1, compute_advert_body_digest,
        compute_proposal_digest,
    };

    #[test]
    fn runtime_error_summary_includes_nested_causes() {
        let error = eyre::eyre!("serial console: missing python3")
            .wrap_err("Inrou PortableVm failed healthcheck during startup")
            .wrap_err("start inrou Soracloud service `hayahi_live` revision `v1` replica 1");

        let summary = runtime_error_summary(&error);

        assert!(summary.contains("start inrou Soracloud service"));
        assert!(summary.contains("Inrou PortableVm failed healthcheck during startup"));
        assert!(summary.contains("serial console: missing python3"));
    }
    use sorafs_node::{NodeHandle, config::StorageConfig};

    #[test]
    fn local_read_snapshot_allows_bounded_lag_but_rejects_wrong_tip() {
        let committed = Hash::prehashed([0x11; Hash::LENGTH]);
        let stale = Hash::prehashed([0x22; Hash::LENGTH]);

        assert!(local_read_snapshot_covers_committed_state(
            100,
            Some(committed),
            100,
            Some(committed),
        ));
        assert!(!local_read_snapshot_covers_committed_state(
            100,
            Some(stale),
            100,
            Some(committed),
        ));
        assert!(local_read_snapshot_covers_committed_state(
            99,
            Some(stale),
            100,
            Some(committed),
        ));
        assert!(!local_read_snapshot_covers_committed_state(
            100_u64.saturating_sub(SORACLOUD_LOCAL_READ_MAX_SNAPSHOT_LAG_BLOCKS + 1),
            Some(stale),
            100,
            Some(committed),
        ));
        assert!(!local_read_snapshot_covers_committed_state(
            101,
            Some(stale),
            100,
            Some(committed),
        ));
    }

    fn load_deployment_bundle_fixture() -> Result<SoraDeploymentBundleV1> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/sora_deployment_bundle_v1.json");
        let raw = fs::read_to_string(path)?;
        Ok(norito::json::from_str(&raw)?)
    }

    fn load_agent_manifest_fixture() -> Result<AgentApartmentManifestV1> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/agent_apartment_manifest_v1.json");
        let raw = fs::read_to_string(path)?;
        Ok(norito::json::from_str(&raw)?)
    }

    fn test_state() -> Result<Arc<State>> {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        Ok(Arc::new(State::new_for_testing(World::new(), kura, query)))
    }

    #[test]
    fn soracloud_runtime_submission_metadata_uses_explicit_gas_asset() -> Result<()> {
        let state = test_state()?;

        let metadata =
            soracloud_runtime_submission_metadata(state.as_ref(), Some("  configured-gas  "));

        assert_eq!(
            metadata
                .get("gas_asset_id")
                .expect("gas metadata")
                .as_ref()
                .to_string(),
            "configured-gas"
        );
        Ok(())
    }

    #[test]
    fn soracloud_runtime_submission_metadata_omits_missing_gas_asset() -> Result<()> {
        let state = test_state()?;

        let metadata = soracloud_runtime_submission_metadata(state.as_ref(), None);

        assert!(metadata.get("gas_asset_id").is_none());
        Ok(())
    }

    fn sample_hf_resource_profile_for_tests() -> SoraHfResourceProfileV1 {
        SoraHfResourceProfileV1 {
            required_model_bytes: 3 * 1024 * 1024 * 1024,
            backend_family: SoraHfBackendFamilyV1::Transformers,
            model_format: SoraHfModelFormatV1::Safetensors,
            disk_cache_bytes_floor: 4 * 1024 * 1024 * 1024,
            ram_bytes_floor: 4 * 1024 * 1024 * 1024,
            vram_bytes_floor: 0,
        }
    }

    fn sample_agent_record() -> Result<SoraAgentApartmentRecordV1> {
        let manifest = load_agent_manifest_fixture()?;
        Ok(SoraAgentApartmentRecordV1 {
            schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest_hash: Hash::prehashed([0xAA; Hash::LENGTH]),
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: 1,
            lease_started_sequence: 1,
            lease_expires_sequence: 42,
            last_renewed_sequence: 1,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 7,
            process_started_sequence: 1,
            last_active_sequence: 9,
            last_checkpoint_sequence: None,
            checkpoint_count: 0,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 0,
                key_sizes: BTreeMap::new(),
            },
            revoked_policy_capabilities: BTreeSet::from(["wallet.sign".to_string()]),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: 500,
            autonomy_budget_remaining_units: 325,
            artifact_allowlist: BTreeMap::new(),
            autonomy_run_history: Vec::new(),
            manifest,
        })
    }

    fn sample_runtime_state(bundle: &SoraDeploymentBundleV1) -> SoraServiceRuntimeStateV1 {
        SoraServiceRuntimeStateV1 {
            schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            active_service_version: bundle.service.service_version.clone(),
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 425,
            materialized_bundle_hash: bundle.container.bundle_hash,
            rollout_handle: None,
            pending_mailbox_message_count: 3,
            last_receipt_id: None,
        }
    }

    fn sample_deployment_state(bundle: &SoraDeploymentBundleV1) -> SoraServiceDeploymentStateV1 {
        let service_lease = (bundle.service.execution_plane
            == iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService)
            .then_some(iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
                status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                quota_class: "taira-open".to_owned(),
                deployment_deposit_nanos: 1_000_000_000,
                prepaid_runtime_balance_nanos: 50_000_000_000,
                runtime_nanos_per_sequence: 250_000,
                storage_nanos_per_gib_sequence: 25_000,
                egress_nanos_per_mib: 5_000,
                lease_started_sequence: 0,
                lease_expires_sequence: 100,
                last_billed_sequence: 0,
                accounted_egress_bytes: 0,
                last_status_reason: None,
            });
        SoraServiceDeploymentStateV1 {
            schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            current_service_version: bundle.service.service_version.clone(),
            current_service_manifest_hash: bundle.service.container.manifest_hash,
            current_container_manifest_hash: bundle.service.container.manifest_hash,
            revision_count: 1,
            process_generation: 5,
            process_started_sequence: 11,
            active_rollout: None,
            last_rollout: None,
            config_generation: 0,
            secret_generation: 0,
            service_configs: BTreeMap::new(),
            service_secrets: BTreeMap::new(),
            service_lease,
            lease_volume_states: Vec::new(),
        }
    }

    fn soracloud_entrypoint(name: &str, entry_pc: u64) -> ivm::EmbeddedEntrypointDescriptor {
        ivm::EmbeddedEntrypointDescriptor {
            name: name.to_owned(),
            kind: EntryPointKind::Public,
            params: Vec::new(),
            return_type: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc,
        }
    }

    fn soracloud_contract_artifact_with_words(entrypoints: &[&str], code_words: &[u32]) -> Vec<u8> {
        let metadata = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        };
        let contract_interface = ivm::EmbeddedContractInterfaceV1 {
            compiler_fingerprint: "irohad-soracloud-tests".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: entrypoints
                .iter()
                .map(|name| soracloud_entrypoint(name, 0))
                .collect(),
            states: Vec::new(),
        };
        let mut bytes = metadata.encode();
        bytes.extend_from_slice(&contract_interface.encode_section());
        for word in code_words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    fn simple_soracloud_contract_artifact(entrypoints: &[&str]) -> Vec<u8> {
        soracloud_contract_artifact_with_words(entrypoints, &[ivm::encoding::wide::encode_halt()])
    }

    fn bundle_handler(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
    ) -> iroha_data_model::soracloud::SoraServiceHandlerV1 {
        bundle
            .service
            .handlers
            .iter()
            .find(|handler| handler.handler_name.as_ref() == handler_name)
            .cloned()
            .expect("fixture handler must exist")
    }

    fn sample_mailbox_message(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
        payload_bytes: Vec<u8>,
    ) -> SoraServiceMailboxMessageV1 {
        let payload_commitment = Hash::new(&payload_bytes);
        SoraServiceMailboxMessageV1 {
            schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::new(Encode::encode(&(
                "soracloud.runtime.tests.mailbox",
                bundle.service.service_name.as_ref(),
                handler_name,
                payload_commitment,
            ))),
            from_service: "scheduler".parse().expect("literal name"),
            from_handler: "dispatch".parse().expect("literal name"),
            to_service: bundle.service.service_name.clone(),
            to_handler: handler_name.parse().expect("fixture handler name"),
            payload_bytes,
            payload_commitment,
            enqueue_sequence: 6,
            available_after_sequence: 6,
            expires_at_sequence: None,
        }
    }

    fn sample_ordered_mailbox_request(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
        mailbox_message: SoraServiceMailboxMessageV1,
    ) -> SoracloudOrderedMailboxExecutionRequest {
        SoracloudOrderedMailboxExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            execution_sequence: 7,
            deployment: sample_deployment_state(bundle),
            bundle: bundle.clone(),
            handler: Some(bundle_handler(bundle, handler_name)),
            mailbox_message,
            runtime_state: Some(sample_runtime_state(bundle)),
            authoritative_pending_mailbox_messages: 1,
        }
    }

    fn sample_inrou_test_bundle() -> Result<SoraDeploymentBundleV1> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.container.inrou = Some(iroha_data_model::soracloud::SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: iroha_data_model::soracloud::SoraInrouGuestOsV1::DebianSlim,
            guest_images: BTreeMap::from([
                (
                    SoraInrouGuestIsaV1::X8664,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                        initrd_image_path: Some("/inrou/x86_64/initrd.img".to_owned()),
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    SoraInrouGuestIsaV1::Aarch64,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                        initrd_image_path: Some("/inrou/aarch64/initrd.img".to_owned()),
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: Some("/inrou/bootstrap-user-data.yml".to_owned()),
            ssh_authorized_keys: vec!["ssh-ed25519 AAAATESTKEY soracloud-tests".to_owned()],
        });
        bundle.container.entrypoint = "/bin/sh".to_owned();
        bundle.container.args = vec!["-lc".to_owned(), "echo inrou-test".to_owned()];
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Open;
        bundle.container.lifecycle.start_grace_secs =
            std::num::NonZeroU32::new(180).expect("nonzero");
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.replicas = std::num::NonZeroU16::new(1).expect("replica");
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        bundle.service.artifacts.clear();
        bundle.service.lease_volumes = vec![
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "root_disk".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                mount_path: "/".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024 * 1024).expect("bytes"),
            },
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "index_state".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                mount_path: "/var/lib/ton-indexer".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(128 * 1024 * 1024).expect("bytes"),
            },
        ];
        Ok(bundle)
    }

    fn insert_inrou_service_placement_fixture(
        state: &mut Arc<State>,
        bundle: &SoraDeploymentBundleV1,
        local_peer_id: &str,
        replica_slots: impl IntoIterator<Item = u16>,
    ) {
        let selected_guest_isa = current_host_inrou_guest_isa();
        let placements = replica_slots
            .into_iter()
            .map(|replica_slot| SoraInrouReplicaPlacementV1 {
                replica_slot,
                validator_account_id: ALICE_ID.clone(),
                peer_id: local_peer_id.to_owned(),
                selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                selected_guest_isa,
                selected_geography_tag: None,
                selection_latency_ms: None,
            })
            .collect::<Vec<_>>();
        Arc::get_mut(state)
            .expect("unique test state")
            .world
            .soracloud_inrou_service_placements_mut_for_testing()
            .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements,
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
    }

    fn materialize_inrou_replica_plan_for_tests(
        bundle: &SoraDeploymentBundleV1,
    ) -> Result<(
        tempfile::TempDir,
        SoracloudRuntimeServicePlan,
        HostedHttpWorkerCacheKey,
    )> {
        let mut state = test_state()?;
        let deployment_state = sample_deployment_state(bundle);
        let process_generation = deployment_state.process_generation;
        let local_peer_id = "12D3KooWPortableVmReplicaPlanFixture";
        let selected_guest_isa = current_host_inrou_guest_isa();
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
            world
                .soracloud_inrou_service_placements_mut_for_testing()
                .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let service_dir = temp_dir
            .path()
            .join("services")
            .join(sanitize_path_component(
                bundle.service.service_name.as_ref(),
            ))
            .join(sanitize_path_component(&bundle.service.service_version));
        let replica_plan: SoracloudRuntimeServicePlan = read_json_optional(
            service_dir
                .join("replicas/replica-0001/runtime_plan.json")
                .as_path(),
        )?
        .expect("replica runtime plan");
        let cache_key = HostedHttpWorkerCacheKey {
            runtime: bundle.container.runtime,
            backend: replica_plan
                .inrou
                .as_ref()
                .map(|inrou| inrou.selected_backend),
            guest_isa: replica_plan
                .inrou
                .as_ref()
                .map(|inrou| inrou.selected_guest_isa),
            service_name: bundle.service.service_name.to_string(),
            service_version: bundle.service.service_version.clone(),
            replica_slot: 1,
            bundle_hash: replica_plan.bundle_hash.clone(),
            bundle_path: replica_plan.bundle_path.clone(),
            entrypoint: replica_plan.entrypoint.clone(),
            process_generation,
            args: bundle.container.args.clone(),
            effective_env: replica_plan.effective_env.clone(),
            healthcheck_path: bundle.container.lifecycle.healthcheck_path.clone(),
            service_data_dir: build_native_service_data_dir(
                temp_dir.path(),
                bundle.service.service_name.as_ref(),
            ),
        };
        Ok((temp_dir, replica_plan, cache_key))
    }

    fn wait_for_hosted_http_runtime_state_to_be_healthy(
        manager: &SoracloudRuntimeManager,
        service_dir: &Path,
        healthcheck_path: Option<&str>,
        timeout: Duration,
    ) -> Result<SoracloudHostedHttpRuntimeStateV1> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let runtime_state =
                read_hosted_http_runtime_state(service_dir)?.expect("hosted runtime state");
            let replica_health_ok = runtime_state.replicas.iter().all(|replica| {
                replica.health_status == SoraServiceHealthStatusV1::Healthy
                    && replica.listen_base_url.as_deref().is_some_and(|base_url| {
                        probe_hosted_http_health(base_url, healthcheck_path).is_ok()
                    })
            });
            if runtime_state.health_status == SoraServiceHealthStatusV1::Healthy
                && replica_health_ok
            {
                return Ok(runtime_state);
            }
            if std::time::Instant::now() >= deadline {
                let replica_errors = runtime_state
                    .replicas
                    .iter()
                    .map(|replica| {
                        format!(
                            "slot {}: status={:?}, error={:?}, listen_base_url={:?}",
                            replica.replica_slot,
                            replica.health_status,
                            replica.last_error,
                            replica.listen_base_url,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let worker_diagnostics = manager
                    .hosted_http_workers
                    .lock()
                    .iter()
                    .map(|((service_name, service_version, replica_slot), worker)| {
                        let guard = worker.lock();
                        format!(
                            "{service_name}@{service_version} replica {replica_slot}: pid={:?}, url={}, stderr={}",
                            guard.pid(),
                            guard.listen_base_url,
                            stderr_log_excerpt(&guard.stderr_log_path),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let replica_log_diagnostics = runtime_state
                    .replicas
                    .iter()
                    .map(|replica| {
                        let replica_dir = service_dir.join(format!(
                            "replicas/replica-{slot:04}",
                            slot = replica.replica_slot
                        ));
                        format!(
                            "slot {} stderr={} console={}",
                            replica.replica_slot,
                            stderr_log_excerpt(&replica_dir.join("inrou.stderr.log")),
                            stderr_log_excerpt(&replica_dir.join("inrou.console.log")),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                eyre::bail!(
                    "timed out waiting for hosted HTTP runtime state to become healthy: service_status={:?}, service_error={:?}, replica_statuses=[{}]\nworker diagnostics:\n{worker_diagnostics}\nreplica logs:\n{replica_log_diagnostics}",
                    runtime_state.health_status,
                    runtime_state.last_error,
                    replica_errors,
                );
            }
            thread::sleep(Duration::from_millis(250));
            manager.reconcile_once()?;
        }
    }

    fn write_inrou_test_bundle_file(path: &Path, contents: &str) -> Result<()> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| eyre::eyre!("missing parent for {}", path.display()))?,
        )?;
        fs::write(path, contents)?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }

    fn create_inrou_bundle_archive_for_linux_test(
        temp_dir: &Path,
        kernel_image: &Path,
        rootfs_image: &Path,
        initrd_image: Option<&Path>,
        bootstrap_overlay: &str,
    ) -> Result<Vec<u8>> {
        let bundle_root = temp_dir.join("bundle-root");
        for guest_dir in ["x86_64", "aarch64"] {
            let guest_root = bundle_root.join("inrou").join(guest_dir);
            fs::create_dir_all(&guest_root)?;
            fs::copy(kernel_image, guest_root.join("vmlinux"))?;
            fs::copy(rootfs_image, guest_root.join("rootfs.ext4"))?;
            if let Some(initrd_image) = initrd_image {
                fs::copy(initrd_image, guest_root.join("initrd.img"))?;
            } else {
                fs::write(guest_root.join("initrd.img"), b"initrd placeholder")?;
            }
        }
        fs::write(
            bundle_root.join("inrou/bootstrap-user-data.yml"),
            bootstrap_overlay,
        )?;
        write_inrou_test_bundle_file(
            &bundle_root.join("bin/sh"),
            "#!/bin/sh\nexec /bin/sh \"$@\"\n",
        )?;

        let archive_path = temp_dir.join("inrou-test-bundle.tgz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&bundle_root)
            .arg(".")
            .status()?;
        if !status.success() {
            eyre::bail!("tar failed while building Inrou Linux/KVM smoke bundle: {status}");
        }
        Ok(fs::read(archive_path)?)
    }

    #[cfg(target_os = "linux")]
    fn linux_smoke_required_env_path(name: &str) -> Result<PathBuf> {
        let value = std::env::var(name)
            .wrap_err_with(|| format!("missing required environment variable `{name}`"))?;
        let path = PathBuf::from(value);
        if !path.is_file() {
            eyre::bail!(
                "environment variable `{name}` must point to an existing file, got {}",
                path.display()
            );
        }
        Ok(path)
    }

    #[cfg(target_os = "linux")]
    fn require_linux_kvm_smoke_prerequisites() -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 to exercise the Linux/KVM Inrou smoke test."
            );
            return Ok(());
        }
        if std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1") {
            println!(
                "Skipping: set IROHA_INROU_LINUX_KVM=1 plus explicit guest asset env vars to run the Linux/KVM Inrou smoke test."
            );
            return Ok(());
        }
        if !Path::new("/dev/kvm").exists() {
            eyre::bail!("/dev/kvm is required for the Linux/KVM Inrou smoke test");
        }
        if !Path::new("/dev/net/tun").exists() {
            eyre::bail!("/dev/net/tun is required for the Linux/KVM Inrou smoke test");
        }
        let uid = Command::new("id").arg("-u").output()?;
        if !uid.status.success() || String::from_utf8_lossy(&uid.stdout).trim() != "0" {
            eyre::bail!(
                "Linux/KVM Inrou smoke test must run as root so tap devices and firewall rules can be created"
            );
        }
        if !inrou_ip_forward_enabled()? {
            eyre::bail!("Linux/KVM Inrou smoke test requires /proc/sys/net/ipv4/ip_forward = 1");
        }
        let required_programs = [
            "firecracker",
            "ip",
            "iptables",
            "tar",
            "exportfs",
            "rpc.nfsd",
            "mount",
            "chown",
        ];
        for program in required_programs {
            if resolve_executable_on_path(program).is_none() {
                eyre::bail!("Linux/KVM Inrou smoke test requires `{program}` on PATH");
            }
        }
        if resolve_inrou_mke2fs_executable().is_none() {
            eyre::bail!("Linux/KVM Inrou smoke test requires `mke2fs` or `mkfs.ext4` on PATH");
        }
        Ok(())
    }

    fn portable_smoke_required_env_path(name: &str) -> Result<PathBuf> {
        let value = std::env::var(name)
            .wrap_err_with(|| format!("missing required environment variable `{name}`"))?;
        let path = PathBuf::from(value);
        if !path.is_file() {
            eyre::bail!(
                "environment variable `{name}` must point to an existing file, got {}",
                path.display()
            );
        }
        Ok(path)
    }

    fn require_portable_smoke_prerequisites() -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 to exercise the PortableVm Inrou smoke test."
            );
            return Ok(());
        }
        if std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1") {
            println!(
                "Skipping: set IROHA_INROU_PORTABLE=1 plus explicit guest asset env vars to run the PortableVm Inrou smoke test."
            );
            return Ok(());
        }
        let profile = portable_vm_guest_machine_profile(current_host_inrou_guest_isa());
        if resolve_executable_candidates(profile.emulator_candidates).is_none() {
            eyre::bail!(
                "PortableVm Inrou smoke test requires one of {:?} on PATH",
                profile.emulator_candidates
            );
        }
        for program in ["tar", "qemu-img"] {
            if resolve_executable_on_path(program).is_none() {
                eyre::bail!("PortableVm Inrou smoke test requires `{program}` on PATH");
            }
        }
        Ok(())
    }

    fn spawn_http_fixture(body: Vec<u8>) -> Result<(String, std::thread::JoinHandle<()>)> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::write_all(&mut stream, &body);
            }
        });
        Ok((format!("http://{address}/fixture"), handle))
    }

    #[derive(Clone)]
    struct RemoteManifestFixture {
        manifest_digest: ManifestDigest,
        order_id: ReplicationOrderId,
        issued_epoch: u64,
        canonical_order: Vec<u8>,
        manifest_id_hex: String,
        manifest_response_body: Vec<u8>,
        plan_response_body: Vec<u8>,
        chunk_path: String,
        payload: Vec<u8>,
    }

    #[derive(Clone)]
    struct HttpFixtureResponse {
        status_code: u16,
        content_type: &'static str,
        body: Vec<u8>,
        content_length_override: Option<u64>,
        extra_headers: Vec<(String, String)>,
    }

    impl HttpFixtureResponse {
        fn json(body: Vec<u8>) -> Self {
            Self {
                status_code: 200,
                content_type: "application/json",
                body,
                content_length_override: None,
                extra_headers: Vec::new(),
            }
        }

        fn binary(body: Vec<u8>) -> Self {
            Self {
                status_code: 200,
                content_type: "application/octet-stream",
                body,
                content_length_override: None,
                extra_headers: Vec::new(),
            }
        }

        fn head_ok(content_type: &'static str, content_length: u64) -> Self {
            Self {
                status_code: 200,
                content_type,
                body: Vec::new(),
                content_length_override: Some(content_length),
                extra_headers: Vec::new(),
            }
        }

        fn with_header(mut self, key: &str, value: &str) -> Self {
            self.extra_headers.push((key.to_owned(), value.to_owned()));
            self
        }

        fn text(status_code: u16, body: &str) -> Self {
            Self {
                status_code,
                content_type: "text/plain; charset=utf-8",
                body: body.as_bytes().to_vec(),
                content_length_override: None,
                extra_headers: Vec::new(),
            }
        }

        fn not_found() -> Self {
            Self {
                status_code: 404,
                content_type: "text/plain; charset=utf-8",
                body: b"not found".to_vec(),
                content_length_override: None,
                extra_headers: Vec::new(),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CapturedHttpRequest {
        method: String,
        path: String,
        headers: BTreeMap<String, String>,
        body: Vec<u8>,
    }

    struct HttpRouteFixture {
        base_url: String,
        stop_tx: mpsc::Sender<()>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for HttpRouteFixture {
        fn drop(&mut self) {
            let _ = self.stop_tx.send(());
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn fixed_chunker_handle() -> ChunkerProfileHandle {
        ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: BLAKE3_256_MULTIHASH_CODE,
        }
    }

    fn build_remote_manifest_fixture(
        payload: &[u8],
        provider_id: [u8; 32],
        order_seed: u8,
    ) -> Result<RemoteManifestFixture> {
        let (_plan, manifest) = build_sorafs_manifest(payload)?;
        let manifest_digest = ManifestDigest::from_manifest(&manifest)?;
        let manifest_id_hex = hex::encode(&manifest.root_cid);
        let chunk_profile_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        let chunk_digest_hex = hex::encode(blake3::hash(payload).as_bytes());
        let manifest_response = StorageManifestResponseDto {
            manifest_id_hex: manifest_id_hex.clone(),
            manifest_b64: String::new(),
            manifest_digest_hex: hex::encode(manifest_digest.as_bytes()),
            payload_digest_hex: chunk_digest_hex.clone(),
            content_length: payload.len() as u64,
            chunk_count: 1,
            chunk_profile_handle: chunk_profile_handle.clone(),
            stored_at_unix_secs: 1,
            files: Vec::new(),
        };
        let manifest_response_body = norito::json::to_vec(&manifest_response)?;
        let mut chunk_entry = norito::json::native::Map::new();
        chunk_entry.insert("chunk_index".into(), norito::json::Value::from(0_u64));
        chunk_entry.insert("offset".into(), norito::json::Value::from(0_u64));
        chunk_entry.insert(
            "length".into(),
            norito::json::Value::from(payload.len() as u64),
        );
        chunk_entry.insert(
            "digest_blake3".into(),
            norito::json::Value::from(chunk_digest_hex.clone()),
        );
        let mut plan = norito::json::native::Map::new();
        plan.insert("chunk_count".into(), norito::json::Value::from(1_u64));
        plan.insert(
            "content_length".into(),
            norito::json::Value::from(payload.len() as u64),
        );
        plan.insert(
            "payload_digest_blake3".into(),
            norito::json::Value::from(chunk_digest_hex.clone()),
        );
        plan.insert(
            "chunk_profile_handle".into(),
            norito::json::Value::from(chunk_profile_handle),
        );
        plan.insert(
            "chunk_digests_blake3".into(),
            norito::json::Value::Array(vec![norito::json::Value::from(chunk_digest_hex.clone())]),
        );
        plan.insert(
            "chunks".into(),
            norito::json::Value::Array(vec![norito::json::Value::Object(chunk_entry)]),
        );
        let mut plan_response = norito::json::native::Map::new();
        plan_response.insert(
            "manifest_id_hex".into(),
            norito::json::Value::from(manifest_id_hex.clone()),
        );
        plan_response.insert("plan".into(), norito::json::Value::Object(plan));
        let plan_response_body = norito::json::to_vec(&norito::json::Value::Object(plan_response))?;
        let order_id = ReplicationOrderId::new([order_seed; 32]);
        let canonical_order = norito::to_bytes(&ReplicationOrderV1 {
            order_id: *order_id.as_bytes(),
            manifest_cid: manifest.root_cid.clone(),
            providers: vec![provider_id],
            redundancy: 1,
            deadline: u64::from(order_seed) + 600,
            policy_hash: [0x91; 32],
        })?;
        Ok(RemoteManifestFixture {
            manifest_digest,
            order_id,
            issued_epoch: u64::from(order_seed),
            canonical_order,
            manifest_id_hex: manifest_id_hex.clone(),
            manifest_response_body,
            plan_response_body,
            chunk_path: format!("/v1/sorafs/storage/chunk/{manifest_id_hex}/{chunk_digest_hex}"),
            payload: payload.to_vec(),
        })
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Result<(String, String)> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let request = String::from_utf8_lossy(&buffer);
        let request_line = request.lines().next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        Ok((
            parts.next().unwrap_or_default().to_owned(),
            parts.next().unwrap_or_default().to_owned(),
        ))
    }

    fn write_http_response(
        stream: &mut std::net::TcpStream,
        response: &HttpFixtureResponse,
    ) -> Result<()> {
        let reason = match response.status_code {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Response",
        };
        let content_length = response
            .content_length_override
            .unwrap_or_else(|| u64::try_from(response.body.len()).unwrap_or(u64::MAX));
        let mut headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n",
            response.status_code, reason, content_length, response.content_type,
        );
        for (key, value) in &response.extra_headers {
            headers.push_str(key);
            headers.push_str(": ");
            headers.push_str(value);
            headers.push_str("\r\n");
        }
        headers.push_str("\r\n");
        stream.write_all(headers.as_bytes())?;
        stream.write_all(&response.body)?;
        Ok(())
    }

    fn parse_http_request(buffer: &[u8]) -> Result<CapturedHttpRequest> {
        let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
            return Err(eyre::eyre!(
                "HTTP fixture request missing header terminator"
            ));
        };
        let header_bytes = &buffer[..header_end];
        let request = String::from_utf8_lossy(header_bytes);
        let mut lines = request.lines();
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_owned();
        let path = parts.next().unwrap_or_default().to_owned();
        let mut headers = BTreeMap::new();
        let mut content_length = 0_usize;
        for line in lines {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_owned();
            if key == "content-length" {
                content_length = value.parse::<usize>().unwrap_or(0);
            }
            headers.insert(key, value);
        }
        let body_start = header_end + 4;
        if buffer.len() < body_start.saturating_add(content_length) {
            return Err(eyre::eyre!(
                "HTTP fixture request body shorter than declared Content-Length"
            ));
        }
        Ok(CapturedHttpRequest {
            method,
            path,
            headers,
            body: buffer[body_start..body_start + content_length].to_vec(),
        })
    }

    fn read_http_request_full(stream: &mut std::net::TcpStream) -> Result<CapturedHttpRequest> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let mut expected_total_len = None;
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    if expected_total_len.is_none()
                        && let Some(header_end) =
                            buffer.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let header_text = String::from_utf8_lossy(&buffer[..header_end]);
                        let content_length = header_text
                            .lines()
                            .skip(1)
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.trim()
                                    .eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        expected_total_len = Some(header_end + 4 + content_length);
                    }
                    if expected_total_len.is_some_and(|expected| buffer.len() >= expected) {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        parse_http_request(&buffer)
    }

    fn spawn_recording_http_route_fixture(
        routes: BTreeMap<(String, String), HttpFixtureResponse>,
    ) -> Result<(HttpRouteFixture, Arc<Mutex<Vec<CapturedHttpRequest>>>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let base_url = format!("http://{}", listener.local_addr()?);
        let captured = Arc::new(Mutex::new(Vec::new()));
        let captured_requests = Arc::clone(&captured);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = match read_http_request_full(&mut stream) {
                            Ok(request) => {
                                let key = (request.method.clone(), request.path.clone());
                                captured_requests
                                    .lock()
                                    .expect("fixture capture mutex")
                                    .push(request);
                                routes
                                    .get(&key)
                                    .cloned()
                                    .unwrap_or_else(HttpFixtureResponse::not_found)
                            }
                            Err(_) => HttpFixtureResponse::not_found(),
                        };
                        let _ = write_http_response(&mut stream, &response);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok((
            HttpRouteFixture {
                base_url,
                stop_tx,
                handle: Some(handle),
            },
            captured,
        ))
    }

    fn spawn_remote_hydration_fixture(
        fixtures: &[RemoteManifestFixture],
    ) -> Result<HttpRouteFixture> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let base_url = format!("http://{}", listener.local_addr()?);
        let mut routes = BTreeMap::<(String, String), HttpFixtureResponse>::new();
        let mut token_response = norito::json::native::Map::new();
        token_response.insert(
            "token_base64".into(),
            norito::json::Value::from("fixture-stream-token"),
        );
        routes.insert(
            ("POST".to_owned(), "/v1/sorafs/storage/token".to_owned()),
            HttpFixtureResponse::json(norito::json::to_vec(&norito::json::Value::Object(
                token_response,
            ))?),
        );
        for fixture in fixtures {
            routes.insert(
                (
                    "GET".to_owned(),
                    format!("/v1/sorafs/storage/manifest/{}", fixture.manifest_id_hex),
                ),
                HttpFixtureResponse::json(fixture.manifest_response_body.clone()),
            );
            routes.insert(
                (
                    "GET".to_owned(),
                    format!("/v1/sorafs/storage/plan/{}", fixture.manifest_id_hex),
                ),
                HttpFixtureResponse::json(fixture.plan_response_body.clone()),
            );
            routes.insert(
                ("GET".to_owned(), fixture.chunk_path.clone()),
                HttpFixtureResponse::binary(fixture.payload.clone()),
            );
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = match read_http_request(&mut stream) {
                            Ok((method, path)) => routes
                                .get(&(method, path))
                                .cloned()
                                .unwrap_or_else(HttpFixtureResponse::not_found),
                            Err(_) => HttpFixtureResponse::not_found(),
                        };
                        let _ = write_http_response(&mut stream, &response);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(HttpRouteFixture {
            base_url,
            stop_tx,
            handle: Some(handle),
        })
    }

    fn test_provider_cache(
        base_url: &str,
        provider_id: [u8; 32],
    ) -> Result<Arc<AsyncRwLock<ProviderAdvertCache>>> {
        let advert_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0xA5; 32])?;
        let advert_public = PublicKey::from(advert_key.clone());
        let council_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x42; 32])?;
        let council_public = PublicKey::from(council_key.clone());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let issued_at = now.saturating_sub(60);
        let expires_at = issued_at + 600;

        let capabilities = vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: ProviderCapabilityRangeV1 {
                    max_chunk_span: 32,
                    min_granularity: 8,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                }
                .to_bytes()?,
            },
        ];
        let stream_budget = Some(StreamBudgetV1 {
            max_in_flight: 8,
            max_bytes_per_sec: 8_388_608,
            burst_bytes: Some(1_048_576),
        });
        let transport_hints = Some(vec![TransportHintV1 {
            protocol: TransportProtocol::ToriiHttpRange,
            priority: 0,
        }]);
        let endpoint = AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: base_url.to_owned(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        };
        let body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
            stake: StakePointer {
                pool_id: [0x21; 32],
                stake_amount: 1_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1_000,
                max_concurrent_streams: 8,
            },
            capabilities: capabilities.clone(),
            endpoints: vec![endpoint.clone()],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 5,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: stream_budget.clone(),
            transport_hints: transport_hints.clone(),
        };
        let body_bytes = norito::to_bytes(&body)?;
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body: body.clone(),
            signature: sorafs_manifest::AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: advert_public.to_bytes().1.to_vec(),
                signature: Signature::new(&advert_key, &body_bytes).payload().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let proposal = ProviderAdmissionProposalV1 {
            version: PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: body.profile_id.clone(),
            profile_aliases: body.profile_aliases.clone(),
            stake: body.stake.clone(),
            capabilities: body.capabilities.clone(),
            endpoints: vec![EndpointAdmissionV1 {
                endpoint,
                attestation: EndpointAttestationV1 {
                    version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: EndpointAttestationKind::Mtls,
                    attested_at: issued_at.saturating_sub(10),
                    expires_at: expires_at + 60,
                    leaf_certificate: vec![0xAA],
                    intermediate_certificates: Vec::new(),
                    alpn_ids: vec!["h2".to_owned()],
                    report: Vec::new(),
                },
            }],
            advert_key: advert_public
                .to_bytes()
                .1
                .try_into()
                .expect("ed25519 key is 32 bytes"),
            jurisdiction_code: "US".to_owned(),
            contact_uri: Some("mailto:ops@example.test".to_owned()),
            stream_budget,
            transport_hints,
        };
        let proposal_digest = compute_proposal_digest(&proposal)?;
        let envelope = ProviderAdmissionEnvelopeV1 {
            version: PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal,
            proposal_digest,
            advert_body: body.clone(),
            advert_body_digest: compute_advert_body_digest(&body)?,
            issued_at,
            retention_epoch: expires_at + 600,
            council_signatures: vec![CouncilSignature {
                signer: council_public
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("ed25519 key is 32 bytes"),
                signature: Signature::new(&council_key, &proposal_digest)
                    .payload()
                    .to_vec(),
            }],
            notes: None,
        };
        let admission = AdmissionRegistry::from_envelopes([envelope])?;
        let mut cache = ProviderAdvertCache::new(
            vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            Arc::new(admission),
        );
        cache
            .ingest(advert, issued_at.saturating_add(1))
            .map_err(|error| eyre::eyre!(error.to_string()))?;
        Ok(Arc::new(AsyncRwLock::new(cache)))
    }

    fn approve_remote_hydration_sources(
        state: &Arc<State>,
        fixtures: &[RemoteManifestFixture],
    ) -> Result<()> {
        let view = state.view();
        let pricing = view.world().sorafs_pricing().clone();
        let next_height = NonZeroU64::new(
            u64::try_from(view.height())
                .unwrap_or(u64::MAX.saturating_sub(1))
                .saturating_add(1),
        )
        .expect("nonzero block height");
        let header = BlockHeader::new(next_height, view.latest_block_hash(), None, None, 0, 0);
        drop(view);

        let mut block = state.block(header);
        {
            let mut pin_manifests = block.world.pin_manifests_mut_for_testing().transaction();
            for fixture in fixtures {
                let policy = PinPolicy::default();
                let content_length = fixture.payload.len() as u64;
                let amount_nano = pricing.public_pin_fee_nano(
                    policy.storage_class,
                    content_length,
                    policy.min_replicas,
                    fixture.issued_epoch,
                    policy.retention_epoch,
                );
                let mut record = PinManifestRecord::new(
                    fixture.manifest_digest,
                    fixed_chunker_handle(),
                    [0; 32],
                    policy,
                    (*ALICE_ID).clone(),
                    fixture.issued_epoch,
                    None,
                    None,
                    Metadata::default(),
                )
                .with_content_length(content_length);
                record.record_pin_fee_payment(PinFeePayment {
                    paid_by: (*ALICE_ID).clone(),
                    fee_asset_id: state.gov.sorafs_pin_fee_asset_id.clone(),
                    treasury_account_id: state.gov.sorafs_pin_fee_treasury_account.clone(),
                    amount_nano,
                });
                record.approve(fixture.issued_epoch, None);
                pin_manifests.insert(fixture.manifest_digest, record);
            }
            pin_manifests.apply();
        }
        {
            let mut replication_orders = block
                .world
                .replication_orders_mut_for_testing()
                .transaction();
            for fixture in fixtures {
                replication_orders.insert(
                    fixture.order_id,
                    ReplicationOrderRecord {
                        order_id: fixture.order_id,
                        manifest_digest: fixture.manifest_digest,
                        issued_by: (*ALICE_ID).clone(),
                        issued_epoch: fixture.issued_epoch,
                        deadline_epoch: fixture.issued_epoch + 600,
                        canonical_order: fixture.canonical_order.clone(),
                        status: ReplicationOrderStatus::Completed(fixture.issued_epoch + 1),
                    },
                );
            }
            replication_orders.apply();
        }
        block.commit()?;
        Ok(())
    }

    fn test_runtime_manager_config(state_dir: PathBuf) -> SoracloudRuntimeManagerConfig {
        let mut runtime = iroha_config::parameters::actual::SoracloudRuntime {
            state_dir,
            ..Default::default()
        };
        runtime.inrou.enabled = true;
        SoracloudRuntimeManagerConfig::from_runtime_config(&runtime)
    }

    fn test_runtime_handle(
        manager: &SoracloudRuntimeManager,
        state: Arc<State>,
    ) -> SoracloudRuntimeManagerHandle {
        SoracloudRuntimeManagerHandle {
            snapshot: Arc::clone(&manager.snapshot),
            config: Arc::new(manager.config.clone()),
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state,
            hf_local_workers: Arc::clone(&manager.hf_local_workers),
            host_violation_reporter: Arc::clone(&manager.host_violation_reporter),
            mutation_sink: manager.mutation_sink.as_ref().map(Arc::clone),
            generated_hf_reconcile_attempts_ms: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
        }
    }

    #[derive(Default)]
    struct RecordingRuntimeMutationSink {
        instructions: parking_lot::Mutex<Vec<InstructionBox>>,
    }

    impl RecordingRuntimeMutationSink {
        #[allow(dead_code)]
        fn submitted_runtime_states(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::SetSoracloudRuntimeState> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<iroha_data_model::isi::soracloud::SetSoracloudRuntimeState>(
                        )
                        .cloned()
                })
                .collect()
        }

        #[allow(dead_code)]
        fn submitted_inrou_host_capabilities(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::AdvertiseSoracloudInrouHost> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<iroha_data_model::isi::soracloud::AdvertiseSoracloudInrouHost>()
                        .cloned()
                })
                .collect()
        }

        fn submitted_inrou_replica_runtime_states(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::SetSoracloudInrouReplicaRuntimeState> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::SetSoracloudInrouReplicaRuntimeState,
                        >()
                        .cloned()
                })
                .collect()
        }

        #[allow(dead_code)]
        fn submitted_cleared_inrou_replica_runtime_states(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState,
                        >()
                        .cloned()
                })
                .collect()
        }

        fn submitted_service_lease_usage(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::ReportSoracloudServiceLeaseUsage> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::ReportSoracloudServiceLeaseUsage,
                        >()
                        .cloned()
                })
                .collect()
        }

        fn submitted_violation_reports(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::ReportSoracloudModelHostViolation> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::ReportSoracloudModelHostViolation,
                        >()
                        .cloned()
                })
                .collect()
        }

        fn submitted_model_host_heartbeats(
            &self,
        ) -> Vec<iroha_data_model::isi::soracloud::HeartbeatSoracloudModelHost> {
            self.instructions
                .lock()
                .iter()
                .filter_map(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::HeartbeatSoracloudModelHost,
                        >()
                        .cloned()
                })
                .collect()
        }

        fn submitted_model_host_reconciles(&self) -> usize {
            self.instructions
                .lock()
                .iter()
                .filter(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(*instruction)
                        .downcast_ref::<iroha_data_model::isi::soracloud::ReconcileSoracloudModelHosts>()
                        .is_some()
                })
                .count()
        }

        #[allow(dead_code)]
        fn submitted_inrou_placement_reconciles(&self) -> usize {
            self.instructions
                .lock()
                .iter()
                .filter(|instruction| {
                    iroha_data_model::isi::Instruction::as_any(*instruction)
                        .downcast_ref::<
                            iroha_data_model::isi::soracloud::ReconcileSoracloudInrouPlacements,
                        >()
                        .is_some()
                })
                .count()
        }
    }

    impl SoracloudRuntimeMutationSink for RecordingRuntimeMutationSink {
        fn submit_instruction(
            &self,
            instruction: InstructionBox,
            _endpoint: &'static str,
        ) -> eyre::Result<()> {
            self.instructions.lock().push(instruction);
            Ok(())
        }

        fn submit_model_host_heartbeat(
            &self,
            validator_account_id: &AccountId,
            heartbeat_expires_at_ms: u64,
        ) -> eyre::Result<()> {
            let payload = encode_model_host_heartbeat_provenance_payload(
                validator_account_id,
                heartbeat_expires_at_ms,
            )?;
            self.instructions.lock().push(InstructionBox::from(
                iroha_data_model::isi::soracloud::HeartbeatSoracloudModelHost {
                    validator_account_id: validator_account_id.clone(),
                    heartbeat_expires_at_ms,
                    provenance: ManifestProvenance {
                        signer: ALICE_KEYPAIR.public_key().clone(),
                        signature: iroha_crypto::Signature::new(
                            ALICE_KEYPAIR.private_key(),
                            &payload,
                        ),
                    },
                },
            ));
            Ok(())
        }

        fn submit_inrou_host_capability(
            &self,
            capability: &SoraInrouHostCapabilityRecordV1,
        ) -> eyre::Result<()> {
            let payload = encode_inrou_host_advertise_provenance_payload(capability)?;
            self.instructions.lock().push(InstructionBox::from(
                iroha_data_model::isi::soracloud::AdvertiseSoracloudInrouHost {
                    capability: capability.clone(),
                    provenance: ManifestProvenance {
                        signer: ALICE_KEYPAIR.public_key().clone(),
                        signature: iroha_crypto::Signature::new(
                            ALICE_KEYPAIR.private_key(),
                            &payload,
                        ),
                    },
                },
            ));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct GeneratedHfServiceFixture {
        source_id: Hash,
        pool_id: Hash,
        bundle: SoraDeploymentBundleV1,
    }

    fn insert_generated_hf_service_fixture(
        state: &mut Arc<State>,
        service_name: &str,
        repo_id: &str,
        resolved_revision: &str,
        model_name: &str,
    ) -> Result<GeneratedHfServiceFixture> {
        let source_id = Hash::new(format!("generated-hf-source:{service_name}").as_bytes());
        let pool_id = Hash::new(format!("generated-hf-pool:{service_name}").as_bytes());
        let lease_asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x09,
        ])
        .expect("fixture asset definition");
        let bundle = iroha_core::soracloud_runtime::build_soracloud_hf_generated_service_bundle(
            service_name.parse().expect("valid generated service name"),
            &source_id.to_string(),
            repo_id,
            resolved_revision,
            model_name,
        );

        let world = &mut Arc::get_mut(state).expect("unique test state").world;
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
                sample_deployment_state(&bundle),
            );
        world.soracloud_hf_sources_mut_for_testing().insert(
            source_id,
            SoraHfSourceRecordV1 {
                schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                source_id,
                repo_id: repo_id.to_owned(),
                resolved_revision: resolved_revision.to_owned(),
                model_name: model_name.to_owned(),
                adapter_id: "hf.shared.v1".to_owned(),
                normalized_runtime_hash: Hash::new(
                    format!("generated-hf-runtime:{service_name}").as_bytes(),
                ),
                resource_profile: Some(sample_hf_resource_profile_for_tests()),
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
                    storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                    lease_asset_definition_id,
                    base_fee_nanos: 10_000,
                    lease_term_ms: 60_000,
                    window_started_at_ms: 10,
                    window_expires_at_ms: 60_010,
                    active_member_count: 1,
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
                    joined_at_ms: 10,
                    updated_at_ms: 20,
                    total_paid_nanos: 10_000,
                    total_refunded_nanos: 0,
                    last_charge_nanos: 10_000,
                    total_compute_paid_nanos: 0,
                    total_compute_refunded_nanos: 0,
                    last_compute_charge_nanos: 0,
                    service_bindings: BTreeSet::from([bundle.service.service_name.to_string()]),
                    apartment_bindings: BTreeSet::new(),
                },
            );

        Ok(GeneratedHfServiceFixture {
            source_id,
            pool_id,
            bundle,
        })
    }

    fn assign_fixture_artifact_hashes(
        bundle: &mut SoraDeploymentBundleV1,
        bundle_bytes: &[u8],
        label: &str,
    ) -> Vec<Vec<u8>> {
        let service_name = bundle.service.service_name.to_string();
        bundle.container.bundle_hash = Hash::new(bundle_bytes);
        let mut payloads = Vec::with_capacity(bundle.service.artifacts.len());
        for (index, artifact) in bundle.service.artifacts.iter_mut().enumerate() {
            let payload =
                format!("{label}:{service_name}:{index}:{}", artifact.artifact_path).into_bytes();
            artifact.artifact_hash = Hash::new(&payload);
            payloads.push(payload);
        }
        payloads
    }

    fn insert_generated_hf_placement_fixture(
        state: &mut Arc<State>,
        fixture: &GeneratedHfServiceFixture,
        local_role: SoraHfPlacementHostRoleV1,
        local_status: SoraHfPlacementHostStatusV1,
        local_peer_id: &str,
    ) -> Hash {
        let placement_id = Hash::new(
            format!(
                "generated-hf-placement:{}",
                fixture.bundle.service.service_name
            )
            .as_bytes(),
        );
        let mut assigned_hosts = Vec::new();
        if local_role == SoraHfPlacementHostRoleV1::Replica {
            assigned_hosts.push(SoraHfPlacementHostAssignmentV1 {
                validator_account_id: BOB_ID.clone(),
                peer_id: "12D3KooWGeneratedHfFixturePrimary".to_owned(),
                role: SoraHfPlacementHostRoleV1::Primary,
                status: SoraHfPlacementHostStatusV1::Warm,
                host_class: "cpu.large".to_owned(),
            });
        }
        assigned_hosts.push(SoraHfPlacementHostAssignmentV1 {
            validator_account_id: ALICE_ID.clone(),
            peer_id: local_peer_id.to_owned(),
            role: local_role,
            status: local_status,
            host_class: "cpu.large".to_owned(),
        });
        if local_role == SoraHfPlacementHostRoleV1::Primary {
            assigned_hosts.push(SoraHfPlacementHostAssignmentV1 {
                validator_account_id: BOB_ID.clone(),
                peer_id: "12D3KooWGeneratedHfFixtureReplica".to_owned(),
                role: SoraHfPlacementHostRoleV1::Replica,
                status: SoraHfPlacementHostStatusV1::Warm,
                host_class: "cpu.large".to_owned(),
            });
        }

        Arc::get_mut(state)
            .expect("unique test state")
            .world
            .soracloud_hf_placements_mut_for_testing()
            .insert(
                fixture.pool_id,
                SoraHfPlacementRecordV1 {
                    schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                    placement_id,
                    source_id: fixture.source_id,
                    pool_id: fixture.pool_id,
                    status: if local_role == SoraHfPlacementHostRoleV1::Primary
                        && local_status == SoraHfPlacementHostStatusV1::Warm
                    {
                        SoraHfPlacementStatusV1::Ready
                    } else {
                        SoraHfPlacementStatusV1::Degraded
                    },
                    selection_seed_hash: Hash::new(
                        format!(
                            "generated-hf-placement-seed:{}",
                            fixture.bundle.service.service_name
                        )
                        .as_bytes(),
                    ),
                    resource_profile: sample_hf_resource_profile_for_tests(),
                    eligible_validator_count: u32::try_from(assigned_hosts.len())
                        .expect("assigned host count fits in u32"),
                    adaptive_target_host_count: u16::try_from(assigned_hosts.len())
                        .expect("assigned host count fits in u16"),
                    assigned_hosts,
                    total_reservation_fee_nanos: 20_000,
                    last_rebalance_at_ms: 20,
                    last_error: None,
                },
            );
        placement_id
    }

    fn set_generated_hf_primary_assignment_status(
        state: &mut Arc<State>,
        fixture: &GeneratedHfServiceFixture,
        primary_status: SoraHfPlacementHostStatusV1,
    ) {
        let placements = Arc::get_mut(state)
            .expect("unique test state")
            .world
            .soracloud_hf_placements_mut_for_testing();
        let mut placement = placements
            .view()
            .get(&fixture.pool_id)
            .cloned()
            .expect("generated HF placement fixture");
        for assignment in &mut placement.assigned_hosts {
            if assignment.role == SoraHfPlacementHostRoleV1::Primary {
                assignment.status = primary_status;
            }
        }
        placement.status = if primary_status == SoraHfPlacementHostStatusV1::Warm {
            SoraHfPlacementStatusV1::Ready
        } else {
            SoraHfPlacementStatusV1::Degraded
        };
        placements.insert(fixture.pool_id, placement);
    }

    fn set_generated_hf_service_route_visibility(
        state: &mut Arc<State>,
        fixture: &GeneratedHfServiceFixture,
        visibility: SoraRouteVisibilityV1,
    ) {
        let key = (
            fixture.bundle.service.service_name.to_string(),
            fixture.bundle.service.service_version.clone(),
        );
        let revisions = Arc::get_mut(state)
            .expect("unique test state")
            .world
            .soracloud_service_revisions_mut_for_testing();
        let mut bundle = revisions
            .view()
            .get(&key)
            .cloned()
            .expect("generated HF fixture bundle should exist");
        bundle
            .service
            .route
            .as_mut()
            .expect("generated HF fixture route")
            .visibility = visibility;
        revisions.insert(key, bundle);
    }

    fn insert_local_model_host_capability_fixture(
        state: &mut Arc<State>,
        validator_account_id: &AccountId,
        peer_id: &str,
        heartbeat_expires_at_ms: u64,
    ) {
        Arc::get_mut(state)
            .expect("unique test state")
            .world
            .soracloud_model_host_capabilities_mut_for_testing()
            .insert(
                validator_account_id.clone(),
                SoraModelHostCapabilityRecordV1 {
                    schema_version: SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
                    validator_account_id: validator_account_id.clone(),
                    peer_id: peer_id.to_owned(),
                    supported_backends: BTreeSet::from([SoraHfBackendFamilyV1::Transformers]),
                    supported_formats: BTreeSet::from([SoraHfModelFormatV1::Safetensors]),
                    max_model_bytes: 8 * 1024 * 1024 * 1024,
                    max_disk_cache_bytes: 32 * 1024 * 1024 * 1024,
                    max_ram_bytes: 32 * 1024 * 1024 * 1024,
                    max_vram_bytes: 0,
                    max_concurrent_resident_models: 2,
                    host_class: "cpu.large".to_owned(),
                    advertised_at_ms: 10,
                    heartbeat_expires_at_ms,
                },
            );
    }

    fn seed_local_artifact_cache(
        artifacts_root: &Path,
        bundle_hash: Hash,
        bundle_bytes: &[u8],
        artifact_hashes_and_bytes: impl IntoIterator<Item = (Hash, Vec<u8>)>,
    ) -> Result<()> {
        fs::create_dir_all(artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle_hash)),
            bundle_bytes,
        )?;
        for (artifact_hash, payload) in artifact_hashes_and_bytes {
            fs::write(artifacts_root.join(hash_cache_name(artifact_hash)), payload)?;
        }
        Ok(())
    }

    fn test_sorafs_node(temp_dir: &tempfile::TempDir) -> NodeHandle {
        NodeHandle::new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(temp_dir.path().join("sorafs-storage"))
                .build(),
        )
    }

    fn build_sorafs_manifest(
        payload: &[u8],
    ) -> Result<(CarBuildPlan, sorafs_manifest::ManifestV1)> {
        let plan = CarBuildPlan::single_file(payload)?;
        let digest = blake3::hash(payload);
        let manifest = ManifestBuilder::new()
            .root_cid(digest.as_bytes().to_vec())
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(digest.into())
            .car_size(plan.content_length)
            .pin_policy(ManifestPinPolicy::default())
            .build()?;
        Ok((plan, manifest))
    }

    fn ingest_sorafs_payload(node: &NodeHandle, payload: &[u8]) -> Result<StoredManifest> {
        let (plan, manifest) = build_sorafs_manifest(payload)?;
        let mut reader = payload;
        let manifest_id = node.ingest_manifest(&manifest, &plan, &mut reader)?;
        node.manifest_metadata(&manifest_id).map_err(Into::into)
    }

    fn approve_sorafs_manifests(state: &Arc<State>, manifests: &[StoredManifest]) -> Result<()> {
        let view = state.view();
        let pricing = view.world().sorafs_pricing().clone();
        let next_height = NonZeroU64::new(
            u64::try_from(view.height())
                .unwrap_or(u64::MAX.saturating_sub(1))
                .saturating_add(1),
        )
        .expect("nonzero block height");
        let header = BlockHeader::new(next_height, view.latest_block_hash(), None, None, 0, 0);
        drop(view);

        let mut block = state.block(header);
        let mut pin_manifests = block.world.pin_manifests_mut_for_testing().transaction();
        for manifest in manifests {
            let digest = ManifestDigest::new(*manifest.manifest_digest());
            let policy = PinPolicy::default();
            let submitted_epoch = 1;
            let content_length = manifest.content_length();
            let amount_nano = pricing.public_pin_fee_nano(
                policy.storage_class,
                content_length,
                policy.min_replicas,
                submitted_epoch,
                policy.retention_epoch,
            );
            let mut record = PinManifestRecord::new(
                ManifestDigest::new(*manifest.manifest_digest()),
                ChunkerProfileHandle {
                    profile_id: 1,
                    namespace: "sorafs".to_owned(),
                    name: "sf1".to_owned(),
                    semver: "1.0.0".to_owned(),
                    multihash_code: BLAKE3_256_MULTIHASH_CODE,
                },
                [0; 32],
                policy,
                (*ALICE_ID).clone(),
                submitted_epoch,
                None,
                None,
                Metadata::default(),
            )
            .with_content_length(content_length);
            record.record_pin_fee_payment(PinFeePayment {
                paid_by: (*ALICE_ID).clone(),
                fee_asset_id: state.gov.sorafs_pin_fee_asset_id.clone(),
                treasury_account_id: state.gov.sorafs_pin_fee_treasury_account.clone(),
                amount_nano,
            });
            record.approve(submitted_epoch, None);
            pin_manifests.insert(digest, record);
        }
        pin_manifests.apply();
        block.commit()?;
        Ok(())
    }

    #[test]
    fn manager_config_uses_explicit_soracloud_runtime_settings() {
        let runtime = iroha_config::parameters::actual::SoracloudRuntime {
            production_mode: true,
            state_dir: PathBuf::from("/tmp/iroha-soracloud-runtime-config"),
            reconcile_interval: Duration::from_secs(17),
            hydration_concurrency: std::num::NonZeroUsize::new(9)
                .expect("nonzero hydration concurrency"),
            cache_budgets: iroha_config::parameters::actual::SoracloudRuntimeCacheBudgets {
                bundle_bytes: std::num::NonZeroU64::new(1_024).expect("nonzero"),
                static_asset_bytes: std::num::NonZeroU64::new(2_048).expect("nonzero"),
                journal_bytes: std::num::NonZeroU64::new(3_072).expect("nonzero"),
                checkpoint_bytes: std::num::NonZeroU64::new(4_096).expect("nonzero"),
                model_artifact_bytes: std::num::NonZeroU64::new(5_120).expect("nonzero"),
                model_weight_bytes: std::num::NonZeroU64::new(6_144).expect("nonzero"),
            },
            inrou: iroha_config::parameters::actual::SoracloudRuntimeInrou {
                max_concurrent_vms: std::num::NonZeroUsize::new(2).expect("nonzero concurrent vms"),
                enabled: true,
                start_grace: Duration::from_secs(11),
                stop_grace: Duration::from_secs(13),
                proxy_only: false,
            },
            submission: iroha_config::parameters::actual::SoracloudRuntimeSubmission {
                gas_asset_id: Some("xor#wonderland".to_owned()),
            },
            egress: iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["cdn.sora.test".to_string()],
                rate_per_minute: std::num::NonZeroU32::new(120),
                max_bytes_per_minute: std::num::NonZeroU64::new(262_144),
            },
            hf: iroha_config::parameters::actual::SoracloudRuntimeHuggingFace {
                hub_base_url: "https://models.sora.test".to_owned(),
                api_base_url: "https://models.sora.test/api".to_owned(),
                inference_base_url: "https://inference.sora.test/models".to_owned(),
                request_timeout: Duration::from_secs(9),
                local_execution_enabled: false,
                local_runner_program: "python3.12".to_owned(),
                local_runner_timeout: Duration::from_secs(45),
                model_host_heartbeat_ttl: Duration::from_secs(30),
                allow_inference_bridge_fallback: false,
                import_max_files: 12,
                import_max_file_bytes: 32 * 1024 * 1024,
                import_max_total_bytes: 256 * 1024 * 1024,
                import_file_allowlist: vec!["config.json".to_owned(), "*.safetensors".to_owned()],
                inference_token: Some("fixture-token".to_owned()),
            },
        };

        let manager = SoracloudRuntimeManagerConfig::from_runtime_config(&runtime);
        assert_eq!(manager.state_dir, runtime.state_dir);
        assert_eq!(manager.production_mode, runtime.production_mode);
        assert_eq!(manager.reconcile_interval, runtime.reconcile_interval);
        assert_eq!(manager.hydration_concurrency, runtime.hydration_concurrency);
        assert_eq!(manager.cache_budgets, runtime.cache_budgets);
        assert_eq!(manager.inrou, runtime.inrou);
        assert_eq!(manager.submission, runtime.submission);
        assert_eq!(manager.egress, runtime.egress);
        assert_eq!(manager.hf, runtime.hf);
    }

    #[test]
    #[should_panic(expected = "egress.default_allow = false")]
    fn manager_config_rejects_unsafe_direct_actual_production_posture() {
        let mut runtime = iroha_config::parameters::actual::SoracloudRuntime {
            production_mode: true,
            ..Default::default()
        };
        runtime.inrou.enabled = true;
        runtime.submission.gas_asset_id = Some("xor#wonderland".to_owned());
        runtime.egress.default_allow = true;
        runtime.egress.rate_per_minute = std::num::NonZeroU32::new(60);
        runtime.egress.max_bytes_per_minute = std::num::NonZeroU64::new(1_048_576);

        let _ = SoracloudRuntimeManagerConfig::from_runtime_config(&runtime);
    }

    #[test]
    fn hosted_http_concurrency_limit_uses_inrou_vm_budget() {
        let mut config =
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime-limit"));
        config.inrou.max_concurrent_vms =
            std::num::NonZeroUsize::new(2).expect("nonzero inrou vm limit");
        let manager = SoracloudRuntimeManager::new(config, test_state().expect("test state"));

        assert_eq!(manager.hosted_http_concurrency_limit(), 2);
    }

    #[test]
    fn proxy_only_inrou_host_advertises_zero_capacity() {
        let mut config =
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime-proxy-only"));
        config.inrou.proxy_only = true;
        config.inrou.max_concurrent_vms =
            std::num::NonZeroUsize::new(7).expect("nonzero inrou vm limit");
        config =
            config.with_local_host_identity(ALICE_ID.clone(), "12D3KooWProxyOnlyRuntimeHostAdvert");
        let manager = SoracloudRuntimeManager::new(config, test_state().expect("test state"));

        assert_eq!(manager.hosted_http_concurrency_limit(), 0);
        let (capability, auto_proxy_only) = manager
            .build_local_inrou_host_capability_record(123)
            .expect("host identity configured");
        assert!(!auto_proxy_only);
        assert!(capability.proxy_only);
        assert_eq!(capability.max_hosted_replica_capacity, 0);
        assert_eq!(capability.max_cpu_millis, 0);
        assert_eq!(capability.max_memory_bytes, 0);
        assert_eq!(capability.max_storage_bytes, 0);
    }

    #[test]
    fn disabled_inrou_host_does_not_advertise_or_host() {
        let mut config =
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime-disabled"));
        config.inrou.enabled = false;
        config =
            config.with_local_host_identity(ALICE_ID.clone(), "12D3KooWDisabledRuntimeHostAdvert");
        let state = test_state().expect("test state");
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));

        assert_eq!(manager.hosted_http_concurrency_limit(), 0);
        assert!(
            manager
                .build_local_inrou_host_capability_record(123)
                .is_none()
        );
        let view = state.view();
        assert!(
            manager
                .local_inrou_host_capability_refresh_candidate(&view)
                .is_none()
        );
    }

    #[test]
    fn inrou_host_heartbeat_ttl_tolerates_public_taira_queue_lag() {
        let config = test_runtime_manager_config(PathBuf::from(
            "/tmp/test-soracloud-runtime-inrou-heartbeat-ttl",
        ));
        let now_ms = 1_000;

        assert_eq!(
            desired_inrou_host_heartbeat_expiry_ms(now_ms, &config),
            now_ms + INROU_HOST_HEARTBEAT_TTL_FLOOR_MS
        );
    }

    #[test]
    fn inrou_host_capability_refresh_waits_until_heartbeat_margin() {
        let config = test_runtime_manager_config(PathBuf::from(
            "/tmp/test-soracloud-runtime-inrou-refresh-margin",
        ))
        .with_local_host_identity(ALICE_ID.clone(), "12D3KooWInrouRefreshMarginHost");
        let manager =
            SoracloudRuntimeManager::new(config.clone(), test_state().expect("test state"));
        let now_ms = 1_000_000;
        let (desired, _) = manager
            .build_local_inrou_host_capability_record(now_ms)
            .expect("host identity configured");
        let ttl_ms = inrou_host_heartbeat_ttl_ms(&config);
        let margin_ms = inrou_host_heartbeat_refresh_margin_ms(&config);
        assert!(ttl_ms > margin_ms.saturating_add(5_000));

        let mut existing = desired.clone();
        existing.advertised_at_ms = now_ms.saturating_sub(5_000);
        existing.heartbeat_expires_at_ms = desired.heartbeat_expires_at_ms.saturating_sub(5_000);
        assert!(existing.heartbeat_expires_at_ms < desired.heartbeat_expires_at_ms);
        assert!(!inrou_host_capability_refresh_needed(
            Some(&existing),
            &desired,
            now_ms,
            &config
        ));

        existing.heartbeat_expires_at_ms = now_ms.saturating_add(margin_ms);
        assert!(inrou_host_capability_refresh_needed(
            Some(&existing),
            &desired,
            now_ms,
            &config
        ));

        existing.heartbeat_expires_at_ms = now_ms.saturating_add(ttl_ms);
        existing.supported_guest_isas.clear();
        assert!(inrou_host_capability_refresh_needed(
            Some(&existing),
            &desired,
            now_ms,
            &config
        ));
    }

    #[test]
    fn inrou_host_capability_refresh_candidate_respects_authoritative_state() -> Result<()> {
        let mut state = test_state()?;
        let config = test_runtime_manager_config(PathBuf::from(
            "/tmp/test-soracloud-runtime-host-refresh-candidate",
        ))
        .with_local_host_identity(ALICE_ID.clone(), "12D3KooWRuntimeHostRefreshCandidate");
        let (mut capability, _auto_proxy_only) = {
            let manager = SoracloudRuntimeManager::new(config.clone(), Arc::clone(&state));
            manager
                .build_local_inrou_host_capability_record(123)
                .expect("host identity configured")
        };
        capability.heartbeat_expires_at_ms = u64::MAX;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_inrou_host_capabilities_mut_for_testing()
                .insert(ALICE_ID.clone(), capability);
        }

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        let view = state.view();

        assert!(
            manager
                .local_inrou_host_capability_refresh_candidate(&view)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn refresh_local_inrou_host_capability_submits_candidate() {
        let mut config =
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime-host-refresh"));
        config.inrou.proxy_only = true;
        config =
            config.with_local_host_identity(ALICE_ID.clone(), "12D3KooWRuntimeHostRefreshSubmit");
        let state = test_state().expect("test state");
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state))
            .with_mutation_sink(mutation_sink.clone());
        let candidate = {
            let view = state.view();
            manager.local_inrou_host_capability_refresh_candidate(&view)
        };

        manager.refresh_local_inrou_host_capability_if_needed(candidate);

        let capabilities = mutation_sink.submitted_inrou_host_capabilities();
        assert_eq!(capabilities.len(), 1);
        assert!(capabilities[0].capability.proxy_only);
    }

    #[test]
    fn inrou_placement_reconcile_request_obeys_needed_flag_and_cooldown() {
        let config = test_runtime_manager_config(PathBuf::from(
            "/tmp/test-soracloud-runtime-placement-reconcile",
        ));
        let state = test_state().expect("test state");
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager =
            SoracloudRuntimeManager::new(config, state).with_mutation_sink(mutation_sink.clone());

        manager.request_inrou_placement_reconcile_if_needed(false);
        assert_eq!(mutation_sink.submitted_inrou_placement_reconciles(), 0);

        manager.request_inrou_placement_reconcile_if_needed(true);
        manager.request_inrou_placement_reconcile_if_needed(true);
        assert_eq!(mutation_sink.submitted_inrou_placement_reconciles(), 1);
    }

    #[test]
    fn auto_proxy_only_inrou_host_advert_is_suppressed() {
        assert!(!should_submit_local_inrou_host_capability(true));
        assert!(should_submit_local_inrou_host_capability(false));
    }

    #[test]
    fn derive_hf_runtime_status_distinguishes_pending_deployment_and_ready() {
        assert_eq!(
            derive_hf_runtime_status(
                SoraHfSourceStatusV1::PendingImport,
                false,
                false,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
            ),
            SoracloudRuntimeHfSourceStatus::PendingImport
        );
        assert_eq!(
            derive_hf_runtime_status(
                SoraHfSourceStatusV1::PendingImport,
                true,
                false,
                1,
                1,
                0,
                0,
                0,
                0,
                0,
            ),
            SoracloudRuntimeHfSourceStatus::Ready
        );
        assert_eq!(
            derive_hf_runtime_status(
                SoraHfSourceStatusV1::PendingImport,
                true,
                false,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
            ),
            SoracloudRuntimeHfSourceStatus::PendingDeployment
        );
        assert_eq!(
            derive_hf_runtime_status(
                SoraHfSourceStatusV1::Failed,
                true,
                false,
                1,
                1,
                0,
                0,
                0,
                0,
                0,
            ),
            SoracloudRuntimeHfSourceStatus::Failed
        );
        assert_eq!(
            derive_hf_runtime_status(SoraHfSourceStatusV1::Ready, true, true, 1, 1, 0, 0, 0, 0, 0,),
            SoracloudRuntimeHfSourceStatus::Failed
        );
    }

    #[test]
    fn reconcile_once_persists_active_service_and_apartment_materializations() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.required_config_names = vec!["ui/settings".to_string()];
        bundle.container.config_exports = vec![
            iroha_data_model::soracloud::SoraConfigExportV1 {
                config_name: "ui/settings".to_string(),
                target: SoraConfigExportTargetV1::Env("UI_SETTINGS_JSON".to_string()),
            },
            iroha_data_model::soracloud::SoraConfigExportV1 {
                config_name: "ui/settings".to_string(),
                target: SoraConfigExportTargetV1::File("runtime/ui_settings.json".to_string()),
            },
        ];
        bundle.service.container.manifest_hash = bundle.container_manifest_hash();
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "persist-materialization");
        let config_value = Json::new("https://api.example.test");
        let service_secret_ciphertext = b"authoritative-db-password".to_vec();
        let mut deployment = sample_deployment_state(&bundle);
        deployment.config_generation = 4;
        deployment.secret_generation = 3;
        deployment.service_configs.insert(
            "ui/settings".to_string(),
            SoraServiceConfigEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                config_name: "ui/settings".to_string(),
                value_hash: Hash::new(config_value.get().as_bytes()),
                value_json: config_value.clone(),
                last_update_sequence: 12,
            },
        );
        deployment.service_secrets.insert(
            "db/password".to_string(),
            SoraServiceSecretEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                secret_name: "db/password".to_string(),
                envelope: SecretEnvelopeV1 {
                    schema_version: SECRET_ENVELOPE_VERSION_V1,
                    encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
                    key_id: "kms/runtime/test".to_string(),
                    key_version: std::num::NonZeroU32::new(1).expect("non-zero"),
                    nonce: vec![7, 8, 9, 10],
                    ciphertext: service_secret_ciphertext.clone(),
                    commitment: Hash::new(&service_secret_ciphertext),
                    aad_digest: None,
                },
                last_update_sequence: 13,
            },
        );
        let runtime = sample_runtime_state(&bundle);
        let apartment = sample_agent_record()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment.manifest.apartment_name.to_string(), apartment);
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        seed_local_artifact_cache(
            &temp_dir.path().join("artifacts"),
            bundle.container.bundle_hash,
            &bundle_bytes,
            bundle
                .service
                .artifacts
                .iter()
                .zip(artifact_payloads)
                .map(|(artifact, payload)| (artifact.artifact_hash, payload)),
        )?;
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let service_versions = snapshot
            .services
            .get("web_portal")
            .expect("service snapshot present");
        let plan = service_versions
            .get("2026.02.0")
            .expect("service version snapshot present");
        assert_eq!(plan.runtime, SoraContainerRuntimeV1::Ivm);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert_eq!(plan.authoritative_pending_mailbox_messages, 0);
        assert_eq!(plan.config_generation, 4);
        assert_eq!(plan.secret_generation, 3);
        assert_eq!(plan.config_entry_count, 1);
        assert_eq!(plan.secret_entry_count, 1);
        assert_eq!(plan.config_exports.len(), 2);
        assert_eq!(
            plan.effective_env
                .get("UI_SETTINGS_JSON")
                .expect("exported env var"),
            config_value.get()
        );
        assert_eq!(
            snapshot
                .apartments
                .get("ops_agent")
                .expect("apartment snapshot present")
                .process_generation,
            7
        );
        assert!(temp_dir.path().join("runtime_snapshot.json").exists());
        assert!(
            temp_dir
                .path()
                .join("services/web_portal/2026.02.0/runtime_plan.json")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("services/web_portal/2026.02.0/deployment_bundle.json")
                .exists()
        );
        assert_eq!(
            fs::read_to_string(
                temp_dir
                    .path()
                    .join("services/web_portal/2026.02.0/configs/ui/settings"),
            )?,
            config_value.get().clone(),
        );
        let effective_env: BTreeMap<String, String> = read_json_optional(
            &temp_dir
                .path()
                .join("services/web_portal/2026.02.0/effective_env.json"),
        )?
        .expect("effective env should exist");
        assert_eq!(
            effective_env
                .get("UI_SETTINGS_JSON")
                .expect("exported env var"),
            config_value.get()
        );
        assert_eq!(
            fs::read_to_string(
                temp_dir
                    .path()
                    .join("services/web_portal/2026.02.0/config_exports/runtime/ui_settings.json"),
            )?,
            config_value.get().clone(),
        );
        let materialized_secret_entry: SoraServiceSecretEntryV1 = read_json_optional(
            &temp_dir
                .path()
                .join("services/web_portal/2026.02.0/secret_envelopes/db/password"),
        )?
        .expect("materialized secret envelope should exist");
        assert_eq!(materialized_secret_entry.secret_name, "db/password");
        assert_eq!(
            fs::read(
                temp_dir
                    .path()
                    .join("secrets/web_portal/2026.02.0/db/password"),
            )?,
            service_secret_ciphertext
        );
        assert!(temp_dir.path().join("journals").exists());
        assert!(temp_dir.path().join("checkpoints").exists());
        assert!(temp_dir.path().join("secrets").exists());
        assert!(
            temp_dir
                .path()
                .join("apartments/ops_agent/runtime_plan.json")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("apartments/ops_agent/apartment_manifest.json")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_stale_authoritative_service_materializations() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "prune-materialization");
        let config_value = Json::new(true);
        let mut deployment = sample_deployment_state(&bundle);
        deployment.config_generation = 1;
        deployment.secret_generation = 1;
        deployment.service_configs.insert(
            "runtime/feature_flag".to_string(),
            SoraServiceConfigEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                config_name: "runtime/feature_flag".to_string(),
                value_hash: Hash::new(config_value.get().as_bytes()),
                value_json: config_value.clone(),
                last_update_sequence: 3,
            },
        );
        deployment.service_secrets.insert(
            "db/password".to_string(),
            SoraServiceSecretEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                secret_name: "db/password".to_string(),
                envelope: SecretEnvelopeV1 {
                    schema_version: SECRET_ENVELOPE_VERSION_V1,
                    encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
                    key_id: "kms/runtime/test".to_string(),
                    key_version: std::num::NonZeroU32::new(1).expect("non-zero"),
                    nonce: vec![1, 2, 3, 4],
                    ciphertext: b"prune-me".to_vec(),
                    commitment: Hash::new(b"prune-me"),
                    aad_digest: None,
                },
                last_update_sequence: 4,
            },
        );
        let runtime = sample_runtime_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        seed_local_artifact_cache(
            &temp_dir.path().join("artifacts"),
            bundle.container.bundle_hash,
            &bundle_bytes,
            bundle
                .service
                .artifacts
                .iter()
                .zip(artifact_payloads)
                .map(|(artifact, payload)| (artifact.artifact_hash, payload)),
        )?;
        manager.reconcile_once()?;

        let config_path = temp_dir
            .path()
            .join("services/web_portal/2026.02.0/configs/runtime/feature_flag");
        let secret_envelope_path = temp_dir
            .path()
            .join("services/web_portal/2026.02.0/secret_envelopes/db/password");
        let secret_payload_path = temp_dir
            .path()
            .join("secrets/web_portal/2026.02.0/db/password");
        assert!(config_path.exists());
        assert!(secret_envelope_path.exists());
        assert!(secret_payload_path.exists());

        drop(manager);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            let deployments = world.soracloud_service_deployments_mut_for_testing();
            let mut deployment = deployments
                .view()
                .get(&bundle.service.service_name)
                .cloned()
                .expect("deployment state should remain present");
            deployment.config_generation = 2;
            deployment.secret_generation = 2;
            deployment.service_configs.clear();
            deployment.service_secrets.clear();
            deployments.insert(bundle.service.service_name.clone(), deployment);
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("service version snapshot present");
        assert_eq!(plan.config_entry_count, 0);
        assert_eq!(plan.secret_entry_count, 0);
        assert_eq!(plan.config_generation, 2);
        assert_eq!(plan.secret_generation, 2);
        assert!(!config_path.exists());
        assert!(!secret_envelope_path.exists());
        assert!(!secret_payload_path.exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_projects_hf_source_runtime_readiness_from_bound_services() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "query"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "hf-runtime-ready");
        let source_id = Hash::new(b"hf-source");
        let pool_id = Hash::new(b"hf-pool");
        let lease_asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x00,
        ])
        .expect("asset definition");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
            world.soracloud_hf_sources_mut_for_testing().insert(
                source_id,
                SoraHfSourceRecordV1 {
                    schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                    source_id,
                    repo_id: "openai/gpt-oss".to_owned(),
                    resolved_revision: "main".to_owned(),
                    model_name: "gpt_oss_20b".to_owned(),
                    adapter_id: "hf.shared.v1".to_owned(),
                    normalized_runtime_hash: Hash::new(b"hf-runtime"),
                    resource_profile: Some(sample_hf_resource_profile_for_tests()),
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
                        storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                        lease_asset_definition_id,
                        base_fee_nanos: 10_000,
                        lease_term_ms: 60_000,
                        window_started_at_ms: 10,
                        window_expires_at_ms: 60_010,
                        active_member_count: 1,
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
                        joined_at_ms: 10,
                        updated_at_ms: 20,
                        total_paid_nanos: 10_000,
                        total_refunded_nanos: 0,
                        last_charge_nanos: 10_000,
                        total_compute_paid_nanos: 0,
                        total_compute_refunded_nanos: 0,
                        last_compute_charge_nanos: 0,
                        service_bindings: BTreeSet::from([bundle.service.service_name.to_string()]),
                        apartment_bindings: BTreeSet::new(),
                    },
                );
        }

        let temp_dir = tempfile::tempdir()?;
        seed_local_artifact_cache(
            &temp_dir.path().join("artifacts"),
            bundle.container.bundle_hash,
            &bundle_bytes,
            bundle
                .service
                .artifacts
                .iter()
                .zip(artifact_payloads)
                .map(|(artifact, payload)| (artifact.artifact_hash, payload)),
        )?;
        let source_root = temp_dir
            .path()
            .join("hf_sources")
            .join(sanitize_path_component(&source_id.to_string()));
        fs::create_dir_all(source_root.join("files"))?;
        write_json_atomic(
            &source_root.join("import_manifest.json"),
            &HfLocalImportManifestV1 {
                schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
                source_id: source_id.to_string(),
                repo_id: "openai/gpt-oss".to_owned(),
                requested_revision: "main".to_owned(),
                resolved_commit: Some("fixture-commit".to_owned()),
                model_name: "gpt_oss_20b".to_owned(),
                adapter_id: "hf.shared.v1".to_owned(),
                pipeline_tag: Some("text-generation".to_owned()),
                library_name: Some("transformers".to_owned()),
                tags: vec!["text-generation".to_owned()],
                imported_at_ms: 20,
                imported_files: Vec::new(),
                skipped_files: Vec::new(),
                raw_model_info_path: None,
                import_error: None,
            },
        )?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let projection = manager
            .snapshot
            .read()
            .hf_sources
            .get(&source_id.to_string())
            .cloned()
            .expect("hf runtime projection");
        assert_eq!(
            projection.runtime_status,
            SoracloudRuntimeHfSourceStatus::Ready
        );
        assert_eq!(
            projection.bound_service_names,
            vec!["web_portal".to_owned()]
        );
        assert_eq!(
            projection.materialized_service_names,
            vec!["web_portal".to_owned()]
        );
        assert_eq!(projection.bundle_cache_miss_count, 0);
        assert_eq!(projection.artifact_cache_miss_count, 0);
        Ok(())
    }

    #[test]
    fn reconcile_once_synthesizes_generated_hf_bundle_without_sorafs_importer() -> Result<()> {
        let mut state = test_state()?;
        let source_id = Hash::new(b"generated-hf-source");
        let pool_id = Hash::new(b"generated-hf-pool");
        let lease_asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x01,
        ])
        .expect("asset definition");
        let bundle = iroha_core::soracloud_runtime::build_soracloud_hf_generated_service_bundle(
            "hf_generated_service"
                .parse()
                .expect("valid generated service name"),
            &source_id.to_string(),
            "openai/gpt-oss",
            "main",
            "gpt-oss",
        );
        let apartment_manifest =
            iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
                "hf_generated_agent"
                    .parse()
                    .expect("valid generated apartment name"),
                &bundle,
            );

        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_agent_apartments_mut_for_testing().insert(
                apartment_manifest.apartment_name.to_string(),
                SoraAgentApartmentRecordV1 {
                    schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
                    manifest_hash: Hash::new(Encode::encode(&apartment_manifest)),
                    manifest: apartment_manifest.clone(),
                    status: SoraAgentRuntimeStatusV1::Running,
                    deployed_sequence: 1,
                    lease_started_sequence: 1,
                    lease_expires_sequence: 128,
                    last_renewed_sequence: 1,
                    restart_count: 0,
                    last_restart_sequence: None,
                    last_restart_reason: None,
                    process_generation: 1,
                    process_started_sequence: 1,
                    last_active_sequence: 1,
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
                    autonomy_budget_ceiling_units: 1_000,
                    autonomy_budget_remaining_units: 1_000,
                    artifact_allowlist: BTreeMap::new(),
                    autonomy_run_history: Vec::new(),
                        },
            );
            world.soracloud_hf_sources_mut_for_testing().insert(
                source_id,
                SoraHfSourceRecordV1 {
                    schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
                    source_id,
                    repo_id: "openai/gpt-oss".to_owned(),
                    resolved_revision: "main".to_owned(),
                    model_name: "gpt-oss".to_owned(),
                    adapter_id: "hf.shared.v1".to_owned(),
                    normalized_runtime_hash: Hash::new(b"generated-hf-runtime"),
                    resource_profile: Some(sample_hf_resource_profile_for_tests()),
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
                        storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                        lease_asset_definition_id,
                        base_fee_nanos: 10_000,
                        lease_term_ms: 60_000,
                        window_started_at_ms: 10,
                        window_expires_at_ms: 60_010,
                        active_member_count: 1,
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
                        joined_at_ms: 10,
                        updated_at_ms: 20,
                        total_paid_nanos: 10_000,
                        total_refunded_nanos: 0,
                        last_charge_nanos: 10_000,
                        total_compute_paid_nanos: 0,
                        total_compute_refunded_nanos: 0,
                        last_compute_charge_nanos: 0,
                        service_bindings: BTreeSet::from([bundle.service.service_name.to_string()]),
                        apartment_bindings: BTreeSet::from([apartment_manifest
                            .apartment_name
                            .to_string()]),
                    },
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let source_root = temp_dir
            .path()
            .join("hf_sources")
            .join(sanitize_path_component(&source_id.to_string()));
        fs::create_dir_all(source_root.join("files"))?;
        write_json_atomic(
            &source_root.join("import_manifest.json"),
            &HfLocalImportManifestV1 {
                schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
                source_id: source_id.to_string(),
                repo_id: "openai/gpt-oss".to_owned(),
                requested_revision: "main".to_owned(),
                resolved_commit: Some("fixture-commit".to_owned()),
                model_name: "gpt-oss".to_owned(),
                adapter_id: "hf.shared.v1".to_owned(),
                pipeline_tag: Some("text-generation".to_owned()),
                library_name: Some("transformers".to_owned()),
                tags: vec!["text-generation".to_owned()],
                imported_at_ms: 20,
                imported_files: Vec::new(),
                skipped_files: Vec::new(),
                raw_model_info_path: None,
                import_error: None,
            },
        )?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let service_plan = snapshot
            .services
            .get("hf_generated_service")
            .and_then(|versions| versions.get("hf.generated.v1"))
            .expect("generated service plan");
        assert!(service_plan.bundle_available_locally);
        assert_eq!(service_plan.artifacts.len(), 1);

        let projection = snapshot
            .hf_sources
            .get(&source_id.to_string())
            .cloned()
            .expect("generated hf projection");
        assert_eq!(
            projection.runtime_status,
            SoracloudRuntimeHfSourceStatus::Ready
        );
        assert_eq!(
            projection.materialized_service_names,
            vec!["hf_generated_service".to_owned()]
        );
        assert_eq!(
            projection.materialized_apartment_names,
            vec!["hf_generated_agent".to_owned()]
        );

        let cached_bundle = temp_dir
            .path()
            .join("artifacts")
            .join(hash_cache_name(bundle.container.bundle_hash));
        assert!(cached_bundle.exists());
        assert_eq!(
            fs::read(cached_bundle)?,
            iroha_core::soracloud_runtime::soracloud_hf_generated_service_contract_artifact()
        );
        Ok(())
    }

    #[test]
    fn hf_import_file_selected_matches_exact_and_suffix_patterns() {
        let allowlist = vec![
            "config.json".to_owned(),
            "*.safetensors".to_owned(),
            "tokenizer.json".to_owned(),
        ];
        assert!(hf_import_file_selected("config.json", &allowlist));
        assert!(hf_import_file_selected("MODEL.SAFETENSORS", &allowlist));
        assert!(hf_import_file_selected("tokenizer.json", &allowlist));
        assert!(!hf_import_file_selected("README.md", &allowlist));
        assert!(!hf_import_file_selected("config.yaml", &allowlist));
    }

    #[test]
    fn hf_url_helpers_build_expected_routes() -> Result<()> {
        let info = hf_model_info_url("huggingface.co/api", "openai-community/gpt2", "main")?;
        let file = hf_repo_file_url(
            "https://huggingface.co",
            "openai-community/gpt2",
            "main",
            "config.json",
        )?;
        let inference = hf_inference_url(
            "router.huggingface.co/hf-inference/models",
            "openai-community/gpt2",
        )?;
        assert_eq!(
            info.as_str(),
            "https://huggingface.co/api/models/openai-community/gpt2/revision/main"
        );
        assert_eq!(
            file.as_str(),
            "https://huggingface.co/openai-community/gpt2/resolve/main/config.json"
        );
        assert_eq!(
            inference.as_str(),
            "https://router.huggingface.co/hf-inference/models/openai-community/gpt2"
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_imports_hf_source_into_shared_local_cache() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_import_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let config_json = br#"{"model_type":"gpt2"}"#.to_vec();
        let tokenizer_json = br#"{"version":"1.0"}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation", "causal-lm"],
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "tokenizer.json"},
                {"rfilename": "weights.safetensors"},
                {"rfilename": "README.md"}
            ]
        });
        let model_info_body = norito::json::to_vec(&model_info)?;

        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(model_info_body),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json.clone()),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/tokenizer.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(tokenizer_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/tokenizer.json".to_owned(),
            ),
            HttpFixtureResponse::json(tokenizer_json.clone()),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/weights.safetensors".to_owned(),
            ),
            HttpFixtureResponse::head_ok("application/octet-stream", 1_024),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec![
            "config.json".to_owned(),
            "tokenizer.json".to_owned(),
            "*.safetensors".to_owned(),
        ];
        config.hf.import_max_file_bytes = 128;
        config.hf.import_max_total_bytes = 512;

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let source_id = fixture.source_id.to_string();
        let manifest = read_hf_import_manifest(temp_dir.path(), &source_id)?
            .expect("reconcile should write an HF import manifest");
        assert_eq!(manifest.source_id, source_id);
        assert_eq!(manifest.repo_id, "openai-community/gpt2");
        assert_eq!(manifest.requested_revision, "main");
        assert_eq!(manifest.resolved_commit.as_deref(), Some("commit-123"));
        assert_eq!(manifest.pipeline_tag.as_deref(), Some("text-generation"));
        assert_eq!(manifest.library_name.as_deref(), Some("transformers"));
        assert_eq!(manifest.imported_files.len(), 2);
        assert_eq!(
            manifest
                .imported_files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            vec!["config.json".to_owned(), "tokenizer.json".to_owned()]
        );
        assert!(
            manifest
                .skipped_files
                .iter()
                .any(|entry| entry.contains("weights.safetensors"))
        );
        assert_eq!(
            fs::read(&manifest.imported_files[0].local_path)?,
            config_json
        );
        assert_eq!(
            fs::read(&manifest.imported_files[1].local_path)?,
            tokenizer_json
        );
        let projection = manager
            .snapshot
            .read()
            .hf_sources
            .get(&fixture.source_id.to_string())
            .cloned()
            .expect("runtime snapshot should include the imported HF source");
        assert_eq!(
            projection.runtime_status,
            SoracloudRuntimeHfSourceStatus::Ready
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_records_hf_import_error_manifest_on_failure() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_import_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::text(500, "boom"),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let manifest = read_hf_import_manifest(temp_dir.path(), &fixture.source_id.to_string())?
            .expect("failed imports should still leave an HF error manifest");
        assert!(manifest.imported_files.is_empty());
        assert!(manifest.import_error.is_some());
        assert!(
            manifest
                .import_error
                .as_deref()
                .is_some_and(|message| message.contains("returned 500"))
        );
        let projection = manager
            .snapshot
            .read()
            .hf_sources
            .get(&fixture.source_id.to_string())
            .cloned()
            .expect("runtime snapshot should include the failed HF source");
        assert_eq!(
            projection.runtime_status,
            SoracloudRuntimeHfSourceStatus::Failed
        );
        assert!(
            projection
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("returned 500"))
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_reports_warmup_no_show_for_local_warming_host_import_failure() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_import_warmup_no_show",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWLocalWarmupNoShowRuntimeHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warming,
            local_peer_id,
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::text(500, "boom"),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        manager.reconcile_once()?;

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(reports[0].kind, SoraModelHostViolationKindV1::WarmupNoShow);
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("failed before readiness"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_metadata_reports_import_manifest() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_metadata_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let config_json = br#"{"model_type":"gpt2"}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-456",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json.clone()),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.inference_token = Some("hf-runtime-token".to_owned());

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "metadata".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "GET".to_owned(),
                request_path: "/metadata".to_owned(),
                handler_path: "/metadata".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"hf-metadata-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        let decoded: HfGeneratedMetadataResponse =
            norito::json::from_slice(&response.response_bytes)?;
        assert!(decoded.imported);
        assert_eq!(decoded.repo_id, "openai-community/gpt2");
        assert_eq!(decoded.requested_revision, "main");
        assert_eq!(decoded.resolved_commit.as_deref(), Some("commit-456"));
        assert_eq!(decoded.imported_file_count, 1);
        assert_eq!(decoded.imported_total_bytes, config_json.len() as u64);
        assert_eq!(decoded.imported_files[0].path, "config.json");
        assert!(decoded.inference_local_enabled);
        assert!(decoded.inference_bridge_enabled);
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::AuditReceipt
        );
        assert!(response.runtime_receipt.is_some());
        Ok(())
    }

    #[test]
    fn reconcile_once_imports_generated_hf_source_only_for_locally_assigned_host() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_local_assignment_import",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWLocalAssignedRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json = br#"{"model_type":"gpt2"}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-assigned-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];

        let unassigned_manager = SoracloudRuntimeManager::new(
            config
                .clone()
                .with_local_host_identity(ALICE_ID.clone(), "12D3KooWUnassignedRuntimeHost"),
            Arc::clone(&state),
        );
        unassigned_manager.reconcile_once()?;
        assert!(
            read_hf_import_manifest(temp_dir.path(), &fixture.source_id.to_string())?.is_none(),
            "HF sources should stay metadata-only on unassigned hosts",
        );

        let assigned_manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        assigned_manager.reconcile_once()?;
        assert!(
            read_hf_import_manifest(temp_dir.path(), &fixture.source_id.to_string())?.is_some(),
            "assigned hosts should materialize the canonical HF import",
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconcile_task_imports_generated_hf_source_without_panicking() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_async_reconcile_import",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWAsyncReconcileRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json = br#"{"model_type":"gpt2"}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-async-reconcile-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];

        let manager = Arc::new(SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        ));
        let shutdown = ShutdownSignal::new();
        let task = Arc::clone(&manager).spawn_reconcile_task(shutdown.clone());
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if read_hf_import_manifest(temp_dir.path(), &fixture.source_id.to_string())?
                    .is_some()
                {
                    break Ok::<(), eyre::Report>(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| eyre::eyre!("timed out waiting for background HF import manifest"))??;
        shutdown.send();
        task.await.expect("reconcile task should shut down cleanly");

        assert!(
            read_hf_import_manifest(temp_dir.path(), &fixture.source_id.to_string())?.is_some(),
            "background reconcile should import the assigned HF source without panicking",
        );
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_requires_configured_token() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_infer_requires_token",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let (server, _captured) = spawn_recording_http_route_fixture(BTreeMap::new())?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.inference_base_url = format!("{}/hf-inference/models", server.base_url);
        config.hf.local_execution_enabled = false;
        config.hf.allow_inference_bridge_fallback = true;
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::from([(
                    HF_ALLOW_BRIDGE_FALLBACK_HEADER_V1.to_owned(),
                    "1".to_owned(),
                )]),
                request_body: br#"{"inputs":"hello"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-infer-no-token"),
            })
            .expect_err("generated HF inference should require a configured token");
        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(
            error
                .message
                .contains("requires `soracloud_runtime.hf.inference_token`")
        );
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_does_not_bridge_without_explicit_opt_in() -> Result<()>
    {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_infer_no_bridge_opt_in",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let (server, _captured) = spawn_recording_http_route_fixture(BTreeMap::new())?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.inference_base_url = format!("{}/hf-inference/models", server.base_url);
        config.hf.local_execution_enabled = false;
        config.hf.allow_inference_bridge_fallback = true;
        config.hf.inference_token = Some("hf-test-token".to_owned());
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: br#"{"inputs":"hello"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-infer-no-bridge-opt-in"),
            })
            .expect_err("generated HF inference should fail closed without bridge opt-in");
        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("has no enabled runtime backend"));
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_executes_imported_model_locally() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_local_infer_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let local_peer_id = "12D3KooWLocalInferRuntimeHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"echo","prefix":"local:"}}"#
                .to_vec();
        let model_info = norito::json!({
            "sha": "commit-local-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let request_body =
            br#"{"inputs":"Hello from Soracloud","parameters":{"max_new_tokens":4}}"#.to_vec();
        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: Some("wait_for_model=true".to_owned()),
                request_headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
                request_body,
                request_commitment: Hash::new(b"hf-local-infer-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        let decoded: norito::json::Value = norito::json::from_slice(&response.response_bytes)?;
        assert_eq!(
            decoded.get("backend").and_then(norito::json::Value::as_str),
            Some("local_fixture")
        );
        assert_eq!(
            decoded.get("repo_id").and_then(norito::json::Value::as_str),
            Some("openai-community/gpt2")
        );
        assert_eq!(
            decoded.get("inputs").and_then(norito::json::Value::as_str),
            Some("Hello from Soracloud")
        );
        assert_eq!(
            decoded.get("text").and_then(norito::json::Value::as_str),
            Some("local:Hello from Soracloud")
        );
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::AuditReceipt
        );
        let runtime_receipt = response
            .runtime_receipt
            .as_ref()
            .expect("generated HF local inference should emit a runtime receipt");
        let expected_validator = ALICE_ID.clone();
        assert_eq!(runtime_receipt.placement_id, Some(placement_id));
        assert_eq!(
            runtime_receipt.selected_validator_account_id.as_ref(),
            Some(&expected_validator)
        );
        assert_eq!(
            runtime_receipt.selected_peer_id.as_deref(),
            Some(local_peer_id)
        );
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_reuses_resident_worker_across_calls() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_local_worker_reuse_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let local_peer_id = "12D3KooWLocalReuseRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"echo","prefix":"reuse:"}}"#
                .to_vec();
        let model_info = norito::json!({
            "sha": "commit-local-reuse-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let first = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
                request_body: br#"{"inputs":"first"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-local-worker-reuse-first"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;
        let second = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
                request_body: br#"{"inputs":"second"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-local-worker-reuse-second"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        let first_json: norito::json::Value = norito::json::from_slice(&first.response_bytes)?;
        let second_json: norito::json::Value = norito::json::from_slice(&second.response_bytes)?;
        assert_eq!(
            first_json
                .get("worker_instance_id")
                .and_then(norito::json::Value::as_str),
            second_json
                .get("worker_instance_id")
                .and_then(norito::json::Value::as_str)
        );
        assert_eq!(
            first_json.get("text").and_then(norito::json::Value::as_str),
            Some("reuse:first")
        );
        assert_eq!(
            second_json
                .get("text")
                .and_then(norito::json::Value::as_str),
            Some("reuse:second")
        );
        assert_eq!(manager.hf_local_workers.lock().len(), 1);
        Ok(())
    }

    #[test]
    fn reconcile_once_starts_resident_worker_for_local_warm_replica() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_replica_worker_probe_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWReplicaProbeRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"echo","prefix":"probe:"}}"#
                .to_vec();
        let model_info = norito::json!({
            "sha": "commit-replica-probe-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        assert_eq!(manager.hf_local_workers.lock().len(), 1);
        Ok(())
    }

    #[test]
    fn reconcile_once_submits_model_host_heartbeat_after_successful_warming_probe() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_warming_probe_heartbeat_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWWarmingProbeRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warming,
            local_peer_id,
        );
        let current_heartbeat_expiry_ms = soracloud_runtime_observed_at_ms().saturating_add(20_000);
        insert_local_model_host_capability_fixture(
            &mut state,
            &ALICE_ID,
            local_peer_id,
            current_heartbeat_expiry_ms,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"echo","prefix":"warm:"}}"#
                .to_vec();
        let model_info = norito::json!({
            "sha": "commit-warming-heartbeat-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        manager.reconcile_once()?;

        let heartbeats = mutation_sink.submitted_model_host_heartbeats();
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].validator_account_id, *ALICE_ID);
        assert!(heartbeats[0].heartbeat_expires_at_ms > current_heartbeat_expiry_ms);
        assert_eq!(
            heartbeats[0].provenance.signer,
            ALICE_KEYPAIR.public_key().clone()
        );
        assert_eq!(manager.hf_local_workers.lock().len(), 1);
        assert!(mutation_sink.submitted_violation_reports().is_empty());
        Ok(())
    }

    #[test]
    fn reconcile_once_reports_advert_contradiction_for_local_peer_mismatch() -> Result<()> {
        let mut state = test_state()?;
        insert_local_model_host_capability_fixture(
            &mut state,
            &ALICE_ID,
            "12D3KooWAdvertisedDifferentPeer",
            soracloud_runtime_observed_at_ms().saturating_add(30_000),
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), "12D3KooWActualLocalPeer"),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());

        manager.reconcile_once()?;

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AdvertContradiction
        );
        assert_eq!(reports[0].placement_id, None);
        assert!(reports[0].detail.as_deref().is_some_and(|detail| {
            detail.contains("does not match the authoritative model-host advert peer id")
        }));
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn report_generated_hf_proxy_failure_reports_primary_host_violation() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_proxy_primary_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWLocalProxyIngressHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        handle.report_generated_hf_proxy_failure(
            &SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: br#"{"inputs":"hello"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-proxy-primary-failure"),
            },
            "12D3KooWGeneratedHfFixturePrimary",
            &SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "proxy request timed out",
            ),
        );

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *BOB_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("targeting primary peer"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn report_generated_hf_local_proxy_failure_reports_local_replica_violation() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_local_proxy_replica_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWLocalReplicaForwardingFailure";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        handle.report_generated_hf_local_proxy_failure(
            &SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: br#"{"inputs":"hello"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-local-replica-forwarding-failure"),
            },
            &SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "Soracloud proxy routing requires an attached P2P network",
            ),
        );

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0].detail.as_deref().is_some_and(
                |detail| detail.contains("failed to forward generated-HF proxy traffic")
            )
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_reconcile_submits_reconcile_when_no_warm_primary() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_missing_primary_reconcile_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWMissingWarmPrimaryReplica";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        set_generated_hf_primary_assignment_status(
            &mut state,
            &fixture,
            SoraHfPlacementHostStatusV1::Warming,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-missing-primary-reconcile"),
        };
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "generated HF service has no warm authoritative primary host",
        );

        handle.request_generated_hf_reconcile(&request, &error);
        handle.request_generated_hf_reconcile(&request, &error);

        assert!(mutation_sink.submitted_violation_reports().is_empty());
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_reconcile_reports_warm_primary_authority_failure() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_primary_authority_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWPrimaryAuthorityFailureHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-primary-authority-failure-reconcile"),
        };
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "authoritative primary rejected proxy execution",
        );

        handle.request_generated_hf_reconcile(&request, &error);
        handle.request_generated_hf_reconcile(&request, &error);

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("local authoritative primary peer"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_reconcile_reports_warming_primary_authority_failure() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_warming_primary_authority_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWWarmingPrimaryAuthorityFailureHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warming,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-warming-primary-authority-failure-reconcile"),
        };
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "warming primary rejected proxy execution",
        );

        handle.request_generated_hf_reconcile(&request, &error);
        handle.request_generated_hf_reconcile(&request, &error);

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(reports[0].kind, SoraModelHostViolationKindV1::WarmupNoShow);
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("local authoritative primary peer"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_reconcile_submits_reconcile_for_assigned_replica_authority_failure()
    -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_replica_authority_failure_reconcile_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWReplicaAuthorityFailureHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-replica-authority-failure-reconcile"),
        };
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "local peer rejected generated-HF proxy execution because it is not the authoritative warm primary",
        );

        handle.request_generated_hf_reconcile(&request, &error);
        handle.request_generated_hf_reconcile(&request, &error);

        assert!(mutation_sink.submitted_violation_reports().is_empty());
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_proxy_responder_reconcile_submits_for_assigned_replica() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_unexpected_responder_reconcile_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWGeneratedHfFixtureReplica";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-unexpected-responder-reconcile"),
        };

        handle.request_generated_hf_proxy_responder_reconcile(
            &request,
            "12D3KooWGeneratedHfFixtureReplica",
            "12D3KooWGeneratedHfFixturePrimary",
        );
        handle.request_generated_hf_proxy_responder_reconcile(
            &request,
            "12D3KooWGeneratedHfFixtureReplica",
            "12D3KooWGeneratedHfFixturePrimary",
        );

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("answered a generated-HF proxy response"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_proxy_responder_reconcile_reports_warming_responder_no_show()
    -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_unexpected_warming_responder_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWGeneratedHfFixtureReplica";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warming,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-unexpected-warming-responder-reconcile"),
        };

        handle.request_generated_hf_proxy_responder_reconcile(
            &request,
            "12D3KooWGeneratedHfFixtureReplica",
            "12D3KooWGeneratedHfFixturePrimary",
        );

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert_eq!(reports[0].kind, SoraModelHostViolationKindV1::WarmupNoShow);
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn request_generated_hf_proxy_responder_reconcile_ignores_unassigned_responder() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_unexpected_unassigned_responder_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWGeneratedHfFixtureReplica";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(PathBuf::from("/tmp/test-soracloud-runtime"))
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: br#"{"inputs":"hello"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-unexpected-unassigned-responder-reconcile"),
        };

        handle.request_generated_hf_proxy_responder_reconcile(
            &request,
            "12D3KooWUnexpectedUnassignedResponder",
            "12D3KooWGeneratedHfFixturePrimary",
        );

        assert!(mutation_sink.submitted_violation_reports().is_empty());
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 0);
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_reports_primary_worker_failure_once() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_local_worker_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let local_peer_id = "12D3KooWLocalWorkerFailureRuntimeHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"explode"}}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-local-worker-failure-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let build_request = || SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: fixture.bundle.service.service_name.to_string(),
            service_version: fixture.bundle.service.service_version.clone(),
            handler_name: "infer".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "POST".to_owned(),
            request_path: "/infer".to_owned(),
            handler_path: "/infer".to_owned(),
            request_query: None,
            request_headers: BTreeMap::from([(
                "content-type".to_owned(),
                "application/json".to_owned(),
            )]),
            request_body: br#"{"inputs":"failure"}"#.to_vec(),
            request_commitment: Hash::new(b"hf-local-worker-failure"),
        };

        let first_error = handle
            .execute_local_read(build_request())
            .expect_err("generated HF inference should fail when the resident worker errors");
        assert_eq!(
            first_error.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(
            first_error
                .message
                .contains("unsupported _soracloud_fixture mode")
        );
        let second_error = handle
            .execute_local_read(build_request())
            .expect_err("failure should remain visible on repeated calls");
        assert_eq!(
            second_error.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("primary host"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        Ok(())
    }

    #[test]
    fn reconcile_once_reports_warm_replica_worker_failure() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_replica_worker_failure_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        let local_peer_id = "12D3KooWReplicaWorkerFailureRuntimeHost";
        let placement_id = insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );
        let config_json =
            br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"explode"}}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-replica-worker-failure-123",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        let (server, _captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.allow_inference_bridge_fallback = false;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());

        let manager = SoracloudRuntimeManager::new(
            config.with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());
        manager.reconcile_once()?;

        let reports = mutation_sink.submitted_violation_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].validator_account_id, *ALICE_ID);
        assert_eq!(
            reports[0].kind,
            SoraModelHostViolationKindV1::AssignedHeartbeatMiss
        );
        assert_eq!(reports[0].placement_id, Some(placement_id));
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("warm replica host"))
        );
        assert_eq!(mutation_sink.submitted_model_host_reconciles(), 1);
        assert_eq!(manager.hf_local_workers.lock().len(), 1);
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_rejects_local_replica_without_proxy() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_replica_infer_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let local_peer_id = "12D3KooWReplicaRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Replica,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: None,
                request_headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
                request_body: br#"{"inputs":"hello"}"#.to_vec(),
                request_commitment: Hash::new(b"hf-replica-infer-request"),
            })
            .expect_err("replica hosts should fail closed until proxy-to-primary is implemented");
        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("replica"));
        Ok(())
    }

    #[test]
    fn execute_local_read_generated_hf_infer_forwards_request_to_inference_bridge() -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_infer_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let config_json = br#"{"model_type":"gpt2"}"#.to_vec();
        let model_info = norito::json!({
            "sha": "commit-789",
            "pipeline_tag": "text-generation",
            "library_name": "transformers",
            "tags": ["text-generation"],
            "siblings": [{"rfilename": "config.json"}]
        });
        let inference_body = br#"{"generated_text":"hello from hf"}"#.to_vec();
        let mut routes = BTreeMap::new();
        routes.insert(
            (
                "GET".to_owned(),
                "/api/models/openai-community/gpt2/revision/main".to_owned(),
            ),
            HttpFixtureResponse::json(norito::json::to_vec(&model_info)?),
        );
        routes.insert(
            (
                "HEAD".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::head_ok(
                "application/json",
                u64::try_from(config_json.len()).expect("fixture length fits in u64"),
            ),
        );
        routes.insert(
            (
                "GET".to_owned(),
                "/openai-community/gpt2/resolve/main/config.json".to_owned(),
            ),
            HttpFixtureResponse::json(config_json),
        );
        routes.insert(
            (
                "POST".to_owned(),
                "/hf-inference/models/openai-community/gpt2?wait_for_model=true".to_owned(),
            ),
            HttpFixtureResponse::json(inference_body.clone())
                .with_header("Content-Encoding", "identity"),
        );
        let (server, captured) = spawn_recording_http_route_fixture(routes)?;

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.hf.hub_base_url = server.base_url.clone();
        config.hf.api_base_url = format!("{}/api", server.base_url);
        config.hf.inference_base_url = format!("{}/hf-inference/models", server.base_url);
        config.hf.local_execution_enabled = false;
        config.hf.allow_inference_bridge_fallback = true;
        config.hf.import_file_allowlist = vec!["config.json".to_owned()];
        config.hf.inference_token = Some("hf-test-token".to_owned());

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let request_body = br#"{"inputs":"Hello from Soracloud"}"#.to_vec();
        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: fixture.bundle.service.service_name.to_string(),
                service_version: fixture.bundle.service.service_version.clone(),
                handler_name: "infer".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/infer".to_owned(),
                handler_path: "/infer".to_owned(),
                request_query: Some("wait_for_model=true".to_owned()),
                request_headers: BTreeMap::from([
                    ("content-type".to_owned(), "application/json".to_owned()),
                    ("accept".to_owned(), "application/json".to_owned()),
                    (
                        HF_ALLOW_BRIDGE_FALLBACK_HEADER_V1.to_owned(),
                        "1".to_owned(),
                    ),
                ]),
                request_body: request_body.clone(),
                request_commitment: Hash::new(b"hf-infer-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(response.response_bytes, inference_body);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(response.content_encoding.as_deref(), Some("identity"));
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::AuditReceipt
        );
        assert!(response.runtime_receipt.is_some());

        let captured = captured.lock().expect("fixture capture mutex").clone();
        let inference_request = captured
            .iter()
            .find(|request| {
                request.method == "POST"
                    && request.path
                        == "/hf-inference/models/openai-community/gpt2?wait_for_model=true"
            })
            .cloned()
            .expect("fixture should capture the forwarded inference request");
        assert_eq!(
            inference_request
                .headers
                .get("authorization")
                .map(String::as_str),
            Some("Bearer hf-test-token")
        );
        assert_eq!(
            inference_request
                .headers
                .get("content-type")
                .map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            inference_request.headers.get("accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(inference_request.body, request_body);
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_stale_materializations_and_reports_missing_bundle_cache() -> Result<()>
    {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let _artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "missing-bundle");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let stale_dir = temp_dir.path().join("services/stale_service/stale_version");
        fs::create_dir_all(&stale_dir)?;
        fs::write(stale_dir.join("runtime_plan.json"), "{}")?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let bundle_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("bundle plan present");
        assert!(!bundle_plan.bundle_available_locally);
        assert_eq!(
            bundle_plan.health_status,
            SoraServiceHealthStatusV1::Hydrating
        );
        assert!(
            bundle_plan
                .artifacts
                .iter()
                .any(|artifact| artifact.kind == SoraArtifactKindV1::Bundle
                    && !artifact.available_locally)
        );
        assert!(!temp_dir.path().join("services/stale_service").exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_marks_hydrated_ivm_service_healthy_without_runtime_state() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.service.artifacts.clear();
        let bundle_bytes = simple_soracloud_contract_artifact(&["query"]);
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get(bundle.service.service_name.as_ref())
            .and_then(|versions| versions.get(&bundle.service.service_version))
            .expect("hydrated IVM service plan");
        assert_eq!(plan.runtime, SoraContainerRuntimeV1::Ivm);
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| artifact.available_locally)
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_projects_http_service_inrou_runtime_into_snapshot() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        let deployment_state = sample_deployment_state(&bundle);
        let expected_process_generation = deployment_state.process_generation;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
        }
        let local_peer_id = "12D3KooWHttpServiceInrouRuntimeHost";
        insert_inrou_service_placement_fixture(
            &mut state,
            &bundle,
            local_peer_id,
            1..=bundle.service.replicas.get(),
        );

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get(bundle.service.service_name.as_ref())
            .and_then(|versions| versions.get(&bundle.service.service_version))
            .expect("Inrou runtime plan present");
        assert_eq!(plan.runtime, SoraContainerRuntimeV1::Inrou);
        assert_eq!(plan.process_generation, Some(expected_process_generation));
        assert_eq!(plan.desired_replica_count, bundle.service.replicas.get());
        assert_eq!(
            plan.local_replica_slots,
            (1..=bundle.service.replicas.get()).collect::<Vec<_>>()
        );
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Hydrating);
        Ok(())
    }

    #[test]
    fn reconcile_once_stamps_snapshot_with_local_peer_identity() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
        }

        let local_peer_id = "12D3KooWSnapshotOriginRuntimeHost";
        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        assert_eq!(snapshot.local_peer_id.as_deref(), Some(local_peer_id));
        Ok(())
    }

    #[test]
    fn reconcile_once_proxy_only_inrou_host_does_not_publish_replica_runtime_state() -> Result<()> {
        let mut state = test_state()?;
        let bundle = sample_inrou_test_bundle()?;
        let local_peer_id = "12D3KooWProxyOnlyRuntimeHost";
        let deployment_state = sample_deployment_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
            world
                .soracloud_inrou_service_placements_mut_for_testing()
                .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa: current_host_inrou_guest_isa(),
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), local_peer_id);
        config.inrou.proxy_only = true;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state))
            .with_mutation_sink(mutation_sink.clone());

        manager.reconcile_once()?;

        let capabilities = mutation_sink.submitted_inrou_host_capabilities();
        assert_eq!(capabilities.len(), 1);
        assert!(capabilities[0].capability.proxy_only);
        assert_eq!(capabilities[0].capability.max_hosted_replica_capacity, 0);
        assert!(
            mutation_sink
                .submitted_inrou_replica_runtime_states()
                .is_empty()
        );
        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get(bundle.service.service_name.as_ref())
            .and_then(|versions| versions.get(&bundle.service.service_version))
            .expect("Inrou runtime plan present");
        assert_eq!(plan.runtime, SoraContainerRuntimeV1::Inrou);
        assert_eq!(plan.process_generation, None);
        assert!(plan.local_replica_slots.is_empty());
        assert!(plan.local_replicas.is_empty());
        assert!(manager.hosted_http_workers.lock().is_empty());
        Ok(())
    }

    #[test]
    fn reconcile_once_materializes_canary_http_service_inrou_runtime_state() -> Result<()> {
        let mut state = test_state()?;
        let mut active_bundle = load_deployment_bundle_fixture()?;
        active_bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        active_bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        active_bundle.service.state_bindings.clear();
        active_bundle.service.handlers.clear();
        active_bundle.service.artifacts.clear();

        let mut canary_bundle = active_bundle.clone();
        canary_bundle.service.service_version = "2026.03.0".to_string();
        canary_bundle.container.bundle_path = "/bundles/web_portal_canary.to".to_string();

        let active_bundle_bytes = simple_soracloud_contract_artifact(&["entry_active"]);
        let canary_bundle_bytes = simple_soracloud_contract_artifact(&["entry_canary"]);
        active_bundle.container.bundle_hash = Hash::new(&active_bundle_bytes);
        canary_bundle.container.bundle_hash = Hash::new(&canary_bundle_bytes);

        let mut deployment = sample_deployment_state(&active_bundle);
        let expected_process_generation = deployment.process_generation;
        deployment.revision_count = 2;
        deployment.active_rollout = Some(SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "rollout-2026-03".to_string(),
            baseline_version: Some(active_bundle.service.service_version.clone()),
            candidate_version: canary_bundle.service.service_version.clone(),
            canary_percent: 20,
            traffic_percent: 20,
            stage: SoraRolloutStageV1::Canary,
            health_failures: 0,
            max_health_failures: 3,
            health_window_secs: 60,
            created_sequence: 17,
            updated_sequence: 29,
        });
        deployment.last_rollout = deployment.active_rollout.clone();

        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    active_bundle.service.service_name.to_string(),
                    active_bundle.service.service_version.clone(),
                ),
                active_bundle.clone(),
            );
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    canary_bundle.service.service_name.to_string(),
                    canary_bundle.service.service_version.clone(),
                ),
                canary_bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(active_bundle.service.service_name.clone(), deployment);
        }
        let local_peer_id = "12D3KooWCanaryHttpServiceInrouRuntimeHost";
        insert_inrou_service_placement_fixture(&mut state, &active_bundle, local_peer_id, [1_u16]);
        insert_inrou_service_placement_fixture(&mut state, &canary_bundle, local_peer_id, [1_u16]);

        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(active_bundle.container.bundle_hash)),
            &active_bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(canary_bundle.container.bundle_hash)),
            &canary_bundle_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let active_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("active service plan present");
        let canary_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.03.0"))
            .expect("canary service plan present");

        assert_eq!(
            active_plan.process_generation,
            Some(expected_process_generation)
        );
        assert_eq!(
            canary_plan.process_generation,
            Some(expected_process_generation)
        );
        assert_eq!(
            canary_plan.role,
            SoracloudRuntimeRevisionRole::CanaryCandidate
        );
        assert_eq!(
            active_plan.health_status,
            SoraServiceHealthStatusV1::Degraded
        );
        assert_eq!(
            canary_plan.health_status,
            SoraServiceHealthStatusV1::Degraded
        );
        assert!(
            temp_dir
                .path()
                .join("services/web_portal/2026.02.0")
                .join(SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1)
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("services/web_portal/2026.03.0")
                .join(SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1)
                .exists()
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_materializes_replica_runtime_state_summary_for_multi_replica_http_service()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.replicas = std::num::NonZeroU16::new(2).expect("replicas");
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        bundle.service.artifacts.clear();
        bundle.service.lease_volumes = vec![
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "root_disk".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                mount_path: "/".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("bytes"),
            },
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "index_state".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                mount_path: "/var/lib/ton-indexer".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(1024 * 1024).expect("bytes"),
            },
        ];

        let bundle_bytes = simple_soracloud_contract_artifact(&["entry_active"]);
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let deployment_state = sample_deployment_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
        }
        let local_peer_id = "12D3KooWMultiReplicaHttpServiceInrouRuntimeHost";
        insert_inrou_service_placement_fixture(&mut state, &bundle, local_peer_id, [1_u16, 2]);

        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let service_dir = temp_dir.path().join("services/web_portal/2026.02.0");
        let summary = read_hosted_http_runtime_state(&service_dir)?;
        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("replicated Inrou plan present");
        assert_eq!(plan.desired_replica_count, 2);
        if inrou_host_platform_supports_local_materialization() {
            let summary = summary.expect("revision runtime summary should be written");
            assert_eq!(plan.local_replica_slots, vec![1, 2]);
            assert_eq!(plan.local_replicas.len(), 2);
            assert_eq!(plan.local_replicas[0].replica_slot, 1);
            assert_eq!(
                plan.local_replicas[0].health_status,
                SoraServiceHealthStatusV1::Degraded
            );
            assert!(
                plan.local_replicas[0]
                    .materialization_dir
                    .ends_with("services/web_portal/2026.02.0/replicas/replica-0001")
            );
            assert_eq!(plan.local_replicas[1].replica_slot, 2);
            assert_eq!(
                plan.local_replicas[1].health_status,
                SoraServiceHealthStatusV1::Degraded
            );
            assert!(
                plan.local_replicas[1]
                    .materialization_dir
                    .ends_with("services/web_portal/2026.02.0/replicas/replica-0002")
            );
            assert_eq!(summary.replicas.len(), 2);
            assert_eq!(summary.health_status, SoraServiceHealthStatusV1::Degraded);
            assert!(summary.listen_base_url.is_none());
            assert_eq!(summary.replicas[0].replica_slot, 1);
            assert_eq!(summary.replicas[1].replica_slot, 2);
            assert!(
                service_dir
                    .join("replicas/replica-0001/runtime_plan.json")
                    .exists()
            );
            assert!(
                service_dir
                    .join("replicas/replica-0002/runtime_plan.json")
                    .exists()
            );
            assert!(
                service_dir
                    .join("replicas/replica-0001")
                    .join(SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1)
                    .exists()
            );
            assert!(
                service_dir
                    .join("replicas/replica-0002")
                    .join(SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1)
                    .exists()
            );
            let replica_one_plan: SoracloudRuntimeServicePlan = read_json_optional(
                service_dir
                    .join("replicas/replica-0001/runtime_plan.json")
                    .as_path(),
            )?
            .expect("replica one runtime plan");
            let replica_two_plan: SoracloudRuntimeServicePlan = read_json_optional(
                service_dir
                    .join("replicas/replica-0002/runtime_plan.json")
                    .as_path(),
            )?
            .expect("replica two runtime plan");
            let replica_one_root = replica_one_plan
                .lease_volumes
                .iter()
                .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
                .expect("replica one root volume");
            let replica_two_root = replica_two_plan
                .lease_volumes
                .iter()
                .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
                .expect("replica two root volume");
            let replica_one_shared = replica_one_plan
                .lease_volumes
                .iter()
                .find(|volume| volume.kind == SoraLeaseVolumeKindV1::ServiceLeaseVolume)
                .expect("replica one shared service volume");
            let replica_two_shared = replica_two_plan
                .lease_volumes
                .iter()
                .find(|volume| volume.kind == SoraLeaseVolumeKindV1::ServiceLeaseVolume)
                .expect("replica two shared service volume");
            assert!(replica_one_root.local_materialization_dir.ends_with(
                "service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0001/root_disk"
            ));
            assert!(replica_two_root.local_materialization_dir.ends_with(
                "service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0002/root_disk"
            ));
            assert_eq!(
                replica_one_shared.local_materialization_dir,
                replica_two_shared.local_materialization_dir
            );
            assert!(replica_one_shared.local_materialization_dir.ends_with(
                "service_data/web_portal/revisions/2026.02.0/volumes/shared/index_state"
            ));
        } else {
            assert!(summary.is_none());
            assert!(plan.local_replica_slots.is_empty());
            assert!(plan.local_replicas.is_empty());
            assert!(
                !service_dir
                    .join("replicas/replica-0001/runtime_plan.json")
                    .exists()
            );
            assert!(
                !service_dir
                    .join("replicas/replica-0002/runtime_plan.json")
                    .exists()
            );
        }
        Ok(())
    }

    #[test]
    fn reconcile_once_retries_http_service_runtime_state_until_authoritative_state_catches_up()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.container.inrou = Some(iroha_data_model::soracloud::SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: iroha_data_model::soracloud::SoraInrouGuestOsV1::DebianSlim,
            guest_images: BTreeMap::from([
                (
                    SoraInrouGuestIsaV1::X8664,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    SoraInrouGuestIsaV1::Aarch64,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: None,
            ssh_authorized_keys: vec!["ssh-ed25519 test-key irohad-tests".to_owned()],
        });
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        let deployment_state = sample_deployment_state(&bundle);
        let local_peer_id = "12D3KooWHostedRuntimeStateReplica";
        let selected_guest_isa = current_host_inrou_guest_isa();
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
            world
                .soracloud_inrou_service_placements_mut_for_testing()
                .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());

        manager.reconcile_once()?;
        manager.reconcile_once()?;

        let submitted_states = mutation_sink.submitted_inrou_replica_runtime_states();
        if inrou_host_platform_supports_local_materialization() {
            assert_eq!(
                submitted_states.len(),
                2,
                "hosted replica runtime state must be retried while the authoritative chain state is still missing"
            );
            let submitted_state = &submitted_states[0].state;
            assert_eq!(submitted_state.service_name, bundle.service.service_name);
            assert_eq!(
                submitted_state.service_version,
                bundle.service.service_version
            );
            assert_eq!(submitted_state.replica_slot, 1);
            assert_eq!(submitted_state.validator_account_id, *ALICE_ID);
            assert_eq!(submitted_state.peer_id, local_peer_id);
            assert_eq!(
                submitted_state.materialized_bundle_hash,
                bundle.container.bundle_hash
            );
        } else {
            assert!(
                submitted_states.is_empty(),
                "unsupported hosts should not publish authoritative placed-replica Inrou runtime state"
            );
        }
        Ok(())
    }

    #[test]
    fn submit_http_service_lease_usage_update_deduplicates_until_authoritative_state_catches_up()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::Inrou;
        bundle.service.execution_plane =
            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
        bundle.service.state_bindings.clear();
        bundle.service.handlers.clear();
        let deployment_state = sample_deployment_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    deployment_state.clone(),
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let mutation_sink = Arc::new(RecordingRuntimeMutationSink::default());
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_mutation_sink(mutation_sink.clone());

        let view = state.view();
        manager.submit_http_service_lease_usage_update(
            &view,
            bundle.service.service_name.as_ref(),
            &bundle.service.service_version,
            8 * 1024 * 1024,
        );
        manager.submit_http_service_lease_usage_update(
            &view,
            bundle.service.service_name.as_ref(),
            &bundle.service.service_version,
            8 * 1024 * 1024,
        );
        drop(view);

        let submitted_usage = mutation_sink.submitted_service_lease_usage();
        assert_eq!(
            submitted_usage.len(),
            1,
            "identical lease-usage reports should be deduplicated until chain state catches up"
        );
        assert_eq!(submitted_usage[0].accounted_egress_bytes, 8 * 1024 * 1024);

        let mut caught_up_state = test_state()?;
        let mut caught_up_deployment = deployment_state.clone();
        caught_up_deployment
            .service_lease
            .as_mut()
            .expect("lease")
            .accounted_egress_bytes = 8 * 1024 * 1024;
        {
            let world = &mut Arc::get_mut(&mut caught_up_state)
                .expect("unique caught-up test state")
                .world;
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), caught_up_deployment);
        }
        let caught_up_manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&caught_up_state),
        )
        .with_mutation_sink(mutation_sink.clone());
        let caught_up_view = caught_up_state.view();
        caught_up_manager.submit_http_service_lease_usage_update(
            &caught_up_view,
            bundle.service.service_name.as_ref(),
            &bundle.service.service_version,
            8 * 1024 * 1024,
        );
        drop(caught_up_view);

        assert_eq!(
            mutation_sink.submitted_service_lease_usage().len(),
            1,
            "no additional lease-usage report should be submitted once authoritative state matches"
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_cache_buckets_by_authoritative_sequence_and_refreshes_snapshot()
    -> Result<()> {
        let mut state = test_state()?;
        let mut active_bundle = load_deployment_bundle_fixture()?;
        let mut canary_bundle = active_bundle.clone();
        canary_bundle.service.service_version = "2026.03.0".to_string();
        canary_bundle.container.bundle_path = "/bundles/web_portal_canary.to".to_string();

        let active_bundle_bytes = simple_soracloud_contract_artifact(&["entry_active"]);
        let canary_bundle_bytes = simple_soracloud_contract_artifact(&["entry_canary"]);
        active_bundle.container.bundle_hash = Hash::new(&active_bundle_bytes);
        canary_bundle.container.bundle_hash = Hash::new(&canary_bundle_bytes);

        let active_asset_bytes = b"active-asset".to_vec();
        let canary_asset_bytes = b"canary-asset".to_vec();
        active_bundle.service.artifacts[0].artifact_hash = Hash::new(&active_asset_bytes);
        active_bundle.service.artifacts[0].artifact_path = "/public/active.html".to_string();
        canary_bundle.service.artifacts[0].artifact_hash = Hash::new(&canary_asset_bytes);
        canary_bundle.service.artifacts[0].artifact_path = "/public/canary.html".to_string();

        let mut deployment = sample_deployment_state(&active_bundle);
        deployment.revision_count = 2;
        deployment.active_rollout = Some(SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "rollout-2026-03".to_string(),
            baseline_version: Some(active_bundle.service.service_version.clone()),
            candidate_version: canary_bundle.service.service_version.clone(),
            canary_percent: 20,
            traffic_percent: 20,
            stage: SoraRolloutStageV1::Canary,
            health_failures: 0,
            max_health_failures: 3,
            health_window_secs: 60,
            created_sequence: 17,
            updated_sequence: 29,
        });
        deployment.last_rollout = deployment.active_rollout.clone();

        let mut runtime = sample_runtime_state(&active_bundle);
        runtime.rollout_handle = Some("rollout-2026-03".to_string());

        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    active_bundle.service.service_name.to_string(),
                    active_bundle.service.service_version.clone(),
                ),
                active_bundle.clone(),
            );
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    canary_bundle.service.service_name.to_string(),
                    canary_bundle.service.service_version.clone(),
                ),
                canary_bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(active_bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(active_bundle.service.service_name.clone(), runtime);
        }

        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(active_bundle.container.bundle_hash)),
            &active_bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(canary_bundle.container.bundle_hash)),
            &canary_bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(
                active_bundle.service.artifacts[0].artifact_hash,
            )),
            &active_asset_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(
                canary_bundle.service.artifacts[0].artifact_hash,
            )),
            &canary_asset_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.cache_budgets.bundle_bytes = std::num::NonZeroU64::new(
            u64::try_from(active_bundle_bytes.len().max(canary_bundle_bytes.len()))
                .expect("bundle size fits in u64"),
        )
        .expect("nonzero bundle budget");
        config.cache_budgets.static_asset_bytes = std::num::NonZeroU64::new(
            u64::try_from(active_asset_bytes.len().max(canary_asset_bytes.len()))
                .expect("asset size fits in u64"),
        )
        .expect("nonzero asset budget");

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let active_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("active service plan present");
        let canary_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.03.0"))
            .expect("canary service plan present");
        let active_asset_plan = active_plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == SoraArtifactKindV1::StaticAsset)
            .expect("active static asset plan");
        let canary_asset_plan = canary_plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == SoraArtifactKindV1::StaticAsset)
            .expect("canary static asset plan");

        assert!(!active_plan.bundle_available_locally);
        assert!(canary_plan.bundle_available_locally);
        assert!(!active_asset_plan.available_locally);
        assert!(canary_asset_plan.available_locally);
        assert!(
            !artifacts_root
                .join(hash_cache_name(active_bundle.container.bundle_hash))
                .exists()
        );
        assert!(
            artifacts_root
                .join(hash_cache_name(canary_bundle.container.bundle_hash))
                .exists()
        );
        assert!(
            !artifacts_root
                .join(hash_cache_name(
                    active_bundle.service.artifacts[0].artifact_hash
                ))
                .exists()
        );
        assert!(
            artifacts_root
                .join(hash_cache_name(
                    canary_bundle.service.artifacts[0].artifact_hash
                ))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_tied_cache_candidates_by_stable_key() -> Result<()> {
        let state = test_state()?;
        let temp_dir = tempfile::tempdir()?;
        let journals_root = temp_dir.path().join("journals");
        fs::create_dir_all(&journals_root)?;

        let first_hash = Hash::new(b"journal-alpha");
        let second_hash = Hash::new(b"journal-omega");
        let payload = b"journal-entry".to_vec();
        let first_name = hash_cache_name(first_hash);
        let second_name = hash_cache_name(second_hash);
        let first_path = journals_root.join(&first_name);
        let second_path = journals_root.join(&second_name);
        fs::write(&first_path, &payload)?;
        fs::write(&second_path, &payload)?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.cache_budgets.journal_bytes =
            std::num::NonZeroU64::new(u64::try_from(payload.len()).expect("payload size fits"))
                .expect("nonzero journal budget");

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let (removed, retained) = if first_name <= second_name {
            (first_path, second_path)
        } else {
            (second_path, first_path)
        };
        assert!(!removed.exists());
        assert!(retained.exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_hydrates_missing_artifacts_from_committed_sorafs_store() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "hydrated-from-sorafs");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let sorafs_node = test_sorafs_node(&temp_dir);
        let mut committed_manifests = vec![ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?];
        for payload in &artifact_payloads {
            committed_manifests.push(ingest_sorafs_payload(&sorafs_node, payload)?);
        }
        approve_sorafs_manifests(&state, &committed_manifests)?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("hydrated service plan");
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| artifact.available_locally)
        );
        assert_eq!(
            fs::read(
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(bundle.container.bundle_hash))
            )?,
            bundle_bytes
        );
        for (artifact, payload) in bundle.service.artifacts.iter().zip(artifact_payloads) {
            assert_eq!(
                fs::read(
                    temp_dir
                        .path()
                        .join("artifacts")
                        .join(hash_cache_name(artifact.artifact_hash))
                )?,
                payload
            );
        }
        Ok(())
    }

    #[test]
    fn reconcile_once_hydrates_missing_artifacts_from_committed_remote_sorafs_provider()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.service.artifacts.truncate(1);
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "remote-provider");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let provider_id = [0x11; 32];
        let remote_payloads = std::iter::once(bundle_bytes.clone())
            .chain(artifact_payloads.iter().cloned())
            .collect::<Vec<_>>();
        let remote_fixtures = remote_payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                build_remote_manifest_fixture(
                    payload,
                    provider_id,
                    u8::try_from(index + 1).expect("fixture index fits in u8"),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        approve_remote_hydration_sources(&state, &remote_fixtures)?;

        let server = spawn_remote_hydration_fixture(&remote_fixtures)?;
        let provider_cache = test_provider_cache(&server.base_url, provider_id)?;

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_provider_cache(provider_cache);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("hydrated service plan");
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| artifact.available_locally)
        );
        assert_eq!(
            fs::read(
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(bundle.container.bundle_hash))
            )?,
            bundle_bytes
        );
        for (artifact, payload) in bundle.service.artifacts.iter().zip(artifact_payloads) {
            assert_eq!(
                fs::read(
                    temp_dir
                        .path()
                        .join("artifacts")
                        .join(hash_cache_name(artifact.artifact_hash))
                )?,
                payload
            );
        }
        Ok(())
    }

    #[test]
    fn reconcile_once_skips_remote_sorafs_payloads_that_do_not_match_expected_hash() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.service.artifacts.clear();
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let provider_id = [0x11; 32];
        let remote_fixture = build_remote_manifest_fixture(b"wrong-remote-bundle", provider_id, 1)?;
        approve_remote_hydration_sources(&state, std::slice::from_ref(&remote_fixture))?;

        let server = spawn_remote_hydration_fixture(std::slice::from_ref(&remote_fixture))?;
        let provider_cache = test_provider_cache(&server.base_url, provider_id)?;

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_provider_cache(provider_cache);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("service plan");
        assert!(!plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Hydrating);
        assert!(
            !temp_dir
                .path()
                .join("artifacts")
                .join(hash_cache_name(bundle.container.bundle_hash))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_hydrates_hash_matched_local_sorafs_artifacts_without_pin_registry()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "uncommitted-sorafs");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let sorafs_node = test_sorafs_node(&temp_dir);
        let _bundle_manifest = ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?;
        for payload in &artifact_payloads {
            let _stored = ingest_sorafs_payload(&sorafs_node, payload)?;
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("service plan");
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| artifact.available_locally)
        );
        assert_eq!(
            fs::read(
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(bundle.container.bundle_hash))
            )?,
            bundle_bytes
        );
        for (artifact, payload) in bundle.service.artifacts.iter().zip(artifact_payloads) {
            assert_eq!(
                fs::read(
                    temp_dir
                        .path()
                        .join("artifacts")
                        .join(hash_cache_name(artifact.artifact_hash))
                )?,
                payload
            );
        }
        Ok(())
    }

    #[test]
    fn restore_persisted_snapshot_rehydrates_missing_artifacts_from_committed_sorafs_store()
    -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "restart-rehydrate");
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }

        let temp_dir = tempfile::tempdir()?;
        let sorafs_node = test_sorafs_node(&temp_dir);
        let mut committed_manifests = vec![ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?];
        for payload in &artifact_payloads {
            committed_manifests.push(ingest_sorafs_payload(&sorafs_node, payload)?);
        }
        approve_sorafs_manifests(&state, &committed_manifests)?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node.clone());
        manager.reconcile_once()?;

        let bundle_cache_path = temp_dir
            .path()
            .join("artifacts")
            .join(hash_cache_name(bundle.container.bundle_hash));
        let artifact_cache_paths = bundle
            .service
            .artifacts
            .iter()
            .map(|artifact| {
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(artifact.artifact_hash))
            })
            .collect::<Vec<_>>();
        fs::remove_file(&bundle_cache_path)?;
        for path in &artifact_cache_paths {
            fs::remove_file(path)?;
        }

        let restarted_manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        assert!(restarted_manager.restore_persisted_snapshot()?);
        restarted_manager.reconcile_once()?;

        let snapshot = restarted_manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("restarted service plan");
        assert!(plan.bundle_available_locally);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| artifact.available_locally)
        );
        assert_eq!(fs::read(bundle_cache_path)?, bundle_bytes);
        for (path, payload) in artifact_cache_paths.iter().zip(artifact_payloads) {
            assert_eq!(fs::read(path)?, payload);
        }
        Ok(())
    }

    #[test]
    fn restore_persisted_snapshot_preserves_last_snapshot_if_reconcile_fails() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    sample_deployment_state(&bundle),
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let expected_snapshot = SoracloudRuntimeSnapshot {
            schema_version: SoracloudRuntimeSnapshot::default().schema_version,
            observed_height: 77,
            observed_block_hash: Some(Hash::prehashed([0x55; Hash::LENGTH]).to_string()),
            local_peer_id: None,
            services: BTreeMap::from([(
                "restored_service".to_string(),
                BTreeMap::from([(
                    "2026.03.0".to_string(),
                    SoracloudRuntimeServicePlan {
                        service_name: "restored_service".to_string(),
                        service_version: "2026.03.0".to_string(),
                        role: SoracloudRuntimeRevisionRole::Active,
                        traffic_percent: 100,
                        runtime: SoraContainerRuntimeV1::Ivm,
                        execution_plane:
                            iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
                        bundle_hash: Hash::prehashed([0x33; Hash::LENGTH]).to_string(),
                        bundle_path: "sorafs://restored.bundle".to_string(),
                        entrypoint: "main".to_string(),
                        inrou: None,
                        bundle_cache_path: temp_dir
                            .path()
                            .join("artifacts/restored_bundle")
                            .display()
                            .to_string(),
                        bundle_available_locally: true,
                        process_generation: Some(9),
                        desired_replica_count: 1,
                        local_replica_slots: Vec::new(),
                        local_replicas: Vec::new(),
                        health_status: SoraServiceHealthStatusV1::Healthy,
                        load_factor_bps: 250,
                        reported_pending_mailbox_messages: 2,
                        authoritative_pending_mailbox_messages: 2,
                        rollout_handle: None,
                        config_generation: 0,
                        secret_generation: 0,
                        quota_class: None,
                        service_lease_status: None,
                        lease_expires_sequence: None,
                        remaining_runtime_balance_nanos: None,
                        config_entry_count: 0,
                        secret_entry_count: 0,
                        config_exports: Vec::new(),
                        supports_host_read_config: true,
                        supports_host_read_secret_envelope: true,
                        supports_private_secret_payload_reads: false,
                        materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0")
                            .display()
                            .to_string(),
                        config_materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0/configs")
                            .display()
                            .to_string(),
                        effective_env: BTreeMap::new(),
                        effective_env_materialization_path: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0/effective_env.json")
                            .display()
                            .to_string(),
                        config_exports_materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0/config_exports")
                            .display()
                            .to_string(),
                        secret_envelopes_materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0/secret_envelopes")
                            .display()
                            .to_string(),
                        secret_payload_materialization_dir: temp_dir
                            .path()
                            .join("secrets/restored_service/2026.03.0")
                            .display()
                            .to_string(),
                        lease_volumes: Vec::new(),
                        mailboxes: Vec::new(),
                        artifacts: Vec::new(),
                    },
                )]),
            )]),
            apartments: BTreeMap::new(),
            hf_sources: BTreeMap::new(),
        };
        write_json_atomic(
            temp_dir.path().join("runtime_snapshot.json").as_path(),
            &expected_snapshot,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        assert!(manager.restore_persisted_snapshot()?);
        let error = manager
            .reconcile_once()
            .expect_err("reconcile should fail without the admitted revision bundle");
        assert!(
            error
                .to_string()
                .contains("references missing admitted revision"),
            "unexpected reconcile error: {error:?}"
        );
        assert_eq!(manager.snapshot.read().clone(), expected_snapshot);
        Ok(())
    }

    #[test]
    fn execute_local_read_serves_hydrated_asset_with_committed_binding() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        let asset_bytes = b"<html><body>portal</body></html>".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle.service.artifacts[0].artifact_hash = Hash::new(&asset_bytes);
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.service.artifacts[0].artifact_hash)),
            &asset_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "assets".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset,
                request_method: "GET".to_owned(),
                request_path: "/app/assets".to_owned(),
                handler_path: "/".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"asset-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(response.response_bytes, asset_bytes);
        assert_eq!(
            response.content_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::StateCommitment
        );
        assert!(response.runtime_receipt.is_none());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].artifact_hash,
            Some(bundle.service.artifacts[0].artifact_hash)
        );
        Ok(())
    }

    #[test]
    fn execute_local_read_rejects_internal_service_route() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        let asset_bytes = b"<html><body>portal</body></html>".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle.service.artifacts[0].artifact_hash = Hash::new(&asset_bytes);
        bundle
            .service
            .route
            .as_mut()
            .expect("fixture route")
            .visibility = iroha_data_model::soracloud::SoraRouteVisibilityV1::Internal;
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.service.artifacts[0].artifact_hash)),
            &asset_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "assets".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset,
                request_method: "GET".to_owned(),
                request_path: "/app/assets".to_owned(),
                handler_path: "/".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"asset-request"),
            })
            .expect_err("internal routes must not execute through public local-read");

        assert_eq!(
            error.kind,
            SoracloudRuntimeExecutionErrorKind::InvalidRequest
        );
        assert!(error.message.contains("local-read route is not public"));
        Ok(())
    }

    #[test]
    fn execute_local_read_runs_query_handler_from_admitted_ivm_bundle() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let query_entrypoint = bundle_handler(&bundle, "query").entrypoint;
        let query_body = make_pointer_tlv(PointerType::Json, br#"{"ok":true}"#);
        let bundle_bytes = simple_soracloud_contract_artifact(&[query_entrypoint.as_str()]);
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
            world
                .soracloud_service_state_entries_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        "session_store".to_owned(),
                        "/state/session/alice".to_owned(),
                    ),
                    SoraServiceStateEntryV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                        service_name: bundle.service.service_name.clone(),
                        service_version: bundle.service.service_version.clone(),
                        binding_name: "session_store".parse().expect("valid binding"),
                        state_key: "/state/session/alice".to_owned(),
                        encryption:
                            iroha_data_model::soracloud::SoraStateEncryptionV1::ClientCiphertext,
                        payload: b"alice-session".to_vec(),
                        payload_bytes: std::num::NonZeroU64::new(13).expect("nonzero"),
                        payload_commitment: Hash::new(b"alice-session"),
                        last_update_sequence: 4,
                        governance_tx_hash: Hash::new(b"gov-session"),
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "query".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "GET".to_owned(),
                request_path: "/app/query".to_owned(),
                handler_path: "/".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: query_body,
                request_commitment: Hash::new(b"query-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(response.response_bytes, br#"{"ok":true}"#);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::AuditReceipt
        );
        assert!(response.runtime_receipt.is_some());
        assert!(response.bindings.is_empty());
        Ok(())
    }

    #[test]
    fn execute_local_read_passes_query_metadata_in_r11() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let query_entrypoint = bundle_handler(&bundle, "query").entrypoint;
        let copy_metadata_to_r10 =
            ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 10, 11, 0);
        let bundle_bytes = soracloud_contract_artifact_with_words(
            &[query_entrypoint.as_str()],
            &[copy_metadata_to_r10, ivm::encoding::wide::encode_halt()],
        );
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "query".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "POST".to_owned(),
                request_path: "/app/query/profile".to_owned(),
                handler_path: "/profile".to_owned(),
                request_query: Some("verbose=1".to_owned()),
                request_headers: BTreeMap::from([(
                    "accept".to_owned(),
                    "application/json".to_owned(),
                )]),
                request_body: br#"{"hello":"world"}"#.to_vec(),
                request_commitment: Hash::new(b"query-request-r11"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        let decoded: norito::json::Value = norito::json::from_slice(&response.response_bytes)?;
        assert_eq!(
            decoded
                .get("request_path")
                .and_then(norito::json::Value::as_str),
            Some("/app/query/profile")
        );
        assert_eq!(
            decoded
                .get("handler_path")
                .and_then(norito::json::Value::as_str),
            Some("/profile")
        );
        assert_eq!(
            decoded
                .get("request_query")
                .and_then(norito::json::Value::as_str),
            Some("verbose=1")
        );
        assert_eq!(
            decoded
                .get("request_body_is_tlv")
                .and_then(norito::json::Value::as_bool),
            Some(false)
        );
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_runs_update_handler_from_admitted_ivm_bundle() -> Result<()> {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes =
            simple_soracloud_contract_artifact(&["apply_update", "apply_private_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &artifact_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"hello-update".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request.clone())
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        let runtime_state = result.runtime_state.expect("runtime state");
        assert_eq!(
            runtime_state.health_status,
            SoraServiceHealthStatusV1::Healthy
        );
        assert_eq!(runtime_state.pending_mailbox_message_count, 0);
        assert_eq!(
            result.runtime_receipt.handler_class,
            SoraServiceHandlerClassV1::Update
        );
        assert_eq!(
            result.runtime_receipt.request_commitment,
            request.mailbox_message.payload_commitment
        );
        assert_eq!(
            result.runtime_receipt.mailbox_message_id,
            Some(request.mailbox_message.message_id)
        );
        assert_ne!(
            result.runtime_receipt.result_commitment,
            Hash::prehashed([0; Hash::LENGTH])
        );
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_runs_private_update_handler_from_admitted_ivm_bundle() -> Result<()>
    {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes =
            simple_soracloud_contract_artifact(&["apply_update", "apply_private_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &artifact_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"secret-input".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request)
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        assert_eq!(
            result.runtime_receipt.handler_class,
            SoraServiceHandlerClassV1::PrivateUpdate
        );
        assert_eq!(
            result.runtime_state.expect("runtime state").health_status,
            SoraServiceHealthStatusV1::Healthy
        );
        Ok(())
    }

    #[test]
    fn ivm_host_private_runtime_reads_secret_and_credential_material() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Allowlist(Vec::new());
        let temp_dir = tempfile::tempdir()?;
        let service_root = temp_dir
            .path()
            .join("secrets")
            .join(bundle.service.service_name.to_string())
            .join(bundle.service.service_version.clone())
            .join("db");
        fs::create_dir_all(&service_root)?;
        fs::write(service_root.join("password"), b"super-secret")?;
        let credential_root = temp_dir
            .path()
            .join("credentials")
            .join(bundle.service.service_name.to_string())
            .join(bundle.service.service_version.clone())
            .join("vault");
        fs::create_dir_all(&credential_root)?;
        fs::write(credential_root.join("token"), br#"{"token":"abc"}"#)?;

        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        let private_host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );
        private_host.require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET)?;
        private_host.require_private_runtime(SYSCALL_SORACLOUD_READ_CREDENTIAL)?;
        assert_eq!(
            private_host.read_material("secrets", "db/password")?,
            Some(b"super-secret".to_vec())
        );
        assert_eq!(
            private_host.read_material("credentials", "vault/token")?,
            Some(br#"{"token":"abc"}"#.to_vec())
        );

        let public_request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"public".to_vec()),
        );
        let public_host = SoracloudIvmHost::new(
            public_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );
        let secret_error = public_host
            .require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET)
            .expect_err("public handlers cannot read secrets");
        assert_eq!(secret_error.metered_gas(), Some(ivm::gas::G_SORACLOUD));
        assert!(matches!(
            secret_error.as_unmetered(),
            VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_SECRET
            }
        ));
        let credential_error = public_host
            .require_private_runtime(SYSCALL_SORACLOUD_READ_CREDENTIAL)
            .expect_err("public handlers cannot read credentials");
        assert_eq!(credential_error.metered_gas(), Some(ivm::gas::G_SORACLOUD));
        assert!(matches!(
            credential_error.as_unmetered(),
            VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_CREDENTIAL
            }
        ));
        Ok(())
    }

    #[test]
    fn ivm_host_private_runtime_prefers_authoritative_service_secret_entry() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Allowlist(Vec::new());
        let temp_dir = tempfile::tempdir()?;
        let service_root = temp_dir
            .path()
            .join("secrets")
            .join(bundle.service.service_name.to_string())
            .join(bundle.service.service_version.clone())
            .join("db");
        fs::create_dir_all(&service_root)?;
        fs::write(service_root.join("password"), b"filesystem-secret")?;

        let mut private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        private_request.deployment.service_secrets.insert(
            "db/password".to_string(),
            SoraServiceSecretEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                secret_name: "db/password".to_string(),
                envelope: SecretEnvelopeV1 {
                    schema_version: SECRET_ENVELOPE_VERSION_V1,
                    encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
                    key_id: "kms/runtime/test".to_string(),
                    key_version: std::num::NonZeroU32::new(1).expect("non-zero"),
                    nonce: vec![1, 2, 3, 4],
                    ciphertext: b"authoritative-secret".to_vec(),
                    commitment: Hash::new(b"authoritative-secret"),
                    aad_digest: None,
                },
                last_update_sequence: 17,
            },
        );
        let private_host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );
        private_host.require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET)?;
        assert_eq!(
            private_host.read_material("secrets", "db/password")?,
            Some(b"authoritative-secret".to_vec())
        );
        Ok(())
    }

    #[test]
    fn ivm_host_public_runtime_reads_authoritative_service_config_entry() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Allowlist(Vec::new());
        let temp_dir = tempfile::tempdir()?;

        let value_json = Json::from(norito::json!({
            "featureFlag": true,
            "theme": "dawn"
        }));
        let expected_payload = value_json.get().as_bytes().to_vec();
        let mut public_request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"public".to_vec()),
        );
        public_request.deployment.service_configs.insert(
            "ui/settings".to_string(),
            SoraServiceConfigEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
                config_name: "ui/settings".to_string(),
                value_hash: Hash::new(&expected_payload),
                value_json,
                last_update_sequence: 22,
            },
        );
        let public_host = SoracloudIvmHost::new(
            public_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );

        let response = public_host.read_service_config("ui/settings")?;
        assert!(response.found);
        assert_eq!(response.payload_bytes, expected_payload);
        Ok(())
    }

    #[test]
    fn local_read_public_inputs_encode_trigger_event_json_for_ivm_helpers() {
        let body_tlv = make_pointer_tlv(PointerType::Blob, br#"{"hello":"world"}"#);
        let metadata_value = norito::json::Value::Object(norito::json::Map::from([
            (
                "request_path".to_owned(),
                norito::json::Value::from("/api/auth/me"),
            ),
            (
                "request_method".to_owned(),
                norito::json::Value::from("GET"),
            ),
        ]));
        let metadata_json =
            Json::from_norito_value_ref(&metadata_value).expect("metadata JSON value");
        let metadata_bytes = norito::to_bytes(&metadata_json).expect("metadata norito bytes");
        let metadata_tlv = make_pointer_tlv(PointerType::Json, &metadata_bytes);

        let inputs = local_read_public_inputs(&body_tlv, &metadata_tlv, 42).expect("public inputs");
        let trigger_event_tlv = inputs
            .get(&public_input_name("trigger_event_json").expect("input name"))
            .expect("trigger event public input");
        let trigger_event_tlv =
            ivm::pointer_abi::validate_tlv_bytes(trigger_event_tlv).expect("valid JSON TLV");
        assert_eq!(trigger_event_tlv.type_id, PointerType::Json);

        let trigger_json: Json =
            norito::decode_from_bytes(trigger_event_tlv.payload).expect("JSON wrapper");
        let trigger_value: norito::json::Value = trigger_json
            .try_into_any_norito()
            .expect("trigger event JSON value");
        assert_eq!(
            trigger_value
                .get("_request_body")
                .and_then(norito::json::Value::as_str),
            Some("7b2268656c6c6f223a22776f726c64227d")
        );
        assert_eq!(
            trigger_value
                .get("_request_meta")
                .and_then(|metadata| metadata.get("request_path"))
                .and_then(norito::json::Value::as_str),
            Some("/api/auth/me")
        );
        assert_eq!(
            trigger_value
                .get("observed_height")
                .and_then(norito::json::Value::as_u64),
            Some(42)
        );
    }

    #[test]
    fn ivm_host_public_runtime_reads_authoritative_service_secret_envelope() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Allowlist(Vec::new());
        let temp_dir = tempfile::tempdir()?;

        let envelope = SecretEnvelopeV1 {
            schema_version: SECRET_ENVELOPE_VERSION_V1,
            encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
            key_id: "kms/runtime/test".to_string(),
            key_version: std::num::NonZeroU32::new(1).expect("non-zero"),
            nonce: vec![1, 2, 3, 4],
            ciphertext: b"enveloped-secret".to_vec(),
            commitment: Hash::new(b"enveloped-secret"),
            aad_digest: None,
        };
        let mut public_request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"public".to_vec()),
        );
        public_request.deployment.service_secrets.insert(
            "db/password".to_string(),
            SoraServiceSecretEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
                secret_name: "db/password".to_string(),
                envelope: envelope.clone(),
                last_update_sequence: 23,
            },
        );
        let public_host = SoracloudIvmHost::new(
            public_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );

        let response = public_host.read_service_secret_envelope("db/password");
        assert_eq!(response.envelope, Some(envelope));
        Ok(())
    }

    #[test]
    fn ivm_host_query_runtime_tracks_committed_state_read_bindings() -> Result<()> {
        let bundle = load_deployment_bundle_fixture()?;
        let temp_dir = tempfile::tempdir()?;
        let query_request = sample_ordered_mailbox_request(
            &bundle,
            "query",
            sample_mailbox_message(&bundle, "query", b"public-query".to_vec()),
        );
        let entry = SoraServiceStateEntryV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            service_version: bundle.service.service_version.clone(),
            binding_name: "session_store".parse().expect("valid binding"),
            state_key: "/state/session/alice".to_owned(),
            encryption: iroha_data_model::soracloud::SoraStateEncryptionV1::ClientCiphertext,
            payload: b"alice-session".to_vec(),
            payload_bytes: std::num::NonZeroU64::new(13).expect("non-zero"),
            payload_commitment: Hash::new(b"alice-session"),
            last_update_sequence: 4,
            governance_tx_hash: Hash::new(b"gov-session"),
            source_action: SoraServiceLifecycleActionV1::StateMutation,
        };
        let mut committed_entries = BTreeMap::new();
        committed_entries.insert(
            (
                "session_store".to_owned(),
                "/state/session/alice".to_owned(),
            ),
            entry.clone(),
        );
        let mut host = SoracloudIvmHost::new(
            query_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            committed_entries,
        );
        let request_envelope = SoracloudHostRequestEnvelopeV1 {
            schema_version: iroha_data_model::soracloud::SORACLOUD_HOST_REQUEST_VERSION_V1,
            operation: SoracloudHostOperationV1::ReadCommittedState,
            payload: SoracloudHostRequestPayloadV1::ReadCommittedState(
                iroha_data_model::soracloud::SoracloudReadCommittedStateRequestV1 {
                    binding_name: "session_store".parse().expect("valid binding"),
                    state_key: "/state/session/alice".to_owned(),
                },
            ),
        };
        let request_payload = norito::to_bytes(&request_envelope)?;
        let request_tlv = make_pointer_tlv(PointerType::SoracloudRequest, &request_payload);
        let mut vm = IVM::new(u64::MAX);
        let request_ptr = vm.alloc_input_tlv(&request_tlv)?;
        vm.set_register(10, request_ptr);

        host.syscall(SYSCALL_SORACLOUD_READ_COMMITTED_STATE, &mut vm)?;

        let bindings = host.local_read_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0], state_entry_binding(&entry));
        Ok(())
    }

    #[test]
    fn inrou_tap_firewall_plan_supports_open_and_isolated() -> Result<()> {
        assert_eq!(
            inrou_tap_firewall_plan(&SoraNetworkPolicyV1::Open)?,
            InrouTapFirewallPlan::Open
        );
        assert_eq!(
            inrou_tap_firewall_plan(&SoraNetworkPolicyV1::Isolated)?,
            InrouTapFirewallPlan::Isolated
        );
        Ok(())
    }

    #[test]
    fn inrou_tap_firewall_plan_resolves_allowlist_ipv4_endpoints() -> Result<()> {
        let plan = inrou_tap_firewall_plan(&SoraNetworkPolicyV1::Allowlist(vec![
            SoraNetworkAllowlistEntryV1::new("127.0.0.1", [80, 443]),
        ]))?;
        assert_eq!(
            plan,
            InrouTapFirewallPlan::Allowlist(vec![
                InrouTapResolvedAllowlistEndpoint {
                    host: "127.0.0.1".to_owned(),
                    address: "127.0.0.1".parse().expect("valid IPv4"),
                    port: 80,
                },
                InrouTapResolvedAllowlistEndpoint {
                    host: "127.0.0.1".to_owned(),
                    address: "127.0.0.1".parse().expect("valid IPv4"),
                    port: 443,
                },
            ])
        );
        Ok(())
    }

    #[test]
    fn resolve_inrou_allowlist_endpoints_deduplicates_ipv4_entries() -> Result<()> {
        let endpoints = resolve_inrou_allowlist_endpoints(&[
            SoraNetworkAllowlistEntryV1::new(" 127.0.0.1 ", [443, 443]),
            SoraNetworkAllowlistEntryV1::new("127.0.0.1", [443]),
        ])?;

        assert_eq!(
            endpoints,
            vec![InrouTapResolvedAllowlistEndpoint {
                host: "127.0.0.1".to_owned(),
                address: "127.0.0.1".parse().expect("valid IPv4"),
                port: 443,
            }]
        );
        Ok(())
    }

    #[test]
    fn resolve_inrou_allowlist_endpoints_rejects_ipv6_only_literals() {
        let error =
            resolve_inrou_allowlist_endpoints(&[SoraNetworkAllowlistEntryV1::new("::1", [443])])
                .expect_err("IPv6-only allowlist entries should fail closed");
        let message = error.to_string();
        assert!(message.contains("no IPv4 endpoints"));
        assert!(message.contains("::1"));
    }

    #[test]
    fn resolve_inrou_allowlist_endpoints_rejects_empty_port_lists() {
        let error = resolve_inrou_allowlist_endpoints(&[SoraNetworkAllowlistEntryV1::new(
            "127.0.0.1",
            Vec::<u16>::new(),
        )])
        .expect_err("allowlist entries without ports should fail closed");
        let message = error.to_string();
        assert!(message.contains("no IPv4 endpoints"));
        assert!(message.contains("127.0.0.1"));
        assert!(message.contains("[]"));
    }

    #[test]
    fn inrou_tap_firewall_plan_rejects_allowlist_without_ipv4_endpoints() {
        let error = inrou_tap_firewall_plan(&SoraNetworkPolicyV1::Allowlist(vec![
            SoraNetworkAllowlistEntryV1::new("::1", [443]),
        ]))
        .expect_err("allowlist should fail closed when no IPv4 endpoint is enforceable");
        let message = error.to_string();
        assert!(message.contains("no IPv4 endpoints"));
        assert!(message.contains("::1"));
    }

    #[test]
    fn planned_inrou_tap_firewall_rules_place_allowlist_accepts_above_default_drop() {
        let rules = planned_inrou_tap_firewall_rules(
            "irtest0",
            "172.31.10.2/32",
            &InrouTapFirewallPlan::Allowlist(vec![
                InrouTapResolvedAllowlistEndpoint {
                    host: "ton.example".to_owned(),
                    address: "127.0.0.1".parse().expect("valid IPv4"),
                    port: 443,
                },
                InrouTapResolvedAllowlistEndpoint {
                    host: "ton.example".to_owned(),
                    address: "127.0.0.2".parse().expect("valid IPv4"),
                    port: 8443,
                },
            ]),
        );
        assert_eq!(rules.len(), 8);
        assert_eq!(
            rules[0].context,
            "install Inrou shared-storage ingress rule"
        );
        assert_eq!(rules[1].context, "install Inrou egress masquerade rule");
        assert_eq!(
            rules[2].context,
            "install Inrou established host-input rule"
        );
        assert_eq!(
            rules[3].context,
            "install Inrou host-input default-drop rule"
        );
        assert_eq!(rules[4].context, "install Inrou return-traffic rule");
        assert_eq!(
            rules[5].context,
            "install Inrou allowlist default-drop rule"
        );
        assert_eq!(rules[6].context, "install Inrou allowlist forward rule");
        assert_eq!(rules[7].context, "install Inrou allowlist forward rule");
        assert_eq!(rules[6].args[8], "127.0.0.1");
        assert_eq!(rules[6].args[10], "443");
        assert_eq!(rules[7].args[8], "127.0.0.2");
        assert_eq!(rules[7].args[10], "8443");
    }

    #[test]
    fn planned_inrou_tap_firewall_rules_allowlist_empty_keeps_default_drop() {
        let rules = planned_inrou_tap_firewall_rules(
            "irtest0",
            "172.31.10.2/32",
            &InrouTapFirewallPlan::Allowlist(Vec::new()),
        );

        assert_eq!(rules.len(), 6);
        assert_eq!(
            rules[5].context,
            "install Inrou allowlist default-drop rule"
        );
        assert_eq!(rules[5].args[0], "-I");
        assert_eq!(rules[5].args[1], "FORWARD");
        assert_eq!(rules[5].args[4], "irtest0");
        assert_eq!(rules[5].args[5], "-j");
        assert_eq!(rules[5].args[6], "DROP");
    }

    #[test]
    fn planned_inrou_tap_firewall_rules_keep_isolated_policy_private() {
        let rules = planned_inrou_tap_firewall_rules(
            "irtest0",
            "172.31.10.2/32",
            &InrouTapFirewallPlan::Isolated,
        );
        assert_eq!(rules.len(), 4);
        assert_eq!(
            rules[0].context,
            "install Inrou shared-storage ingress rule"
        );
        assert_eq!(rules[0].args[1], "INPUT");
        assert_eq!(rules[0].args[8], "2049");
        assert_eq!(
            rules[1].context,
            "install Inrou established host-input rule"
        );
        assert_eq!(rules[1].args[1], "INPUT");
        assert_eq!(
            rules[2].context,
            "install Inrou host-input default-drop rule"
        );
        assert_eq!(rules[2].args[1], "INPUT");
        assert_eq!(rules[3].context, "install Inrou isolated forward-drop rule");
        assert_eq!(rules[3].args[1], "FORWARD");
        assert_eq!(rules[3].args[6], "DROP");
    }

    #[test]
    fn planned_inrou_tap_firewall_rules_open_policy_keeps_return_path_before_forward_out() {
        let rules = planned_inrou_tap_firewall_rules(
            "irtest0",
            "172.31.10.2/32",
            &InrouTapFirewallPlan::Open,
        );

        assert_eq!(rules.len(), 6);
        assert_eq!(
            rules[0].context,
            "install Inrou shared-storage ingress rule"
        );
        assert_eq!(rules[1].context, "install Inrou egress masquerade rule");
        assert_eq!(
            rules[2].context,
            "install Inrou established host-input rule"
        );
        assert_eq!(
            rules[3].context,
            "install Inrou host-input default-drop rule"
        );
        assert_eq!(rules[4].context, "install Inrou return-traffic rule");
        assert_eq!(rules[5].context, "install Inrou forward-out rule");
        assert_eq!(rules[4].args[1], "FORWARD");
        assert_eq!(rules[4].args[3], "-o");
        assert_eq!(rules[4].args[4], "irtest0");
        assert_eq!(rules[5].args[1], "FORWARD");
        assert_eq!(rules[5].args[3], "-i");
        assert_eq!(rules[5].args[4], "irtest0");
        assert_eq!(rules[5].args[6], "ACCEPT");
    }

    #[test]
    fn inrou_tap_delete_rule_args_strip_insert_position_and_flip_mode() {
        let delete_args = inrou_tap_delete_rule_args(&[
            "-t".to_owned(),
            "nat".to_owned(),
            "-I".to_owned(),
            "POSTROUTING".to_owned(),
            "1".to_owned(),
            "-s".to_owned(),
            "172.31.10.2/32".to_owned(),
            "-j".to_owned(),
            "MASQUERADE".to_owned(),
        ]);
        assert_eq!(
            delete_args,
            vec![
                "-t".to_owned(),
                "nat".to_owned(),
                "-D".to_owned(),
                "POSTROUTING".to_owned(),
                "-s".to_owned(),
                "172.31.10.2/32".to_owned(),
                "-j".to_owned(),
                "MASQUERADE".to_owned(),
            ]
        );
    }

    #[test]
    fn build_portable_vm_network_plan_projects_host_forwarding_and_restricts_allowlists()
    -> Result<()> {
        let open = build_portable_vm_network_plan(8080, &InrouTapFirewallPlan::Open)?;
        assert!(open.netdev.contains("hostfwd=tcp:127.0.0.1:"));
        assert!(!open.netdev.contains("restrict=on"));
        assert!(open.listen_base_url.starts_with("http://127.0.0.1:"));

        let isolated = build_portable_vm_network_plan(8080, &InrouTapFirewallPlan::Isolated)?;
        assert!(isolated.netdev.contains("restrict=on"));
        assert!(isolated.allowlist_hosts.is_empty());

        let allowlist = build_portable_vm_network_plan(
            8080,
            &InrouTapFirewallPlan::Allowlist(vec![InrouTapResolvedAllowlistEndpoint {
                host: "ton.example".to_owned(),
                address: "127.0.0.1".parse().expect("IPv4"),
                port: 443,
            }]),
        )?;
        assert!(allowlist.netdev.contains("restrict=on"));
        assert!(
            allowlist
                .netdev
                .contains("guestfwd=tcp:127.0.0.1:443-tcp:127.0.0.1:443")
        );
        assert_eq!(
            allowlist.allowlist_hosts,
            vec![("ton.example".to_owned(), "127.0.0.1".parse().expect("IPv4"))]
        );
        Ok(())
    }

    #[test]
    fn build_portable_vm_allowlist_hosts_overlay_skips_ip_literals() {
        let overlay = build_portable_vm_allowlist_hosts_overlay(&[
            ("ton.example".to_owned(), "127.0.0.1".parse().expect("IPv4")),
            ("127.0.0.1".to_owned(), "127.0.0.1".parse().expect("IPv4")),
        ])
        .expect("overlay should be generated");
        assert!(overlay.contains("127.0.0.1 ton.example"));
        assert!(!overlay.contains("127.0.0.1 127.0.0.1"));
    }

    #[test]
    #[serial]
    fn portable_vm_accel_accepts_explicit_override() -> Result<()> {
        let result = portable_vm_accel_from(Some("tcg"));
        assert_eq!(result?, "tcg");
        Ok(())
    }

    #[test]
    #[serial]
    fn portable_vm_accel_rejects_unknown_override() {
        let result = portable_vm_accel_from(Some("nope"));
        assert!(result.is_err());
    }

    #[test]
    fn build_inrou_portable_network_config_matches_predictable_interface_names() {
        let network_config = build_inrou_portable_network_config();
        assert!(network_config.contains("match:\n      name: \"e*\""));
        assert!(network_config.contains("dhcp4: true"));
        assert!(!network_config.contains("  eth0:\n"));
    }

    #[test]
    fn build_inrou_user_data_projects_portable_block_mounts_and_allowlist_overlay() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (_temp_dir, replica_plan, cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let shared_mounts = vec![InrouSharedFilesystemMount {
            mount_path: "/var/lib/ton-indexer".to_owned(),
            kind: InrouSharedFilesystemMountKind::BlockDevice {
                device_serial: "sora-index_state".to_owned(),
                filesystem_type: "ext4".to_owned(),
                mount_options: "rw,nofail".to_owned(),
            },
        }];

        let user_data = build_inrou_user_data(
            &replica_plan,
            &cache_key,
            bundle
                .service
                .route
                .as_ref()
                .expect("route")
                .service_port
                .get(),
            &shared_mounts,
            Some("package_update: true\npackages:\n  - python3-minimal\n"),
            Some("127.0.0.1 api.sora.internal\n10.0.0.5 rpc.sora.internal\n"),
            Some(&replica_plan.bundle_hash),
        );

        assert!(user_data.contains("/soracloud/bundle.tgz"));
        assert!(user_data.contains("/var/lib/soracloud/materialization/bundle"));
        assert!(user_data.contains("/etc/soracloud/allowlist-hosts"));
        assert!(user_data.contains("if [ -f /etc/soracloud/allowlist-hosts ]; then"));
        assert!(user_data.contains(
            "grep -qxF \"$line\" /tmp/soracloud-hosts || echo \"$line\" >> /tmp/soracloud-hosts"
        ));
        assert!(user_data.contains("/dev/disk/by-id/virtio-sora-index_state"));
        assert!(user_data.contains("mkfs.ext4 -F \"$device_path\""));
        assert!(user_data.contains("mount -t 'ext4' -o 'rw,nofail' \"$device_path\""));
        assert!(user_data.contains("chown inrou:inrou '/var/lib/ton-indexer'"));
        assert!(user_data.contains("StandardOutput=journal+console"));
        assert!(!user_data.contains("mount.nfs"));
        assert!(!user_data.contains("virtiofs"));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn build_inrou_user_data_projects_mounts_overlay_and_replica_env() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (_temp_dir, replica_plan, cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let shared_mounts = vec![InrouSharedFilesystemMount {
            mount_path: "/var/lib/ton-indexer".to_owned(),
            kind: InrouSharedFilesystemMountKind::Nfs {
                guest_mount_source:
                    "172.31.10.1:/srv/soracloud/web_portal/revisions/2026.02.0/volumes/shared/index_state"
                        .to_owned(),
                mount_options: "rw,hard,nofail,proto=tcp,port=2049,vers=4".to_owned(),
            },
        }];

        let user_data = build_inrou_user_data(
            &replica_plan,
            &cache_key,
            bundle
                .service
                .route
                .as_ref()
                .expect("route")
                .service_port
                .get(),
            &shared_mounts,
            Some("package_update: true\npackages:\n  - python3-minimal\n"),
            None,
            None,
        );

        assert!(user_data.contains("ssh-ed25519 AAAATESTKEY soracloud-tests"));
        assert!(user_data.contains("export SORACLOUD_REPLICA_SLOT='1'"));
        assert!(user_data.contains("export PORT='8080'"));
        assert!(user_data.contains("/usr/local/bin/inrou-launch.sh"));
        assert!(user_data.contains("/usr/local/bin/inrou-prepare.sh"));
        assert!(user_data.contains("PermissionsStartOnly=true"));
        assert!(user_data.contains("StandardError=journal+console"));
        assert!(user_data.contains("ExecStartPre=/usr/local/bin/inrou-prepare.sh"));
        assert!(user_data.contains("systemctl enable --now inrou-app.service"));
        assert!(user_data.contains("uid: 1000"));
        assert!(user_data.contains("mount -t nfs -o 'rw,hard,nofail,proto=tcp,port=2049,vers=4'"));
        assert!(user_data.contains(
            "172.31.10.1:/srv/soracloud/web_portal/revisions/2026.02.0/volumes/shared/index_state"
        ));
        assert!(
            user_data
                .contains("Inrou shared service storage requires mount.nfs in the guest image")
        );
        assert!(!user_data.contains("apt-get install -y --no-install-recommends nfs-common"));
        assert!(user_data.contains("package_update: true"));
        assert!(user_data.contains("python3-minimal"));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_inrou_firecracker_config_serializes_boot_source_drives_and_network() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let temp_dir = tempfile::tempdir()?;
        let kernel_image_path = temp_dir.path().join("vmlinux");
        let initrd_image_path = temp_dir.path().join("initrd.img");
        let root_disk_path = temp_dir.path().join("rootfs.ext4");
        let seed_image_path = temp_dir.path().join("seed.ext4");
        fs::write(&kernel_image_path, b"kernel")?;
        fs::write(&initrd_image_path, b"initrd")?;
        fs::write(&root_disk_path, b"rootfs")?;
        fs::write(&seed_image_path, b"seed")?;

        let config_path = write_inrou_firecracker_config(
            temp_dir.path(),
            &kernel_image_path,
            Some(&initrd_image_path),
            &root_disk_path,
            &seed_image_path,
            &InrouTapNetworkAttachment {
                ip_binary: PathBuf::from("/sbin/ip"),
                iptables_binary: PathBuf::from("/sbin/iptables"),
                exportfs_binary: None,
                tap_name: "irtest0".to_owned(),
                host_ip: "172.31.10.1".to_owned(),
                guest_ip: "172.31.10.2".to_owned(),
                guest_mac: "06:fc:01:02:03:04".to_owned(),
                firewall_plan: InrouTapFirewallPlan::Open,
                installed_firewall_rules: Vec::new(),
                installed_nfs_exports: Vec::new(),
            },
            bundle.container.resources,
        )?;

        let config: norito::json::Value = norito::json::from_slice(&fs::read(&config_path)?)?;
        let boot_source = config
            .get("boot-source")
            .and_then(norito::json::Value::as_object)
            .expect("boot source object");
        let drives = config
            .get("drives")
            .and_then(norito::json::Value::as_array)
            .expect("drive array");
        let machine_config = config
            .get("machine-config")
            .and_then(norito::json::Value::as_object)
            .expect("machine config object");
        let network_interfaces = config
            .get("network-interfaces")
            .and_then(norito::json::Value::as_array)
            .expect("network interfaces");
        let kernel_image_path_string = kernel_image_path.display().to_string();
        let initrd_image_path_string = initrd_image_path.display().to_string();

        assert_eq!(
            boot_source
                .get("kernel_image_path")
                .and_then(norito::json::Value::as_str),
            Some(kernel_image_path_string.as_str())
        );
        assert_eq!(
            boot_source
                .get("initrd_path")
                .and_then(norito::json::Value::as_str),
            Some(initrd_image_path_string.as_str())
        );
        assert_eq!(drives.len(), 2);
        assert_eq!(
            machine_config
                .get("vcpu_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            machine_config
                .get("mem_size_mib")
                .and_then(norito::json::Value::as_u64),
            Some(512)
        );
        assert_eq!(
            network_interfaces
                .first()
                .and_then(norito::json::Value::as_object)
                .and_then(|iface| iface.get("host_dev_name"))
                .and_then(norito::json::Value::as_str),
            Some("irtest0")
        );
        assert_eq!(
            network_interfaces
                .first()
                .and_then(norito::json::Value::as_object)
                .and_then(|iface| iface.get("guest_mac"))
                .and_then(norito::json::Value::as_str),
            Some("06:fc:01:02:03:04")
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ensure_inrou_root_disk_copies_once_and_reuses_existing_rootfs() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (temp_dir, replica_plan, _cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let base_rootfs_image_path = temp_dir.path().join("base-rootfs.ext4");
        fs::write(&base_rootfs_image_path, b"base-rootfs-v1")?;
        let root_volume = replica_plan
            .lease_volumes
            .iter()
            .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
            .expect("root volume");

        let first_root_disk = ensure_inrou_root_disk(&base_rootfs_image_path, root_volume)?;
        assert_eq!(fs::read(&first_root_disk)?, b"base-rootfs-v1");

        fs::write(&base_rootfs_image_path, b"base-rootfs-v2")?;
        let second_root_disk = ensure_inrou_root_disk(&base_rootfs_image_path, root_volume)?;
        assert_eq!(first_root_disk, second_root_disk);
        assert_eq!(fs::read(&second_root_disk)?, b"base-rootfs-v1");
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (temp_dir, replica_plan, _cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let base_rootfs_image_path = temp_dir.path().join("base-rootfs.ext4");
        fs::write(&base_rootfs_image_path, b"base-rootfs-v1")?;
        let root_volume = replica_plan
            .lease_volumes
            .iter()
            .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
            .expect("root volume");
        let qemu_img = temp_dir.path().join("qemu-img");
        let args_log = temp_dir.path().join("qemu-img.args");
        fs::write(
            &qemu_img,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\nprev=\"\"\nout=\"\"\nfor arg in \"$@\"; do out=\"$prev\"; prev=\"$arg\"; done\nif [ -n \"$out\" ]; then : > \"$out\"; fi\n",
                args_log.display()
            ),
        )?;
        fs::set_permissions(&qemu_img, fs::Permissions::from_mode(0o755))?;

        let root_disk_path =
            ensure_inrou_portable_root_disk(&qemu_img, &base_rootfs_image_path, root_volume)?;
        assert_eq!(
            root_disk_path.file_name().and_then(|name| name.to_str()),
            Some("rootfs.qcow2")
        );
        let args = fs::read_to_string(args_log)?;
        assert!(args.contains("create"));
        assert!(args.contains("-f"));
        assert!(args.contains("qcow2"));
        assert!(args.contains("-F"));
        assert!(args.contains("raw"));
        assert!(args.contains(base_rootfs_image_path.display().to_string().as_str()));
        assert!(args.contains(root_disk_path.display().to_string().as_str()));
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn ensure_inrou_portable_root_disk_reuses_existing_overlay_without_qemu_img() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (temp_dir, replica_plan, _cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let root_volume = replica_plan
            .lease_volumes
            .iter()
            .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
            .expect("root volume");
        let root_disk_path =
            PathBuf::from(&root_volume.local_materialization_dir).join("rootfs.qcow2");
        fs::create_dir_all(root_disk_path.parent().expect("root disk parent"))?;
        fs::write(&root_disk_path, b"existing-overlay")?;

        let qemu_img = temp_dir.path().join("qemu-img");
        fs::write(&qemu_img, "#!/bin/sh\nexit 99\n")?;
        fs::set_permissions(&qemu_img, fs::Permissions::from_mode(0o755))?;
        let missing_base_rootfs = temp_dir.path().join("missing-base-rootfs.ext4");

        let reused = ensure_inrou_portable_root_disk(&qemu_img, &missing_base_rootfs, root_volume)?;

        assert_eq!(reused, root_disk_path);
        assert_eq!(fs::read(&reused)?, b"existing-overlay");
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn ensure_inrou_portable_root_disk_rejects_base_larger_than_budget() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (temp_dir, replica_plan, _cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let base_rootfs_image_path = temp_dir.path().join("base-rootfs.ext4");
        fs::write(&base_rootfs_image_path, b"larger-than-budget")?;
        let mut root_volume = replica_plan
            .lease_volumes
            .iter()
            .find(|volume| volume.kind == SoraLeaseVolumeKindV1::PersistentRootLeaseVolume)
            .expect("root volume")
            .clone();
        root_volume.max_total_bytes = 4;
        let qemu_img = temp_dir.path().join("missing-qemu-img");

        let error =
            ensure_inrou_portable_root_disk(&qemu_img, &base_rootfs_image_path, &root_volume)
                .expect_err("oversized base rootfs should fail before qemu-img runs");

        let message = error.to_string();
        assert!(message.contains("exceeds root lease budget"));
        assert!(message.contains(root_volume.volume_name.as_str()));
        let root_disk_path =
            PathBuf::from(&root_volume.local_materialization_dir).join("rootfs.qcow2");
        assert!(
            !root_disk_path.exists(),
            "oversized base rootfs must not leave a qcow2 overlay behind"
        );
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn ensure_inrou_portable_lease_disks_create_reusable_raw_images() -> Result<()> {
        let bundle = sample_inrou_test_bundle()?;
        let (temp_dir, replica_plan, _cache_key) =
            materialize_inrou_replica_plan_for_tests(&bundle)?;
        let qemu_img = temp_dir.path().join("qemu-img");
        let args_log = temp_dir.path().join("qemu-img.args");
        fs::write(
            &qemu_img,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> {}\nprev=\"\"\nout=\"\"\nfor arg in \"$@\"; do out=\"$prev\"; prev=\"$arg\"; done\nif [ -n \"$out\" ]; then : > \"$out\"; fi\n",
                args_log.display()
            ),
        )?;
        fs::set_permissions(&qemu_img, fs::Permissions::from_mode(0o755))?;

        let disks = ensure_inrou_portable_lease_disks(&qemu_img, &replica_plan)?;
        assert_eq!(disks.len(), 1);
        assert_eq!(
            disks[0]
                .image_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("lease.raw")
        );
        assert_eq!(disks[0].device_serial, "sora-index_state");
        let first_args = fs::read_to_string(&args_log)?;
        assert!(first_args.contains("create"));
        assert!(first_args.contains("-f"));
        assert!(first_args.contains("raw"));
        assert!(first_args.contains(disks[0].image_path.display().to_string().as_str()));

        fs::write(&args_log, "")?;
        let second_disks = ensure_inrou_portable_lease_disks(&qemu_img, &replica_plan)?;
        assert_eq!(second_disks[0].image_path, disks[0].image_path);
        assert!(fs::read_to_string(&args_log)?.is_empty());
        Ok(())
    }

    #[test]
    #[ignore = "requires unprivileged guest assets plus IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1"]
    fn inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck() -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")
            || std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")
        {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1 to run the PortableVm Inrou smoke test."
            );
            return Ok(());
        }
        require_portable_smoke_prerequisites()?;

        let kernel_image = portable_smoke_required_env_path("IROHA_INROU_PORTABLE_KERNEL_IMAGE")?;
        let rootfs_image = portable_smoke_required_env_path("IROHA_INROU_PORTABLE_ROOTFS_IMAGE")?;
        let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        if let Some(initrd_image) = initrd_image.as_ref()
            && !initrd_image.is_file()
        {
            eyre::bail!(
                "IROHA_INROU_PORTABLE_INITRD_IMAGE must point to an existing file, got {}",
                initrd_image.display()
            );
        }

        let python_http_server = r#"cat >/tmp/inrou-health.py <<'PY'
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/healthz":
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        body = b"ok\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass

HTTPServer(("0.0.0.0", int(os.environ["PORT"])), Handler).serve_forever()
PY
mkdir -p /var/lib/ton-indexer
printf 'booted\n' >/var/lib/ton-indexer/boot-marker
exec python3 /tmp/inrou-health.py
"#;
        let bootstrap_overlay = "package_update: true\npackages:\n  - python3-minimal\n";
        let temp_dir = tempfile::tempdir()?;
        let bundle_bytes = create_inrou_bundle_archive_for_linux_test(
            temp_dir.path(),
            &kernel_image,
            &rootfs_image,
            initrd_image.as_deref(),
            bootstrap_overlay,
        )?;

        let mut bundle = sample_inrou_test_bundle()?;
        bundle.container.args = vec!["-lc".to_owned(), python_http_server.to_owned()];
        bundle.container.bundle_path = "/bundles/inrou-portable-smoke.tgz".to_owned();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle
            .container
            .inrou
            .as_mut()
            .expect("inrou manifest")
            .guest_images
            .iter_mut()
            .for_each(|(guest_isa, guest_image)| {
                guest_image.initrd_image_path = initrd_image.as_ref().map(|_| match guest_isa {
                    SoraInrouGuestIsaV1::X8664 => "/inrou/x86_64/initrd.img".to_owned(),
                    SoraInrouGuestIsaV1::Aarch64 => "/inrou/aarch64/initrd.img".to_owned(),
                });
            });

        let mut state = test_state()?;
        let deployment_state = sample_deployment_state(&bundle);
        let local_peer_id = "12D3KooWPortableVmSmokePeer";
        let selected_guest_isa = current_host_inrou_guest_isa();
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
            world
                .soracloud_inrou_service_placements_mut_for_testing()
                .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
        }

        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), local_peer_id);
        config.inrou.start_grace = Duration::from_secs(240);
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let service_dir = temp_dir
            .path()
            .join("services")
            .join(sanitize_path_component(
                bundle.service.service_name.as_ref(),
            ))
            .join(sanitize_path_component(&bundle.service.service_version));
        let runtime_state = wait_for_hosted_http_runtime_state_to_be_healthy(
            &manager,
            &service_dir,
            bundle.container.lifecycle.healthcheck_path.as_deref(),
            Duration::from_secs(30),
        )?;
        let replica = runtime_state
            .replicas
            .first()
            .expect("replica runtime state present");
        assert_eq!(manager.hosted_http_workers.lock().len(), 1);
        assert!(
            service_dir
                .join("replicas/replica-0001/inrou_cloud_init/meta-data")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0001/root_disk/rootfs.qcow2")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/shared/index_state/lease.raw")
                .exists()
        );
        probe_hosted_http_health(
            replica
                .listen_base_url
                .as_deref()
                .expect("replica listen base url"),
            bundle.container.lifecycle.healthcheck_path.as_deref(),
        )?;
        Ok(())
    }

    #[test]
    #[ignore = "requires unprivileged guest assets plus IROHA_INROU_PORTABLE_SMOKE_BUNDLE_FILE"]
    fn inrou_portable_smoke_boots_external_bundle_and_serves_healthcheck() -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")
            || std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")
        {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1 to run the external bundle PortableVm smoke test."
            );
            return Ok(());
        }
        require_portable_smoke_prerequisites()?;

        let kernel_image = portable_smoke_required_env_path("IROHA_INROU_PORTABLE_KERNEL_IMAGE")?;
        let rootfs_image = portable_smoke_required_env_path("IROHA_INROU_PORTABLE_ROOTFS_IMAGE")?;
        let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        if let Some(initrd_image) = initrd_image.as_ref()
            && !initrd_image.is_file()
        {
            eyre::bail!(
                "IROHA_INROU_PORTABLE_INITRD_IMAGE must point to an existing file, got {}",
                initrd_image.display()
            );
        }
        let external_bundle =
            portable_smoke_required_env_path("IROHA_INROU_PORTABLE_SMOKE_BUNDLE_FILE")?;
        let external_entrypoint = std::env::var("IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT")
            .unwrap_or_else(|_| "/app/launch.sh".to_owned());
        let external_healthcheck = std::env::var("IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK")
            .unwrap_or_else(|_| "/health".to_owned());

        let temp_dir = tempfile::tempdir()?;
        let bundle_root = temp_dir.path().join("external-bundle-root");
        fs::create_dir_all(&bundle_root)?;
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&external_bundle)
            .arg("-C")
            .arg(&bundle_root)
            .status()?;
        if !status.success() {
            eyre::bail!(
                "tar failed while extracting external Inrou bundle {}: {status}",
                external_bundle.display()
            );
        }

        let selected_guest_isa = current_host_inrou_guest_isa();
        let guest_dir = match selected_guest_isa {
            SoraInrouGuestIsaV1::X8664 => "x86_64",
            SoraInrouGuestIsaV1::Aarch64 => "aarch64",
        };
        let inrou_dir = bundle_root.join("inrou").join(guest_dir);
        fs::create_dir_all(&inrou_dir)?;
        fs::copy(&kernel_image, inrou_dir.join("vmlinux"))?;
        fs::copy(&rootfs_image, inrou_dir.join("rootfs.ext4"))?;
        if let Some(initrd_image) = initrd_image.as_ref() {
            fs::copy(initrd_image, inrou_dir.join("initrd.img"))?;
        }

        let archive_path = temp_dir.path().join("external-inrou-bundle.tgz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&bundle_root)
            .arg(".")
            .status()?;
        if !status.success() {
            eyre::bail!("tar failed while repacking external Inrou bundle: {status}");
        }
        let bundle_bytes = fs::read(&archive_path)?;

        let mut bundle = sample_inrou_test_bundle()?;
        bundle.container.entrypoint = external_entrypoint;
        bundle.container.args.clear();
        bundle.container.bundle_path = "/bundles/external-inrou-smoke.tgz".to_owned();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle.container.lifecycle.healthcheck_path = Some(external_healthcheck);
        bundle
            .container
            .inrou
            .as_mut()
            .expect("inrou manifest")
            .bootstrap_user_data_path = None;
        bundle
            .container
            .env
            .insert("APP_ENV".to_owned(), "production".to_owned());
        bundle.container.env.insert(
            "RUST_LOG".to_owned(),
            "hayahi_ingress=debug,tower_http=debug".to_owned(),
        );
        bundle.container.env.insert(
            "SORACLOUD_TEMPLATE".to_owned(),
            "hayahi-live-smoke".to_owned(),
        );
        if let Some(route) = bundle.service.route.as_ref() {
            bundle.container.env.insert(
                "SORACLOUD_HTTP_PORT".to_owned(),
                route.service_port.get().to_string(),
            );
        }
        bundle.service.lease_volumes = vec![
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "root_disk".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                mount_path: "/".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024 * 1024).expect("bytes"),
            },
            iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
                volume_name: "shared_cache".parse().expect("volume"),
                kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
                mount_path: "/lease/shared-cache".to_owned(),
                max_total_bytes: std::num::NonZeroU64::new(512 * 1024 * 1024).expect("bytes"),
            },
        ];

        let mut state = test_state()?;
        let deployment_state = sample_deployment_state(&bundle);
        let local_peer_id = "12D3KooWPortableVmExternalBundlePeer";
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
            world
                .soracloud_inrou_service_placements_mut_for_testing()
                .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
        }

        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), local_peer_id);
        config.inrou.start_grace = Duration::from_secs(240);
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let service_dir = temp_dir
            .path()
            .join("services")
            .join(sanitize_path_component(
                bundle.service.service_name.as_ref(),
            ))
            .join(sanitize_path_component(&bundle.service.service_version));
        let runtime_state = wait_for_hosted_http_runtime_state_to_be_healthy(
            &manager,
            &service_dir,
            bundle.container.lifecycle.healthcheck_path.as_deref(),
            Duration::from_secs(30),
        )?;
        let replica = runtime_state
            .replicas
            .first()
            .expect("replica runtime state present");
        probe_hosted_http_health(
            replica
                .listen_base_url
                .as_deref()
                .expect("replica listen base url"),
            bundle.container.lifecycle.healthcheck_path.as_deref(),
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires root on a real Linux/KVM host plus explicit guest assets; set IROHA_RUN_IGNORED=1 IROHA_INROU_LINUX_KVM=1"]
    fn inrou_linux_kvm_smoke_boots_debian_guest_and_serves_healthcheck() -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")
            || std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")
        {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 IROHA_INROU_LINUX_KVM=1 to run the Linux/KVM Inrou smoke test."
            );
            return Ok(());
        }
        require_linux_kvm_smoke_prerequisites()?;

        let kernel_image = linux_smoke_required_env_path("IROHA_INROU_LINUX_KVM_KERNEL_IMAGE")?;
        let rootfs_image = linux_smoke_required_env_path("IROHA_INROU_LINUX_KVM_ROOTFS_IMAGE")?;
        let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        if let Some(initrd_image) = initrd_image.as_ref()
            && !initrd_image.is_file()
        {
            eyre::bail!(
                "IROHA_INROU_LINUX_KVM_INITRD_IMAGE must point to an existing file, got {}",
                initrd_image.display()
            );
        }

        let python_http_server = r#"cat >/tmp/inrou-health.py <<'PY'
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/healthz":
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        body = b"ok\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass

HTTPServer(("0.0.0.0", int(os.environ["PORT"])), Handler).serve_forever()
PY
mkdir -p /var/lib/ton-indexer
printf 'booted\n' >/var/lib/ton-indexer/boot-marker
exec python3 /tmp/inrou-health.py
"#;
        let bootstrap_overlay =
            "package_update: true\npackages:\n  - python3-minimal\n  - nfs-common\n";
        let temp_dir = tempfile::tempdir()?;
        let bundle_bytes = create_inrou_bundle_archive_for_linux_test(
            temp_dir.path(),
            &kernel_image,
            &rootfs_image,
            initrd_image.as_deref(),
            bootstrap_overlay,
        )?;

        let mut bundle = sample_inrou_test_bundle()?;
        bundle.container.args = vec!["-lc".to_owned(), python_http_server.to_owned()];
        bundle.container.bundle_path = "/bundles/inrou-linux-kvm-smoke.tgz".to_owned();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle
            .container
            .inrou
            .as_mut()
            .expect("inrou manifest")
            .guest_images
            .iter_mut()
            .for_each(|(guest_isa, guest_image)| {
                guest_image.initrd_image_path = initrd_image.as_ref().map(|_| match guest_isa {
                    SoraInrouGuestIsaV1::X8664 => "/inrou/x86_64/initrd.img".to_owned(),
                    SoraInrouGuestIsaV1::Aarch64 => "/inrou/aarch64/initrd.img".to_owned(),
                });
            });

        let mut state = test_state()?;
        let deployment_state = sample_deployment_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
        }

        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.inrou.start_grace = Duration::from_secs(240);
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let service_dir = temp_dir
            .path()
            .join("services")
            .join(sanitize_path_component(
                bundle.service.service_name.as_ref(),
            ))
            .join(sanitize_path_component(&bundle.service.service_version));
        let runtime_state =
            read_hosted_http_runtime_state(&service_dir)?.expect("hosted runtime state");
        let replica = runtime_state
            .replicas
            .first()
            .expect("replica runtime state present");

        assert_eq!(
            runtime_state.health_status,
            SoraServiceHealthStatusV1::Healthy
        );
        assert_eq!(replica.health_status, SoraServiceHealthStatusV1::Healthy);
        assert_eq!(manager.hosted_http_workers.lock().len(), 1);
        assert!(
            service_dir
                .join("replicas/replica-0001/firecracker-config.json")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0001/root_disk/rootfs.ext4")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/shared/index_state/boot-marker")
                .exists()
        );
        probe_hosted_http_health(
            replica
                .listen_base_url
                .as_deref()
                .expect("replica listen base url"),
            bundle.container.lifecycle.healthcheck_path.as_deref(),
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires root on a real Linux/KVM host plus explicit guest assets; set IROHA_RUN_IGNORED=1 IROHA_INROU_LINUX_KVM=1"]
    fn inrou_linux_kvm_smoke_shares_service_volume_across_replicas_and_keeps_root_state_isolated()
    -> Result<()> {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")
            || std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")
        {
            println!(
                "Skipping: set IROHA_RUN_IGNORED=1 IROHA_INROU_LINUX_KVM=1 to run the Linux/KVM Inrou replica smoke test."
            );
            return Ok(());
        }
        require_linux_kvm_smoke_prerequisites()?;

        let kernel_image = linux_smoke_required_env_path("IROHA_INROU_LINUX_KVM_KERNEL_IMAGE")?;
        let rootfs_image = linux_smoke_required_env_path("IROHA_INROU_LINUX_KVM_ROOTFS_IMAGE")?;
        let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        if let Some(initrd_image) = initrd_image.as_ref()
            && !initrd_image.is_file()
        {
            eyre::bail!(
                "IROHA_INROU_LINUX_KVM_INITRD_IMAGE must point to an existing file, got {}",
                initrd_image.display()
            );
        }

        let python_http_server = r#"cat >/tmp/inrou-shared-volume.py <<'PY'
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

slot = os.environ["SORACLOUD_REPLICA_SLOT"]
root_marker = "/var/lib/soracloud/materialization/root-slot.txt"
os.makedirs("/var/lib/ton-indexer", exist_ok=True)
with open(f"/var/lib/ton-indexer/replica-{slot}.txt", "w", encoding="utf-8") as handle:
    handle.write(f"{slot}\n")
os.makedirs("/var/lib/soracloud/materialization", exist_ok=True)
with open(root_marker, "w", encoding="utf-8") as handle:
    handle.write(f"{slot}\n")

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/healthz":
            body = b"ok\n"
        elif self.path == "/root-slot":
            with open(root_marker, "rb") as handle:
                body = handle.read()
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass

HTTPServer(("0.0.0.0", int(os.environ["PORT"])), Handler).serve_forever()
PY
exec python3 /tmp/inrou-shared-volume.py
"#;
        let bootstrap_overlay =
            "package_update: true\npackages:\n  - python3-minimal\n  - nfs-common\n";
        let temp_dir = tempfile::tempdir()?;
        let bundle_bytes = create_inrou_bundle_archive_for_linux_test(
            temp_dir.path(),
            &kernel_image,
            &rootfs_image,
            initrd_image.as_deref(),
            bootstrap_overlay,
        )?;

        let mut bundle = sample_inrou_test_bundle()?;
        bundle.container.args = vec!["-lc".to_owned(), python_http_server.to_owned()];
        bundle.container.bundle_path = "/bundles/inrou-linux-kvm-replica-smoke.tgz".to_owned();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle.service.replicas = std::num::NonZeroU16::new(2).expect("replica count");
        bundle
            .container
            .inrou
            .as_mut()
            .expect("inrou manifest")
            .guest_images
            .iter_mut()
            .for_each(|(guest_isa, guest_image)| {
                guest_image.initrd_image_path = initrd_image.as_ref().map(|_| match guest_isa {
                    SoraInrouGuestIsaV1::X8664 => "/inrou/x86_64/initrd.img".to_owned(),
                    SoraInrouGuestIsaV1::Aarch64 => "/inrou/aarch64/initrd.img".to_owned(),
                });
            });

        let mut state = test_state()?;
        let deployment_state = sample_deployment_state(&bundle);
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_service_revisions_mut_for_testing().insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment_state);
        }

        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.inrou.start_grace = Duration::from_secs(240);
        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let service_dir = temp_dir
            .path()
            .join("services")
            .join(sanitize_path_component(
                bundle.service.service_name.as_ref(),
            ))
            .join(sanitize_path_component(&bundle.service.service_version));
        let runtime_state =
            read_hosted_http_runtime_state(&service_dir)?.expect("hosted runtime state");
        assert_eq!(runtime_state.replicas.len(), 2);
        assert_eq!(
            runtime_state.health_status,
            SoraServiceHealthStatusV1::Healthy
        );

        let mut observed_root_slots = BTreeSet::new();
        for replica in &runtime_state.replicas {
            assert_eq!(replica.health_status, SoraServiceHealthStatusV1::Healthy);
            let listen_base_url = replica
                .listen_base_url
                .as_deref()
                .expect("replica listen base url");
            probe_hosted_http_health(
                listen_base_url,
                bundle.container.lifecycle.healthcheck_path.as_deref(),
            )?;
            observed_root_slots.insert(
                fetch_hosted_http_text(listen_base_url, "/root-slot")?
                    .trim()
                    .to_owned(),
            );
        }
        assert_eq!(
            observed_root_slots,
            BTreeSet::from(["1".to_owned(), "2".to_owned()])
        );
        assert_eq!(manager.hosted_http_workers.lock().len(), 2);
        assert_eq!(
            fs::read_to_string(
                temp_dir
                    .path()
                    .join("service_data/web_portal/revisions/2026.02.0/volumes/shared/index_state/replica-1.txt")
            )?,
            "1\n"
        );
        assert_eq!(
            fs::read_to_string(
                temp_dir
                    .path()
                    .join("service_data/web_portal/revisions/2026.02.0/volumes/shared/index_state/replica-2.txt")
            )?,
            "2\n"
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0001/root_disk/rootfs.ext4")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("service_data/web_portal/revisions/2026.02.0/volumes/per-replica/replica-0002/root_disk/rootfs.ext4")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn probe_hosted_http_health_accepts_paths_without_a_leading_slash() -> Result<()> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let read = std::io::Read::read(&mut stream, &mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..read]);
                let status_line = if request.starts_with("GET /healthz HTTP/1.1") {
                    "HTTP/1.1 200 OK\r\n"
                } else {
                    "HTTP/1.1 404 Not Found\r\n"
                };
                let body = if request.starts_with("GET /healthz HTTP/1.1") {
                    b"ok\n".as_slice()
                } else {
                    b"".as_slice()
                };
                let response = format!(
                    "{status_line}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::write_all(&mut stream, body);
            }
        });
        probe_hosted_http_health(&format!("http://{address}"), Some("healthz"))?;
        handle.join().expect("fixture thread should complete");
        Ok(())
    }

    #[test]
    fn fetch_hosted_http_text_accepts_paths_without_a_leading_slash() -> Result<()> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let read = std::io::Read::read(&mut stream, &mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..read]);
                let (status_line, body) = if request.starts_with("GET /root-slot HTTP/1.1") {
                    ("HTTP/1.1 200 OK\r\n", b"replica-1\n".as_slice())
                } else {
                    ("HTTP/1.1 404 Not Found\r\n", b"".as_slice())
                };
                let response = format!(
                    "{status_line}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::write_all(&mut stream, body);
            }
        });
        let body = fetch_hosted_http_text(&format!("http://{address}"), "root-slot")?;
        handle.join().expect("fixture thread should complete");
        assert_eq!(body, "replica-1\n");
        Ok(())
    }

    #[test]
    fn ivm_host_egress_fetch_enforces_allowlist_rate_and_byte_limits() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        let body = b"hello-egress".to_vec();
        let expected_hash = Hash::new(&body);
        let (url, server) = spawn_http_fixture(body.clone())?;
        let (allowed_host, allowed_port) =
            url_host_port(&url).expect("fixture URL should include a host and port");
        bundle.container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                allowed_host,
                [allowed_port],
            )]);
        let temp_dir = tempfile::tempdir()?;
        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        let mut host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["127.0.0.1".to_owned()],
                rate_per_minute: std::num::NonZeroU32::new(1),
                max_bytes_per_minute: std::num::NonZeroU64::new(32),
            },
            BTreeMap::new(),
        );
        let response = host.egress_fetch(SoracloudEgressFetchRequestV1 {
            url: url.clone(),
            expected_hash: Some(expected_hash),
            max_bytes: 32,
        })?;
        server.join().expect("fixture server should complete");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, body);
        assert_eq!(response.body_hash, expected_hash);

        let rate_limited = host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url,
                expected_hash: Some(expected_hash),
                max_bytes: 32,
            })
            .expect_err("second request must exceed the per-minute rate limit");
        assert_eq!(rate_limited, VMError::PermissionDenied);

        let disallowed = host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url: "http://example.com/blocked".to_owned(),
                expected_hash: Some(Hash::new(b"blocked")),
                max_bytes: 32,
            })
            .expect_err("disallowed hosts must be rejected before fetch");
        assert_eq!(disallowed, VMError::PermissionDenied);

        let (url, server) = spawn_http_fixture(b"too-large".to_vec())?;
        let (allowed_host, allowed_port) =
            url_host_port(&url).expect("fixture URL should include a host and port");
        bundle.container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                allowed_host,
                [allowed_port],
            )]);
        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private-2".to_vec()),
        );
        let mut byte_limited_host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["127.0.0.1".to_owned()],
                rate_per_minute: std::num::NonZeroU32::new(5),
                max_bytes_per_minute: std::num::NonZeroU64::new(4),
            },
            BTreeMap::new(),
        );
        let byte_limited = byte_limited_host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url,
                expected_hash: Some(Hash::new(b"too-large")),
                max_bytes: 16,
            })
            .expect_err("responses above the byte budget must be rejected");
        server.join().expect("fixture server should complete");
        assert_eq!(byte_limited, VMError::PermissionDenied);
        Ok(())
    }

    #[test]
    fn ivm_host_egress_fetch_rejects_allowlisted_host_on_unlisted_port() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                "127.0.0.1",
                [443],
            )]);
        let temp_dir = tempfile::tempdir()?;
        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        let mut host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["127.0.0.1".to_owned()],
                rate_per_minute: std::num::NonZeroU32::new(5),
                max_bytes_per_minute: std::num::NonZeroU64::new(32),
            },
            BTreeMap::new(),
        );

        let error = host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url: "http://127.0.0.1:9/disallowed-port".to_owned(),
                expected_hash: Some(Hash::new(b"blocked")),
                max_bytes: 32,
            })
            .expect_err("requests on unlisted ports must be rejected before fetch");
        assert_eq!(error, VMError::PermissionDenied);
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_returns_deterministic_failure_for_missing_bundle_cache() -> Result<()>
    {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes = simple_soracloud_contract_artifact(&["apply_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"missing-bundle".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request)
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        assert_eq!(
            result.runtime_state.expect("runtime state").health_status,
            SoraServiceHealthStatusV1::Degraded
        );
        assert_eq!(result.runtime_receipt.journal_artifact_hash, None);
        assert_eq!(result.runtime_receipt.checkpoint_artifact_hash, None);
        Ok(())
    }

    #[test]
    fn execute_local_read_fails_closed_when_runtime_snapshot_is_behind() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
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
                    sample_deployment_state(&bundle),
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                bundle.service.service_name.clone(),
                sample_runtime_state(&bundle),
            );
        }
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        manager.snapshot.write().observed_height = 99;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "query".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "GET".to_owned(),
                request_path: "/app/query".to_owned(),
                handler_path: "/".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"stale-query"),
            })
            .expect_err("stale runtime snapshots must fail closed");
        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        Ok(())
    }

    #[test]
    fn execute_apartment_returns_authoritative_status_and_commitment() -> Result<()> {
        let mut state = test_state()?;
        let apartment = sample_agent_record()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world.soracloud_agent_apartments_mut_for_testing().insert(
                apartment.manifest.apartment_name.to_string(),
                apartment.clone(),
            );
        }
        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let result = handle
            .execute_apartment(SoracloudApartmentExecutionRequest {
                observed_height: 0,
                observed_block_hash: None,
                apartment_name: apartment.manifest.apartment_name.to_string(),
                process_generation: apartment.process_generation,
                operation: "checkpoint".to_owned(),
                request_commitment: Hash::new(b"checkpoint-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(result.status, apartment.status);
        assert!(result.checkpoint_artifact_hash.is_none());
        assert!(result.journal_artifact_hash.is_none());
        assert_ne!(result.result_commitment, Hash::new(b"checkpoint-request"));
        Ok(())
    }

    #[test]
    fn execute_apartment_generated_hf_autonomy_run_executes_locally_and_persists_summary()
    -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_agent_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let apartment_name: Name = "hf_agent".parse().expect("valid apartment name");
        let manifest = iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
            apartment_name.clone(),
            &fixture.bundle,
        );
        let run = SoraAgentAutonomyRunRecordV1 {
            run_id: "hf_agent:autonomy:42".to_owned(),
            artifact_hash: "hash:HFAGENT#01".to_owned(),
            provenance_hash: Some("hash:HFPROV#01".to_owned()),
            budget_units: 75,
            run_label: "fallback label".to_owned(),
            workflow_input_json: Some(
                "{\"inputs\":[\"alpha\",\"beta\"],\"parameters\":{\"max_new_tokens\":4}}"
                    .to_owned(),
            ),
            approved_process_generation: 1,
            request_commitment: Hash::new(b"hf-agent-run"),
            approved_sequence: 42,
        };
        let apartment = SoraAgentApartmentRecordV1 {
            schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest_hash: Hash::new(Encode::encode(&manifest)),
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: 1,
            lease_started_sequence: 1,
            lease_expires_sequence: 400,
            last_renewed_sequence: 1,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 1,
            process_started_sequence: 1,
            last_active_sequence: 42,
            last_checkpoint_sequence: Some(42),
            checkpoint_count: 1,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 128,
                key_sizes: BTreeMap::from([("/autonomy/hf_agent:autonomy:42".to_owned(), 128)]),
            },
            revoked_policy_capabilities: BTreeSet::new(),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: 1_000,
            autonomy_budget_remaining_units: 925,
            artifact_allowlist: BTreeMap::from([(
                run.artifact_hash.clone(),
                SoraAgentArtifactAllowRuleV1 {
                    artifact_hash: run.artifact_hash.clone(),
                    provenance_hash: run.provenance_hash.clone(),
                    added_sequence: 41,
                },
            )]),
            autonomy_run_history: vec![run.clone()],
            manifest,
        };
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment_name.to_string(), apartment.clone());
        }
        let local_peer_id = "12D3KooWHfAgentAutonomyRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );

        let temp_dir = tempfile::tempdir()?;
        let source_root = temp_dir
            .path()
            .join("hf_sources")
            .join(sanitize_path_component(&fixture.source_id.to_string()));
        let files_root = source_root.join("files");
        fs::create_dir_all(&files_root)?;
        let config_json = br#"{
  "model_type": "gpt2",
  "_soracloud_fixture": {
    "mode": "echo",
    "prefix": "agent:"
  }
}"#;
        let config_path = files_root.join("config.json");
        write_bytes_atomic(&config_path, config_json)?;
        write_json_atomic(
            &source_root.join("import_manifest.json"),
            &HfLocalImportManifestV1 {
                schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
                source_id: fixture.source_id.to_string(),
                repo_id: "openai-community/gpt2".to_owned(),
                requested_revision: "main".to_owned(),
                resolved_commit: Some("main".to_owned()),
                model_name: "gpt2".to_owned(),
                adapter_id: "hf.shared.v1".to_owned(),
                pipeline_tag: Some("text-generation".to_owned()),
                library_name: Some("transformers".to_owned()),
                tags: vec!["text-generation".to_owned()],
                imported_at_ms: 20,
                imported_files: vec![HfImportedFileV1 {
                    path: "config.json".to_owned(),
                    content_length: u64::try_from(config_json.len()).unwrap_or(u64::MAX),
                    payload_hash: Hash::new(config_json).to_string(),
                    local_path: config_path.display().to_string(),
                }],
                skipped_files: Vec::new(),
                raw_model_info_path: None,
                import_error: None,
            },
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let request = SoracloudApartmentExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            apartment_name: apartment_name.to_string(),
            process_generation: apartment.process_generation,
            operation: format!("autonomy-run:{}", run.run_id),
            request_commitment: run.request_commitment,
        };
        let result = handle
            .execute_apartment(request.clone())
            .map_err(|error| eyre::eyre!("{error:?}"))?;
        assert_eq!(result.status, SoraAgentRuntimeStatusV1::Running);
        assert!(result.checkpoint_artifact_hash.is_some());
        assert!(result.journal_artifact_hash.is_some());

        let (summary, journal_hash) = read_apartment_autonomy_execution_summary(
            temp_dir.path(),
            apartment_name.as_ref(),
            &run.run_id,
        )
        .map_err(|error| eyre::eyre!("{error:?}"))?
        .expect("persisted autonomy execution summary");
        assert!(summary.succeeded);
        assert!(summary.workflow_steps.is_empty());
        assert_eq!(
            summary.service_name.as_deref(),
            Some(fixture.bundle.service.service_name.as_ref())
        );
        let runtime_receipt = summary
            .runtime_receipt
            .as_ref()
            .expect("runtime receipt persisted");
        assert_eq!(
            runtime_receipt.service_name.as_ref(),
            fixture.bundle.service.service_name.as_ref()
        );
        assert_eq!(runtime_receipt.handler_name.as_ref(), "infer");
        let response_json = summary.response_json.as_ref().expect("response json");
        assert_eq!(
            response_json
                .get("backend")
                .and_then(norito::json::Value::as_str),
            Some("local_fixture")
        );
        assert_eq!(
            response_json
                .get("inputs")
                .and_then(norito::json::Value::as_array)
                .and_then(|inputs| inputs.first())
                .and_then(norito::json::Value::as_str),
            Some("alpha")
        );
        assert_eq!(
            response_json
                .get("inputs")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            response_json
                .get("parameters")
                .and_then(norito::json::Value::as_object)
                .and_then(|parameters| parameters.get("max_new_tokens"))
                .and_then(norito::json::Value::as_u64),
            Some(4)
        );
        assert_eq!(
            response_json
                .get("text")
                .and_then(norito::json::Value::as_str),
            Some("agent:['alpha', 'beta']")
        );
        assert_eq!(result.journal_artifact_hash, Some(journal_hash));
        assert_eq!(
            result.checkpoint_artifact_hash,
            summary.checkpoint_artifact_hash
        );

        let second = handle
            .execute_apartment(request)
            .map_err(|error| eyre::eyre!("{error:?}"))?;
        assert_eq!(second.result_commitment, result.result_commitment);
        assert_eq!(second.journal_artifact_hash, result.journal_artifact_hash);
        assert_eq!(
            second.checkpoint_artifact_hash,
            result.checkpoint_artifact_hash
        );
        Ok(())
    }

    #[test]
    fn execute_apartment_generated_hf_autonomy_workflow_executes_multiple_steps_locally()
    -> Result<()> {
        let mut state = test_state()?;
        let fixture = insert_generated_hf_service_fixture(
            &mut state,
            "hf_agent_workflow_service",
            "openai-community/gpt2",
            "main",
            "gpt2",
        )?;
        set_generated_hf_service_route_visibility(
            &mut state,
            &fixture,
            SoraRouteVisibilityV1::Public,
        );
        let apartment_name: Name = "hf_workflow_agent".parse().expect("valid apartment name");
        let manifest = iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
            apartment_name.clone(),
            &fixture.bundle,
        );
        let run = SoraAgentAutonomyRunRecordV1 {
            run_id: "hf_workflow_agent:autonomy:9".to_owned(),
            artifact_hash: "hash:HFAGENT#WF".to_owned(),
            provenance_hash: Some("hash:HFPROV#WF".to_owned()),
            budget_units: 90,
            run_label: "workflow".to_owned(),
            workflow_input_json: Some(
                "{\"workflow_version\":1,\"steps\":[{\"step_id\":\"draft\",\"request\":{\"inputs\":\"alpha\"}},{\"step_id\":\"refine\",\"request\":{\"inputs\":\"${steps.draft.text}\",\"parameters\":{\"max_new_tokens\":2}}}]}"
                    .to_owned(),
            ),
            approved_process_generation: 1,
            request_commitment: Hash::new(b"hf-agent-workflow-run"),
            approved_sequence: 9,
        };
        let apartment = SoraAgentApartmentRecordV1 {
            schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest_hash: Hash::new(Encode::encode(&manifest)),
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: 1,
            lease_started_sequence: 1,
            lease_expires_sequence: 400,
            last_renewed_sequence: 1,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 1,
            process_started_sequence: 1,
            last_active_sequence: 9,
            last_checkpoint_sequence: Some(9),
            checkpoint_count: 1,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 128,
                key_sizes: BTreeMap::from([(
                    "/autonomy/hf_workflow_agent:autonomy:9".to_owned(),
                    128,
                )]),
            },
            revoked_policy_capabilities: BTreeSet::new(),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: 1_000,
            autonomy_budget_remaining_units: 910,
            artifact_allowlist: BTreeMap::from([(
                run.artifact_hash.clone(),
                SoraAgentArtifactAllowRuleV1 {
                    artifact_hash: run.artifact_hash.clone(),
                    provenance_hash: run.provenance_hash.clone(),
                    added_sequence: 8,
                },
            )]),
            autonomy_run_history: vec![run.clone()],
            manifest,
        };
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment_name.to_string(), apartment.clone());
        }
        let local_peer_id = "12D3KooWHfWorkflowAutonomyRuntimeHost";
        insert_generated_hf_placement_fixture(
            &mut state,
            &fixture,
            SoraHfPlacementHostRoleV1::Primary,
            SoraHfPlacementHostStatusV1::Warm,
            local_peer_id,
        );

        let temp_dir = tempfile::tempdir()?;
        let source_root = temp_dir
            .path()
            .join("hf_sources")
            .join(sanitize_path_component(&fixture.source_id.to_string()));
        let files_root = source_root.join("files");
        fs::create_dir_all(&files_root)?;
        let config_json = br#"{
  "model_type": "gpt2",
  "_soracloud_fixture": {
    "mode": "echo",
    "prefix": "wf:"
  }
}"#;
        let config_path = files_root.join("config.json");
        write_bytes_atomic(&config_path, config_json)?;
        write_json_atomic(
            &source_root.join("import_manifest.json"),
            &HfLocalImportManifestV1 {
                schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
                source_id: fixture.source_id.to_string(),
                repo_id: "openai-community/gpt2".to_owned(),
                requested_revision: "main".to_owned(),
                resolved_commit: Some("main".to_owned()),
                model_name: "gpt2".to_owned(),
                adapter_id: "hf.shared.v1".to_owned(),
                pipeline_tag: Some("text-generation".to_owned()),
                library_name: Some("transformers".to_owned()),
                tags: vec!["text-generation".to_owned()],
                imported_at_ms: 20,
                imported_files: vec![HfImportedFileV1 {
                    path: "config.json".to_owned(),
                    content_length: u64::try_from(config_json.len()).unwrap_or(u64::MAX),
                    payload_hash: Hash::new(config_json).to_string(),
                    local_path: config_path.display().to_string(),
                }],
                skipped_files: Vec::new(),
                raw_model_info_path: None,
                import_error: None,
            },
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf())
                .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let result = handle
            .execute_apartment(SoracloudApartmentExecutionRequest {
                observed_height: 0,
                observed_block_hash: None,
                apartment_name: apartment_name.to_string(),
                process_generation: apartment.process_generation,
                operation: format!("autonomy-run:{}", run.run_id),
                request_commitment: run.request_commitment,
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        let (summary, _journal_hash) = read_apartment_autonomy_execution_summary(
            temp_dir.path(),
            apartment_name.as_ref(),
            &run.run_id,
        )
        .map_err(|error| eyre::eyre!("{error:?}"))?
        .expect("persisted workflow summary");
        assert!(summary.succeeded);
        assert_eq!(summary.workflow_steps.len(), 2);
        assert_eq!(summary.workflow_steps[0].step_id.as_deref(), Some("draft"));
        assert_eq!(
            summary.workflow_steps[0]
                .response_json
                .as_ref()
                .and_then(|value| value.get("text"))
                .and_then(norito::json::Value::as_str),
            Some("wf:alpha")
        );
        assert_eq!(summary.workflow_steps[1].step_id.as_deref(), Some("refine"));
        assert_eq!(
            summary.workflow_steps[1]
                .response_json
                .as_ref()
                .and_then(|value| value.get("inputs"))
                .and_then(norito::json::Value::as_str),
            Some("wf:alpha")
        );
        let response_json = summary
            .response_json
            .as_ref()
            .expect("workflow response json");
        assert_eq!(
            response_json
                .get("step_count")
                .and_then(norito::json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            response_json
                .get("final_response")
                .and_then(|value| value.get("text"))
                .and_then(norito::json::Value::as_str),
            Some("wf:wf:alpha")
        );
        assert_eq!(
            result.checkpoint_artifact_hash,
            summary.checkpoint_artifact_hash
        );
        Ok(())
    }
}
