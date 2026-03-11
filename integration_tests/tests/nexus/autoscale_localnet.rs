#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet autoscale regression test for Nexus lane expansion/contraction.

use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{Level, isi::Log, nexus::LaneId},
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_test_network::{NetworkBuilder, NetworkPeer};

const TOTAL_PEERS: usize = 4;
const INITIAL_PROVISIONED_LANES: usize = 1;
const EXPANDED_PROVISIONED_LANES: usize = 2;
const PRIMARY_LANE_ID: u32 = 0;
const ELASTIC_LANE_ID: u32 = 1;
const LOAD_TX_COUNT: usize = 48;
const STRICT_CYCLE_LOAD_TX_COUNT: usize = 96;
const LANE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EXPANSION_PROBE_INTERVAL: Duration = Duration::from_millis(250);
const EXPANSION_TOP_UP_EVERY_HEARTBEATS: u64 = 4;
const EXPANSION_TOP_UP_TX_COUNT: usize = 16;
const EXPANSION_REINFORCE_EVERY_HEARTBEATS: u64 = 12;
const EXPANSION_REINFORCE_TX_COUNT: usize = 32;
const EXPANSION_STATUS_SIGNAL_GRACE: Duration = Duration::from_secs(8);
const EXPANSION_POST_STORAGE_STATUS_WINDOW: Duration = Duration::from_secs(8);
const EXPANSION_POST_STORAGE_TOP_UP_TX_COUNT: usize = 64;
const AUTOSCALE_COOLDOWN_CLEARANCE_BLOCK_DELTA: u64 = 2;
const AUTOSCALE_COOLDOWN_CLEARANCE_TIMEOUT: Duration = Duration::from_secs(45);
const CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);
const STRICT_CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);
const SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const STRICT_SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const AUTOSCALE_SOAK_DURATION: Duration = Duration::from_secs(30 * 60);
const AUTOSCALE_SOAK_CYCLE_RETRY_LIMIT: usize = 4;
const AUTOSCALE_MULTI_CYCLE_RETRY_LIMIT: usize = 2;
const AUTOSCALE_SOAK_DEFAULT_SEED: &str = "autoscale-localnet-soak-default";
const AUTOSCALE_SOAK_SEED_ENV: &str = "IROHA_AUTOSCALE_SOAK_SEED";
const AUTOSCALE_SOAK_ARTIFACT_DIR_ENV: &str = "IROHA_AUTOSCALE_SOAK_ARTIFACT_DIR";
const AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV: &str = "IROHA_AUTOSCALE_SOAK_FORCE_FAIL_CYCLE";
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const QUORUM_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const STATUS_SNAPSHOT_RETRY_LIMIT: u32 = 5;
const STATUS_SNAPSHOT_RETRY_BACKOFF: Duration = Duration::from_millis(250);
static LOAD_TX_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-out transition";
const AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-in transition";

fn autoscale_localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .with_pipeline_time(Duration::from_millis(300))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(["nexus", "autoscale", "enabled"], true)
                .write(["nexus", "autoscale", "min_lanes"], 1_i64)
                .write(["nexus", "autoscale", "max_lanes"], 2_i64)
                .write(["nexus", "autoscale", "target_block_ms"], 3000_i64)
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 0.1_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.05_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    0.6_f64,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    0.5_f64,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 2_i64)
                .write(["nexus", "autoscale", "scale_in_window_blocks"], 4_i64)
                .write(["nexus", "autoscale", "cooldown_blocks"], 1_i64)
                .write(["nexus", "autoscale", "per_lane_target_tps"], 40_i64);
        })
}

fn active_lane_segments(peer: &NetworkPeer) -> Result<Vec<String>> {
    let blocks_root = peer.kura_store_dir().join("blocks");
    if !blocks_root.exists() {
        return Ok(Vec::new());
    }

    let mut lanes = Vec::new();
    for entry in fs::read_dir(&blocks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with("lane_") {
            lanes.push(name);
        }
    }
    lanes.sort();
    Ok(lanes)
}

fn lane_snapshot(network: &sandbox::SerializedNetwork) -> Result<Vec<(usize, Vec<String>)>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            active_lane_segments(peer)
                .map(|lanes| (index, lanes))
                .map_err(|err| eyre!("read active lane segments on peer {index}: {err}"))
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default)]
struct ElasticLaneStorageStats {
    file_count: u64,
    total_bytes: u64,
    newest_modified_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct AutoscaleTransitionStats {
    scale_out_transitions: u64,
    scale_in_transitions: u64,
}

#[derive(Clone, Debug, Default)]
struct ExpandContractCycleOutcome {
    expansion_time_s: f64,
    contraction_time_s: f64,
    peers_with_scale_out_after_expansion: usize,
    peers_with_scale_in_after_expansion: usize,
    peers_with_scale_in_since_cycle_start: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct SoakTimingSummary {
    min_s: f64,
    avg_s: f64,
    max_s: f64,
}

impl SoakTimingSummary {
    fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let mut min_s = f64::INFINITY;
        let mut max_s = f64::NEG_INFINITY;
        let mut total = 0.0_f64;
        for sample in samples {
            min_s = min_s.min(*sample);
            max_s = max_s.max(*sample);
            total += sample;
        }
        Self {
            min_s,
            avg_s: total / samples.len() as f64,
            max_s,
        }
    }
}

#[derive(Clone, Debug)]
struct AutoscaleSoakRunSummary {
    test_name: String,
    started_at_unix_ms: u64,
    ended_at_unix_ms: u64,
    duration_s: f64,
    cycles_completed: usize,
    attempts_total: usize,
    attempt_failures_total: usize,
    retries_used_total: usize,
    max_attempt_used_in_any_cycle: usize,
    expansion_timing: SoakTimingSummary,
    contraction_timing: SoakTimingSummary,
    scale_out_quorum_misses_total: usize,
    scale_in_post_expansion_quorum_misses_total: usize,
    final_result: String,
    failure_cycle: Option<usize>,
    failure_reason: Option<String>,
}

impl AutoscaleSoakRunSummary {
    fn to_json_value(&self) -> norito::json::Value {
        let mut map = norito::json::Map::new();
        map.insert(
            "test_name".into(),
            norito::json::Value::from(self.test_name.clone()),
        );
        map.insert(
            "started_at_unix_ms".into(),
            norito::json::Value::from(self.started_at_unix_ms),
        );
        map.insert(
            "ended_at_unix_ms".into(),
            norito::json::Value::from(self.ended_at_unix_ms),
        );
        map.insert(
            "duration_s".into(),
            norito::json::Value::from(self.duration_s),
        );
        map.insert(
            "cycles_completed".into(),
            norito::json::Value::from(usize_to_u64(self.cycles_completed)),
        );
        map.insert(
            "attempts_total".into(),
            norito::json::Value::from(usize_to_u64(self.attempts_total)),
        );
        map.insert(
            "attempt_failures_total".into(),
            norito::json::Value::from(usize_to_u64(self.attempt_failures_total)),
        );
        map.insert(
            "retries_used_total".into(),
            norito::json::Value::from(usize_to_u64(self.retries_used_total)),
        );
        map.insert(
            "max_attempt_used_in_any_cycle".into(),
            norito::json::Value::from(usize_to_u64(self.max_attempt_used_in_any_cycle)),
        );
        map.insert(
            "expansion_time_s_min".into(),
            norito::json::Value::from(self.expansion_timing.min_s),
        );
        map.insert(
            "expansion_time_s_avg".into(),
            norito::json::Value::from(self.expansion_timing.avg_s),
        );
        map.insert(
            "expansion_time_s_max".into(),
            norito::json::Value::from(self.expansion_timing.max_s),
        );
        map.insert(
            "contraction_time_s_min".into(),
            norito::json::Value::from(self.contraction_timing.min_s),
        );
        map.insert(
            "contraction_time_s_avg".into(),
            norito::json::Value::from(self.contraction_timing.avg_s),
        );
        map.insert(
            "contraction_time_s_max".into(),
            norito::json::Value::from(self.contraction_timing.max_s),
        );
        map.insert(
            "scale_out_quorum_misses_total".into(),
            norito::json::Value::from(usize_to_u64(self.scale_out_quorum_misses_total)),
        );
        map.insert(
            "scale_in_post_expansion_quorum_misses_total".into(),
            norito::json::Value::from(usize_to_u64(
                self.scale_in_post_expansion_quorum_misses_total,
            )),
        );
        map.insert(
            "final_result".into(),
            norito::json::Value::from(self.final_result.clone()),
        );
        map.insert(
            "failure_cycle".into(),
            self.failure_cycle
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "failure_reason".into(),
            self.failure_reason
                .clone()
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        norito::json::Value::Object(map)
    }
}

#[derive(Clone, Debug)]
struct AutoscaleSoakCycleEvent {
    event_type: &'static str,
    timestamp_unix_ms: u64,
    elapsed_s: f64,
    cycle_index: usize,
    attempt: usize,
    quorum_required: usize,
    scale_out_transition_peers: Option<usize>,
    scale_in_peers_after_expansion: Option<usize>,
    scale_in_peers_since_cycle_start: Option<usize>,
    expansion_time_s: Option<f64>,
    contraction_time_s: Option<f64>,
    reason: Option<String>,
}

impl AutoscaleSoakCycleEvent {
    fn to_json_value(&self) -> norito::json::Value {
        let mut map = norito::json::Map::new();
        map.insert(
            "event_type".into(),
            norito::json::Value::from(self.event_type.to_owned()),
        );
        map.insert(
            "timestamp_unix_ms".into(),
            norito::json::Value::from(self.timestamp_unix_ms),
        );
        map.insert(
            "elapsed_s".into(),
            norito::json::Value::from(self.elapsed_s),
        );
        map.insert(
            "cycle_index".into(),
            norito::json::Value::from(usize_to_u64(self.cycle_index)),
        );
        map.insert(
            "attempt".into(),
            norito::json::Value::from(usize_to_u64(self.attempt)),
        );
        map.insert(
            "quorum_required".into(),
            norito::json::Value::from(usize_to_u64(self.quorum_required)),
        );
        map.insert(
            "scale_out_transition_peers".into(),
            self.scale_out_transition_peers
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "scale_in_peers_after_expansion".into(),
            self.scale_in_peers_after_expansion
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "scale_in_peers_since_cycle_start".into(),
            self.scale_in_peers_since_cycle_start
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "expansion_time_s".into(),
            self.expansion_time_s
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "contraction_time_s".into(),
            self.contraction_time_s
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "reason".into(),
            self.reason
                .clone()
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        norito::json::Value::Object(map)
    }
}

#[derive(Debug)]
struct AutoscaleSoakReporter {
    test_name: String,
    started_at_unix_ms: u64,
    started_at: Instant,
    summary_path: PathBuf,
    events_path: PathBuf,
    events_writer: BufWriter<fs::File>,
    cycles_completed: usize,
    attempts_total: usize,
    attempt_failures_total: usize,
    retries_used_total: usize,
    max_attempt_used_in_any_cycle: usize,
    expansion_times_s: Vec<f64>,
    contraction_times_s: Vec<f64>,
    scale_out_quorum_misses_total: usize,
    scale_in_post_expansion_quorum_misses_total: usize,
}

impl AutoscaleSoakReporter {
    fn from_paths(test_name: &str, summary_path: PathBuf, events_path: PathBuf) -> Result<Self> {
        if let Some(parent) = summary_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                eyre!(
                    "create autoscale soak summary dir {}: {err}",
                    parent.display()
                )
            })?;
        }
        if let Some(parent) = events_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                eyre!(
                    "create autoscale soak events dir {}: {err}",
                    parent.display()
                )
            })?;
        }
        let events_file = fs::File::create(&events_path).map_err(|err| {
            eyre!(
                "create autoscale soak event file {}: {err}",
                events_path.display()
            )
        })?;
        Ok(Self {
            test_name: test_name.to_owned(),
            started_at_unix_ms: current_unix_ms(),
            started_at: Instant::now(),
            summary_path,
            events_path,
            events_writer: BufWriter::new(events_file),
            cycles_completed: 0,
            attempts_total: 0,
            attempt_failures_total: 0,
            retries_used_total: 0,
            max_attempt_used_in_any_cycle: 0,
            expansion_times_s: Vec::new(),
            contraction_times_s: Vec::new(),
            scale_out_quorum_misses_total: 0,
            scale_in_post_expansion_quorum_misses_total: 0,
        })
    }

    fn new(network: &sandbox::SerializedNetwork, test_name: &str) -> Result<Self> {
        let artifact_root = autoscale_soak_artifact_root(network)?;
        fs::create_dir_all(&artifact_root).map_err(|err| {
            eyre!(
                "create autoscale soak artifact dir {}: {err}",
                artifact_root.display()
            )
        })?;
        let summary_path = artifact_root.join("autoscale_soak_summary.json");
        let events_path = artifact_root.join("autoscale_soak_events.jsonl");

        eprintln!(
            "[autoscale-localnet][soak] artifacts: summary={}, events={}",
            summary_path.display(),
            events_path.display()
        );

        Self::from_paths(test_name, summary_path, events_path)
    }

    #[cfg(test)]
    fn new_for_paths(summary_path: PathBuf, events_path: PathBuf, test_name: &str) -> Result<Self> {
        Self::from_paths(test_name, summary_path, events_path)
    }

    fn summary_path(&self) -> &Path {
        &self.summary_path
    }

    fn events_path(&self) -> &Path {
        &self.events_path
    }

    fn write_event(&mut self, mut event: AutoscaleSoakCycleEvent) -> Result<()> {
        event.timestamp_unix_ms = current_unix_ms();
        event.elapsed_s = self.started_at.elapsed().as_secs_f64();
        let line = norito::json::to_string(&event.to_json_value())
            .map_err(|err| eyre!("serialize autoscale soak event JSON: {err}"))?;
        writeln!(self.events_writer, "{line}").map_err(|err| {
            eyre!(
                "write autoscale soak event JSONL {}: {err}",
                self.events_path.display()
            )
        })?;
        self.events_writer.flush().map_err(|err| {
            eyre!(
                "flush autoscale soak event JSONL {}: {err}",
                self.events_path.display()
            )
        })?;
        Ok(())
    }

    fn record_cycle_start(&mut self, cycle_index: usize, quorum_required: usize) -> Result<()> {
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "cycle_start",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt: 0,
            quorum_required,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: None,
            expansion_time_s: None,
            contraction_time_s: None,
            reason: None,
        })
    }

    fn record_attempt_start(
        &mut self,
        cycle_index: usize,
        attempt: usize,
        quorum_required: usize,
        load_tx_count: usize,
    ) -> Result<()> {
        self.attempts_total = self.attempts_total.saturating_add(1);
        self.max_attempt_used_in_any_cycle = self.max_attempt_used_in_any_cycle.max(attempt);
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "attempt_start",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: None,
            expansion_time_s: None,
            contraction_time_s: None,
            reason: Some(format!("load_tx_count={load_tx_count}")),
        })
    }

    fn record_attempt_retry(
        &mut self,
        cycle_index: usize,
        attempt: usize,
        quorum_required: usize,
        next_attempt: usize,
        reason: &str,
    ) -> Result<()> {
        self.attempt_failures_total = self.attempt_failures_total.saturating_add(1);
        self.retries_used_total = self.retries_used_total.saturating_add(1);
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "attempt_retry",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: None,
            expansion_time_s: None,
            contraction_time_s: None,
            reason: Some(format!("next_attempt={next_attempt}; reason={reason}")),
        })
    }

    fn record_attempt_failure(
        &mut self,
        cycle_index: usize,
        attempt: usize,
        quorum_required: usize,
        reason: &str,
    ) -> Result<()> {
        self.attempt_failures_total = self.attempt_failures_total.saturating_add(1);
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "contraction_result",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: None,
            expansion_time_s: None,
            contraction_time_s: None,
            reason: Some(format!("attempt_failed={reason}")),
        })
    }

    fn record_cycle_success(
        &mut self,
        cycle_index: usize,
        attempt: usize,
        quorum_required: usize,
        cycle_outcome: &ExpandContractCycleOutcome,
    ) -> Result<()> {
        self.cycles_completed = self.cycles_completed.saturating_add(1);
        self.expansion_times_s.push(cycle_outcome.expansion_time_s);
        self.contraction_times_s
            .push(cycle_outcome.contraction_time_s);

        if cycle_outcome.peers_with_scale_out_after_expansion < quorum_required {
            self.scale_out_quorum_misses_total =
                self.scale_out_quorum_misses_total.saturating_add(1);
        }
        if cycle_outcome.peers_with_scale_in_after_expansion < quorum_required {
            self.scale_in_post_expansion_quorum_misses_total = self
                .scale_in_post_expansion_quorum_misses_total
                .saturating_add(1);
        }

        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "expansion_result",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: Some(cycle_outcome.peers_with_scale_out_after_expansion),
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: Some(
                cycle_outcome.peers_with_scale_in_since_cycle_start,
            ),
            expansion_time_s: Some(cycle_outcome.expansion_time_s),
            contraction_time_s: None,
            reason: None,
        })?;
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "contraction_result",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: Some(cycle_outcome.peers_with_scale_in_after_expansion),
            scale_in_peers_since_cycle_start: Some(
                cycle_outcome.peers_with_scale_in_since_cycle_start,
            ),
            expansion_time_s: None,
            contraction_time_s: Some(cycle_outcome.contraction_time_s),
            reason: None,
        })?;
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "cycle_complete",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index,
            attempt,
            quorum_required,
            scale_out_transition_peers: Some(cycle_outcome.peers_with_scale_out_after_expansion),
            scale_in_peers_after_expansion: Some(cycle_outcome.peers_with_scale_in_after_expansion),
            scale_in_peers_since_cycle_start: Some(
                cycle_outcome.peers_with_scale_in_since_cycle_start,
            ),
            expansion_time_s: Some(cycle_outcome.expansion_time_s),
            contraction_time_s: Some(cycle_outcome.contraction_time_s),
            reason: None,
        })
    }

    fn finalize(
        mut self,
        final_result: &str,
        failure_cycle: Option<usize>,
        failure_reason: Option<String>,
    ) -> Result<()> {
        self.write_event(AutoscaleSoakCycleEvent {
            event_type: "run_complete",
            timestamp_unix_ms: 0,
            elapsed_s: 0.0,
            cycle_index: failure_cycle.unwrap_or(self.cycles_completed),
            attempt: self.max_attempt_used_in_any_cycle,
            quorum_required: 0,
            scale_out_transition_peers: None,
            scale_in_peers_after_expansion: None,
            scale_in_peers_since_cycle_start: None,
            expansion_time_s: None,
            contraction_time_s: None,
            reason: Some(
                failure_reason
                    .clone()
                    .map(|reason| format!("result={final_result}; reason={reason}"))
                    .unwrap_or_else(|| format!("result={final_result}")),
            ),
        })?;
        self.events_writer.flush().map_err(|err| {
            eyre!(
                "flush autoscale soak event JSONL {}: {err}",
                self.events_path.display()
            )
        })?;

        let ended_at_unix_ms = current_unix_ms();
        let summary = AutoscaleSoakRunSummary {
            test_name: self.test_name.clone(),
            started_at_unix_ms: self.started_at_unix_ms,
            ended_at_unix_ms,
            duration_s: self.started_at.elapsed().as_secs_f64(),
            cycles_completed: self.cycles_completed,
            attempts_total: self.attempts_total,
            attempt_failures_total: self.attempt_failures_total,
            retries_used_total: self.retries_used_total,
            max_attempt_used_in_any_cycle: self.max_attempt_used_in_any_cycle,
            expansion_timing: SoakTimingSummary::from_samples(&self.expansion_times_s),
            contraction_timing: SoakTimingSummary::from_samples(&self.contraction_times_s),
            scale_out_quorum_misses_total: self.scale_out_quorum_misses_total,
            scale_in_post_expansion_quorum_misses_total: self
                .scale_in_post_expansion_quorum_misses_total,
            final_result: final_result.to_owned(),
            failure_cycle,
            failure_reason,
        };

        let mut rendered = norito::json::to_string_pretty(&summary.to_json_value())
            .map_err(|err| eyre!("serialize autoscale soak summary JSON: {err}"))?;
        rendered.push('\n');
        fs::write(&self.summary_path, rendered).map_err(|err| {
            eyre!(
                "write autoscale soak summary {}: {err}",
                self.summary_path.display()
            )
        })?;
        Ok(())
    }
}

