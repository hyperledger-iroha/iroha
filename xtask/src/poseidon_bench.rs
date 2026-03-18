use std::{
    error::Error,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use norito::json;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct PoseidonBenchOptions {
    pub batch_size: usize,
    pub iterations: u32,
    pub json_out: PathBuf,
    pub markdown_out: Option<PathBuf>,
    pub allow_overwrite: bool,
}

pub struct PoseidonBenchReport {
    pub timestamp: String,
    pub environment: PoseidonBenchEnvironment,
    pub batch_size: usize,
    pub iterations: u32,
    pub widths: Vec<PoseidonWidthReport>,
}

pub struct PoseidonBenchEnvironment {
    pub target: String,
    pub arch: String,
    pub os: String,
    pub cuda_feature: bool,
    pub cuda_available: bool,
    pub cuda_disabled: bool,
    pub cuda_last_error: Option<String>,
    pub metal_feature: bool,
    pub metal_available: bool,
    pub metal_disabled: bool,
    pub metal_last_error: Option<String>,
}

pub struct PoseidonWidthReport {
    pub width: u8,
    pub scalar: PerfSample,
    pub cuda: CudaSample,
}

pub struct PerfSample {
    pub total_ops: usize,
    pub elapsed_ms: f64,
    pub ops_per_sec: f64,
}

pub struct CudaSample {
    pub available: bool,
    pub disabled: bool,
    pub total_ops: usize,
    pub elapsed_ms: Option<f64>,
    pub ops_per_sec: Option<f64>,
    pub speedup_vs_scalar: Option<f64>,
    pub parity: ParityOutcome,
    pub note: Option<String>,
}

pub struct ParityOutcome {
    pub checked: bool,
    pub ok: bool,
    pub mismatch_index: Option<usize>,
    pub expected: Option<u64>,
    pub actual: Option<u64>,
}

pub fn run_poseidon_bench(
    options: PoseidonBenchOptions,
) -> Result<PoseidonBenchReport, Box<dyn Error>> {
    if options.batch_size == 0 {
        return Err("poseidon-cuda-bench requires --batch-size > 0".into());
    }
    if options.iterations == 0 {
        return Err("poseidon-cuda-bench requires --iterations > 0".into());
    }

    let runtime_errors = ivm::acceleration_runtime_errors();
    let (metal_feature, metal_available, metal_disabled) = metal_runtime_status();
    let env = PoseidonBenchEnvironment {
        target: std::env::var("TARGET")
            .unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),
        arch: std::env::var("CARGO_CFG_TARGET_ARCH")
            .unwrap_or_else(|_| std::env::consts::ARCH.to_string()),
        os: std::env::var("CARGO_CFG_TARGET_OS")
            .unwrap_or_else(|_| std::env::consts::OS.to_string()),
        cuda_feature: cfg!(feature = "cuda"),
        cuda_available: ivm::cuda_available(),
        cuda_disabled: ivm::cuda_disabled(),
        cuda_last_error: runtime_errors.cuda,
        metal_feature,
        metal_available,
        metal_disabled,
        metal_last_error: runtime_errors.metal,
    };

    let timestamp =
        OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;

    let inputs2 = poseidon2_inputs(options.batch_size);
    let inputs6 = poseidon6_inputs(options.batch_size);

    let expected2: Vec<u64> = inputs2
        .iter()
        .map(|&(a, b)| ivm::poseidon2_simd(a, b))
        .collect();
    let expected6: Vec<u64> = inputs6
        .iter()
        .map(|&inputs| ivm::poseidon6_simd(inputs))
        .collect();

    let scalar2 = measure_scalar_poseidon2(&inputs2, options.iterations);
    let scalar6 = measure_scalar_poseidon6(&inputs6, options.iterations);

    let cuda2 = measure_cuda_poseidon2(&inputs2, &expected2, &scalar2, options.iterations);
    let cuda6 = measure_cuda_poseidon6(&inputs6, &expected6, &scalar6, options.iterations);

    let report = PoseidonBenchReport {
        timestamp,
        environment: env,
        batch_size: options.batch_size,
        iterations: options.iterations,
        widths: vec![
            PoseidonWidthReport {
                width: 2,
                scalar: scalar2,
                cuda: cuda2,
            },
            PoseidonWidthReport {
                width: 6,
                scalar: scalar6,
                cuda: cuda6,
            },
        ],
    };

    write_json(&report, &options.json_out, options.allow_overwrite)?;
    if let Some(path) = options.markdown_out.as_ref() {
        write_markdown(&report, path, options.allow_overwrite)?;
    }

    Ok(report)
}

