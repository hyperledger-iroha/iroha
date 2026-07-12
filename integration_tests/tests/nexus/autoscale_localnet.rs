#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet autoscale regression test for Nexus lane expansion/contraction.

use std::{
    borrow::Cow,
    collections::{BTreeMap, btree_map::Entry},
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, TxConfirmationStatus},
    crypto::Hash,
    data_model::{
        Level,
        block::consensus::{
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            committed_lane_block_status_counts_as_progress,
        },
        isi::Log,
        nexus::{DataSpaceId, LaneId},
        prelude::{HashOf, SignedTransaction},
    },
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_test_network::{NetworkBuilder, NetworkPeer};
use toml::{Table, Value as TomlValue};

const TOTAL_PEERS: usize = 4;
const INITIAL_PROVISIONED_LANES: usize = 1;
const EXPANDED_PROVISIONED_LANES: usize = 2;
const ELASTIC_LANE_ID: u32 = 1;
const PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES: usize = 3;
const PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES: usize = 4;
const PUBLIC_PROFILE_ELASTIC_LANE_ID: u32 = 3;
const OBSERVED_AUTOSCALE_LANE_IDS: [u32; 2] = [ELASTIC_LANE_ID, PUBLIC_PROFILE_ELASTIC_LANE_ID];
const STRICT_CYCLE_LOAD_TX_COUNT: usize = 96;
const PUBLIC_PROFILE_STRICT_CYCLE_LOAD_TX_COUNT: usize = 256;
const LOCALNET_AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO: f64 = 0.05;
const LOCALNET_AUTOSCALE_SCALE_IN_UTILIZATION_RATIO: f64 = 0.04;
const LOCALNET_AUTOSCALE_PER_LANE_TARGET_TPS: i64 = 100;
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
const STRICT_CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);
const SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const STRICT_SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const AUTOSCALE_SOAK_DURATION: Duration = Duration::from_secs(30 * 60);
const AUTOSCALE_SOAK_CYCLE_RETRY_LIMIT: usize = 4;
const AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT: usize = 4;
const AUTOSCALE_MULTI_CYCLE_RETRY_LIMIT: usize = 4;
const AUTOSCALE_SOAK_DEFAULT_SEED: &str = "autoscale-localnet-soak-default";
const AUTOSCALE_SOAK_SEED_ENV: &str = "IROHA_AUTOSCALE_SOAK_SEED";
const AUTOSCALE_SOAK_DURATION_ENV: &str = "IROHA_AUTOSCALE_SOAK_DURATION_SECS";
const AUTOSCALE_SOAK_ARTIFACT_DIR_ENV: &str = "IROHA_AUTOSCALE_SOAK_ARTIFACT_DIR";
const AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV: &str = "IROHA_AUTOSCALE_SOAK_FORCE_FAIL_CYCLE";
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const QUORUM_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const SUBMISSION_READY_TIMEOUT: Duration = Duration::from_secs(180);
const SUBMISSION_READY_POLL: Duration = Duration::from_millis(500);
const STATUS_SNAPSHOT_RETRY_LIMIT: u32 = 5;
const STATUS_SNAPSHOT_RETRY_BACKOFF: Duration = Duration::from_millis(250);
const LOAD_ACTIVITY_SAMPLE_LIMIT_PER_CLIENT: usize = 1;
static AUTOSCALE_LOCALNET_TEST_MUTEX: Mutex<()> = Mutex::new(());
static LOAD_TX_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-out transition";
const AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-in transition";

fn lane_descriptor(index: i64, alias: &str) -> Table {
    let mut lane = Table::new();
    lane.insert("index".into(), TomlValue::Integer(index));
    lane.insert("alias".into(), TomlValue::String(alias.to_owned()));
    lane.insert("metadata".into(), TomlValue::Table(Table::new()));
    lane
}

fn public_profile_lane_catalog() -> TomlValue {
    TomlValue::Array(vec![
        TomlValue::Table(lane_descriptor(0, "core")),
        TomlValue::Table(lane_descriptor(1, "governance")),
        TomlValue::Table(lane_descriptor(2, "zk")),
    ])
}

fn autoscale_localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .with_pipeline_time(Duration::from_millis(300))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "autoscale", "enabled"], true)
                .write(["nexus", "autoscale", "min_lanes"], 1_i64)
                .write(["nexus", "autoscale", "max_lanes"], 2_i64)
                .write(["nexus", "autoscale", "target_block_ms"], 60000_i64)
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 1.20_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.80_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    LOCALNET_AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    LOCALNET_AUTOSCALE_SCALE_IN_UTILIZATION_RATIO,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 2_i64)
                .write(["nexus", "autoscale", "scale_in_window_blocks"], 4_i64)
                .write(["nexus", "autoscale", "cooldown_blocks"], 1_i64)
                .write(
                    ["nexus", "autoscale", "per_lane_target_tps"],
                    LOCALNET_AUTOSCALE_PER_LANE_TARGET_TPS,
                );
        })
}

fn autoscale_public_profile_localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .with_pipeline_time(Duration::from_millis(300))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 3_i64)
                .write(["nexus", "lane_catalog"], public_profile_lane_catalog())
                .write(["nexus", "autoscale", "enabled"], true)
                .write(["nexus", "autoscale", "min_lanes"], 3_i64)
                .write(["nexus", "autoscale", "max_lanes"], 4_i64)
                .write(["nexus", "autoscale", "target_block_ms"], 120000_i64)
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 1.20_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.80_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    0.05_f64,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    0.04_f64,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 2_i64)
                .write(["nexus", "autoscale", "scale_in_window_blocks"], 4_i64)
                .write(["nexus", "autoscale", "cooldown_blocks"], 1_i64)
                .write(["nexus", "autoscale", "per_lane_target_tps"], 32_i64);
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
    scale_out_ambiguous_heights: u64,
    scale_in_ambiguous_heights: u64,
}

#[derive(Clone, Debug, Default)]
struct ExpandContractCycleOutcome {
    expansion_time_s: f64,
    contraction_time_s: f64,
    peers_with_scale_out_after_expansion: usize,
    peers_with_direct_applied_committed_lane_block_after_expansion: usize,
    peers_with_scale_in_after_expansion: usize,
    peers_with_scale_in_since_cycle_start: usize,
    scale_in_transition_required: bool,
}

#[derive(Clone, Debug)]
struct LoadSubmissionSample {
    client_index: usize,
    hash: HashOf<SignedTransaction>,
}

#[derive(Clone, Debug, Default)]
struct LoadSubmissionReport {
    attempted: usize,
    submitted: usize,
    per_client_submitted: Vec<usize>,
    samples: Vec<LoadSubmissionSample>,
    first_error: Option<String>,
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
    quorum_required_max: usize,
    successful_scale_out_min_peers: Option<usize>,
    direct_applied_committed_lane_block_cycle_count: usize,
    direct_applied_committed_lane_block_min_peers: Option<usize>,
    required_scale_in_cycle_count: usize,
    required_scale_in_min_quorum_peers: Option<usize>,
    optional_scale_in_cycle_count: usize,
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
            "quorum_required_max".into(),
            norito::json::Value::from(usize_to_u64(self.quorum_required_max)),
        );
        map.insert(
            "successful_scale_out_min_peers".into(),
            self.successful_scale_out_min_peers
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "direct_applied_committed_lane_block_cycle_count".into(),
            norito::json::Value::from(usize_to_u64(
                self.direct_applied_committed_lane_block_cycle_count,
            )),
        );
        map.insert(
            "direct_applied_committed_lane_block_min_peers".into(),
            self.direct_applied_committed_lane_block_min_peers
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "required_scale_in_cycle_count".into(),
            norito::json::Value::from(usize_to_u64(self.required_scale_in_cycle_count)),
        );
        map.insert(
            "required_scale_in_min_quorum_peers".into(),
            self.required_scale_in_min_quorum_peers
                .map(usize_to_u64)
                .map(norito::json::Value::from)
                .unwrap_or(norito::json::Value::Null),
        );
        map.insert(
            "optional_scale_in_cycle_count".into(),
            norito::json::Value::from(usize_to_u64(self.optional_scale_in_cycle_count)),
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
    direct_applied_committed_lane_block_peers: Option<usize>,
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
            "direct_applied_committed_lane_block_peers".into(),
            self.direct_applied_committed_lane_block_peers
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
    quorum_required_max: usize,
    successful_scale_out_min_peers: Option<usize>,
    direct_applied_committed_lane_block_cycle_count: usize,
    direct_applied_committed_lane_block_min_peers: Option<usize>,
    required_scale_in_cycle_count: usize,
    required_scale_in_min_quorum_peers: Option<usize>,
    optional_scale_in_cycle_count: usize,
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
            quorum_required_max: 0,
            successful_scale_out_min_peers: None,
            direct_applied_committed_lane_block_cycle_count: 0,
            direct_applied_committed_lane_block_min_peers: None,
            required_scale_in_cycle_count: 0,
            required_scale_in_min_quorum_peers: None,
            optional_scale_in_cycle_count: 0,
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
            direct_applied_committed_lane_block_peers: None,
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
            direct_applied_committed_lane_block_peers: None,
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
            direct_applied_committed_lane_block_peers: None,
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
            direct_applied_committed_lane_block_peers: None,
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
        ensure!(
            cycle_outcome.peers_with_scale_out_after_expansion >= quorum_required,
            "autoscale soak cycle {cycle_index} attempt {attempt}: fresh scale-out transition quorum miss ({}/{TOTAL_PEERS}; required {quorum_required}) must fail the soak instead of being summarized as success",
            cycle_outcome.peers_with_scale_out_after_expansion,
        );
        let scale_in_transition_quorum_met = scale_in_transition_quorum_satisfied(
            cycle_outcome.peers_with_scale_in_after_expansion,
            Some(cycle_outcome.peers_with_scale_in_since_cycle_start),
            quorum_required,
        );
        ensure!(
            !cycle_outcome.scale_in_transition_required || scale_in_transition_quorum_met,
            "autoscale soak cycle {cycle_index} attempt {attempt}: required scale-in transition quorum miss after contraction (after expansion: {}/{TOTAL_PEERS}; since cycle start: {}/{TOTAL_PEERS}; required {quorum_required}) must fail the soak instead of being summarized as success",
            cycle_outcome.peers_with_scale_in_after_expansion,
            cycle_outcome.peers_with_scale_in_since_cycle_start,
        );
        self.cycles_completed = self.cycles_completed.saturating_add(1);
        self.expansion_times_s.push(cycle_outcome.expansion_time_s);
        self.contraction_times_s
            .push(cycle_outcome.contraction_time_s);
        self.quorum_required_max = self.quorum_required_max.max(quorum_required);
        self.successful_scale_out_min_peers = Some(
            self.successful_scale_out_min_peers
                .map_or(cycle_outcome.peers_with_scale_out_after_expansion, |min| {
                    min.min(cycle_outcome.peers_with_scale_out_after_expansion)
                }),
        );
        if cycle_outcome.peers_with_direct_applied_committed_lane_block_after_expansion > 0 {
            self.direct_applied_committed_lane_block_cycle_count = self
                .direct_applied_committed_lane_block_cycle_count
                .saturating_add(1);
            self.direct_applied_committed_lane_block_min_peers =
                Some(self.direct_applied_committed_lane_block_min_peers.map_or(
                    cycle_outcome.peers_with_direct_applied_committed_lane_block_after_expansion,
                    |min| {
                        min.min(
                            cycle_outcome
                                .peers_with_direct_applied_committed_lane_block_after_expansion,
                        )
                    },
                ));
        }
        if cycle_outcome.scale_in_transition_required {
            self.required_scale_in_cycle_count =
                self.required_scale_in_cycle_count.saturating_add(1);
            let scale_in_quorum_peers = cycle_outcome
                .peers_with_scale_in_after_expansion
                .max(cycle_outcome.peers_with_scale_in_since_cycle_start);
            self.required_scale_in_min_quorum_peers = Some(
                self.required_scale_in_min_quorum_peers
                    .map_or(scale_in_quorum_peers, |min| min.min(scale_in_quorum_peers)),
            );
        } else {
            self.optional_scale_in_cycle_count =
                self.optional_scale_in_cycle_count.saturating_add(1);
        }

        if cycle_outcome.peers_with_scale_out_after_expansion < quorum_required {
            self.scale_out_quorum_misses_total =
                self.scale_out_quorum_misses_total.saturating_add(1);
        }
        if cycle_outcome.scale_in_transition_required && !scale_in_transition_quorum_met {
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
            direct_applied_committed_lane_block_peers: Some(
                cycle_outcome.peers_with_direct_applied_committed_lane_block_after_expansion,
            ),
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
            direct_applied_committed_lane_block_peers: Some(
                cycle_outcome.peers_with_direct_applied_committed_lane_block_after_expansion,
            ),
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
            direct_applied_committed_lane_block_peers: Some(
                cycle_outcome.peers_with_direct_applied_committed_lane_block_after_expansion,
            ),
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
            direct_applied_committed_lane_block_peers: None,
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
            quorum_required_max: self.quorum_required_max,
            successful_scale_out_min_peers: self.successful_scale_out_min_peers,
            direct_applied_committed_lane_block_cycle_count: self
                .direct_applied_committed_lane_block_cycle_count,
            direct_applied_committed_lane_block_min_peers: self
                .direct_applied_committed_lane_block_min_peers,
            required_scale_in_cycle_count: self.required_scale_in_cycle_count,
            required_scale_in_min_quorum_peers: self.required_scale_in_min_quorum_peers,
            optional_scale_in_cycle_count: self.optional_scale_in_cycle_count,
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

fn peer_elastic_lane_storage_stats(
    peer: &NetworkPeer,
    lane_id: u32,
) -> Result<Option<ElasticLaneStorageStats>> {
    let blocks_root = peer.kura_store_dir().join("blocks");
    if !blocks_root.exists() {
        return Ok(None);
    }
    for entry in fs::read_dir(&blocks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if is_autoscale_elastic_storage_segment(&name, lane_id) {
            return collect_directory_tree_stats(&entry.path()).map(Some);
        }
    }
    Ok(None)
}

fn elastic_lane_storage_snapshot(
    network: &sandbox::SerializedNetwork,
    lane_id: u32,
) -> Result<Vec<Option<ElasticLaneStorageStats>>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            peer_elastic_lane_storage_stats(peer, lane_id)
                .map_err(|err| eyre!("read elastic lane {lane_id} storage on peer {index}: {err}"))
        })
        .collect()
}

fn elastic_lane_storage_progressed(
    current: Option<ElasticLaneStorageStats>,
    baseline: Option<ElasticLaneStorageStats>,
) -> bool {
    match (current, baseline) {
        (Some(current), Some(baseline)) => {
            current.file_count > baseline.file_count || current.total_bytes > baseline.total_bytes
        }
        (Some(_), None) => true,
        _ => false,
    }
}

fn peers_with_elastic_storage_progress(
    current: &[Option<ElasticLaneStorageStats>],
    baseline: &[Option<ElasticLaneStorageStats>],
) -> usize {
    if current.len() != TOTAL_PEERS || baseline.len() != current.len() {
        return 0;
    }
    current
        .iter()
        .zip(baseline.iter())
        .filter(|(current, baseline)| elastic_lane_storage_progressed(**current, **baseline))
        .count()
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
        ..AutoscaleTransitionStats::default()
    }
}

fn strip_ansi_escape_codes(input: &str) -> Cow<'_, str> {
    if !input.as_bytes().contains(&0x1B) {
        return Cow::Borrowed(input);
    }
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1B' && chars.next_if_eq(&'[').is_some() {
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    Cow::Owned(output)
}

fn line_unique_unsigned_field(line: &str, field: &str) -> Option<u64> {
    match line_unsigned_field_occurrences(line, field).as_slice() {
        [Some(value)] => Some(*value),
        _ => None,
    }
}

fn line_has_unique_unsigned_field(line: &str, field: &str, expected: u64) -> bool {
    matches!(
        line_unsigned_field_occurrences(line, field).as_slice(),
        [Some(value)] if *value == expected
    )
}

fn line_unsigned_field_occurrences(line: &str, field: &str) -> Vec<Option<u64>> {
    let prefixes = [
        format!("{field}="),
        format!("{field}:"),
        format!("\"{field}\":"),
    ];
    let mut values = Vec::new();
    for prefix in prefixes {
        for (offset, _) in line.match_indices(prefix.as_str()) {
            if line_offset_inside_quoted_or_keyed_value(line, offset) {
                continue;
            }
            if offset > 0
                && line[..offset].chars().next_back().is_some_and(|previous| {
                    !previous.is_ascii_whitespace() && !matches!(previous, '{' | '[' | '(' | ',')
                })
            {
                continue;
            }
            values.push(parse_unsigned_field_value(&line[offset + prefix.len()..]));
        }
    }
    values
}

fn line_offset_inside_quoted_or_keyed_value(line: &str, offset: usize) -> bool {
    let mut quote = None;
    let mut escaped = false;
    let mut containers = Vec::new();
    for (index, ch) in line.char_indices() {
        if index >= offset {
            break;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active_quote) = quote {
            match ch {
                '\\' => escaped = true,
                quote_ch if quote_ch == active_quote => quote = None,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => containers.push((')', line_opener_starts_keyed_value(line, index))),
            '[' => containers.push((']', line_opener_starts_keyed_value(line, index))),
            '{' => containers.push(('}', line_opener_starts_keyed_value(line, index))),
            ')' | ']' | '}' => {
                if containers
                    .last()
                    .is_some_and(|(expected, _)| *expected == ch)
                {
                    containers.pop();
                }
            }
            _ => {}
        }
    }
    quote.is_some() || containers.iter().any(|(_, keyed_value)| *keyed_value)
}

fn line_opener_starts_keyed_value(line: &str, opener_offset: usize) -> bool {
    let before_opener = line[..opener_offset].trim_end_matches(|ch: char| ch.is_ascii_whitespace());
    let Some(delimiter) = before_opener.chars().next_back() else {
        return false;
    };
    if !matches!(delimiter, '=' | ':') {
        return false;
    }
    before_opener[..before_opener.len() - delimiter.len_utf8()]
        .trim_end_matches(|ch: char| ch.is_ascii_whitespace())
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '"' || ch == '\'' || ch.is_ascii_alphanumeric() || ch == '_')
}

fn parse_unsigned_field_value(raw: &str) -> Option<u64> {
    let value = raw.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
    let digit_len = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .map(char::len_utf8)
        .sum::<usize>();
    if digit_len == 0 {
        return None;
    }
    let digits = &value[..digit_len];
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    let parsed = digits.parse::<u64>().ok()?;
    value[digit_len..]
        .chars()
        .next()
        .is_none_or(|next| {
            next.is_ascii_whitespace() || matches!(next, ',' | ';' | '}' | ']' | ')')
        })
        .then_some(parsed)
}

fn line_has_lane_field(line: &str, lane_id: u32) -> bool {
    line_has_unique_unsigned_field(line, "lane", u64::from(lane_id))
}

fn line_has_transition_marker(line: &str, marker: &str) -> bool {
    line.match_indices(marker).any(|(offset, _)| {
        let prefix = &line[..offset];
        let suffix = &line[offset + marker.len()..];
        if !line_marker_suffix_is_boundary(suffix) {
            return false;
        }
        if line_marker_prefix_is_message_field(prefix)
            || line_marker_prefix_is_tracing_target(prefix)
        {
            return true;
        }
        !line_offset_inside_quoted_or_keyed_value(line, offset)
            && prefix
                .chars()
                .next_back()
                .is_none_or(|ch| ch.is_ascii_whitespace())
            && prefix
                .chars()
                .rev()
                .find(|ch| !ch.is_ascii_whitespace())
                .is_none_or(|previous| {
                    !matches!(previous, '=' | ':' | '"' | '\'' | '(' | '[' | '{')
                })
    })
}

fn line_marker_prefix_is_tracing_target(prefix: &str) -> bool {
    let trimmed = prefix.trim_end_matches(|ch: char| ch.is_ascii_whitespace());
    let Some(before_target_colon) = trimmed.strip_suffix(':') else {
        return false;
    };
    let mut tokens = before_target_colon.split_ascii_whitespace().rev();
    let Some(target) = tokens.next() else {
        return false;
    };
    let Some(level) = tokens.next() else {
        return false;
    };
    line_log_level_token(level) && line_rust_target_token(target)
}

fn line_log_level_token(token: &str) -> bool {
    matches!(token, "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR")
}

fn line_rust_target_token(token: &str) -> bool {
    token.contains("::")
        && !token.starts_with("::")
        && !token.ends_with("::")
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
}

fn line_marker_suffix_is_boundary(suffix: &str) -> bool {
    suffix
        .chars()
        .next()
        .is_none_or(|next| matches!(next, '"' | '\'' | ',' | ';' | '}' | ']' | ')'))
}

fn line_marker_prefix_is_message_field(prefix: &str) -> bool {
    let trimmed = prefix.trim_end_matches(|ch: char| ch.is_ascii_whitespace());
    message_field_prefix_has_suffix(trimmed, "\"message\":\"")
        || message_field_prefix_has_suffix(trimmed, "\"msg\":\"")
        || message_field_prefix_has_suffix(trimmed, "message=\"")
        || message_field_prefix_has_suffix(trimmed, "msg=\"")
        || message_field_prefix_has_suffix(trimmed, "message=")
        || message_field_prefix_has_suffix(trimmed, "msg=")
        || message_field_prefix_has_suffix(trimmed, "message:")
        || message_field_prefix_has_suffix(trimmed, "msg:")
}

fn message_field_prefix_has_suffix(prefix: &str, suffix: &str) -> bool {
    let Some(before) = prefix.strip_suffix(suffix) else {
        return false;
    };
    before.chars().next_back().is_none_or(|previous| {
        previous.is_ascii_whitespace() || matches!(previous, '{' | '[' | '(' | ',')
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AutoscaleTransitionLogFingerprint {
    active_lanes: u64,
    autoscale_capacity_lanes: u64,
    latency_ratio_permille: u64,
    utilization_permille: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AutoscaleTransitionHeightCounts {
    unambiguous: u64,
    ambiguous: u64,
}

fn autoscale_transition_log_fingerprint(
    line: &str,
    lane_id: u32,
    marker: &str,
    latency_field: &str,
    utilization_field: &str,
) -> Option<(u64, AutoscaleTransitionLogFingerprint)> {
    if !line_has_transition_marker(line, marker) || !line_has_lane_field(line, lane_id) {
        return None;
    }

    let height = line_unique_unsigned_field(line, "height")?;
    let active_lanes = line_unique_unsigned_field(line, "active_lanes")?;
    let autoscale_capacity_lanes = line_unique_unsigned_field(line, "autoscale_capacity_lanes")?;
    if active_lanes == 0 || active_lanes != autoscale_capacity_lanes {
        return None;
    }

    Some((
        height,
        AutoscaleTransitionLogFingerprint {
            active_lanes,
            autoscale_capacity_lanes,
            latency_ratio_permille: line_unique_unsigned_field(line, latency_field)?,
            utilization_permille: line_unique_unsigned_field(line, utilization_field)?,
        },
    ))
}

fn autoscale_transition_height_counts(
    log_contents: &str,
    lane_id: u32,
    marker: &str,
    latency_field: &str,
    utilization_field: &str,
) -> AutoscaleTransitionHeightCounts {
    let mut heights = BTreeMap::<u64, Option<AutoscaleTransitionLogFingerprint>>::new();

    for raw_line in log_contents.lines() {
        let line = strip_ansi_escape_codes(raw_line);
        let Some((height, fingerprint)) = autoscale_transition_log_fingerprint(
            line.as_ref(),
            lane_id,
            marker,
            latency_field,
            utilization_field,
        ) else {
            continue;
        };

        match heights.entry(height) {
            Entry::Vacant(entry) => {
                entry.insert(Some(fingerprint));
            }
            Entry::Occupied(mut entry) => {
                if entry.get().is_some_and(|current| current == fingerprint) {
                    continue;
                }
                entry.insert(None);
            }
        }
    }

    AutoscaleTransitionHeightCounts {
        unambiguous: u64::try_from(heights.values().filter(|value| value.is_some()).count())
            .unwrap_or(u64::MAX),
        ambiguous: u64::try_from(heights.values().filter(|value| value.is_none()).count())
            .unwrap_or(u64::MAX),
    }
}

fn parse_autoscale_transition_stats_for_lane(
    log_contents: &str,
    lane_id: u32,
) -> AutoscaleTransitionStats {
    let scale_out_counts = autoscale_transition_height_counts(
        log_contents,
        lane_id,
        AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        "out_latency_ratio_permille",
        "out_utilization_p95_permille",
    );
    let scale_in_counts = autoscale_transition_height_counts(
        log_contents,
        lane_id,
        AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        "in_latency_ratio_permille",
        "in_utilization_p95_permille",
    );
    AutoscaleTransitionStats {
        scale_out_transitions: scale_out_counts.unambiguous,
        scale_in_transitions: scale_in_counts.unambiguous,
        scale_out_ambiguous_heights: scale_out_counts.ambiguous,
        scale_in_ambiguous_heights: scale_in_counts.ambiguous,
    }
}

fn peer_autoscale_transition_stats_for_lane(
    peer: &NetworkPeer,
    lane_id: u32,
) -> Result<AutoscaleTransitionStats> {
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
                    "read lane-specific autoscale transition log {:?}: {error}",
                    stdout_log_path
                )
            });
        }
    };
    Ok(parse_autoscale_transition_stats_for_lane(
        &log_contents,
        lane_id,
    ))
}