fn collect_directory_tree_stats(root: &Path) -> Result<ElasticLaneStorageStats> {
    let mut stats = ElasticLaneStorageStats::default();
    let mut pending = vec![PathBuf::from(root)];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_type = entry.file_type()?;
            if entry_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !entry_type.is_file() {
                continue;
            }
            stats.file_count = stats.file_count.saturating_add(1);
            stats.total_bytes = stats
                .total_bytes
                .saturating_add(entry.metadata().map(|meta| meta.len()).unwrap_or_default());
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok())
                .unwrap_or_default();
            stats.newest_modified_unix_ms = stats.newest_modified_unix_ms.max(modified);
        }
    }
    Ok(stats)
}

fn peer_elastic_lane_storage_stats(peer: &NetworkPeer) -> Result<Option<ElasticLaneStorageStats>> {
    let blocks_root = peer.kura_store_dir().join("blocks");
    if !blocks_root.exists() {
        return Ok(None);
    }
    let elastic_lane_prefix = format!("lane_{ELASTIC_LANE_ID:03}");
    for entry in fs::read_dir(&blocks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with(&elastic_lane_prefix) {
            return collect_directory_tree_stats(&entry.path()).map(Some);
        }
    }
    Ok(None)
}

fn elastic_lane_storage_snapshot(
    network: &sandbox::SerializedNetwork,
) -> Result<Vec<Option<ElasticLaneStorageStats>>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            peer_elastic_lane_storage_stats(peer)
                .map_err(|err| eyre!("read elastic lane storage on peer {index}: {err}"))
        })
        .collect()
}

fn elastic_lane_storage_progressed(
    current: Option<ElasticLaneStorageStats>,
    baseline: Option<ElasticLaneStorageStats>,
) -> bool {
    match (current, baseline) {
        (Some(current), Some(baseline)) => {
            current.file_count > baseline.file_count
                || current.total_bytes > baseline.total_bytes
                || current.newest_modified_unix_ms > baseline.newest_modified_unix_ms
        }
        (Some(_), None) => true,
        _ => false,
    }
}

fn peer_run_stdout_log_path(peer: &NetworkPeer) -> Result<Option<PathBuf>> {
    let peer_dir = peer
        .kura_store_dir()
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| eyre!("derive peer directory from kura_store_dir"))?;
    if !peer_dir.exists() {
        return Ok(None);
    }
    let mut latest_run = None::<(u64, PathBuf)>;
    for entry in fs::read_dir(&peer_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let Some(run_segment) = file_name
            .strip_prefix("run-")
            .and_then(|name| name.strip_suffix("-stdout.log"))
        else {
            continue;
        };
        let Ok(run_id) = run_segment.parse::<u64>() else {
            continue;
        };
        match latest_run {
            Some((latest_run_id, _)) if run_id <= latest_run_id => {}
            _ => latest_run = Some((run_id, entry.path())),
        }
    }
    Ok(latest_run.map(|(_, path)| path))
}

fn parse_autoscale_transition_stats(log_contents: &str) -> AutoscaleTransitionStats {
    let scale_out_transitions = u64::try_from(
        log_contents
            .matches(AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let scale_in_transitions = u64::try_from(
        log_contents
            .matches(AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER)
            .count(),
    )
    .unwrap_or(u64::MAX);
    AutoscaleTransitionStats {
        scale_out_transitions,
        scale_in_transitions,
    }
}

fn peer_autoscale_transition_stats(peer: &NetworkPeer) -> Result<AutoscaleTransitionStats> {
    let Some(stdout_log_path) = peer_run_stdout_log_path(peer)? else {
        return Ok(AutoscaleTransitionStats::default());
    };
    let log_contents = match fs::read_to_string(&stdout_log_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AutoscaleTransitionStats::default());
        }
        Err(err) => {
            return Err(err).map_err(|error| {
                eyre!(
                    "read autoscale transition log {:?}: {error}",
                    stdout_log_path
                )
            });
        }
    };
    Ok(parse_autoscale_transition_stats(&log_contents))
}

fn autoscale_transition_snapshot(
    network: &sandbox::SerializedNetwork,
) -> Result<Vec<AutoscaleTransitionStats>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            peer_autoscale_transition_stats(peer)
                .map_err(|err| eyre!("read autoscale transition stats on peer {index}: {err}"))
        })
        .collect()
}

fn peers_with_scale_out_transition(
    current: &[AutoscaleTransitionStats],
    baseline: &[AutoscaleTransitionStats],
) -> usize {
    current
        .iter()
        .enumerate()
        .filter(|(index, current)| {
            current.scale_out_transitions
                > baseline
                    .get(*index)
                    .map(|stats| stats.scale_out_transitions)
                    .unwrap_or_default()
        })
        .count()
}

fn peers_with_scale_in_transition(
    current: &[AutoscaleTransitionStats],
    baseline: &[AutoscaleTransitionStats],
) -> usize {
    current
        .iter()
        .enumerate()
        .filter(|(index, current)| {
            current.scale_in_transitions
                > baseline
                    .get(*index)
                    .map(|stats| stats.scale_in_transitions)
                    .unwrap_or_default()
        })
        .count()
}

fn scale_in_transition_counts(
    current: &[AutoscaleTransitionStats],
    baseline_after_expansion: &[AutoscaleTransitionStats],
    baseline_since_cycle_start: Option<&[AutoscaleTransitionStats]>,
) -> (usize, Option<usize>) {
    let peers_after_expansion = peers_with_scale_in_transition(current, baseline_after_expansion);
    let peers_since_cycle_start = baseline_since_cycle_start
        .map(|baseline| peers_with_scale_in_transition(current, baseline));
    (peers_after_expansion, peers_since_cycle_start)
}