fn poseidon_report_to_json(report: &PoseidonBenchReport) -> json::Value {
    let mut root = json::Map::new();
    root.insert(
        "timestamp".into(),
        json::Value::from(report.timestamp.clone()),
    );
    root.insert(
        "batch_size".into(),
        json::Value::from(report.batch_size as u64),
    );
    root.insert(
        "iterations".into(),
        json::Value::from(report.iterations as u64),
    );

    let mut env = json::Map::new();
    env.insert(
        "target".into(),
        json::Value::from(report.environment.target.clone()),
    );
    env.insert(
        "arch".into(),
        json::Value::from(report.environment.arch.clone()),
    );
    env.insert(
        "os".into(),
        json::Value::from(report.environment.os.clone()),
    );
    env.insert(
        "cuda_feature".into(),
        json::Value::from(report.environment.cuda_feature),
    );
    env.insert(
        "cuda_available".into(),
        json::Value::from(report.environment.cuda_available),
    );
    env.insert(
        "cuda_disabled".into(),
        json::Value::from(report.environment.cuda_disabled),
    );
    env.insert(
        "cuda_last_error".into(),
        report
            .environment
            .cuda_last_error
            .as_ref()
            .map(|s| json::Value::from(s.clone()))
            .unwrap_or(json::Value::Null),
    );
    env.insert(
        "metal_feature".into(),
        json::Value::from(report.environment.metal_feature),
    );
    env.insert(
        "metal_available".into(),
        json::Value::from(report.environment.metal_available),
    );
    env.insert(
        "metal_disabled".into(),
        json::Value::from(report.environment.metal_disabled),
    );
    env.insert(
        "metal_last_error".into(),
        report
            .environment
            .metal_last_error
            .as_ref()
            .map(|s| json::Value::from(s.clone()))
            .unwrap_or(json::Value::Null),
    );
    root.insert("environment".into(), json::Value::Object(env));

    let widths: Vec<json::Value> = report
        .widths
        .iter()
        .map(|w| {
            let mut scalar = json::Map::new();
            scalar.insert(
                "total_ops".into(),
                json::Value::from(w.scalar.total_ops as u64),
            );
            scalar.insert("elapsed_ms".into(), json::Value::from(w.scalar.elapsed_ms));
            scalar.insert(
                "ops_per_sec".into(),
                json::Value::from(w.scalar.ops_per_sec),
            );

            let parity = &w.cuda.parity;
            let mut parity_map = json::Map::new();
            parity_map.insert("checked".into(), json::Value::from(parity.checked));
            parity_map.insert("ok".into(), json::Value::from(parity.ok));
            parity_map.insert(
                "mismatch_index".into(),
                parity
                    .mismatch_index
                    .map(|v| json::Value::from(v as u64))
                    .unwrap_or(json::Value::Null),
            );
            parity_map.insert(
                "expected".into(),
                parity
                    .expected
                    .map(json::Value::from)
                    .unwrap_or(json::Value::Null),
            );
            parity_map.insert(
                "actual".into(),
                parity
                    .actual
                    .map(json::Value::from)
                    .unwrap_or(json::Value::Null),
            );

            let mut cuda = json::Map::new();
            cuda.insert("available".into(), json::Value::from(w.cuda.available));
            cuda.insert("disabled".into(), json::Value::from(w.cuda.disabled));
            cuda.insert(
                "total_ops".into(),
                json::Value::from(w.cuda.total_ops as u64),
            );
            cuda.insert(
                "elapsed_ms".into(),
                w.cuda
                    .elapsed_ms
                    .map(json::Value::from)
                    .unwrap_or(json::Value::Null),
            );
            cuda.insert(
                "ops_per_sec".into(),
                w.cuda
                    .ops_per_sec
                    .map(json::Value::from)
                    .unwrap_or(json::Value::Null),
            );
            cuda.insert(
                "speedup_vs_scalar".into(),
                w.cuda
                    .speedup_vs_scalar
                    .map(json::Value::from)
                    .unwrap_or(json::Value::Null),
            );
            cuda.insert("parity".into(), json::Value::Object(parity_map));
            cuda.insert(
                "note".into(),
                w.cuda
                    .note
                    .as_ref()
                    .map(|s| json::Value::from(s.clone()))
                    .unwrap_or(json::Value::Null),
            );

            let mut width_map = json::Map::new();
            width_map.insert("width".into(), json::Value::from(w.width as u64));
            width_map.insert("scalar".into(), json::Value::Object(scalar));
            width_map.insert("cuda".into(), json::Value::Object(cuda));
            json::Value::Object(width_map)
        })
        .collect();
    root.insert("widths".into(), json::Value::Array(widths));

    json::Value::Object(root)
}
fn write_json(
    report: &PoseidonBenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !allow_overwrite {
        return Err(format!("refusing to overwrite existing file {}", path.display()).into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let value = poseidon_report_to_json(report);
    let data = json::to_vec_pretty(&value).map_err(|err| -> Box<dyn Error> { Box::new(err) })?;
    fs::write(path, data)?;
    Ok(())
}

fn write_markdown(
    report: &PoseidonBenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !allow_overwrite {
        return Err(format!("refusing to overwrite existing file {}", path.display()).into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut out = String::new();
    out.push_str("# Poseidon CUDA bench\n\n");
    out.push_str(&format!(
        "- timestamp: {}\n- batch size: {}\n- iterations: {}\n",
        report.timestamp, report.batch_size, report.iterations
    ));
    out.push_str(&format!(
        "- arch/os: {}/{}\n- target: {}\n- cuda feature: {}\n- cuda available: {}\n- cuda disabled: {}\n",
        report.environment.arch,
        report.environment.os,
        report.environment.target,
        report.environment.cuda_feature,
        report.environment.cuda_available,
        report.environment.cuda_disabled
    ));
    out.push_str(&format!(
        "- metal feature: {}\n- metal available: {}\n- metal disabled: {}\n",
        report.environment.metal_feature,
        report.environment.metal_available,
        report.environment.metal_disabled
    ));
    if let Some(err) = report.environment.cuda_last_error.as_ref() {
        out.push_str(&format!("- cuda last error: {err}\n"));
    }
    if let Some(err) = report.environment.metal_last_error.as_ref() {
        out.push_str(&format!("- metal last error: {err}\n"));
    }
    out.push('\n');
    out.push_str(
        "| width | backend | total ops | elapsed (ms) | ops/s | speedup vs scalar | parity |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for w in &report.widths {
        out.push_str(&format!(
            "| {} | scalar | {} | {:.3} | {:.1} | 1.00× | ok |\n",
            w.width, w.scalar.total_ops, w.scalar.elapsed_ms, w.scalar.ops_per_sec
        ));
        if w.cuda.available {
            let parity_label = if w.cuda.parity.ok { "ok" } else { "mismatch" };
            let elapsed = w.cuda.elapsed_ms.unwrap_or(0.0);
            let ops = w.cuda.ops_per_sec.unwrap_or(0.0);
            let speedup = w.cuda.speedup_vs_scalar.unwrap_or(0.0);
            out.push_str(&format!(
                "| {} | cuda | {} | {:.3} | {:.1} | {:.2}× | {} |\n",
                w.width, w.cuda.total_ops, elapsed, ops, speedup, parity_label
            ));
        } else {
            let note = w
                .cuda
                .note
                .as_deref()
                .unwrap_or("CUDA unavailable; recorded fallback only");
            out.push_str(&format!(
                "| {} | cuda | 0 | n/a | n/a | n/a | {note} |\n",
                w.width
            ));
        }
    }

    fs::write(path, out)?;
    Ok(())
}

fn metal_runtime_status() -> (bool, bool, bool) {
    #[cfg(target_os = "macos")]
    {
        (
            cfg!(feature = "metal"),
            ivm::metal_available(),
            ivm::metal_disabled(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        (cfg!(feature = "metal"), false, true)
    }
}

fn measure_scalar_poseidon2(inputs: &[(u64, u64)], iterations: u32) -> PerfSample {
    let total_ops = inputs.len() * iterations as usize;
    let elapsed = time_backend(iterations, || {
        for &(a, b) in inputs {
            black_box(ivm::poseidon2_simd(a, b));
        }
    });
    PerfSample {
        total_ops,
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        ops_per_sec: ops_per_second(total_ops, elapsed),
    }
}

fn measure_scalar_poseidon6(inputs: &[[u64; 6]], iterations: u32) -> PerfSample {
    let total_ops = inputs.len() * iterations as usize;
    let elapsed = time_backend(iterations, || {
        for &vals in inputs {
            black_box(ivm::poseidon6_simd(vals));
        }
    });
    PerfSample {
        total_ops,
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        ops_per_sec: ops_per_second(total_ops, elapsed),
    }
}

fn measure_cuda_poseidon2(
    inputs: &[(u64, u64)],
    expected: &[u64],
    scalar: &PerfSample,
    iterations: u32,
) -> CudaSample {
    measure_cuda_backend(
        inputs.len(),
        iterations,
        scalar,
        ivm::poseidon2_cuda_many(inputs),
        || ivm::poseidon2_cuda_many(inputs),
        expected,
    )
}

fn measure_cuda_poseidon6(
    inputs: &[[u64; 6]],
    expected: &[u64],
    scalar: &PerfSample,
    iterations: u32,
) -> CudaSample {
    measure_cuda_backend(
        inputs.len(),
        iterations,
        scalar,
        ivm::poseidon6_cuda_many(inputs),
        || ivm::poseidon6_cuda_many(inputs),
        expected,
    )
}

fn measure_cuda_backend<F>(
    batch_len: usize,
    iterations: u32,
    scalar: &PerfSample,
    first_call: Option<Vec<u64>>,
    call: F,
    expected: &[u64],
) -> CudaSample
where
    F: FnMut() -> Option<Vec<u64>>,
{
    let available = ivm::cuda_available();
    let disabled = ivm::cuda_disabled();
    let mut parity = ParityOutcome {
        checked: false,
        ok: false,
        mismatch_index: None,
        expected: None,
        actual: None,
    };
    let mut note = ivm::acceleration_runtime_errors().cuda;

    let Some(outputs) = first_call else {
        return CudaSample {
            available,
            disabled,
            total_ops: 0,
            elapsed_ms: None,
            ops_per_sec: None,
            speedup_vs_scalar: None,
            parity,
            note,
        };
    };

    parity = parity_check(expected, &outputs);
    if !parity.ok {
        note.get_or_insert_with(|| "CUDA parity mismatch".to_string());
    }

    let total_ops = batch_len * iterations as usize;
    let elapsed = measure_iterations(iterations, call).unwrap_or_else(|| {
        note.get_or_insert_with(|| "CUDA backend disabled during timing run".to_string());
        Duration::from_secs(0)
    });
    let ops_sec = if elapsed.is_zero() {
        None
    } else {
        Some(ops_per_second(total_ops, elapsed))
    };

    CudaSample {
        available,
        disabled,
        total_ops,
        elapsed_ms: ops_sec.map(|_| elapsed.as_secs_f64() * 1000.0),
        ops_per_sec: ops_sec,
        speedup_vs_scalar: ops_sec.map(|cuda| cuda / scalar.ops_per_sec),
        parity,
        note,
    }
}

fn parity_check(expected: &[u64], actual: &[u64]) -> ParityOutcome {
    let mut outcome = ParityOutcome {
        checked: true,
        ok: true,
        mismatch_index: None,
        expected: None,
        actual: None,
    };
    if expected.len() != actual.len() {
        outcome.ok = false;
        outcome.mismatch_index = Some(0);
        return outcome;
    }
    for (idx, (lhs, rhs)) in expected.iter().zip(actual.iter()).enumerate() {
        if lhs != rhs {
            outcome.ok = false;
            outcome.mismatch_index = Some(idx);
            outcome.expected = Some(*lhs);
            outcome.actual = Some(*rhs);
            break;
        }
    }
    outcome
}

fn poseidon2_inputs(batch_size: usize) -> Vec<(u64, u64)> {
    let mut rng = StdRng::seed_from_u64(0xFACE_C0DE);
    (0..batch_size)
        .map(|_| (rng.next_u64(), rng.next_u64()))
        .collect()
}

fn poseidon6_inputs(batch_size: usize) -> Vec<[u64; 6]> {
    let mut rng = StdRng::seed_from_u64(0xBADC_0FFE);
    let mut inputs = Vec::with_capacity(batch_size);
    for _ in 0..batch_size {
        inputs.push([
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
        ]);
    }
    inputs
}

fn measure_iterations<F, T>(iterations: u32, mut f: F) -> Option<Duration>
where
    F: FnMut() -> Option<T>,
{
    let start = Instant::now();
    for _ in 0..iterations {
        let value = f()?;
        black_box(value);
    }
    Some(start.elapsed())
}

fn time_backend<F: FnMut()>(iterations: u32, mut f: F) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

fn ops_per_second(total_ops: usize, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }
    (total_ops as f64) / elapsed.as_secs_f64()
}

#[cfg(test)]
mod tests {
    use norito::json::Value;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn inputs_are_deterministic() {
        let a = poseidon2_inputs(4);
        let b = poseidon2_inputs(4);
        assert_eq!(a, b, "poseidon2 inputs must be deterministic");

        let inputs6 = poseidon6_inputs(2);
        assert_eq!(inputs6.len(), 2);
        assert_ne!(inputs6[0], inputs6[1]);
    }

    #[test]
    fn rejects_zero_arguments() {
        let opts = PoseidonBenchOptions {
            batch_size: 0,
            iterations: 1,
            json_out: PathBuf::from("ignored.json"),
            markdown_out: None,
            allow_overwrite: true,
        };
        assert!(run_poseidon_bench(opts).is_err());

        let opts = PoseidonBenchOptions {
            batch_size: 4,
            iterations: 0,
            json_out: PathBuf::from("ignored.json"),
            markdown_out: None,
            allow_overwrite: true,
        };
        assert!(run_poseidon_bench(opts).is_err());
    }

    #[test]
    fn writes_json_and_markdown() {
        let json_tmp = NamedTempFile::new().unwrap();
        let md_tmp = NamedTempFile::new().unwrap();
        let opts = PoseidonBenchOptions {
            batch_size: 4,
            iterations: 1,
            json_out: json_tmp.path().to_path_buf(),
            markdown_out: Some(md_tmp.path().to_path_buf()),
            allow_overwrite: true,
        };
        let report = run_poseidon_bench(opts).expect("bench should run");
        assert_eq!(report.widths.len(), 2);
        let json = fs::read_to_string(json_tmp.path()).unwrap();
        assert!(json.contains("\"width\": 2") && json.contains("\"width\": 6"));
        let value: Value = json::from_str(&json).unwrap();
        assert!(value["environment"]["metal_feature"].is_bool());
        assert!(value["environment"]["metal_available"].is_bool());
        assert!(value["environment"]["metal_disabled"].is_bool());
        let md = fs::read_to_string(md_tmp.path()).unwrap();
        assert!(md.contains("Poseidon CUDA bench"));
        assert!(md.contains("metal feature"));
    }
}
