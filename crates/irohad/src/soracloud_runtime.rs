//! Embedded Soracloud runtime-manager reconciliation for `irohad`.
//!
//! This subsystem continuously projects authoritative Soracloud world state
//! into a node-local materialization plan and now serves deterministic local
//! reads/apartment observations directly from the committed snapshot plus the
//! hydrated artifact cache. Soracloud runtime v1 is currently `Ivm`-only;
//! `NativeProcess` deployments are rejected during admission and runtime
//! activation.
//!
//! TODO: Replace the synthetic ordered-mailbox executor with the real IVM
//! Soracloud host surface. Authoritative mailbox payload bytes are now
//! available, but the VM/ABI cutover is still pending.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use eyre::WrapErr;
use iroha_core::soracloud_runtime::{
    SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
    SoracloudLocalReadRequest, SoracloudLocalReadResponse,
    SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
    SoracloudRuntime, SoracloudRuntimeApartmentPlan, SoracloudRuntimeArtifactPlan,
    SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
    SoracloudRuntimeMailboxPlan, SoracloudRuntimeReadHandle, SoracloudRuntimeRevisionRole,
    SoracloudRuntimeServicePlan, SoracloudRuntimeSnapshot,
};
use iroha_core::state::{State, StateView, WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    Encode,
    name::Name,
    soracloud::{
        SoraAgentApartmentRecordV1, SoraArtifactKindV1, SoraCertifiedResponsePolicyV1,
        SoraDeploymentBundleV1, SoraRuntimeReceiptV1, SoraServiceDeploymentStateV1,
        SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceHealthStatusV1,
        SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1, SoraServiceStateEntryV1,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use mv::storage::StorageReadOnly;
use parking_lot::RwLock;
use tokio::task::JoinHandle;

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
}

impl SoracloudRuntimeManagerConfig {
    /// Build a runtime-manager configuration from the parsed Soracloud runtime settings.
    #[must_use]
    pub fn from_runtime_config(config: &iroha_config::parameters::actual::SoracloudRuntime) -> Self {
        Self {
            state_dir: config.state_dir.clone(),
            reconcile_interval: config.reconcile_interval,
            hydration_concurrency: config.hydration_concurrency,
            cache_budgets: config.cache_budgets.clone(),
            native_process: config.native_process.clone(),
            egress: config.egress.clone(),
        }
    }
}

/// Executable handle to the embedded Soracloud runtime manager.
#[derive(Clone)]
pub struct SoracloudRuntimeManagerHandle {
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    state_dir: Arc<PathBuf>,
    state: Arc<State>,
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
}

impl SoracloudRuntimeReadHandle for SoracloudRuntimeManagerHandle {
    fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        SoracloudRuntimeManagerHandle::snapshot(self)
    }

    fn state_dir(&self) -> PathBuf {
        SoracloudRuntimeManagerHandle::state_dir(self)
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
        let context = resolve_local_read_context(&view, &request)?;

        match request.handler_class {
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset => {
                execute_asset_local_read(&request, &context, self.state_dir.as_ref())
            }
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query => {
                execute_query_local_read(&view, &request, &context)
            }
        }
    }

    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        ensure_ivm_runtime(
            request.bundle.container.runtime,
            request.deployment.service_name.as_ref(),
            &request.deployment.current_service_version,
        )
        .map_err(|message| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                message,
            )
        })?;
        let handler_class = request
            .handler
            .as_ref()
            .map(|handler| handler.class)
            .unwrap_or(SoraServiceHandlerClassV1::Update);
        let failure = request.handler.is_none();
        let health_status = if failure {
            SoraServiceHealthStatusV1::Degraded
        } else {
            SoraServiceHealthStatusV1::Healthy
        };
        let outcome_label = if failure { "missing_handler" } else { "synthetic_ok" };
        let result_commitment = mailbox_result_commitment(
            request.mailbox_message.message_id,
            request.deployment.service_name.as_ref(),
            &request.deployment.current_service_version,
            request.mailbox_message.to_handler.as_ref(),
            outcome_label,
            request.execution_sequence,
        );
        let receipt_id = mailbox_receipt_id(
            request.mailbox_message.message_id,
            request.deployment.service_name.as_ref(),
            &request.deployment.current_service_version,
            request.execution_sequence,
            outcome_label,
        );
        let runtime_state = Some(updated_runtime_state(
            request.runtime_state.clone(),
            &request,
            health_status,
        ));

        Ok(SoracloudOrderedMailboxExecutionResult {
            state_mutations: Vec::new(),
            outbound_mailbox_messages: Vec::new(),
            runtime_state,
            runtime_receipt: SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id,
                service_name: request.deployment.service_name,
                service_version: request.deployment.current_service_version,
                handler_name: request.mailbox_message.to_handler.clone(),
                handler_class,
                request_commitment: request.mailbox_message.payload_commitment,
                result_commitment,
                certified_by: SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: request.execution_sequence,
                mailbox_message_id: Some(request.mailbox_message.message_id),
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
            },
        })
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