fn peer_client_with_timeout(peer: &NetworkPeer) -> Client {
    let mut client = peer.client();
    client.torii_request_timeout = TORII_REQUEST_TIMEOUT;
    client
}

#[derive(Clone, Debug)]
struct LaneStatusSnapshot {
    lane_id: u32,
    capacity: u64,
    committed: u64,
}

#[derive(Clone, Debug)]
struct LaneCommitmentSnapshot {
    lane_id: u32,
    block_height: u64,
    tx_count: u64,
    teu_total: u64,
}

#[derive(Clone, Debug)]
struct LaneRelaySnapshot {
    lane_id: u32,
    block_height: u64,
}

#[derive(Clone, Debug)]
struct LaneValidatorSnapshot {
    lane_id: u32,
    total: u64,
    active: u64,
    pending_activation: u64,
    max_activation_epoch: u64,
    max_activation_height: u64,
}

#[derive(Clone, Debug)]
struct PeerStatusSnapshot {
    lanes: Vec<LaneStatusSnapshot>,
    lane_commitments: Vec<LaneCommitmentSnapshot>,
    lane_governance_ids: Vec<u32>,
    lane_relay: Vec<LaneRelaySnapshot>,
    lane_validators: Vec<LaneValidatorSnapshot>,
    commit_signatures_required: u64,
    commit_qc_validator_set_len: u64,
    txs_approved: u64,
    txs_rejected: u64,
    blocks_non_empty: u64,
}

fn decode_lane_validator_snapshot(
    payload: &norito::json::Value,
    lane_id: u32,
) -> Option<LaneValidatorSnapshot> {
    let root = payload.as_object()?;
    let items = root.get("items").and_then(norito::json::Value::as_array);
    let total = root
        .get("total")
        .and_then(norito::json::Value::as_u64)
        .or_else(|| items.and_then(|entries| u64::try_from(entries.len()).ok()))
        .unwrap_or_default();

    let mut active = 0_u64;
    let mut pending_activation = 0_u64;
    let mut max_activation_epoch = 0_u64;
    let mut max_activation_height = 0_u64;
    if let Some(entries) = items {
        for entry in entries {
            let entry_obj = entry.as_object();
            let status_type = entry
                .as_object()
                .and_then(|item| item.get("status"))
                .and_then(norito::json::Value::as_object)
                .and_then(|status| status.get("type"))
                .and_then(norito::json::Value::as_str);
            match status_type {
                Some("Active") => active = active.saturating_add(1),
                Some("PendingActivation") => {
                    pending_activation = pending_activation.saturating_add(1);
                }
                _ => {}
            }
            if let Some(epoch) = entry_obj
                .and_then(|item| item.get("activation_epoch"))
                .and_then(norito::json::Value::as_u64)
            {
                max_activation_epoch = max_activation_epoch.max(epoch);
            }
            if let Some(height) = entry_obj
                .and_then(|item| item.get("activation_height"))
                .and_then(norito::json::Value::as_u64)
            {
                max_activation_height = max_activation_height.max(height);
            }
        }
    }

    Some(LaneValidatorSnapshot {
        lane_id,
        total,
        active,
        pending_activation,
        max_activation_epoch,
        max_activation_height,
    })
}

fn is_not_found_lane_validator_error(message: &str) -> bool {
    message.contains("404") || message.contains("Not Found")
}

fn fetch_lane_validator_snapshot(client: &Client, lane_id: u32) -> Option<LaneValidatorSnapshot> {
    match client.get_public_lane_validators(LaneId::new(lane_id)) {
        Ok(payload) => decode_lane_validator_snapshot(&payload, lane_id),
        Err(err) => {
            let message = err.to_string();
            if is_not_found_lane_validator_error(&message) {
                Some(LaneValidatorSnapshot {
                    lane_id,
                    total: 0,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 0,
                    max_activation_height: 0,
                })
            } else {
                None
            }
        }
    }
}

fn status_snapshot(network: &sandbox::SerializedNetwork) -> Result<Vec<PeerStatusSnapshot>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let client = peer_client_with_timeout(peer);
            let status = client
                .get_status()
                .map_err(|err| eyre!("fetch peer {index} status failed: {err}"))?;
            let lanes = status
                .teu_lane_commit
                .iter()
                .map(|lane| LaneStatusSnapshot {
                    lane_id: lane.lane_id,
                    capacity: lane.capacity,
                    committed: lane.committed,
                })
                .collect::<Vec<_>>();
            let (lane_commitments, lane_governance_ids, lane_relay) =
                match client.get_sumeragi_status() {
                    Ok(sumeragi_status) => (
                        sumeragi_status
                            .lane_commitments
                            .into_iter()
                            .map(|lane| LaneCommitmentSnapshot {
                                lane_id: lane.lane_id.as_u32(),
                                block_height: lane.block_height,
                                tx_count: lane.tx_count,
                                teu_total: lane.teu_total,
                            })
                            .collect::<Vec<_>>(),
                        sumeragi_status
                            .lane_governance
                            .into_iter()
                            .map(|lane| lane.lane_id.as_u32())
                            .collect::<Vec<_>>(),
                        sumeragi_status
                            .lane_relay_envelopes
                            .into_iter()
                            .map(|lane| LaneRelaySnapshot {
                                lane_id: lane.lane_id.as_u32(),
                                block_height: lane.block_height,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    Err(_) => (Vec::new(), Vec::new(), Vec::new()),
                };
            let lane_validators = fetch_lane_validator_snapshot(&client, ELASTIC_LANE_ID)
                .into_iter()
                .collect::<Vec<_>>();
            let commit_signatures_required = status
                .sumeragi
                .as_ref()
                .map(|sumeragi| sumeragi.commit_signatures_required)
                .unwrap_or_default();
            let commit_qc_validator_set_len = status
                .sumeragi
                .as_ref()
                .map(|sumeragi| sumeragi.commit_qc_validator_set_len)
                .unwrap_or_default();
            Ok(PeerStatusSnapshot {
                lanes,
                lane_commitments,
                lane_governance_ids,
                lane_relay,
                lane_validators,
                commit_signatures_required,
                commit_qc_validator_set_len,
                txs_approved: status.txs_approved,
                txs_rejected: status.txs_rejected,
                blocks_non_empty: status.blocks_non_empty,
            })
        })
        .collect()
}

fn all_peers_have_storage_lane_count(
    snapshot: &[(usize, Vec<String>)],
    expected_count: usize,
) -> bool {
    snapshot
        .iter()
        .all(|(_, lanes)| lanes.len() == expected_count)
}

fn peer_has_active_lane_capacity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lanes
        .iter()
        .any(|lane| lane.lane_id == lane_id && (lane.capacity > 0 || lane.committed > 0))
}

fn peer_has_lane_commitment_activity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lane_commitments
        .iter()
        .any(|lane| lane.lane_id == lane_id && (lane.tx_count > 0 || lane.teu_total > 0))
}

fn peer_lane_status(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<&LaneStatusSnapshot> {
    peer.lanes.iter().find(|lane| lane.lane_id == lane_id)
}

fn peer_lane_commitment_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneCommitmentSnapshot> {
    peer.lane_commitments
        .iter()
        .filter(|lane| lane.lane_id == lane_id)
        .max_by_key(|lane| lane.block_height)
}

fn peer_lane_relay_height(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<u64> {
    peer.lane_relay
        .iter()
        .filter(|relay| relay.lane_id == lane_id)
        .map(|relay| relay.block_height)
        .max()
}

fn peer_lane_validator_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneValidatorSnapshot> {
    peer.lane_validators
        .iter()
        .filter(|lane| lane.lane_id == lane_id)
        .max_by_key(|lane| (lane.active, lane.pending_activation, lane.total))
}

fn peer_has_lane_declaration(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lanes
        .iter()
        .any(|lane| lane.lane_id == lane_id && (lane.capacity > 0 || lane.committed > 0))
        || peer
            .lane_commitments
            .iter()
            .any(|lane| lane.lane_id == lane_id)
        || peer
            .lane_governance_ids
            .iter()
            .any(|declared_lane| *declared_lane == lane_id)
        || peer.lane_validators.iter().any(|lane| {
            lane.lane_id == lane_id
                && (lane.total > 0 || lane.active > 0 || lane.pending_activation > 0)
        })
}

fn peer_has_lane_declaration_transition(
    peer: &PeerStatusSnapshot,
    baseline_peer: Option<&PeerStatusSnapshot>,
    lane_id: u32,
) -> bool {
    let Some(baseline_peer) = baseline_peer else {
        return false;
    };
    !peer_has_lane_declaration(baseline_peer, lane_id) && peer_has_lane_declaration(peer, lane_id)
}

fn peer_has_lane_progress_transition(
    peer: &PeerStatusSnapshot,
    baseline_peer: Option<&PeerStatusSnapshot>,
    lane_id: u32,
) -> bool {
    let Some(baseline_peer) = baseline_peer else {
        return false;
    };

    let status_progressed = match (
        peer_lane_status(peer, lane_id),
        peer_lane_status(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.capacity > baseline.capacity || current.committed > baseline.committed
        }
        (Some(current), None) => current.capacity > 0 || current.committed > 0,
        _ => false,
    };

    let commitment_progressed = match (
        peer_lane_commitment_snapshot(peer, lane_id),
        peer_lane_commitment_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.block_height > baseline.block_height
                || current.tx_count > baseline.tx_count
                || current.teu_total > baseline.teu_total
        }
        (Some(current), None) => {
            current.block_height > 0 || current.tx_count > 0 || current.teu_total > 0
        }
        _ => false,
    };

    let relay_progressed = match (
        peer_lane_relay_height(peer, lane_id),
        peer_lane_relay_height(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => current > baseline,
        (Some(current), None) => current > 0,
        _ => false,
    };

    let validator_progressed = match (
        peer_lane_validator_snapshot(peer, lane_id),
        peer_lane_validator_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.active > baseline.active
                || current.pending_activation > baseline.pending_activation
                || current.total > baseline.total
                || current.max_activation_epoch > baseline.max_activation_epoch
                || current.max_activation_height > baseline.max_activation_height
        }
        _ => false,
    };

    status_progressed || commitment_progressed || relay_progressed || validator_progressed
}

fn peers_with_expanded_lane_signal(
    snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    lane_id: u32,
) -> usize {
    let mut peers_with_signal = 0_usize;
    for (index, peer) in snapshot.iter().enumerate() {
        let baseline_peer = baseline_snapshot.and_then(|baseline| baseline.get(index));
        if peer_has_active_lane_capacity(peer, lane_id)
            || peer_has_lane_commitment_activity(peer, lane_id)
            || peer_has_lane_declaration_transition(peer, baseline_peer, lane_id)
            || peer_has_lane_progress_transition(peer, baseline_peer, lane_id)
        {
            peers_with_signal += 1;
        }
    }
    peers_with_signal
}

#[derive(Clone, Copy, Debug, Default)]
struct ExpansionSignalBreakdown {
    peers_with_active_capacity: usize,
    peers_with_commitment_activity: usize,
    peers_with_lane_declaration: usize,
    peers_with_lane_declaration_transition: usize,
    peers_with_lane_progress_transition: usize,
    peers_with_lane_validator_snapshot: usize,
    peers_with_lane_validator_activity: usize,
}

fn expansion_signal_breakdown(
    snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    lane_id: u32,
) -> ExpansionSignalBreakdown {
    let mut breakdown = ExpansionSignalBreakdown::default();
    for (index, peer) in snapshot.iter().enumerate() {
        let baseline_peer = baseline_snapshot.and_then(|baseline| baseline.get(index));
        if peer_has_active_lane_capacity(peer, lane_id) {
            breakdown.peers_with_active_capacity += 1;
        }
        if peer_has_lane_commitment_activity(peer, lane_id) {
            breakdown.peers_with_commitment_activity += 1;
        }
        if peer_has_lane_declaration(peer, lane_id) {
            breakdown.peers_with_lane_declaration += 1;
        }
        if peer_has_lane_declaration_transition(peer, baseline_peer, lane_id) {
            breakdown.peers_with_lane_declaration_transition += 1;
        }
        if peer_has_lane_progress_transition(peer, baseline_peer, lane_id) {
            breakdown.peers_with_lane_progress_transition += 1;
        }
        if let Some(validator) = peer_lane_validator_snapshot(peer, lane_id) {
            breakdown.peers_with_lane_validator_snapshot += 1;
            if validator.active > 0 || validator.pending_activation > 0 {
                breakdown.peers_with_lane_validator_activity += 1;
            }
        }
    }
    breakdown
}

fn expansion_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    quorum_required: usize,
) -> bool {
    peers_with_expanded_lane_signal(status_snapshot, baseline_snapshot, ELASTIC_LANE_ID)
        >= quorum_required
}

