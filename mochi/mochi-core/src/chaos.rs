//! Izanami-backed chaos helpers for the Mochi desktop shell.

use std::{
    fs,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use color_eyre::{Result as EyreResult, eyre::eyre};
use iroha_data_model::{block::SignedBlock, domain::DomainId, isi::InstructionBox};
use iroha_genesis::GenesisBlock;
use iroha_test_samples::ALICE_KEYPAIR;
use tokio::runtime::Handle;
use toml::{Table, Value};

use crate::{Supervisor, ToriiClient};
use izanami::faults::{
    CpuStressConfig, DiskSaturationConfig, FaultClient, FaultConfig, FaultPeer, FaultScenarioKind,
    NetworkLatencyConfig, NetworkPartitionConfig, apply_fault_scenario,
};

/// Named chaos presets surfaced in the Mochi desktop shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChaosPreset {
    /// Stop and restart the selected peer.
    PeerBounce,
    /// Temporarily exaggerate gossip delays for one peer.
    SlowGossip,
    /// Isolate one peer from the trusted roster for a short period.
    PartitionOne,
    /// Add transient CPU and disk pressure locally.
    ResourcePressure,
    /// Submit malformed payloads to Torii to emulate a noisy client.
    BadActor,
    /// Wipe one peer's storage and bring it back.
    WipeAndRejoin,
}

impl ChaosPreset {
    /// Human-readable preset label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PeerBounce => "Peer bounce",
            Self::SlowGossip => "Slow gossip",
            Self::PartitionOne => "Partition one",
            Self::ResourcePressure => "Resource pressure",
            Self::BadActor => "Bad actor",
            Self::WipeAndRejoin => "Wipe and rejoin",
        }
    }

    /// One-line explanation suitable for UI cards.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::PeerBounce => "Stop one peer, pause briefly, then restart it.",
            Self::SlowGossip => "Restart one peer with a slower gossip cadence for a moment.",
            Self::PartitionOne => "Restart one peer with an empty trusted-peer roster.",
            Self::ResourcePressure => "Run short CPU and disk pressure bursts locally.",
            Self::BadActor => "Send malformed payloads to Torii and watch rejections land.",
            Self::WipeAndRejoin => "Blow away one peer's local storage and let it rejoin.",
        }
    }

    /// Reasonable default duration for the preset.
    #[must_use]
    pub fn default_duration(self) -> Duration {
        match self {
            Self::PeerBounce => Duration::from_secs(4),
            Self::SlowGossip => Duration::from_secs(8),
            Self::PartitionOne => Duration::from_secs(8),
            Self::ResourcePressure => Duration::from_secs(10),
            Self::BadActor => Duration::from_secs(3),
            Self::WipeAndRejoin => Duration::from_secs(6),
        }
    }

    /// Ordered list for UI selectors.
    #[must_use]
    pub fn all() -> [Self; 6] {
        [
            Self::PeerBounce,
            Self::SlowGossip,
            Self::PartitionOne,
            Self::ResourcePressure,
            Self::BadActor,
            Self::WipeAndRejoin,
        ]
    }
}

/// User-selected parameters for a single chaos run.
#[derive(Debug, Clone)]
pub struct ChaosRunRequest {
    /// Preset to execute.
    pub preset: ChaosPreset,
    /// Alias of the target peer.
    pub peer_alias: String,
    /// Approximate effect duration for time-bound presets.
    pub duration: Duration,
    /// Deterministic seed forwarded to Izanami fault helpers.
    pub seed: u64,
}

/// Incremental progress update emitted while a chaos run is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChaosEvent {
    /// Human-readable progress line.
    pub message: String,
}

/// Final summary for a finished chaos run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChaosReport {
    /// Preset that ran.
    pub preset: ChaosPreset,
    /// Targeted peer alias.
    pub peer_alias: String,
    /// Wall-clock start time.
    pub started_at: SystemTime,
    /// Wall-clock finish time.
    pub finished_at: SystemTime,
    /// Whether cancellation was requested before the run completed.
    pub cancelled: bool,
    /// Ordered event log captured during execution.
    pub events: Vec<String>,
}

/// Result of a chaos run, including the restored supervisor state.
#[derive(Debug)]
pub struct ChaosRunResult {
    /// Supervisor returned to the UI after the run.
    pub supervisor: Supervisor,
    /// Final report built from the captured event log.
    pub report: ChaosReport,
    /// Optional error from the run.
    pub error: Option<ChaosError>,
}

/// Chaos runner failures.
#[derive(Debug, thiserror::Error)]
pub enum ChaosError {
    /// Requested peer alias is not part of the supervised topology.
    #[error("unknown peer `{alias}`")]
    UnknownPeer { alias: String },
    /// The current run was cancelled by the UI.
    #[error("chaos run cancelled")]
    Cancelled,
    /// Generic failure wrapper for UI presentation.
    #[error("{0}")]
    Message(String),
}