fn autoscale_transition_snapshot_for_lane(
    network: &sandbox::SerializedNetwork,
    lane_id: u32,
) -> Result<Vec<AutoscaleTransitionStats>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            peer_autoscale_transition_stats_for_lane(peer, lane_id).map_err(|err| {
                eyre!("read lane-specific autoscale transition stats on peer {index}: {err}")
            })
        })
        .collect()
}

fn peers_with_scale_out_transition(
    current: &[AutoscaleTransitionStats],
    baseline: &[AutoscaleTransitionStats],
) -> usize {
    if current.len() != baseline.len() {
        return 0;
    }

    current
        .iter()
        .enumerate()
        .filter(|(index, current)| {
            baseline.get(*index).is_some_and(|stats| {
                current.scale_out_ambiguous_heights == 0
                    && stats.scale_out_ambiguous_heights == 0
                    && current.scale_out_transitions > stats.scale_out_transitions
            })
        })
        .count()
}

fn peers_with_scale_in_transition(
    current: &[AutoscaleTransitionStats],
    baseline: &[AutoscaleTransitionStats],
) -> usize {
    if current.len() != baseline.len() {
        return 0;
    }

    current
        .iter()
        .enumerate()
        .filter(|(index, current)| {
            baseline.get(*index).is_some_and(|stats| {
                current.scale_in_ambiguous_heights == 0
                    && stats.scale_in_ambiguous_heights == 0
                    && current.scale_in_transitions > stats.scale_in_transitions
            })
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

fn scale_in_transition_quorum_satisfied(
    peers_after_expansion: usize,
    peers_since_cycle_start: Option<usize>,
    quorum_required: usize,
) -> bool {
    peers_after_expansion >= quorum_required
        || peers_since_cycle_start.is_some_and(|peers| peers >= quorum_required)
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
    dataspace_id: u64,
    block_height: u64,
    descriptor_hash: Option<Hash>,
    merge_admissible: bool,
}

#[derive(Clone, Debug)]
struct CommittedLaneBlockSnapshot {
    lane_id: u32,
    dataspace_id: u64,
    lane_block_height: u64,
    lane_block_view: u64,
    descriptor_hash: Hash,
    proposal_hash: Hash,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    qc_mode_tag: String,
    execution_status: String,
    executable_payload_available: bool,
    validator_count: u32,
    min_quorum: u32,
    prepare_qc_signer_count: u32,
    commit_qc_signer_count: u32,
}

#[derive(Clone, Debug)]
struct LaneValidatorSnapshot {
    lane_id: u32,
    total: u64,
    active: u64,
    pending_activation: u64,
    jailed: u64,
    exiting: u64,
    max_activation_epoch: u64,
    max_activation_height: u64,
}

#[derive(Clone, Copy, Default)]
struct ProposalGateSnapshot {
    height: u64,
    view: u64,
    queue_len: u64,
    pending_blocks_total: u64,
    pending_blocks_blocking: u64,
    active_pending_for_tip: u64,
    queue_saturated: bool,
    active_pending: bool,
    rbc_backlog: bool,
    relay_backpressure: bool,
    consensus_queue_backpressure: bool,
    should_defer: bool,
    only_pacing_backpressure: bool,
    commit_inflight_active: bool,
    cached_proposal_present: bool,
    cached_proposal_hint_present: bool,
    round_liveness_present: bool,
    frontier_owner_present: bool,
    missing_qc_liveness_active: bool,
    last_pacemaker_attempt_age_ms: u64,
    last_successful_proposal_age_ms: u64,
}

impl std::fmt::Debug for ProposalGateSnapshot {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("ProposalGateSnapshot")
            .field("height", &self.height)
            .field("view", &self.view)
            .field("queue_len", &self.queue_len)
            .field("pending_blocks_total", &self.pending_blocks_total)
            .field("pending_blocks_blocking", &self.pending_blocks_blocking)
            .field("active_pending_for_tip", &self.active_pending_for_tip)
            .field("queue_saturated", &self.queue_saturated)
            .field("active_pending", &self.active_pending)
            .field("rbc_backlog", &self.rbc_backlog)
            .field("relay_backpressure", &self.relay_backpressure)
            .field(
                "consensus_queue_backpressure",
                &self.consensus_queue_backpressure,
            )
            .field("should_defer", &self.should_defer)
            .field("only_pacing_backpressure", &self.only_pacing_backpressure)
            .field("commit_inflight_active", &self.commit_inflight_active)
            .field("cached_proposal_present", &self.cached_proposal_present)
            .field(
                "cached_proposal_hint_present",
                &self.cached_proposal_hint_present,
            )
            .field("round_liveness_present", &self.round_liveness_present)
            .field("frontier_owner_present", &self.frontier_owner_present)
            .field(
                "missing_qc_liveness_active",
                &self.missing_qc_liveness_active,
            )
            .field(
                "last_pacemaker_attempt_age_ms",
                &self.last_pacemaker_attempt_age_ms,
            )
            .field(
                "last_successful_proposal_age_ms",
                &self.last_successful_proposal_age_ms,
            )
            .finish()
    }
}

#[derive(Clone, Default)]
struct PeerStatusSnapshot {
    lanes: Vec<LaneStatusSnapshot>,
    lane_commitments: Vec<LaneCommitmentSnapshot>,
    lane_governance_ids: Vec<u32>,
    lane_relay: Vec<LaneRelaySnapshot>,
    committed_lane_blocks: Vec<CommittedLaneBlockSnapshot>,
    lane_validators: Vec<LaneValidatorSnapshot>,
    commit_signatures_required: u64,
    commit_qc_validator_set_len: u64,
    tx_queue_depth: u64,
    tx_queue_capacity: u64,
    tx_queue_saturated: bool,
    tx_queue_saturated_by_count: bool,
    tx_queue_saturated_by_bytes: bool,
    tx_queue_saturated_by_age: bool,
    tx_queue_oldest_queued_age_ms: u64,
    commit_inflight_active: bool,
    commit_inflight_height: u64,
    commit_inflight_view: u64,
    commit_inflight_elapsed_ms: u64,
    commit_inflight_timeout_total: u64,
    pending_rbc_entries: u64,
    pacemaker_backpressure_deferrals_total: u64,
    proposal_gate: ProposalGateSnapshot,
    txs_approved: u64,
    txs_rejected: u64,
    blocks_non_empty: u64,
}

impl std::fmt::Debug for PeerStatusSnapshot {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("PeerStatusSnapshot")
            .field("lanes", &self.lanes)
            .field("lane_commitments", &self.lane_commitments)
            .field("lane_governance_ids", &self.lane_governance_ids)
            .field("lane_relay", &self.lane_relay)
            .field("committed_lane_blocks", &self.committed_lane_blocks)
            .field("lane_validators", &self.lane_validators)
            .field(
                "commit_signatures_required",
                &self.commit_signatures_required,
            )
            .field(
                "commit_qc_validator_set_len",
                &self.commit_qc_validator_set_len,
            )
            .field("tx_queue_depth", &self.tx_queue_depth)
            .field("tx_queue_capacity", &self.tx_queue_capacity)
            .field("tx_queue_saturated", &self.tx_queue_saturated)
            .field(
                "tx_queue_saturated_by_count",
                &self.tx_queue_saturated_by_count,
            )
            .field(
                "tx_queue_saturated_by_bytes",
                &self.tx_queue_saturated_by_bytes,
            )
            .field("tx_queue_saturated_by_age", &self.tx_queue_saturated_by_age)
            .field(
                "tx_queue_oldest_queued_age_ms",
                &self.tx_queue_oldest_queued_age_ms,
            )
            .field("commit_inflight_active", &self.commit_inflight_active)
            .field("commit_inflight_height", &self.commit_inflight_height)
            .field("commit_inflight_view", &self.commit_inflight_view)
            .field(
                "commit_inflight_elapsed_ms",
                &self.commit_inflight_elapsed_ms,
            )
            .field(
                "commit_inflight_timeout_total",
                &self.commit_inflight_timeout_total,
            )
            .field("pending_rbc_entries", &self.pending_rbc_entries)
            .field(
                "pacemaker_backpressure_deferrals_total",
                &self.pacemaker_backpressure_deferrals_total,
            )
            .field("proposal_gate", &self.proposal_gate)
            .field("txs_approved", &self.txs_approved)
            .field("txs_rejected", &self.txs_rejected)
            .field("blocks_non_empty", &self.blocks_non_empty)
            .finish()
    }
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
    let mut jailed = 0_u64;
    let mut exiting = 0_u64;
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
                Some("Jailed") => jailed = jailed.saturating_add(1),
                Some("Exiting") => exiting = exiting.saturating_add(1),
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
        jailed,
        exiting,
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
                    jailed: 0,
                    exiting: 0,
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
            let sumeragi_status = client.get_sumeragi_status().ok();
            let lane_commitments = sumeragi_status
                .as_ref()
                .map(|sumeragi_status| {
                    sumeragi_status
                        .lane_commitments
                        .iter()
                        .map(|lane| LaneCommitmentSnapshot {
                            lane_id: lane.lane_id.as_u32(),
                            block_height: lane.block_height,
                            tx_count: lane.tx_count,
                            teu_total: lane.teu_total,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let lane_governance_ids = sumeragi_status
                .as_ref()
                .map(|sumeragi_status| {
                    sumeragi_status
                        .lane_governance
                        .iter()
                        .map(|lane| lane.lane_id.as_u32())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let lane_relay = sumeragi_status
                .as_ref()
                .map(|sumeragi_status| {
                    sumeragi_status
                        .lane_relay_envelopes
                        .iter()
                        .map(|lane| LaneRelaySnapshot {
                            lane_id: lane.lane_id.as_u32(),
                            dataspace_id: lane.dataspace_id.as_u64(),
                            block_height: lane.block_height,
                            descriptor_hash: lane.lane_block_descriptor_hash,
                            merge_admissible: lane.is_merge_admissible(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let committed_lane_blocks = sumeragi_status
                .as_ref()
                .map(|sumeragi_status| {
                    sumeragi_status
                        .committed_lane_blocks
                        .iter()
                        .map(|entry| CommittedLaneBlockSnapshot {
                            lane_id: entry.lane_id.as_u32(),
                            dataspace_id: entry.dataspace_id.as_u64(),
                            lane_block_height: entry.lane_block_height,
                            lane_block_view: entry.lane_block_view,
                            descriptor_hash: entry.descriptor_hash,
                            proposal_hash: entry.proposal_hash,
                            subject_hash: entry.subject_hash,
                            payload_ownership_hash: entry.payload_ownership_hash,
                            rbc_instance_hash: entry.rbc_instance_hash,
                            qc_mode_tag: entry.qc_mode_tag.clone(),
                            execution_status: entry.execution_status.clone(),
                            executable_payload_available: entry.executable_payload_available,
                            validator_count: entry.validator_count,
                            min_quorum: entry.min_quorum,
                            prepare_qc_signer_count: entry.prepare_qc_signer_count,
                            commit_qc_signer_count: entry.commit_qc_signer_count,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let (
                tx_queue_depth,
                tx_queue_capacity,
                tx_queue_saturated,
                tx_queue_saturated_by_count,
                tx_queue_saturated_by_bytes,
                tx_queue_saturated_by_age,
                tx_queue_oldest_queued_age_ms,
                commit_inflight_active,
                commit_inflight_height,
                commit_inflight_view,
                commit_inflight_elapsed_ms,
                commit_inflight_timeout_total,
                pending_rbc_entries,
                pacemaker_backpressure_deferrals_total,
                proposal_gate,
            ) = sumeragi_status
                .as_ref()
                .map(|sumeragi_status| {
                    let gate = sumeragi_status.proposal_gate;
                    (
                        sumeragi_status.tx_queue_depth,
                        sumeragi_status.tx_queue_capacity,
                        sumeragi_status.tx_queue_saturated,
                        sumeragi_status.tx_queue_saturated_by_count,
                        sumeragi_status.tx_queue_saturated_by_bytes,
                        sumeragi_status.tx_queue_saturated_by_age,
                        sumeragi_status.tx_queue_oldest_queued_age_ms,
                        sumeragi_status.commit_inflight.active,
                        sumeragi_status.commit_inflight.height,
                        sumeragi_status.commit_inflight.view,
                        sumeragi_status.commit_inflight.elapsed_ms,
                        sumeragi_status.commit_inflight.timeout_total,
                        u64::try_from(sumeragi_status.pending_rbc.entries.len())
                            .unwrap_or(u64::MAX),
                        sumeragi_status.pacemaker_backpressure_deferrals_total,
                        ProposalGateSnapshot {
                            height: gate.height,
                            view: gate.view,
                            queue_len: gate.queue_len,
                            pending_blocks_total: gate.pending_blocks_total,
                            pending_blocks_blocking: gate.pending_blocks_blocking,
                            active_pending_for_tip: gate.active_pending_for_tip,
                            queue_saturated: gate.queue_saturated,
                            active_pending: gate.active_pending,
                            rbc_backlog: gate.rbc_backlog,
                            relay_backpressure: gate.relay_backpressure,
                            consensus_queue_backpressure: gate.consensus_queue_backpressure,
                            should_defer: gate.should_defer,
                            only_pacing_backpressure: gate.only_pacing_backpressure,
                            commit_inflight_active: gate.commit_inflight_active,
                            cached_proposal_present: gate.cached_proposal_present,
                            cached_proposal_hint_present: gate.cached_proposal_hint_present,
                            round_liveness_present: gate.round_liveness_present,
                            frontier_owner_present: gate.frontier_owner_present,
                            missing_qc_liveness_active: gate.missing_qc_liveness_active,
                            last_pacemaker_attempt_age_ms: gate.last_pacemaker_attempt_age_ms,
                            last_successful_proposal_age_ms: gate.last_successful_proposal_age_ms,
                        },
                    )
                })
                .unwrap_or((
                    0,
                    0,
                    false,
                    false,
                    false,
                    false,
                    0,
                    false,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    ProposalGateSnapshot::default(),
                ));
            let lane_validators = OBSERVED_AUTOSCALE_LANE_IDS
                .iter()
                .filter_map(|lane_id| fetch_lane_validator_snapshot(&client, *lane_id))
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
                committed_lane_blocks,
                lane_validators,
                commit_signatures_required,
                commit_qc_validator_set_len,
                tx_queue_depth,
                tx_queue_capacity,
                tx_queue_saturated,
                tx_queue_saturated_by_count,
                tx_queue_saturated_by_bytes,
                tx_queue_saturated_by_age,
                tx_queue_oldest_queued_age_ms,
                commit_inflight_active,
                commit_inflight_height,
                commit_inflight_view,
                commit_inflight_elapsed_ms,
                commit_inflight_timeout_total,
                pending_rbc_entries,
                pacemaker_backpressure_deferrals_total,
                proposal_gate,
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
    if snapshot.len() != TOTAL_PEERS {
        return false;
    }
    snapshot
        .iter()
        .all(|(_, lanes)| lanes.len() == expected_count)
}

fn storage_lane_id(segment: &str) -> Option<u32> {
    let rest = segment.strip_prefix("lane_")?;
    let digits = rest.get(..3)?;
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let slug = rest.get(3..)?.strip_prefix('_')?;
    if slug.is_empty()
        || slug.starts_with('_')
        || slug.ends_with('_')
        || slug.contains("__")
        || !slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    digits.parse().ok()
}

fn autoscale_elastic_storage_segment(lane_id: u32) -> String {
    format!("lane_{lane_id:03}_elastic_lane_{lane_id}")
}

fn is_autoscale_elastic_storage_segment(segment: &str, lane_id: u32) -> bool {
    segment == autoscale_elastic_storage_segment(lane_id)
        && storage_lane_id(segment) == Some(lane_id)
}

fn all_peers_have_storage_lane_profile(
    snapshot: &[(usize, Vec<String>)],
    expected_count: usize,
    required_lane_id: u32,
) -> bool {
    if snapshot.len() != TOTAL_PEERS {
        return false;
    }
    let Some(expected_count_u32) = u32::try_from(expected_count).ok() else {
        return false;
    };
    snapshot.iter().all(|(_, lanes)| {
        if lanes.len() != expected_count {
            return false;
        }
        let Some(mut lane_ids) = lanes
            .iter()
            .map(|lane| storage_lane_id(lane))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let has_required_elastic_segment = lanes
            .iter()
            .any(|lane| is_autoscale_elastic_storage_segment(lane, required_lane_id));
        lane_ids.sort_unstable();
        lane_ids.dedup();
        lane_ids.len() == expected_count
            && has_required_elastic_segment
            && lane_ids.into_iter().eq(0..expected_count_u32)
    })
}

fn peer_has_active_lane_capacity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer_lane_status(peer, lane_id).is_some_and(|lane| lane.capacity > 0 || lane.committed > 0)
}

fn peer_has_lane_commitment_activity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer_lane_commitment_evidence(peer, lane_id)
        .unambiguous()
        .is_some_and(|lane| lane.tx_count > 0 || lane.teu_total > 0)
}

enum LaneStatusEvidence<'a> {
    Missing,
    Unambiguous(&'a LaneStatusSnapshot),
    Ambiguous,
}

fn peer_lane_status_evidence(peer: &PeerStatusSnapshot, lane_id: u32) -> LaneStatusEvidence<'_> {
    let mut matches = peer.lanes.iter().filter(|lane| lane.lane_id == lane_id);
    let Some(lane) = matches.next() else {
        return LaneStatusEvidence::Missing;
    };
    if matches.next().is_some() {
        return LaneStatusEvidence::Ambiguous;
    }
    LaneStatusEvidence::Unambiguous(lane)
}

fn peer_lane_status(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<&LaneStatusSnapshot> {
    match peer_lane_status_evidence(peer, lane_id) {
        LaneStatusEvidence::Unambiguous(snapshot) => Some(snapshot),
        LaneStatusEvidence::Missing | LaneStatusEvidence::Ambiguous => None,
    }
}

enum LaneCommitmentEvidence<'a> {
    Missing,
    Unambiguous(&'a LaneCommitmentSnapshot),
    Ambiguous,
}

impl<'a> LaneCommitmentEvidence<'a> {
    fn unambiguous(self) -> Option<&'a LaneCommitmentSnapshot> {
        match self {
            Self::Unambiguous(snapshot) => Some(snapshot),
            Self::Missing | Self::Ambiguous => None,
        }
    }
}

fn peer_lane_commitment_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneCommitmentSnapshot> {
    peer_lane_commitment_evidence(peer, lane_id).unambiguous()
}

fn peer_lane_commitment_evidence(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> LaneCommitmentEvidence<'_> {
    let mut latest = None::<&LaneCommitmentSnapshot>;
    let mut latest_is_ambiguous = false;

    for lane in peer
        .lane_commitments
        .iter()
        .filter(|lane| lane.lane_id == lane_id)
    {
        let Some(current) = latest else {
            latest = Some(lane);
            latest_is_ambiguous = false;
            continue;
        };
        match lane.block_height.cmp(&current.block_height) {
            std::cmp::Ordering::Greater => {
                latest = Some(lane);
                latest_is_ambiguous = false;
            }
            std::cmp::Ordering::Equal => {
                if lane.tx_count != current.tx_count || lane.teu_total != current.teu_total {
                    latest_is_ambiguous = true;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }

    if latest_is_ambiguous {
        LaneCommitmentEvidence::Ambiguous
    } else {
        match latest {
            Some(snapshot) => LaneCommitmentEvidence::Unambiguous(snapshot),
            None => LaneCommitmentEvidence::Missing,
        }
    }
}

fn lane_relay_latest_rows_equivalent(left: &LaneRelaySnapshot, right: &LaneRelaySnapshot) -> bool {
    left.lane_id == right.lane_id
        && left.dataspace_id == right.dataspace_id
        && left.block_height == right.block_height
        && left.descriptor_hash == right.descriptor_hash
        && left.merge_admissible == right.merge_admissible
}

enum LaneRelayEvidence<'a> {
    Missing,
    Unambiguous(&'a LaneRelaySnapshot),
    Ambiguous,
}

fn latest_lane_relay_evidence(peer: &PeerStatusSnapshot, lane_id: u32) -> LaneRelayEvidence<'_> {
    let mut latest = None::<&LaneRelaySnapshot>;
    let mut latest_is_ambiguous = false;

    for relay in peer.lane_relay.iter().filter(|relay| {
        relay.lane_id == lane_id && relay.dataspace_id == DataSpaceId::UNIVERSAL.as_u64()
    }) {
        let Some(current) = latest else {
            latest = Some(relay);
            latest_is_ambiguous = false;
            continue;
        };
        match relay.block_height.cmp(&current.block_height) {
            std::cmp::Ordering::Greater => {
                latest = Some(relay);
                latest_is_ambiguous = false;
            }
            std::cmp::Ordering::Equal => {
                if !lane_relay_latest_rows_equivalent(current, relay) {
                    latest_is_ambiguous = true;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }

    if latest_is_ambiguous {
        LaneRelayEvidence::Ambiguous
    } else {
        match latest {
            Some(snapshot) => LaneRelayEvidence::Unambiguous(snapshot),
            None => LaneRelayEvidence::Missing,
        }
    }
}

fn relay_row_counts_as_expansion_progress(relay: &LaneRelaySnapshot) -> bool {
    relay.dataspace_id == DataSpaceId::UNIVERSAL.as_u64()
        && relay.merge_admissible
        && relay.descriptor_hash.is_some()
}

fn peer_lane_relay_progress_height(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<u64> {
    match latest_lane_relay_evidence(peer, lane_id) {
        LaneRelayEvidence::Unambiguous(relay) if relay_row_counts_as_expansion_progress(relay) => {
            Some(relay.block_height)
        }
        LaneRelayEvidence::Missing
        | LaneRelayEvidence::Unambiguous(_)
        | LaneRelayEvidence::Ambiguous => None,
    }
}

fn committed_lane_block_targets_default_dataspace(block: &CommittedLaneBlockSnapshot) -> bool {
    block.dataspace_id == DataSpaceId::UNIVERSAL.as_u64()
}

fn committed_lane_block_has_canonical_quorum_metadata(block: &CommittedLaneBlockSnapshot) -> bool {
    let Ok(validator_count) = usize::try_from(block.validator_count) else {
        return false;
    };
    let Ok(min_quorum) = usize::try_from(block.min_quorum) else {
        return false;
    };
    let Ok(prepare_qc_signer_count) = usize::try_from(block.prepare_qc_signer_count) else {
        return false;
    };
    let Ok(commit_qc_signer_count) = usize::try_from(block.commit_qc_signer_count) else {
        return false;
    };
    if validator_count == 0 || validator_count > TOTAL_PEERS || min_quorum == 0 {
        return false;
    }
    let expected_quorum = commit_quorum_from_len(validator_count).max(1);
    min_quorum == expected_quorum
        && prepare_qc_signer_count >= min_quorum
        && prepare_qc_signer_count <= validator_count
        && commit_qc_signer_count >= min_quorum
        && commit_qc_signer_count <= validator_count
}

fn committed_lane_block_is_certified(block: &CommittedLaneBlockSnapshot) -> bool {
    committed_lane_block_targets_default_dataspace(block)
        && committed_lane_block_has_canonical_quorum_metadata(block)
        && committed_lane_block_status_counts_as_progress(
            block.execution_status.as_str(),
            block.executable_payload_available,
        )
}

fn committed_lane_block_is_direct_applied(block: &CommittedLaneBlockSnapshot) -> bool {
    block.execution_status == COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
        && committed_lane_block_is_certified(block)
}

fn committed_lane_block_latest_rows_equivalent(
    left: &CommittedLaneBlockSnapshot,
    right: &CommittedLaneBlockSnapshot,
) -> bool {
    left.lane_id == right.lane_id
        && left.dataspace_id == right.dataspace_id
        && left.lane_block_height == right.lane_block_height
        && left.lane_block_view == right.lane_block_view
        && left.descriptor_hash == right.descriptor_hash
        && left.proposal_hash == right.proposal_hash
        && left.subject_hash == right.subject_hash
        && left.payload_ownership_hash == right.payload_ownership_hash
        && left.rbc_instance_hash == right.rbc_instance_hash
        && left.qc_mode_tag == right.qc_mode_tag
        && left.execution_status == right.execution_status
        && left.executable_payload_available == right.executable_payload_available
        && left.validator_count == right.validator_count
        && left.min_quorum == right.min_quorum
        && left.prepare_qc_signer_count == right.prepare_qc_signer_count
        && left.commit_qc_signer_count == right.commit_qc_signer_count
}

enum CommittedLaneBlockEvidence<'a> {
    Missing,
    Unambiguous(&'a CommittedLaneBlockSnapshot),
    Ambiguous,
}

fn latest_committed_lane_block_evidence<'a>(
    peer: &'a PeerStatusSnapshot,
    lane_id: u32,
    predicate: impl Fn(&CommittedLaneBlockSnapshot) -> bool,
) -> CommittedLaneBlockEvidence<'a> {
    let mut latest = None::<&CommittedLaneBlockSnapshot>;
    let mut latest_is_ambiguous = false;

    for block in peer.committed_lane_blocks.iter().filter(|block| {
        block.lane_id == lane_id && committed_lane_block_targets_default_dataspace(block)
    }) {
        let block_key = (block.lane_block_height, block.lane_block_view);
        let Some(current) = latest else {
            latest = Some(block);
            latest_is_ambiguous = false;
            continue;
        };
        let current_key = (current.lane_block_height, current.lane_block_view);
        match block_key.cmp(&current_key) {
            std::cmp::Ordering::Greater => {
                latest = Some(block);
                latest_is_ambiguous = false;
            }
            std::cmp::Ordering::Equal => {
                if !committed_lane_block_latest_rows_equivalent(current, block) {
                    latest_is_ambiguous = true;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }

    if latest_is_ambiguous {
        CommittedLaneBlockEvidence::Ambiguous
    } else {
        match latest {
            Some(snapshot) if predicate(snapshot) => {
                CommittedLaneBlockEvidence::Unambiguous(snapshot)
            }
            Some(_) => CommittedLaneBlockEvidence::Ambiguous,
            None => CommittedLaneBlockEvidence::Missing,
        }
    }
}

fn latest_unambiguous_committed_lane_block_snapshot<'a>(
    peer: &'a PeerStatusSnapshot,
    lane_id: u32,
    predicate: impl Fn(&CommittedLaneBlockSnapshot) -> bool,
) -> Option<&'a CommittedLaneBlockSnapshot> {
    match latest_committed_lane_block_evidence(peer, lane_id, predicate) {
        CommittedLaneBlockEvidence::Unambiguous(snapshot) => Some(snapshot),
        CommittedLaneBlockEvidence::Missing | CommittedLaneBlockEvidence::Ambiguous => None,
    }
}

fn peer_committed_lane_block_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&CommittedLaneBlockSnapshot> {
    latest_unambiguous_committed_lane_block_snapshot(
        peer,
        lane_id,
        committed_lane_block_is_certified,
    )
}

fn peer_direct_applied_committed_lane_block_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&CommittedLaneBlockSnapshot> {
    latest_unambiguous_committed_lane_block_snapshot(
        peer,
        lane_id,
        committed_lane_block_is_direct_applied,
    )
}

fn peers_with_direct_applied_committed_lane_block_for_lane(
    snapshot: &[PeerStatusSnapshot],
    lane_id: u32,
) -> usize {
    snapshot
        .iter()
        .filter(|peer| peer_direct_applied_committed_lane_block_snapshot(peer, lane_id).is_some())
        .count()
}

enum LaneValidatorEvidence<'a> {
    Missing,
    Unambiguous(&'a LaneValidatorSnapshot),
    Ambiguous,
}

fn lane_validator_snapshots_equivalent(
    left: &LaneValidatorSnapshot,
    right: &LaneValidatorSnapshot,
) -> bool {
    left.lane_id == right.lane_id
        && left.total == right.total
        && left.active == right.active
        && left.pending_activation == right.pending_activation
        && left.jailed == right.jailed
        && left.exiting == right.exiting
        && left.max_activation_epoch == right.max_activation_epoch
        && left.max_activation_height == right.max_activation_height
}

fn peer_lane_validator_evidence(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> LaneValidatorEvidence<'_> {
    let mut selected = None::<&LaneValidatorSnapshot>;
    for snapshot in peer
        .lane_validators
        .iter()
        .filter(|snapshot| snapshot.lane_id == lane_id)
    {
        let Some(current) = selected else {
            selected = Some(snapshot);
            continue;
        };
        if !lane_validator_snapshots_equivalent(current, snapshot) {
            return LaneValidatorEvidence::Ambiguous;
        }
    }

    match selected {
        Some(snapshot) => LaneValidatorEvidence::Unambiguous(snapshot),
        None => LaneValidatorEvidence::Missing,
    }
}

fn peer_lane_validator_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneValidatorSnapshot> {
    match peer_lane_validator_evidence(peer, lane_id) {
        LaneValidatorEvidence::Unambiguous(snapshot) => Some(snapshot),
        LaneValidatorEvidence::Missing | LaneValidatorEvidence::Ambiguous => None,
    }
}

fn lane_validator_has_live_activity(lane: &LaneValidatorSnapshot) -> bool {
    lane.active > 0 || lane.pending_activation > 0 || lane.jailed > 0 || lane.exiting > 0
}

fn peer_has_lane_declaration(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer_has_active_lane_capacity(peer, lane_id)
        || peer_lane_commitment_snapshot(peer, lane_id).is_some()
        || peer
            .lane_governance_ids
            .iter()
            .any(|declared_lane| *declared_lane == lane_id)
        || peer_committed_lane_block_snapshot(peer, lane_id).is_some()
        || peer_lane_validator_snapshot(peer, lane_id).is_some_and(lane_validator_has_live_activity)
}

fn peer_has_ambiguous_lane_evidence(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    matches!(
        peer_lane_status_evidence(peer, lane_id),
        LaneStatusEvidence::Ambiguous
    ) || matches!(
        peer_lane_commitment_evidence(peer, lane_id),
        LaneCommitmentEvidence::Ambiguous
    ) || matches!(
        latest_committed_lane_block_evidence(peer, lane_id, committed_lane_block_is_certified),
        CommittedLaneBlockEvidence::Ambiguous
    ) || matches!(
        latest_lane_relay_evidence(peer, lane_id),
        LaneRelayEvidence::Ambiguous
    ) || matches!(
        peer_lane_validator_evidence(peer, lane_id),
        LaneValidatorEvidence::Ambiguous
    )
}

fn peer_has_lane_declaration_transition(
    peer: &PeerStatusSnapshot,
    baseline_peer: Option<&PeerStatusSnapshot>,
    lane_id: u32,
) -> bool {
    let Some(baseline_peer) = baseline_peer else {
        return false;
    };
    if peer_has_ambiguous_lane_evidence(peer, lane_id)
        || peer_has_ambiguous_lane_evidence(baseline_peer, lane_id)
    {
        return false;
    }
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
    if peer_has_ambiguous_lane_evidence(peer, lane_id)
        || peer_has_ambiguous_lane_evidence(baseline_peer, lane_id)
    {
        return false;
    }

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
        peer_lane_relay_progress_height(peer, lane_id),
        peer_lane_relay_progress_height(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => current > baseline,
        (Some(current), None) => current > 0,
        _ => false,
    };

    let validator_progressed = match (
        peer_lane_validator_snapshot(peer, lane_id),
        peer_lane_validator_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) if lane_validator_has_live_activity(current) => {
            current.active > baseline.active
                || current.pending_activation > baseline.pending_activation
                || current.jailed > baseline.jailed
                || current.exiting > baseline.exiting
        }
        _ => false,
    };

    let committed_lane_block_progressed = match (
        peer_committed_lane_block_snapshot(peer, lane_id),
        peer_committed_lane_block_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.lane_block_height > baseline.lane_block_height
                || (current.lane_block_height == baseline.lane_block_height
                    && current.lane_block_view > baseline.lane_block_view)
        }
        (Some(current), None) => current.lane_block_height > 0,
        _ => false,
    };

    status_progressed
        || commitment_progressed
        || relay_progressed
        || committed_lane_block_progressed
        || validator_progressed
}

fn peers_with_expanded_lane_signal(
    snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    lane_id: u32,
) -> usize {
    if baseline_snapshot.is_some_and(|baseline| baseline.len() != snapshot.len()) {
        return 0;
    }

    let mut peers_with_signal = 0_usize;
    for (index, peer) in snapshot.iter().enumerate() {
        let baseline_peer = baseline_snapshot.and_then(|baseline| baseline.get(index));
        let current_state_counts_without_baseline = baseline_snapshot.is_none()
            && (peer_has_active_lane_capacity(peer, lane_id)
                || peer_has_lane_commitment_activity(peer, lane_id)
                || peer_committed_lane_block_snapshot(peer, lane_id).is_some());
        if current_state_counts_without_baseline
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
    peers_with_committed_lane_block: usize,
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
        if peer_committed_lane_block_snapshot(peer, lane_id).is_some() {
            breakdown.peers_with_committed_lane_block += 1;
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
            if lane_validator_has_live_activity(validator) {
                breakdown.peers_with_lane_validator_activity += 1;
            }
        }
    }
    breakdown
}

fn expansion_observed_on_quorum_peers_for_lane(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    elastic_lane_id: u32,
    quorum_required: usize,
) -> bool {
    peers_with_expanded_lane_signal(status_snapshot, baseline_snapshot, elastic_lane_id)
        >= quorum_required
}

fn expansion_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    quorum_required: usize,
) -> bool {
    expansion_observed_on_quorum_peers_for_lane(
        status_snapshot,
        baseline_snapshot,
        ELASTIC_LANE_ID,
        quorum_required,
    )
}

fn scale_out_transition_observed_on_quorum_peers(
    transition_snapshot: &[AutoscaleTransitionStats],
    baseline_transitions: &[AutoscaleTransitionStats],
    quorum_required: usize,
) -> bool {
    peers_with_scale_out_transition(transition_snapshot, baseline_transitions) >= quorum_required
}

fn expansion_observed_on_quorum_or_scale_out_transition_for_lane(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    transition_snapshot: &[AutoscaleTransitionStats],
    baseline_transitions: &[AutoscaleTransitionStats],
    elastic_lane_id: u32,
    quorum_required: usize,
) -> bool {
    expansion_observed_on_quorum_peers_for_lane(
        status_snapshot,
        baseline_snapshot,
        elastic_lane_id,
        quorum_required,
    ) || scale_out_transition_observed_on_quorum_peers(
        transition_snapshot,
        baseline_transitions,
        quorum_required,
    )
}

fn expansion_observed_on_quorum_or_scale_out_transition(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    transition_snapshot: &[AutoscaleTransitionStats],
    baseline_transitions: &[AutoscaleTransitionStats],
    quorum_required: usize,
) -> bool {
    expansion_observed_on_quorum_or_scale_out_transition_for_lane(
        status_snapshot,
        baseline_snapshot,
        transition_snapshot,
        baseline_transitions,
        ELASTIC_LANE_ID,
        quorum_required,
    )
}

fn expansion_observed_on_storage_for_count(
    storage_snapshot: &[(usize, Vec<String>)],
    expanded_provisioned_lanes: usize,
) -> bool {
    all_peers_have_storage_lane_count(storage_snapshot, expanded_provisioned_lanes)
}

fn expansion_observed_on_storage_for_lane_count(
    storage_snapshot: &[(usize, Vec<String>)],
    expanded_provisioned_lanes: usize,
    elastic_lane_id: u32,
) -> bool {
    all_peers_have_storage_lane_profile(
        storage_snapshot,
        expanded_provisioned_lanes,
        elastic_lane_id,
    )
}

fn expansion_observed_on_storage(storage_snapshot: &[(usize, Vec<String>)]) -> bool {
    expansion_observed_on_storage_for_lane_count(
        storage_snapshot,
        EXPANDED_PROVISIONED_LANES,
        ELASTIC_LANE_ID,
    )
}

fn peer_has_contracted_profile(
    status: &PeerStatusSnapshot,
    base_lane_count: usize,
    elastic_lane_id: u32,
) -> bool {
    let base_lanes_active = (0..base_lane_count).all(|lane_index| {
        u32::try_from(lane_index)
            .ok()
            .is_some_and(|lane_id| peer_has_active_lane_capacity(status, lane_id))
    });
    let elastic_lane_undeclared = !peer_has_lane_declaration(status, elastic_lane_id);
    let elastic_lane_status_idle = match peer_lane_status_evidence(status, elastic_lane_id) {
        LaneStatusEvidence::Missing => true,
        LaneStatusEvidence::Unambiguous(snapshot) => {
            snapshot.capacity == 0 && snapshot.committed == 0
        }
        LaneStatusEvidence::Ambiguous => false,
    };
    let elastic_lane_commitments_idle = match peer_lane_commitment_evidence(status, elastic_lane_id)
    {
        LaneCommitmentEvidence::Missing => true,
        LaneCommitmentEvidence::Unambiguous(snapshot) => {
            snapshot.tx_count == 0 && snapshot.teu_total == 0
        }
        LaneCommitmentEvidence::Ambiguous => false,
    };
    let elastic_lane_relay_idle = matches!(
        latest_lane_relay_evidence(status, elastic_lane_id),
        LaneRelayEvidence::Missing
    );
    let elastic_committed_lane_blocks_idle = matches!(
        latest_committed_lane_block_evidence(
            status,
            elastic_lane_id,
            committed_lane_block_is_certified,
        ),
        CommittedLaneBlockEvidence::Missing
    );
    let elastic_lane_validator_idle = match peer_lane_validator_evidence(status, elastic_lane_id) {
        LaneValidatorEvidence::Missing => true,
        LaneValidatorEvidence::Unambiguous(snapshot) => !lane_validator_has_live_activity(snapshot),
        LaneValidatorEvidence::Ambiguous => false,
    };

    base_lanes_active
        && elastic_lane_undeclared
        && elastic_lane_status_idle
        && elastic_lane_commitments_idle
        && elastic_lane_relay_idle
        && elastic_committed_lane_blocks_idle
        && elastic_lane_validator_idle
}

fn peers_with_contracted_profile(
    snapshot: &[PeerStatusSnapshot],
    base_lane_count: usize,
    elastic_lane_id: u32,
) -> usize {
    snapshot
        .iter()
        .filter(|status| peer_has_contracted_profile(status, base_lane_count, elastic_lane_id))
        .count()
}

fn contraction_observed_on_quorum_peers_for_profile(
    status_snapshot: &[PeerStatusSnapshot],
    base_lane_count: usize,
    elastic_lane_id: u32,
    quorum_required: usize,
) -> bool {
    peers_with_contracted_profile(status_snapshot, base_lane_count, elastic_lane_id)
        >= quorum_required
}

fn contraction_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
) -> bool {
    contraction_observed_on_quorum_peers_for_profile(
        status_snapshot,
        INITIAL_PROVISIONED_LANES,
        ELASTIC_LANE_ID,
        quorum_required,
    )
}

fn should_require_scale_in_transition_for_lane(
    requested: bool,
    post_expansion_snapshot: &[PeerStatusSnapshot],
    pre_cycle_snapshot: &[PeerStatusSnapshot],
    elastic_lane_id: u32,
    quorum_required: usize,
) -> bool {
    requested
        && expansion_observed_on_quorum_peers_for_lane(
            post_expansion_snapshot,
            Some(pre_cycle_snapshot),
            elastic_lane_id,
            quorum_required,
        )
}

fn should_require_scale_in_transition(
    requested: bool,
    post_expansion_snapshot: &[PeerStatusSnapshot],
    pre_cycle_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
) -> bool {
    should_require_scale_in_transition_for_lane(
        requested,
        post_expansion_snapshot,
        pre_cycle_snapshot,
        ELASTIC_LANE_ID,
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

fn tx_confirmation_status_counts_as_load_activity(status: &TxConfirmationStatus) -> bool {
    matches!(
        status,
        TxConfirmationStatus::Queued
            | TxConfirmationStatus::Approved(_)
            | TxConfirmationStatus::Committed
            | TxConfirmationStatus::Applied
            | TxConfirmationStatus::Rejected(_)
            | TxConfirmationStatus::Expired
    )
}

fn tx_confirmation_status_counts_as_post_cycle_progress(status: &TxConfirmationStatus) -> bool {
    matches!(
        status,
        TxConfirmationStatus::Approved(_)
            | TxConfirmationStatus::Committed
            | TxConfirmationStatus::Applied
            | TxConfirmationStatus::Rejected(_)
            | TxConfirmationStatus::Expired
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitQuorumSource {
    RequiredStatus,
    ValidatorSetLen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CommitQuorumObservation {
    Ready {
        quorum_required: usize,
        source: CommitQuorumSource,
    },
    NoEvidence,
    ConflictingRequired {
        observed: Vec<u64>,
    },
    InvalidRequired {
        quorum_required: u64,
        peer_count: usize,
        observed: Vec<u64>,
    },
    ConflictingValidatorSetLen {
        observed: Vec<u64>,
    },
    InvalidValidatorSetLen {
        validator_set_len: u64,
        derived_quorum: Option<usize>,
        peer_count: usize,
    },
}

impl CommitQuorumObservation {
    fn quorum_required(&self) -> Option<usize> {
        match self {
            Self::Ready {
                quorum_required, ..
            } => Some(*quorum_required),
            Self::NoEvidence
            | Self::ConflictingRequired { .. }
            | Self::InvalidRequired { .. }
            | Self::ConflictingValidatorSetLen { .. }
            | Self::InvalidValidatorSetLen { .. } => None,
        }
    }

    fn timeout_error(&self, context: &str) -> Option<String> {
        match self {
            Self::Ready { .. } | Self::NoEvidence => None,
            Self::ConflictingRequired { observed } => Some(format!(
                "{context}: conflicting commit quorum values after timeout; observed values: {observed:?}"
            )),
            Self::InvalidRequired {
                quorum_required,
                peer_count,
                observed,
            } => Some(format!(
                "{context}: invalid commit quorum value {quorum_required} for peer count {peer_count} after timeout; observed values: {observed:?}"
            )),
            Self::ConflictingValidatorSetLen { observed } => Some(format!(
                "{context}: conflicting commit-QC validator-set lengths after timeout; observed values: {observed:?}"
            )),
            Self::InvalidValidatorSetLen {
                validator_set_len,
                derived_quorum,
                peer_count,
            } => Some(format!(
                "{context}: invalid derived quorum {derived_quorum:?} from validator set len {validator_set_len} for peer count {peer_count} after timeout"
            )),
        }
    }
}

fn commit_quorum_observation(
    snapshot: &[PeerStatusSnapshot],
    peer_count: usize,
) -> CommitQuorumObservation {
    let observed_required = snapshot
        .iter()
        .map(|status| status.commit_signatures_required)
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    let observed_validator_set_len = snapshot
        .iter()
        .map(|status| status.commit_qc_validator_set_len)
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();

    if !observed_required.is_empty() {
        let min_required = *observed_required.iter().min().unwrap_or(&0);
        let max_required = *observed_required.iter().max().unwrap_or(&0);
        if min_required != max_required {
            return CommitQuorumObservation::ConflictingRequired {
                observed: observed_required,
            };
        }

        let Ok(quorum_required) = usize::try_from(max_required) else {
            return CommitQuorumObservation::InvalidRequired {
                quorum_required: max_required,
                peer_count,
                observed: observed_required,
            };
        };
        if !(1..=peer_count).contains(&quorum_required) {
            return CommitQuorumObservation::InvalidRequired {
                quorum_required: max_required,
                peer_count,
                observed: observed_required,
            };
        }
        let expected_quorum = match expected_commit_quorum_from_validator_set_len_observation(
            &observed_validator_set_len,
            peer_count,
        ) {
            Ok(Some((_validator_set_len, expected_quorum))) => expected_quorum,
            Ok(None) => commit_quorum_from_len(peer_count).max(1),
            Err(observation) => return observation,
        };
        if quorum_required != expected_quorum {
            return CommitQuorumObservation::InvalidRequired {
                quorum_required: max_required,
                peer_count,
                observed: observed_required,
            };
        }
        return CommitQuorumObservation::Ready {
            quorum_required,
            source: CommitQuorumSource::RequiredStatus,
        };
    }

    if let Some((_validator_set_len, quorum_required)) =
        match expected_commit_quorum_from_validator_set_len_observation(
            &observed_validator_set_len,
            peer_count,
        ) {
            Ok(observation) => observation,
            Err(observation) => return observation,
        }
    {
        return CommitQuorumObservation::Ready {
            quorum_required,
            source: CommitQuorumSource::ValidatorSetLen,
        };
    }

    CommitQuorumObservation::NoEvidence
}

fn expected_commit_quorum_from_validator_set_len_observation(
    observed_validator_set_len: &[u64],
    peer_count: usize,
) -> Result<Option<(usize, usize)>, CommitQuorumObservation> {
    if observed_validator_set_len.is_empty() {
        return Ok(None);
    }
    let min_len = *observed_validator_set_len.iter().min().unwrap_or(&0);
    let max_len = *observed_validator_set_len.iter().max().unwrap_or(&0);
    if min_len != max_len {
        return Err(CommitQuorumObservation::ConflictingValidatorSetLen {
            observed: observed_validator_set_len.to_vec(),
        });
    }

    let Ok(validator_set_len) = usize::try_from(max_len) else {
        return Err(CommitQuorumObservation::InvalidValidatorSetLen {
            validator_set_len: max_len,
            derived_quorum: None,
            peer_count,
        });
    };
    let quorum_required = commit_quorum_from_len(validator_set_len);
    if validator_set_len > peer_count {
        return Err(CommitQuorumObservation::InvalidValidatorSetLen {
            validator_set_len: max_len,
            derived_quorum: Some(quorum_required),
            peer_count,
        });
    }
    if !(1..=peer_count).contains(&quorum_required) {
        return Err(CommitQuorumObservation::InvalidValidatorSetLen {
            validator_set_len: max_len,
            derived_quorum: Some(quorum_required),
            peer_count,
        });
    }
    Ok(Some((validator_set_len, quorum_required)))
}

fn wait_for_commit_quorum_required(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
    context: &str,
) -> Result<usize> {
    let started = Instant::now();
    let mut last_error = None::<String>;
    let mut last_observation = CommitQuorumObservation::NoEvidence;
    let mut consecutive_failures = 0_u32;

    while started.elapsed() <= timeout {
        match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_failures = 0;
                let observation = commit_quorum_observation(&snapshot, network.peers().len());
                if let Some(quorum_required) = observation.quorum_required() {
                    return Ok(quorum_required);
                }
                last_observation = observation;

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

    if let Some(error) = last_observation.timeout_error(context) {
        return Err(eyre!("{error}"));
    }

    let fallback_quorum = commit_quorum_from_len(network.peers().len());
    eprintln!(
        "[autoscale-localnet] commit quorum fallback from peer count: {} (context: {context}; last observation={last_observation:?}; last error={last_error:?})",
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

fn autoscale_soak_duration_from_env_value(raw: Option<&str>) -> Duration {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(AUTOSCALE_SOAK_DURATION)
}

fn autoscale_soak_duration() -> Duration {
    let raw = std::env::var(AUTOSCALE_SOAK_DURATION_ENV).ok();
    autoscale_soak_duration_from_env_value(raw.as_deref())
}

fn autoscale_soak_force_fail_cycle() -> Option<usize> {
    std::env::var(AUTOSCALE_SOAK_FORCE_FAIL_CYCLE_ENV)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn submit_load_round_robin(clients: &[Client], tx_count: usize) -> Result<LoadSubmissionReport> {
    ensure!(
        !clients.is_empty(),
        "load submission requires at least one client"
    );

    let mut report = LoadSubmissionReport {
        attempted: tx_count,
        submitted: 0,
        per_client_submitted: vec![0; clients.len()],
        samples: Vec::new(),
        first_error: None,
    };
    let mut samples_per_client = vec![0_usize; clients.len()];
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
            Ok(hash) => {
                report.submitted = report.submitted.saturating_add(1);
                report.per_client_submitted[client_index] =
                    report.per_client_submitted[client_index].saturating_add(1);
                if samples_per_client[client_index] < LOAD_ACTIVITY_SAMPLE_LIMIT_PER_CLIENT {
                    report
                        .samples
                        .push(LoadSubmissionSample { client_index, hash });
                    samples_per_client[client_index] =
                        samples_per_client[client_index].saturating_add(1);
                }
            }
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(format!(
                        "submit autoscale load transaction {tx} failed: {err}"
                    ));
                }
            }
        }
    }
    report.first_error = first_error.clone();
    if tx_count > 0 && report.submitted < tx_count {
        eprintln!(
            "[autoscale-localnet] partial load submission: submitted {}/{tx_count}; per-client counts: {:?}; first error: {first_error:?}",
            report.submitted, report.per_client_submitted,
        );
    }
    validate_load_submission_outcome(tx_count, report.submitted, first_error.as_deref())?;
    Ok(report)
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

fn cycle_load_tx_count(base_load_tx_count: usize, attempt: usize) -> usize {
    base_load_tx_count.saturating_mul(attempt.max(1))
}

fn single_cycle_load_tx_count(attempt: usize) -> usize {
    cycle_load_tx_count(STRICT_CYCLE_LOAD_TX_COUNT, attempt)
}

fn strict_cycle_load_tx_count(attempt: usize) -> usize {
    cycle_load_tx_count(STRICT_CYCLE_LOAD_TX_COUNT, attempt)
}

fn soak_cycle_load_tx_count(attempt: usize) -> usize {
    cycle_load_tx_count(STRICT_CYCLE_LOAD_TX_COUNT, attempt)
}

fn public_profile_strict_cycle_load_tx_count(attempt: usize) -> usize {
    cycle_load_tx_count(PUBLIC_PROFILE_STRICT_CYCLE_LOAD_TX_COUNT, attempt)
}

fn should_run_cooldown_clearance(cycle_index: usize, attempt: usize) -> bool {
    cycle_index > 1 && attempt <= 1
}

fn wait_for_submission_ready(clients: &[Client], timeout: Duration, context: &str) -> Result<()> {
    ensure!(
        !clients.is_empty(),
        "{context}: submission readiness requires at least one client"
    );

    let started = Instant::now();
    let mut probe_seq = 0_u64;
    let mut last_errors = vec![None::<String>; clients.len()];
    while started.elapsed() <= timeout {
        for (client_index, client) in clients.iter().enumerate() {
            match client.submit(Log::new(
                Level::INFO,
                format!("autoscale-submission-ready-{probe_seq}-{client_index}"),
            )) {
                Ok(hash) => {
                    eprintln!(
                        "[autoscale-localnet] {context}: submission readiness accepted by client {client_index} with tx {hash}"
                    );
                    return Ok(());
                }
                Err(err) => {
                    last_errors[client_index] = Some(err.to_string());
                }
            }
            probe_seq = probe_seq.saturating_add(1);
        }
        thread::sleep(SUBMISSION_READY_POLL);
    }

    Err(eyre!(
        "{context}: timed out waiting for Torii transaction submission readiness; last per-client errors: {last_errors:?}"
    ))
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
    require_scale_out_transition: bool,
    scale_out_transition_observed_on_quorum: bool,
) -> usize {
    if require_scale_out_transition && scale_out_transition_observed_on_quorum {
        return 0;
    }
    if storage_expanded && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE {
        if require_scale_out_transition {
            return 0;
        }
        return expansion_post_storage_top_up_tx_count(cycle_load_tx_count);
    }
    expansion_scaled_top_up_tx_count(heartbeat_seq, cycle_load_tx_count)
}

fn expansion_probe_ready(
    expansion_observed_on_status: bool,
    scale_out_transition_observed_on_quorum: bool,
    require_scale_out_transition: bool,
    require_expansion_status: bool,
) -> bool {
    if require_scale_out_transition && require_expansion_status {
        scale_out_transition_observed_on_quorum && expansion_observed_on_status
    } else if require_scale_out_transition {
        scale_out_transition_observed_on_quorum
    } else {
        expansion_observed_on_status || scale_out_transition_observed_on_quorum
    }
}

fn wait_for_expanded_lanes_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    top_up_clients: &[Client],
    cycle_load_tx_count: usize,
    baseline_status_snapshot: &[PeerStatusSnapshot],
    baseline_elastic_storage_snapshot: &[Option<ElasticLaneStorageStats>],
    baseline_autoscale_transitions: &[AutoscaleTransitionStats],
    elastic_lane_id: u32,
    expanded_provisioned_lanes: usize,
    require_scale_out_transition: bool,
    require_expansion_status: bool,
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

    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        let elastic_storage_snapshot = elastic_lane_storage_snapshot(network, elastic_lane_id)?;
        let peers_with_storage_progress = peers_with_elastic_storage_progress(
            &elastic_storage_snapshot,
            baseline_elastic_storage_snapshot,
        );
        let storage_progressed_on_quorum = peers_with_storage_progress >= quorum_required;
        let storage_expanded = expansion_observed_on_storage_for_lane_count(
            &storage_snapshot,
            expanded_provisioned_lanes,
            elastic_lane_id,
        );
        let elapsed = started.elapsed();
        let fallback_ready_at =
            EXPANSION_STATUS_SIGNAL_GRACE + EXPANSION_POST_STORAGE_STATUS_WINDOW;

        let status_snapshot = match status_snapshot(network) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                last_status_error = Some(err.to_string());
                let scale_out_transition_observed_on_quorum =
                    match autoscale_transition_snapshot_for_lane(network, elastic_lane_id) {
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
                if scale_out_transition_observed_on_quorum && !require_expansion_status {
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
                    require_scale_out_transition,
                    scale_out_transition_observed_on_quorum,
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

        let expansion_observed_on_status = expansion_observed_on_quorum_peers_for_lane(
            &status_snapshot,
            Some(baseline_status_snapshot),
            elastic_lane_id,
            quorum_required,
        );
        let scale_out_transition_observed_on_quorum =
            match autoscale_transition_snapshot_for_lane(network, elastic_lane_id) {
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

        let expansion_ready = expansion_probe_ready(
            expansion_observed_on_status,
            scale_out_transition_observed_on_quorum,
            require_scale_out_transition,
            require_expansion_status,
        );
        if expansion_ready {
            if require_scale_out_transition && require_expansion_status {
                eprintln!(
                    "[autoscale-localnet] {context}: expansion observed via status lane activity/lifecycle transitions and deterministic autoscale scale-out transitions (scale-out transitions {last_scale_out_transition_peers}/{quorum_required})"
                );
            } else if expansion_observed_on_status {
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
                elastic_lane_id,
            );
            let signal_breakdown = expansion_signal_breakdown(
                &status_snapshot,
                Some(baseline_status_snapshot),
                elastic_lane_id,
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
            require_scale_out_transition,
            scale_out_transition_observed_on_quorum,
        );
        if top_up_tx_count > 0 {
            if let Err(err) = submit_load_round_robin(top_up_clients, top_up_tx_count) {
                last_top_up_error = Some(err.to_string());
            }
        }
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for expanded lane profile (lane {elastic_lane_id} active via status `capacity>0 || committed>0`, sumeragi lane commitment `tx_count>0 || teu_total>0`, public-lane validator lifecycle activity (`active || pending_activation || jailed || exiting`), baseline transition via lane declaration/progress, or deterministic autoscale scale-out transitions on >= {quorum_required}/{TOTAL_PEERS} peers{}; storage lane count={expanded_provisioned_lanes} accepted only as fallback after grace {:?} + post-storage status window {:?} when elastic lane storage progresses on >= {quorum_required}/{TOTAL_PEERS} peers and scale-out transition quorum is not required); last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last elastic storage snapshot: {last_elastic_storage_snapshot:?}; last autoscale transition snapshot: {last_transition_snapshot:?}; last scale-out transition peers: {last_scale_out_transition_peers}/{TOTAL_PEERS}; last status error: {last_status_error:?}; last transition error: {last_transition_error:?}; last heartbeat error: {last_heartbeat_error:?}; last top-up error: {last_top_up_error:?}",
        if require_scale_out_transition {
            if require_expansion_status {
                "; strict mode requires fresh deterministic scale-out transition quorum after the cycle baseline and expanded-lane status evidence"
            } else {
                "; strict mode requires fresh deterministic scale-out transition quorum after the cycle baseline"
            }
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
    load_activity_probe: Option<(&[Client], &LoadSubmissionReport)>,
    baseline_non_empty: u64,
    baseline_txs_approved: u64,
    baseline_txs_rejected: u64,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
    sample_status_counts_as_progress: fn(&TxConfirmationStatus) -> bool,
    sample_progress_label: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_status_snapshot = Vec::new();
    let mut last_status_error = None::<String>;
    let mut last_heartbeat_error = None::<String>;
    let mut last_sample_status_snapshot = Vec::<String>::new();
    let mut last_sample_status_error = None::<String>;
    let load_submission_first_error =
        load_activity_probe.and_then(|(_, report)| report.first_error.clone());

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

        if let Some((submitters, load_report)) = load_activity_probe {
            last_sample_status_snapshot.clear();
            last_sample_status_error = None;
            for sample in &load_report.samples {
                let Some(client) = submitters.get(sample.client_index) else {
                    last_sample_status_error = Some(format!(
                        "load sample references missing submitter client index {}",
                        sample.client_index
                    ));
                    continue;
                };
                match client.get_transaction_status(sample.hash) {
                    Ok(Some(status)) => {
                        let observation = format!(
                            "client {} tx {} => {status:?}",
                            sample.client_index, sample.hash
                        );
                        last_sample_status_snapshot.push(observation.clone());
                        if sample_status_counts_as_progress(&status) {
                            eprintln!(
                                "[autoscale-localnet] {context}: {sample_progress_label} observed via pipeline status ({observation}); submitted {}/{}; per-client counts: {:?}",
                                load_report.submitted,
                                load_report.attempted,
                                load_report.per_client_submitted,
                            );
                            return Ok(());
                        }
                    }
                    Ok(None) => {
                        last_sample_status_snapshot.push(format!(
                            "client {} tx {} => pending",
                            sample.client_index, sample.hash
                        ));
                    }
                    Err(err) => {
                        last_sample_status_error = Some(format!(
                            "client {} tx {} status probe failed: {err}",
                            sample.client_index, sample.hash
                        ));
                    }
                }
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
        "{context}: timed out waiting for chain progress (baseline blocks_non_empty={baseline_non_empty}, txs_approved={baseline_txs_approved}, txs_rejected={baseline_txs_rejected}); last status snapshot: {last_status_snapshot:?}; last status error: {last_status_error:?}; last heartbeat error: {last_heartbeat_error:?}; last sample status snapshot: {last_sample_status_snapshot:?}; last sample status error: {last_sample_status_error:?}; load submission first error: {load_submission_first_error:?}"
    ))
}

fn wait_for_contracted_lanes(
    network: &sandbox::SerializedNetwork,
    heartbeat_clients: &[Client],
    heartbeat_prefix: &str,
    base_lane_count: usize,
    elastic_lane_id: u32,
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
                if let Err(heartbeat_err) =
                    submit_rotating_heartbeat(heartbeat_clients, heartbeat_prefix, heartbeat_seq)
                {
                    last_heartbeat_error = Some(heartbeat_err);
                }
                heartbeat_seq = heartbeat_seq.saturating_add(1);
                thread::sleep(heartbeat_interval);
                continue;
            }
        };

        let contracted_on_quorum = contraction_observed_on_quorum_peers_for_profile(
            &status_snapshot,
            base_lane_count,
            elastic_lane_id,
            quorum_required,
        );
        let scale_in_transition_observed_on_quorum = if require_scale_in_transition {
            let baseline_after_expansion =
                baseline_autoscale_transitions_after_expansion.unwrap_or_default();
            match autoscale_transition_snapshot_for_lane(network, elastic_lane_id) {
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
                    scale_in_transition_quorum_satisfied(
                        peers_with_scale_in_after_expansion,
                        peers_with_scale_in_since_cycle,
                        quorum_required,
                    )
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
        if let Err(heartbeat_err) =
            submit_rotating_heartbeat(heartbeat_clients, heartbeat_prefix, heartbeat_seq)
        {
            last_heartbeat_error = Some(heartbeat_err);
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);
        thread::sleep(heartbeat_interval);
    }

    let since_cycle_start_diagnostics = last_scale_in_transition_peers_since_cycle_start
        .map(|peers| format!("{peers}/{TOTAL_PEERS}"))
        .unwrap_or_else(|| "n/a".to_owned());
    Err(eyre!(
        "{context}: timed out waiting for contracted lane profile (base lanes 0..{base_lane_count} declared; lane {elastic_lane_id} undeclared and idle on >= {quorum_required}/{TOTAL_PEERS} peers{}) ; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last autoscale transition snapshot: {last_transition_snapshot:?}; last scale-in transition peers after expansion: {last_scale_in_transition_peers_after_expansion}/{TOTAL_PEERS}; last scale-in transition peers since cycle start: {since_cycle_start_diagnostics}; last status error: {last_status_error:?}; last transition error: {last_transition_error:?}; last heartbeat error: {last_heartbeat_error:?}",
        if require_scale_in_transition {
            " and deterministic autoscale scale-in transitions observed on quorum peers after expansion baseline"
        } else {
            ""
        }
    ))
}

fn submit_rotating_heartbeat(
    clients: &[Client],
    prefix: &str,
    sequence: u64,
) -> Result<(), String> {
    let Some(client) = clients.get(usize::try_from(sequence).unwrap_or(0) % clients.len().max(1))
    else {
        return Ok(());
    };
    client
        .submit(Log::new(Level::INFO, format!("{prefix}-{sequence}")))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn run_expand_contract_cycle(
    network: &sandbox::SerializedNetwork,
    submitters: &[Client],
    quorum_required: usize,
    initial_provisioned_lanes: usize,
    expanded_provisioned_lanes: usize,
    elastic_lane_id: u32,
    cycle_index: usize,
    attempt: usize,
    require_scale_out_transition: bool,
    require_scale_in_transition: bool,
    require_expansion_status_before_contraction: bool,
    load_tx_count: usize,
) -> Result<ExpandContractCycleOutcome> {
    let pre_contraction_context = format!("autoscale contraction pre-check cycle {cycle_index}");
    wait_for_contracted_lanes(
        network,
        submitters,
        &format!("autoscale-precheck-heartbeat-cycle-{cycle_index}"),
        initial_provisioned_lanes,
        elastic_lane_id,
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
            None,
            cooldown_baseline_height
                .saturating_add(AUTOSCALE_COOLDOWN_CLEARANCE_BLOCK_DELTA.saturating_sub(1)),
            u64::MAX,
            u64::MAX,
            AUTOSCALE_COOLDOWN_CLEARANCE_TIMEOUT,
            &cooldown_context,
            &cooldown_prefix,
            EXPANSION_PROBE_INTERVAL,
            tx_confirmation_status_counts_as_load_activity,
            "load activity",
        )?;
        let post_cooldown_context =
            format!("autoscale post-cooldown contraction check cycle {cycle_index}");
        wait_for_contracted_lanes(
            network,
            submitters,
            &format!("autoscale-post-cooldown-heartbeat-cycle-{cycle_index}"),
            initial_provisioned_lanes,
            elastic_lane_id,
            quorum_required,
            SCALE_IN_WAIT_TIMEOUT,
            &post_cooldown_context,
            None,
            None,
            false,
            CONTRACTION_HEARTBEAT_INTERVAL,
        )?;
    }

    let pre_cycle_status = status_snapshot(network)?;
    let pre_cycle_elastic_storage = elastic_lane_storage_snapshot(network, elastic_lane_id)?;
    let pre_cycle_autoscale_transitions =
        autoscale_transition_snapshot_for_lane(network, elastic_lane_id)?;
    let pre_cycle_max_non_empty_height = max_non_empty_height(&pre_cycle_status);
    let pre_cycle_max_txs_approved = max_txs_approved(&pre_cycle_status);
    let pre_cycle_max_txs_rejected = max_txs_rejected(&pre_cycle_status);

    let load_started = Instant::now();
    let load_report = submit_load_round_robin(submitters, load_tx_count)?;
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
        Some((submitters, &load_report)),
        pre_cycle_max_non_empty_height,
        pre_cycle_max_txs_approved,
        pre_cycle_max_txs_rejected,
        SCALE_OUT_WAIT_TIMEOUT,
        &activity_context,
        &activity_prefix,
        EXPANSION_PROBE_INTERVAL,
        tx_confirmation_status_counts_as_load_activity,
        "load activity",
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
        elastic_lane_id,
        expanded_provisioned_lanes,
        require_scale_out_transition,
        require_expansion_status_before_contraction,
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
    let post_expansion_autoscale_transitions =
        autoscale_transition_snapshot_for_lane(network, elastic_lane_id)?;
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
    ensure!(
        !require_scale_out_transition || peers_with_scale_out >= quorum_required,
        "autoscale cycle {cycle_index}: expansion profile was observed but fresh deterministic autoscale scale-out transitions were not observed on quorum peers after the cycle baseline (scale-out peers after expansion snapshot: {peers_with_scale_out}/{TOTAL_PEERS}; required quorum: {quorum_required})"
    );
    let post_expansion_status = status_snapshot(network)?;
    let peers_with_direct_applied_committed_lane_block =
        peers_with_direct_applied_committed_lane_block_for_lane(
            &post_expansion_status,
            elastic_lane_id,
        );
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] direct-applied committed lane-block peers after expansion: {peers_with_direct_applied_committed_lane_block}/{TOTAL_PEERS}"
    );
    let require_scale_in_transition_this_cycle = if require_expansion_status_before_contraction {
        require_scale_in_transition
    } else {
        should_require_scale_in_transition_for_lane(
            require_scale_in_transition,
            &post_expansion_status,
            &pre_cycle_status,
            elastic_lane_id,
            quorum_required,
        )
    };
    if require_scale_in_transition && !require_scale_in_transition_this_cycle {
        eprintln!(
            "[autoscale-localnet][cycle {cycle_index}] scale-in transition check relaxed: expansion status signal was not observed on quorum peers; validating contraction profile only"
        );
    }

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
        submitters,
        &contraction_prefix,
        initial_provisioned_lanes,
        elastic_lane_id,
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
    let post_contraction_autoscale_transitions =
        autoscale_transition_snapshot_for_lane(network, elastic_lane_id)?;
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
            scale_in_transition_quorum_satisfied(
                peers_with_scale_in_after_expansion,
                Some(peers_with_scale_in_since_cycle_start),
                quorum_required,
            ),
            "autoscale cycle {cycle_index}: contraction profile was observed but deterministic autoscale scale-in transitions were not observed on quorum peers after expansion or since the cycle baseline (scale-in peers after expansion snapshot: {peers_with_scale_in_after_expansion}/{TOTAL_PEERS}; since cycle start: {peers_with_scale_in_since_cycle_start}/{TOTAL_PEERS}; required quorum: {quorum_required})"
        );
    } else if require_scale_in_transition {
        eprintln!(
            "[autoscale-localnet][cycle {cycle_index}] scale-in transition check skipped: expansion status signal did not reach quorum during this cycle"
        );
    }
    let mut post_cycle_status = status_snapshot(network)?;
    let mut post_cycle_progress_confirmed = false;
    if !chain_progress_advanced(
        &post_cycle_status,
        pre_cycle_max_non_empty_height,
        pre_cycle_max_txs_approved,
        pre_cycle_max_txs_rejected,
    ) {
        let post_cycle_probe_client = peer_client_with_timeout(network.peer());
        let post_cycle_context = format!("autoscale cycle {cycle_index} post-cycle confirmation");
        let post_cycle_prefix = format!("autoscale-post-cycle-heartbeat-{cycle_index}");
        wait_for_chain_progress_with_heartbeat(
            network,
            &post_cycle_probe_client,
            Some((submitters, &load_report)),
            pre_cycle_max_non_empty_height,
            pre_cycle_max_txs_approved,
            pre_cycle_max_txs_rejected,
            AUTOSCALE_COOLDOWN_CLEARANCE_TIMEOUT,
            &post_cycle_context,
            &post_cycle_prefix,
            CONTRACTION_HEARTBEAT_INTERVAL,
            tx_confirmation_status_counts_as_post_cycle_progress,
            "post-cycle tx progress",
        )?;
        post_cycle_progress_confirmed = true;
        post_cycle_status = status_snapshot(network)?;
    }
    let post_cycle_max_non_empty_height = max_non_empty_height(&post_cycle_status);
    let post_cycle_max_txs_approved = max_txs_approved(&post_cycle_status);
    let post_cycle_max_txs_rejected = max_txs_rejected(&post_cycle_status);
    ensure!(
        chain_progress_advanced(
            &post_cycle_status,
            pre_cycle_max_non_empty_height,
            pre_cycle_max_txs_approved,
            pre_cycle_max_txs_rejected
        ) || post_cycle_progress_confirmed,
        "autoscale cycle {cycle_index}: chain activity did not advance (blocks_non_empty: {pre_cycle_max_non_empty_height}->{post_cycle_max_non_empty_height}, txs_approved: {pre_cycle_max_txs_approved}->{post_cycle_max_txs_approved}, txs_rejected: {pre_cycle_max_txs_rejected}->{post_cycle_max_txs_rejected})"
    );

    Ok(ExpandContractCycleOutcome {
        expansion_time_s,
        contraction_time_s,
        peers_with_scale_out_after_expansion: peers_with_scale_out,
        peers_with_direct_applied_committed_lane_block_after_expansion:
            peers_with_direct_applied_committed_lane_block,
        peers_with_scale_in_after_expansion,
        peers_with_scale_in_since_cycle_start,
        scale_in_transition_required: require_scale_in_transition_this_cycle,
    })
}

#[test]
fn nexus_autoscale_expands_and_contracts_lanes_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_expands_and_contracts_lanes_in_localnet);
    let _test_guard = AUTOSCALE_LOCALNET_TEST_MUTEX
        .lock()
        .expect("autoscale localnet test mutex poisoned");
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
    wait_for_submission_ready(&submitters, SUBMISSION_READY_TIMEOUT, context)?;
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum",
    )?;
    eprintln!("[autoscale-localnet] dynamic commit quorum (2f+1): {quorum_required}");

    let _cycle_outcome = {
        let mut cycle_attempt = 1_usize;
        loop {
            let attempt_load_tx_count = single_cycle_load_tx_count(cycle_attempt);
            eprintln!(
                "[autoscale-localnet] attempt {cycle_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {attempt_load_tx_count})"
            );
            match run_expand_contract_cycle(
                &network,
                &submitters,
                quorum_required,
                INITIAL_PROVISIONED_LANES,
                EXPANDED_PROVISIONED_LANES,
                ELASTIC_LANE_ID,
                1,
                cycle_attempt,
                false,
                false,
                false,
                attempt_load_tx_count,
            ) {
                Ok(outcome) => break outcome,
                Err(err) if cycle_attempt < AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT => {
                    let next_attempt = cycle_attempt.saturating_add(1);
                    let next_load_tx_count = single_cycle_load_tx_count(next_attempt);
                    eprintln!(
                        "[autoscale-localnet] attempt {cycle_attempt} failed; retrying with attempt {next_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {next_load_tx_count}): {err}"
                    );
                    cycle_attempt = next_attempt;
                }
                Err(err) => {
                    return Err(eyre!(
                        "autoscale single-cycle expansion failed after {cycle_attempt} attempt(s): {err}"
                    ));
                }
            }
        }
    };
    eprintln!(
        "[autoscale-localnet] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );

    Ok(())
}

#[test]
fn nexus_autoscale_repeats_expand_contract_cycles_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_repeats_expand_contract_cycles_in_localnet);
    let _test_guard = AUTOSCALE_LOCALNET_TEST_MUTEX
        .lock()
        .expect("autoscale localnet test mutex poisoned");
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
    wait_for_submission_ready(&submitters, SUBMISSION_READY_TIMEOUT, context)?;
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
                INITIAL_PROVISIONED_LANES,
                EXPANDED_PROVISIONED_LANES,
                ELASTIC_LANE_ID,
                cycle_index,
                cycle_attempt,
                true,
                true,
                false,
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
fn nexus_autoscale_strict_expand_contract_transitions_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_strict_expand_contract_transitions_in_localnet);
    let _test_guard = AUTOSCALE_LOCALNET_TEST_MUTEX
        .lock()
        .expect("autoscale localnet test mutex poisoned");
    configure_load_sequence_seed(None);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet][strict] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count for strict transitions",
    )?;
    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    wait_for_submission_ready(&submitters, SUBMISSION_READY_TIMEOUT, context)?;
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for strict transitions",
    )?;
    eprintln!("[autoscale-localnet][strict] dynamic commit quorum (2f+1): {quorum_required}");

    let mut cycle_attempt = 1_usize;
    loop {
        let attempt_load_tx_count = strict_cycle_load_tx_count(cycle_attempt);
        eprintln!(
            "[autoscale-localnet][strict] attempt {cycle_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {attempt_load_tx_count})"
        );
        match run_expand_contract_cycle(
            &network,
            &submitters,
            quorum_required,
            INITIAL_PROVISIONED_LANES,
            EXPANDED_PROVISIONED_LANES,
            ELASTIC_LANE_ID,
            1,
            cycle_attempt,
            true,
            true,
            true,
            attempt_load_tx_count,
        ) {
            Ok(_) => break,
            Err(err) if cycle_attempt < AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT => {
                let next_attempt = cycle_attempt.saturating_add(1);
                let next_load_tx_count = strict_cycle_load_tx_count(next_attempt);
                eprintln!(
                    "[autoscale-localnet][strict] attempt {cycle_attempt} failed; retrying with attempt {next_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {next_load_tx_count}): {err}"
                );
                cycle_attempt = next_attempt;
            }
            Err(err) => {
                return Err(eyre!(
                    "autoscale strict transition cycle failed after {cycle_attempt} attempt(s): {err}"
                ));
            }
        }
    }

    eprintln!(
        "[autoscale-localnet][strict] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[test]
fn nexus_autoscale_public_profile_strict_expand_contract_transitions_in_localnet() -> Result<()> {
    let context =
        stringify!(nexus_autoscale_public_profile_strict_expand_contract_transitions_in_localnet);
    let _test_guard = AUTOSCALE_LOCALNET_TEST_MUTEX
        .lock()
        .expect("autoscale localnet test mutex poisoned");
    configure_load_sequence_seed(None);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        autoscale_public_profile_localnet_builder(),
        context,
    )?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet][public-profile] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    wait_for_storage_lane_count(
        &network,
        PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count for public-profile strict transitions",
    )?;
    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    wait_for_submission_ready(&submitters, SUBMISSION_READY_TIMEOUT, context)?;
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for public-profile strict transitions",
    )?;
    eprintln!(
        "[autoscale-localnet][public-profile] dynamic commit quorum (2f+1): {quorum_required}"
    );

    let mut cycle_attempt = 1_usize;
    loop {
        let attempt_load_tx_count = public_profile_strict_cycle_load_tx_count(cycle_attempt);
        eprintln!(
            "[autoscale-localnet][public-profile] attempt {cycle_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {attempt_load_tx_count})"
        );
        match run_expand_contract_cycle(
            &network,
            &submitters,
            quorum_required,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            1,
            cycle_attempt,
            true,
            true,
            true,
            attempt_load_tx_count,
        ) {
            Ok(_) => break,
            Err(err) if cycle_attempt < AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT => {
                let next_attempt = cycle_attempt.saturating_add(1);
                let next_load_tx_count = public_profile_strict_cycle_load_tx_count(next_attempt);
                eprintln!(
                    "[autoscale-localnet][public-profile] attempt {cycle_attempt} failed; retrying with attempt {next_attempt}/{AUTOSCALE_SINGLE_CYCLE_RETRY_LIMIT} (load tx count: {next_load_tx_count}): {err}"
                );
                cycle_attempt = next_attempt;
            }
            Err(err) => {
                return Err(eyre!(
                    "autoscale public-profile strict transition cycle failed after {cycle_attempt} attempt(s): {err}"
                ));
            }
        }
    }

    let final_status = status_snapshot(&network)?;
    ensure!(
        contraction_observed_on_quorum_peers_for_profile(
            &final_status,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            quorum_required,
        ),
        "public-profile autoscale did not preserve base lanes 0..{} after retiring elastic lane {}; final status snapshot: {final_status:?}",
        PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
        PUBLIC_PROFILE_ELASTIC_LANE_ID,
    );

    eprintln!(
        "[autoscale-localnet][public-profile] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[test]
#[ignore = "long-running autoscale soak"]
fn nexus_autoscale_soak_expand_contract_cycles_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_soak_expand_contract_cycles_in_localnet);
    let _test_guard = AUTOSCALE_LOCALNET_TEST_MUTEX
        .lock()
        .expect("autoscale localnet test mutex poisoned");
    let soak_seed = autoscale_soak_seed();
    let soak_duration = autoscale_soak_duration();
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
    wait_for_submission_ready(&submitters, SUBMISSION_READY_TIMEOUT, context)?;
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for soak",
    )?;
    eprintln!("[autoscale-localnet][soak] dynamic commit quorum (2f+1): {quorum_required}");
    eprintln!(
        "[autoscale-localnet][soak] deterministic seed ({AUTOSCALE_SOAK_SEED_ENV}): {soak_seed}; load sequence start: {load_sequence_seed}; duration ({AUTOSCALE_SOAK_DURATION_ENV}): {:.3}s",
        soak_duration.as_secs_f64()
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
        while soak_started.elapsed() < soak_duration {
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
                    INITIAL_PROVISIONED_LANES,
                    EXPANDED_PROVISIONED_LANES,
                    ELASTIC_LANE_ID,
                    cycle_index,
                    attempt,
                    true,
                    true,
                    false,
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
    use iroha::{
        client::TxConfirmationStatus,
        crypto::Hash,
        data_model::{
            block::consensus::{
                COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
                COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
                COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
                COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
                COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            },
            nexus::DataSpaceId,
        },
    };
    use tempfile::tempdir;

    use super::{
        AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER, AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        AutoscaleSoakCycleEvent, AutoscaleSoakReporter, AutoscaleSoakRunSummary,
        AutoscaleTransitionStats, CommitQuorumObservation, CommitQuorumSource,
        CommittedLaneBlockSnapshot, ElasticLaneStorageStats, ExpandContractCycleOutcome,
        LaneCommitmentSnapshot, LaneRelaySnapshot, LaneStatusSnapshot, LaneValidatorSnapshot,
        PUBLIC_PROFILE_ELASTIC_LANE_ID, PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
        PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES, PeerStatusSnapshot, SoakTimingSummary,
        autoscale_soak_duration_from_env_value, commit_quorum_observation,
        committed_lane_block_has_canonical_quorum_metadata, committed_lane_block_is_certified,
        contraction_observed_on_quorum_peers, contraction_observed_on_quorum_peers_for_profile,
        elastic_lane_storage_progressed, expansion_observed_on_quorum_or_scale_out_transition,
        expansion_observed_on_quorum_or_scale_out_transition_for_lane,
        expansion_observed_on_quorum_peers, expansion_observed_on_quorum_peers_for_lane,
        expansion_observed_on_storage, expansion_observed_on_storage_for_count,
        expansion_observed_on_storage_for_lane_count, expansion_probe_ready,
        expansion_probe_top_up_tx_count, expansion_scaled_top_up_tx_count,
        expansion_top_up_tx_count, is_autoscale_elastic_storage_segment,
        parse_autoscale_transition_stats, parse_autoscale_transition_stats_for_lane,
        peer_committed_lane_block_snapshot, peer_direct_applied_committed_lane_block_snapshot,
        peer_lane_commitment_snapshot, peer_lane_status, peer_lane_validator_snapshot,
        peers_with_direct_applied_committed_lane_block_for_lane,
        peers_with_elastic_storage_progress, peers_with_expanded_lane_signal,
        peers_with_scale_in_transition, peers_with_scale_out_transition,
        scale_in_transition_counts, scale_in_transition_quorum_satisfied,
        scale_out_transition_observed_on_quorum_peers, should_require_scale_in_transition,
        should_require_scale_in_transition_for_lane, should_run_cooldown_clearance,
        single_cycle_load_tx_count, soak_cycle_load_tx_count, storage_lane_id,
        tx_confirmation_status_counts_as_load_activity,
        tx_confirmation_status_counts_as_post_cycle_progress, validate_load_submission_outcome,
    };

    fn status_with_declared_lanes(lane_ids: &[u32]) -> PeerStatusSnapshot {
        PeerStatusSnapshot {
            lanes: lane_ids
                .iter()
                .map(|lane_id| LaneStatusSnapshot {
                    lane_id: *lane_id,
                    capacity: 1_000,
                    committed: 1,
                })
                .collect(),
            lane_commitments: Vec::new(),
            lane_governance_ids: lane_ids.to_vec(),
            lane_relay: Vec::new(),
            lane_validators: Vec::new(),
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            txs_approved: 10,
            txs_rejected: 0,
            blocks_non_empty: 10,
            ..PeerStatusSnapshot::default()
        }
    }

    fn relay_snapshot(
        lane_id: u32,
        dataspace_id: u64,
        block_height: u64,
        descriptor_hash: Option<Hash>,
        merge_admissible: bool,
    ) -> LaneRelaySnapshot {
        LaneRelaySnapshot {
            lane_id,
            dataspace_id,
            block_height,
            descriptor_hash,
            merge_admissible,
        }
    }

    fn descriptor_backed_relay_snapshot(
        lane_id: u32,
        dataspace_id: u64,
        block_height: u64,
    ) -> LaneRelaySnapshot {
        relay_snapshot(
            lane_id,
            dataspace_id,
            block_height,
            Some(Hash::new(format!(
                "autoscale-localnet-relay-descriptor:{lane_id}:{dataspace_id}:{block_height}"
            ))),
            true,
        )
    }

    fn status_with_commit_quorum(
        commit_signatures_required: u64,
        commit_qc_validator_set_len: u64,
    ) -> PeerStatusSnapshot {
        PeerStatusSnapshot {
            commit_signatures_required,
            commit_qc_validator_set_len,
            ..PeerStatusSnapshot::default()
        }
    }

    #[test]
    fn commit_quorum_observation_requires_consistent_explicit_quorum() {
        let consistent = vec![
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 4),
        ];
        assert_eq!(
            commit_quorum_observation(&consistent, 4),
            CommitQuorumObservation::Ready {
                quorum_required: 3,
                source: CommitQuorumSource::RequiredStatus
            }
        );

        let conflicting_required = vec![
            status_with_commit_quorum(2, 4),
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(0, 4),
        ];
        let observation = commit_quorum_observation(&conflicting_required, 4);
        assert_eq!(
            observation,
            CommitQuorumObservation::ConflictingRequired {
                observed: vec![2, 3, 3]
            }
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect("conflicting required quorum should be terminal after timeout")
                .contains("conflicting commit quorum values"),
            "conflicting explicit quorum evidence must not fall back to validator-set length"
        );

        let explicit_with_conflicting_validator_len = vec![
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 5),
            status_with_commit_quorum(3, 4),
            status_with_commit_quorum(3, 0),
        ];
        let observation = commit_quorum_observation(&explicit_with_conflicting_validator_len, 4);
        assert_eq!(
            observation,
            CommitQuorumObservation::ConflictingValidatorSetLen {
                observed: vec![4, 5, 4]
            }
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect(
                    "conflicting validator-set length should remain terminal with explicit quorum"
                )
                .contains("conflicting commit-QC validator-set lengths"),
            "clean explicit quorum must not mask split-brain validator-set length evidence"
        );
    }

    #[test]
    fn commit_quorum_observation_rejects_invalid_explicit_quorum() {
        let invalid_required = vec![
            status_with_commit_quorum(5, 4),
            status_with_commit_quorum(5, 4),
            status_with_commit_quorum(5, 4),
            status_with_commit_quorum(5, 4),
        ];
        let observation = commit_quorum_observation(&invalid_required, 4);
        assert_eq!(
            observation,
            CommitQuorumObservation::InvalidRequired {
                quorum_required: 5,
                peer_count: 4,
                observed: vec![5, 5, 5, 5]
            }
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect("invalid explicit quorum should be terminal after timeout")
                .contains("invalid commit quorum value")
        );
    }

    #[test]
    fn commit_quorum_observation_rejects_downgraded_explicit_quorum() {
        let downgraded_against_validator_set = vec![
            status_with_commit_quorum(2, 4),
            status_with_commit_quorum(2, 4),
            status_with_commit_quorum(2, 4),
            status_with_commit_quorum(2, 4),
        ];
        let observation = commit_quorum_observation(&downgraded_against_validator_set, 4);
        assert_eq!(
            observation,
            CommitQuorumObservation::InvalidRequired {
                quorum_required: 2,
                peer_count: 4,
                observed: vec![2, 2, 2, 2]
            }
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect("downgraded explicit quorum should be terminal after timeout")
                .contains("invalid commit quorum value")
        );

        let downgraded_without_validator_set = vec![
            status_with_commit_quorum(2, 0),
            status_with_commit_quorum(2, 0),
            status_with_commit_quorum(2, 0),
            status_with_commit_quorum(2, 0),
        ];
        assert_eq!(
            commit_quorum_observation(&downgraded_without_validator_set, 4),
            CommitQuorumObservation::InvalidRequired {
                quorum_required: 2,
                peer_count: 4,
                observed: vec![2, 2, 2, 2]
            },
            "explicit quorum without validator-set evidence must not downgrade below peer-count quorum"
        );

        let smaller_validator_set = vec![
            status_with_commit_quorum(2, 2),
            status_with_commit_quorum(2, 2),
            status_with_commit_quorum(2, 2),
            status_with_commit_quorum(2, 2),
        ];
        assert_eq!(
            commit_quorum_observation(&smaller_validator_set, 4),
            CommitQuorumObservation::Ready {
                quorum_required: 2,
                source: CommitQuorumSource::RequiredStatus
            },
            "explicit quorum may be below peer-count quorum only when consistent validator-set evidence explains it"
        );
    }

    #[test]
    fn commit_quorum_observation_derives_only_from_consistent_validator_set_len() {
        let derived = vec![
            status_with_commit_quorum(0, 4),
            status_with_commit_quorum(0, 4),
            status_with_commit_quorum(0, 4),
            status_with_commit_quorum(0, 4),
        ];
        assert_eq!(
            commit_quorum_observation(&derived, 4),
            CommitQuorumObservation::Ready {
                quorum_required: 3,
                source: CommitQuorumSource::ValidatorSetLen
            }
        );

        let conflicting_validator_len = vec![
            status_with_commit_quorum(0, 4),
            status_with_commit_quorum(0, 5),
            status_with_commit_quorum(0, 4),
            status_with_commit_quorum(0, 0),
        ];
        let observation = commit_quorum_observation(&conflicting_validator_len, 4);
        assert_eq!(
            observation,
            CommitQuorumObservation::ConflictingValidatorSetLen {
                observed: vec![4, 5, 4]
            }
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect("conflicting validator-set length should be terminal after timeout")
                .contains("conflicting commit-QC validator-set lengths")
        );

        let invalid_validator_len = vec![
            status_with_commit_quorum(0, 5),
            status_with_commit_quorum(0, 5),
            status_with_commit_quorum(0, 5),
            status_with_commit_quorum(0, 5),
        ];
        let observation = commit_quorum_observation(&invalid_validator_len, 4);
        assert!(
            matches!(
                observation,
                CommitQuorumObservation::InvalidValidatorSetLen {
                    validator_set_len: 5,
                    derived_quorum: Some(_),
                    peer_count: 4
                }
            ),
            "validator-set length larger than the observed peer set must fail closed"
        );
        assert!(
            observation
                .timeout_error("quorum unit")
                .expect("invalid validator-set length should be terminal after timeout")
                .contains("invalid derived quorum")
        );

        let no_evidence = vec![PeerStatusSnapshot::default(); 4];
        assert_eq!(
            commit_quorum_observation(&no_evidence, 4),
            CommitQuorumObservation::NoEvidence
        );
        assert!(
            CommitQuorumObservation::NoEvidence
                .timeout_error("quorum unit")
                .is_none(),
            "peer-count fallback is reserved for the no-evidence case"
        );
    }

    fn certified_lane_block_status(
        lane_id: u32,
        height: u64,
        prepare_signers: u32,
        commit_signers: u32,
    ) -> CommittedLaneBlockSnapshot {
        CommittedLaneBlockSnapshot {
            lane_id,
            dataspace_id: 0,
            lane_block_height: height,
            lane_block_view: 0,
            descriptor_hash: Hash::new(format!("autoscale-localnet-descriptor:{lane_id}:{height}")),
            proposal_hash: Hash::new(format!("autoscale-localnet-proposal:{lane_id}:{height}")),
            subject_hash: Hash::new(format!("autoscale-localnet-subject:{lane_id}:{height}")),
            payload_ownership_hash: Hash::new(format!(
                "autoscale-localnet-payload-ownership:{lane_id}:{height}",
            )),
            rbc_instance_hash: Hash::new(format!(
                "autoscale-localnet-rbc-instance:{lane_id}:{height}",
            )),
            qc_mode_tag: format!("autoscale-localnet-qc-mode:{lane_id}:{height}"),
            execution_status: COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION
                .to_owned(),
            executable_payload_available: true,
            validator_count: 4,
            min_quorum: 3,
            prepare_qc_signer_count: prepare_signers,
            commit_qc_signer_count: commit_signers,
        }
    }

    fn committed_lane_block_status_with_execution(
        lane_id: u32,
        height: u64,
        execution_status: &str,
        executable_payload_available: bool,
    ) -> CommittedLaneBlockSnapshot {
        let mut block = certified_lane_block_status(lane_id, height, 3, 3);
        block.execution_status = execution_status.to_owned();
        block.executable_payload_available = executable_payload_available;
        block
    }

    fn status_with_committed_lane_block(
        lane_id: u32,
        height: u64,
        prepare_signers: u32,
        commit_signers: u32,
    ) -> PeerStatusSnapshot {
        PeerStatusSnapshot {
            committed_lane_blocks: vec![certified_lane_block_status(
                lane_id,
                height,
                prepare_signers,
                commit_signers,
            )],
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            ..PeerStatusSnapshot::default()
        }
    }

    #[test]
    fn committed_lane_block_status_counts_as_expansion_only_after_qc_quorum() {
        let certified = vec![
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            PeerStatusSnapshot::default(),
        ];
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &certified, None, 1, 3
        ));

        let under_quorum_prepare = vec![
            status_with_committed_lane_block(1, 1, 2, 3),
            status_with_committed_lane_block(1, 1, 2, 3),
            status_with_committed_lane_block(1, 1, 2, 3),
            PeerStatusSnapshot::default(),
        ];
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(&under_quorum_prepare, None, 1, 3),
            "prepare under-quorum committed lane-block status must not fake expansion"
        );

        let under_quorum_commit = vec![
            status_with_committed_lane_block(1, 1, 3, 2),
            status_with_committed_lane_block(1, 1, 3, 2),
            status_with_committed_lane_block(1, 1, 3, 2),
            PeerStatusSnapshot::default(),
        ];
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(&under_quorum_commit, None, 1, 3),
            "commit under-quorum committed lane-block status must not fake expansion"
        );

        let mut wrong_dataspace = vec![
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            PeerStatusSnapshot::default(),
        ];
        for peer in wrong_dataspace.iter_mut().take(3) {
            peer.committed_lane_blocks
                .first_mut()
                .expect("committed lane-block fixture")
                .dataspace_id = 42;
        }
        assert!(
            peer_committed_lane_block_snapshot(&wrong_dataspace[0], 1).is_none(),
            "wrong-dataspace committed lane-block status must not match the default autoscale route"
        );
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(&wrong_dataspace, None, 1, 3),
            "wrong-dataspace committed lane-block status must not fake expansion"
        );
    }

    #[test]
    fn direct_applied_committed_lane_block_count_requires_direct_status_and_qc() {
        let direct = committed_lane_block_status_with_execution(
            1,
            2,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );
        let canonical = committed_lane_block_status_with_execution(
            1,
            2,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            true,
        );
        let forged_direct = committed_lane_block_status_with_execution(
            1,
            2,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            false,
        );
        let mut under_quorum_direct = committed_lane_block_status_with_execution(
            1,
            3,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );
        under_quorum_direct.commit_qc_signer_count = 2;
        let wrong_lane_direct = committed_lane_block_status_with_execution(
            2,
            2,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );

        let snapshot = vec![
            PeerStatusSnapshot {
                committed_lane_blocks: vec![direct.clone()],
                ..PeerStatusSnapshot::default()
            },
            PeerStatusSnapshot {
                committed_lane_blocks: vec![canonical],
                ..PeerStatusSnapshot::default()
            },
            PeerStatusSnapshot {
                committed_lane_blocks: vec![forged_direct, under_quorum_direct],
                ..PeerStatusSnapshot::default()
            },
            PeerStatusSnapshot {
                committed_lane_blocks: vec![wrong_lane_direct, direct],
                ..PeerStatusSnapshot::default()
            },
        ];

        assert_eq!(
            peers_with_direct_applied_committed_lane_block_for_lane(&snapshot, 1),
            2,
            "only certified direct-applied lane-block receipts for the observed lane should count"
        );
        assert!(
            peer_direct_applied_committed_lane_block_snapshot(&snapshot[0], 1).is_some(),
            "certified direct application should be observable as direct evidence"
        );
        assert!(
            peer_direct_applied_committed_lane_block_snapshot(&snapshot[1], 1).is_none(),
            "canonical application receipts must not inflate direct-execution evidence"
        );
        assert!(
            peer_direct_applied_committed_lane_block_snapshot(&snapshot[2], 1).is_none(),
            "forged availability and under-quorum direct statuses must fail closed"
        );
    }

    #[test]
    fn committed_lane_block_status_rejects_ambiguous_latest_rows() {
        let certified_a = certified_lane_block_status(1, 2, 3, 3);
        let mut certified_b = certified_lane_block_status(1, 2, 3, 3);
        certified_b.commit_qc_signer_count = 4;
        let stale = certified_lane_block_status(1, 1, 3, 3);
        let conflicting_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![stale, certified_a.clone(), certified_b],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_committed_lane_block_snapshot(&conflicting_peer, 1).is_none(),
            "conflicting same-height committed lane-block rows must fail closed"
        );
        let conflicting_snapshot = vec![
            conflicting_peer.clone(),
            conflicting_peer.clone(),
            conflicting_peer,
            PeerStatusSnapshot::default(),
        ];
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(&conflicting_snapshot, None, 1, 3),
            "conflicting latest committed lane-block rows must not fake autoscale expansion"
        );

        let mut descriptor_hash_b = certified_a.clone();
        descriptor_hash_b.descriptor_hash =
            Hash::new(b"autoscale-localnet-conflicting-descriptor-hash");
        let descriptor_hash_conflicting_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![certified_a.clone(), descriptor_hash_b],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_committed_lane_block_snapshot(&descriptor_hash_conflicting_peer, 1).is_none(),
            "same-height committed lane-block rows with descriptor-hash drift must fail closed"
        );
        let descriptor_hash_conflicting_snapshot = vec![
            descriptor_hash_conflicting_peer.clone(),
            descriptor_hash_conflicting_peer.clone(),
            descriptor_hash_conflicting_peer,
            PeerStatusSnapshot::default(),
        ];
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(
                &descriptor_hash_conflicting_snapshot,
                None,
                1,
                3,
            ),
            "descriptor-hash drift must not fake autoscale expansion"
        );

        let mut proposal_hash_b = certified_a.clone();
        proposal_hash_b.proposal_hash = Hash::new(b"autoscale-localnet-conflicting-proposal-hash");
        let proposal_hash_conflicting_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![certified_a.clone(), proposal_hash_b],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_committed_lane_block_snapshot(&proposal_hash_conflicting_peer, 1).is_none(),
            "same-height committed lane-block rows with proposal-hash drift must fail closed"
        );

        let identity_drift_cases = [
            ("subject-hash", {
                let mut drifted = certified_a.clone();
                drifted.subject_hash = Hash::new(b"autoscale-localnet-conflicting-subject-hash");
                drifted
            }),
            ("payload-ownership-hash", {
                let mut drifted = certified_a.clone();
                drifted.payload_ownership_hash =
                    Hash::new(b"autoscale-localnet-conflicting-payload-ownership-hash");
                drifted
            }),
            ("rbc-instance-hash", {
                let mut drifted = certified_a.clone();
                drifted.rbc_instance_hash =
                    Hash::new(b"autoscale-localnet-conflicting-rbc-instance-hash");
                drifted
            }),
            ("qc-mode", {
                let mut drifted = certified_a.clone();
                drifted.qc_mode_tag.push_str(":drift");
                drifted
            }),
        ];
        for (field, drifted) in identity_drift_cases {
            let conflicting_peer = PeerStatusSnapshot {
                committed_lane_blocks: vec![certified_a.clone(), drifted],
                ..PeerStatusSnapshot::default()
            };
            assert!(
                peer_committed_lane_block_snapshot(&conflicting_peer, 1).is_none(),
                "same-height committed lane-block rows with {field} drift must fail closed",
            );
            let conflicting_snapshot = vec![
                conflicting_peer.clone(),
                conflicting_peer.clone(),
                conflicting_peer,
                PeerStatusSnapshot::default(),
            ];
            assert!(
                !expansion_observed_on_quorum_peers_for_lane(&conflicting_snapshot, None, 1, 3),
                "{field} drift must not fake autoscale expansion",
            );
        }

        let exact_duplicate_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![certified_a.clone(), certified_a],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_committed_lane_block_snapshot(&exact_duplicate_peer, 1).is_some(),
            "exact duplicate status rows should remain idempotent"
        );
        let exact_duplicate_snapshot = vec![
            exact_duplicate_peer.clone(),
            exact_duplicate_peer.clone(),
            exact_duplicate_peer,
            PeerStatusSnapshot::default(),
        ];
        assert!(
            expansion_observed_on_quorum_peers_for_lane(&exact_duplicate_snapshot, None, 1, 3),
            "exact duplicate committed lane-block rows should not hide valid expansion evidence"
        );

        let direct_a = committed_lane_block_status_with_execution(
            1,
            3,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );
        let mut direct_b = committed_lane_block_status_with_execution(
            1,
            3,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );
        direct_b.commit_qc_signer_count = 4;
        let direct_conflicting_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![direct_a, direct_b],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_direct_applied_committed_lane_block_snapshot(&direct_conflicting_peer, 1)
                .is_none(),
            "conflicting same-height direct-applied rows must fail closed"
        );
        let direct_conflicting_snapshot = vec![
            direct_conflicting_peer.clone(),
            direct_conflicting_peer.clone(),
            direct_conflicting_peer,
            PeerStatusSnapshot::default(),
        ];
        assert_eq!(
            peers_with_direct_applied_committed_lane_block_for_lane(
                &direct_conflicting_snapshot,
                1,
            ),
            0,
            "ambiguous direct-applied rows must not inflate direct-execution evidence"
        );

        let mut wrong_dataspace_direct = committed_lane_block_status_with_execution(
            1,
            4,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        );
        wrong_dataspace_direct.dataspace_id = 99;
        let wrong_dataspace_peer = PeerStatusSnapshot {
            committed_lane_blocks: vec![wrong_dataspace_direct],
            ..PeerStatusSnapshot::default()
        };
        assert!(
            peer_direct_applied_committed_lane_block_snapshot(&wrong_dataspace_peer, 1).is_none(),
            "wrong-dataspace direct-applied rows must not inflate direct-execution evidence"
        );
    }

    #[test]
    fn committed_lane_block_status_rejects_conflict_and_unknown_execution_states() {
        let allowed = [
            (COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD, false),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
                true,
            ),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
                true,
            ),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
                true,
            ),
            (COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK, true),
            (
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
                true,
            ),
        ];
        for (execution_status, executable_payload_available) in allowed {
            let block = committed_lane_block_status_with_execution(
                1,
                1,
                execution_status,
                executable_payload_available,
            );
            assert!(
                committed_lane_block_is_certified(&block),
                "{execution_status} with matching availability should count as certified progress"
            );
        }

        let conflict = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
            false,
        );
        assert!(
            !committed_lane_block_is_certified(&conflict),
            "receipt/preflight conflicts must not fake autoscale expansion progress"
        );

        let preflight_rejected = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            true,
        );
        assert!(
            !committed_lane_block_is_certified(&preflight_rejected),
            "direct-execution preflight rejections must not fake autoscale expansion progress"
        );

        let predecessor_blocked = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            false,
        );
        assert!(
            !committed_lane_block_is_certified(&predecessor_blocked),
            "predecessor-blocked lane blocks must not fake autoscale expansion progress"
        );

        let forged_predecessor_blocked = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            true,
        );
        assert!(
            !committed_lane_block_is_certified(&forged_predecessor_blocked),
            "predecessor-blocked status must not become progress with a forged executable flag"
        );

        let forged_direct_applied = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            false,
        );
        assert!(
            !committed_lane_block_is_certified(&forged_direct_applied),
            "direct-applied status must not become progress with a forged missing executable flag"
        );

        let unknown_executable =
            committed_lane_block_status_with_execution(1, 1, "future_unknown_state", true);
        assert!(
            !committed_lane_block_is_certified(&unknown_executable),
            "unknown executable statuses must be audited before rollout parsers count them"
        );

        let forged_awaiting = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            true,
        );
        assert!(
            !committed_lane_block_is_certified(&forged_awaiting),
            "awaiting status must not also claim executable payload availability"
        );

        let forged_recovered = committed_lane_block_status_with_execution(
            1,
            1,
            COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
            false,
        );
        assert!(
            !committed_lane_block_is_certified(&forged_recovered),
            "payload-ready statuses must carry matching executable availability"
        );
    }

    #[test]
    fn committed_lane_block_progress_requires_monotonic_certified_height() {
        let baseline = vec![
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            PeerStatusSnapshot::default(),
        ];
        let progressed = vec![
            status_with_committed_lane_block(1, 2, 3, 3),
            status_with_committed_lane_block(1, 2, 3, 3),
            status_with_committed_lane_block(1, 2, 3, 3),
            PeerStatusSnapshot::default(),
        ];
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &progressed,
            Some(&baseline),
            1,
            3
        ));

        let stale = vec![
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            status_with_committed_lane_block(1, 1, 3, 3),
            PeerStatusSnapshot::default(),
        ];
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(&stale, Some(&baseline), 1, 3),
            "same-height committed lane-block status must not fake post-baseline progress"
        );
    }

    fn utilization_permille_for_probe_tx(active_lanes: u64) -> u64 {
        let latency_ms = u64::try_from(super::EXPANSION_PROBE_INTERVAL.as_millis())
            .expect("probe interval milliseconds must fit into u64");
        let per_lane_target_tps = u64::try_from(super::LOCALNET_AUTOSCALE_PER_LANE_TARGET_TPS)
            .expect("localnet autoscale target must be positive");
        1_000_000_u64
            .saturating_div(latency_ms.max(1))
            .saturating_div(
                active_lanes
                    .max(1)
                    .saturating_mul(per_lane_target_tps)
                    .max(1),
            )
    }

    fn ratio_permille(ratio: f64) -> u64 {
        (ratio * 1_000.0).round() as u64
    }

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
            expansion_probe_top_up_tx_count(4, false, Duration::from_secs(8), 96, false, false),
            24
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(4, true, Duration::from_secs(7), 96, false, false),
            24
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(1, true, Duration::from_secs(8), 96, false, false),
            96
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(7, true, Duration::from_secs(15), 384, false, false),
            384
        );
    }

    #[test]
    fn strict_expansion_probe_stops_top_up_after_storage_expands() {
        assert_eq!(
            expansion_probe_top_up_tx_count(7, true, Duration::from_secs(15), 384, true, false),
            0
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(4, true, Duration::from_secs(7), 96, true, false),
            24
        );
    }

    #[test]
    fn strict_expansion_probe_stops_top_up_after_scale_out_quorum() {
        assert_eq!(
            expansion_probe_top_up_tx_count(4, false, Duration::from_secs(1), 384, true, true),
            0
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(12, true, Duration::from_secs(7), 384, true, true),
            0
        );
    }

    #[test]
    fn expansion_probe_ready_requires_status_when_strict_status_requested() {
        assert!(
            expansion_probe_ready(false, true, true, false),
            "strict transition mode may accept transition evidence when status is not explicitly required"
        );
        assert!(
            !expansion_probe_ready(false, true, true, true),
            "strict status mode must not accept transition evidence without expanded-lane status"
        );
        assert!(
            !expansion_probe_ready(true, false, true, true),
            "strict status mode must still require fresh scale-out transition evidence"
        );
        assert!(
            expansion_probe_ready(true, true, true, true),
            "strict status mode is ready only when both evidence streams agree"
        );
        assert!(
            expansion_probe_ready(true, false, false, false),
            "non-strict expansion can use status evidence alone"
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
    fn tx_confirmation_statuses_count_as_load_activity() {
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Queued
        ));
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Approved(None)
        ));
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Committed
        ));
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Applied
        ));
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Rejected(None)
        ));
        assert!(tx_confirmation_status_counts_as_load_activity(
            &TxConfirmationStatus::Expired
        ));
    }

    #[test]
    fn tx_confirmation_statuses_count_as_post_cycle_progress() {
        assert!(!tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Queued
        ));
        assert!(tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Approved(None)
        ));
        assert!(tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Committed
        ));
        assert!(tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Applied
        ));
        assert!(tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Rejected(None)
        ));
        assert!(tx_confirmation_status_counts_as_post_cycle_progress(
            &TxConfirmationStatus::Expired
        ));
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
    fn autoscale_soak_duration_env_value_preserves_long_default_unless_positive_seconds() {
        assert_eq!(
            autoscale_soak_duration_from_env_value(None),
            super::AUTOSCALE_SOAK_DURATION
        );
        assert_eq!(
            autoscale_soak_duration_from_env_value(Some("")),
            super::AUTOSCALE_SOAK_DURATION
        );
        assert_eq!(
            autoscale_soak_duration_from_env_value(Some("0")),
            super::AUTOSCALE_SOAK_DURATION
        );
        assert_eq!(
            autoscale_soak_duration_from_env_value(Some("not-a-number")),
            super::AUTOSCALE_SOAK_DURATION
        );
        assert_eq!(
            autoscale_soak_duration_from_env_value(Some(" 120 ")),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn single_cycle_load_profile_escalates_per_attempt() {
        assert_eq!(
            single_cycle_load_tx_count(0),
            super::STRICT_CYCLE_LOAD_TX_COUNT
        );
        assert_eq!(
            single_cycle_load_tx_count(1),
            super::STRICT_CYCLE_LOAD_TX_COUNT
        );
        assert_eq!(
            single_cycle_load_tx_count(2),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 2
        );
        assert_eq!(
            single_cycle_load_tx_count(3),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 3
        );
        assert_eq!(
            single_cycle_load_tx_count(4),
            super::STRICT_CYCLE_LOAD_TX_COUNT * 4
        );
    }

    #[test]
    fn cooldown_clearance_runs_for_later_cycles_and_retries() {
        assert!(!should_run_cooldown_clearance(1, 1));
        assert!(should_run_cooldown_clearance(2, 1));
        assert!(!should_run_cooldown_clearance(1, 2));
        assert!(!should_run_cooldown_clearance(1, 3));
        assert!(!should_run_cooldown_clearance(2, 2));
        assert!(!should_run_cooldown_clearance(2, 3));
    }

    #[test]
    fn localnet_probe_heartbeat_stays_below_autoscale_thresholds() {
        assert!(
            utilization_permille_for_probe_tx(1)
                < ratio_permille(super::LOCALNET_AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO),
            "one probe tx must not pre-trigger scale-out before the cycle baseline"
        );
        assert!(
            utilization_permille_for_probe_tx(2)
                < ratio_permille(super::LOCALNET_AUTOSCALE_SCALE_IN_UTILIZATION_RATIO),
            "one probe tx across expanded localnet lanes must still be cold enough for scale-in"
        );
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
        assert!(elastic_lane_storage_progressed(Some(baseline), None,));
    }

    #[test]
    fn elastic_lane_storage_progress_rejects_metadata_only_static_or_missing_state() {
        let baseline = ElasticLaneStorageStats {
            file_count: 3,
            total_bytes: 300,
            newest_modified_unix_ms: 123,
        };

        assert!(!elastic_lane_storage_progressed(
            Some(baseline),
            Some(baseline),
        ));
        assert!(
            !elastic_lane_storage_progressed(
                Some(ElasticLaneStorageStats {
                    file_count: 3,
                    total_bytes: 300,
                    newest_modified_unix_ms: 124,
                }),
                Some(baseline),
            ),
            "metadata-only timestamp movement must not satisfy elastic-lane storage progress"
        );
        assert!(!elastic_lane_storage_progressed(None, Some(baseline)));
        assert!(!elastic_lane_storage_progressed(None, None));
    }

    #[test]
    fn elastic_lane_storage_progress_requires_aligned_peer_snapshots() {
        let baseline = ElasticLaneStorageStats {
            file_count: 1,
            total_bytes: 100,
            newest_modified_unix_ms: 1,
        };
        let progressed = ElasticLaneStorageStats {
            file_count: 2,
            total_bytes: 120,
            newest_modified_unix_ms: 2,
        };
        let current = vec![Some(progressed); super::TOTAL_PEERS];
        let aligned_baseline = vec![Some(baseline); super::TOTAL_PEERS];

        assert_eq!(
            peers_with_elastic_storage_progress(&current, &aligned_baseline),
            super::TOTAL_PEERS
        );
        assert_eq!(
            peers_with_elastic_storage_progress(&current[..3], &aligned_baseline),
            0
        );
        assert_eq!(
            peers_with_elastic_storage_progress(&current, &aligned_baseline[..3]),
            0
        );
        assert_eq!(peers_with_elastic_storage_progress(&current, &[]), 0);
    }

    #[test]
    fn storage_lane_id_rejects_prefix_spoofed_segments() {
        assert_eq!(storage_lane_id("lane_003_elastic_lane_3"), Some(3));
        assert_eq!(storage_lane_id("lane_003_elastic3"), Some(3));
        assert!(is_autoscale_elastic_storage_segment(
            "lane_003_elastic_lane_3",
            3
        ));
        assert!(!is_autoscale_elastic_storage_segment(
            "lane_003_elastic3",
            3
        ));
        assert!(!is_autoscale_elastic_storage_segment(
            "lane_003_duplicate",
            3
        ));

        assert_eq!(storage_lane_id("lane_003"), None);
        assert_eq!(storage_lane_id("lane_003_"), None);
        assert_eq!(storage_lane_id("lane_003shadow"), None);
        assert_eq!(storage_lane_id("lane_003-elastic"), None);
        assert_eq!(storage_lane_id("lane_003.0"), None);
        assert_eq!(storage_lane_id("lane_003__elastic"), None);
        assert_eq!(storage_lane_id("lane_003_elastic_"), None);
        assert_eq!(storage_lane_id("lane_03_elastic_lane_3"), None);
        assert_eq!(storage_lane_id("lane_0003_elastic_lane_3"), None);
        assert_eq!(storage_lane_id("prefix_lane_003_elastic_lane_3"), None);
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
    fn autoscale_transition_stats_parse_lane_specific_log_markers() {
        let log = format!(
            "INFO \u{1b}[32mheight\u{1b}[0m=2 \u{1b}[32mlane\u{1b}[0m=3 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=3 lane=3; active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=4 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             {{\"height\":4,\"lane\":3,\"active_lanes\":3,\"autoscale_capacity_lanes\":3,\"out_latency_ratio_permille\":1200,\"out_utilization_p95_permille\":700,\"message\":\"{out}\"}}\n\
             height: 5 lane: 3 active_lanes: 3 autoscale_capacity_lanes: 3 in_latency_ratio_permille: 700 in_utilization_p95_permille: 20 {input}\n\
             lane=3 {out}\n\
             height=2 lane=3 active_lanes=3 {out}\n\
             height=2 lane=3 autoscale_capacity_lanes=1 {out}\n\
             height=2 lane=3 active_lanes=3 autoscale_capacity_lanes=3 in_latency_ratio_permille=700 in_utilization_p95_permille=20 {out}\n\
             height=2 lane=3 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {input}\n\
             target_lane=3 height=2 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             target-lane=3 height=2 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=03 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=3shadow active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=3_extra active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=3.0 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=3-ghost active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             height=2 lane=30 active_lanes=3 autoscale_capacity_lanes=3 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(lane_three.scale_out_transitions, 3);
        assert_eq!(lane_three.scale_in_transitions, 1);

        let lane_four = parse_autoscale_transition_stats_for_lane(&log, 4);
        assert_eq!(lane_four.scale_out_transitions, 1);
        assert_eq!(lane_four.scale_in_transitions, 0);

        let lane_one = parse_autoscale_transition_stats_for_lane(&log, 1);
        assert_eq!(
            lane_one.scale_out_transitions, 0,
            "lane=30 must not be counted as lane=1"
        );
        assert_eq!(lane_one.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_stats_parse_tracing_target_log_markers() {
        let log = format!(
            "  \u{1b}[2m2026-06-30T14:07:21.764405Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[1;32miroha_core::state\u{1b}[0m\u{1b}[32m: \u{1b}[32m{out}, \u{1b}[1;32mheight\u{1b}[0m\u{1b}[32m: 3, \u{1b}[1;32mlane\u{1b}[0m\u{1b}[32m: 3, \u{1b}[1;32mactive_lanes\u{1b}[0m\u{1b}[32m: 3, \u{1b}[1;32mautoscale_capacity_lanes\u{1b}[0m\u{1b}[32m: 3, \u{1b}[1;32mout_latency_ratio_permille\u{1b}[0m\u{1b}[32m: 189, \u{1b}[1;32mout_utilization_p95_permille\u{1b}[0m\u{1b}[32m: 284\u{1b}[0m\n\
             2026-06-30T14:07:23.860632Z WARN iroha_core::state: {input}, height: 9, lane: 3, active_lanes: 4, autoscale_capacity_lanes: 4, in_latency_ratio_permille: 700, in_utilization_p95_permille: 20",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(lane_three.scale_out_transitions, 1);
        assert_eq!(lane_three.scale_in_transitions, 1);
    }

    #[test]
    fn autoscale_transition_stats_reject_mismatched_capacity_fields() {
        let log = format!(
            "INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=1 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO {{\"height\":2,\"lane\":3,\"active_lanes\":4,\"autoscale_capacity_lanes\":1,\"in_latency_ratio_permille\":600,\"in_utilization_p95_permille\":20,\"message\":\"{input}\"}}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 {input}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "scale-out evidence must reject stale logs where active_lanes and autoscale_capacity_lanes disagree"
        );
        assert_eq!(
            lane_three.scale_in_transitions, 1,
            "scale-in evidence must reject stale logs where active_lanes and autoscale_capacity_lanes disagree"
        );
    }

    #[test]
    fn autoscale_transition_stats_reject_zero_capacity_fields() {
        let log = format!(
            "INFO height=2 lane=3 active_lanes=0 autoscale_capacity_lanes=0 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO {{\"height\":2,\"lane\":3,\"active_lanes\":0,\"autoscale_capacity_lanes\":0,\"in_latency_ratio_permille\":600,\"in_utilization_p95_permille\":20,\"message\":\"{input}\"}}\n\
             INFO height=2 lane=3 active_lanes=1 autoscale_capacity_lanes=1 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=1 autoscale_capacity_lanes=1 in_latency_ratio_permille=600 in_utilization_p95_permille=20 {input}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "zero active/capacity lane transition markers must not satisfy scale-out evidence"
        );
        assert_eq!(
            lane_three.scale_in_transitions, 1,
            "zero active/capacity lane transition markers must not satisfy scale-in evidence"
        );
    }

    #[test]
    fn autoscale_transition_stats_reject_keyed_tracing_target_spoofing() {
        let log = format!(
            "INFO details: iroha_core::state: {out}, height: 3, lane: 3, active_lanes: 3, autoscale_capacity_lanes: 3, out_latency_ratio_permille: 189, out_utilization_p95_permille: 284\n\
             INFO detail=iroha_core::state: {out}, height: 3, lane: 3, active_lanes: 3, autoscale_capacity_lanes: 3, out_latency_ratio_permille: 189, out_utilization_p95_permille: 284",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(lane_three.scale_out_transitions, 0);
        assert_eq!(lane_three.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_stats_reject_ambiguous_lane_fields() {
        let log = format!(
            "INFO height=2 lane=3 lane=4 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=4, lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 {input}\n\
             INFO {{\"height\":2,\"lane\":4,\"active_lanes\":4,\"autoscale_capacity_lanes\":1,\"out_latency_ratio_permille\":1200,\"out_utilization_p95_permille\":700}} stale lane=3 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "only the final unambiguous lane=3 scale-out line should count"
        );
        assert_eq!(
            lane_three.scale_in_transitions, 0,
            "conflicting lane fields must not fake a lane=3 scale-in transition"
        );

        let lane_four = parse_autoscale_transition_stats_for_lane(&log, 4);
        assert_eq!(
            lane_four.scale_out_transitions, 0,
            "conflicting lane fields must not count for the structured lane=4 line"
        );
        assert_eq!(lane_four.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_stats_reject_duplicate_producer_fields() {
        let log = format!(
            "INFO height=2 height=3 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 active_lanes=5 autoscale_capacity_lanes=1 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 autoscale_capacity_lanes=2 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 in_utilization_p95_permille=20 {input}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "only the unambiguous scale-out line should count"
        );
        assert_eq!(
            lane_three.scale_in_transitions, 0,
            "duplicate producer fields must not fake a scale-in transition"
        );
    }

    #[test]
    fn autoscale_transition_stats_deduplicates_same_height_and_rejects_conflicts() {
        let baseline_log = format!(
            "INFO height=10 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );
        let duplicate_current_log = format!(
            "{baseline_log}\n\
             INFO height=10 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );
        let conflicting_current_log = format!(
            "{duplicate_current_log}\n\
             INFO height=10 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1300 out_utilization_p95_permille=700 {out}\n\
             INFO height=12 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 {input}\n\
             INFO height=12 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=21 {input}\n\
             INFO height=13 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 {input}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );
        let progressed_current_log = format!(
            "{duplicate_current_log}\n\
             INFO height=11 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );

        let baseline = parse_autoscale_transition_stats_for_lane(&baseline_log, 3);
        let duplicate_current =
            parse_autoscale_transition_stats_for_lane(&duplicate_current_log, 3);
        let conflicting_current =
            parse_autoscale_transition_stats_for_lane(&conflicting_current_log, 3);
        let progressed_current =
            parse_autoscale_transition_stats_for_lane(&progressed_current_log, 3);

        assert_eq!(
            duplicate_current.scale_out_transitions, baseline.scale_out_transitions,
            "exact duplicate transition rows for one height must be idempotent"
        );
        assert_eq!(
            conflicting_current.scale_out_transitions, 0,
            "conflicting same-height scale-out rows must be dropped as ambiguous"
        );
        assert_eq!(
            conflicting_current.scale_out_ambiguous_heights, 1,
            "the conflicting scale-out height must remain visible as ambiguous evidence"
        );
        assert_eq!(
            conflicting_current.scale_in_transitions, 1,
            "conflicting same-height scale-in rows must be dropped while later clean heights count"
        );
        assert_eq!(
            conflicting_current.scale_in_ambiguous_heights, 1,
            "the conflicting scale-in height must remain visible as ambiguous evidence"
        );
        assert_eq!(
            progressed_current.scale_out_transitions,
            baseline.scale_out_transitions + 1,
            "a new clean height must still count as fresh transition evidence"
        );

        let duplicate_snapshot = vec![duplicate_current; 4];
        let baseline_snapshot = vec![baseline; 4];
        assert!(
            !scale_out_transition_observed_on_quorum_peers(
                &duplicate_snapshot,
                &baseline_snapshot,
                3,
            ),
            "same-height duplicate rows must not manufacture fresh scale-out quorum"
        );

        let partial_progressed_snapshot = vec![progressed_current; 3];
        assert!(!scale_out_transition_observed_on_quorum_peers(
            &partial_progressed_snapshot,
            &baseline_snapshot,
            3,
        ));
        assert_eq!(
            peers_with_scale_out_transition(&partial_progressed_snapshot, &baseline_snapshot),
            0,
            "transition deltas must fail closed when current and baseline peer counts differ"
        );

        let progressed_snapshot = vec![progressed_current; 4];
        assert!(scale_out_transition_observed_on_quorum_peers(
            &progressed_snapshot,
            &baseline_snapshot,
            3,
        ));
        assert_eq!(
            peers_with_scale_out_transition(&progressed_snapshot, &baseline_snapshot[..3]),
            0,
            "transition deltas must fail closed when baseline peer rows are missing"
        );
        let scale_in_progressed_current = AutoscaleTransitionStats {
            scale_in_transitions: baseline.scale_in_transitions + 1,
            ..AutoscaleTransitionStats::default()
        };
        let scale_in_progressed_snapshot = vec![scale_in_progressed_current; 4];
        assert_eq!(
            peers_with_scale_in_transition(&scale_in_progressed_snapshot, &baseline_snapshot),
            4,
            "clean scale-in deltas should count when peer rows align"
        );
        assert_eq!(
            peers_with_scale_in_transition(&scale_in_progressed_snapshot, &baseline_snapshot[..3]),
            0,
            "scale-in transition deltas must fail closed when baseline peer rows are missing"
        );

        let ambiguous_baseline = AutoscaleTransitionStats {
            scale_out_transitions: 1,
            scale_in_transitions: 1,
            scale_out_ambiguous_heights: 1,
            scale_in_ambiguous_heights: 1,
        };
        let repaired_current = AutoscaleTransitionStats {
            scale_out_transitions: 2,
            scale_in_transitions: 2,
            ..AutoscaleTransitionStats::default()
        };
        let ambiguous_current = AutoscaleTransitionStats {
            scale_out_transitions: 2,
            scale_in_transitions: 2,
            scale_out_ambiguous_heights: 1,
            scale_in_ambiguous_heights: 1,
        };
        let clean_baseline = AutoscaleTransitionStats {
            scale_out_transitions: 1,
            scale_in_transitions: 1,
            ..AutoscaleTransitionStats::default()
        };

        assert_eq!(
            peers_with_scale_out_transition(
                &vec![repaired_current; 3],
                &vec![ambiguous_baseline; 3]
            ),
            0,
            "ambiguous baseline scale-out evidence must not be repairable into fresh quorum"
        );
        assert_eq!(
            peers_with_scale_in_transition(
                &vec![repaired_current; 3],
                &vec![ambiguous_baseline; 3]
            ),
            0,
            "ambiguous baseline scale-in evidence must not be repairable into fresh quorum"
        );
        assert_eq!(
            peers_with_scale_out_transition(&vec![ambiguous_current; 3], &vec![clean_baseline; 3]),
            0,
            "ambiguous current scale-out evidence must fail closed"
        );
        assert_eq!(
            peers_with_scale_in_transition(&vec![ambiguous_current; 3], &vec![clean_baseline; 3]),
            0,
            "ambiguous current scale-in evidence must fail closed"
        );
    }

    #[test]
    fn autoscale_transition_stats_reject_malformed_duplicate_producer_fields() {
        let log = format!(
            "INFO height=2 height=bogus lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 lane=bogus active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 active_lanes=bogus autoscale_capacity_lanes=1 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 autoscale_capacity_lanes=bogus out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_latency_ratio_permille=bogus out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 out_utilization_p95_permille=bogus {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 in_latency_ratio_permille=600 in_utilization_p95_permille=20 in_utilization_p95_permille=bogus {input}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
            input = AUTOSCALE_SCALE_IN_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "malformed duplicate fields must not be ignored as harmless text"
        );
        assert_eq!(
            lane_three.scale_in_transitions, 0,
            "malformed duplicate ratio fields must not fake a scale-in transition"
        );
    }

    #[test]
    fn autoscale_transition_stats_reject_non_ascii_or_control_numeric_separators() {
        let log = format!(
            "INFO height=2 lane=\u{00a0}3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3\u{00a0} active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200\u{2007} out_utilization_p95_permille=700 {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700\u{1f} {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 1,
            "only the canonical ASCII-delimited transition line should count"
        );
        assert_eq!(lane_three.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_stats_reject_quoted_detail_field_spoofing() {
        let log = format!(
            "INFO detail=\"height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\"\n\
             INFO message=\"{out}\" detail=\"height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700\"\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 detail=\"{out}\"\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 detail={out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 detail: {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 detail=( {out} )\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 detail=[ {out} ]\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 notmessage={out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 notmessage: {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 message=\"{out} forged\"\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}-forged\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             {{\"height\":3,\"lane\":3,\"active_lanes\":4,\"autoscale_capacity_lanes\":4,\"out_latency_ratio_permille\":1200,\"out_utilization_p95_permille\":700,\"message\":\"{out}\"}}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 2,
            "only the real structured text event and JSON message event should count"
        );
        assert_eq!(lane_three.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_stats_reject_keyed_container_field_spoofing() {
        let log = format!(
            "INFO detail=[height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700] {out}\n\
             INFO detail=(height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700) {out}\n\
             INFO detail={{height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700}} {out}\n\
             INFO detail='height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700' message=\"{out}\"\n\
             INFO message=\"{out}\" detail=[height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700]\n\
             INFO payload: [height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700] {out}\n\
             INFO height=2 lane=3 active_lanes=4 autoscale_capacity_lanes=4 out_latency_ratio_permille=1200 out_utilization_p95_permille=700 {out}\n\
             {{\"height\":3,\"lane\":3,\"active_lanes\":4,\"autoscale_capacity_lanes\":4,\"out_latency_ratio_permille\":1200,\"out_utilization_p95_permille\":700,\"message\":\"{out}\"}}",
            out = AUTOSCALE_SCALE_OUT_TRANSITION_LOG_MARKER,
        );

        let lane_three = parse_autoscale_transition_stats_for_lane(&log, 3);
        assert_eq!(
            lane_three.scale_out_transitions, 2,
            "only top-level producer fields should satisfy transition evidence"
        );
        assert_eq!(lane_three.scale_in_transitions, 0);
    }

    #[test]
    fn autoscale_transition_delta_uses_peer_baseline() {
        let baseline = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 3,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats::default(),
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 4,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
        ];

        assert_eq!(peers_with_scale_out_transition(&current, &baseline), 2);
        assert_eq!(peers_with_scale_in_transition(&current, &baseline), 2);
    }

    #[test]
    fn autoscale_transition_delta_rejects_rollbacks_and_missing_current_peers() {
        let baseline = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 3,
                scale_in_transitions: 3,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 2,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 4,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 3,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
        ];

        assert_eq!(
            peers_with_scale_out_transition(&current, &baseline),
            0,
            "scale-out transition deltas must fail closed when current peer rows are missing"
        );
        assert_eq!(
            peers_with_scale_in_transition(&current, &baseline),
            0,
            "scale-in transition deltas must fail closed when current peer rows are missing"
        );
    }

    #[test]
    fn autoscale_transition_delta_rejects_missing_baseline_peers() {
        let baseline = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 2,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 2,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 99,
                scale_in_transitions: 99,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 99,
                scale_in_transitions: 99,
                ..AutoscaleTransitionStats::default()
            },
        ];

        assert_eq!(
            peers_with_scale_out_transition(&current, &baseline),
            0,
            "peers without a matching baseline entry must not manufacture fresh scale-out quorum"
        );
        assert_eq!(
            peers_with_scale_in_transition(&current, &baseline),
            0,
            "peers without a matching baseline entry must not manufacture fresh scale-in quorum"
        );
        assert!(!scale_out_transition_observed_on_quorum_peers(
            &current, &baseline, 3
        ));
    }

    #[test]
    fn strict_scale_in_quorum_passes_with_post_expansion_transitions() {
        let baseline_since_cycle_start = vec![AutoscaleTransitionStats::default(); 4];
        let baseline_after_expansion = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
        ];
        let current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 2,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 2,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
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
    fn strict_scale_in_quorum_accepts_cycle_start_deltas_when_snapshot_races() {
        let baseline_since_cycle_start = vec![AutoscaleTransitionStats::default(); 4];
        let baseline_after_expansion = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 1,
                ..AutoscaleTransitionStats::default()
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
        assert!(scale_in_transition_quorum_satisfied(
            after_expansion,
            since_cycle_start,
            3
        ));
    }

    #[test]
    fn strict_scale_in_quorum_rejects_missing_after_expansion_and_cycle_start_deltas() {
        assert!(!scale_in_transition_quorum_satisfied(2, Some(2), 3));
        assert!(!scale_in_transition_quorum_satisfied(2, None, 3));
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
            quorum_required_max: 3,
            successful_scale_out_min_peers: Some(3),
            direct_applied_committed_lane_block_cycle_count: 2,
            direct_applied_committed_lane_block_min_peers: Some(3),
            required_scale_in_cycle_count: 2,
            required_scale_in_min_quorum_peers: Some(3),
            optional_scale_in_cycle_count: 1,
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
        assert!(root.contains_key("quorum_required_max"));
        assert!(root.contains_key("successful_scale_out_min_peers"));
        assert!(root.contains_key("direct_applied_committed_lane_block_cycle_count"));
        assert!(root.contains_key("direct_applied_committed_lane_block_min_peers"));
        assert!(root.contains_key("required_scale_in_cycle_count"));
        assert!(root.contains_key("required_scale_in_min_quorum_peers"));
        assert!(root.contains_key("optional_scale_in_cycle_count"));
        assert!(root.contains_key("scale_out_quorum_misses_total"));
        assert!(root.contains_key("scale_in_post_expansion_quorum_misses_total"));
        assert!(root.contains_key("final_result"));
        assert!(root.contains_key("failure_cycle"));
        assert!(root.contains_key("failure_reason"));
    }

    #[test]
    fn soak_reporter_rejects_successful_cycle_without_fresh_scale_out_quorum() -> Result<()> {
        let dir = tempdir()?;
        let mut reporter = AutoscaleSoakReporter::new_for_paths(
            dir.path().join("summary.json"),
            dir.path().join("events.jsonl"),
            "strict-soak-regression",
        )?;
        let stale_expansion_outcome = ExpandContractCycleOutcome {
            expansion_time_s: 0.001,
            contraction_time_s: 20.0,
            peers_with_scale_out_after_expansion: 1,
            peers_with_direct_applied_committed_lane_block_after_expansion: 0,
            peers_with_scale_in_after_expansion: 4,
            peers_with_scale_in_since_cycle_start: 4,
            scale_in_transition_required: true,
        };

        let err = reporter
            .record_cycle_success(17, 1, 3, &stale_expansion_outcome)
            .expect_err("fresh scale-out quorum misses must not be summarized as success");
        assert!(
            err.to_string()
                .contains("fresh scale-out transition quorum miss")
        );
        Ok(())
    }

    #[test]
    fn soak_reporter_rejects_successful_cycle_without_scale_in_quorum() -> Result<()> {
        let dir = tempdir()?;
        let mut reporter = AutoscaleSoakReporter::new_for_paths(
            dir.path().join("summary.json"),
            dir.path().join("events.jsonl"),
            "strict-soak-regression",
        )?;
        let missing_scale_in_outcome = ExpandContractCycleOutcome {
            expansion_time_s: 0.001,
            contraction_time_s: 20.0,
            peers_with_scale_out_after_expansion: 3,
            peers_with_direct_applied_committed_lane_block_after_expansion: 0,
            peers_with_scale_in_after_expansion: 2,
            peers_with_scale_in_since_cycle_start: 2,
            scale_in_transition_required: true,
        };

        let err = reporter
            .record_cycle_success(17, 1, 3, &missing_scale_in_outcome)
            .expect_err("missing scale-in quorum must not be summarized as success");
        assert!(
            err.to_string()
                .contains("required scale-in transition quorum miss")
        );
        Ok(())
    }

    #[test]
    fn soak_reporter_accepts_optional_cycle_without_scale_in_quorum() -> Result<()> {
        let dir = tempdir()?;
        let mut reporter = AutoscaleSoakReporter::new_for_paths(
            dir.path().join("summary.json"),
            dir.path().join("events.jsonl"),
            "strict-soak-regression",
        )?;
        let optional_scale_in_outcome = ExpandContractCycleOutcome {
            expansion_time_s: 0.001,
            contraction_time_s: 0.1,
            peers_with_scale_out_after_expansion: 3,
            peers_with_direct_applied_committed_lane_block_after_expansion: 0,
            peers_with_scale_in_after_expansion: 0,
            peers_with_scale_in_since_cycle_start: 0,
            scale_in_transition_required: false,
        };

        reporter.record_cycle_success(17, 1, 3, &optional_scale_in_outcome)?;
        assert_eq!(reporter.cycles_completed, 1);
        assert_eq!(reporter.scale_in_post_expansion_quorum_misses_total, 0);
        Ok(())
    }

    #[test]
    fn soak_reporter_summary_records_successful_quorum_minima() -> Result<()> {
        let dir = tempdir()?;
        let summary_path = dir.path().join("summary.json");
        let events_path = dir.path().join("events.jsonl");
        let mut reporter = AutoscaleSoakReporter::new_for_paths(
            summary_path.clone(),
            events_path,
            "strict-soak-regression",
        )?;
        let required_scale_in_outcome = ExpandContractCycleOutcome {
            expansion_time_s: 0.5,
            contraction_time_s: 2.0,
            peers_with_scale_out_after_expansion: 4,
            peers_with_direct_applied_committed_lane_block_after_expansion: 4,
            peers_with_scale_in_after_expansion: 2,
            peers_with_scale_in_since_cycle_start: 3,
            scale_in_transition_required: true,
        };
        let optional_scale_in_outcome = ExpandContractCycleOutcome {
            expansion_time_s: 0.4,
            contraction_time_s: 0.1,
            peers_with_scale_out_after_expansion: 3,
            peers_with_direct_applied_committed_lane_block_after_expansion: 2,
            peers_with_scale_in_after_expansion: 0,
            peers_with_scale_in_since_cycle_start: 0,
            scale_in_transition_required: false,
        };

        reporter.record_cycle_success(1, 1, 3, &required_scale_in_outcome)?;
        reporter.record_cycle_success(2, 1, 3, &optional_scale_in_outcome)?;
        reporter.finalize("pass", None, None)?;

        let summary_content = fs::read_to_string(&summary_path)?;
        let summary_json: norito::json::Value = norito::json::from_str(&summary_content)?;
        let root = summary_json
            .as_object()
            .expect("summary payload must be object");
        assert_eq!(
            root.get("quorum_required_max")
                .and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            root.get("successful_scale_out_min_peers")
                .and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            root.get("direct_applied_committed_lane_block_cycle_count")
                .and_then(norito::json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            root.get("direct_applied_committed_lane_block_min_peers")
                .and_then(norito::json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            root.get("required_scale_in_cycle_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            root.get("required_scale_in_min_quorum_peers")
                .and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            root.get("optional_scale_in_cycle_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            root.get("scale_out_quorum_misses_total")
                .and_then(norito::json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            root.get("scale_in_post_expansion_quorum_misses_total")
                .and_then(norito::json::Value::as_u64),
            Some(0)
        );
        Ok(())
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
            direct_applied_committed_lane_block_peers: Some(3),
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
            root.get("direct_applied_committed_lane_block_peers")
                .and_then(norito::json::Value::as_u64),
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
    fn public_profile_expansion_ignores_wrong_elastic_lane_signal() {
        let pre_cycle_snapshot = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut wrong_elastic_snapshot = pre_cycle_snapshot.clone();
        for peer in wrong_elastic_snapshot.iter_mut().take(3) {
            let lane = peer
                .lanes
                .iter_mut()
                .find(|lane| lane.lane_id == 1)
                .expect("wrong elastic lane must exist");
            lane.capacity += 1;
            lane.committed += 1;
        }

        assert!(expansion_observed_on_quorum_peers_for_lane(
            &wrong_elastic_snapshot,
            Some(&pre_cycle_snapshot),
            1,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &wrong_elastic_snapshot,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!should_require_scale_in_transition_for_lane(
            true,
            &wrong_elastic_snapshot,
            &pre_cycle_snapshot,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_expansion_requires_elastic_lane_three_quorum() {
        let pre_cycle_snapshot = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut partial_lane_three_snapshot = pre_cycle_snapshot.clone();
        for peer in partial_lane_three_snapshot.iter_mut().take(2) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 1_000,
                committed: 1,
            });
            peer.lane_governance_ids
                .push(PUBLIC_PROFILE_ELASTIC_LANE_ID);
        }

        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &partial_lane_three_snapshot,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        partial_lane_three_snapshot[2]
            .lane_commitments
            .push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &partial_lane_three_snapshot,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_expansion_rejects_missing_baseline_peer_rows() {
        let full_baseline = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let partial_baseline = full_baseline[..2].to_vec();
        let mut expanded_snapshot = full_baseline.clone();
        for peer in expanded_snapshot.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 1_000,
                committed: 1,
            });
            peer.lane_governance_ids
                .push(PUBLIC_PROFILE_ELASTIC_LANE_ID);
        }

        assert!(expansion_observed_on_quorum_peers_for_lane(
            &expanded_snapshot,
            None,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert_eq!(
            peers_with_expanded_lane_signal(
                &expanded_snapshot,
                Some(&partial_baseline),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            0,
            "partial baseline snapshots must not contribute expansion status evidence"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &expanded_snapshot,
            Some(&partial_baseline),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &expanded_snapshot,
            Some(&partial_baseline),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            2
        ));
    }

    #[test]
    fn public_profile_expansion_rejects_duplicate_elastic_status_rows() {
        let pre_cycle_snapshot = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut duplicate_elastic_status = pre_cycle_snapshot.clone();
        for peer in duplicate_elastic_status.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 0,
                committed: 0,
            });
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 1_000,
                committed: 1,
            });
        }

        assert_eq!(
            peers_with_expanded_lane_signal(
                &duplicate_elastic_status,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            0,
            "duplicate elastic-lane status rows are malformed evidence, even when one row is active"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &duplicate_elastic_status,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_status_gate_rejects_transition_only_expansion_signal() {
        let status_without_lane_three = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let baseline_transitions = vec![AutoscaleTransitionStats::default(); 4];
        let transition_snapshot = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats::default(),
        ];

        assert!(
            expansion_observed_on_quorum_or_scale_out_transition_for_lane(
                &status_without_lane_three,
                None,
                &transition_snapshot,
                &baseline_transitions,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3
            )
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &status_without_lane_three,
            None,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!should_require_scale_in_transition_for_lane(
            true,
            &status_without_lane_three,
            &status_without_lane_three,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_expansion_rejects_stale_elastic_status_without_progress() {
        let stale_elastic_snapshot =
            vec![status_with_declared_lanes(&[0, 1, 2, PUBLIC_PROFILE_ELASTIC_LANE_ID,]); 4];

        assert!(expansion_observed_on_quorum_peers_for_lane(
            &stale_elastic_snapshot,
            None,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &stale_elastic_snapshot,
            Some(&stale_elastic_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!should_require_scale_in_transition_for_lane(
            true,
            &stale_elastic_snapshot,
            &stale_elastic_snapshot,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_expansion_requires_progress_after_stale_commitments() {
        let mut stale_commitments = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in stale_commitments.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        }
        let mut progressed_commitments = stale_commitments.clone();
        for peer in progressed_commitments.iter_mut().take(3) {
            peer.lane_commitments[0].block_height = 43;
        }

        assert!(expansion_observed_on_quorum_peers_for_lane(
            &stale_commitments,
            None,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &stale_commitments,
            Some(&stale_commitments),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &progressed_commitments,
            Some(&stale_commitments),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_expansion_rejects_ambiguous_latest_commitments() {
        let baseline_without_elastic_commitments = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut conflicting_commitments = baseline_without_elastic_commitments.clone();
        for peer in conflicting_commitments.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 2,
                teu_total: 1,
            });
        }
        assert!(
            peer_lane_commitment_snapshot(
                &conflicting_commitments[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "conflicting latest commitment rows must be ambiguous"
        );
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(
                &conflicting_commitments,
                None,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "conflicting latest commitment rows must not fake expansion"
        );
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(
                &conflicting_commitments,
                Some(&baseline_without_elastic_commitments),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "ambiguous latest commitment rows must not fake a post-baseline lane declaration"
        );

        let mut exact_duplicates = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in exact_duplicates.iter_mut().take(3) {
            let commitment = LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            };
            peer.lane_commitments.push(commitment.clone());
            peer.lane_commitments.push(commitment);
        }
        assert!(
            peer_lane_commitment_snapshot(&exact_duplicates[0], PUBLIC_PROFILE_ELASTIC_LANE_ID)
                .is_some(),
            "exact duplicate latest commitment rows should remain idempotent"
        );
        assert!(
            expansion_observed_on_quorum_peers_for_lane(
                &exact_duplicates,
                None,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "exact duplicate commitment rows should not hide valid expansion evidence"
        );

        let mut stale_positive_latest_zero = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in stale_positive_latest_zero.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 41,
                tx_count: 1,
                teu_total: 1,
            });
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 0,
                teu_total: 0,
            });
        }
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(
                &stale_positive_latest_zero,
                None,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "stale commitment activity must not count when the latest row is idle"
        );

        let mut baseline_commitments = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in baseline_commitments.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        }
        let mut ambiguous_progress = baseline_commitments.clone();
        for peer in ambiguous_progress.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 43,
                tx_count: 2,
                teu_total: 2,
            });
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 43,
                tx_count: 3,
                teu_total: 2,
            });
        }
        assert!(
            !expansion_observed_on_quorum_peers_for_lane(
                &ambiguous_progress,
                Some(&baseline_commitments),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "ambiguous latest commitment progress must not fake post-baseline expansion"
        );
    }

    #[test]
    fn public_profile_expansion_rejects_ambiguous_baseline_lane_evidence_repairs() {
        let clean_baseline = vec![status_with_declared_lanes(&[0, 1, 2]); 4];

        let mut ambiguous_status_baseline = clean_baseline.clone();
        let mut repaired_status = clean_baseline.clone();
        for (baseline_peer, repaired_peer) in ambiguous_status_baseline
            .iter_mut()
            .zip(repaired_status.iter_mut())
            .take(3)
        {
            baseline_peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 0,
                committed: 0,
            });
            baseline_peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 1_000,
                committed: 1,
            });
            repaired_peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 2_000,
                committed: 2,
            });
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &repaired_status,
                Some(&ambiguous_status_baseline),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous baseline status rows must not be repairable into fresh expansion"
        );

        let mut ambiguous_commitment_baseline = clean_baseline.clone();
        let mut repaired_commitments = clean_baseline.clone();
        for (baseline_peer, repaired_peer) in ambiguous_commitment_baseline
            .iter_mut()
            .zip(repaired_commitments.iter_mut())
            .take(3)
        {
            baseline_peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
            baseline_peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 2,
                teu_total: 1,
            });
            repaired_peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 43,
                tx_count: 3,
                teu_total: 2,
            });
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &repaired_commitments,
                Some(&ambiguous_commitment_baseline),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous baseline commitment rows must not be repairable into fresh expansion"
        );

        let mut ambiguous_committed_block_baseline = clean_baseline.clone();
        let mut repaired_committed_blocks = clean_baseline.clone();
        for (baseline_peer, repaired_peer) in ambiguous_committed_block_baseline
            .iter_mut()
            .zip(repaired_committed_blocks.iter_mut())
            .take(3)
        {
            let certified_a = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            let mut certified_b =
                certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            certified_b.commit_qc_signer_count = 4;
            baseline_peer.committed_lane_blocks.push(certified_a);
            baseline_peer.committed_lane_blocks.push(certified_b);
            repaired_peer
                .committed_lane_blocks
                .push(certified_lane_block_status(
                    PUBLIC_PROFILE_ELASTIC_LANE_ID,
                    43,
                    3,
                    3,
                ));
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &repaired_committed_blocks,
                Some(&ambiguous_committed_block_baseline),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous baseline committed-lane rows must not be repairable into fresh expansion"
        );

        let mut ambiguous_validator_baseline = clean_baseline.clone();
        let mut repaired_validators = clean_baseline.clone();
        for (baseline_peer, repaired_peer) in ambiguous_validator_baseline
            .iter_mut()
            .zip(repaired_validators.iter_mut())
            .take(3)
        {
            baseline_peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
            baseline_peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 1,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
            repaired_peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 2,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 8,
                max_activation_height: 43,
            });
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &repaired_validators,
                Some(&ambiguous_validator_baseline),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous baseline validator rows must not be repairable into fresh expansion"
        );
    }

    #[test]
    fn public_profile_expansion_requires_target_lane_relay_progress() {
        let pre_cycle_snapshot = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut wrong_lane_relay = pre_cycle_snapshot.clone();
        for peer in wrong_lane_relay.iter_mut().take(3) {
            peer.lane_relay.push(descriptor_backed_relay_snapshot(
                1,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            ));
        }

        assert!(expansion_observed_on_quorum_peers_for_lane(
            &wrong_lane_relay,
            Some(&pre_cycle_snapshot),
            1,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &wrong_lane_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut wrong_dataspace_relay = pre_cycle_snapshot.clone();
        for peer in wrong_dataspace_relay.iter_mut().take(3) {
            peer.lane_relay.push(descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::new(42).as_u64(),
                42,
            ));
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &wrong_dataspace_relay,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "wrong-dataspace relay rows must not fake default-dataspace expansion"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &wrong_dataspace_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut descriptorless_relay = pre_cycle_snapshot.clone();
        for peer in descriptorless_relay.iter_mut().take(3) {
            peer.lane_relay.push(relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
                None,
                true,
            ));
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &descriptorless_relay,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "descriptorless default-dataspace relay rows must not fake expansion"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &descriptorless_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut non_merge_relay = pre_cycle_snapshot.clone();
        for peer in non_merge_relay.iter_mut().take(3) {
            peer.lane_relay.push(relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
                Some(Hash::new(b"autoscale-localnet-non-merge-relay")),
                false,
            ));
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &non_merge_relay,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "non-merge-admissible default-dataspace relay rows must not fake expansion"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &non_merge_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut duplicate_relay = pre_cycle_snapshot.clone();
        for peer in duplicate_relay.iter_mut().take(3) {
            let relay = descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            );
            peer.lane_relay.push(relay.clone());
            peer.lane_relay.push(relay);
        }
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &duplicate_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut ambiguous_descriptor_relay = pre_cycle_snapshot.clone();
        for peer in ambiguous_descriptor_relay.iter_mut().take(3) {
            let relay = descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            );
            let mut drifted_relay = relay.clone();
            drifted_relay.descriptor_hash =
                Some(Hash::new(b"autoscale-localnet-relay-descriptor-drift"));
            peer.lane_relay.push(relay);
            peer.lane_relay.push(drifted_relay);
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &ambiguous_descriptor_relay,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "same-height descriptor drift must not fake relay progress"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &ambiguous_descriptor_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut ambiguous_merge_relay = pre_cycle_snapshot.clone();
        for peer in ambiguous_merge_relay.iter_mut().take(3) {
            let relay = descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            );
            let mut drifted_relay = relay.clone();
            drifted_relay.merge_admissible = false;
            peer.lane_relay.push(relay);
            peer.lane_relay.push(drifted_relay);
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &ambiguous_merge_relay,
                Some(&pre_cycle_snapshot),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "same-height merge-admissibility drift must not fake relay progress"
        );
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &ambiguous_merge_relay,
            Some(&pre_cycle_snapshot),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut progressed_from_ambiguous_baseline = pre_cycle_snapshot.clone();
        for peer in progressed_from_ambiguous_baseline.iter_mut().take(3) {
            peer.lane_relay.push(descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                43,
            ));
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &progressed_from_ambiguous_baseline,
                Some(&ambiguous_descriptor_relay),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous baseline relay evidence must not be repairable into fresh expansion"
        );

        let mut stale_relay = pre_cycle_snapshot.clone();
        for peer in stale_relay.iter_mut().take(3) {
            peer.lane_relay.push(descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            ));
        }
        assert!(!expansion_observed_on_quorum_peers_for_lane(
            &stale_relay,
            Some(&stale_relay),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut progressed_relay = stale_relay.clone();
        for peer in progressed_relay.iter_mut().take(3) {
            peer.lane_relay
                .iter_mut()
                .find(|relay| relay.lane_id == PUBLIC_PROFILE_ELASTIC_LANE_ID)
                .expect("elastic-lane relay fixture")
                .block_height += 1;
        }
        assert!(expansion_observed_on_quorum_peers_for_lane(
            &progressed_relay,
            Some(&stale_relay),
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_contraction_rejects_missing_base_or_lingering_elastic_state() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        assert!(contraction_observed_on_quorum_peers_for_profile(
            &contracted_public_profile,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let missing_base_lane = vec![status_with_declared_lanes(&[0, 1]); 4];
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &missing_base_lane,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut elastic_governance = contracted_public_profile.clone();
        for peer in elastic_governance.iter_mut().take(3) {
            peer.lane_governance_ids
                .push(PUBLIC_PROFILE_ELASTIC_LANE_ID);
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &elastic_governance,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut elastic_commitment = contracted_public_profile.clone();
        for peer in elastic_commitment.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &elastic_commitment,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut elastic_relay = contracted_public_profile.clone();
        for peer in elastic_relay.iter_mut().take(3) {
            peer.lane_relay.push(relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
                None,
                false,
            ));
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &elastic_relay,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut wrong_dataspace_relay = contracted_public_profile.clone();
        for peer in wrong_dataspace_relay.iter_mut().take(3) {
            peer.lane_relay.push(descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::new(42).as_u64(),
                42,
            ));
        }
        assert!(
            contraction_observed_on_quorum_peers_for_profile(
                &wrong_dataspace_relay,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3
            ),
            "wrong-dataspace relay rows must not block default-dataspace contraction"
        );

        let mut ambiguous_relay = contracted_public_profile.clone();
        for peer in ambiguous_relay.iter_mut().take(3) {
            let relay = descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::UNIVERSAL.as_u64(),
                42,
            );
            let mut drifted_relay = relay.clone();
            drifted_relay.descriptor_hash =
                Some(Hash::new(b"autoscale-localnet-contraction-relay-drift"));
            peer.lane_relay.push(relay);
            peer.lane_relay.push(drifted_relay);
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &ambiguous_relay,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));

        let mut ambiguous_wrong_dataspace_relay = contracted_public_profile.clone();
        for peer in ambiguous_wrong_dataspace_relay.iter_mut().take(3) {
            let relay = descriptor_backed_relay_snapshot(
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                DataSpaceId::new(42).as_u64(),
                42,
            );
            let mut drifted_relay = relay.clone();
            drifted_relay.descriptor_hash =
                Some(Hash::new(b"autoscale-localnet-wrong-dataspace-relay-drift"));
            peer.lane_relay.push(relay);
            peer.lane_relay.push(drifted_relay);
        }
        assert!(
            contraction_observed_on_quorum_peers_for_profile(
                &ambiguous_wrong_dataspace_relay,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3
            ),
            "wrong-dataspace ambiguous relay rows must not block default-dataspace contraction"
        );

        let mut elastic_validator = contracted_public_profile.clone();
        for peer in elastic_validator.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 1,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 1,
                max_activation_height: 42,
            });
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &elastic_validator,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_contraction_rejects_ambiguous_committed_lane_block_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut ambiguous_committed_lane_blocks = contracted_public_profile.clone();
        for peer in ambiguous_committed_lane_blocks.iter_mut().take(3) {
            let certified_a = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            let mut certified_b =
                certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            certified_b.commit_qc_signer_count = 4;
            peer.committed_lane_blocks.push(certified_a);
            peer.committed_lane_blocks.push(certified_b);
        }
        assert!(
            peer_committed_lane_block_snapshot(
                &ambiguous_committed_lane_blocks[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "conflicting committed lane-block rows must be ambiguous"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &ambiguous_committed_lane_blocks,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "ambiguous committed lane-block rows must not prove safe lane destruction"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &ambiguous_committed_lane_blocks,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous committed lane-block rows must not fake expansion either"
        );

        let mut descriptor_hash_drift = contracted_public_profile.clone();
        for peer in descriptor_hash_drift.iter_mut().take(3) {
            let certified = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            let mut drifted = certified.clone();
            drifted.descriptor_hash =
                Hash::new(b"autoscale-localnet-public-profile-conflicting-descriptor-hash");
            peer.committed_lane_blocks.push(certified);
            peer.committed_lane_blocks.push(drifted);
        }
        assert!(
            peer_committed_lane_block_snapshot(
                &descriptor_hash_drift[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "same-height committed lane-block rows with descriptor-hash drift must be ambiguous"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &descriptor_hash_drift,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "descriptor-hash drift must not prove safe lane destruction"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &descriptor_hash_drift,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "descriptor-hash drift must not fake public-profile expansion"
        );

        let mut proposal_hash_drift = contracted_public_profile.clone();
        for peer in proposal_hash_drift.iter_mut().take(3) {
            let certified = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            let mut drifted = certified.clone();
            drifted.proposal_hash =
                Hash::new(b"autoscale-localnet-public-profile-conflicting-proposal-hash");
            peer.committed_lane_blocks.push(certified);
            peer.committed_lane_blocks.push(drifted);
        }
        assert!(
            peer_committed_lane_block_snapshot(
                &proposal_hash_drift[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "same-height committed lane-block rows with proposal-hash drift must be ambiguous"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &proposal_hash_drift,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "proposal-hash drift must not prove safe lane destruction"
        );

        let certified_template =
            certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
        let identity_drift_cases = [
            ("subject-hash", {
                let mut drifted = certified_template.clone();
                drifted.subject_hash =
                    Hash::new(b"autoscale-localnet-public-profile-conflicting-subject-hash");
                drifted
            }),
            ("payload-ownership-hash", {
                let mut drifted = certified_template.clone();
                drifted.payload_ownership_hash = Hash::new(
                    b"autoscale-localnet-public-profile-conflicting-payload-ownership-hash",
                );
                drifted
            }),
            ("rbc-instance-hash", {
                let mut drifted = certified_template.clone();
                drifted.rbc_instance_hash =
                    Hash::new(b"autoscale-localnet-public-profile-conflicting-rbc-instance-hash");
                drifted
            }),
            ("qc-mode", {
                let mut drifted = certified_template.clone();
                drifted.qc_mode_tag.push_str(":drift");
                drifted
            }),
        ];
        for (field, drifted) in identity_drift_cases {
            let mut identity_drift = contracted_public_profile.clone();
            for peer in identity_drift.iter_mut().take(3) {
                peer.committed_lane_blocks.push(certified_template.clone());
                peer.committed_lane_blocks.push(drifted.clone());
            }
            assert!(
                peer_committed_lane_block_snapshot(
                    &identity_drift[0],
                    PUBLIC_PROFILE_ELASTIC_LANE_ID
                )
                .is_none(),
                "same-height committed lane-block rows with {field} drift must be ambiguous",
            );
            assert!(
                !contraction_observed_on_quorum_peers_for_profile(
                    &identity_drift,
                    PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                    PUBLIC_PROFILE_ELASTIC_LANE_ID,
                    3,
                ),
                "{field} drift must not prove safe lane destruction",
            );
            assert_eq!(
                peers_with_expanded_lane_signal(
                    &identity_drift,
                    Some(&contracted_public_profile),
                    PUBLIC_PROFILE_ELASTIC_LANE_ID,
                ),
                0,
                "{field} drift must not fake public-profile expansion",
            );
        }

        let mut exact_committed_lane_block_duplicates = contracted_public_profile.clone();
        for peer in exact_committed_lane_block_duplicates.iter_mut().take(3) {
            let certified = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            peer.committed_lane_blocks.push(certified.clone());
            peer.committed_lane_blocks.push(certified);
        }
        assert!(
            peer_committed_lane_block_snapshot(
                &exact_committed_lane_block_duplicates[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_some(),
            "exact duplicate committed lane-block rows should remain idempotent"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &exact_committed_lane_block_duplicates,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "certified committed lane-block evidence must block lane destruction"
        );
    }

    #[test]
    fn public_profile_contraction_ignores_wrong_dataspace_committed_lane_blocks() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut wrong_dataspace_committed_lane_blocks = contracted_public_profile.clone();
        for peer in wrong_dataspace_committed_lane_blocks.iter_mut().take(3) {
            let mut certified =
                certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 3, 3);
            certified.dataspace_id = 42;
            peer.committed_lane_blocks.push(certified);
        }

        assert!(
            peer_committed_lane_block_snapshot(
                &wrong_dataspace_committed_lane_blocks[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "wrong-dataspace committed lane-block rows must not match the retired public-profile route"
        );
        assert!(
            contraction_observed_on_quorum_peers_for_profile(
                &wrong_dataspace_committed_lane_blocks,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "wrong-dataspace committed lane-block rows should not block destruction of the default-dataspace elastic lane"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &wrong_dataspace_committed_lane_blocks,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "wrong-dataspace committed lane-block rows must not fake expansion either"
        );
    }

    #[test]
    fn public_profile_committed_lane_blocks_require_canonical_quorum_metadata() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut quorum_downgraded = contracted_public_profile.clone();
        for peer in quorum_downgraded.iter_mut().take(3) {
            let mut downgraded =
                certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 1, 1);
            downgraded.validator_count = 4;
            downgraded.min_quorum = 1;
            peer.committed_lane_blocks.push(downgraded);
        }
        assert!(
            !committed_lane_block_has_canonical_quorum_metadata(
                &quorum_downgraded[0].committed_lane_blocks[0],
            ),
            "fixture must carry downgraded quorum metadata"
        );
        assert!(
            peer_committed_lane_block_snapshot(
                &quorum_downgraded[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "quorum-downgraded committed lane-block rows must not be certified progress"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &quorum_downgraded,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "quorum-downgraded committed lane-block rows must not fake expansion"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &quorum_downgraded,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "malformed default-dataspace committed lane-block rows must block safe destruction"
        );

        let mut overclaimed_signers = contracted_public_profile.clone();
        for peer in overclaimed_signers.iter_mut().take(3) {
            let overclaimed = certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 5, 5);
            peer.committed_lane_blocks.push(overclaimed);
        }
        assert!(
            !committed_lane_block_has_canonical_quorum_metadata(
                &overclaimed_signers[0].committed_lane_blocks[0],
            ),
            "fixture must carry impossible signer counts"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &overclaimed_signers,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "over-claimed committed lane-block signer counts must not fake expansion"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &overclaimed_signers,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "impossible signer-count committed lane-block rows must block safe destruction"
        );

        let mut impossible_validator_set = contracted_public_profile.clone();
        for peer in impossible_validator_set.iter_mut().take(3) {
            let mut impossible =
                certified_lane_block_status(PUBLIC_PROFILE_ELASTIC_LANE_ID, 42, 67, 67);
            impossible.validator_count = 100;
            impossible.min_quorum = 67;
            peer.committed_lane_blocks.push(impossible);
        }
        assert!(
            !committed_lane_block_has_canonical_quorum_metadata(
                &impossible_validator_set[0].committed_lane_blocks[0],
            ),
            "fixture must carry a validator set larger than the four-peer localnet"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &impossible_validator_set,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "self-consistent but impossible validator-set sizes must not fake expansion"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &impossible_validator_set,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "impossible validator-set committed lane-block rows must block safe destruction"
        );
    }

    #[test]
    fn public_profile_contraction_rejects_ambiguous_commitment_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut ambiguous_commitments = contracted_public_profile.clone();
        for peer in ambiguous_commitments.iter_mut().take(3) {
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 0,
                teu_total: 0,
            });
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 0,
            });
        }
        assert!(
            peer_lane_commitment_snapshot(
                &ambiguous_commitments[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            )
            .is_none(),
            "conflicting latest commitment rows must be ambiguous"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &ambiguous_commitments,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "ambiguous commitment rows must not prove safe lane destruction"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &ambiguous_commitments,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous commitment rows must not fake expansion either"
        );

        let mut exact_terminal_duplicates = contracted_public_profile.clone();
        for peer in exact_terminal_duplicates.iter_mut().take(3) {
            let terminal = LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 0,
                teu_total: 0,
            };
            peer.lane_commitments.push(terminal.clone());
            peer.lane_commitments.push(terminal);
        }
        assert!(
            peer_lane_commitment_snapshot(
                &exact_terminal_duplicates[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_some(),
            "exact duplicate terminal commitment rows should remain idempotent"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &exact_terminal_duplicates,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "lingering commitment rows are still lane declarations and must block destruction"
        );
    }

    #[test]
    fn public_profile_contraction_allows_terminal_validator_audit_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut terminal_validator_rows = contracted_public_profile.clone();
        for peer in terminal_validator_rows.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
        }

        assert!(contraction_observed_on_quorum_peers_for_profile(
            &terminal_validator_rows,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert_eq!(
            peers_with_expanded_lane_signal(
                &terminal_validator_rows,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            0,
            "terminal public-validator audit rows must not fake elastic-lane expansion"
        );
    }

    #[test]
    fn public_profile_contraction_rejects_ambiguous_validator_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut ambiguous_validator_rows = contracted_public_profile.clone();
        for peer in ambiguous_validator_rows.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 1,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
        }
        assert!(
            peer_lane_validator_snapshot(
                &ambiguous_validator_rows[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "conflicting validator rows must be ambiguous"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &ambiguous_validator_rows,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "ambiguous validator rows must not prove safe lane destruction"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &ambiguous_validator_rows,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "ambiguous validator rows must not fake expansion either"
        );

        let mut exact_terminal_duplicates = contracted_public_profile.clone();
        for peer in exact_terminal_duplicates.iter_mut().take(3) {
            let terminal = LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            };
            peer.lane_validators.push(terminal.clone());
            peer.lane_validators.push(terminal);
        }
        assert!(
            peer_lane_validator_snapshot(
                &exact_terminal_duplicates[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_some(),
            "exact duplicate terminal rows should remain idempotent"
        );
        assert!(
            contraction_observed_on_quorum_peers_for_profile(
                &exact_terminal_duplicates,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "exact duplicate terminal validator rows should not block contraction"
        );
    }

    #[test]
    fn public_profile_contraction_rejects_revivable_validator_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut jailed_validator_rows = contracted_public_profile.clone();
        for peer in jailed_validator_rows.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 1,
                exiting: 0,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &jailed_validator_rows,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert_eq!(
            peers_with_expanded_lane_signal(
                &jailed_validator_rows,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            3,
            "jailed public validators remain live lane-scoped state"
        );

        let mut jailed_with_terminal_noise = jailed_validator_rows.clone();
        for peer in jailed_with_terminal_noise.iter_mut().take(3) {
            let validator = peer
                .lane_validators
                .iter_mut()
                .find(|validator| validator.lane_id == PUBLIC_PROFILE_ELASTIC_LANE_ID)
                .expect("elastic lane validator snapshot");
            validator.total = validator.total.saturating_add(3);
            validator.max_activation_epoch = validator.max_activation_epoch.saturating_add(3);
            validator.max_activation_height = validator.max_activation_height.saturating_add(30);
        }
        assert_eq!(
            peers_with_expanded_lane_signal(
                &jailed_with_terminal_noise,
                Some(&jailed_validator_rows),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            0,
            "terminal audit rows beside an already-live validator must not fake fresh expansion progress"
        );

        let mut exiting_validator_rows = contracted_public_profile.clone();
        for peer in exiting_validator_rows.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 1,
                max_activation_epoch: 7,
                max_activation_height: 42,
            });
        }
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &exiting_validator_rows,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
        assert_eq!(
            peers_with_expanded_lane_signal(
                &exiting_validator_rows,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID
            ),
            3,
            "exiting public validators remain live until release"
        );
    }

    #[test]
    fn public_profile_contraction_requires_clean_quorum() {
        let mut mixed_snapshot = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in mixed_snapshot.iter_mut().skip(2) {
            peer.lane_governance_ids
                .push(PUBLIC_PROFILE_ELASTIC_LANE_ID);
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        }

        assert!(contraction_observed_on_quorum_peers_for_profile(
            &mixed_snapshot,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            2
        ));
        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &mixed_snapshot,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_contraction_rejects_zero_capacity_base_lane() {
        let mut zero_capacity_base_lane = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in zero_capacity_base_lane.iter_mut().take(3) {
            let lane = peer
                .lanes
                .iter_mut()
                .find(|lane| lane.lane_id == 2)
                .expect("base lane must exist");
            lane.capacity = 0;
            lane.committed = 0;
        }

        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &zero_capacity_base_lane,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_contraction_rejects_duplicate_base_lane_status_rows() {
        let mut duplicate_base_lane = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in duplicate_base_lane.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: 2,
                capacity: 9_000,
                committed: 10,
            });
        }

        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &duplicate_base_lane,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_contraction_rejects_duplicate_elastic_status_rows() {
        let contracted_public_profile = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        let mut duplicate_elastic_status = contracted_public_profile.clone();
        for peer in duplicate_elastic_status.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 0,
                committed: 0,
            });
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 1_000,
                committed: 1,
            });
        }
        assert!(
            peer_lane_status(&duplicate_elastic_status[0], PUBLIC_PROFILE_ELASTIC_LANE_ID)
                .is_none(),
            "duplicate elastic-lane status rows must be malformed evidence"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &duplicate_elastic_status,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "duplicate elastic-lane status rows must not prove safe lane destruction"
        );
        assert_eq!(
            peers_with_expanded_lane_signal(
                &duplicate_elastic_status,
                Some(&contracted_public_profile),
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            ),
            0,
            "duplicate elastic-lane status rows must not fake expansion either"
        );

        let mut exact_terminal_duplicates = contracted_public_profile.clone();
        for peer in exact_terminal_duplicates.iter_mut().take(3) {
            let terminal = LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 0,
                committed: 0,
            };
            peer.lanes.push(terminal.clone());
            peer.lanes.push(terminal);
        }
        assert!(
            peer_lane_status(
                &exact_terminal_duplicates[0],
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
            )
            .is_none(),
            "exact duplicate terminal status rows are still malformed evidence"
        );
        assert!(
            !contraction_observed_on_quorum_peers_for_profile(
                &exact_terminal_duplicates,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "duplicate terminal elastic-lane status rows must not prove safe lane destruction"
        );

        let mut terminal_status = contracted_public_profile.clone();
        for peer in terminal_status.iter_mut().take(3) {
            peer.lanes.push(LaneStatusSnapshot {
                lane_id: PUBLIC_PROFILE_ELASTIC_LANE_ID,
                capacity: 0,
                committed: 0,
            });
        }
        assert!(
            peer_lane_status(&terminal_status[0], PUBLIC_PROFILE_ELASTIC_LANE_ID)
                .is_some_and(|lane| lane.capacity == 0 && lane.committed == 0),
            "one terminal status row should remain unambiguous"
        );
        assert!(
            contraction_observed_on_quorum_peers_for_profile(
                &terminal_status,
                PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
                PUBLIC_PROFILE_ELASTIC_LANE_ID,
                3,
            ),
            "one terminal elastic-lane status row should remain idle"
        );
    }

    #[test]
    fn public_profile_contraction_rejects_stale_base_declarations_without_active_capacity() {
        let mut stale_base_lane = vec![status_with_declared_lanes(&[0, 1, 2]); 4];
        for peer in stale_base_lane.iter_mut().take(3) {
            peer.lanes.retain(|lane| lane.lane_id != 2);
            peer.lane_governance_ids.push(2);
            peer.lane_commitments.push(LaneCommitmentSnapshot {
                lane_id: 2,
                block_height: 42,
                tx_count: 1,
                teu_total: 1,
            });
        }

        assert!(!contraction_observed_on_quorum_peers_for_profile(
            &stale_base_lane,
            PUBLIC_PROFILE_INITIAL_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID,
            3
        ));
    }

    #[test]
    fn public_profile_storage_fallback_requires_four_lanes_on_all_peers() {
        assert!(!expansion_observed_on_storage_for_count(
            &[],
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &[],
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));

        let three_lane_storage = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                ],
            );
            4
        ];
        assert!(!expansion_observed_on_storage_for_count(
            &three_lane_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));

        let mut partial_four_lane_storage = three_lane_storage.clone();
        partial_four_lane_storage[0]
            .1
            .push("lane_003_elastic_lane_3".to_owned());
        partial_four_lane_storage[1]
            .1
            .push("lane_003_elastic_lane_3".to_owned());
        partial_four_lane_storage[2]
            .1
            .push("lane_003_elastic_lane_3".to_owned());
        assert!(!expansion_observed_on_storage_for_count(
            &partial_four_lane_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));

        let expanded_storage = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_003_elastic_lane_3".to_owned(),
                ],
            );
            4
        ];
        assert!(expansion_observed_on_storage_for_lane_count(
            &expanded_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));
        let partial_expanded_storage = expanded_storage[..3].to_vec();
        assert!(!expansion_observed_on_storage_for_count(
            &partial_expanded_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &partial_expanded_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));
        assert!(!expansion_observed_on_storage_for_count(
            &partial_four_lane_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));

        let duplicate_elastic_missing_base = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_003_elastic_lane_3".to_owned(),
                    "lane_003_duplicate".to_owned(),
                ],
            );
            4
        ];
        assert!(expansion_observed_on_storage_for_count(
            &duplicate_elastic_missing_base,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &duplicate_elastic_missing_base,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));

        let valid_profile_with_extra_spoofed_lane = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_003_elastic_lane_3".to_owned(),
                    "lane_003shadow".to_owned(),
                ],
            );
            4
        ];
        assert!(!expansion_observed_on_storage_for_lane_count(
            &valid_profile_with_extra_spoofed_lane,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));

        let valid_profile_with_duplicate_elastic = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_003_elastic_lane_3".to_owned(),
                    "lane_003_duplicate".to_owned(),
                ],
            );
            4
        ];
        assert!(!expansion_observed_on_storage_for_lane_count(
            &valid_profile_with_duplicate_elastic,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));
    }

    #[test]
    fn public_profile_storage_fallback_rejects_wrong_elastic_lane_directory() {
        let wrong_elastic_storage = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_004_elastic_lane_4".to_owned(),
                ],
            );
            4
        ];

        assert!(expansion_observed_on_storage_for_count(
            &wrong_elastic_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &wrong_elastic_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));
    }

    #[test]
    fn public_profile_storage_fallback_rejects_wrong_elastic_lane_slug() {
        let wrong_elastic_slug_storage = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_003_duplicate".to_owned(),
                ],
            );
            4
        ];

        assert!(expansion_observed_on_storage_for_count(
            &wrong_elastic_slug_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert_eq!(storage_lane_id("lane_003_duplicate"), Some(3));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &wrong_elastic_slug_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
        ));
    }

    #[test]
    fn public_profile_storage_fallback_rejects_prefix_spoofed_lane_directory() {
        let prefix_spoofed_storage = vec![
            (
                0,
                vec![
                    "lane_000_core".to_owned(),
                    "lane_001_governance".to_owned(),
                    "lane_002_zk".to_owned(),
                    "lane_003shadow".to_owned(),
                ],
            );
            4
        ];

        assert!(expansion_observed_on_storage_for_count(
            &prefix_spoofed_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES
        ));
        assert!(!expansion_observed_on_storage_for_lane_count(
            &prefix_spoofed_storage,
            PUBLIC_PROFILE_EXPANDED_PROVISIONED_LANES,
            PUBLIC_PROFILE_ELASTIC_LANE_ID
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
            },
        ];
        let baseline_transitions = vec![AutoscaleTransitionStats::default(); 4];
        let transition_snapshot = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 101,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
                ..PeerStatusSnapshot::default()
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
                    jailed: 0,
                    exiting: 0,
                    max_activation_epoch: 2,
                    max_activation_height: 102,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
    fn expansion_rejects_ambiguous_lane_validator_rows() {
        let baseline_snapshot = vec![status_with_declared_lanes(&[0]); 4];
        let mut ambiguous_live_rows = baseline_snapshot.clone();
        for peer in ambiguous_live_rows.iter_mut().take(3) {
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: 1,
                total: 4,
                active: 0,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 1,
                max_activation_height: 100,
            });
            peer.lane_validators.push(LaneValidatorSnapshot {
                lane_id: 1,
                total: 4,
                active: 1,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 1,
                max_activation_height: 100,
            });
        }
        assert!(
            peer_lane_validator_snapshot(&ambiguous_live_rows[0], 1).is_none(),
            "conflicting validator rows must not expose a selected live row"
        );
        assert!(
            !expansion_observed_on_quorum_peers(&ambiguous_live_rows, Some(&baseline_snapshot), 3,),
            "conflicting validator rows must not fake post-baseline expansion"
        );
        assert!(
            !expansion_observed_on_quorum_peers(&ambiguous_live_rows, None, 3),
            "conflicting validator rows must not fake current expansion"
        );

        let mut exact_live_duplicates = baseline_snapshot.clone();
        for peer in exact_live_duplicates.iter_mut().take(3) {
            let live = LaneValidatorSnapshot {
                lane_id: 1,
                total: 4,
                active: 1,
                pending_activation: 0,
                jailed: 0,
                exiting: 0,
                max_activation_epoch: 1,
                max_activation_height: 100,
            };
            peer.lane_validators.push(live.clone());
            peer.lane_validators.push(live);
        }
        assert!(
            peer_lane_validator_snapshot(&exact_live_duplicates[0], 1).is_some(),
            "exact duplicate live validator rows should remain idempotent"
        );
        assert!(
            expansion_observed_on_quorum_peers(&exact_live_duplicates, Some(&baseline_snapshot), 3),
            "exact duplicate live validator rows should still prove expansion"
        );
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
    fn strict_expansion_requires_fresh_scale_out_delta_after_baseline() {
        let baseline_transitions = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 1,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats::default(),
        ];
        let stale_current = baseline_transitions.clone();

        assert!(!scale_out_transition_observed_on_quorum_peers(
            &stale_current,
            &baseline_transitions,
            3,
        ));

        let fresh_current = vec![
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats {
                scale_out_transitions: 2,
                scale_in_transitions: 0,
                ..AutoscaleTransitionStats::default()
            },
            AutoscaleTransitionStats::default(),
        ];
        assert!(scale_out_transition_observed_on_quorum_peers(
            &fresh_current,
            &baseline_transitions,
            3,
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
                ..PeerStatusSnapshot::default()
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
            ..PeerStatusSnapshot::default()
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
            ..PeerStatusSnapshot::default()
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
            ..PeerStatusSnapshot::default()
        }];
        assert!(!contraction_observed_on_quorum_peers(
            &elastic_still_active,
            1
        ));
    }
}