fn scale_out_transition_observed_on_quorum_peers(
    transition_snapshot: &[AutoscaleTransitionStats],
    baseline_transitions: &[AutoscaleTransitionStats],
    quorum_required: usize,
) -> bool {
    peers_with_scale_out_transition(transition_snapshot, baseline_transitions) >= quorum_required
}

fn expansion_observed_on_quorum_or_scale_out_transition(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    transition_snapshot: &[AutoscaleTransitionStats],
    baseline_transitions: &[AutoscaleTransitionStats],
    quorum_required: usize,
) -> bool {
    expansion_observed_on_quorum_peers(status_snapshot, baseline_snapshot, quorum_required)
        || scale_out_transition_observed_on_quorum_peers(
            transition_snapshot,
            baseline_transitions,
            quorum_required,
        )
}

fn expansion_observed_on_storage(storage_snapshot: &[(usize, Vec<String>)]) -> bool {
    all_peers_have_storage_lane_count(storage_snapshot, EXPANDED_PROVISIONED_LANES)
}

fn expansion_observed_with_prior_scale_out_quorum_on_storage(
    storage_snapshot: &[(usize, Vec<String>)],
    baseline_transitions: &[AutoscaleTransitionStats],
    quorum_required: usize,
) -> bool {
    expansion_observed_on_storage(storage_snapshot)
        && peers_with_scale_out_transition(baseline_transitions, &[]) >= quorum_required
}

fn peer_has_contracted_profile(status: &PeerStatusSnapshot) -> bool {
    let primary_lane_active = status
        .lanes
        .iter()
        .any(|lane| lane.lane_id == PRIMARY_LANE_ID && lane.capacity > 0);
    let elastic_lane_undeclared = !peer_has_lane_declaration(status, ELASTIC_LANE_ID);
    let elastic_lane_commitments_idle = !peer_has_lane_commitment_activity(status, ELASTIC_LANE_ID);
    let elastic_lane_relay_idle = peer_lane_relay_height(status, ELASTIC_LANE_ID).is_none();
    let elastic_lane_validator_idle = match peer_lane_validator_snapshot(status, ELASTIC_LANE_ID) {
        Some(snapshot) => {
            snapshot.total == 0
                && snapshot.active == 0
                && snapshot.pending_activation == 0
                && snapshot.max_activation_epoch == 0
                && snapshot.max_activation_height == 0
        }
        None => true,
    };

    primary_lane_active
        && elastic_lane_undeclared
        && elastic_lane_commitments_idle
        && elastic_lane_relay_idle
        && elastic_lane_validator_idle
}

fn peers_with_contracted_profile(snapshot: &[PeerStatusSnapshot]) -> usize {
    snapshot
        .iter()
        .filter(|status| peer_has_contracted_profile(status))
        .count()
}

fn contraction_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
) -> bool {
    peers_with_contracted_profile(status_snapshot) >= quorum_required
}

fn should_require_scale_in_transition(
    requested: bool,
    post_expansion_snapshot: &[PeerStatusSnapshot],
    pre_cycle_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
) -> bool {
    requested
        && expansion_observed_on_quorum_peers(
            post_expansion_snapshot,
            Some(pre_cycle_snapshot),
            quorum_required,
        )
}

fn max_non_empty_height(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.blocks_non_empty)
        .max()
        .unwrap_or_default()
}

fn max_txs_approved(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.txs_approved)
        .max()
        .unwrap_or_default()
}

fn max_txs_rejected(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.txs_rejected)
        .max()
        .unwrap_or_default()
}

fn chain_progress_advanced(
    snapshot: &[PeerStatusSnapshot],
    baseline_non_empty: u64,
    baseline_txs_approved: u64,
    baseline_txs_rejected: u64,
) -> bool {
    max_non_empty_height(snapshot) > baseline_non_empty
        || max_txs_approved(snapshot) > baseline_txs_approved
        || max_txs_rejected(snapshot) > baseline_txs_rejected
}

fn wait_for_commit_quorum_required(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
    context: &str,
) -> Result<usize> {
    let started = Instant::now();
    let mut last_error = None::<String>;
    let mut last_observed_required = Vec::<u64>::new();
    let mut last_observed_validator_set_len = Vec::<u64>::new();
    let mut consecutive_failures = 0_u32;

    while started.elapsed() <= timeout {
        match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_failures = 0;
                let observed_required = snapshot
                    .iter()
                    .map(|status| status.commit_signatures_required)
                    .filter(|value| *value > 0)
                    .collect::<Vec<_>>();
                last_observed_required = observed_required.clone();

                if !observed_required.is_empty() {
                    let min_required = *observed_required.iter().min().unwrap_or(&0);
                    let max_required = *observed_required.iter().max().unwrap_or(&0);
                    if min_required == max_required {
                        let quorum_required = usize::try_from(max_required).map_err(|_| {
                            eyre!("{context}: quorum value does not fit usize: {max_required}")
                        })?;
                        ensure!(
                            (1..=network.peers().len()).contains(&quorum_required),
                            "{context}: invalid commit quorum value {quorum_required} for peer count {}; observed values: {observed_required:?}",
                            network.peers().len()
                        );
                        return Ok(quorum_required);
                    }
                }

                let observed_validator_set_len = snapshot
                    .iter()
                    .map(|status| status.commit_qc_validator_set_len)
                    .filter(|value| *value > 0)
                    .collect::<Vec<_>>();
                last_observed_validator_set_len = observed_validator_set_len.clone();
                if !observed_validator_set_len.is_empty() {
                    let min_len = *observed_validator_set_len.iter().min().unwrap_or(&0);
                    let max_len = *observed_validator_set_len.iter().max().unwrap_or(&0);
                    if min_len == max_len {
                        let validator_set_len = usize::try_from(max_len).map_err(|_| {
                            eyre!("{context}: validator set length does not fit usize: {max_len}")
                        })?;
                        let quorum_required = commit_quorum_from_len(validator_set_len);
                        ensure!(
                            (1..=network.peers().len()).contains(&quorum_required),
                            "{context}: invalid derived quorum {quorum_required} from validator set len {validator_set_len} for peer count {}",
                            network.peers().len()
                        );
                        return Ok(quorum_required);
                    }
                }

                thread::sleep(STATUS_SNAPSHOT_RETRY_BACKOFF);
            }
            Err(err) => {
                last_error = Some(err.to_string());
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= STATUS_SNAPSHOT_RETRY_LIMIT {
                    return Err(eyre!(
                        "{context}: status snapshot failed {} consecutive times; last error: {last_error:?}",
                        STATUS_SNAPSHOT_RETRY_LIMIT
                    ));
                }
                thread::sleep(STATUS_SNAPSHOT_RETRY_BACKOFF);
            }
        }
    }

    let fallback_quorum = commit_quorum_from_len(network.peers().len());
    eprintln!(
        "[autoscale-localnet] commit quorum fallback from peer count: {} (context: {context}; last required={last_observed_required:?}; last validator_set_len={last_observed_validator_set_len:?}; last error={last_error:?})",
        fallback_quorum
    );
    Ok(fallback_quorum)
}

fn wait_for_storage_lane_count(
    network: &sandbox::SerializedNetwork,
    expected_count: usize,
    timeout: Duration,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_storage_snapshot = Vec::new();
    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        if all_peers_have_storage_lane_count(&storage_snapshot, expected_count) {
            return Ok(());
        }
        last_storage_snapshot = storage_snapshot;
        thread::sleep(LANE_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for {expected_count} provisioned lane directories on all peers; last storage snapshot: {last_storage_snapshot:?}"
    ))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn current_unix_ms() -> u64 {
    UNIX_EPOCH
        .elapsed()
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or_default()
}

fn autoscale_soak_artifact_root(network: &sandbox::SerializedNetwork) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(AUTOSCALE_SOAK_ARTIFACT_DIR_ENV) {
        return Ok(PathBuf::from(path));
    }
    let peer_dir = network
        .peer()
        .kura_store_dir()
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| eyre!("derive peer directory from kura_store_dir"))?;
    Ok(peer_dir.parent().map(Path::to_path_buf).unwrap_or(peer_dir))
}

