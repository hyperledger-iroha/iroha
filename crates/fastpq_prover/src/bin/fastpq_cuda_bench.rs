//! FASTPQ CUDA benchmark harness.
//!
//! Records CPU vs GPU timings for FFT, IFFT, LDE, Poseidon column hashing,
//! and BN254 helper kernels with deterministic inputs so CUDA captures can be
//! wrapped alongside the Metal bundles already used in release evidence.

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
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::RATE as POSEIDON_RATE;
#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::trace::{
    PoseidonColumnBatch, hash_columns_cpu_batch_inputs, hash_columns_gpu_batch,
};
use fastpq_prover::{
    CudaBackendError, ExecutionMode, Planner, clear_execution_mode_observer, fastpq_bn254_fft,
    fastpq_bn254_lde, set_execution_mode_observer,
};
use halo2curves::{bn256::Fr as Bn254Fr, ff::PrimeField};
use iroha_zkp_halo2::{Bn254Scalar, IpaScalar};
use norito::{
    derive::JsonSerialize,
    json::{self, Value},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;
const BN254_LIMBS: usize = 4;
#[cfg(feature = "fastpq-gpu")]
const POSEIDON_COLUMN_DOMAIN_PREFIX: &str = "fastpq:v1:trace:column:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationFilter {
    All,
    Only(BenchOperation),
}

impl OperationFilter {
    fn includes(self, operation: BenchOperation) -> bool {
        matches!(self, Self::All) || matches!(self, Self::Only(value) if value == operation)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Only(operation) => operation.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchOperation {
    Fft,
    Ifft,
    Lde,
    Poseidon,
}

impl BenchOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fft => "fft",
            Self::Ifft => "ifft",
            Self::Lde => "lde",
            Self::Poseidon => "poseidon_hash_columns",
        }
    }
}