#[derive(Clone)]
struct ResolvedLocalReadContext {
    deployment: SoraServiceDeploymentStateV1,
    bundle: SoraDeploymentBundleV1,
    handler: SoraServiceHandlerV1,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize
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
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize
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

/// Embedded `irohad` Soracloud runtime-manager actor.
pub(crate) struct SoracloudRuntimeManager {
    config: SoracloudRuntimeManagerConfig,
    state: Arc<State>,
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
}

impl SoracloudRuntimeManager {
    /// Construct the runtime manager for the supplied node state.
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, state: Arc<State>) -> Self {
        Self {
            config,
            state,
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
        }
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
        if let Err(error) = manager.reconcile_once() {
            iroha_logger::warn!(
                ?error,
                state_dir = %manager.config.state_dir.display(),
                "initial Soracloud runtime-manager reconciliation failed"
            );
        }
        let handle = SoracloudRuntimeManagerHandle {
            snapshot: Arc::clone(&manager.snapshot),
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state: Arc::clone(&manager.state),
        };
        let task = Arc::clone(&manager).spawn_reconcile_task(shutdown_signal);
        (
            handle,
            Child::new(task, OnShutdown::Wait(Duration::from_secs(1))),
        )
    }

    fn spawn_reconcile_task(self: Arc<Self>, shutdown_signal: ShutdownSignal) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(self.config.reconcile_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(error) = self.reconcile_once() {
                            iroha_logger::warn!(
                                ?error,
                                state_dir = %self.config.state_dir.display(),
                                "Soracloud runtime-manager reconciliation failed"
                            );
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

        let view = self.state.view();
        let bundle_registry = collect_service_revision_registry(&view);
        let snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;

        self.write_service_materializations(&snapshot, &bundle_registry)?;
        self.write_apartment_materializations(&snapshot, &view)?;
        self.prune_stale_service_materializations(&snapshot)?;
        self.prune_stale_apartment_materializations(&snapshot)?;
        write_json_atomic(
            &self.config.state_dir.join("runtime_snapshot.json"),
            &snapshot,
        )?;
        *self.snapshot.write() = snapshot;
        Ok(())
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
                write_json_atomic(&apartment_root.join("apartment_manifest.json"), &record.manifest)?;
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
}

fn execute_asset_local_read(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let Some(artifact) = resolve_asset_artifact(&context.bundle, &context.handler, &request.handler_path)
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
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
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
    let bindings = rows
        .iter()
        .map(state_entry_binding)
        .collect::<Vec<_>>();
    let result_commitment = Hash::new(&response_bytes);
    let runtime_receipt = match context.handler.certified_response {
        SoraCertifiedResponsePolicyV1::AuditReceipt => Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
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

fn validate_local_runtime_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudLocalReadRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height || request.observed_block_hash != committed_block_hash
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
    if request.observed_height != committed_height || request.observed_block_hash != committed_block_hash
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

fn resolve_local_read_context(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
) -> Result<ResolvedLocalReadContext, SoracloudRuntimeExecutionError> {
    let service_id: Name = request
        .service_name
        .parse()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("invalid Soracloud service name `{}`: {error}", request.service_name),
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
                request.service_name,
                deployment.current_service_version,
                request.service_version
            ),
        ));
    }
    let Some(bundle) = view
        .world()
        .soracloud_service_revisions()
        .get(&(request.service_name.clone(), request.service_version.clone()))
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

    Ok(ResolvedLocalReadContext {
        deployment,
        bundle,
        handler,
    })
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
            format!("read hydrated Soracloud artifact cache {}: {error}", cache_path.display()),
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
) -> SoraRuntimeReceiptV1 {
    let emitted_sequence = next_authoritative_observation_sequence_from_view(deployment.service_name.as_ref(), request.observed_height);
    SoraRuntimeReceiptV1 {
        schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: Hash::new(
            Encode::encode(&(
                "soracloud:local-read",
                deployment.service_name.as_ref(),
                deployment.current_service_version.as_str(),
                handler.handler_name.as_ref(),
                request.request_commitment,
                result_commitment,
                certified_by,
            )),
        ),
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
    }
}

