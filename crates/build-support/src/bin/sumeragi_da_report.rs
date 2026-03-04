#![deny(warnings)]
//! Summarise Sumeragi data-availability runs by ingesting JSON summaries emitted
//! by the large-payload integration helpers. The tool scans an artifact directory
//! (either passed as the first CLI argument or via `SUMERAGI_DA_ARTIFACT_DIR`),
//! groups runs per scenario, and renders a Markdown report containing aggregated
//! latency and throughput measurements alongside per-run details.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fmt::Write as _,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use norito::json::{self, Map, Value};

type Result<T> = std::result::Result<T, ReportError>;

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
        "| Scenario | Runs | Peers | Payload (MiB) | RBC deliver median (ms) | RBC deliver max (ms) | Commit median (ms) | Commit max (ms) | Throughput median (MiB/s) | Throughput min (MiB/s) | RBC<=Commit | BG queue max | P2P drops max |"
    )?;
    writeln!(
        output,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )?;

    for (scenario, runs) in &grouped {
        let summary = ScenarioSummary::from_runs(scenario, runs)?;
        writeln!(
            output,
            "| {scenario} | {runs} | {peers} | {payload_mib:.2} | {rbc_median:.0} | {rbc_max:.0} | {commit_median:.0} | {commit_max:.0} | {throughput_median:.2} | {throughput_min:.2} | {deliver_guard} | {queue_bg_max} | {queue_p2p_max} |",
            runs = summary.runs,
            peers = summary.peers,
            payload_mib = summary.payload_mib,
            rbc_median = summary.rbc_deliver.median,
            rbc_max = summary.rbc_deliver.max,
            commit_median = summary.commit.median,
            commit_max = summary.commit.max,
            throughput_median = summary.throughput.median,
            throughput_min = summary.throughput.min,
            deliver_guard = if summary.all_deliver_within_commit {
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
    rbc_deliver_ms: f64,
    commit_ms: f64,
    throughput_mib_s: f64,
    total_chunks: u64,
    received_chunks: u64,
    ready_count: u64,
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

        let scenario = require_string(root, "scenario", &path)?;
        let peers = require_u64(root, "peers", &path)?;
        let payload_bytes = require_u64(root, "payload_bytes", &path)?;

        let timings = require_object(root, "timings", &path)?;
        let rbc_deliver_ms = require_f64(timings, "rbc_deliver_ms", &path)?;
        let commit_ms = require_f64(timings, "commit_ms", &path)?;
        let throughput_mib_s = require_f64(timings, "throughput_mib_s", &path)?;

        let rbc = require_object(root, "rbc", &path)?;
        let total_chunks = require_u64(rbc, "total_chunks", &path)?;
        let received_chunks = require_u64(rbc, "received_chunks", &path)?;
        let ready_count = require_u64(rbc, "ready_count", &path)?;
        let view = require_u64(rbc, "view", &path)?;
        let height = require_u64(rbc, "height", &path)?;
        let block_hash = require_string(rbc, "block_hash", &path)?;

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
            rbc_deliver_ms,
            commit_ms,
            throughput_mib_s,
            total_chunks,
            received_chunks,
            ready_count,
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
    deliver_total_min: f64,
    deliver_total_max: f64,
    ready_total_min: f64,
    ready_total_max: f64,
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
        let mut deliver_total_min = f64::INFINITY;
        let mut deliver_total_max = f64::NEG_INFINITY;
        let mut ready_total_min = f64::INFINITY;
        let mut ready_total_max = f64::NEG_INFINITY;

        for value in list {
            let obj = value.as_object().ok_or_else(|| ReportError::InvalidType {
                path: path.to_path_buf(),
                field: "per_peer_metrics[]".into(),
                expected: "object",
                actual: value_type(value),
            })?;

            let payload = require_f64(obj, "payload_bytes", path)?;
            let deliver = require_f64(obj, "deliver_total", path)?;
            let ready = require_f64(obj, "ready_total", path)?;

            payload_bytes_min = payload_bytes_min.min(payload);
            payload_bytes_max = payload_bytes_max.max(payload);
            deliver_total_min = deliver_total_min.min(deliver);
            deliver_total_max = deliver_total_max.max(deliver);
            ready_total_min = ready_total_min.min(ready);
            ready_total_max = ready_total_max.max(ready);
        }

        Ok(Self {
            peers: list.len(),
            payload_bytes_min,
            payload_bytes_max,
            deliver_total_min,
            deliver_total_max,
            ready_total_min,
            ready_total_max,
        })
    }
}

#[derive(Debug, Clone)]
struct ScenarioSummary {
    runs: usize,
    peers: u64,
    payload_bytes: u64,
    payload_mib: f64,
    rbc_deliver: Stats,
    commit: Stats,
    throughput: Stats,
    all_deliver_within_commit: bool,
    total_chunks: BTreeSet<u64>,
    ready_counts: BTreeSet<u64>,
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

        let mut rbc_times = Vec::with_capacity(runs.len());
        let mut commit_times = Vec::with_capacity(runs.len());
        let mut throughputs = Vec::with_capacity(runs.len());
        let mut all_deliver_within_commit = true;
        let mut total_chunks = BTreeSet::new();
        let mut ready_counts = BTreeSet::new();
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

            rbc_times.push(run.rbc_deliver_ms);
            commit_times.push(run.commit_ms);
            throughputs.push(run.throughput_mib_s);
            all_deliver_within_commit &= run.rbc_deliver_ms <= run.commit_ms;
            total_chunks.insert(run.total_chunks);
            ready_counts.insert(run.ready_count);

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
            rbc_deliver: Stats::from_values(&rbc_times),
            commit: Stats::from_values(&commit_times),
            throughput: Stats::from_values(&throughputs),
            all_deliver_within_commit,
            total_chunks,
            ready_counts,
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
            "- RBC chunks observed: {}",
            format_u64_set(&self.total_chunks)
        )?;
        writeln!(
            output,
            "- READY vote counts: {}",
            format_u64_set(&self.ready_counts)
        )?;
        writeln!(
            output,
            "- RBC<=Commit observed: {}",
            if self.all_deliver_within_commit {
                "yes"
            } else {
                "no"
            }
        )?;
        writeln!(
            output,
            "- RBC delivery mean (ms): {:.2}",
            self.rbc_deliver.mean
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
            writeln!(
                output,
                "- per-peer deliver broadcasts: {:.0} - {:.0}",
                peer.deliver_total_min, peer.deliver_total_max
            )?;
            writeln!(
                output,
                "- per-peer ready broadcasts: {:.0} - {:.0}",
                peer.ready_total_min, peer.ready_total_max
            )?;
        }
        Ok(())
    }

    fn render_runs_table(&self, output: &mut String) -> Result<()> {
        writeln!(
            output,
            "\n| Run | Source | Block | Height | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Received | BG queue max | P2P drops |"
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
                run.rbc_deliver_ms,
                run.commit_ms,
                run.throughput_mib_s,
                if run.rbc_deliver_ms <= run.commit_ms {
                    "yes"
                } else {
                    "no"
                },
                run.ready_count,
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
    deliver_total_min: f64,
    deliver_total_max: f64,
    ready_total_min: f64,
    ready_total_max: f64,
    first: bool,
}

impl AggregatePeerMetrics {
    fn ingest(&mut self, metrics: &PeerMetricsSummary) {
        if self.first {
            self.payload_bytes_min = metrics.payload_bytes_min;
            self.payload_bytes_max = metrics.payload_bytes_max;
            self.deliver_total_min = metrics.deliver_total_min;
            self.deliver_total_max = metrics.deliver_total_max;
            self.ready_total_min = metrics.ready_total_min;
            self.ready_total_max = metrics.ready_total_max;
            self.first = false;
        } else {
            self.payload_bytes_min = self.payload_bytes_min.min(metrics.payload_bytes_min);
            self.payload_bytes_max = self.payload_bytes_max.max(metrics.payload_bytes_max);
            self.deliver_total_min = self.deliver_total_min.min(metrics.deliver_total_min);
            self.deliver_total_max = self.deliver_total_max.max(metrics.deliver_total_max);
            self.ready_total_min = self.ready_total_min.min(metrics.ready_total_min);
            self.ready_total_max = self.ready_total_max.max(metrics.ready_total_max);
        }
    }
}

impl Default for AggregatePeerMetrics {
    fn default() -> Self {
        Self {
            payload_bytes_min: f64::INFINITY,
            payload_bytes_max: f64::NEG_INFINITY,
            deliver_total_min: f64::INFINITY,
            deliver_total_max: f64::NEG_INFINITY,
            ready_total_min: f64::INFINITY,
            ready_total_max: f64::NEG_INFINITY,
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
    use std::fs::{self, File};

    use super::*;

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
            "scenario": "queue_test",
            "peers": 4,
            "payload_bytes": 1024,
            "timings": {
                "rbc_deliver_ms": 900,
                "commit_ms": 1100,
                "throughput_mib_s": 3.7
            },
            "rbc": {
                "height": 2,
                "view": 1,
                "total_chunks": 10,
                "received_chunks": 10,
                "ready_count": 3,
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
            rbc_deliver_ms: 1_000.0,
            commit_ms: 1_200.0,
            throughput_mib_s: 3.5,
            total_chunks: 10,
            received_chunks: 10,
            ready_count: 3,
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
        second.rbc_deliver_ms = 900.0;
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
            "scenario": "sumeragi_rbc_da_large_payload_four_peers",
            "peers": 4,
            "payload_bytes": 11010048,
            "timings": {
                "rbc_deliver_ms": 3200,
                "commit_ms": 3500,
                "rbc_delivered_seconds": 3.2,
                "commit_elapsed_seconds": 3.5,
                "throughput_mib_s": 3.1
            },
            "rbc": {
                "height": 12,
                "view": 2,
                "total_chunks": 168,
                "received_chunks": 168,
                "ready_count": 4,
                "block_hash": "abcd1234efgh5678"
            },
            "per_peer_metrics": [
                {"peer_index": 0, "payload_bytes": 11010048, "deliver_total": 1, "ready_total": 1},
                {"peer_index": 1, "payload_bytes": 11010048, "deliver_total": 1, "ready_total": 1},
                {"peer_index": 2, "payload_bytes": 11010048, "deliver_total": 1, "ready_total": 1},
                {"peer_index": 3, "payload_bytes": 11010048, "deliver_total": 1, "ready_total": 1}
            ]
        }"#;
        File::create(&path)
            .unwrap()
            .write_all(json.as_bytes())
            .unwrap();

        let sample = ScenarioSample::from_path(path.clone()).unwrap();
        assert_eq!(sample.scenario, "sumeragi_rbc_da_large_payload_four_peers");
        assert_eq!(sample.peers, 4);
        assert_eq!(sample.payload_bytes, 11_010_048);
        assert!(sample.peer_metrics.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_report_emits_markdown() {
        let dir = test_directory("render_report");
        fs::create_dir_all(&dir).unwrap();
        write_summary(
            &dir.join("scenario-one.summary.json"),
            "sumeragi_rbc_da_large_payload_four_peers",
            4,
            11_010_048,
            3_200,
            3_400,
            3.1,
        );
        write_summary(
            &dir.join("scenario-two.summary.json"),
            "sumeragi_rbc_da_large_payload_four_peers",
            4,
            11_010_048,
            3_000,
            3_200,
            3.3,
        );

        let report = generate_report(&dir).unwrap();
        assert!(report.contains("# Sumeragi Data-Availability Report"));
        assert!(report.contains("sumeragi_rbc_da_large_payload_four_peers"));
        assert!(report.contains("Throughput (MiB/s)"));
        assert!(report.contains("RBC deliver (ms)"));

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
        rbc_ms: u64,
        commit_ms: u64,
        throughput: f64,
    ) {
        let json = format!(
            "{{\n  \"scenario\": \"{scenario}\",\n  \"peers\": {peers},\n  \"payload_bytes\": {payload},\n  \"timings\": {{\n    \"rbc_deliver_ms\": {rbc_ms},\n    \"commit_ms\": {commit_ms},\n    \"rbc_delivered_seconds\": {rbc_s},\n    \"commit_elapsed_seconds\": {commit_s},\n    \"throughput_mib_s\": {throughput}\n  }},\n  \"rbc\": {{\n    \"height\": 10,\n    \"view\": 1,\n    \"total_chunks\": 168,\n    \"received_chunks\": 168,\n    \"ready_count\": 4,\n    \"block_hash\": \"deadbeefcafebabe{scenario}\"\n  }},\n  \"per_peer_metrics\": [\n    {{\"peer_index\": 0, \"payload_bytes\": {payload}, \"deliver_total\": 1, \"ready_total\": 1}},\n    {{\"peer_index\": 1, \"payload_bytes\": {payload}, \"deliver_total\": 1, \"ready_total\": 1}},\n    {{\"peer_index\": 2, \"payload_bytes\": {payload}, \"deliver_total\": 1, \"ready_total\": 1}},\n    {{\"peer_index\": 3, \"payload_bytes\": {payload}, \"deliver_total\": 1, \"ready_total\": 1}}\n  ]\n}}",
            scenario = scenario,
            peers = peers,
            payload = payload,
            rbc_ms = rbc_ms,
            commit_ms = commit_ms,
            throughput = throughput,
            rbc_s = u64_to_f64(rbc_ms) / 1000.0,
            commit_s = u64_to_f64(commit_ms) / 1000.0,
        );

        fs::write(path, json).unwrap();
    }
}