/// Run a preset against a supervisor and return the restored supervisor with a report.
pub fn run_chaos_preset<F>(
    supervisor: Supervisor,
    handle: &Handle,
    request: ChaosRunRequest,
    cancel: &AtomicBool,
    mut on_event: F,
) -> ChaosRunResult
where
    F: FnMut(ChaosEvent),
{
    let started_at = SystemTime::now();
    let mut event_log = Vec::new();
    let mut emit = |message: String| {
        event_log.push(message.clone());
        on_event(ChaosEvent { message });
    };

    emit(format!(
        "Starting {} against {}.",
        request.preset.label(),
        request.peer_alias
    ));

    let shared = Arc::new(Mutex::new(supervisor));
    let result = match request.preset {
        ChaosPreset::PeerBounce => run_peer_bounce(&shared, &request.peer_alias, cancel, &mut emit),
        ChaosPreset::SlowGossip => MochiFaultPeer::from_shared(&shared, &request.peer_alias)
            .and_then(|peer| {
                run_izanami_fault(
                    handle,
                    peer,
                    FaultScenarioKind::NetworkLatencySpike,
                    fault_config_for_latency(request.duration),
                    request.duration,
                    request.seed,
                    cancel,
                    &mut emit,
                )
            }),
        ChaosPreset::PartitionOne => MochiFaultPeer::from_shared(&shared, &request.peer_alias)
            .and_then(|peer| {
                run_izanami_fault(
                    handle,
                    peer,
                    FaultScenarioKind::NetworkPartition,
                    fault_config_for_partition(request.duration),
                    request.duration,
                    request.seed,
                    cancel,
                    &mut emit,
                )
            }),
        ChaosPreset::ResourcePressure => MochiFaultPeer::from_shared(&shared, &request.peer_alias)
            .and_then(|peer| {
                run_resource_pressure(
                    handle,
                    peer,
                    request.duration,
                    request.seed,
                    cancel,
                    &mut emit,
                )
            }),
        ChaosPreset::BadActor => MochiFaultPeer::from_shared(&shared, &request.peer_alias)
            .and_then(|peer| run_bad_actor(peer.client(), handle, cancel, &mut emit)),
        ChaosPreset::WipeAndRejoin => MochiFaultPeer::from_shared(&shared, &request.peer_alias)
            .and_then(|peer| {
                run_izanami_fault(
                    handle,
                    peer,
                    FaultScenarioKind::WipeStorage,
                    FaultConfig {
                        interval: Duration::from_secs(1)..=Duration::from_secs(1),
                        crash_restart: false,
                        wipe_storage: true,
                        spam_invalid_transactions: false,
                        network_latency: None,
                        network_partition: None,
                        cpu_stress: None,
                        disk_saturation: None,
                    },
                    request.duration,
                    request.seed,
                    cancel,
                    &mut emit,
                )
            }),
    };

    let cancelled = cancel.load(Ordering::Relaxed);
    match &result {
        Ok(()) if cancelled => emit("Chaos run cancelled after the current action.".to_owned()),
        Ok(()) => emit("Chaos run completed.".to_owned()),
        Err(ChaosError::Cancelled) => emit("Chaos run cancelled.".to_owned()),
        Err(err) => emit(format!("Chaos run failed: {err}")),
    }

    let supervisor = Arc::into_inner(shared)
        .expect("chaos peer handles should be dropped before recovering supervisor")
        .into_inner()
        .unwrap_or_else(|poison| poison.into_inner());

    let report = ChaosReport {
        preset: request.preset,
        peer_alias: request.peer_alias,
        started_at,
        finished_at: SystemTime::now(),
        cancelled: cancelled || matches!(result, Err(ChaosError::Cancelled)),
        events: event_log,
    };

    ChaosRunResult {
        supervisor,
        report,
        error: result.err(),
    }
}

#[derive(Clone)]
struct MochiFaultClient {
    client: ToriiClient,
}

impl FaultClient for MochiFaultClient {
    fn submit_instruction<I>(&self, _instruction: I) -> EyreResult<()>
    where
        I: Into<InstructionBox>,
    {
        Err(eyre!(
            "mochi fault client does not support direct instruction submission"
        ))
    }
}

#[derive(Clone)]
struct MochiFaultPeer {
    supervisor: Arc<Mutex<Supervisor>>,
    alias: String,
    storage_dir: PathBuf,
    trusted_peers_pop: Vec<Value>,
    client: MochiFaultClient,
}