fn next_authoritative_observation_sequence_from_view(_service_name: &str, observed_height: u64) -> u64 {
    observed_height.max(1)
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
            let bundle_cache_path = artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
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
                health_status: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
                    .map_or(SoraServiceHealthStatusV1::Hydrating, |state| state.health_status),
                load_factor_bps: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
                    .map_or(0, |state| state.load_factor_bps),
                reported_pending_mailbox_messages: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
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
                        handler.mailbox.as_ref().map(|mailbox| SoracloudRuntimeMailboxPlan {
                            handler_name: handler.handler_name.to_string(),
                            queue_name: mailbox.queue_name.to_string(),
                            max_pending_messages: mailbox.max_pending_messages.get(),
                            max_message_bytes: mailbox.max_message_bytes.get(),
                            retention_blocks: mailbox.retention_blocks.get(),
                        })
                    })
                    .collect(),
                artifacts: build_artifact_plans(bundle, &artifacts_root),
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

    Ok(SoracloudRuntimeSnapshot {
        schema_version: SoracloudRuntimeSnapshot::default().schema_version,
        observed_height: u64::try_from(view.height()).unwrap_or(u64::MAX),
        observed_block_hash: view.latest_block_hash().map(|hash| hash.to_string()),
        services,
        apartments,
    })
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
            ((service_name.clone(), service_version.clone()), bundle.clone())
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

fn mailbox_result_commitment(
    message_id: Hash,
    service_name: &str,
    service_version: &str,
    handler_name: &str,
    outcome_label: &str,
    execution_sequence: u64,
) -> Hash {
    Hash::new(
        format!(
            "soracloud:runtime-result:{message_id}:{service_name}:{service_version}:{handler_name}:{outcome_label}:{execution_sequence}"
        )
        .as_bytes(),
    )
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

fn updated_runtime_state(
    runtime_state: Option<iroha_data_model::soracloud::SoraServiceRuntimeStateV1>,
    request: &SoracloudOrderedMailboxExecutionRequest,
    health_status: SoraServiceHealthStatusV1,
) -> iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
    let mut runtime_state =
        runtime_state.unwrap_or_else(|| synthetic_runtime_state(request, health_status));
    runtime_state.health_status = health_status;
    runtime_state.pending_mailbox_message_count = request
        .authoritative_pending_mailbox_messages
        .saturating_sub(1);
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

fn hash_cache_name(hash: Hash) -> String {
    sanitize_path_component(&hash.to_string())
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
        for version_entry in
            fs::read_dir(&service_path).wrap_err_with(|| format!("read {}", service_path.display()))?
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
    use std::sync::Arc;

    use eyre::Result;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::{
        soracloud::{
            AgentApartmentManifestV1, SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1, SoraContainerRuntimeV1,
            SoraDeploymentBundleV1, SoraServiceDeploymentStateV1, SoraServiceHealthStatusV1,
            SoraServiceRuntimeStateV1,
        },
    };

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
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state,
        }
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
        };

        let manager = SoracloudRuntimeManagerConfig::from_runtime_config(&runtime);
        assert_eq!(manager.state_dir, runtime.state_dir);
        assert_eq!(manager.reconcile_interval, runtime.reconcile_interval);
        assert_eq!(manager.hydration_concurrency, runtime.hydration_concurrency);
        assert_eq!(manager.cache_budgets, runtime.cache_budgets);
        assert_eq!(manager.native_process, runtime.native_process);
        assert_eq!(manager.egress, runtime.egress);
    }

    #[test]
    fn reconcile_once_persists_active_service_and_apartment_materializations() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let apartment = sample_agent_record()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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
        assert!(temp_dir
            .path()
            .join("runtime_snapshot.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("services/web_portal/2026.02.0/runtime_plan.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("services/web_portal/2026.02.0/deployment_bundle.json")
            .exists());
        assert!(temp_dir.path().join("journals").exists());
        assert!(temp_dir.path().join("checkpoints").exists());
        assert!(temp_dir.path().join("secrets").exists());
        assert!(temp_dir
            .path()
            .join("apartments/ops_agent/runtime_plan.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("apartments/ops_agent/apartment_manifest.json")
            .exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_stale_materializations_and_reports_missing_bundle_cache() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), sample_runtime_state(&bundle));
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
        assert!(bundle_plan
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == SoraArtifactKindV1::Bundle && !artifact.available_locally));
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
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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

        let response = handle.execute_local_read(SoracloudLocalReadRequest {
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
        assert_eq!(response.certified_by, SoraCertifiedResponsePolicyV1::StateCommitment);
        assert!(response.runtime_receipt.is_none());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].artifact_hash,
            Some(bundle.service.artifacts[0].artifact_hash)
        );
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
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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
                        encryption: iroha_data_model::soracloud::SoraStateEncryptionV1::ClientCiphertext,
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

        let response = handle.execute_local_read(SoracloudLocalReadRequest {
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
        assert_eq!(response.certified_by, SoraCertifiedResponsePolicyV1::AuditReceipt);
        assert!(response.runtime_receipt.is_some());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].state_key.as_deref(),
            Some("/state/session/alice")
        );
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
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
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
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), sample_runtime_state(&bundle));
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
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment.manifest.apartment_name.to_string(), apartment.clone());
        }
        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let result = handle.execute_apartment(SoracloudApartmentExecutionRequest {
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
}
