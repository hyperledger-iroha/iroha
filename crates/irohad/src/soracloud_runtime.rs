//! Embedded Soracloud runtime-manager reconciliation for `irohad`.
//!
//! This subsystem continuously projects authoritative Soracloud world state
//! into a node-local materialization plan and now serves deterministic local
//! reads/apartment observations directly from the committed snapshot plus the
//! hydrated artifact cache. Soracloud runtime v1 is currently `Ivm`-only;
//! `NativeProcess` deployments are rejected during admission and runtime
//! activation.
//!
//! Ordered mailbox execution now runs admitted IVM bundles directly through
//! the Soracloud host surface while local reads continue to resolve from the
//! committed snapshot plus hydrated artifact cache.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs, io,
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
    SoracloudApartmentAutonomyExecutionSummaryV1, SoracloudApartmentAutonomyWorkflowStepSummaryV1,
    SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
    SoracloudLocalReadRequest, SoracloudLocalReadResponse, SoracloudOrderedMailboxExecutionRequest,
    SoracloudOrderedMailboxExecutionResult, SoracloudPrivateInferenceExecutionAction,
    SoracloudPrivateInferenceExecutionRequest, SoracloudPrivateInferenceExecutionResult,
    SoracloudRuntime, SoracloudRuntimeApartmentPlan, SoracloudRuntimeArtifactPlan,
    SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
    SoracloudRuntimeHfSourcePlan, SoracloudRuntimeHfSourceStatus, SoracloudRuntimeMailboxPlan,
    SoracloudRuntimeReadHandle, SoracloudRuntimeRevisionRole, SoracloudRuntimeServicePlan,
    SoracloudRuntimeSnapshot, SoracloudUploadedModelEncryptionRecipient,
    soracloud_hf_generated_bundle_payload_if_applicable, soracloud_hf_generated_source_binding,
};
use iroha_core::{queue::Queue, tx::AcceptedTransaction};
use iroha_core::state::{State, StateReadOnly, StateView, WorldReadOnly};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    Encode,
    ChainId,
    isi::{self, InstructionBox},
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SORA_RUNTIME_RECEIPT_VERSION_V1, SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1, SORACLOUD_HOST_RESPONSE_VERSION_V1,
        SoraAgentApartmentRecordV1, SoraAgentRuntimeStatusV1, SoraArtifactKindV1,
        SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1, SoraDeploymentBundleV1,
        encode_model_host_heartbeat_provenance_payload,
        SoraModelHostViolationKindV1,
        SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1,
        SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
        SoraHfSharedLeaseMemberStatusV1, SoraHfSharedLeaseStatusV1, SoraHfSourceStatusV1,
        SoraModelPrivacyModeV1, SoraNetworkPolicyV1, SoraPrivateInferenceCheckpointV1,
        SoraPrivateInferenceSessionStatusV1, SoraPrivateInferenceSessionV1, SoraRuntimeReceiptV1,
        SoraRouteVisibilityV1, SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHandlerV1,
        SoraServiceHealthStatusV1, SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1,
        SoraServiceRuntimeStateV1, SoraServiceStateEntryV1, SoraStateBindingV1,
        SoraStateMutationOperationV1, SoraUploadedModelBindingStatusV1, SoraUploadedModelBundleV1,
        SoraUploadedModelKeyEncapsulationV1, SoraUploadedModelKeyWrapAeadV1,
        SoracloudAppendJournalResponseV1, SoracloudEgressFetchRequestV1,
        SoracloudEgressFetchResponseV1, SoracloudEmitMailboxMessageRequestV1,
        SoracloudEmitMailboxMessageResponseV1, SoracloudEmitStateMutationRequestV1,
        SoracloudEmitStateMutationResponseV1, SoracloudHostOperationV1,
        SoracloudHostRequestEnvelopeV1, SoracloudHostRequestPayloadV1,
        SoracloudHostResponseEnvelopeV1, SoracloudHostResponsePayloadV1,
        SoracloudPublishCheckpointResponseV1, SoracloudReadCommittedStateResponseV1,
        SoracloudReadCredentialResponseV1, SoracloudReadSecretResponseV1,
    },
    sorafs::pin_registry::ManifestDigest,
    transaction::TransactionBuilder,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_torii::sorafs::{
    EndpointKind, ProviderAdvertCache, ReplicationOrderV1, TransportProtocol,
    api::StorageManifestResponseDto,
};
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    syscalls::{
        SYSCALL_SORACLOUD_APPEND_JOURNAL, SYSCALL_SORACLOUD_EGRESS_FETCH,
        SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE, SYSCALL_SORACLOUD_EMIT_STATE_MUTATION,
        SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT, SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
        SYSCALL_SORACLOUD_READ_CREDENTIAL, SYSCALL_SORACLOUD_READ_SECRET,
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

/// Runtime-manager configuration derived from the explicit Soracloud runtime settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SoracloudRuntimeManagerConfig {
    /// Root directory for local runtime materialization state.
    pub state_dir: PathBuf,
    /// Reconciliation cadence against authoritative state.
    pub reconcile_interval: Duration,
    /// Reserved concurrency budget for future hydration workers.
    pub hydration_concurrency: NonZeroUsize,
    /// Configured artifact-cache budgets for the embedded runtime manager.
    pub cache_budgets: iroha_config::parameters::actual::SoracloudRuntimeCacheBudgets,
    /// Deterministic `NativeProcess` hosting limits.
    pub native_process: iroha_config::parameters::actual::SoracloudRuntimeNativeProcess,
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
}

/// Queue-backed mutation sink used by `irohad` to report runtime-originated Soracloud health events.
#[derive(Clone)]
pub(crate) struct QueuedSoracloudRuntimeMutationSink {
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
    authority: AccountId,
    key_pair: KeyPair,
}