impl MochiFaultPeer {
    fn from_shared(shared: &Arc<Mutex<Supervisor>>, alias: &str) -> Result<Self, ChaosError> {
        let guard = shared
            .lock()
            .map_err(|_| ChaosError::Message("supervisor mutex poisoned".to_owned()))?;
        let peer = guard
            .peers()
            .iter()
            .find(|peer| peer.alias() == alias)
            .ok_or_else(|| ChaosError::UnknownPeer {
                alias: alias.to_owned(),
            })?;
        let config = fs::read_to_string(peer.config_path())
            .map_err(|err| ChaosError::Message(format!("read peer config failed: {err}")))?;
        let table: Table = toml::from_str(&config)
            .map_err(|err| ChaosError::Message(format!("parse peer config failed: {err}")))?;
        let trusted_peers_pop = table
            .get("trusted_peers_pop")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                ChaosError::Message("peer config missing trusted_peers_pop roster".to_owned())
            })?;
        let peer_dir = peer
            .config_path()
            .parent()
            .ok_or_else(|| ChaosError::Message("peer config is missing a parent dir".to_owned()))?
            .to_path_buf();
        let client = guard.torii_client(alias).ok_or_else(|| {
            ChaosError::Message(format!("torii client unavailable for `{alias}`"))
        })?;
        drop(guard);
        Ok(Self {
            supervisor: shared.clone(),
            alias: alias.to_owned(),
            storage_dir: peer_dir.join("storage"),
            trusted_peers_pop,
            client: MochiFaultClient { client },
        })
    }
}

impl FaultPeer for MochiFaultPeer {
    type Client = MochiFaultClient;

    fn mnemonic(&self) -> &str {
        &self.alias
    }

    fn kura_store_dir(&self) -> PathBuf {
        self.storage_dir.clone()
    }

    fn client(&self) -> Self::Client {
        self.client.clone()
    }

    fn trusted_peers_pop_entries(&self) -> EyreResult<Vec<Value>> {
        Ok(self.trusted_peers_pop.clone())
    }

    fn shutdown(&self) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        let supervisor = self.supervisor.clone();
        let alias = self.alias.clone();
        Box::pin(async move {
            if let Ok(mut guard) = supervisor.lock() {
                let _ = guard.stop_peer(&alias);
            }
        })
    }

    fn restart_with_layers<'a>(
        &'a self,
        _config_layers: &'a Arc<Vec<Table>>,
        extra_layers: &'a [Table],
        _genesis: &'a Arc<GenesisBlock>,
    ) -> Pin<Box<dyn std::future::Future<Output = EyreResult<()>> + Send + 'a>> {
        let supervisor = self.supervisor.clone();
        let alias = self.alias.clone();
        let extra_layers = extra_layers.to_vec();
        Box::pin(async move {
            let mut guard = supervisor
                .lock()
                .map_err(|_| eyre!("supervisor mutex poisoned"))?;
            guard
                .restart_peer_with_extra_layers(&alias, &extra_layers)
                .map_err(|err| eyre!(err.to_string()))
        })
    }
}

fn run_peer_bounce<F>(
    shared: &Arc<Mutex<Supervisor>>,
    alias: &str,
    cancel: &AtomicBool,
    emit: &mut F,
) -> Result<(), ChaosError>
where
    F: FnMut(String),
{
    emit(format!("Stopping {}.", alias));
    {
        let mut guard = shared
            .lock()
            .map_err(|_| ChaosError::Message("supervisor mutex poisoned".to_owned()))?;
        let _ = guard.stop_peer(alias);
    }
    sleep_with_cancel(Duration::from_secs(2), cancel)?;
    emit(format!("Restarting {}.", alias));
    let mut guard = shared
        .lock()
        .map_err(|_| ChaosError::Message("supervisor mutex poisoned".to_owned()))?;
    guard
        .start_peer(alias)
        .map_err(|err| ChaosError::Message(err.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn run_izanami_fault<F>(
    handle: &Handle,
    peer: MochiFaultPeer,
    scenario: FaultScenarioKind,
    config: FaultConfig,
    duration: Duration,
    seed: u64,
    cancel: &AtomicBool,
    emit: &mut F,
) -> Result<(), ChaosError>
where
    F: FnMut(String),
{
    if cancel.load(Ordering::Relaxed) {
        return Err(ChaosError::Cancelled);
    }
    emit(format!("Applying {}.", scenario_label(scenario)));
    let config_layers = Arc::new(Vec::<Table>::new());
    let genesis = Arc::new(GenesisBlock(SignedBlock::genesis(
        Vec::new(),
        ALICE_KEYPAIR.private_key(),
        None,
        None,
    )));
    let base_domain = DomainId::try_new("wonderland", "universal").expect("static domain id");
    let deadline = Instant::now() + duration.max(Duration::from_secs(1));
    handle
        .block_on(async {
            apply_fault_scenario(
                scenario,
                &peer,
                &config,
                &config_layers,
                &genesis,
                &base_domain,
                deadline,
                seed,
            )
            .await
        })
        .map_err(|err| ChaosError::Message(err.to_string()))
}

fn run_resource_pressure<F>(
    handle: &Handle,
    peer: MochiFaultPeer,
    duration: Duration,
    seed: u64,
    cancel: &AtomicBool,
    emit: &mut F,
) -> Result<(), ChaosError>
where
    F: FnMut(String),
{
    let half = Duration::from_millis((duration.as_millis().max(2) / 2) as u64);
    run_izanami_fault(
        handle,
        peer.clone(),
        FaultScenarioKind::CpuStress,
        FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            crash_restart: false,
            wipe_storage: false,
            spam_invalid_transactions: false,
            network_latency: None,
            network_partition: None,
            cpu_stress: Some(CpuStressConfig {
                duration: half..=half,
                workers: 1..=2,
            }),
            disk_saturation: None,
        },
        half,
        seed,
        cancel,
        emit,
    )?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ChaosError::Cancelled);
    }
    run_izanami_fault(
        handle,
        peer,
        FaultScenarioKind::DiskSaturation,
        FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            crash_restart: false,
            wipe_storage: false,
            spam_invalid_transactions: false,
            network_latency: None,
            network_partition: None,
            cpu_stress: None,
            disk_saturation: Some(DiskSaturationConfig {
                duration: half..=half,
                bytes: 4 * 1_048_576..=6 * 1_048_576,
            }),
        },
        half,
        seed.saturating_add(1),
        cancel,
        emit,
    )
}