fn deterministic_seed_hash(seed: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn configure_load_sequence_seed(seed: Option<&str>) -> u64 {
    let sequence_seed = seed
        .filter(|value| !value.trim().is_empty())
        .map(deterministic_seed_hash)
        .unwrap_or_default();
    LOAD_TX_SEQUENCE.store(sequence_seed, Ordering::Relaxed);
    sequence_seed
}

fn autoscale_soak_seed() -> String {
    std::env::var(AUTOSCALE_SOAK_SEED_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| AUTOSCALE_SOAK_DEFAULT_SEED.to_owned())
}

fn autoscale_soak_force_fail_cycle() -> Option<usize> {
    std::env::var(AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn submit_load_round_robin(clients: &[Client], tx_count: usize) -> Result<()> {
    ensure!(
        !clients.is_empty(),
        "load submission requires at least one client"
    );

    let mut submitted = 0_usize;
    let mut first_error = None::<String>;
    for tx in 0..tx_count {
        let load_sequence = LOAD_TX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let client_index =
            usize::try_from(load_sequence % usize_to_u64(clients.len())).unwrap_or(0);
        let client = &clients[client_index];
        match client.submit(Log::new(
            Level::INFO,
            format!("autoscale-load-{load_sequence}"),
        )) {
            Ok(_) => submitted = submitted.saturating_add(1),
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(format!(
                        "submit autoscale load transaction {tx} failed: {err}"
                    ));
                }
            }
        }
    }
    if tx_count > 0 && submitted < tx_count {
        eprintln!(
            "[autoscale-localnet] partial load submission: submitted {submitted}/{tx_count}; first error: {first_error:?}"
        );
    }
    validate_load_submission_outcome(tx_count, submitted, first_error.as_deref())?;
    Ok(())
}

fn validate_load_submission_outcome(
    attempted: usize,
    submitted: usize,
    first_error: Option<&str>,
) -> Result<()> {
    ensure!(
        attempted == 0 || submitted > 0,
        "all autoscale load submissions failed ({submitted}/{attempted}); first error: {first_error:?}"
    );
    Ok(())
}

fn soak_cycle_load_tx_count(attempt: usize) -> usize {
    STRICT_CYCLE_LOAD_TX_COUNT.saturating_mul(attempt.max(1))
}

fn should_run_cooldown_clearance(cycle_index: usize, attempt: usize) -> bool {
    cycle_index > 1 && attempt <= 1
}

fn expansion_top_up_tx_count(heartbeat_seq: u64) -> usize {
    if heartbeat_seq == 0 {
        return 0;
    }
    if heartbeat_seq % EXPANSION_REINFORCE_EVERY_HEARTBEATS == 0 {
        return EXPANSION_REINFORCE_TX_COUNT;
    }
    if heartbeat_seq % EXPANSION_TOP_UP_EVERY_HEARTBEATS == 0 {
        return EXPANSION_TOP_UP_TX_COUNT;
    }
    0
}

fn expansion_scaled_top_up_tx_count(heartbeat_seq: u64, cycle_load_tx_count: usize) -> usize {
    let baseline = expansion_top_up_tx_count(heartbeat_seq);
    if baseline == 0 {
        return 0;
    }

    if heartbeat_seq % EXPANSION_REINFORCE_EVERY_HEARTBEATS == 0 {
        return baseline.max(cycle_load_tx_count.saturating_div(2));
    }
    baseline.max(cycle_load_tx_count.saturating_div(4))
}

fn expansion_post_storage_top_up_tx_count(cycle_load_tx_count: usize) -> usize {
    EXPANSION_POST_STORAGE_TOP_UP_TX_COUNT.max(cycle_load_tx_count)
}

fn expansion_probe_top_up_tx_count(
    heartbeat_seq: u64,
    storage_expanded: bool,
    elapsed: Duration,
    cycle_load_tx_count: usize,
) -> usize {
    if storage_expanded && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE {
        return expansion_post_storage_top_up_tx_count(cycle_load_tx_count);
    }
    expansion_scaled_top_up_tx_count(heartbeat_seq, cycle_load_tx_count)
}

fn wait_for_expanded_lanes_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    top_up_clients: &[Client],
    cycle_load_tx_count: usize,
    baseline_status_snapshot: &[PeerStatusSnapshot],
    baseline_elastic_storage_snapshot: &[Option<ElasticLaneStorageStats>],
    baseline_autoscale_transitions: &[AutoscaleTransitionStats],
    require_scale_out_transition: bool,
    quorum_required: usize,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_storage_snapshot = Vec::new();
    let mut last_elastic_storage_snapshot = Vec::new();
    let mut last_status_snapshot = Vec::new();
    let mut last_transition_snapshot = Vec::new();
    let mut last_heartbeat_error = None::<String>;
    let mut last_top_up_error = None::<String>;
    let mut last_status_error = None::<String>;
    let mut last_transition_error = None::<String>;
    let mut last_scale_out_transition_peers = 0_usize;
    let mut post_grace_wait_logged = false;
    let baseline_scale_out_transition_peers =
        peers_with_scale_out_transition(baseline_autoscale_transitions, &[]);

    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        let elastic_storage_snapshot = elastic_lane_storage_snapshot(network)?;
        let peers_with_storage_progress = elastic_storage_snapshot
            .iter()
            .enumerate()
            .filter(|(index, current)| {
                let baseline = baseline_elastic_storage_snapshot
                    .get(*index)
                    .copied()
                    .flatten();
                elastic_lane_storage_progressed(**current, baseline)
            })
            .count();
        let storage_progressed_on_quorum = peers_with_storage_progress >= quorum_required;
        let storage_expanded = expansion_observed_on_storage(&storage_snapshot);
        let elapsed = started.elapsed();
        let fallback_ready_at =
            EXPANSION_STATUS_SIGNAL_GRACE + EXPANSION_POST_STORAGE_STATUS_WINDOW;

        if require_scale_out_transition
            && expansion_observed_with_prior_scale_out_quorum_on_storage(
                &storage_snapshot,
                baseline_autoscale_transitions,
                quorum_required,
            )
        {
            eprintln!(
                "[autoscale-localnet] {context}: expansion accepted via persisted storage lane profile with previously satisfied deterministic autoscale scale-out transition quorum (baseline scale-out transitions {baseline_scale_out_transition_peers}/{quorum_required})"
            );
            return Ok(());
        }

        let status_snapshot = match status_snapshot(network) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                last_status_error = Some(err.to_string());
                let scale_out_transition_observed_on_quorum =
                    match autoscale_transition_snapshot(network) {
                        Ok(snapshot) => {
                            last_scale_out_transition_peers = peers_with_scale_out_transition(
                                &snapshot,
                                baseline_autoscale_transitions,
                            );
                            last_transition_snapshot = snapshot;
                            last_transition_error = None;
                            last_scale_out_transition_peers >= quorum_required
                        }
                        Err(transition_err) => {
                            last_transition_error = Some(transition_err.to_string());
                            false
                        }
                    };
                if scale_out_transition_observed_on_quorum {
                    eprintln!(
                        "[autoscale-localnet] {context}: expansion observed via deterministic autoscale scale-out transitions despite status errors (scale-out transitions {last_scale_out_transition_peers}/{quorum_required})"
                    );
                    return Ok(());
                }
                if !require_scale_out_transition
                    && storage_expanded
                    && storage_progressed_on_quorum
                    && elapsed >= fallback_ready_at
                {
                    eprintln!(
                        "[autoscale-localnet] {context}: expansion observed via storage lane provisioning+progress fallback after status errors (storage progress {peers_with_storage_progress}/{quorum_required}, scale-out transitions {last_scale_out_transition_peers}/{quorum_required}, grace {:?} + post-storage status window {:?}); last transition error: {last_transition_error:?}",
                        EXPANSION_STATUS_SIGNAL_GRACE, EXPANSION_POST_STORAGE_STATUS_WINDOW
                    );
                    return Ok(());
                }
                if storage_expanded
                    && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE
                    && !post_grace_wait_logged
                {
                    eprintln!(
                        "[autoscale-localnet] {context}: storage expansion reached grace window; continuing status probe for {:?} before fallback{}",
                        EXPANSION_POST_STORAGE_STATUS_WINDOW,
                        if require_scale_out_transition {
                            " (scale-out transition quorum required)"
                        } else {
                            ""
                        }
                    );
                    post_grace_wait_logged = true;
                }
                last_storage_snapshot = storage_snapshot;
                last_elastic_storage_snapshot = elastic_storage_snapshot;
                if let Err(heartbeat_err) = heartbeat_client.submit(Log::new(
                    Level::INFO,
                    format!("{heartbeat_prefix}-{heartbeat_seq}"),
                )) {
                    last_heartbeat_error = Some(heartbeat_err.to_string());
                }
                heartbeat_seq = heartbeat_seq.saturating_add(1);
                let top_up_tx_count = expansion_probe_top_up_tx_count(
                    heartbeat_seq,
                    storage_expanded,
                    elapsed,
                    cycle_load_tx_count,
                );
                if top_up_tx_count > 0 {
                    if let Err(top_up_err) =
                        submit_load_round_robin(top_up_clients, top_up_tx_count)
                    {
                        last_top_up_error = Some(top_up_err.to_string());
                    }
                }
                thread::sleep(heartbeat_interval);
                continue;
            }
        };

        let expansion_observed_on_status = expansion_observed_on_quorum_peers(
            &status_snapshot,
            Some(baseline_status_snapshot),
            quorum_required,
        );
        let scale_out_transition_observed_on_quorum = match autoscale_transition_snapshot(network) {
            Ok(snapshot) => {
                last_scale_out_transition_peers =
                    peers_with_scale_out_transition(&snapshot, baseline_autoscale_transitions);
                last_transition_snapshot = snapshot;
                last_transition_error = None;
                last_scale_out_transition_peers >= quorum_required
            }
            Err(err) => {
                last_transition_error = Some(err.to_string());
                false
            }
        };

        let expansion_ready = if require_scale_out_transition {
            scale_out_transition_observed_on_quorum
        } else {
            expansion_observed_on_status || scale_out_transition_observed_on_quorum
        };
        if expansion_ready {
            if expansion_observed_on_status {
                eprintln!(
                    "[autoscale-localnet] {context}: expansion observed via status lane activity/lifecycle transitions"
                );
            } else if scale_out_transition_observed_on_quorum {
                eprintln!(
                    "[autoscale-localnet] {context}: expansion observed via deterministic autoscale scale-out transitions (scale-out transitions {last_scale_out_transition_peers}/{quorum_required})"
                );
            }
            return Ok(());
        }
        if !require_scale_out_transition
            && storage_expanded
            && storage_progressed_on_quorum
            && elapsed >= fallback_ready_at
        {
            let peers_with_status_signal = peers_with_expanded_lane_signal(
                &status_snapshot,
                Some(baseline_status_snapshot),
                ELASTIC_LANE_ID,
            );
            let signal_breakdown = expansion_signal_breakdown(
                &status_snapshot,
                Some(baseline_status_snapshot),
                ELASTIC_LANE_ID,
            );
            eprintln!(
                "[autoscale-localnet] {context}: expansion observed via storage lane provisioning+progress fallback after {:.3}s (status signal {peers_with_status_signal}/{quorum_required}, storage progress {peers_with_storage_progress}/{quorum_required}, scale-out transitions {last_scale_out_transition_peers}/{quorum_required}, grace {:?} + post-storage status window {:?}); signal breakdown: {signal_breakdown:?}; last transition error: {last_transition_error:?}",
                elapsed.as_secs_f64(),
                EXPANSION_STATUS_SIGNAL_GRACE,
                EXPANSION_POST_STORAGE_STATUS_WINDOW
            );
            return Ok(());
        }
        if storage_expanded && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE && !post_grace_wait_logged {
            eprintln!(
                "[autoscale-localnet] {context}: storage expansion reached grace window; continuing status probe for {:?} before fallback{}",
                EXPANSION_POST_STORAGE_STATUS_WINDOW,
                if require_scale_out_transition {
                    " (scale-out transition quorum required)"
                } else {
                    ""
                }
            );
            post_grace_wait_logged = true;
        }
        last_storage_snapshot = storage_snapshot;
        last_elastic_storage_snapshot = elastic_storage_snapshot;
        last_status_snapshot = status_snapshot;

        if let Err(err) = heartbeat_client.submit(Log::new(
            Level::INFO,
            format!("{heartbeat_prefix}-{heartbeat_seq}"),
        )) {
            last_heartbeat_error = Some(err.to_string());
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);

        let top_up_tx_count = expansion_probe_top_up_tx_count(
            heartbeat_seq,
            storage_expanded,
            elapsed,
            cycle_load_tx_count,
        );
        if top_up_tx_count > 0 {
            if let Err(err) = submit_load_round_robin(top_up_clients, top_up_tx_count) {
                last_top_up_error = Some(err.to_string());
            }
        }
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for expanded lane profile (lane {ELASTIC_LANE_ID} active via status `capacity>0 || committed>0`, sumeragi lane commitment `tx_count>0 || teu_total>0`, public-lane validator lifecycle activity (`active || pending_activation`), baseline transition via lane declaration/progress, or deterministic autoscale scale-out transitions on >= {quorum_required}/{TOTAL_PEERS} peers{}; storage lane count={EXPANDED_PROVISIONED_LANES} accepted only as fallback after grace {:?} + post-storage status window {:?} when elastic lane storage progresses on >= {quorum_required}/{TOTAL_PEERS} peers and scale-out transition quorum is not required); last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last elastic storage snapshot: {last_elastic_storage_snapshot:?}; last autoscale transition snapshot: {last_transition_snapshot:?}; last scale-out transition peers: {last_scale_out_transition_peers}/{TOTAL_PEERS}; last status error: {last_status_error:?}; last transition error: {last_transition_error:?}; last heartbeat error: {last_heartbeat_error:?}; last top-up error: {last_top_up_error:?}",
        if require_scale_out_transition {
            "; strict mode requires deterministic scale-out transition quorum unless the cycle baseline already satisfies that quorum and the storage lane profile remains expanded"
        } else {
            ""
        },
        EXPANSION_STATUS_SIGNAL_GRACE,
        EXPANSION_POST_STORAGE_STATUS_WINDOW,
    ))
}

fn wait_for_chain_progress_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    baseline_non_empty: u64,
    baseline_txs_approved: u64,
    baseline_txs_rejected: u64,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_status_snapshot = Vec::new();
    let mut last_status_error = None::<String>;
    let mut last_heartbeat_error = None::<String>;

    while started.elapsed() <= timeout {
        match status_snapshot(network) {
            Ok(snapshot) => {
                if chain_progress_advanced(
                    &snapshot,
                    baseline_non_empty,
                    baseline_txs_approved,
                    baseline_txs_rejected,
                ) {
                    return Ok(());
                }
                last_status_snapshot = snapshot;
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
            }
        }

        if let Err(err) = heartbeat_client.submit(Log::new(
            Level::INFO,
            format!("{heartbeat_prefix}-{heartbeat_seq}"),
        )) {
            last_heartbeat_error = Some(err.to_string());
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for chain progress (baseline blocks_non_empty={baseline_non_empty}, txs_approved={baseline_txs_approved}, txs_rejected={baseline_txs_rejected}); last status snapshot: {last_status_snapshot:?}; last status error: {last_status_error:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
}

fn wait_for_contracted_lanes(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: Option<&Client>,
    heartbeat_prefix: &str,
    quorum_required: usize,
    timeout: Duration,
    context: &str,
    baseline_autoscale_transitions_since_cycle_start: Option<&[AutoscaleTransitionStats]>,
    baseline_autoscale_transitions_after_expansion: Option<&[AutoscaleTransitionStats]>,
    require_scale_in_transition: bool,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_storage_snapshot = Vec::new();
    let mut last_status_snapshot = Vec::new();
    let mut last_transition_snapshot = Vec::new();
    let mut last_status_error = None::<String>;
    let mut last_transition_error = None::<String>;
    let mut last_heartbeat_error = None::<String>;
    let mut last_scale_in_transition_peers_after_expansion = 0_usize;
    let mut last_scale_in_transition_peers_since_cycle_start = None::<usize>;

    while started.elapsed() <= timeout {
        let status_snapshot = match status_snapshot(network) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                last_status_error = Some(err.to_string());
                if let Some(client) = heartbeat_client {
                    if let Err(heartbeat_err) = client.submit(Log::new(
                        Level::INFO,
                        format!("{heartbeat_prefix}-{heartbeat_seq}"),
                    )) {
                        last_heartbeat_error = Some(heartbeat_err.to_string());
                    }
                    heartbeat_seq = heartbeat_seq.saturating_add(1);
                }
                thread::sleep(heartbeat_interval);
                continue;
            }
        };

        let contracted_on_quorum =
            contraction_observed_on_quorum_peers(&status_snapshot, quorum_required);
        let scale_in_transition_observed_on_quorum = if require_scale_in_transition {
            let baseline_after_expansion =
                baseline_autoscale_transitions_after_expansion.unwrap_or_default();
            match autoscale_transition_snapshot(network) {
                Ok(snapshot) => {
                    let (peers_with_scale_in_after_expansion, peers_with_scale_in_since_cycle) =
                        scale_in_transition_counts(
                            &snapshot,
                            baseline_after_expansion,
                            baseline_autoscale_transitions_since_cycle_start,
                        );
                    last_scale_in_transition_peers_after_expansion =
                        peers_with_scale_in_after_expansion;
                    last_scale_in_transition_peers_since_cycle_start =
                        peers_with_scale_in_since_cycle;
                    last_transition_snapshot = snapshot;
                    peers_with_scale_in_after_expansion >= quorum_required
                }
                Err(err) => {
                    last_transition_error = Some(err.to_string());
                    false
                }
            }
        } else {
            true
        };

        if contracted_on_quorum && scale_in_transition_observed_on_quorum {
            return Ok(());
        }

        last_status_snapshot = status_snapshot;
        last_storage_snapshot = lane_snapshot(network).unwrap_or_default();
        if let Some(client) = heartbeat_client {
            if let Err(heartbeat_err) = client.submit(Log::new(
                Level::INFO,
                format!("{heartbeat_prefix}-{heartbeat_seq}"),
            )) {
                last_heartbeat_error = Some(heartbeat_err.to_string());
            }
            heartbeat_seq = heartbeat_seq.saturating_add(1);
        }
        thread::sleep(heartbeat_interval);
    }

    let since_cycle_start_diagnostics = last_scale_in_transition_peers_since_cycle_start
        .map(|peers| format!("{peers}/{TOTAL_PEERS}"))
        .unwrap_or_else(|| "n/a".to_owned());
    Err(eyre!(
        "{context}: timed out waiting for contracted lane profile (lane {PRIMARY_LANE_ID} active; lane {ELASTIC_LANE_ID} undeclared and idle on >= {quorum_required}/{TOTAL_PEERS} peers{}) ; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last autoscale transition snapshot: {last_transition_snapshot:?}; last scale-in transition peers after expansion: {last_scale_in_transition_peers_after_expansion}/{TOTAL_PEERS}; last scale-in transition peers since cycle start: {since_cycle_start_diagnostics}; last status error: {last_status_error:?}; last transition error: {last_transition_error:?}; last heartbeat error: {last_heartbeat_error:?}",
        if require_scale_in_transition {
            " and deterministic autoscale scale-in transitions observed on quorum peers after expansion baseline"
        } else {
            ""
        }
    ))
}