impl QueuedSoracloudRuntimeMutationSink {
    /// Construct the queue-backed mutation sink using the node's validator authority.
    pub(crate) fn new(
        chain_id: Arc<ChainId>,
        queue: Arc<Queue>,
        state: Arc<State>,
        authority: AccountId,
        key_pair: KeyPair,
    ) -> Self {
        Self {
            chain_id,
            queue,
            state,
            authority,
            key_pair,
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
            .sign(self.key_pair.private_key());
        let view = self.state.view();
        let params = view.world().parameters();
        let accepted = AcceptedTransaction::accept(
            tx,
            &self.chain_id,
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            self.state.crypto().as_ref(),
        )
        .wrap_err_with(|| format!("accept internal Soracloud runtime mutation at `{endpoint}`"))?;
        drop(view);
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
        self.submit_instruction(instruction, "/internal/soracloud/runtime/model-host-heartbeat")
    }
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
        Self {
            state_dir: config.state_dir.clone(),
            reconcile_interval: config.reconcile_interval,
            hydration_concurrency: config.hydration_concurrency,
            cache_budgets: config.cache_budgets.clone(),
            native_process: config.native_process.clone(),
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
        if request.handler_name != "infer" || error.kind == SoracloudRuntimeExecutionErrorKind::InvalidRequest
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
            SoraHfPlacementHostStatusV1::Warm => SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
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
        let has_warm_primary = iroha_core::soracloud_runtime::resolve_generated_hf_primary_assignment(
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

        let committed_entries = collect_committed_service_state_entries(
            &self.state.view(),
            request.deployment.service_name.as_ref(),
        );
        let host = SoracloudIvmHost::new(
            request.clone(),
            self.state_dir(),
            self.config.egress.clone(),
            committed_entries,
        );
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
        match mailbox_payload_tlv_bytes(&request.mailbox_message.payload_bytes) {
            Ok(tlv_bytes) => match vm.alloc_input_tlv(&tlv_bytes) {
                Ok(ptr) => vm.set_register(10, ptr),
                Err(error) => {
                    return Ok(deterministic_mailbox_failure_result(
                        request,
                        vm_error_label(&error),
                        SoraServiceHealthStatusV1::Degraded,
                    ));
                }
            },
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result(
                    request,
                    vm_error_label(&error),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        }
        vm.set_register(11, request.execution_sequence);
        vm.set_register(12, request.observed_height);
        if let Err(error) = vm.run() {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
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
        .into_execution_result()
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

    fn execute_private_inference(
        &self,
        request: SoracloudPrivateInferenceExecutionRequest,
    ) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError> {
        let view = self.state.view();
        let snapshot = self.snapshot();
        validate_private_inference_snapshot(&view, &snapshot, &request)?;
        let world = view.world();
        let Some(record) = world
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

        let session_key = (request.apartment_name.clone(), request.session_id.clone());
        let Some(session) = world
            .soracloud_private_inference_sessions()
            .get(&session_key)
            .cloned()
        else {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "private session `{}` is not registered for apartment `{}`",
                    request.session_id, request.apartment_name
                ),
            ));
        };
        let Some(binding) = record.uploaded_model_binding.as_ref() else {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "apartment `{}` does not have an uploaded model binding",
                    request.apartment_name
                ),
            ));
        };
        if binding.status != SoraUploadedModelBindingStatusV1::Active {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "uploaded model binding for apartment `{}` is {:?}",
                    request.apartment_name, binding.status
                ),
            ));
        }
        if binding.privacy_mode != SoraModelPrivacyModeV1::PrivateExecution {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "uploaded model binding for apartment `{}` is not private execution",
                    request.apartment_name
                ),
            ));
        }
        if binding.service_name != session.service_name
            || binding.model_id != session.model_id
            || binding.weight_version != session.weight_version
            || binding.bundle_root != session.bundle_root
        {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "private session `{}` does not match apartment `{}` uploaded model binding",
                    request.session_id, request.apartment_name
                ),
            ));
        }

        let bundle_key = (
            session.service_name.as_ref().to_owned(),
            session.model_id.clone(),
            session.weight_version.clone(),
        );
        let Some(bundle) = world
            .soracloud_uploaded_model_bundles()
            .get(&bundle_key)
            .cloned()
        else {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "uploaded model bundle `{}` version `{}` is not registered for service `{}`",
                    session.model_id, session.weight_version, session.service_name
                ),
            ));
        };
        if bundle.bundle_root != session.bundle_root
            || bundle.compile_profile_hash != binding.compile_profile_hash
        {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "private session `{}` no longer matches the admitted bundle manifest",
                    request.session_id
                ),
            ));
        }
        if world
            .soracloud_private_compile_profiles()
            .get(&bundle.compile_profile_hash)
            .is_none()
        {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "private compile profile `{}` is not materialized for uploaded model `{}`",
                    bundle.compile_profile_hash, session.model_id
                ),
            ));
        }

        match &request.action {
            SoracloudPrivateInferenceExecutionAction::Start => {
                execute_private_inference_start(&view, &bundle, &session, &request)
            }
            SoracloudPrivateInferenceExecutionAction::Release { decrypt_request_id } => {
                execute_private_inference_release(
                    &view,
                    &bundle,
                    &session,
                    decrypt_request_id,
                    &request,
                )
            }
        }
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
    committed_entries: BTreeMap<(String, String), SoraServiceStateEntryV1>,
    binding_totals: BTreeMap<String, u64>,
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
            committed_entries,
            binding_totals,
            staged_state_mutations: Vec::new(),
            staged_outbound_mailbox_messages: Vec::new(),
            staged_journal: None,
            staged_checkpoint: None,
            egress_requests: 0,
            egress_bytes: 0,
        }
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
            Err(VMError::NotImplemented { syscall })
        }
    }

    fn read_request_payload(
        &self,
        vm: &mut IVM,
        expected_operation: SoracloudHostOperationV1,
        syscall: u32,
    ) -> Result<SoracloudHostRequestPayloadV1, VMError> {
        let tlv = vm.memory.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::SoracloudRequest {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id_raw(),
            });
        }
        let envelope = norito::decode_from_bytes::<SoracloudHostRequestEnvelopeV1>(tlv.payload)
            .map_err(|_| VMError::NoritoInvalid)?;
        envelope.validate().map_err(|_| VMError::NoritoInvalid)?;
        if envelope.operation != expected_operation {
            return Err(VMError::NotImplemented { syscall });
        }
        Ok(envelope.payload)
    }

    fn write_response(
        &self,
        vm: &mut IVM,
        operation: SoracloudHostOperationV1,
        payload: SoracloudHostResponsePayloadV1,
    ) -> Result<(), VMError> {
        let envelope = SoracloudHostResponseEnvelopeV1 {
            schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
            operation,
            payload,
        };
        envelope.validate().map_err(|_| VMError::NoritoInvalid)?;
        let payload_bytes = norito::to_bytes(&envelope).map_err(|_| VMError::NoritoInvalid)?;
        let tlv = make_pointer_tlv(PointerType::SoracloudResponse, &payload_bytes);
        let ptr = vm.alloc_input_tlv(&tlv)?;
        vm.set_register(10, ptr);
        Ok(())
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
                let Some(payload_bytes) = request.payload_bytes else {
                    return Err(VMError::NoritoInvalid);
                };
                if payload_bytes == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let Some(payload_commitment) = request.payload_commitment else {
                    return Err(VMError::NoritoInvalid);
                };
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
                if request.payload_bytes.is_some() || request.payload_commitment.is_some() {
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

        let mutation = iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation {
            binding_name,
            state_key: request.state_key.clone(),
            operation: request.operation,
            encryption: request.encryption,
            payload_bytes: request.payload_bytes,
            payload_commitment: request.payload_commitment,
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
    ) -> SoracloudEmitMailboxMessageResponseV1 {
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
        SoracloudEmitMailboxMessageResponseV1 {
            message_id,
            payload_commitment,
        }
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

    fn host_network_allows(&self, host: &str) -> bool {
        let container_policy: &SoraCapabilityPolicyV1 = &self.request.bundle.container.capabilities;
        let container_allows = match &container_policy.network {
            SoraNetworkPolicyV1::Isolated => false,
            SoraNetworkPolicyV1::Allowlist(hosts) => hosts.iter().any(|allowed| allowed == host),
        };
        if !container_allows {
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
        let Some(host) = url_host(&request.url) else {
            return Err(VMError::PermissionDenied);
        };
        if !self.host_network_allows(host) {
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
}

impl IVMHost for SoracloudIvmHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            SYSCALL_SORACLOUD_READ_COMMITTED_STATE => {
                let SoracloudHostRequestPayloadV1::ReadCommittedState(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::ReadCommittedState,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let entry = self
                    .committed_entries
                    .get(&Self::state_entry_key(
                        &request.binding_name,
                        &request.state_key,
                    ))
                    .cloned();
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCommittedState,
                    SoracloudHostResponsePayloadV1::ReadCommittedState(
                        SoracloudReadCommittedStateResponseV1 { entry },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => {
                let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::EmitStateMutation,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.stage_state_mutation(request)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitStateMutation,
                    SoracloudHostResponsePayloadV1::EmitStateMutation(response),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => {
                let SoracloudHostRequestPayloadV1::EmitMailboxMessage(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::EmitMailboxMessage,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.stage_outbound_mailbox_message(request);
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitMailboxMessage,
                    SoracloudHostResponsePayloadV1::EmitMailboxMessage(response),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_APPEND_JOURNAL => {
                let SoracloudHostRequestPayloadV1::AppendJournal(request) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::AppendJournal, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
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
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => {
                let SoracloudHostRequestPayloadV1::PublishCheckpoint(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::PublishCheckpoint,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
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
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_READ_SECRET => {
                self.require_private_runtime(number)?;
                let SoracloudHostRequestPayloadV1::ReadSecret(request) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::ReadSecret, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let payload_bytes = self.read_material("secrets", &request.secret_name)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadSecret,
                    SoracloudHostResponsePayloadV1::ReadSecret(SoracloudReadSecretResponseV1 {
                        found: payload_bytes.is_some(),
                        payload_bytes: payload_bytes.unwrap_or_default(),
                    }),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_READ_CREDENTIAL => {
                self.require_private_runtime(number)?;
                let SoracloudHostRequestPayloadV1::ReadCredential(request) = self
                    .read_request_payload(vm, SoracloudHostOperationV1::ReadCredential, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let payload_bytes = self.read_material("credentials", &request.credential_name)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCredential,
                    SoracloudHostResponsePayloadV1::ReadCredential(
                        SoracloudReadCredentialResponseV1 {
                            found: payload_bytes.is_some(),
                            payload_bytes: payload_bytes.unwrap_or_default(),
                        },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EGRESS_FETCH => {
                let SoracloudHostRequestPayloadV1::EgressFetch(request) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::EgressFetch, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.egress_fetch(request)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EgressFetch,
                    SoracloudHostResponsePayloadV1::EgressFetch(response),
                )?;
                Ok(0)
            }
            _ => Err(VMError::UnknownSyscall(number)),
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

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct LocalQueryResponse {
    schema_version: u16,
    service_name: String,
    service_version: String,
    handler_name: String,
    observed_height: u64,
    observed_block_hash: Option<String>,
    entries: Vec<LocalQueryEntry>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct LocalQueryEntry {
    binding_name: String,
    state_key: String,
    service_version: String,
    encryption: iroha_data_model::soracloud::SoraStateEncryptionV1,
    payload_bytes: u64,
    payload_commitment: Hash,
    last_update_sequence: u64,
    governance_tx_hash: Hash,
    source_action: SoraServiceLifecycleActionV1,
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
    host_violation_reporter: Arc<SoracloudModelHostViolationReporter>,
    mutation_sink: Option<Arc<dyn SoracloudRuntimeMutationSink>>,
    last_model_host_heartbeat_attempt_ms: Mutex<Option<u64>>,
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

impl SoracloudRuntimeManager {
    /// Construct the runtime manager for the supplied node state.
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, state: Arc<State>) -> Self {
        Self {
            config,
            state,
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
            hf_local_workers: Arc::new(Mutex::new(BTreeMap::new())),
            host_violation_reporter: SoracloudModelHostViolationReporter::disabled(),
            mutation_sink: None,
            last_model_host_heartbeat_attempt_ms: Mutex::new(None),
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
            .map_err(|panic| eyre::eyre!("Soracloud startup reconcile thread panicked: {panic:?}"))?
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

        let view = self.state.view();
        self.report_local_model_host_advert_contradictions(&view);
        let bundle_registry = collect_service_revision_registry(&view);
        let initial_snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;

        self.write_service_materializations(&initial_snapshot, &bundle_registry)?;
        self.write_apartment_materializations(&initial_snapshot, &view)?;
        self.prune_stale_service_materializations(&initial_snapshot)?;
        self.prune_stale_apartment_materializations(&initial_snapshot)?;
        self.import_hf_sources(&view, &initial_snapshot)?;
        self.probe_local_hf_execution_hosts(&view, &initial_snapshot);
        self.hydrate_missing_artifacts(&view, &initial_snapshot, &bundle_registry)?;
        self.enforce_cache_budgets(&view, &initial_snapshot)?;
        let snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;
        self.prune_stale_hf_local_workers(&snapshot);
        write_json_atomic(
            &self.config.state_dir.join("runtime_snapshot.json"),
            &snapshot,
        )?;
        *self.snapshot.write() = snapshot;
        Ok(())
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
        let active_sources = snapshot
            .hf_sources
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
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

    fn services_root(&self) -> PathBuf {
        self.config.state_dir.join("services")
    }

    fn apartments_root(&self) -> PathBuf {
        self.config.state_dir.join("apartments")
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

    fn hf_source_root(&self, source_id: &str) -> PathBuf {
        self.hf_sources_root()
            .join(sanitize_path_component(source_id))
    }

    fn write_service_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
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
                if let Some(bundle) =
                    bundle_registry.get(&(service_name.clone(), service_version.clone()))
                {
                    write_json_atomic(&version_dir.join("deployment_bundle.json"), bundle)?;
                }
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

    fn local_hf_source_assignment_allowed(
        &self,
        view: &StateView<'_>,
        source_id: &str,
    ) -> bool {
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
                SoraHfPlacementHostStatusV1::Warm => (
                    SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
                    "warm",
                ),
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
        view: &StateView<'_>,
        stored_manifests: &[StoredManifest],
        remote_sources: &[RemoteHydrationSource],
        expected_hash: Hash,
    ) -> eyre::Result<Option<Vec<u8>>> {
        if let Some(sorafs_node) = self.sorafs_node.as_ref() {
            for manifest in stored_manifests {
                if !manifest_is_committed(view, &self.state, manifest.manifest_digest()) {
                    continue;
                }
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

    let filters = parse_query_params(request.request_query.as_deref());
    let binding_filter = filters.get("binding").map(String::as_str);
    let key_filter = filters.get("key").map(String::as_str);
    let prefix_filter = filters.get("prefix").map(String::as_str);
    let limit = filters
        .get("limit")
        .and_then(|limit| limit.parse::<usize>().ok())
        .unwrap_or(256)
        .max(1);

    let mut rows = view
        .world()
        .soracloud_service_state_entries()
        .iter()
        .filter(|((_service_name, binding_name, state_key), entry)| {
            entry.service_name.as_ref() == request.service_name
                && binding_filter.is_none_or(|filter| filter == binding_name.as_str())
                && key_filter.is_none_or(|filter| filter == state_key.as_str())
                && prefix_filter.is_none_or(|filter| state_key.starts_with(filter))
        })
        .map(|((_service_name, _binding_name, _state_key), entry)| entry.clone())
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.binding_name
            .cmp(&right.binding_name)
            .then_with(|| left.state_key.cmp(&right.state_key))
    });
    rows.truncate(limit);

    let entries = rows
        .iter()
        .map(|entry| LocalQueryEntry {
            binding_name: entry.binding_name.to_string(),
            state_key: entry.state_key.clone(),
            service_version: entry.service_version.clone(),
            encryption: entry.encryption,
            payload_bytes: entry.payload_bytes.get(),
            payload_commitment: entry.payload_commitment,
            last_update_sequence: entry.last_update_sequence,
            governance_tx_hash: entry.governance_tx_hash,
            source_action: entry.source_action,
        })
        .collect::<Vec<_>>();
    let response = LocalQueryResponse {
        schema_version: 1,
        service_name: request.service_name.clone(),
        service_version: context.deployment.current_service_version.clone(),
        handler_name: context.handler.handler_name.to_string(),
        observed_height: request.observed_height,
        observed_block_hash: request.observed_block_hash.map(|hash| hash.to_string()),
        entries,
    };
    let response_bytes = norito::json::to_json(&response)
        .map(|json| json.into_bytes())
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("serialize Soracloud query response: {error}"),
            )
        })?;
    let bindings = rows.iter().map(state_entry_binding).collect::<Vec<_>>();
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
        content_type: Some("application/json".to_owned()),
        content_encoding: None,
        cache_control: Some("no-store".to_owned()),
        bindings,
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt,
    })
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
            format!(
                "materialize embedded HF local runner for source `{source_id}`: {error}"
            ),
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
            format!(
                "serialize local HF worker probe request for source `{source_id}`: {error}"
            ),
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
            format!(
                "generated HF local worker probe for source `{source_id}` failed: {message}"
            ),
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
    if snapshot.observed_height != committed_height
        || parse_snapshot_hash(snapshot.observed_block_hash.as_deref())? != committed_block_hash
    {
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

fn validate_private_inference_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudPrivateInferenceExecutionRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height
        || request.observed_block_hash != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "private inference snapshot is stale: request observed height/hash {:?}/{:?}, committed {:?}/{:?}",
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

const PRIVATE_INFERENCE_MAX_COMPUTE_UNITS_V1: u64 = 16;

fn private_inference_compute_units(session: &SoraPrivateInferenceSessionV1) -> u64 {
    u64::from(session.token_budget)
        .max(1)
        .min(PRIVATE_INFERENCE_MAX_COMPUTE_UNITS_V1)
}

fn private_inference_updated_at_ms(view: &StateView<'_>) -> u64 {
    view.latest_block()
        .map(|block| u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1)
        .max(1)
}

fn private_inference_ciphertext_state_root(
    session: &SoraPrivateInferenceSessionV1,
    bundle: &SoraUploadedModelBundleV1,
    step: u32,
    request_commitment: Hash,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.private.ciphertext_state.v1",
        session.session_id.as_str(),
        session.apartment.as_ref(),
        session.service_name.as_ref(),
        session.model_id.as_str(),
        session.weight_version.as_str(),
        bundle.bundle_root,
        bundle.compile_profile_hash,
        (
            session.input_commitments.clone(),
            session.token_budget,
            session.image_budget,
            step,
            request_commitment,
        ),
    )))
}

fn private_inference_receipt_hash(
    session_id: &str,
    step: u32,
    phase: &str,
    request_commitment: Hash,
    ciphertext_state_root: Hash,
    released_token: Option<&str>,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.private.receipt.v1",
        session_id,
        step,
        phase,
        request_commitment,
        ciphertext_state_root,
        released_token,
    )))
}

fn private_inference_receipt_root(
    session_id: &str,
    step: u32,
    status: SoraPrivateInferenceSessionStatusV1,
    xor_cost_nanos: u128,
    receipt_hash: Hash,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.private.receipt_root.v1",
        session_id,
        step,
        status,
        xor_cost_nanos,
        receipt_hash,
    )))
}

fn private_inference_result_commitment(
    session_id: &str,
    action: &SoracloudPrivateInferenceExecutionAction,
    status: SoraPrivateInferenceSessionStatusV1,
    receipt_root: Hash,
    xor_cost_nanos: u128,
    checkpoint: &SoraPrivateInferenceCheckpointV1,
) -> Hash {
    let (action_label, decrypt_request_id) = match action {
        SoracloudPrivateInferenceExecutionAction::Start => ("start", None),
        SoracloudPrivateInferenceExecutionAction::Release { decrypt_request_id } => {
            ("release", Some(decrypt_request_id.as_str()))
        }
    };
    Hash::new(Encode::encode(&(
        "soracloud.private.result.v1",
        session_id,
        action_label,
        decrypt_request_id,
        status,
        receipt_root,
        xor_cost_nanos,
        checkpoint.clone(),
    )))
}

fn latest_private_inference_checkpoint(
    view: &StateView<'_>,
    session_id: &str,
) -> Option<SoraPrivateInferenceCheckpointV1> {
    view.world()
        .soracloud_private_inference_checkpoints()
        .iter()
        .filter_map(|((stored_session_id, _step), checkpoint)| {
            (stored_session_id == session_id).then(|| checkpoint.clone())
        })
        .max_by_key(|checkpoint| checkpoint.step)
}

fn private_inference_released_token(
    session: &SoraPrivateInferenceSessionV1,
    checkpoint: &SoraPrivateInferenceCheckpointV1,
    request_commitment: Hash,
) -> String {
    Hash::new(Encode::encode(&(
        "soracloud.private.released_token.v1",
        session.session_id.as_str(),
        session.model_id.as_str(),
        session.weight_version.as_str(),
        checkpoint.step,
        checkpoint.receipt_hash,
        request_commitment,
    )))
    .to_string()
    .chars()
    .take(24)
    .collect()
}

fn execute_private_inference_start(
    view: &StateView<'_>,
    bundle: &SoraUploadedModelBundleV1,
    session: &SoraPrivateInferenceSessionV1,
    request: &SoracloudPrivateInferenceExecutionRequest,
) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError> {
    if !matches!(
        session.status,
        SoraPrivateInferenceSessionStatusV1::Admitted
            | SoraPrivateInferenceSessionStatusV1::Running
    ) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` is {:?}, expected Admitted or Running before runtime start",
                session.session_id, session.status
            ),
        ));
    }
    if latest_private_inference_checkpoint(view, &session.session_id).is_some() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` already has a recorded checkpoint",
                session.session_id
            ),
        ));
    }

    let step = 1;
    let compute_units = private_inference_compute_units(session);
    let ciphertext_state_root =
        private_inference_ciphertext_state_root(session, bundle, step, request.request_commitment);
    let receipt_hash = private_inference_receipt_hash(
        &session.session_id,
        step,
        "start",
        request.request_commitment,
        ciphertext_state_root,
        None,
    );
    let xor_cost_nanos = session.xor_cost_nanos.saturating_add(
        bundle
            .pricing_policy
            .runtime_step_xor_nanos
            .saturating_mul(u128::from(compute_units)),
    );
    let receipt_root = private_inference_receipt_root(
        &session.session_id,
        step,
        SoraPrivateInferenceSessionStatusV1::AwaitingDecryption,
        xor_cost_nanos,
        receipt_hash,
    );
    let checkpoint = SoraPrivateInferenceCheckpointV1 {
        schema_version: iroha_data_model::soracloud::SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
        session_id: session.session_id.clone(),
        step,
        ciphertext_state_root,
        receipt_hash,
        decrypt_request_id: format!("{}:decrypt:{step}", session.session_id),
        released_token: None,
        compute_units,
        updated_at_ms: private_inference_updated_at_ms(view),
    };
    Ok(SoracloudPrivateInferenceExecutionResult {
        status: SoraPrivateInferenceSessionStatusV1::AwaitingDecryption,
        receipt_root,
        xor_cost_nanos,
        result_commitment: private_inference_result_commitment(
            &session.session_id,
            &request.action,
            SoraPrivateInferenceSessionStatusV1::AwaitingDecryption,
            receipt_root,
            xor_cost_nanos,
            &checkpoint,
        ),
        checkpoint,
    })
}

fn execute_private_inference_release(
    view: &StateView<'_>,
    bundle: &SoraUploadedModelBundleV1,
    session: &SoraPrivateInferenceSessionV1,
    decrypt_request_id: &str,
    request: &SoracloudPrivateInferenceExecutionRequest,
) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError> {
    if session.status != SoraPrivateInferenceSessionStatusV1::AwaitingDecryption {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` is {:?}, expected AwaitingDecryption before output release",
                session.session_id, session.status
            ),
        ));
    }
    let Some(pending_checkpoint) = latest_private_inference_checkpoint(view, &session.session_id)
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` does not have an awaiting-decryption checkpoint",
                session.session_id
            ),
        ));
    };
    if pending_checkpoint.decrypt_request_id != decrypt_request_id {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` checkpoint decrypt_request_id `{}` does not match requested `{}`",
                session.session_id, pending_checkpoint.decrypt_request_id, decrypt_request_id
            ),
        ));
    }
    if pending_checkpoint.released_token.is_some() {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "private session `{}` checkpoint step {} already released output",
                session.session_id, pending_checkpoint.step
            ),
        ));
    }

    let released_token =
        private_inference_released_token(session, &pending_checkpoint, request.request_commitment);
    let receipt_hash = private_inference_receipt_hash(
        &session.session_id,
        pending_checkpoint.step,
        "release",
        request.request_commitment,
        pending_checkpoint.ciphertext_state_root,
        Some(released_token.as_str()),
    );
    let xor_cost_nanos = session
        .xor_cost_nanos
        .saturating_add(bundle.pricing_policy.decrypt_release_xor_nanos);
    let receipt_root = private_inference_receipt_root(
        &session.session_id,
        pending_checkpoint.step,
        SoraPrivateInferenceSessionStatusV1::Completed,
        xor_cost_nanos,
        receipt_hash,
    );
    let checkpoint = SoraPrivateInferenceCheckpointV1 {
        schema_version: iroha_data_model::soracloud::SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
        session_id: session.session_id.clone(),
        step: pending_checkpoint.step,
        ciphertext_state_root: pending_checkpoint.ciphertext_state_root,
        receipt_hash,
        decrypt_request_id: pending_checkpoint.decrypt_request_id,
        released_token: Some(released_token),
        compute_units: pending_checkpoint.compute_units,
        updated_at_ms: private_inference_updated_at_ms(view),
    };
    Ok(SoracloudPrivateInferenceExecutionResult {
        status: SoraPrivateInferenceSessionStatusV1::Completed,
        receipt_root,
        xor_cost_nanos,
        result_commitment: private_inference_result_commitment(
            &session.session_id,
            &request.action,
            SoraPrivateInferenceSessionStatusV1::Completed,
            receipt_root,
            xor_cost_nanos,
            &checkpoint,
        ),
        checkpoint,
    })
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
                .any(|allowed| allowed.eq_ignore_ascii_case(&route.host))
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
    let hf_execution_host =
        if let Some(binding) = soracloud_hf_generated_source_binding(&bundle) {
            resolve_local_hf_execution_host(view, request.service_name.as_str(), &binding.source_id, config)?
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
                && placement.status
                    != iroha_data_model::soracloud::SoraHfPlacementStatusV1::Retired
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
    let Some(placement) = resolve_active_hf_placement_for_service(view, service_name, source_id)? else {
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

fn parse_query_params(query: Option<&str>) -> BTreeMap<String, String> {
    query
        .unwrap_or_default()
        .split('&')
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=').unwrap_or((entry, ""));
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
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

fn build_runtime_snapshot(
    view: &StateView<'_>,
    bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    state_dir: &Path,
    artifacts_root: PathBuf,
) -> eyre::Result<SoracloudRuntimeSnapshot> {
    let mut services = BTreeMap::new();
    let world = view.world();

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
            ensure_ivm_runtime(bundle.container.runtime, &service_name, &service_version)
                .map_err(eyre::Report::msg)?;
            let is_runtime_active = runtime_state
                .as_ref()
                .is_some_and(|state| state.active_service_version == service_version);
            let service_dir = state_dir
                .join("services")
                .join(sanitize_path_component(&service_name))
                .join(sanitize_path_component(&service_version));
            let bundle_cache_path =
                artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
            let active_runtime_state = runtime_state
                .as_ref()
                .filter(|state| state.active_service_version == service_version);
            let artifact_plans = build_artifact_plans(bundle, &artifacts_root);
            let hydration_complete = artifact_plans
                .iter()
                .all(|artifact| artifact.available_locally);
            let plan = SoracloudRuntimeServicePlan {
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                role,
                traffic_percent,
                runtime: bundle.container.runtime,
                bundle_hash: bundle.container.bundle_hash.to_string(),
                bundle_path: bundle.container.bundle_path.clone(),
                entrypoint: bundle.container.entrypoint.clone(),
                bundle_cache_path: bundle_cache_path.display().to_string(),
                bundle_available_locally: bundle_cache_path.exists(),
                process_generation: is_runtime_active.then_some(deployment.process_generation),
                health_status: if hydration_complete {
                    active_runtime_state.map_or(SoraServiceHealthStatusV1::Hydrating, |state| {
                        state.health_status
                    })
                } else {
                    SoraServiceHealthStatusV1::Hydrating
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
                materialization_dir: service_dir.display().to_string(),
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
    runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1,
    service_name: &str,
    service_version: &str,
) -> Result<(), String> {
    match runtime {
        iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm => Ok(()),
        iroha_data_model::soracloud::SoraContainerRuntimeV1::NativeProcess => Err(format!(
            "service `{service_name}` revision `{service_version}` targets unsupported Soracloud runtime `NativeProcess`; v1 currently admits only `Ivm`"
        )),
    }
}

fn authoritative_mailbox_result_commitment(
    request: &SoracloudOrderedMailboxExecutionRequest,
    state_mutations: &[iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation],
    outbound_mailbox_messages: &[SoraServiceMailboxMessageV1],
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
    Hash::new(Encode::encode(&(
        "soracloud:runtime-result:v1",
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
        request.mailbox_message.to_handler.as_ref(),
        request.execution_sequence,
        mutation_fingerprints,
        outbound_fingerprints,
        runtime_state.clone(),
        journal_artifact_hash,
        checkpoint_artifact_hash,
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
    match error {
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

fn url_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split('/').next()?;
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority
        .strip_prefix('[')
        .and_then(|value| value.split_once(']').map(|(host, _)| host))
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(authority));
    if host.is_empty() { None } else { Some(host) }
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
            .name(format!(
                "hf-runner-{}",
                sanitize_path_component(&source_id)
            ))
            .spawn(move || {
                use io::BufRead as _;

                let mut reader = io::BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let payload = line
                                .trim_end_matches(['\r', '\n'])
                                .as_bytes()
                                .to_vec();
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
            AgentApartmentManifestV1, SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            SORA_HF_SOURCE_RECORD_VERSION_V1, SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1, SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            SORA_SERVICE_RUNTIME_STATE_VERSION_V1, SoraAgentArtifactAllowRuleV1,
            SoraAgentAutonomyRunRecordV1, SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1,
            SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraHfBackendFamilyV1,
            SoraHfModelFormatV1, SoraHfPlacementHostAssignmentV1,
            SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
            SoraHfPlacementStatusV1, SoraHfResourceProfileV1, SoraHfSharedLeaseMemberStatusV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1,
            SoraHfSourceRecordV1, SoraHfSourceStatusV1, SoraRolloutStageV1,
            SoraModelHostCapabilityRecordV1,
            SoraRouteVisibilityV1,
            SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHealthStatusV1,
            SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1, SoraServiceRuntimeStateV1,
        },
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, PinManifestRecord, PinPolicy, ReplicationOrderId,
            ReplicationOrderRecord, ReplicationOrderStatus,
        },
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
    use iroha_torii::sorafs::AdmissionRegistry;
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
    use sorafs_node::{NodeHandle, config::StorageConfig};

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
            uploaded_model_binding: None,
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
        }
    }

    fn soracloud_entrypoint(name: &str, entry_pc: u64) -> ivm::EmbeddedEntrypointDescriptor {
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

    fn simple_soracloud_contract_artifact(entrypoints: &[&str]) -> Vec<u8> {
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
        };
        let mut bytes = metadata.encode();
        bytes.extend_from_slice(&contract_interface.encode_section());
        bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        bytes
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
                let mut record = PinManifestRecord::new(
                    fixture.manifest_digest,
                    fixed_chunker_handle(),
                    [0; 32],
                    PinPolicy::default(),
                    (*ALICE_ID).clone(),
                    fixture.issued_epoch,
                    None,
                    None,
                    Metadata::default(),
                );
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
        let runtime = iroha_config::parameters::actual::SoracloudRuntime {
            state_dir,
            ..Default::default()
        };
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
            generated_hf_reconcile_attempts_ms: Arc::new(parking_lot::Mutex::new(
                BTreeMap::new(),
            )),
        }
    }

    #[derive(Default)]
    struct RecordingRuntimeMutationSink {
        instructions: parking_lot::Mutex<Vec<InstructionBox>>,
    }

    impl RecordingRuntimeMutationSink {
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
            format!("generated-hf-placement:{}", fixture.bundle.service.service_name).as_bytes(),
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
                PinPolicy::default(),
                (*ALICE_ID).clone(),
                1,
                None,
                None,
                Metadata::default(),
            );
            record.approve(1, None);
            pin_manifests.insert(digest, record);
        }
        pin_manifests.apply();
        block.commit()?;
        Ok(())
    }

    #[test]
    fn manager_config_uses_explicit_soracloud_runtime_settings() {
        let runtime = iroha_config::parameters::actual::SoracloudRuntime {
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
            native_process: iroha_config::parameters::actual::SoracloudRuntimeNativeProcess {
                max_concurrent_processes: std::num::NonZeroUsize::new(3)
                    .expect("nonzero concurrent processes"),
                cpu_millis: std::num::NonZeroU32::new(1_500).expect("nonzero cpu"),
                memory_bytes: std::num::NonZeroU64::new(65_536).expect("nonzero memory"),
                ephemeral_storage_bytes: std::num::NonZeroU64::new(131_072)
                    .expect("nonzero storage"),
                max_open_files: std::num::NonZeroU32::new(64).expect("nonzero files"),
                max_tasks: std::num::NonZeroU16::new(32).expect("nonzero tasks"),
                start_grace: Duration::from_secs(3),
                stop_grace: Duration::from_secs(5),
            },
            egress: iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: true,
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
        assert_eq!(manager.reconcile_interval, runtime.reconcile_interval);
        assert_eq!(manager.hydration_concurrency, runtime.hydration_concurrency);
        assert_eq!(manager.cache_budgets, runtime.cache_budgets);
        assert_eq!(manager.native_process, runtime.native_process);
        assert_eq!(manager.egress, runtime.egress);
        assert_eq!(manager.hf, runtime.hf);
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
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "persist-materialization");
        let deployment = sample_deployment_state(&bundle);
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
                    uploaded_model_binding: None,
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
            second_json.get("text").and_then(norito::json::Value::as_str),
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
        let current_heartbeat_expiry_ms =
            soracloud_runtime_observed_at_ms().saturating_add(20_000);
        insert_local_model_host_capability_fixture(
            &mut state,
            &ALICE_ID,
            local_peer_id,
            current_heartbeat_expiry_ms,
        );
        let config_json = br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"echo","prefix":"warm:"}}"#
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
        assert_eq!(reports[0].kind, SoraModelHostViolationKindV1::AdvertContradiction);
        assert_eq!(reports[0].placement_id, None);
        assert!(
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("does not match the authoritative model-host advert peer id"))
        );
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
            reports[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("failed to forward generated-HF proxy traffic"))
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
        let config_json = br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"explode"}}"#
            .to_vec();
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
        assert_eq!(first_error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(
            first_error
                .message
                .contains("unsupported _soracloud_fixture mode")
        );
        let second_error = handle
            .execute_local_read(build_request())
            .expect_err("failure should remain visible on repeated calls");
        assert_eq!(second_error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);

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
        let config_json = br#"{"model_type":"gpt2","_soracloud_fixture":{"mode":"explode"}}"#
            .to_vec();
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
    fn reconcile_once_rejects_unsupported_native_process_runtime() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::NativeProcess;
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
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let error = manager
            .reconcile_once()
            .expect_err("native-process revisions must be rejected during activation");
        assert!(
            error.to_string().contains("admits only `Ivm`"),
            "unexpected reconcile error: {error:?}"
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
    fn reconcile_once_skips_uncommitted_sorafs_artifacts_and_keeps_service_hydrating() -> Result<()>
    {
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
        assert!(!plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Hydrating);
        assert!(
            plan.artifacts
                .iter()
                .all(|artifact| !artifact.available_locally)
        );
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
                        bundle_hash: Hash::prehashed([0x33; Hash::LENGTH]).to_string(),
                        bundle_path: "sorafs://restored.bundle".to_string(),
                        entrypoint: "main".to_string(),
                        bundle_cache_path: temp_dir
                            .path()
                            .join("artifacts/restored_bundle")
                            .display()
                            .to_string(),
                        bundle_available_locally: true,
                        process_generation: Some(9),
                        health_status: SoraServiceHealthStatusV1::Healthy,
                        load_factor_bps: 250,
                        reported_pending_mailbox_messages: 2,
                        authoritative_pending_mailbox_messages: 2,
                        rollout_handle: None,
                        materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0")
                            .display()
                            .to_string(),
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

        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::InvalidRequest);
        assert!(error.message.contains("local-read route is not public"));
        Ok(())
    }

    #[test]
    fn execute_local_read_returns_query_metadata_and_audit_receipt() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
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
                        payload_bytes: std::num::NonZeroU64::new(64).expect("nonzero"),
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
                request_query: Some("binding=session_store".to_owned()),
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"query-request"),
            })
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        let decoded: LocalQueryResponse = norito::json::from_slice(&response.response_bytes)?;
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].binding_name, "session_store");
        assert_eq!(decoded.entries[0].state_key, "/state/session/alice");
        assert_eq!(
            response.certified_by,
            SoraCertifiedResponsePolicyV1::AuditReceipt
        );
        assert!(response.runtime_receipt.is_some());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].state_key.as_deref(),
            Some("/state/session/alice")
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
        assert!(matches!(
            public_host.require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET),
            Err(VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_SECRET
            })
        ));
        assert!(matches!(
            public_host.require_private_runtime(SYSCALL_SORACLOUD_READ_CREDENTIAL),
            Err(VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_CREDENTIAL
            })
        ));
        Ok(())
    }

    #[test]
    fn ivm_host_egress_fetch_enforces_allowlist_rate_and_byte_limits() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec!["127.0.0.1".to_owned()]);
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
        let body = b"hello-egress".to_vec();
        let expected_hash = Hash::new(&body);
        let (url, server) = spawn_http_fixture(body.clone())?;
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
        let (url, server) = spawn_http_fixture(b"too-large".to_vec())?;
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
            uploaded_model_binding: None,
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
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
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
            uploaded_model_binding: None,
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
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
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