fn run_bad_actor<F>(
    client: MochiFaultClient,
    handle: &Handle,
    cancel: &AtomicBool,
    emit: &mut F,
) -> Result<(), ChaosError>
where
    F: FnMut(String),
{
    emit("Submitting malformed transaction payloads.".to_owned());
    for attempt in 0..3u8 {
        if cancel.load(Ordering::Relaxed) {
            return Err(ChaosError::Cancelled);
        }
        let payload = vec![0xff, attempt, 0x00, 0x7f];
        let outcome = handle.block_on(async { client.client.submit_transaction(&payload).await });
        match outcome {
            Ok(()) => emit(format!(
                "Malformed payload {attempt} was unexpectedly accepted."
            )),
            Err(err) => {
                let summary = err.summarize();
                let detail = summary
                    .detail
                    .as_deref()
                    .filter(|detail| !detail.is_empty())
                    .map(|detail| format!(" ({detail})"))
                    .unwrap_or_default();
                emit(format!(
                    "Malformed payload {attempt} rejected as expected: {}{}",
                    summary.message, detail
                ));
            }
        }
    }
    Ok(())
}

fn sleep_with_cancel(duration: Duration, cancel: &AtomicBool) -> Result<(), ChaosError> {
    let start = Instant::now();
    while start.elapsed() < duration {
        if cancel.load(Ordering::Relaxed) {
            return Err(ChaosError::Cancelled);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn fault_config_for_latency(duration: Duration) -> FaultConfig {
    FaultConfig {
        interval: Duration::from_secs(1)..=Duration::from_secs(1),
        crash_restart: false,
        wipe_storage: false,
        spam_invalid_transactions: false,
        network_latency: Some(NetworkLatencyConfig {
            duration: duration..=duration,
            gossip_delay: Duration::from_millis(1_200)..=Duration::from_millis(1_200),
        }),
        network_partition: None,
        cpu_stress: None,
        disk_saturation: None,
    }
}

fn fault_config_for_partition(duration: Duration) -> FaultConfig {
    FaultConfig {
        interval: Duration::from_secs(1)..=Duration::from_secs(1),
        crash_restart: false,
        wipe_storage: false,
        spam_invalid_transactions: false,
        network_latency: None,
        network_partition: Some(NetworkPartitionConfig {
            duration: duration..=duration,
        }),
        cpu_stress: None,
        disk_saturation: None,
    }
}

fn scenario_label(scenario: FaultScenarioKind) -> &'static str {
    match scenario {
        FaultScenarioKind::CrashRestart => "crash/restart fault",
        FaultScenarioKind::WipeStorage => "wipe/rejoin fault",
        FaultScenarioKind::SpamInvalidTransactions => "invalid-traffic fault",
        FaultScenarioKind::NetworkLatencySpike => "network latency fault",
        FaultScenarioKind::NetworkPartition => "network partition fault",
        FaultScenarioKind::CpuStress => "CPU stress fault",
        FaultScenarioKind::DiskSaturation => "disk saturation fault",
    }
}