fn parse_operation_filter(raw: &str) -> Result<OperationFilter, String> {
    if raw.eq_ignore_ascii_case("all") {
        return Ok(OperationFilter::All);
    }
    match raw {
        "fft" => Ok(OperationFilter::Only(BenchOperation::Fft)),
        "ifft" => Ok(OperationFilter::Only(BenchOperation::Ifft)),
        "lde" => Ok(OperationFilter::Only(BenchOperation::Lde)),
        "poseidon_hash_columns" | "poseidon-hash" | "poseidon" => {
            Ok(OperationFilter::Only(BenchOperation::Poseidon))
        }
        _ => Err(format!("unknown --operation '{raw}'")),
    }
}

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
    /// Restrict the benchmark to a single operation (`fft`, `ifft`, `lde`, `poseidon_hash_columns`, or `all`).
    #[arg(long, default_value = "all", value_parser = parse_operation_filter)]
    operation: OperationFilter,
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
    operation_filter: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    bn254_metrics: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    bn254_warnings: Option<Vec<String>>,
    operations: Vec<OperationEntry>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct OperationEntry {
    operation: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    columns: Option<usize>,
    input_len: usize,
    output_len: usize,
    input_bytes: usize,
    output_bytes: usize,
    estimated_gpu_transfer_bytes: usize,
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
    operation_filter: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    bn254_metrics: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    bn254_warnings: Option<Vec<String>>,
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
    #[norito(skip_serializing_if = "Option::is_none")]
    columns: Option<usize>,
    input_len: usize,
    output_len: usize,
    input_bytes: usize,
    output_bytes: usize,
    estimated_gpu_transfer_bytes: usize,
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
struct Bn254ColumnSets {
    coeff_scalars: Vec<Vec<Bn254Scalar>>,
    coeff_limbs: Vec<u64>,
    coset_scalar: Bn254Scalar,
    coset_limbs: [u64; BN254_LIMBS],
}

#[derive(Debug, Clone)]
struct Bn254MetricEntry {
    operation: &'static str,
    cpu: Summary,
    gpu: Option<Summary>,
}

#[derive(Debug, Clone, Default)]
struct Bn254BenchCapture {
    metrics: Option<Value>,
    warnings: Vec<String>,
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
    let bn254_capture = collect_bn254_metrics(&config, padded, planner.blowup_log(), &probe)?;
    let operations = collect_operations(&planner, &config, padded, eval_len, &columns, &probe);
    let metadata = build_metadata(&config)?;
    let report = build_report(
        &config,
        &probe,
        &metadata,
        &operations,
        bn254_capture.metrics.clone(),
        (!bn254_capture.warnings.is_empty()).then_some(bn254_capture.warnings.clone()),
    );
    for warning in &bn254_capture.warnings {
        eprintln!("fastpq_cuda_bench: warning: {warning}");
    }
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
            operation_filter: config.operation.as_str().to_owned(),
            bn254_metrics: bn254_capture.metrics.clone(),
            bn254_warnings: (!bn254_capture.warnings.is_empty())
                .then_some(bn254_capture.warnings.clone()),
            operations: operations
                .iter()
                .map(|entry| OperationEntry {
                    operation: entry.operation.clone(),
                    columns: entry.columns,
                    input_len: entry.input_len,
                    output_len: entry.output_len,
                    input_bytes: entry.input_bytes,
                    output_bytes: entry.output_bytes,
                    estimated_gpu_transfer_bytes: entry.estimated_gpu_transfer_bytes,
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
    bn254_metrics: Option<Value>,
    bn254_warnings: Option<Vec<String>>,
) -> ReportBlock {
    let report_ops = operations
        .iter()
        .map(|entry| ReportOperation {
            operation: entry.operation.clone(),
            columns: entry.columns,
            input_len: entry.input_len,
            output_len: entry.output_len,
            input_bytes: entry.input_bytes,
            output_bytes: entry.output_bytes,
            estimated_gpu_transfer_bytes: entry.estimated_gpu_transfer_bytes,
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
        operation_filter: config.operation.as_str().to_owned(),
        bn254_metrics,
        bn254_warnings,
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
    if config.operation.includes(BenchOperation::Fft) {
        let fft = OperationTimings {
            cpu: measure_in_place(&columns.coeff, config.warmups, config.iterations, |cols| {
                planner.fft_columns(cols);
            }),
            gpu: probe.gpu_available.then(|| {
                measure_in_place(&columns.coeff, config.warmups, config.iterations, |cols| {
                    planner.fft_gpu(cols);
                })
            }),
        };
        entries.push(operation_entry(
            BenchOperation::Fft.as_str(),
            padded,
            padded,
            config.column_count,
            &fft.cpu,
            fft.gpu.as_ref(),
        ));
    }

    if config.operation.includes(BenchOperation::Ifft) {
        let ifft = OperationTimings {
            cpu: measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
                planner.ifft_columns(cols);
            }),
            gpu: probe.gpu_available.then(|| {
                measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
                    planner.ifft_gpu(cols);
                })
            }),
        };
        entries.push(operation_entry(
            BenchOperation::Ifft.as_str(),
            padded,
            padded,
            config.column_count,
            &ifft.cpu,
            ifft.gpu.as_ref(),
        ));
    }

    if config.operation.includes(BenchOperation::Lde) {
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
            BenchOperation::Lde.as_str(),
            padded,
            eval_len,
            config.column_count,
            &lde.cpu,
            lde.gpu.as_ref(),
        ));
    }

    #[cfg(feature = "fastpq-gpu")]
    if config.operation.includes(BenchOperation::Poseidon) {
        entries.push(collect_poseidon_entry(config, columns, probe));
    }

    entries
}

