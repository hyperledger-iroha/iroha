use std::{
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
use norito::json::build_struct_index_parallel_bench;
use norito::{
    derive::JsonSerialize,
    json as serde_json,
    json::{build_struct_index, build_struct_index_scalar_bench},
};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct Stage1BenchOptions {
    pub sizes: Vec<usize>,
    pub iterations: u32,
    pub json_out: PathBuf,
    pub markdown_out: Option<PathBuf>,
    pub allow_overwrite: bool,
}

#[derive(Clone, Debug, Serialize, JsonSerialize)]
pub struct Stage1BenchSample {
    pub size_bytes: usize,
    pub iterations: u32,
    pub default_ms_per_iter: f64,
    pub scalar_ms_per_iter: f64,
    pub speedup_vs_scalar: f64,
    #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
    pub parallel_ms_per_iter: Option<f64>,
    #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
    pub parallel_speedup_vs_scalar: Option<f64>,
}

#[derive(Clone, Debug, Serialize, JsonSerialize)]
pub struct Stage1BenchReport {
    pub timestamp: String,
    pub environment: BenchEnvironment,
    pub samples: Vec<Stage1BenchSample>,
    pub recommended_threshold_bytes: usize,
}

#[derive(Clone, Debug, Serialize, JsonSerialize)]
pub struct BenchEnvironment {
    pub target: String,
    pub arch: String,
    pub os: String,
    pub cpu: Option<String>,
}

pub fn run_stage1_bench(options: Stage1BenchOptions) -> Result<Stage1BenchReport, Box<dyn Error>> {
    if options.sizes.is_empty() {
        return Err("stage1-bench requires at least one --size value".into());
    }
    if options.iterations == 0 {
        return Err("iterations must be greater than zero".into());
    }

    let timestamp =
        OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;
    let env = BenchEnvironment {
        target: std::env::var("TARGET")
            .unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),
        arch: std::env::var("CARGO_CFG_TARGET_ARCH")
            .unwrap_or_else(|_| std::env::consts::ARCH.to_string()),
        os: std::env::var("CARGO_CFG_TARGET_OS")
            .unwrap_or_else(|_| std::env::consts::OS.to_string()),
        cpu: std::env::var("NORITO_CPU_INFO").ok(),
    };

    let mut samples = Vec::with_capacity(options.sizes.len());
    for &size in &options.sizes {
        let payload = make_payload(size);
        // Warm-up
        let _ = build_struct_index(&payload);
        let default_ms = time_backend(options.iterations, || {
            let tape = build_struct_index(&payload);
            std::hint::black_box(tape.offsets.len());
        });
        let scalar_ms = time_backend(options.iterations, || {
            let tape = build_struct_index_scalar_bench(&payload);
            std::hint::black_box(tape.offsets.len());
        });
        let mut sample = Stage1BenchSample {
            size_bytes: payload.len(),
            iterations: options.iterations,
            default_ms_per_iter: default_ms.as_secs_f64() * 1000.0 / (options.iterations as f64),
            scalar_ms_per_iter: scalar_ms.as_secs_f64() * 1000.0 / (options.iterations as f64),
            speedup_vs_scalar: 0.0,
            #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
            parallel_ms_per_iter: None,
            #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
            parallel_speedup_vs_scalar: None,
        };
        sample.speedup_vs_scalar = sample.scalar_ms_per_iter / sample.default_ms_per_iter;
        #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
        {
            let parallel_ms = time_backend(options.iterations, || {
                let tape = build_struct_index_parallel_bench(&payload);
                std::hint::black_box(tape.offsets.len());
            });
            let parallel_ms_per_iter =
                parallel_ms.as_secs_f64() * 1000.0 / (options.iterations as f64);
            sample.parallel_ms_per_iter = Some(parallel_ms_per_iter);
            sample.parallel_speedup_vs_scalar =
                Some(sample.scalar_ms_per_iter / parallel_ms_per_iter);
        }
        samples.push(sample);
    }

    let recommended_threshold_bytes = samples
        .iter()
        .find(|s| s.speedup_vs_scalar >= 1.05)
        .map(|s| s.size_bytes)
        .unwrap_or_else(|| samples.last().map(|s| s.size_bytes).unwrap_or(0));

    let report = Stage1BenchReport {
        timestamp,
        environment: env,
        samples,
        recommended_threshold_bytes,
    };

    write_json(&report, &options.json_out, options.allow_overwrite)?;
    if let Some(md) = options.markdown_out {
        write_markdown(&report, &md, options.allow_overwrite)?;
    }

    Ok(report)
}

fn write_json(
    report: &Stage1BenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !allow_overwrite {
        return Err(format!("refusing to overwrite existing file {}", path.display()).into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(report)?;
    fs::write(path, data)?;
    Ok(())
}

fn write_markdown(
    report: &Stage1BenchReport,
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
    writeln!(
        out,
        "# Norito Stage-1 benchmark\n\n- timestamp: {}\n- target: {}\n- arch/os: {}/{}\n- recommended threshold: {} bytes\n",
        report.timestamp,
        report.environment.target,
        report.environment.arch,
        report.environment.os,
        report.recommended_threshold_bytes
    )?;
    writeln!(
        out,
        "| size (bytes) | default ms/iter | scalar ms/iter | speedup vs scalar |"
    )?;
    writeln!(out, "| --- | --- | --- | --- |")?;
    for sample in &report.samples {
        writeln!(
            out,
            "| {} | {:.3} | {:.3} | {:.2}x |",
            sample.size_bytes,
            sample.default_ms_per_iter,
            sample.scalar_ms_per_iter,
            sample.speedup_vs_scalar
        )?;
    }
    fs::write(path, out)?;
    Ok(())
}

fn time_backend<F: FnMut()>(iterations: u32, mut f: F) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

fn make_payload(target_bytes: usize) -> String {
    // Deterministic large JSON: array of small objects with tricky strings.
    // This mirrors the criterion benchmarks and leans on escaped characters to exercise stage-1 scanning costs.
    let base = r#"{"k":"a\"b","u":"\u0041","bs":"\\\\"}"#;
    let mut s = String::with_capacity(target_bytes + 2);
    s.push('[');
    let mut written = 1usize; // leading '['
    let mut first = true;
    while written + base.len() + 2 <= target_bytes {
        if !first {
            s.push(',');
        }
        s.push_str(base);
        written += base.len() + 1;
        first = false;
    }
    s.push(']');
    if s.len() < target_bytes {
        let pad = "0".repeat(target_bytes - s.len());
        // Insert padding just before the closing bracket to preserve JSON shape.
        s.insert_str(s.len() - 1, &pad);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_meets_requested_size() {
        let p = make_payload(8 * 1024);
        assert!(
            p.len() >= 8 * 1024,
            "payload len {} should meet target",
            p.len()
        );
        assert!(p.starts_with('[') && p.ends_with(']'));
    }

    #[test]
    fn bench_runs_with_small_sizes() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let opts = Stage1BenchOptions {
            sizes: vec![2048, 4096],
            iterations: 1,
            json_out: tmp.path().to_path_buf(),
            markdown_out: None,
            allow_overwrite: true,
        };
        let report = run_stage1_bench(opts).expect("bench should run");
        assert_eq!(report.samples.len(), 2);
        assert!(report.recommended_threshold_bytes >= report.samples[0].size_bytes);
    }
}
