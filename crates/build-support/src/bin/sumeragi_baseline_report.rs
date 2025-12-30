#![deny(warnings)]
//! Summarise Sumeragi `NPoS` baseline runs by ingesting JSON summaries emitted by
//! the performance integration helper. The tool scans an artifact directory
//! (either passed as the first CLI argument or via `SUMERAGI_BASELINE_ARTIFACT_DIR`),
//! groups runs per scenario, and renders a Markdown report containing aggregated
//! throughput and latency measurements alongside per-run details.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write as _},
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use norito::json::{self, Map, Value, native::Number};

type Result<T> = std::result::Result<T, ReportError>;

fn main() -> ExitCode {
    match emit_report(io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("sumeragi-baseline-report: {err}");
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
            let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {
                ReportError::Input(
                    "provide an artifact directory argument or set SUMERAGI_BASELINE_ARTIFACT_DIR"
                        .into(),
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
    writeln!(output, "# Sumeragi NPoS Baseline Report")?;
    writeln!(
        output,
        "\nProcessed {} summary file(s) from `{}`.",
        grouped.values().map(Vec::len).sum::<usize>(),
        root.display()
    )?;

    writeln!(output, "\n## Summary\n")?;
    writeln!(
        output,
        "| Scenario | Runs | Peers | Block target (ms) | k | r | Blocks sampled (median) | Throughput (median blk/s) | Commit EMA (median ms) | Observed block (median ms) |"
    )?;
    writeln!(
        output,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )?;

    for (scenario, runs) in &grouped {
        let summary = ScenarioSummary::from_runs(scenario, runs)?;
        writeln!(
            output,
            "| {scenario} | {runs} | {peers} | {block_ms:.0} | {k} | {r} | {blocks_median:.0} | {throughput_median:.2} | {commit_median:.0} | {observed_block_median:.0} |",
            runs = summary.runs,
            peers = summary.peers,
            block_ms = summary.block_time_ms,
            k = summary.collectors_k,
            r = summary.redundant_send_r,
            blocks_median = summary.blocks_sampled.median,
            throughput_median = summary.throughput.median,
            commit_median = summary.commit.median,
            observed_block_median = summary.observed_block_time.median,
        )?;
    }

    for (scenario, runs) in grouped {
        let summary = ScenarioSummary::from_runs(&scenario, &runs)?;
        summary.render_detail(&mut output, &runs)?;
    }

    Ok(output)
}

#[derive(Debug, Clone)]
struct PhaseStats {
    _samples: u64,
    _min: f64,
    max: f64,
    _mean: f64,
    median: f64,
}

impl PhaseStats {
    fn from_object(map: &Map, path: &Path) -> Result<Self> {
        Ok(Self {
            _samples: require_u64(map, "samples", path)?,
            _min: require_f64(map, "min", path)?,
            max: require_f64(map, "max", path)?,
            _mean: require_f64(map, "mean", path)?,
            median: require_f64(map, "median", path)?,
        })
    }
}

#[derive(Debug, Clone)]
struct ScenarioSample {
    scenario: String,
    source: PathBuf,
    peers: u64,
    block_time_ms: f64,
    collectors_k: u64,
    redundant_send_r: u64,
    blocks_sampled: u64,
    elapsed_ms: f64,
    throughput_blocks_per_sec: f64,
    observed_block_time_ms: f64,
    phase_stats: BTreeMap<String, PhaseStats>,
    telemetry_samples: u64,
    final_height_total: u64,
    final_height_non_empty: u64,
    environment: BTreeMap<String, String>,
    queue_stats: BTreeMap<String, PhaseStats>,
    view_change_installs: Option<PhaseStats>,
    status_view_changes: Option<f64>,
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
        let network = require_object(root, "network", &path)?;
        let peers = require_u64(network, "peers", &path)?;
        let collectors_k = require_u64(network, "collectors_k", &path)?;
        let redundant_send_r = require_u64(network, "redundant_send_r", &path)?;

        let timing = require_object(root, "timing", &path)?;
        let block_time_ms = require_f64(timing, "block_time_target_ms", &path)?;
        let blocks_sampled = require_u64(timing, "blocks_sampled", &path)?;
        let elapsed_ms = require_f64(timing, "elapsed_ms", &path)?;
        let throughput_blocks_per_sec = require_f64(timing, "throughput_blocks_per_sec", &path)?;
        let observed_block_time_ms = require_f64(timing, "observed_block_time_ms", &path)?;

        let phase_root = require_object(root, "phase_latency_ema_ms", &path)?;
        let mut phase_stats = BTreeMap::new();
        for (phase, value) in phase_root {
            let obj = value.as_object().ok_or_else(|| ReportError::InvalidType {
                path: path.clone(),
                field: format!("phase_latency_ema_ms.{phase}"),
                expected: "object",
                actual: value_type(value),
            })?;
            phase_stats.insert(phase.clone(), PhaseStats::from_object(obj, &path)?);
        }

        let queue_root = root
            .get("queue")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut queue_stats = BTreeMap::new();
        for (key, value) in &queue_root {
            if let Some(obj) = value.as_object() {
                queue_stats.insert(key.clone(), PhaseStats::from_object(obj, &path)?);
            }
        }

        let view_root = root
            .get("view_changes")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let view_change_installs = view_root
            .get("install_total")
            .and_then(Value::as_object)
            .map(|obj| PhaseStats::from_object(obj, &path))
            .transpose()?;
        let status_view_changes = view_root
            .get("status_view_changes")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("value"))
            .and_then(Value::as_f64);

        let telemetry_samples = require_u64(root, "telemetry_samples", &path)?;

        let final_height = require_object(root, "final_height", &path)?;
        let final_height_total = require_u64(final_height, "total", &path)?;
        let final_height_non_empty = require_u64(final_height, "non_empty", &path)?;

        let environment = root
            .get("environment")
            .and_then(Value::as_object)
            .map(|env_map| {
                env_map
                    .iter()
                    .filter_map(|(key, value)| match value {
                        Value::String(text) => Some((key.clone(), text.clone())),
                        Value::Number(number) => Some((key.clone(), number_to_string(number))),
                        Value::Bool(flag) => Some((key.clone(), flag.to_string())),
                        _ => None,
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();

        Ok(Self {
            scenario,
            source: path,
            peers,
            block_time_ms,
            collectors_k,
            redundant_send_r,
            blocks_sampled,
            elapsed_ms,
            throughput_blocks_per_sec,
            observed_block_time_ms,
            phase_stats,
            telemetry_samples,
            final_height_total,
            final_height_non_empty,
            environment,
            queue_stats,
            view_change_installs,
            status_view_changes,
        })
    }
}

#[derive(Debug, Clone)]
struct StatsSummary {
    min: f64,
    max: f64,
    _mean: f64,
    median: f64,
}

impl StatsSummary {
    fn from_values(values: &[f64], label: &str) -> Result<Self> {
        if values.is_empty() {
            return Err(ReportError::Aggregate(format!(
                "no samples available for {label}"
            )));
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN values"));
        let len = sorted.len();
        let median = if len.is_multiple_of(2) {
            f64::midpoint(sorted[len / 2 - 1], sorted[len / 2])
        } else {
            sorted[len / 2]
        };
        let count = u32::try_from(len).map_err(|_| {
            ReportError::Aggregate(format!("too many samples to aggregate for {label}"))
        })?;
        let sum: f64 = values.iter().sum();
        Ok(Self {
            min: sorted.first().copied().unwrap_or(0.0),
            max: sorted.last().copied().unwrap_or(0.0),
            _mean: sum / f64::from(count),
            median,
        })
    }
}

struct ScenarioSummary {
    runs: usize,
    peers: u64,
    block_time_ms: f64,
    collectors_k: u64,
    redundant_send_r: u64,
    blocks_sampled: StatsSummary,
    throughput: StatsSummary,
    commit: StatsSummary,
    commit_max: f64,
    observed_block_time: StatsSummary,
}

impl ScenarioSummary {
    fn from_runs(scenario: &str, runs: &[ScenarioSample]) -> Result<Self> {
        if runs.is_empty() {
            return Err(ReportError::Aggregate(format!(
                "no runs for scenario {scenario}"
            )));
        }
        let mut peers_set = BTreeSet::new();
        let mut block_time_values = Vec::new();
        let mut collectors_set = BTreeSet::new();
        let mut redundant_set = BTreeSet::new();

        let mut blocks_sampled_vals = Vec::new();
        let mut throughput_vals = Vec::new();
        let mut commit_median_vals = Vec::new();
        let mut commit_max_vals = Vec::new();
        let mut observed_block_vals = Vec::new();

        for run in runs {
            peers_set.insert(run.peers);
            if block_time_values
                .iter()
                .all(|value| !approx_equal(*value, run.block_time_ms))
            {
                block_time_values.push(run.block_time_ms);
            }
            collectors_set.insert(run.collectors_k);
            redundant_set.insert(run.redundant_send_r);

            blocks_sampled_vals.push(u64_to_f64(run.blocks_sampled, "blocks_sampled")?);
            throughput_vals.push(run.throughput_blocks_per_sec);
            observed_block_vals.push(run.observed_block_time_ms);

            let commit_stats = run.phase_stats.get("commit").ok_or_else(|| {
                ReportError::Aggregate(format!(
                    "scenario `{scenario}` run `{}` missing commit stats",
                    run.source.display()
                ))
            })?;
            commit_median_vals.push(commit_stats.median);
            commit_max_vals.push(commit_stats.max);
        }

        if peers_set.len() != 1 {
            return Err(ReportError::Inconsistent {
                scenario: scenario.into(),
                detail: format!("inconsistent peer counts: {peers_set:?}"),
            });
        }
        if block_time_values.len() != 1 {
            return Err(ReportError::Inconsistent {
                scenario: scenario.into(),
                detail: format!("inconsistent block_time_ms values: {block_time_values:?}"),
            });
        }
        if collectors_set.len() != 1 {
            return Err(ReportError::Inconsistent {
                scenario: scenario.into(),
                detail: format!("inconsistent collectors_k values: {collectors_set:?}"),
            });
        }
        if redundant_set.len() != 1 {
            return Err(ReportError::Inconsistent {
                scenario: scenario.into(),
                detail: format!("inconsistent redundant_send_r values: {redundant_set:?}"),
            });
        }

        Ok(Self {
            runs: runs.len(),
            peers: *peers_set.iter().next().unwrap(),
            block_time_ms: block_time_values[0],
            collectors_k: *collectors_set.iter().next().unwrap(),
            redundant_send_r: *redundant_set.iter().next().unwrap(),
            blocks_sampled: StatsSummary::from_values(&blocks_sampled_vals, "blocks_sampled")?,
            throughput: StatsSummary::from_values(&throughput_vals, "throughput")?,
            commit: StatsSummary::from_values(&commit_median_vals, "commit")?,
            commit_max: commit_max_vals.into_iter().fold(f64::MIN, f64::max),
            observed_block_time: StatsSummary::from_values(
                &observed_block_vals,
                "observed_block_time_ms",
            )?,
        })
    }

    fn render_detail(&self, writer: &mut impl fmt::Write, runs: &[ScenarioSample]) -> Result<()> {
        let Some(first_run) = runs.first() else {
            return Err(ReportError::Aggregate(
                "cannot render scenario detail without runs".into(),
            ));
        };
        self.render_overview_section(writer, first_run)?;
        Self::render_runs_table(writer, runs)?;
        Self::render_environment_section(writer, first_run)?;
        Self::render_queue_metrics(writer, first_run)?;
        Self::render_view_change_section(writer, first_run)?;
        Ok(())
    }

    fn render_overview_section(
        &self,
        writer: &mut impl fmt::Write,
        first_run: &ScenarioSample,
    ) -> Result<()> {
        writeln!(writer, "\n### {}\n", first_run.scenario)?;
        writeln!(writer, "- runs: {}", self.runs)?;
        writeln!(writer, "- peers: {}", self.peers)?;
        writeln!(writer, "- block time target: {:.0} ms", self.block_time_ms)?;
        writeln!(
            writer,
            "- collectors_k / redundant_send_r: {} / {}",
            self.collectors_k, self.redundant_send_r
        )?;
        writeln!(
            writer,
            "- throughput (median): {:.2} blocks/s (min {:.2}, max {:.2})",
            self.throughput.median, self.throughput.min, self.throughput.max
        )?;
        writeln!(
            writer,
            "- commit EMA (median): {:.0} ms (max {:.0} ms)",
            self.commit.median, self.commit_max
        )?;
        writeln!(
            writer,
            "- observed block time (median): {:.0} ms",
            self.observed_block_time.median
        )?;
        Ok(())
    }

    fn render_runs_table(writer: &mut impl fmt::Write, runs: &[ScenarioSample]) -> Result<()> {
        writeln!(
            writer,
            "\n| Run | Source | Blocks sampled | Elapsed (s) | Throughput (blk/s) | Observed block (ms) | Commit median (ms) | Commit max (ms) | Telemetry samples | Final height | View changes |"
        )?;
        writeln!(
            writer,
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        )?;

        for (idx, run) in runs.iter().enumerate() {
            let commit_stats = run.phase_stats.get("commit").ok_or_else(|| {
                ReportError::Aggregate(format!("missing commit stats for {}", run.source.display()))
            })?;
            let elapsed_s = run.elapsed_ms / 1_000.0;
            let view_changes = run
                .status_view_changes
                .map_or_else(|| "n/a".to_string(), |v| format!("{v:.0}"));
            writeln!(
                writer,
                "| {} | {} | {} | {:.2} | {:.2} | {:.0} | {:.0} | {:.0} | {} | {}/{} | {} |",
                idx + 1,
                run.source
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("<unknown>"),
                run.blocks_sampled,
                elapsed_s,
                run.throughput_blocks_per_sec,
                run.observed_block_time_ms,
                commit_stats.median,
                commit_stats.max,
                run.telemetry_samples,
                run.final_height_non_empty,
                run.final_height_total,
                view_changes,
            )?;
        }
        Ok(())
    }

    fn render_environment_section(
        writer: &mut impl fmt::Write,
        first_run: &ScenarioSample,
    ) -> Result<()> {
        if first_run.environment.is_empty() {
            return Ok(());
        }
        let env = &first_run.environment;
        let os_label = env.get("os").map_or("n/a", String::as_str);
        let arch_label = env.get("arch").map_or("n/a", String::as_str);
        let family_label = env.get("family").map_or("n/a", String::as_str);
        writeln!(
            writer,
            "\nEnvironment: OS {os_label}, arch {arch_label}, family {family_label}."
        )?;
        if let Some(model) = env.get("hardware_model") {
            writeln!(writer, "- Hardware model: {model}")?;
        }
        if let Some(cpu) = env
            .get("cpu_topology")
            .or_else(|| env.get("cpu_cores_total"))
            .or_else(|| env.get("logical_cpus"))
        {
            writeln!(writer, "- CPU topology: {cpu}")?;
        }
        if let Some(memory) = env.get("memory").or_else(|| env.get("memory_gb")) {
            writeln!(writer, "- Memory: {memory}")?;
        }
        if let Some(hostname) = env.get("hostname") {
            writeln!(writer, "- Hostname: {hostname}")?;
        }
        if let Some(version) = env.get("system_version") {
            writeln!(writer, "- System version: {version}")?;
        }
        Ok(())
    }

    fn render_queue_metrics(
        writer: &mut impl fmt::Write,
        first_run: &ScenarioSample,
    ) -> Result<()> {
        if first_run.queue_stats.is_empty() {
            return Ok(());
        }
        writeln!(writer, "\nQueue metrics:")?;
        for (key, stats) in &first_run.queue_stats {
            writeln!(
                writer,
                "- {key}: median {:.2}, max {:.2}",
                stats.median, stats.max
            )?;
        }
        Ok(())
    }

    fn render_view_change_section(
        writer: &mut impl fmt::Write,
        first_run: &ScenarioSample,
    ) -> Result<()> {
        if let Some(installs) = &first_run.view_change_installs {
            writeln!(
                writer,
                "\nView-change installs: median {:.0}, max {:.0}",
                installs.median, installs.max
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum ReportError {
    Io(std::io::Error),
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
    Aggregate(String),
    Input(String),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Fmt(err) => write!(f, "formatting error: {err}"),
            Self::Json { path, message } => write!(
                f,
                "failed to parse JSON summary {}: {message}",
                path.display()
            ),
            Self::MissingField { path, field } => {
                write!(f, "summary {} missing field `{field}`", path.display())
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
            Self::Aggregate(detail) => write!(f, "{detail}"),
            Self::Input(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ReportError {}

impl From<std::io::Error> for ReportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<std::fmt::Error> for ReportError {
    fn from(err: std::fmt::Error) -> Self {
        Self::Fmt(err)
    }
}

const USAGE: &str = "Usage: sumeragi_baseline_report [ARTIFACT_DIR]\n\n\
Generate a Markdown report from Sumeragi NPoS baseline summaries.\n\
Pass the directory containing *.summary.json artifacts as the first argument,\n\
or set SUMERAGI_BASELINE_ARTIFACT_DIR. Use --help to display this message.\n";

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

fn approx_equal(lhs: f64, rhs: f64) -> bool {
    const TOLERANCE: f64 = 1.0e-9;
    (lhs - rhs).abs() <= TOLERANCE
}

fn number_to_string(number: &Number) -> String {
    match number {
        Number::I64(value) => value.to_string(),
        Number::U64(value) => value.to_string(),
        Number::F64(value) => {
            let mut rendered = format!("{value}");
            if rendered.contains('.') {
                while rendered.ends_with('0') {
                    rendered.pop();
                }
                if rendered.ends_with('.') {
                    rendered.pop();
                }
            }
            rendered
        }
    }
}

fn u64_to_f64(value: u64, label: &str) -> Result<f64> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_992; // 2^53
    if value > MAX_SAFE_INTEGER {
        return Err(ReportError::Aggregate(format!(
            "{label} value {value} exceeds f64 precision limits"
        )));
    }
    #[allow(clippy::cast_precision_loss)]
    Ok(value as f64)
}