#[cfg(feature = "fastpq-gpu")]
fn collect_poseidon_entry(
    config: &Config,
    columns: &ColumnSets,
    probe: &ExecutionProbe,
) -> OperationEntry {
    let poseidon_domains = poseidon_domains(config.column_count);
    let poseidon_domain_refs: Vec<&str> = poseidon_domains.iter().map(String::as_str).collect();
    let poseidon_input_len = poseidon_input_len(columns.coeff.first().map_or(0, Vec::len));
    let poseidon = OperationTimings {
        cpu: measure_map(
            &columns.coeff,
            config.warmups,
            config.iterations,
            |coeffs| {
                hash_columns_cpu_batch_inputs(&poseidon_domain_refs, coeffs)
                    .expect("cpu poseidon batch shape")
            },
        ),
        gpu: if probe.gpu_available {
            measure_map_optional(
                &columns.coeff,
                config.warmups,
                config.iterations,
                |coeffs| {
                    let batch = PoseidonColumnBatch::from_domains_and_columns(
                        &poseidon_domain_refs,
                        coeffs,
                    )
                    .expect("gpu poseidon batch shape");
                    hash_columns_gpu_batch(&batch)
                },
            )
        } else {
            None
        },
    };
    operation_entry(
        BenchOperation::Poseidon.as_str(),
        poseidon_input_len,
        1,
        config.column_count,
        &poseidon.cpu,
        poseidon.gpu.as_ref(),
    )
}

fn collect_bn254_metrics(
    config: &Config,
    padded: usize,
    blowup_log: u32,
    probe: &ExecutionProbe,
) -> Result<Bn254BenchCapture, String> {
    let trace_log = padded.ilog2();
    if trace_log == 0 {
        return Ok(Bn254BenchCapture::default());
    }
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or_else(|| "BN254 evaluation domain overflow".to_owned())?;
    if eval_log > Bn254Fr::S {
        return Err(format!(
            "BN254 evaluation log {eval_log} exceeds supported two-adicity {}",
            Bn254Fr::S
        ));
    }
    let columns = prepare_bn254_columns(trace_log, config.column_count);
    let fft_twiddles = bn254_stage_twiddles(trace_log);
    let lde_twiddles = bn254_stage_twiddles(eval_log);
    let mut entries = Vec::new();
    let mut warnings = Vec::new();

    if config.operation.includes(BenchOperation::Fft) {
        entries.push(collect_bn254_fft_entry(
            config,
            trace_log,
            &columns,
            &fft_twiddles,
            probe,
            &mut warnings,
        ));
    }

    if config.operation.includes(BenchOperation::Lde) {
        entries.push(collect_bn254_lde_entry(
            config,
            trace_log,
            blowup_log,
            &columns,
            &lde_twiddles,
            probe,
            &mut warnings,
        )?);
    }

    Ok(Bn254BenchCapture {
        metrics: bn254_metrics_value(&entries, &probe.backend_label),
        warnings,
    })
}

fn collect_bn254_fft_entry(
    config: &Config,
    trace_log: u32,
    columns: &Bn254ColumnSets,
    fft_twiddles: &[Bn254Scalar],
    probe: &ExecutionProbe,
    warnings: &mut Vec<String>,
) -> Bn254MetricEntry {
    Bn254MetricEntry {
        operation: "fft",
        cpu: measure_in_place(
            &columns.coeff_scalars,
            config.warmups,
            config.iterations,
            |cols| bn254_cpu_fft(cols, trace_log, fft_twiddles),
        ),
        gpu: if probe.gpu_available {
            match measure_flat_in_place_result(
                &columns.coeff_limbs,
                config.warmups,
                config.iterations,
                |data| fastpq_bn254_fft(data, config.column_count, trace_log),
            ) {
                Ok(summary) => Some(summary),
                Err(err) => {
                    warnings.push(format!("bn254 fft gpu timing skipped: {err}"));
                    None
                }
            }
        } else {
            None
        },
    }
}

