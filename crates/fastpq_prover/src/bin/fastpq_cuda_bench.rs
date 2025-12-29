//! FASTPQ CUDA benchmark harness.
//!
//! Records CPU vs GPU timings for FFT and LDE with deterministic inputs so CUDA
//! captures can be wrapped alongside the Metal bundles already used in release
//! evidence.

#![allow(clippy::missing_panics_doc)]

use std::{
    collections::BTreeMap,
    env,
    fmt::Display,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use clap::Parser;
use fastpq_isi::find_by_name;
use fastpq_prover::{
    ExecutionMode, Planner, clear_execution_mode_observer, set_execution_mode_observer,
};
use norito::{
    derive::JsonSerialize,
    json::{self, Value},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

fn main() {
    if let Err(err) = run() {
        eprintln!("fastpq_cuda_bench: {err}");
        std::process::exit(1);
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "fastpq_cuda_bench")]
struct Config {
    /// Number of trace rows to benchmark (before padding to the next power of two).
    #[arg(long, default_value_t = 20_000)]
    rows: usize,
    /// Number of iterations per operation (warm-ups are excluded from the averages).
    #[arg(long, default_value_t = 5)]
    iterations: usize,
    /// Warm-up runs performed before timing begins.
    #[arg(long, default_value_t = 1)]
    warmups: usize,
    /// Number of columns to exercise.
    #[arg(long, default_value_t = 16)]
    column_count: usize,
    /// Path for the output JSON bundle.
    #[arg(long, default_value = "fastpq_cuda_bench.json")]
    output: PathBuf,
    /// Parameter set name (defaults to the canonical FASTPQ set).
    #[arg(long, default_value = "fastpq-lane-balanced")]
    parameter: String,
    /// Optional annotation describing the host or run.
    #[arg(long)]
    notes: Option<String>,
    /// Optional device label to carry into the metadata.
    #[arg(long)]
    device: Option<String>,
    /// Optional row-usage snapshot to embed in the metadata block.
    #[arg(long, value_name = "PATH")]
    row_usage: Option<PathBuf>,
    /// Fail if the GPU backend is unavailable.
    #[arg(long)]
    require_gpu: bool,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BenchMetadata {
    generated_at: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    platform: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    machine: Option<String>,
    command: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    device: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, Value>>,
    #[norito(skip_serializing_if = "Option::is_none")]
    row_usage_snapshot: Option<RowUsageSnapshot>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct RowUsageSnapshot {
    source: String,
    batches: Value,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BenchmarksBlock {
    rows: usize,
    padded_rows: usize,
    iterations: usize,
    warmups: usize,
    #[norito(skip_serializing_if = "Option::is_none")]
    column_count: Option<usize>,
    execution_mode: String,
    gpu_backend: String,
    gpu_available: bool,
    operations: Vec<OperationEntry>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct OperationEntry {
    operation: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    columns: Option<usize>,
    input_len: usize,
    cpu_mean_ms: f64,
    #[norito(skip_serializing_if = "Option::is_none")]
    gpu_mean_ms: Option<f64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    speedup_ratio: Option<f64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    speedup_delta_ms: Option<f64>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ReportBlock {
    rows: usize,
    padded_rows: usize,
    iterations: usize,
    warmups: usize,
    execution_mode: String,
    gpu_backend: String,
    gpu_available: bool,
    operations: Vec<ReportOperation>,
    metadata: ReportMetadata,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ReportMetadata {
    generated_at: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ReportOperation {
    operation: String,
    input_len: usize,
    cpu: ReportSummary,
    #[norito(skip_serializing_if = "Option::is_none")]
    gpu: Option<ReportSummary>,
    #[norito(skip_serializing_if = "Option::is_none")]
    speedup: Option<Speedup>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ReportSummary {
    mean_ms: f64,
}

#[derive(Debug, Clone, JsonSerialize)]
struct Speedup {
    ratio: f64,
    #[norito(skip_serializing_if = "Option::is_none")]
    delta_ms: Option<f64>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BenchOutput {
    metadata: BenchMetadata,
    benchmarks: BenchmarksBlock,
    report: ReportBlock,
}

#[derive(Debug, Clone)]
struct ExecutionProbe {
    resolved_mode: ExecutionMode,
    backend_label: String,
    gpu_available: bool,
}

#[derive(Debug, Clone)]
struct ColumnSets {
    time: Vec<Vec<u64>>,
    coeff: Vec<Vec<u64>>,
}

#[derive(Debug, Clone)]
struct OperationTimings {
    cpu: Summary,
    gpu: Option<Summary>,
}

#[derive(Debug, Clone)]
struct Summary {
    min: f64,
    max: f64,
    mean: f64,
}

impl Summary {
    fn from_samples(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        for sample in samples {
            min = min.min(*sample);
            max = max.max(*sample);
            sum += *sample;
        }
        #[allow(clippy::cast_precision_loss)]
        let mean = sum / samples.len() as f64;
        Some(Self {
            min: round3(min),
            max: round3(max),
            mean: round3(mean),
        })
    }

    fn mean_ms(&self) -> f64 {
        self.mean
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse();
    if config.rows == 0 {
        return Err("rows must be greater than zero".to_owned());
    }
    if config.iterations == 0 {
        return Err("iterations must be greater than zero".to_owned());
    }

    let params = find_by_name(&config.parameter)
        .ok_or_else(|| format!("parameter set '{}' not found", config.parameter))?;
    let planner = Planner::new(params);
    let padded = config
        .rows
        .checked_next_power_of_two()
        .ok_or_else(|| format!("rows {} exceed supported range", config.rows))?;
    let eval_len = padded
        .checked_shl(planner.blowup_log())
        .ok_or_else(|| "evaluation domain overflow".to_owned())?;

    let probe = resolve_execution_metadata(config.require_gpu)?;
    let columns = prepare_columns(&planner, padded, config.column_count);
    let operations = collect_operations(&planner, &config, padded, eval_len, &columns, &probe);
    let metadata = build_metadata(&config)?;
    let report = build_report(&config, &probe, &metadata, &operations);
    let output = BenchOutput {
        metadata,
        benchmarks: BenchmarksBlock {
            rows: config.rows,
            padded_rows: padded,
            iterations: config.iterations,
            warmups: config.warmups,
            column_count: Some(config.column_count),
            execution_mode: probe.resolved_mode.as_str().to_owned(),
            gpu_backend: probe.backend_label.clone(),
            gpu_available: probe.gpu_available,
            operations: operations
                .iter()
                .map(|entry| OperationEntry {
                    operation: entry.operation.clone(),
                    columns: entry.columns,
                    input_len: entry.input_len,
                    cpu_mean_ms: entry.cpu_mean_ms,
                    gpu_mean_ms: entry.gpu_mean_ms,
                    speedup_ratio: entry.speedup_ratio,
                    speedup_delta_ms: entry.speedup_delta_ms,
                })
                .collect(),
        },
        report,
    };

    let encoded = json::to_vec_pretty(&output).map_err(|err| format!("encode output: {err}"))?;
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create output directory {}: {err}", parent.display()))?;
    }
    fs::write(&config.output, encoded)
        .map_err(|err| format!("write {}: {err}", display_path(&config.output)))?;
    eprintln!("fastpq_cuda_bench: wrote {}", display_path(&config.output));
    Ok(())
}

fn build_report(
    config: &Config,
    probe: &ExecutionProbe,
    metadata: &BenchMetadata,
    operations: &[OperationEntry],
) -> ReportBlock {
    let report_ops = operations
        .iter()
        .map(|entry| ReportOperation {
            operation: entry.operation.clone(),
            input_len: entry.input_len,
            cpu: ReportSummary {
                mean_ms: entry.cpu_mean_ms,
            },
            gpu: entry
                .gpu_mean_ms
                .map(|mean| ReportSummary { mean_ms: mean }),
            speedup: entry.speedup_ratio.map(|ratio| Speedup {
                ratio,
                delta_ms: entry.speedup_delta_ms,
            }),
        })
        .collect();
    ReportBlock {
        rows: config.rows,
        padded_rows: padded_rows(config.rows),
        iterations: config.iterations,
        warmups: config.warmups,
        execution_mode: probe.resolved_mode.as_str().to_owned(),
        gpu_backend: probe.backend_label.clone(),
        gpu_available: probe.gpu_available,
        operations: report_ops,
        metadata: ReportMetadata {
            generated_at: metadata.generated_at.clone(),
            host: metadata.host.clone(),
        },
    }
}

fn build_metadata(config: &Config) -> Result<BenchMetadata, String> {
    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| format!("timestamp formatting failed: {err}"))?;
    let row_usage_snapshot = if let Some(path) = &config.row_usage {
        Some(load_row_usage(path)?)
    } else {
        None
    };
    let command = env::args().collect::<Vec<_>>().join(" ");
    Ok(BenchMetadata {
        generated_at,
        host: detect_hostname(),
        platform: detect_platform(),
        machine: Some(detect_machine()),
        command,
        device: config.device.clone(),
        notes: config.notes.clone(),
        labels: None,
        row_usage_snapshot,
    })
}

fn collect_operations(
    planner: &Planner,
    config: &Config,
    padded: usize,
    eval_len: usize,
    columns: &ColumnSets,
    probe: &ExecutionProbe,
) -> Vec<OperationEntry> {
    let mut entries = Vec::new();
    let fft = OperationTimings {
        cpu: measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
            planner.fft_columns(cols);
        }),
        gpu: probe.gpu_available.then(|| {
            measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
                planner.fft_gpu(cols);
            })
        }),
    };
    entries.push(operation_entry(
        "fft",
        padded,
        config.column_count,
        &fft.cpu,
        fft.gpu.as_ref(),
    ));

    let lde = OperationTimings {
        cpu: measure_map(
            &columns.coeff,
            config.warmups,
            config.iterations,
            |coeffs| planner.lde_columns(coeffs),
        ),
        gpu: probe.gpu_available.then(|| {
            measure_map(
                &columns.coeff,
                config.warmups,
                config.iterations,
                |coeffs| planner.lde_gpu(coeffs),
            )
        }),
    };
    entries.push(operation_entry(
        "lde",
        eval_len,
        config.column_count,
        &lde.cpu,
        lde.gpu.as_ref(),
    ));

    entries
}

fn operation_entry(
    operation: &str,
    input_len: usize,
    columns: usize,
    cpu: &Summary,
    gpu: Option<&Summary>,
) -> OperationEntry {
    let speedup_ratio = gpu.map(|summary| cpu.mean_ms() / summary.mean_ms());
    let speedup_delta_ms = gpu.map(|summary| cpu.mean_ms() - summary.mean_ms());
    OperationEntry {
        operation: operation.to_owned(),
        columns: Some(columns),
        input_len,
        cpu_mean_ms: cpu.mean_ms(),
        gpu_mean_ms: gpu.map(Summary::mean_ms),
        speedup_ratio: speedup_ratio.map(round3),
        speedup_delta_ms: speedup_delta_ms.map(round3),
    }
}

fn prepare_columns(planner: &Planner, padded: usize, column_count: usize) -> ColumnSets {
    let time_columns = generated_columns(padded, column_count, 0x51a2_d3f4);
    let mut coeff_columns = time_columns.clone();
    planner.ifft_columns(&mut coeff_columns);
    ColumnSets {
        time: time_columns,
        coeff: coeff_columns,
    }
}

fn generated_columns(len: usize, column_count: usize, seed: u64) -> Vec<Vec<u64>> {
    let mut columns = Vec::with_capacity(column_count);
    for column in 0..column_count {
        let mut data = Vec::with_capacity(len);
        let column_seed = seed.wrapping_add((column as u64).wrapping_mul(0x9e37_79b9));
        let rotate_value = (column % 31) + 1;
        let rotate = u32::try_from(rotate_value).expect("rotate amount fits into u32");
        for idx in 0..len {
            let raw = column_seed
                ^ ((idx as u64).rotate_left(rotate))
                ^ ((idx as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
            data.push(raw % GOLDILOCKS_MODULUS);
        }
        columns.push(data);
    }
    columns
}

fn measure_in_place(
    template: &[Vec<u64>],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&mut [Vec<u64>]),
) -> Summary {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..warmups {
        let mut data = template.to_vec();
        op(&mut data);
        black_box(data);
    }
    for _ in 0..iterations {
        let mut data = template.to_vec();
        let start = Instant::now();
        op(&mut data);
        let elapsed = elapsed_ms(start.elapsed());
        samples.push(elapsed);
        black_box(data);
    }
    Summary::from_samples(&samples).expect("at least one iteration recorded")
}

fn measure_map<T>(
    template: &[Vec<u64>],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&[Vec<u64>]) -> T,
) -> Summary {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..warmups {
        let data = template.to_vec();
        let result = op(&data);
        black_box(result);
    }
    for _ in 0..iterations {
        let data = template.to_vec();
        let start = Instant::now();
        let result = op(&data);
        let elapsed = elapsed_ms(start.elapsed());
        samples.push(elapsed);
        black_box(result);
    }
    Summary::from_samples(&samples).expect("at least one iteration recorded")
}

fn resolve_execution_metadata(require_gpu: bool) -> Result<ExecutionProbe, String> {
    let requested = ExecutionMode::Auto;
    let label = ArcString::default();
    set_execution_mode_observer({
        let label = label.clone();
        move |req, _resolved, backend| {
            if matches!(req, ExecutionMode::Auto)
                && let Some(kind) = backend
            {
                label.set(kind.as_str());
            }
        }
    });
    let resolved_mode = requested.resolve();
    clear_execution_mode_observer();
    let backend_label = label.into_option().unwrap_or_else(|| "none".to_owned());
    let gpu_available = matches!(resolved_mode, ExecutionMode::Gpu);
    if require_gpu && !gpu_available {
        return Err(format!(
            "GPU execution requested but unavailable (resolved mode={}, backend=\"{}\"). Set FASTPQ_GPU=gpu to force detection.",
            resolved_mode.as_str(),
            backend_label
        ));
    }
    Ok(ExecutionProbe {
        resolved_mode,
        backend_label,
        gpu_available,
    })
}

#[derive(Clone, Default)]
struct ArcString(std::sync::Arc<std::sync::Mutex<Option<String>>>);

impl ArcString {
    fn set(&self, value: &str) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(value.to_owned());
        }
    }

    fn into_option(self) -> Option<String> {
        self.0.lock().ok().and_then(|value| value.clone())
    }
}

fn load_row_usage(path: &Path) -> Result<RowUsageSnapshot, String> {
    let data = fs::read(path).map_err(|err| format!("read {}: {err}", display_path(path)))?;
    let value: Value =
        json::from_slice(&data).map_err(|err| format!("parse {}: {err}", display_path(path)))?;
    let batches = value
        .get("batches")
        .cloned()
        .unwrap_or_else(|| value.clone());
    let source = path
        .file_name()
        .and_then(|value| value.to_str().map(str::to_owned))
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    Ok(RowUsageSnapshot { source, batches })
}

fn detect_hostname() -> Option<String> {
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn detect_platform() -> Option<String> {
    Command::new("uname")
        .arg("-sr")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            Some(format!(
                "{}-{}",
                env::consts::OS.replace('-', "_"),
                env::consts::ARCH
            ))
        })
}

fn detect_machine() -> String {
    env::consts::ARCH.to_owned()
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn padded_rows(rows: usize) -> usize {
    rows.next_power_of_two()
}

fn round3(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

impl Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "min={:.3}ms max={:.3}ms mean={:.3}ms",
            self.min, self.max, self.mean
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_rounding_matches_expected() {
        let summary = Summary::from_samples(&[1.2346, 2.0, 3.9999]).expect("summary");
        assert!((summary.min - 1.235).abs() < f64::EPSILON);
        assert!((summary.max - 4.0).abs() < f64::EPSILON);
        assert!((summary.mean - 2.411).abs() < f64::EPSILON);
    }

    #[test]
    fn generated_columns_are_deterministic() {
        let first = generated_columns(8, 2, 0xdead_beef);
        let second = generated_columns(8, 2, 0xdead_beef);
        assert_eq!(first, second);
    }

    #[test]
    fn measure_helpers_execute_operations() {
        let template = vec![vec![1u64, 2, 3, 4]];
        let summary = measure_in_place(&template, 0, 1, |cols| {
            for column in cols {
                column.reverse();
            }
        });
        assert!(summary.mean >= 0.0);

        let map_summary = measure_map(&template, 0, 1, |cols| {
            cols.iter()
                .map(|col| col.iter().sum::<u64>())
                .collect::<Vec<_>>()
        });
        assert!(map_summary.mean >= 0.0);
    }

    #[test]
    fn load_row_usage_picks_batches_block_when_present() {
        let dir = env::temp_dir();
        let path = dir.join("fastpq_cuda_bench_row_usage.json");
        fs::write(&path, r#"{ "batches": [{ "id": 1 }] }"#).expect("write row usage");
        let snapshot = load_row_usage(&path).expect("load snapshot");
        assert_eq!(snapshot.source, "fastpq_cuda_bench_row_usage.json");
        assert_eq!(
            snapshot
                .batches
                .get(0)
                .and_then(Value::as_object)
                .and_then(|map| map.get("id"))
                .and_then(Value::as_i64),
            Some(1)
        );
        let _ = fs::remove_file(&path);
    }
}
