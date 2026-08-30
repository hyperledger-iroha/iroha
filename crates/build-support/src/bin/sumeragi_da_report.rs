#![deny(warnings)]
//! Summarise Sumeragi data-availability runs by ingesting JSON summaries emitted
//! by the large-payload integration helpers. The tool scans an artifact directory
//! (either passed as the first CLI argument or via `SUMERAGI_DA_ARTIFACT_DIR`),
//! groups runs per scenario, and renders a Markdown report containing aggregated
//! latency and throughput measurements alongside per-run details.
use norito::json::{self, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fmt::Write as _,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
type Result<T> = std::result::Result<T, ReportError>;
const SIGNED_RS16_DA_SCHEMA: &str = "signed_rs16_da_v1";
fn main() -> ExitCode {
    match emit_report(io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("sumeragi-da-report: {err}");
            ExitCode::FAILURE
        }
    }
}
fn emit_report(mut writer: impl Write) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.first().map(String::as_str), Some("--help" | "-h")) {
        writer.write_all(USAGE.as_bytes())?;
        return Ok(());
    }
    run_with_args(&mut writer, &args)
}
fn run_with_args(writer: &mut impl Write, args: &[String]) -> Result<()> {
    let root = match args {
        [] => {
            let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {
                ReportError::Input(
                    "provide an artifact directory argument or set SUMERAGI_DA_ARTIFACT_DIR".into(),
                )
            })?;
            PathBuf::from(env)
        }
        [dir] => PathBuf::from(dir),
        _ => {
            return Err(ReportError::Input(
                "expected at most one argument (artifact directory)".into(),
            ));
        }
    };
    let report = generate_report(&root)?;
    writer.write_all(report.as_bytes())?;
    Ok(())
}
fn generate_report(root: &Path) -> Result<String> {
    if !root.exists() {
        return Err(ReportError::Input(format!(
            "artifact directory {} does not exist",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(ReportError::Input(format!(
            "artifact path {} is not a directory",
            root.display()
        )));
    }
    let mut summary_paths = Vec::new();
    for entry in root.read_dir()? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".summary.json"))
            {
                summary_paths.push(path);
            }
        }
    }
    summary_paths.sort();
    if summary_paths.is_empty() {
        return Err(ReportError::EmptyDataset(root.to_path_buf()));
    }
    let mut samples = Vec::with_capacity(summary_paths.len());
    for path in summary_paths {
        samples.push(ScenarioSample::from_path(path)?);
    }
    let mut grouped: BTreeMap<String, Vec<ScenarioSample>> = BTreeMap::new();
    for sample in samples {
        grouped
            .entry(sample.scenario.clone())
            .or_default()
            .push(sample);
    }
    let mut output = String::new();
    writeln!(output, "# Sumeragi Data-Availability Report")?;
    writeln!(
        output,
        "\nProcessed {} summary file(s) from `{}`.",
        grouped.values().map(Vec::len).sum::<usize>(),
        root.display()
    )?;
    writeln!(output, "\n## Summary\n")?;
    writeln!(
        output,
        "| Scenario | Runs | Peers | Payload (MiB) | Signed DA available median (ms) | Signed DA available max (ms) | Commit median (ms) | Commit max (ms) | Throughput median (MiB/s) | Throughput min (MiB/s) | DA<=Commit | BG queue max | P2P drops max |"
    )?;
    writeln!(
        output,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )?;
    for (scenario, runs) in &grouped {
        let summary = ScenarioSummary::from_runs(scenario, runs)?;
        writeln!(
            output,
            "| {scenario} | {runs} | {peers} | {payload_mib:.2} | {da_median:.0} | {da_max:.0} | {commit_median:.0} | {commit_max:.0} | {throughput_median:.2} | {throughput_min:.2} | {availability_guard} | {queue_bg_max} | {queue_p2p_max} |",
            runs = summary.runs,
            peers = summary.peers,
            payload_mib = summary.payload_mib,
            da_median = summary.da_available.median,
            da_max = summary.da_available.max,
            commit_median = summary.commit.median,
            commit_max = summary.commit.max,
            throughput_median = summary.throughput.median,
            throughput_min = summary.throughput.min,
            availability_guard = if summary.all_available_within_commit {
                "yes"
            } else {
                "no"
            },
            queue_bg_max = summary
                .queue_bg_depth
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |stats| format!("{:.0}", stats.max)),
            queue_p2p_max = summary
                .queue_p2p_drops
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |stats| format!("{:.0}", stats.max)),
        )?;
    }
    for (scenario, runs) in grouped {
        let summary = ScenarioSummary::from_runs(&scenario, &runs)?;
        summary.render_detail(&mut output)?;
    }
    Ok(output)
}
#[derive(Debug, Clone)]
struct ScenarioSample {
    scenario: String,
    peers: u64,
    payload_bytes: u64,
    da_available_ms: f64,
    commit_ms: f64,
    throughput_mib_s: f64,
    total_chunks: u64,
    received_chunks: u64,
    availability_vote_count: u64,
    view: u64,
    height: u64,
    block_hash: String,
    source: PathBuf,
    queue: Option<QueueSample>,
    peer_metrics: Option<PeerMetricsSummary>,
}
impl ScenarioSample {
    fn from_path(path: PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(&path)?;
        let value: Value = json::from_str(&contents).map_err(|err| ReportError::Json {
            path: path.clone(),
            message: err.to_string(),
        })?;
        let root = value.as_object().ok_or_else(|| ReportError::InvalidType {
            path: path.clone(),
            field: "<root>".into(),
            expected: "object",
            actual: value_type(&value),
        })?;
        let schema = require_string(root, "schema", &path)?;
        if schema != SIGNED_RS16_DA_SCHEMA {
            return Err(ReportError::UnsupportedSchema {
                path,
                actual: schema,
            });
        }
        let scenario = require_string(root, "scenario", &path)?;
        let peers = require_u64(root, "peers", &path)?;
        let payload_bytes = require_u64(root, "payload_bytes", &path)?;
        let timings = require_object(root, "timings", &path)?;
        let da_available_ms = require_f64(timings, "da_available_ms", &path)?;
        let commit_ms = require_f64(timings, "commit_ms", &path)?;
        let throughput_mib_s = require_f64(timings, "throughput_mib_s", &path)?;
        let signed_da = require_object(root, "signed_da", &path)?;
        let total_chunks = require_u64(signed_da, "total_chunks", &path)?;
        let received_chunks = require_u64(signed_da, "received_chunks", &path)?;
        let availability_vote_count = require_u64(signed_da, "availability_vote_count", &path)?;
        let view = require_u64(signed_da, "view", &path)?;
        let height = require_u64(signed_da, "height", &path)?;
        let block_hash = require_string(signed_da, "block_hash", &path)?;
        let peer_metrics = match root.get("per_peer_metrics") {
            Some(Value::Array(list)) => Some(PeerMetricsSummary::from_array(list, &path)?),
            Some(other) => {
                return Err(ReportError::InvalidType {
                    path,
                    field: "per_peer_metrics".into(),
                    expected: "array",
                    actual: value_type(other),
                });
            }
            None => None,
        };
        if let Some(summary) = &peer_metrics
            && summary.peers as u64 != peers
        {
            return Err(ReportError::Inconsistent {
                scenario: scenario.clone(),
                detail: format!(
                    "per_peer_metrics array lists {} peers while header reports {}",
                    summary.peers, peers
                ),
            });
        }
        let queue = match root.get("queue") {
            Some(Value::Object(map)) => Some(QueueSample::from_map(map, &path)?),
            Some(other) => {
                return Err(ReportError::InvalidType {
                    path,
                    field: "queue".into(),
                    expected: "object",
                    actual: value_type(other),
                });
            }
            None => None,
        };
        Ok(Self {
            scenario,
            peers,
            payload_bytes,
            da_available_ms,
            commit_ms,
            throughput_mib_s,
            total_chunks,
            received_chunks,
            availability_vote_count,
            view,
            height,
            block_hash,
            source: path,
            queue,
            peer_metrics,
        })
    }
}
#[derive(Debug, Clone)]
struct QueueSample {
    bg_post_queue_depth_max: f64,
    p2p_queue_dropped_total_max: f64,
}
impl QueueSample {
    fn from_map(map: &Map, path: &Path) -> Result<Self> {
        let bg_post_queue_depth_max = require_f64(map, "bg_post_queue_depth_max", path)?;
        let p2p_queue_dropped_total_max = require_f64(map, "p2p_queue_dropped_total_max", path)?;
        Ok(Self {
            bg_post_queue_depth_max,
            p2p_queue_dropped_total_max,
        })
    }
}
#[derive(Debug, Clone)]
struct PeerMetricsSummary {
    peers: usize,
    payload_bytes_min: f64,
    payload_bytes_max: f64,
}
impl PeerMetricsSummary {
    fn from_array(list: &[Value], path: &Path) -> Result<Self> {
        if list.is_empty() {
            return Err(ReportError::Input(format!(
                "per_peer_metrics array in {} is empty",
                path.display()
            )));
        }
        let mut payload_bytes_min = f64::INFINITY;
        let mut payload_bytes_max = f64::NEG_INFINITY;
        for value in list {
            let obj = value.as_object().ok_or_else(|| ReportError::InvalidType {
                path: path.to_path_buf(),
                field: "per_peer_metrics[]".into(),
                expected: "object",
                actual: value_type(value),
            })?;
            let payload = require_f64(obj, "payload_bytes", path)?;
            payload_bytes_min = payload_bytes_min.min(payload);
            payload_bytes_max = payload_bytes_max.max(payload);
        }
        Ok(Self {
            peers: list.len(),
            payload_bytes_min,
            payload_bytes_max,
        })
    }
}
#[derive(Debug, Clone)]
struct ScenarioSummary {
    runs: usize,
    peers: u64,
    payload_bytes: u64,
    payload_mib: f64,
    da_available: Stats,
    commit: Stats,
    throughput: Stats,
    all_available_within_commit: bool,
    total_chunks: BTreeSet<u64>,
    availability_vote_counts: BTreeSet<u64>,
    queue_bg_depth: Option<Stats>,
    queue_p2p_drops: Option<Stats>,
    peer_metrics: Option<AggregatePeerMetrics>,
    runs_detail: Vec<ScenarioSample>,
    scenario: String,
}
impl ScenarioSummary {
    fn from_runs(scenario: &str, runs: &[ScenarioSample]) -> Result<Self> {
        if runs.is_empty() {
            return Err(ReportError::Input(format!(
                "no runs recorded for scenario {scenario}"
            )));
        }
        let mut runs_detail = runs.to_vec();
        runs_detail.sort_by(|a, b| a.source.cmp(&b.source));
        let peers = runs_detail[0].peers;
        let payload_bytes = runs_detail[0].payload_bytes;
        let mut da_available_times = Vec::with_capacity(runs.len());
        let mut commit_times = Vec::with_capacity(runs.len());
        let mut throughputs = Vec::with_capacity(runs.len());
        let mut all_available_within_commit = true;
        let mut total_chunks = BTreeSet::new();
        let mut availability_vote_counts = BTreeSet::new();
        let mut peer_metrics: Option<AggregatePeerMetrics> = None;
        let mut queue_bg_values = Vec::new();
        let mut queue_p2p_values = Vec::new();
        for run in &runs_detail {
            if run.peers != peers {
                return Err(ReportError::Inconsistent {
                    scenario: scenario.to_owned(),
                    detail: format!(
                        "run {} has peers={} which differs from expected {}",
                        run.source.display(),
                        run.peers,
                        peers
                    ),
                });
            }
            if run.payload_bytes != payload_bytes {
                return Err(ReportError::Inconsistent {
                    scenario: scenario.to_owned(),
                    detail: format!(
                        "run {} has payload_bytes={} which differs from expected {}",
                        run.source.display(),
                        run.payload_bytes,
                        payload_bytes
                    ),
                });
            }
            if let Some(peer_summary) = &run.peer_metrics
                && peer_summary.peers as u64 != peers
            {
                return Err(ReportError::Inconsistent {
                    scenario: scenario.to_owned(),
                    detail: format!(
                        "run {} reports {} peer metrics while scenario expects {} peers",
                        run.source.display(),
                        peer_summary.peers,
                        peers
                    ),
                });
            }
            da_available_times.push(run.da_available_ms);
            commit_times.push(run.commit_ms);
            throughputs.push(run.throughput_mib_s);
            all_available_within_commit &= run.da_available_ms <= run.commit_ms;
            total_chunks.insert(run.total_chunks);
            availability_vote_counts.insert(run.availability_vote_count);
            if let Some(sample_peer_metrics) = &run.peer_metrics {
                let aggregate = peer_metrics.get_or_insert_with(AggregatePeerMetrics::default);
                aggregate.ingest(sample_peer_metrics);
            }
            if let Some(queue) = &run.queue {
                queue_bg_values.push(queue.bg_post_queue_depth_max);
                queue_p2p_values.push(queue.p2p_queue_dropped_total_max);
            }
        }
        let queue_bg_depth = if queue_bg_values.is_empty() {
            None
        } else {
            Some(Stats::from_values(&queue_bg_values))
        };
        let queue_p2p_drops = if queue_p2p_values.is_empty() {
            None
        } else {
            Some(Stats::from_values(&queue_p2p_values))
        };
        Ok(Self {
            runs: runs_detail.len(),
            peers,
            payload_bytes,
            payload_mib: u64_to_f64(payload_bytes) / (1024.0 * 1024.0),
            da_available: Stats::from_values(&da_available_times),
            commit: Stats::from_values(&commit_times),
            throughput: Stats::from_values(&throughputs),
            all_available_within_commit,
            total_chunks,
            availability_vote_counts,
            queue_bg_depth,
            queue_p2p_drops,
            peer_metrics,
            runs_detail,
            scenario: scenario.to_owned(),
        })
    }
    fn render_detail(&self, output: &mut String) -> Result<()> {
        self.render_overview(output)?;
        self.render_queue_stats(output)?;
        self.render_peer_metrics(output)?;
        self.render_runs_table(output)?;
        Ok(())
    }
    fn render_overview(&self, output: &mut String) -> Result<()> {
        writeln!(output, "\n### {}\n", self.scenario)?;
        writeln!(output, "- runs: {}", self.runs)?;
        writeln!(output, "- peers: {}", self.peers)?;
        writeln!(
            output,
            "- payload: {} bytes ({:.2} MiB)",
            self.payload_bytes, self.payload_mib
        )?;
        writeln!(
            output,
            "- Signed RS16 chunks observed: {}",
            format_u64_set(&self.total_chunks)
        )?;
        writeln!(
            output,
            "- Availability vote counts: {}",
            format_u64_set(&self.availability_vote_counts)
        )?;
        writeln!(
            output,
            "- DA<=Commit observed: {}",
            if self.all_available_within_commit {
                "yes"
            } else {
                "no"
            }
        )?;
        writeln!(
            output,
            "- Signed DA availability mean (ms): {:.2}",
            self.da_available.mean
        )?;
        writeln!(output, "- Commit mean (ms): {:.2}", self.commit.mean)?;
        writeln!(
            output,
            "- Throughput mean (MiB/s): {:.2}",
            self.throughput.mean
        )?;
        Ok(())
    }
    fn render_queue_stats(&self, output: &mut String) -> Result<()> {
        if let Some(stats) = &self.queue_bg_depth {
            writeln!(
                output,
                "- BG post queue depth max/median: {:.0} / {:.0}",
                stats.max, stats.median
            )?;
        }
        if let Some(stats) = &self.queue_p2p_drops {
            writeln!(
                output,
                "- P2P queue drops max/median: {:.0} / {:.0}",
                stats.max, stats.median
            )?;
        }
        Ok(())
    }
    fn render_peer_metrics(&self, output: &mut String) -> Result<()> {
        if let Some(peer) = &self.peer_metrics {
            writeln!(
                output,
                "- per-peer payload bytes: {:.0} - {:.0}",
                peer.payload_bytes_min, peer.payload_bytes_max
            )?;
        }
        Ok(())
    }
    fn render_runs_table(&self, output: &mut String) -> Result<()> {
        writeln!(
            output,
            "\n| Run | Source | Block | Height | View | Signed DA available (ms) | Commit (ms) | Throughput (MiB/s) | DA<=Commit | Availability votes | Total chunks | Received | BG queue max | P2P drops |"
        )?;
        writeln!(
            output,
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        )?;
        for (index, run) in self.runs_detail.iter().enumerate() {
            let (queue_bg_display, queue_p2p_display) = run.queue.as_ref().map_or_else(
                || ("-".to_owned(), "-".to_owned()),
                |queue| {
                    (
                        format!("{:.0}", queue.bg_post_queue_depth_max),
                        format!("{:.0}", queue.p2p_queue_dropped_total_max),
                    )
                },
            );
            writeln!(
                output,
                "| {} | {} | {} | {} | {} | {:.0} | {:.0} | {:.2} | {} | {} | {} | {} | {} | {} |",
                index + 1,
                file_stem(&run.source),
                shorten_hash(&run.block_hash),
                run.height,
                run.view,
                run.da_available_ms,
                run.commit_ms,
                run.throughput_mib_s,
                if run.da_available_ms <= run.commit_ms {
                    "yes"
                } else {
                    "no"
                },
                run.availability_vote_count,
                run.total_chunks,
                run.received_chunks,
                queue_bg_display,
                queue_p2p_display,
            )?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
struct AggregatePeerMetrics {
    payload_bytes_min: f64,
    payload_bytes_max: f64,
    first: bool,
}
impl AggregatePeerMetrics {
    fn ingest(&mut self, metrics: &PeerMetricsSummary) {
        if self.first {
            self.payload_bytes_min = metrics.payload_bytes_min;
            self.payload_bytes_max = metrics.payload_bytes_max;
            self.first = false;
        } else {
            self.payload_bytes_min = self.payload_bytes_min.min(metrics.payload_bytes_min);
            self.payload_bytes_max = self.payload_bytes_max.max(metrics.payload_bytes_max);
        }
    }
}
impl Default for AggregatePeerMetrics {
    fn default() -> Self {
        Self {
            payload_bytes_min: f64::INFINITY,
            payload_bytes_max: f64::NEG_INFINITY,
            first: true,
        }
    }
}
#[derive(Debug, Clone)]
struct Stats {
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
}
impl Stats {
    fn from_values(values: &[f64]) -> Self {
        assert!(
            !values.is_empty(),
            "Stats::from_values requires non-empty input"
        );
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let min = *sorted.first().unwrap();
        let max = *sorted.last().unwrap();
        let sum: f64 = values.iter().sum();
        let count =
            u32::try_from(values.len()).expect("Stats::from_values input length exceeds u32::MAX");
        let mean = sum / f64::from(count);
        let median = if sorted.len() % 2 == 1 {
            sorted[sorted.len() / 2]
        } else {
            let upper = sorted.len() / 2;
            f64::midpoint(sorted[upper - 1], sorted[upper])
        };
        Self {
            min,
            max,
            mean,
            median,
        }
    }
}
#[derive(Debug)]
enum ReportError {
    Io(io::Error),
    Input(String),
    Fmt(std::fmt::Error),
    Json {
        path: PathBuf,
        message: String,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidType {
        path: PathBuf,
        field: String,
        expected: &'static str,
        actual: &'static str,
    },
    UnsupportedSchema {
        path: PathBuf,
        actual: String,
    },
    EmptyDataset(PathBuf),
    Inconsistent {
        scenario: String,
        detail: String,
    },
}
impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Input(msg) => write!(f, "{msg}"),
            Self::Fmt(err) => write!(f, "formatting error: {err}"),
            Self::Json { path, message } => {
                write!(f, "failed to parse JSON {}: {message}", path.display())
            }
            Self::MissingField { path, field } => {
                write!(f, "missing field `{field}` in {}", path.display())
            }
            Self::InvalidType {
                path,
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "field `{field}` in {} expected {expected} but found {actual}",
                    path.display()
                )
            }
            Self::UnsupportedSchema { path, actual } => write!(
                f,
                "summary {} declares unsupported schema `{actual}`; expected `{SIGNED_RS16_DA_SCHEMA}`",
                path.display()
            ),
            Self::EmptyDataset(root) => {
                write!(f, "no *.summary.json files found in {}", root.display())
            }
            Self::Inconsistent { scenario, detail } => {
                write!(f, "scenario `{scenario}` has inconsistent data: {detail}")
            }
        }
    }
}
impl std::error::Error for ReportError {}
impl From<io::Error> for ReportError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
impl From<std::fmt::Error> for ReportError {
    fn from(err: std::fmt::Error) -> Self {
        Self::Fmt(err)
    }
}
const USAGE: &str = "Usage: sumeragi_da_report [ARTIFACT_DIR]\n\n\
Generate a Markdown report from Sumeragi DA integration test summaries.\n\
Pass the directory containing *.summary.json artifacts as the first argument,\n\
or set SUMERAGI_DA_ARTIFACT_DIR. Use --help to display this message.\n";
fn require_object<'a>(map: &'a Map, key: &str, path: &Path) -> Result<&'a Map> {
    map.get(key).map_or_else(
        || {
            Err(ReportError::MissingField {
                path: path.to_path_buf(),
                field: key.into(),
            })
        },
        |value| match value {
            Value::Object(obj) => Ok(obj),
            other => Err(ReportError::InvalidType {
                path: path.to_path_buf(),
                field: key.into(),
                expected: "object",
                actual: value_type(other),
            }),
        },
    )
}
fn require_u64(map: &Map, key: &str, path: &Path) -> Result<u64> {
    map.get(key).map_or_else(
        || {
            Err(ReportError::MissingField {
                path: path.to_path_buf(),
                field: key.into(),
            })
        },
        |value| {
            value.as_u64().ok_or_else(|| ReportError::InvalidType {
                path: path.to_path_buf(),
                field: key.into(),
                expected: "u64",
                actual: value_type(value),
            })
        },
    )
}
fn require_f64(map: &Map, key: &str, path: &Path) -> Result<f64> {
    map.get(key).map_or_else(
        || {
            Err(ReportError::MissingField {
                path: path.to_path_buf(),
                field: key.into(),
            })
        },
        |value| {
            value.as_f64().ok_or_else(|| ReportError::InvalidType {
                path: path.to_path_buf(),
                field: key.into(),
                expected: "f64",
                actual: value_type(value),
            })
        },
    )
}
fn require_string(map: &Map, key: &str, path: &Path) -> Result<String> {
    map.get(key).map_or_else(
        || {
            Err(ReportError::MissingField {
                path: path.to_path_buf(),
                field: key.into(),
            })
        },
        |value| match value {
            Value::String(s) => Ok(s.clone()),
            other => Err(ReportError::InvalidType {
                path: path.to_path_buf(),
                field: key.into(),
                expected: "string",
                actual: value_type(other),
            }),
        },
    )
}
fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
fn u64_to_f64(value: u64) -> f64 {
    const TWO_POW_32: f64 = 4_294_967_296.0;
    let high = u32::try_from(value >> 32).expect("upper 32 bits fit in u32");
    let low = u32::try_from(value & 0xFFFF_FFFF).expect("lower 32 bits fit in u32");
    f64::from(high).mul_add(TWO_POW_32, f64::from(low))
}
fn format_u64_set(values: &BTreeSet<u64>) -> String {
    if values.is_empty() {
        return "n/a".into();
    }
    let mut iter = values.iter();
    let mut result = iter.next().unwrap().to_string();
    for value in iter {
        result.push_str(", ");
        result.push_str(&value.to_string());
    }
    result
}
fn file_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| path.display().to_string(), str::to_owned)
}
fn shorten_hash(hash: &str) -> String {
    if hash.len() <= 12 {
        return hash.to_owned();
    }
    format!("{}...", &hash[..12])
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    fn assert_close(actual: f64, expected: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-9,
            "expected {expected}, got {actual} (diff {diff})"
        );
    }
    #[test]
    fn stats_from_values_computes_expected() {
        let stats = Stats::from_values(&[10.0, 20.0, 30.0, 40.0]);
        assert_close(stats.min, 10.0);
        assert_close(stats.max, 40.0);
        assert_close(stats.mean, 25.0);
        assert_close(stats.median, 25.0);
    }
    #[test]
    fn scenario_sample_parses_queue_metrics() {
        let dir = test_directory("queue_metrics");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scenario.summary.json");
        let json = r#"{
            "schema": "signed_rs16_da_v1",
            "scenario": "queue_test",
            "peers": 4,
            "payload_bytes": 1024,
            "timings": {
                "da_available_ms": 900,
                "commit_ms": 1100,
                "throughput_mib_s": 3.7
            },
            "signed_da": {
                "height": 2,
                "view": 1,
                "total_chunks": 10,
                "received_chunks": 10,
                "availability_vote_count": 3,
                "block_hash": "abcdef"
            },
            "queue": {
                "bg_post_queue_depth_max": 3.0,
                "p2p_queue_dropped_total_max": 1.0
            }
        }"#;
        fs::write(&path, json).unwrap();
        let sample = ScenarioSample::from_path(path.clone()).unwrap();
        let queue = sample.queue.expect("queue metrics present");
        assert_close(queue.bg_post_queue_depth_max, 3.0);
        assert_close(queue.p2p_queue_dropped_total_max, 1.0);
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn scenario_summary_computes_queue_stats() {
        let base = ScenarioSample {
            scenario: "test".into(),
            peers: 4,
            payload_bytes: 1024,
            da_available_ms: 1_000.0,
            commit_ms: 1_200.0,
            throughput_mib_s: 3.5,
            total_chunks: 10,
            received_chunks: 10,
            availability_vote_count: 3,
            view: 1,
            height: 2,
            block_hash: "abc".into(),
            source: PathBuf::from("sample"),
            queue: Some(QueueSample {
                bg_post_queue_depth_max: 3.0,
                p2p_queue_dropped_total_max: 1.0,
            }),
            peer_metrics: None,
        };
        let mut second = base.clone();
        second.queue = Some(QueueSample {
            bg_post_queue_depth_max: 5.0,
            p2p_queue_dropped_total_max: 0.0,
        });
        second.da_available_ms = 900.0;
        second.commit_ms = 1_100.0;
        second.source = PathBuf::from("sample2");
        let summary = ScenarioSummary::from_runs("test", &[base, second]).unwrap();
        let queue_bg = summary.queue_bg_depth.expect("queue bg stats");
        assert_close(queue_bg.max, 5.0);
        assert_close(queue_bg.min, 3.0);
        let queue_p2p = summary.queue_p2p_drops.expect("queue drop stats");
        assert_close(queue_p2p.max, 1.0);
    }
    #[test]
    fn scenario_sample_parses_valid_summary() {
        let dir = test_directory("sample_parses");
        let path = dir.join("scenario.summary.json");
        fs::create_dir_all(&dir).unwrap();
        let json = r#"{
            "schema": "signed_rs16_da_v1",
            "scenario": "sumeragi_da_large_payload_four_peers",
            "peers": 4,
            "payload_bytes": 11010048,
            "timings": {
                "da_available_ms": 3200,
                "commit_ms": 3500,
                "da_available_seconds": 3.2,
                "commit_elapsed_seconds": 3.5,
                "throughput_mib_s": 3.1
            },
            "signed_da": {
                "height": 12,
                "view": 2,
                "total_chunks": 168,
                "received_chunks": 168,
                "availability_vote_count": 4,
                "block_hash": "abcd1234efgh5678"
            },
            "per_peer_metrics": [
                {"peer_index": 0, "payload_bytes": 11010048},
                {"peer_index": 1, "payload_bytes": 11010048},
                {"peer_index": 2, "payload_bytes": 11010048},
                {"peer_index": 3, "payload_bytes": 11010048}
            ]
        }"#;
        File::create(&path)
            .unwrap()
            .write_all(json.as_bytes())
            .unwrap();
        let sample = ScenarioSample::from_path(path.clone()).unwrap();
        assert_eq!(sample.scenario, "sumeragi_da_large_payload_four_peers");
        assert_eq!(sample.peers, 4);
        assert_eq!(sample.payload_bytes, 11_010_048);
        assert!(sample.peer_metrics.is_some());
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn scenario_sample_rejects_legacy_global_rbc_schema() {
        let dir = test_directory("legacy_schema");
        let path = dir.join("scenario.summary.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            r#"{
                "schema": "global_rbc_v1",
                "scenario": "retired",
                "peers": 4,
                "payload_bytes": 1024,
                "timings": {},
                "signed_da": {}
            }"#,
        )
        .unwrap();
        let error = ScenarioSample::from_path(path.clone())
            .expect_err("retired global-RBC schema must be rejected");
        assert!(matches!(error, ReportError::UnsupportedSchema { .. }));
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn generate_report_emits_markdown() {
        let dir = test_directory("render_report");
        fs::create_dir_all(&dir).unwrap();
        write_summary(
            &dir.join("scenario-one.summary.json"),
            "sumeragi_da_large_payload_four_peers",
            4,
            11_010_048,
            3_200,
            3_400,
            3.1,
        );
        write_summary(
            &dir.join("scenario-two.summary.json"),
            "sumeragi_da_large_payload_four_peers",
            4,
            11_010_048,
            3_000,
            3_200,
            3.3,
        );
        let report = generate_report(&dir).unwrap();
        assert!(report.contains("# Sumeragi Data-Availability Report"));
        assert!(report.contains("sumeragi_da_large_payload_four_peers"));
        assert!(report.contains("Throughput (MiB/s)"));
        assert!(report.contains("Signed DA available (ms)"));
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn run_with_args_requires_data() {
        let dir = test_directory("empty_dataset");
        fs::create_dir_all(&dir).unwrap();
        let mut sink = Vec::new();
        let args = vec![dir.display().to_string()];
        let err = run_with_args(&mut sink, &args).expect_err("empty dataset should error");
        assert!(matches!(err, ReportError::EmptyDataset(_)));
        let _ = fs::remove_dir_all(&dir);
    }
    fn test_directory(suffix: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "sumeragi_da_report_test_{}_{}",
            suffix,
            std::process::id()
        ));
        // Clean up any leftovers from previous runs.
        let _ = fs::remove_dir_all(&base);
        base
    }
    fn write_summary(
        path: &Path,
        scenario: &str,
        peers: u64,
        payload: u64,
        da_ms: u64,
        commit_ms: u64,
        throughput: f64,
    ) {
        let json = format!(
            "{{\n  \"schema\": \"signed_rs16_da_v1\",\n  \"scenario\": \"{scenario}\",\n  \"peers\": {peers},\n  \"payload_bytes\": {payload},\n  \"timings\": {{\n    \"da_available_ms\": {da_ms},\n    \"commit_ms\": {commit_ms},\n    \"da_available_seconds\": {da_s},\n    \"commit_elapsed_seconds\": {commit_s},\n    \"throughput_mib_s\": {throughput}\n  }},\n  \"signed_da\": {{\n    \"height\": 10,\n    \"view\": 1,\n    \"total_chunks\": 168,\n    \"received_chunks\": 168,\n    \"availability_vote_count\": 4,\n    \"block_hash\": \"deadbeefcafebabe{scenario}\"\n  }},\n  \"per_peer_metrics\": [\n    {{\"peer_index\": 0, \"payload_bytes\": {payload}}},\n    {{\"peer_index\": 1, \"payload_bytes\": {payload}}},\n    {{\"peer_index\": 2, \"payload_bytes\": {payload}}},\n    {{\"peer_index\": 3, \"payload_bytes\": {payload}}}\n  ]\n}}",
            scenario = scenario,
            peers = peers,
            payload = payload,
            da_ms = da_ms,
            commit_ms = commit_ms,
            throughput = throughput,
            da_s = u64_to_f64(da_ms) / 1000.0,
            commit_s = u64_to_f64(commit_ms) / 1000.0,
        );
        fs::write(path, json).unwrap();
    }
}