fn collect_bn254_lde_entry(
    config: &Config,
    trace_log: u32,
    blowup_log: u32,
    columns: &Bn254ColumnSets,
    lde_twiddles: &[Bn254Scalar],
    probe: &ExecutionProbe,
    warnings: &mut Vec<String>,
) -> Result<Bn254MetricEntry, String> {
    let eval_log = trace_log
        .checked_add(blowup_log)
        .expect("eval log validated before helper call");
    let eval_len = 1usize << eval_log;
    let output_len = config
        .column_count
        .checked_mul(eval_len)
        .and_then(|values| values.checked_mul(BN254_LIMBS))
        .ok_or_else(|| "BN254 output extent overflow".to_owned())?;
    Ok(Bn254MetricEntry {
        operation: "lde",
        cpu: measure_map(
            &columns.coeff_scalars,
            config.warmups,
            config.iterations,
            |coeffs| {
                bn254_cpu_lde(
                    coeffs,
                    trace_log,
                    blowup_log,
                    lde_twiddles,
                    columns.coset_scalar,
                )
            },
        ),
        gpu: if probe.gpu_available {
            match measure_flat_map_result(
                &columns.coeff_limbs,
                config.warmups,
                config.iterations,
                |coeffs| {
                    let mut out = vec![0u64; output_len];
                    fastpq_bn254_lde(
                        coeffs,
                        config.column_count,
                        trace_log,
                        blowup_log,
                        columns.coset_limbs,
                        &mut out,
                    )?;
                    Ok::<_, CudaBackendError>(out)
                },
            ) {
                Ok(summary) => Some(summary),
                Err(err) => {
                    warnings.push(format!("bn254 lde gpu timing skipped: {err}"));
                    None
                }
            }
        } else {
            None
        },
    })
}

fn operation_entry(
    operation: &str,
    input_len: usize,
    output_len: usize,
    columns: usize,
    cpu: &Summary,
    gpu: Option<&Summary>,
) -> OperationEntry {
    let input_bytes = bytes_for_columns(columns, input_len);
    let output_bytes = bytes_for_columns(columns, output_len);
    let speedup_ratio = gpu.map(|summary| cpu.mean_ms() / summary.mean_ms());
    let speedup_delta_ms = gpu.map(|summary| cpu.mean_ms() - summary.mean_ms());
    OperationEntry {
        operation: operation.to_owned(),
        columns: Some(columns),
        input_len,
        output_len,
        input_bytes,
        output_bytes,
        estimated_gpu_transfer_bytes: input_bytes.saturating_add(output_bytes),
        cpu_mean_ms: cpu.mean_ms(),
        gpu_mean_ms: gpu.map(Summary::mean_ms),
        speedup_ratio: speedup_ratio.map(round3),
        speedup_delta_ms: speedup_delta_ms.map(round3),
    }
}

fn bytes_for_columns(columns: usize, len: usize) -> usize {
    columns
        .checked_mul(len)
        .and_then(|elements| elements.checked_mul(core::mem::size_of::<u64>()))
        .expect("benchmark operation byte count fits usize")
}

fn bn254_metric_name(operation: &str) -> Option<&'static str> {
    match operation {
        "fft" => Some("acceleration.bn254_fft_ms"),
        "lde" => Some("acceleration.bn254_lde_ms"),
        _ => None,
    }
}