fn run_expand_contract_cycle(
    network: &sandbox::SerializedNetwork,
    submitters: &[Client],
    quorum_required: usize,
    cycle_index: usize,
    attempt: usize,
    require_scale_out_transition: bool,
    require_scale_in_transition: bool,
    load_tx_count: usize,
) -> Result<ExpandContractCycleOutcome> {
    let pre_contraction_context = format!("autoscale contraction pre-check cycle {cycle_index}");
    wait_for_contracted_lanes(
        network,
        None,
        "",
        quorum_required,
        SCALE_IN_WAIT_TIMEOUT,
        &pre_contraction_context,
        None,
        None,
        false,
        CONTRACTION_HEARTBEAT_INTERVAL,
    )?;

    if should_run_cooldown_clearance(cycle_index, attempt) {
        let cooldown_probe_client = peer_client_with_timeout(network.peer());
        let cooldown_baseline_status = status_snapshot(network)?;
        let cooldown_baseline_height = max_non_empty_height(&cooldown_baseline_status);
        let cooldown_context = format!("autoscale cooldown clearance cycle {cycle_index}");
        let cooldown_prefix = format!("autoscale-cooldown-heartbeat-cycle-{cycle_index}");
        wait_for_chain_progress_with_heartbeat(
            network,
            &cooldown_probe_client,
            cooldown_baseline_height
                .saturating_add(AUTOSCALE_COOLDOWN_CLEARANCE_BLOCK_DELTA.saturating_sub(1)),
            u64::MAX,
            u64::MAX,
            AUTOSCALE_COOLDOWN_CLEARANCE_TIMEOUT,
            &cooldown_context,
            &cooldown_prefix,
            EXPANSION_PROBE_INTERVAL,
        )?;
    }

    let pre_cycle_status = status_snapshot(network)?;
    let pre_cycle_elastic_storage = elastic_lane_storage_snapshot(network)?;
    let pre_cycle_autoscale_transitions = autoscale_transition_snapshot(network)?;
    let pre_cycle_max_non_empty_height = max_non_empty_height(&pre_cycle_status);
    let pre_cycle_max_txs_approved = max_txs_approved(&pre_cycle_status);
    let pre_cycle_max_txs_rejected = max_txs_rejected(&pre_cycle_status);

    let load_started = Instant::now();
    submit_load_round_robin(submitters, load_tx_count)?;
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] load submission ({} tx): {:.3}s",
        load_tx_count,
        load_started.elapsed().as_secs_f64()
    );
    let activity_probe_client = peer_client_with_timeout(network.peer());
    let activity_context = format!("autoscale activity cycle {cycle_index}");
    let activity_prefix = format!("autoscale-activity-heartbeat-cycle-{cycle_index}");
    wait_for_chain_progress_with_heartbeat(
        network,
        &activity_probe_client,
        pre_cycle_max_non_empty_height,
        pre_cycle_max_txs_approved,
        pre_cycle_max_txs_rejected,
        SCALE_OUT_WAIT_TIMEOUT,
        &activity_context,
        &activity_prefix,
        EXPANSION_PROBE_INTERVAL,
    )?;

    let expansion_probe_client = peer_client_with_timeout(network.peer());
    let expansion_started = Instant::now();
    let expansion_context = format!("autoscale expansion cycle {cycle_index}");
    let expansion_prefix = format!("autoscale-expand-probe-cycle-{cycle_index}");
    let scale_out_timeout = if require_scale_out_transition {
        STRICT_SCALE_OUT_WAIT_TIMEOUT
    } else {
        SCALE_OUT_WAIT_TIMEOUT
    };
    wait_for_expanded_lanes_with_heartbeat(
        network,
        &expansion_probe_client,
        submitters,
        load_tx_count,
        &pre_cycle_status,
        &pre_cycle_elastic_storage,
        &pre_cycle_autoscale_transitions,
        require_scale_out_transition,
        quorum_required,
        scale_out_timeout,
        &expansion_context,
        &expansion_prefix,
        EXPANSION_PROBE_INTERVAL,
    )?;
    let expansion_time_s = expansion_started.elapsed().as_secs_f64();
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] expansion wait: {:.3}s",
        expansion_time_s
    );
    let post_expansion_autoscale_transitions = autoscale_transition_snapshot(network)?;
    let peers_with_scale_out = peers_with_scale_out_transition(
        &post_expansion_autoscale_transitions,
        &pre_cycle_autoscale_transitions,
    );
    let peers_with_scale_in_before_contraction = peers_with_scale_in_transition(
        &post_expansion_autoscale_transitions,
        &pre_cycle_autoscale_transitions,
    );
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] autoscale transition snapshot after expansion: scale-out peers with new transitions {peers_with_scale_out}/{TOTAL_PEERS}, scale-in peers since cycle start {peers_with_scale_in_before_contraction}/{TOTAL_PEERS}"
    );
    let post_expansion_status = status_snapshot(network)?;
    let require_scale_in_transition_this_cycle = should_require_scale_in_transition(
        require_scale_in_transition,
        &post_expansion_status,
        &pre_cycle_status,
        quorum_required,
    );
    if require_scale_in_transition && !require_scale_in_transition_this_cycle {
        eprintln!(
            "[autoscale-localnet][cycle {cycle_index}] scale-in transition check relaxed: expansion status signal was not observed on quorum peers; validating contraction profile only"
        );
    }

    let contraction_heartbeat_client =
        (!require_scale_in_transition_this_cycle).then(|| peer_client_with_timeout(network.peer()));
    let contraction_heartbeat_interval = if require_scale_in_transition_this_cycle {
        STRICT_CONTRACTION_HEARTBEAT_INTERVAL
    } else {
        CONTRACTION_HEARTBEAT_INTERVAL
    };
    let contraction_started = Instant::now();
    let contraction_context = format!("autoscale contraction cycle {cycle_index}");
    let contraction_prefix = format!("autoscale-heartbeat-cycle-{cycle_index}");
    wait_for_contracted_lanes(
        network,
        contraction_heartbeat_client.as_ref(),
        &contraction_prefix,
        quorum_required,
        SCALE_IN_WAIT_TIMEOUT,
        &contraction_context,
        Some(&pre_cycle_autoscale_transitions),
        Some(&post_expansion_autoscale_transitions),
        require_scale_in_transition_this_cycle,
        contraction_heartbeat_interval,
    )?;
    let contraction_time_s = contraction_started.elapsed().as_secs_f64();
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] contraction wait: {:.3}s",
        contraction_time_s
    );
    let post_contraction_autoscale_transitions = autoscale_transition_snapshot(network)?;
    let peers_with_scale_in_after_expansion = peers_with_scale_in_transition(
        &post_contraction_autoscale_transitions,
        &post_expansion_autoscale_transitions,
    );
    let peers_with_scale_in_since_cycle_start = peers_with_scale_in_transition(
        &post_contraction_autoscale_transitions,
        &pre_cycle_autoscale_transitions,
    );
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] autoscale transition snapshot after contraction: scale-in peers with new transitions after expansion {peers_with_scale_in_after_expansion}/{TOTAL_PEERS}; since cycle start {peers_with_scale_in_since_cycle_start}/{TOTAL_PEERS}"
    );
    if require_scale_in_transition_this_cycle {
        ensure!(
            peers_with_scale_in_after_expansion >= quorum_required,
            "autoscale cycle {cycle_index}: contraction profile was observed but deterministic autoscale scale-in transitions were not observed on quorum peers after expansion (scale-in peers after expansion snapshot: {peers_with_scale_in_after_expansion}/{TOTAL_PEERS}; since cycle start: {peers_with_scale_in_since_cycle_start}/{TOTAL_PEERS}; required quorum: {quorum_required})"
        );
    } else if require_scale_in_transition {
        eprintln!(
            "[autoscale-localnet][cycle {cycle_index}] scale-in transition check skipped: expansion status signal did not reach quorum during this cycle"
        );
    }
    let post_cycle_status = status_snapshot(network)?;
    let post_cycle_max_non_empty_height = max_non_empty_height(&post_cycle_status);
    let post_cycle_max_txs_approved = max_txs_approved(&post_cycle_status);
    let post_cycle_max_txs_rejected = max_txs_rejected(&post_cycle_status);
    ensure!(
        chain_progress_advanced(
            &post_cycle_status,
            pre_cycle_max_non_empty_height,
            pre_cycle_max_txs_approved,
            pre_cycle_max_txs_rejected
        ),
        "autoscale cycle {cycle_index}: chain activity did not advance (blocks_non_empty: {pre_cycle_max_non_empty_height}->{post_cycle_max_non_empty_height}, txs_approved: {pre_cycle_max_txs_approved}->{post_cycle_max_txs_approved}, txs_rejected: {pre_cycle_max_txs_rejected}->{post_cycle_max_txs_rejected})"
    );

    Ok(ExpandContractCycleOutcome {
        expansion_time_s,
        contraction_time_s,
        peers_with_scale_out_after_expansion: peers_with_scale_out,
        peers_with_scale_in_after_expansion,
        peers_with_scale_in_since_cycle_start,
    })
}

#[test]
fn nexus_autoscale_expands_and_contracts_lanes_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_expands_and_contracts_lanes_in_localnet);
    configure_load_sequence_seed(None);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    let baseline_started = Instant::now();
    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count",
    )?;
    eprintln!(
        "[autoscale-localnet] baseline lane count wait: {:.3}s",
        baseline_started.elapsed().as_secs_f64()
    );

    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum",
    )?;
    eprintln!("[autoscale-localnet] dynamic commit quorum (2f+1): {quorum_required}");

    let _cycle_outcome = run_expand_contract_cycle(
        &network,
        &submitters,
        quorum_required,
        1,
        1,
        false,
        false,
        LOAD_TX_COUNT,
    )?;
    eprintln!(
        "[autoscale-localnet] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );

    Ok(())
}

