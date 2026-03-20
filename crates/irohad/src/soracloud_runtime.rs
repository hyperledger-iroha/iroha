//! Embedded Soracloud runtime-manager reconciliation for `irohad`.
//!
//! This subsystem does not execute workloads yet. Its job in this first slice
//! is to continuously project authoritative Soracloud world state into a
//! node-local materialization plan so later hydration, IVM hosting, and
//! deterministic `NativeProcess` supervision can attach to a concrete runtime
//! manager instead of ad hoc Torii-local state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use eyre::WrapErr;
use iroha_core::state::{State, StateView};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::BlockHeader,
    prelude::Name,
    soracloud::{
        AgentApartmentManifestV1, SoraAgentApartmentRecordV1, SoraAgentRuntimeStatusV1,
        SoraArtifactKindV1, SoraDeploymentBundleV1, SoraServiceDeploymentStateV1,
        SoraServiceHealthStatusV1, SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1,
        SoraTrainingJobRecordV1, SoraContainerRuntimeV1,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use mv::storage::StorageReadOnly;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::JsonSerialize as _,
};
use parking_lot::RwLock;
use tokio::task::JoinHandle;

const DEFAULT_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const RUNTIME_SNAPSHOT_SCHEMA_VERSION: u16 = 1;

/// Distinguishes the local runtime role of a materialized service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
enum SoracloudRuntimeRevisionRole {
    /// The currently active deployment revision.
    Active,
    /// A canary candidate revision that must be materialized during rollout.
    CanaryCandidate,
}

/// Node-local mailbox materialization metadata for a handler.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub(crate) struct SoracloudRuntimeMailboxPlan {
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
pub(crate) struct SoracloudRuntimeArtifactPlan {
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
pub(crate) struct SoracloudRuntimeServicePlan {
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
pub(crate) struct SoracloudRuntimeApartmentPlan {
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
pub(crate) struct SoracloudRuntimeSnapshot {
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
            schema_version: RUNTIME_SNAPSHOT_SCHEMA_VERSION,
            observed_height: 0,
            observed_block_hash: None,
            services: BTreeMap::new(),
            apartments: BTreeMap::new(),
        }
    }
}

/// Runtime-manager configuration derived from node storage settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SoracloudRuntimeManagerConfig {
    /// Root directory for local runtime materialization state.
    pub state_dir: PathBuf,
    /// Reconciliation cadence against authoritative state.
    pub reconcile_interval: Duration,
}

impl SoracloudRuntimeManagerConfig {
    /// Build a runtime-manager configuration from the node configuration.
    #[must_use]
    pub fn from_node_config(config: &iroha_config::parameters::actual::Root) -> Self {
        // TODO: Promote this into explicit `iroha_config` parameters once the
        // embedded runtime grows real hydration/execution workers and needs
        // user-facing tuning beyond the current bootstrap materialization layer.
        Self {
            state_dir: config.torii.data_dir.join("soracloud_runtime"),
            reconcile_interval: DEFAULT_RECONCILE_INTERVAL,
        }
    }
}

/// Read-only handle to the embedded Soracloud runtime manager.
#[derive(Clone)]
pub struct SoracloudRuntimeManagerHandle {
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    state_dir: Arc<PathBuf>,
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

    fn services_root(&self) -> PathBuf {
        self.config.state_dir.join("services")
    }

    fn apartments_root(&self) -> PathBuf {
        self.config.state_dir.join("apartments")
    }

    fn artifacts_root(&self) -> PathBuf {
        self.config.state_dir.join("artifacts")
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

fn build_runtime_snapshot(
    view: &StateView<'_>,
    bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    state_dir: &Path,
    artifacts_root: PathBuf,
) -> eyre::Result<SoracloudRuntimeSnapshot> {
    let mut services = BTreeMap::new();
    let world = view.world();

    for (service_name, deployment) in world.soracloud_service_deployments().iter() {
        let service_name = service_name.to_string();
        let versions = collect_active_versions(deployment);
        let runtime_state = world.soracloud_service_runtime().get(service_name.parse::<Name>().as_ref().unwrap_or_else(|_| unreachable!()));
        let authoritative_pending = authoritative_mailbox_counts(world.soracloud_mailbox_messages());

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
        schema_version: RUNTIME_SNAPSHOT_SCHEMA_VERSION,
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
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for (_, message) in messages.iter() {
        let entry = counts.entry(message.to_service.to_string()).or_insert(0u32);
        *entry = entry.saturating_add(1);
    }
    counts
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
    let payload = norito::json::to_string(value)
        .map_err(|error| io::Error::other(format!("serialize json: {error}")))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, payload)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the embedded Soracloud runtime manager.

    use super::*;
    use std::{num::{NonZeroU16, NonZeroU64}, sync::Arc};

    use eyre::Result;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::{
        prelude::Name,
        soracloud::{
            AgentApartmentManifestV1, SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            SoraAgentPersistentStateV1, SoraAgentRuntimeStatusV1, SoraCertifiedResponsePolicyV1,
            SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraServiceDeploymentStateV1,
            SoraServiceHealthStatusV1, SoraServiceRuntimeStateV1,
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
        let kura = Arc::new(Kura::blank_kura_for_testing());
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
                keys: BTreeMap::new(),
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

    #[test]
    fn reconcile_once_persists_active_service_and_apartment_materializations() -> Result<()> {
        let state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let apartment = sample_agent_record()?;
        {
            let world = state.world();
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
                .insert(apartment.manifest.apartment_name.clone(), apartment);
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            SoracloudRuntimeManagerConfig {
                state_dir: temp_dir.path().to_path_buf(),
                reconcile_interval: Duration::from_secs(60),
            },
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
        let state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        {
            let world = state.world();
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
            SoracloudRuntimeManagerConfig {
                state_dir: temp_dir.path().to_path_buf(),
                reconcile_interval: Duration::from_secs(60),
            },
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
}