fn bn254_metrics_value(entries: &[Bn254MetricEntry], backend_label: &str) -> Option<Value> {
    let mut map = json::Map::new();
    for entry in entries {
        let Some(metric) = bn254_metric_name(entry.operation) else {
            continue;
        };
        let mut values = json::Map::new();
        values.insert(
            "cpu".to_owned(),
            json::to_value(&round3(entry.cpu.mean_ms())).expect("serialize BN254 cpu mean"),
        );
        if let Some(gpu) = entry.gpu.as_ref() {
            values.insert(
                backend_label.to_owned(),
                json::to_value(&round3(gpu.mean_ms())).expect("serialize BN254 gpu mean"),
            );
        }
        map.insert(metric.to_owned(), Value::Object(values));
    }
    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_domains(column_count: usize) -> Vec<String> {
    (0..column_count)
        .map(|index| format!("{POSEIDON_COLUMN_DOMAIN_PREFIX}bench{index}"))
        .collect()
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_input_len(column_len: usize) -> usize {
    let payload_len = column_len
        .checked_add(2)
        .expect("poseidon payload length fits usize");
    let remainder = payload_len % POSEIDON_RATE;
    if remainder == 0 {
        payload_len
    } else {
        payload_len + (POSEIDON_RATE - remainder)
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

fn prepare_bn254_columns(log_size: u32, column_count: usize) -> Bn254ColumnSets {
    let len = 1usize << log_size;
    let mut coeff_scalars = Vec::with_capacity(column_count);
    for column in 0..column_count {
        let mut data = Vec::with_capacity(len);
        for row in 0..len {
            let value = Bn254Scalar::from(((column as u64 + 1) * 31).wrapping_add(row as u64));
            data.push(value);
        }
        coeff_scalars.push(data);
    }
    let coeff_limbs = flatten_bn254_columns(&coeff_scalars);
    let coset_scalar = Bn254Scalar::from(5u64);
    let coset_limbs = bn254_scalar_to_limbs(&coset_scalar);
    Bn254ColumnSets {
        coeff_scalars,
        coeff_limbs,
        coset_scalar,
        coset_limbs,
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

fn bn254_scalar_to_limbs(value: &Bn254Scalar) -> [u64; BN254_LIMBS] {
    let bytes = (*value).to_bytes();
    let mut limbs = [0u64; BN254_LIMBS];
    for (index, limb) in limbs.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
        *limb = u64::from_le_bytes(buf);
    }
    limbs
}

fn flatten_bn254_columns(columns: &[Vec<Bn254Scalar>]) -> Vec<u64> {
    let scalar_len = columns.first().map_or(0, Vec::len);
    let mut flat = Vec::with_capacity(columns.len() * scalar_len * BN254_LIMBS);
    for column in columns {
        for value in column {
            flat.extend_from_slice(&bn254_scalar_to_limbs(value));
        }
    }
    flat
}

fn bn254_stage_twiddles(log_size: u32) -> Vec<Bn254Scalar> {
    let n = 1usize << log_size;
    let stage_span = n / 2;
    let mut twiddles = vec![Bn254Scalar::zero(); log_size as usize * stage_span];
    let exponent = 1u64 << (Bn254Fr::S - log_size);
    let omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY).pow_u64(exponent);
    for stage in 0..log_size {
        let len = 1usize << (stage + 1);
        let half = len / 2;
        let stride = n / len;
        let stage_offset = stage as usize * stage_span;
        let stride_twiddle = omega.pow_u64(stride as u64);
        let mut value = Bn254Scalar::one();
        for pair in 0..half {
            if pair > 0 {
                value = value.mul(stride_twiddle);
            }
            twiddles[stage_offset + pair] = value;
        }
        if half < stage_span {
            for index in half..stage_span {
                twiddles[stage_offset + index] = twiddles[stage_offset + index % half];
            }
        }
    }
    twiddles
}

fn bn254_cpu_fft(columns: &mut [Vec<Bn254Scalar>], log_size: u32, twiddles: &[Bn254Scalar]) {
    let n = 1usize << log_size;
    for column in columns {
        for stage in 0..log_size {
            let len = 1usize << (stage + 1);
            let half = len / 2;
            let stage_offset = stage as usize * (n / 2);
            for block in (0..n).step_by(len) {
                for pair in 0..half {
                    let index = block + pair;
                    let twiddle = twiddles[stage_offset + pair];
                    let u = column[index];
                    let v = column[index + half].mul(twiddle);
                    column[index] = u.add(v);
                    column[index + half] = u.sub(v);
                }
            }
        }
    }
}

fn bn254_cpu_lde(
    coeffs: &[Vec<Bn254Scalar>],
    trace_log: u32,
    blowup_log: u32,
    twiddles: &[Bn254Scalar],
    coset: Bn254Scalar,
) -> Vec<Vec<Bn254Scalar>> {
    let eval_log = trace_log + blowup_log;
    let trace_len = 1usize << trace_log;
    let eval_len = 1usize << eval_log;
    let mut outputs = Vec::with_capacity(coeffs.len());
    for column in coeffs {
        let mut data = vec![Bn254Scalar::zero(); eval_len];
        data[..trace_len].copy_from_slice(column);
        let mut coset_power = Bn254Scalar::one();
        for coeff in data.iter_mut().take(trace_len) {
            *coeff = (*coeff).mul(coset_power);
            coset_power = coset_power.mul(coset);
        }
        let mut column_fft = vec![data];
        bn254_cpu_fft(&mut column_fft, eval_log, twiddles);
        outputs.push(column_fft.pop().expect("single BN254 column present"));
    }
    outputs
}

fn measure_in_place<T: Clone>(
    template: &[Vec<T>],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&mut [Vec<T>]),
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

fn measure_map<T: Clone, R>(
    template: &[Vec<T>],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&[Vec<T>]) -> R,
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

#[cfg(any(feature = "fastpq-gpu", test))]
fn measure_map_optional<T: Clone, R>(
    template: &[Vec<T>],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&[Vec<T>]) -> Option<R>,
) -> Option<Summary> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..warmups {
        let data = template.to_vec();
        let result = op(&data)?;
        black_box(result);
    }
    for _ in 0..iterations {
        let data = template.to_vec();
        let start = Instant::now();
        let result = op(&data)?;
        let elapsed = elapsed_ms(start.elapsed());
        samples.push(elapsed);
        black_box(result);
    }
    Some(Summary::from_samples(&samples).expect("at least one iteration recorded"))
}

fn measure_flat_in_place_result<T: Clone, E>(
    template: &[T],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&mut [T]) -> Result<(), E>,
) -> Result<Summary, E> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..warmups {
        let mut data = template.to_vec();
        op(&mut data)?;
        black_box(data);
    }
    for _ in 0..iterations {
        let mut data = template.to_vec();
        let start = Instant::now();
        op(&mut data)?;
        let elapsed = elapsed_ms(start.elapsed());
        samples.push(elapsed);
        black_box(data);
    }
    Ok(Summary::from_samples(&samples).expect("at least one iteration recorded"))
}