#[test]
fn nexus_autoscale_repeats_expand_contract_cycles_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_repeats_expand_contract_cycles_in_localnet);
    configure_load_sequence_seed(None);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet][multi-cycle] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    let baseline_started = Instant::now();
    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count for repeated cycles",
    )?;
    eprintln!(
        "[autoscale-localnet][multi-cycle] baseline lane count wait: {:.3}s",
        baseline_started.elapsed().as_secs_f64()
    );

    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for repeated cycles",
    )?;
    eprintln!("[autoscale-localnet][multi-cycle] dynamic commit quorum (2f+1): {quorum_required}");

    for cycle_index in 1..=2 {
        let mut cycle_attempt = 1_usize;
        loop {
            let attempt_load_tx_count = soak_cycle_load_tx_count(cycle_attempt);
            eprintln!(
                "[autoscale-localnet][multi-cycle][cycle {cycle_index}] attempt {cycle_attempt}/{AUTOSCALE_MULTI_CYCLE_RETRY_LIMIT} (load tx count: {attempt_load_tx_count})"
            );
            match run_expand_contract_cycle(
                &network,
                &submitters,
                quorum_required,
                cycle_index,
                cycle_attempt,
                true,
                true,
                attempt_load_tx_count,
            ) {
                Ok(_) => break,
                Err(err) if cycle_attempt < AUTOSCALE_MULTI_CYCLE_RETRY_LIMIT => {
                    let next_attempt = cycle_attempt.saturating_add(1);
                    let next_load_tx_count = soak_cycle_load_tx_count(next_attempt);
                    eprintln!(
                        "[autoscale-localnet][multi-cycle][cycle {cycle_index}] attempt {cycle_attempt} failed; retrying with attempt {next_attempt}/{AUTOSCALE_MULTI_CYCLE_RETRY_LIMIT} (load tx count: {next_load_tx_count}): {err}"
                    );
                    cycle_attempt = next_attempt;
                }
                Err(err) => {
                    return Err(eyre!(
                        "autoscale repeated-cycle {cycle_index} failed after {cycle_attempt} attempt(s): {err}"
                    ));
                }
            }
        }
    }

    eprintln!(
        "[autoscale-localnet][multi-cycle] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[test]
#[ignore = "long-running autoscale soak"]
fn nexus_autoscale_soak_expand_contract_cycles_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_soak_expand_contract_cycles_in_localnet);
    let soak_seed = autoscale_soak_seed();
    let load_sequence_seed = configure_load_sequence_seed(Some(&soak_seed));
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet][soak] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    let baseline_started = Instant::now();
    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count for soak",
    )?;
    eprintln!(
        "[autoscale-localnet][soak] baseline lane count wait: {:.3}s",
        baseline_started.elapsed().as_secs_f64()
    );

    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for soak",
    )?;
    eprintln!("[autoscale-localnet][soak] dynamic commit quorum (2f+1): {quorum_required}");
    eprintln!(
        "[autoscale-localnet][soak] deterministic seed ({AUTOSCALE_SOAK_SEED_ENV}): {soak_seed}; load sequence start: {load_sequence_seed}"
    );

    let mut soak_reporter = AutoscaleSoakReporter::new(&network, context)?;
    let force_fail_cycle = autoscale_soak_force_fail_cycle();
    if let Some(forced_cycle) = force_fail_cycle {
        eprintln!(
            "[autoscale-localnet][soak] forcing fail at cycle {forced_cycle} via {AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV}"
        );
    }

    let soak_started = Instant::now();
    let mut cycle_index = 1_usize;
    let mut failure_cycle = None::<usize>;
    let soak_result = 'soak: {
        while soak_started.elapsed() < AUTOSCALE_SOAK_DURATION {
            if force_fail_cycle == Some(cycle_index) {
                let forced_reason = format!(
                    "autoscale soak forced failure at cycle {cycle_index} via {AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV}"
                );
                soak_reporter.record_attempt_failure(
                    cycle_index,
                    0,
                    quorum_required,
                    &forced_reason,
                )?;
                failure_cycle = Some(cycle_index);
                break 'soak Err(eyre!(forced_reason));
            }
            soak_reporter.record_cycle_start(cycle_index, quorum_required)?;
            let mut attempt = 1_usize;
            loop {
                let attempt_load_tx_count = soak_cycle_load_tx_count(attempt);
                soak_reporter.record_attempt_start(
                    cycle_index,
                    attempt,
                    quorum_required,
                    attempt_load_tx_count,
                )?;
                eprintln!(
                    "[autoscale-localnet][soak][cycle {cycle_index}] attempt {attempt}/{AUTOSCALE_SOAK_CYCLE_RETRY_LIMIT} (load tx count: {attempt_load_tx_count})"
                );
                match run_expand_contract_cycle(
                    &network,
                    &submitters,
                    quorum_required,
                    cycle_index,
                    attempt,
                    true,
                    true,
                    attempt_load_tx_count,
                ) {
                    Ok(cycle_outcome) => {
                        soak_reporter.record_cycle_success(
                            cycle_index,
                            attempt,
                            quorum_required,
                            &cycle_outcome,
                        )?;
                        cycle_index = cycle_index.saturating_add(1);
                        break;
                    }
                    Err(err) if attempt < AUTOSCALE_SOAK_CYCLE_RETRY_LIMIT => {
                        let next_attempt = attempt.saturating_add(1);
                        let next_load_tx_count = soak_cycle_load_tx_count(next_attempt);
                        soak_reporter.record_attempt_retry(
                            cycle_index,
                            attempt,
                            quorum_required,
                            next_attempt,
                            &err.to_string(),
                        )?;
                        eprintln!(
                            "[autoscale-localnet][soak][cycle {cycle_index}] attempt {attempt} failed; retrying with attempt {next_attempt}/{AUTOSCALE_SOAK_CYCLE_RETRY_LIMIT} (load tx count: {next_load_tx_count}): {err}"
                        );
                        attempt = next_attempt;
                    }
                    Err(err) => {
                        let failure = format!(
                            "autoscale soak cycle {cycle_index} failed after {attempt} attempt(s): {err}"
                        );
                        soak_reporter.record_attempt_failure(
                            cycle_index,
                            attempt,
                            quorum_required,
                            &failure,
                        )?;
                        failure_cycle = Some(cycle_index);
                        break 'soak Err(eyre!(failure));
                    }
                }
            }
        }

        ensure!(
            cycle_index > 1,
            "autoscale soak completed without running a full cycle"
        );
        eprintln!(
            "[autoscale-localnet][soak] cycles completed: {}; soak runtime: {:.3}s; total runtime: {:.3}s",
            cycle_index - 1,
            soak_started.elapsed().as_secs_f64(),
            test_started.elapsed().as_secs_f64()
        );
        Ok(())
    };

    let summary_path = soak_reporter.summary_path().to_path_buf();
    let events_path = soak_reporter.events_path().to_path_buf();
    let finalize_result = match &soak_result {
        Ok(()) => soak_reporter.finalize("pass", None, None),
        Err(err) => soak_reporter.finalize("fail", failure_cycle, Some(err.to_string())),
    };
    if let Err(finalize_err) = finalize_result {
        return Err(match soak_result {
            Ok(()) => finalize_err,
            Err(run_err) => eyre!(
                "{run_err}; failed to persist soak artifacts (summary={}, events={}): {finalize_err}",
                summary_path.display(),
                events_path.display()
            ),
        });
    }
    soak_result
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use eyre::Result;
    use tempfile::tempdir;

    use super::{
        AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER, AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        AutoscaleSoakCycleEvent, AutoscaleSoakReporter, AutoscaleSoakRunSummary,
        AutoscaleTransitionStats, ElasticLaneStorageStats, LaneCommitmentSnapshot,
        LaneStatusSnapshot, LaneValidatorSnapshot, PeerStatusSnapshot, SoakTimingSummary,
        contraction_observed_on_quorum_peers, elastic_lane_storage_progressed,
        expansion_observed_on_quorum_or_scale_out_transition, expansion_observed_on_quorum_peers,
        expansion_observed_on_storage, expansion_observed_with_prior_scale_out_quorum_on_storage,
        expansion_probe_top_up_tx_count, expansion_scaled_top_up_tx_count,
        expansion_top_up_tx_count, parse_autoscale_transition_stats,
        peers_with_scale_in_transition, peers_with_scale_out_transition,
        scale_in_transition_counts, scale_out_transition_observed_on_quorum_peers,
        should_require_scale_in_transition, should_run_cooldown_clearance,
        soak_cycle_load_tx_count, validate_load_submission_outcome,
    };

    #[test]
    fn expansion_top_up_profile_is_deterministic() {
        assert_eq!(expansion_top_up_tx_count(0), 0);
        assert_eq!(expansion_top_up_tx_count(1), 0);
        assert_eq!(expansion_top_up_tx_count(3), 0);
        assert_eq!(expansion_top_up_tx_count(4), 16);
        assert_eq!(expansion_top_up_tx_count(8), 16);
        assert_eq!(expansion_top_up_tx_count(11), 0);
        assert_eq!(expansion_top_up_tx_count(12), 32);
        assert_eq!(expansion_top_up_tx_count(24), 32);
    }

    #[test]
    fn expansion_probe_top_up_intensifies_after_storage_grace() {
        assert_eq!(
            expansion_probe_top_up_tx_count(4, false, Duration::from_secs(8), 96),
            24
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(4, true, Duration::from_secs(7), 96),
            24
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(1, true, Duration::from_secs(8), 96),
            96
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(7, true, Duration::from_secs(15), 384),
            384
        );
    }

    #[test]
    fn expansion_scaled_top_up_respects_strict_cycle_load() {
        assert_eq!(expansion_scaled_top_up_tx_count(4, 0), 16);
        assert_eq!(expansion_scaled_top_up_tx_count(4, 96), 24);
        assert_eq!(expansion_scaled_top_up_tx_count(12, 96), 48);
        assert_eq!(expansion_scaled_top_up_tx_count(12, 384), 192);
    }

    #[test]
    fn load_submission_outcome_accepts_full_success() {
        assert!(validate_load_submission_outcome(8, 8, None).is_ok());
    }

    #[test]
    fn load_submission_outcome_accepts_partial_success() {
        assert!(validate_load_submission_outcome(8, 3, Some("peer timeout")).is_ok());
    }

    #[test]
    fn load_submission_outcome_rejects_total_failure() {
        let err = validate_load_submission_outcome(8, 0, Some("peer timeout"))
            .expect_err("all failed submissions must be rejected");
        assert!(
            err.to_string()
                .contains("all autoscale load submissions failed")
        );
    }

    #[test]
    fn load_submission_outcome_accepts_zero_attempts() {
        assert!(validate_load_submission_outcome(0, 0, None).is_ok());
    }

    #[test]
    fn soak_cycle_load_profile_escalates_per_attempt() {
        assert_eq!(
            soak_cycle_load_tx_count(0),
            super::STRICT_CYCLE_LOAD_TX_COUNT
        );
        assert_eq!(
            soak_cycle_load_tx_count(1),
            super::STRICT_CYCLE_LOAD_TX_COUNT
        );
        assert_eq!(
            soak_cycle_load_tx_count(2),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 2
        );
        assert_eq!(
            soak_cycle_load_tx_count(3),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 3
        );
        assert_eq!(
            soak_cycle_load_tx_count(4),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 4
        );
    }

    #[test]
    fn cooldown_clearance_runs_once_per_cycle() {
        assert!(!should_run_cooldown_clearance(1, 1));
        assert!(should_run_cooldown_clearance(2, 1));
        assert!(!should_run_cooldown_clearance(2, 2));
        assert!(!should_run_cooldown_clearance(2, 3));
    }

    #[test]
    fn elastic_lane_storage_progress_detects_growth_or_first_presence() {
        let baseline = ElasticLaneStorageStats {
            file_count: 3,
            total_bytes: 300,
            newest_modified_unix_ms: 123,
        };

        assert!(elastic_lane_storage_progressed(
            Some(ElasticLaneStorageStats {
                file_count: 4,
                total_bytes: 300,
                newest_modified_unix_ms: 123,
            }),
            Some(baseline),
        ));
        assert!(elastic_lane_storage_progressed(
            Some(ElasticLaneStorageStats {
                file_count: 3,
                total_bytes: 301,
                newest_modified_unix_ms: 123,
            }),
            Some(baseline),
        ));
        assert!(elastic_lane_storage_progressed(
            Some(ElasticLaneStorageStats {
                file_count: 3,
                total_bytes: 300,
                newest_modified_unix_ms: 124,
            }),
            Some(baseline),
        ));
        assert!(elastic_lane_storage_progressed(Some(baseline), None,));
    }

    #[test]
    fn elastic_lane_storage_progress_rejects_static_or_missing_state() {
        let baseline = ElasticLaneStorageStats {
            file_count: 3,
            total_bytes: 300,
            newest_modified_unix_ms: 123,
        };

        assert!(!elastic_lane_storage_progressed(
            Some(baseline),
            Some(baseline),
        ));
        assert!(!elastic_lane_storage_progressed(None, Some(baseline)));
        assert!(!elastic_lane_storage_progressed(None, None));
    }

    #[test]
    fn autoscale_transition_stats_parse_log_markers() {
        let log = format!(
            "x\n{}\ny\n{}\n{}\nz",
            AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
            AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER
        );
        let stats = parse_autoscale_transition_stats(&log);
        assert_eq!(stats.scale_out_transitions, 2);
        assert_eq!(stats.scale_in_transitions, 1);
    }

    #[test]
    fn autoscale_transition_delta_uses_peer_baseline() {
        let baseline = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 3,
            },
            AutoscaleTransitionStats::default(),
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 4,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
            },
        ];

        assert_eq!(peers_with_scale_out_transition(&current, &baseline), 2);
        assert_eq!(peers_with_scale_in_transition(&current, &baseline), 2);
    }

    #[test]
    fn strict_scale_in_quorum_passes_with_post_expansion_transitions() {
        let baseline_since_cycle_start = vec![AutoscaleTransitionStats::default(); 4];
        let baseline_after_expansion = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 2,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 2,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
        ];
        let (after_expansion, since_cycle_start) = scale_in_transition_counts(
            &current,
            &baseline_after_expansion,
            Some(&baseline_since_cycle_start),
        );
        assert_eq!(after_expansion, 3);
        assert_eq!(since_cycle_start, Some(3));
        assert!(after_expansion >= 3);
    }

    #[test]
    fn strict_scale_in_quorum_rejects_cycle_start_only_deltas() {
        let baseline_since_cycle_start = vec![AutoscaleTransitionStats::default(); 4];
        let baseline_after_expansion = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
            };
            4
        ];
        let current = baseline_after_expansion.clone();
        let (after_expansion, since_cycle_start) = scale_in_transition_counts(
            &current,
            &baseline_after_expansion,
            Some(&baseline_since_cycle_start),
        );
        assert_eq!(after_expansion, 0);
        assert_eq!(since_cycle_start, Some(4));
        assert!(after_expansion < 3);
        assert!(since_cycle_start.unwrap_or_default() >= 3);
    }

    #[test]
    fn soak_summary_serialization_contains_required_fields() {
        let summary = AutoscaleSoakRunSummary {
            test_name: "nexus_autoscale_soak_expand_contract_cycles_in_localnet".to_owned(),
            started_at_unix_ms: 100,
            ended_at_unix_ms: 200,
            duration_s: 100.0,
            cycles_completed: 4,
            attempts_total: 5,
            attempt_failures_total: 1,
            retries_used_total: 1,
            max_attempt_used_in_any_cycle: 2,
            expansion_timing: SoakTimingSummary {
                min_s: 1.0,
                avg_s: 2.0,
                max_s: 3.0,
            },
            contraction_timing: SoakTimingSummary {
                min_s: 4.0,
                avg_s: 5.0,
                max_s: 6.0,
            },
            scale_out_quorum_misses_total: 0,
            scale_in_post_expansion_quorum_misses_total: 1,
            final_result: "fail".to_owned(),
            failure_cycle: Some(3),
            failure_reason: Some("strict contraction miss".to_owned()),
        };
        let encoded = summary.to_json_value();
        let root = encoded.as_object().expect("summary json must be an object");
        assert!(root.contains_key("test_name"));
        assert!(root.contains_key("started_at_unix_ms"));
        assert!(root.contains_key("ended_at_unix_ms"));
        assert!(root.contains_key("duration_s"));
        assert!(root.contains_key("cycles_completed"));
        assert!(root.contains_key("attempts_total"));
        assert!(root.contains_key("attempt_failures_total"));
        assert!(root.contains_key("retries_used_total"));
        assert!(root.contains_key("max_attempt_used_in_any_cycle"));
        assert!(root.contains_key("expansion_time_s_min"));
        assert!(root.contains_key("expansion_time_s_avg"));
        assert!(root.contains_key("expansion_time_s_max"));
        assert!(root.contains_key("contraction_time_s_min"));
        assert!(root.contains_key("contraction_time_s_avg"));
        assert!(root.contains_key("contraction_time_s_max"));
        assert!(root.contains_key("scale_out_quorum_misses_total"));
        assert!(root.contains_key("scale_in_post_expansion_quorum_misses_total"));
        assert!(root.contains_key("final_result"));
        assert!(root.contains_key("failure_cycle"));
        assert!(root.contains_key("failure_reason"));
    }

    #[test]
    fn soak_event_serialization_contains_cycle_attempt_and_reason() {
        let event = AutoscaleSoakCycleEvent {
            event_type: "attempt_retry",
            timestamp_unix_ms: 123,
            elapsed_s: 9.0,
            cycle_index: 7,
            attempt: 3,
            quorum_required: 3,
            scale_out_transition_peers: Some(2),
            scale_in_peers_after_expansion: Some(1),
            scale_in_peers_since_cycle_start: Some(4),
            expansion_time_s: Some(2.5),
            contraction_time_s: Some(3.5),
            reason: Some("retrying after timeout".to_owned()),
        };
        let encoded = event.to_json_value();
        let root = encoded.as_object().expect("event json must be an object");
        assert_eq!(
            root.get("cycle_index")
                .and_then(norito::json::Value::as_u64),
            Some(7)
        );
        assert_eq!(
            root.get("attempt").and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            root.get("reason").and_then(norito::json::Value::as_str),
            Some("retrying after timeout")
        );
    }

    #[test]
    fn soak_reporter_flushes_artifacts_on_failure() -> Result<()> {
        let temp_dir = tempdir()?;
        let summary_path = temp_dir.path().join("autoscale_soak_summary.json");
        let events_path = temp_dir.path().join("autoscale_soak_events.jsonl");
        let mut reporter =
            AutoscaleSoakReporter::new_for_paths(summary_path.clone(), events_path.clone(), "x")?;
        reporter.record_cycle_start(1, 3)?;
        reporter.record_attempt_start(1, 1, 3, 96)?;
        reporter.record_attempt_failure(1, 1, 3, "forced failure")?;
        reporter.finalize("fail", Some(1), Some("forced failure".to_owned()))?;

        assert!(summary_path.exists(), "summary file must exist");
        assert!(events_path.exists(), "events file must exist");

        let summary_content = fs::read_to_string(&summary_path)?;
        let summary_json: norito::json::Value = norito::json::from_str(&summary_content)?;
        let summary_root = summary_json
            .as_object()
            .expect("summary payload must be object");
        assert_eq!(
            summary_root
                .get("final_result")
                .and_then(norito::json::Value::as_str),
            Some("fail")
        );

        let events_content = fs::read_to_string(&events_path)?;
        assert!(
            events_content.lines().count() >= 3,
            "expected JSONL event lines"
        );
        assert!(
            events_content.contains("\"event_type\":\"run_complete\""),
            "run_complete event must be emitted"
        );
        Ok(())
    }

    #[test]
    fn scale_in_transition_requirement_respects_request_flag() {
        let pre_cycle_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            };
            4
        ];
        let mut post_expansion_snapshot = pre_cycle_snapshot.clone();
        for peer in post_expansion_snapshot.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: 1,
                capacity: 3000,
                committed: 3,
            });
            peer.lane_governance_ids.push(1);
        }

        assert!(!should_require_scale_in_transition(
            false,
            &post_expansion_snapshot,
            &pre_cycle_snapshot,
            3
        ));
        assert!(should_require_scale_in_transition(
            true,
            &post_expansion_snapshot,
            &pre_cycle_snapshot,
            3
        ));
    }

    #[test]
    fn scale_in_transition_requirement_is_disabled_without_expansion_status_quorum() {
        let pre_cycle_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            };
            4
        ];
        let post_expansion_snapshot = pre_cycle_snapshot.clone();

        assert!(!should_require_scale_in_transition(
            true,
            &post_expansion_snapshot,
            &pre_cycle_snapshot,
            3
        ));
    }

    #[test]
    fn expansion_accepts_scale_out_transition_quorum_without_status_signal() {
        let status_without_expansion = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];
        let baseline_transitions = vec![AutoscaleTransitionStats::default(); 4];
        let transition_snapshot = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats::default(),
        ];

        assert!(!expansion_observed_on_quorum_peers(
            &status_without_expansion,
            None,
            3
        ));
        assert!(scale_out_transition_observed_on_quorum_peers(
            &transition_snapshot,
            &baseline_transitions,
            3
        ));
        assert!(expansion_observed_on_quorum_or_scale_out_transition(
            &status_without_expansion,
            None,
            &transition_snapshot,
            &baseline_transitions,
            3
        ));
        assert!(!expansion_observed_on_quorum_or_scale_out_transition(
            &status_without_expansion,
            None,
            &transition_snapshot,
            &baseline_transitions,
            4
        ));
    }

    #[test]
    fn expansion_requires_active_lane_signal_on_quorum_peers() {
        let status_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 5000,
                        committed: 4,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 4000,
                        committed: 3,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 3000,
                        committed: 2,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &status_snapshot,
            None,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &status_snapshot,
            None,
            4
        ));

        let zero_capacity_snapshot = status_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for lane in &mut peer.lanes {
                    if lane.lane_id == 1 {
                        lane.capacity = 0;
                        lane.committed = 0;
                    }
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(!expansion_observed_on_quorum_peers(
            &zero_capacity_snapshot,
            None,
            3
        ));

        let committed_only_snapshot = status_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for lane in &mut peer.lanes {
                    if lane.lane_id == 1 {
                        lane.capacity = 0;
                        lane.committed = 1;
                    }
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(expansion_observed_on_quorum_peers(
            &committed_only_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_sumeragi_lane_commitment_activity_on_quorum_peers() {
        let commitment_only_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 4,
                    teu_total: 128,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 2,
                    teu_total: 64,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 1,
                    teu_total: 32,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &commitment_only_snapshot,
            None,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &commitment_only_snapshot,
            None,
            4
        ));

        let zero_commitment_activity = commitment_only_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for commitment in &mut peer.lane_commitments {
                    commitment.tx_count = 0;
                    commitment.teu_total = 0;
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(!expansion_observed_on_quorum_peers(
            &zero_commitment_activity,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_declaration_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let declaration_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &declaration_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &declaration_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_progress_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let progress_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: baseline_snapshot[0].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[0].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[1].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[1].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[2].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[2].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[3].lanes.clone(),
                lane_commitments: baseline_snapshot[3].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[3].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &progress_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &progress_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_validator_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let validator_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: baseline_snapshot[0].lanes.clone(),
                lane_commitments: baseline_snapshot[0].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[0].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 3,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[1].lanes.clone(),
                lane_commitments: baseline_snapshot[1].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[1].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 2,
                    pending_activation: 1,
                    max_activation_epoch: 1,
                    max_activation_height: 101,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[2].lanes.clone(),
                lane_commitments: baseline_snapshot[2].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[2].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 1,
                    pending_activation: 0,
                    max_activation_epoch: 2,
                    max_activation_height: 102,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[3].lanes.clone(),
                lane_commitments: baseline_snapshot[3].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[3].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: baseline_snapshot[3].lane_validators.clone(),
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &validator_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &validator_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_storage_requires_two_lanes_on_all_peers() {
        let expanded_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                2,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        assert!(expansion_observed_on_storage(&expanded_storage));

        let partial_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (2, vec!["lane_000_default".to_owned()]),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        assert!(!expansion_observed_on_storage(&partial_storage));
    }

    #[test]
    fn strict_expansion_accepts_prior_scale_out_quorum_with_expanded_storage() {
        let expanded_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                2,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        let prior_transitions = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats::default(),
        ];

        assert!(expansion_observed_with_prior_scale_out_quorum_on_storage(
            &expanded_storage,
            &prior_transitions,
            3
        ));
        assert!(!expansion_observed_with_prior_scale_out_quorum_on_storage(
            &expanded_storage,
            &prior_transitions,
            4
        ));
    }

    #[test]
    fn strict_expansion_rejects_prior_scale_out_quorum_when_storage_not_expanded() {
        let partial_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (2, vec!["lane_000_default".to_owned()]),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        let prior_transitions = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
            },
            AutoscaleTransitionStats::default(),
        ];

        assert!(!expansion_observed_with_prior_scale_out_quorum_on_storage(
            &partial_storage,
            &prior_transitions,
            3
        ));
    }

    #[test]
    fn contraction_profile_uses_quorum_threshold() {
        let absent_elastic = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 8000,
                    committed: 10,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 7000,
                    committed: 9,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];
        assert!(contraction_observed_on_quorum_peers(&absent_elastic, 3));
        assert!(!contraction_observed_on_quorum_peers(&absent_elastic, 4));

        let inert_elastic_status_row = vec![PeerStatusSnapshot {
            lanes: vec![
                LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                },
                LaneStatusSnapshot {
                    lane_id: 1,
                    capacity: 0,
                    committed: 0,
                },
            ],
            lane_commitments: vec![],
            lane_governance_ids: vec![],
            lane_relay: vec![],
            lane_validators: vec![],
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            txs_approved: 10,
            txs_rejected: 0,
            blocks_non_empty: 10,
        }];
        assert!(contraction_observed_on_quorum_peers(
            &inert_elastic_status_row,
            1
        ));

        let elastic_still_declared_in_governance = vec![PeerStatusSnapshot {
            lanes: vec![
                LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                },
                LaneStatusSnapshot {
                    lane_id: 1,
                    capacity: 0,
                    committed: 0,
                },
            ],
            lane_commitments: vec![],
            lane_governance_ids: vec![0, 1],
            lane_relay: vec![],
            lane_validators: vec![],
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            txs_approved: 10,
            txs_rejected: 0,
            blocks_non_empty: 10,
        }];
        assert!(!contraction_observed_on_quorum_peers(
            &elastic_still_declared_in_governance,
            1
        ));

        let elastic_still_active = vec![PeerStatusSnapshot {
            lanes: vec![
                LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                },
                LaneStatusSnapshot {
                    lane_id: 1,
                    capacity: 1,
                    committed: 1,
                },
            ],
            lane_commitments: vec![],
            lane_governance_ids: vec![],
            lane_relay: vec![],
            lane_validators: vec![],
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            txs_approved: 10,
            txs_rejected: 0,
            blocks_non_empty: 10,
        }];
        assert!(!contraction_observed_on_quorum_peers(
            &elastic_still_active,
            1
        ));
    }
}