fn measure_flat_map_result<T: Clone, R, E>(
    template: &[T],
    warmups: usize,
    iterations: usize,
    mut op: impl FnMut(&[T]) -> Result<R, E>,
) -> Result<Summary, E> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..warmups {
        let data = template.to_vec();
        let result = op(&data)?;
        black_box(result);
    }
    for _ in 0..iterations {
        let data = template.to_vec();
        let start = Instant::now();
        let result = op(&data)?;
        let elapsed = elapsed_ms(start.elapsed());
        samples.push(elapsed);
        black_box(result);
    }
    Ok(Summary::from_samples(&samples).expect("at least one iteration recorded"))
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

        let optional = measure_map_optional(&template, 0, 1, |_cols| None::<Vec<u64>>);
        assert!(optional.is_none());
    }

    #[test]
    fn operation_entry_records_shapes_and_transfer_bytes() {
        let cpu = Summary::from_samples(&[2.0]).expect("cpu summary");
        let gpu = Summary::from_samples(&[1.0]).expect("gpu summary");
        let entry = operation_entry("lde", 8, 16, 2, &cpu, Some(&gpu));
        assert_eq!(entry.input_len, 8);
        assert_eq!(entry.output_len, 16);
        assert_eq!(entry.input_bytes, 128);
        assert_eq!(entry.output_bytes, 256);
        assert_eq!(entry.estimated_gpu_transfer_bytes, 384);
        assert_eq!(entry.speedup_ratio, Some(2.0));
    }

    #[test]
    fn collect_operations_includes_ifft_and_explicit_lde_shape() {
        let params = find_by_name("fastpq-lane-balanced").expect("parameter set");
        let planner = Planner::new(params);
        let padded = 8usize;
        let eval_len = padded
            .checked_shl(planner.blowup_log())
            .expect("evaluation length");
        let columns = prepare_columns(&planner, padded, 2);
        let config = Config {
            rows: padded,
            iterations: 1,
            warmups: 0,
            column_count: 2,
            output: PathBuf::from("fastpq_cuda_bench.json"),
            parameter: "fastpq-lane-balanced".to_owned(),
            notes: None,
            device: None,
            row_usage: None,
            require_gpu: false,
            operation: OperationFilter::All,
        };
        let probe = ExecutionProbe {
            resolved_mode: ExecutionMode::Cpu,
            backend_label: "none".to_owned(),
            gpu_available: false,
        };
        let operations = collect_operations(&planner, &config, padded, eval_len, &columns, &probe);
        #[cfg(feature = "fastpq-gpu")]
        assert_eq!(operations.len(), 4);
        #[cfg(not(feature = "fastpq-gpu"))]
        assert_eq!(operations.len(), 3);
        assert_eq!(operations[0].operation, "fft");
        assert_eq!(operations[1].operation, "ifft");
        assert_eq!(operations[2].operation, "lde");
        assert_eq!(operations[2].input_len, padded);
        assert_eq!(operations[2].output_len, eval_len);
        #[cfg(feature = "fastpq-gpu")]
        {
            assert_eq!(operations[3].operation, "poseidon_hash_columns");
            assert_eq!(operations[3].output_len, 1);
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_domains_follow_trace_prefix() {
        let domains = poseidon_domains(3);
        assert_eq!(domains[0], "fastpq:v1:trace:column:bench0");
        assert_eq!(domains[2], "fastpq:v1:trace:column:bench2");
    }

    #[test]
    fn operation_filter_parser_accepts_poseidon_aliases() {
        assert_eq!(parse_operation_filter("all").unwrap(), OperationFilter::All);
        assert_eq!(
            parse_operation_filter("fft").unwrap(),
            OperationFilter::Only(BenchOperation::Fft)
        );
        assert_eq!(
            parse_operation_filter("poseidon").unwrap(),
            OperationFilter::Only(BenchOperation::Poseidon)
        );
        assert_eq!(
            parse_operation_filter("poseidon-hash").unwrap(),
            OperationFilter::Only(BenchOperation::Poseidon)
        );
        assert!(parse_operation_filter("bogus").is_err());
    }

    #[test]
    fn collect_operations_honors_single_operation_filter() {
        let params = find_by_name("fastpq-lane-balanced").expect("parameter set");
        let planner = Planner::new(params);
        let padded = 8usize;
        let eval_len = padded
            .checked_shl(planner.blowup_log())
            .expect("evaluation length");
        let columns = prepare_columns(&planner, padded, 2);
        let config = Config {
            rows: padded,
            iterations: 1,
            warmups: 0,
            column_count: 2,
            output: PathBuf::from("fastpq_cuda_bench.json"),
            parameter: "fastpq-lane-balanced".to_owned(),
            notes: None,
            device: None,
            row_usage: None,
            require_gpu: false,
            operation: OperationFilter::Only(BenchOperation::Lde),
        };
        let probe = ExecutionProbe {
            resolved_mode: ExecutionMode::Cpu,
            backend_label: "none".to_owned(),
            gpu_available: false,
        };
        let operations = collect_operations(&planner, &config, padded, eval_len, &columns, &probe);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].operation, "lde");
    }

    #[test]
    fn bn254_metrics_capture_cpu_and_gpu_latency() {
        let cpu_fft = Summary::from_samples(&[2.0]).expect("cpu fft");
        let gpu_fft = Summary::from_samples(&[1.0]).expect("gpu fft");
        let cpu_lde = Summary::from_samples(&[3.0]).expect("cpu lde");
        let entries = vec![
            Bn254MetricEntry {
                operation: "fft",
                cpu: cpu_fft,
                gpu: Some(gpu_fft),
            },
            Bn254MetricEntry {
                operation: "lde",
                cpu: cpu_lde,
                gpu: None,
            },
        ];
        let metrics = bn254_metrics_value(&entries, "cuda").expect("BN254 metrics");
        assert_eq!(
            metrics["acceleration.bn254_fft_ms"]["cpu"],
            norito::json!(2.0)
        );
        assert_eq!(
            metrics["acceleration.bn254_fft_ms"]["cuda"],
            norito::json!(1.0)
        );
        assert_eq!(
            metrics["acceleration.bn254_lde_ms"]["cpu"],
            norito::json!(3.0)
        );
        assert!(metrics["acceleration.bn254_lde_ms"].get("cuda").is_none());
    }

    #[test]
    fn collect_bn254_metrics_honors_operation_filter() {
        let config = Config {
            rows: 8,
            iterations: 1,
            warmups: 0,
            column_count: 2,
            output: PathBuf::from("fastpq_cuda_bench.json"),
            parameter: "fastpq-lane-balanced".to_owned(),
            notes: None,
            device: None,
            row_usage: None,
            require_gpu: false,
            operation: OperationFilter::Only(BenchOperation::Fft),
        };
        let probe = ExecutionProbe {
            resolved_mode: ExecutionMode::Cpu,
            backend_label: "none".to_owned(),
            gpu_available: false,
        };
        let capture = collect_bn254_metrics(&config, 8, 1, &probe).expect("collect BN254 metrics");
        assert!(capture.warnings.is_empty());
        let metrics = capture.metrics.expect("BN254 metrics present");
        assert!(metrics.get("acceleration.bn254_fft_ms").is_some());
        assert!(metrics.get("acceleration.bn254_lde_ms").is_none());
    }

    #[test]
    fn serialized_blocks_record_operation_filter() {
        let config = Config {
            rows: 8,
            iterations: 1,
            warmups: 0,
            column_count: 2,
            output: PathBuf::from("fastpq_cuda_bench.json"),
            parameter: "fastpq-lane-balanced".to_owned(),
            notes: None,
            device: None,
            row_usage: None,
            require_gpu: false,
            operation: OperationFilter::Only(BenchOperation::Lde),
        };
        let probe = ExecutionProbe {
            resolved_mode: ExecutionMode::Cpu,
            backend_label: "none".to_owned(),
            gpu_available: false,
        };
        let operations = vec![operation_entry(
            "lde",
            8,
            64,
            2,
            &Summary::from_samples(&[2.0]).expect("cpu summary"),
            None,
        )];
        let metadata = BenchMetadata {
            generated_at: "2026-03-27T00:00:00Z".to_owned(),
            host: Some("host".to_owned()),
            platform: Some("linux".to_owned()),
            machine: Some("x86_64".to_owned()),
            command: "fastpq_cuda_bench".to_owned(),
            device: None,
            notes: None,
            labels: None,
            row_usage_snapshot: None,
        };
        let bn254_metrics = Some(norito::json!({
            "acceleration.bn254_fft_ms": { "cpu": 2.0 }
        }));
        let bn254_warnings = Some(vec![
            "bn254 fft gpu timing skipped: cudaError_t(1)".to_owned(),
        ]);
        let report = build_report(
            &config,
            &probe,
            &metadata,
            &operations,
            bn254_metrics.clone(),
            bn254_warnings.clone(),
        );
        let report_value: Value =
            json::from_slice(&json::to_vec_pretty(&report).expect("serialize report"))
                .expect("parse report");
        assert_eq!(report_value["operation_filter"], norito::json!("lde"));
        assert_eq!(report_value["operations"][0]["columns"], norito::json!(2));
        assert_eq!(
            report_value["bn254_metrics"]["acceleration.bn254_fft_ms"]["cpu"],
            norito::json!(2.0)
        );
        assert_eq!(
            report_value["bn254_warnings"][0],
            norito::json!("bn254 fft gpu timing skipped: cudaError_t(1)")
        );

        let benchmarks = BenchmarksBlock {
            rows: config.rows,
            padded_rows: padded_rows(config.rows),
            iterations: config.iterations,
            warmups: config.warmups,
            column_count: Some(config.column_count),
            execution_mode: probe.resolved_mode.as_str().to_owned(),
            gpu_backend: probe.backend_label,
            gpu_available: probe.gpu_available,
            operation_filter: config.operation.as_str().to_owned(),
            bn254_metrics,
            bn254_warnings,
            operations,
        };
        let benchmarks_value: Value =
            json::from_slice(&json::to_vec_pretty(&benchmarks).expect("serialize benchmarks"))
                .expect("parse benchmarks");
        assert_eq!(benchmarks_value["operation_filter"], norito::json!("lde"));
        assert_eq!(
            benchmarks_value["bn254_metrics"]["acceleration.bn254_fft_ms"]["cpu"],
            norito::json!(2.0)
        );
        assert_eq!(
            benchmarks_value["bn254_warnings"][0],
            norito::json!("bn254 fft gpu timing skipped: cudaError_t(1)")
        );
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
