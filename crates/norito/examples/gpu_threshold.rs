//! Quick-and-dirty benchmark to compare Norito GPU compression against the CPU fallback.
//! Run with `cargo run --release -p norito --example gpu_threshold` on a host
//! that has Metal (Apple Silicon) or CUDA kernels available.

use std::{
    env,
    fmt::Write as _,
    time::{Duration, Instant},
};

#[cfg(feature = "gpu-compression")]
use norito::core::{gpu_zstd, heuristics::compress_auto, hw};
#[cfg(not(feature = "gpu-compression"))]
use norito::core::{heuristics::compress_auto, hw};

const SIZES: &[usize] = &[
    64 * 1024,
    256 * 1024,
    512 * 1024,
    1024 * 1024,
    2 * 1024 * 1024,
    4 * 1024 * 1024,
    8 * 1024 * 1024,
];
const SAMPLES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    Auto,
    CpuOnly,
    AccelOnly,
}

fn build_payload(len: usize) -> Vec<u8> {
    let mut data = vec![0u8; len];
    for (idx, byte) in data.iter_mut().enumerate() {
        // Deterministic but non-trivial pattern to give the compressor real work.
        *byte = (idx as u8).wrapping_mul(31).wrapping_add(7);
    }
    data
}

fn measure<F: FnMut()>(mut f: F) -> Duration {
    // Warm-up run to amortize first-call setup (e.g., Metal pipeline compilation).
    f();
    let mut best = Duration::MAX;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed();
        best = best.min(elapsed);
    }
    best
}

fn bench_size(
    len: usize,
    iterations: usize,
    policy: Policy,
) -> (Option<Duration>, Option<Duration>) {
    let payload = build_payload(len);
    let baseline_policy = hw::gpu_policy_allowed();

    // CPU baseline (GPU policy disabled) unless caller requested acceleration-only.
    let cpu_time = if policy != Policy::AccelOnly {
        hw::set_gpu_compression_allowed(false);
        let cpu_payload = payload.clone();
        Some(measure(|| {
            let mut total_len = 0usize;
            for _ in 0..iterations {
                let (kind, compressed) =
                    compress_auto(cpu_payload.clone()).expect("CPU compression should succeed");
                std::hint::black_box(&kind);
                total_len = total_len.wrapping_add(compressed.len());
            }
            std::hint::black_box(total_len);
        }))
    } else {
        None
    };

    // Attempt GPU measurement (policy enabled) unless CPU-only.
    let gpu_time = if policy != Policy::CpuOnly {
        #[cfg(feature = "gpu-compression")]
        {
            hw::set_gpu_compression_allowed(true);
            if gpu_zstd::available() {
                let gpu_payload = payload.clone();
                Some(measure(|| {
                    let mut total_len = 0usize;
                    for _ in 0..iterations {
                        let (kind, compressed) = compress_auto(gpu_payload.clone())
                            .expect("GPU compression should succeed");
                        std::hint::black_box(&kind);
                        total_len = total_len.wrapping_add(compressed.len());
                    }
                    std::hint::black_box(total_len);
                }))
            } else {
                None
            }
        }
        #[cfg(not(feature = "gpu-compression"))]
        {
            let _ = (iterations, &payload);
            None
        }
    } else {
        None
    };

    hw::set_gpu_compression_allowed(baseline_policy);
    (cpu_time, gpu_time)
}

fn main() {
    let mut output_json = false;
    let mut policy = Policy::Auto;
    for arg in env::args().skip(1) {
        if arg == "--json" {
            output_json = true;
        } else if let Some(rest) = arg.strip_prefix("--policy=") {
            policy = match rest {
                "cpu" => Policy::CpuOnly,
                "gpu" | "accel" => Policy::AccelOnly,
                _ => Policy::Auto,
            };
        }
    }
    let mut rows = Vec::new();
    let mut gpu_available = false;

    for &size in SIZES {
        let iterations = if size <= 512 * 1024 { 20 } else { 8 };
        let (cpu_opt, gpu_opt) = bench_size(size, iterations, policy);
        if gpu_opt.is_some() {
            gpu_available = true;
        }
        rows.push((
            size / 1024,
            cpu_opt.map(|d| d.as_secs_f64() * 1_000.0),
            gpu_opt.map(|d| d.as_secs_f64() * 1_000.0),
        ));
    }

    if output_json {
        let mut json = String::new();
        json.push_str(&format!("{{\"gpu_available\":{gpu_available},\"rows\":["));
        for (idx, (size, cpu_ms, gpu_ms)) in rows.iter().enumerate() {
            if idx > 0 {
                json.push(',');
            }
            let cpu_val = cpu_ms
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "null".into());
            let gpu_val = gpu_ms
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "null".into());
            let speed = match (cpu_ms, gpu_ms) {
                (Some(cpu), Some(gpu)) if *gpu > 0.0 => {
                    let ratio = cpu / gpu;
                    format!("{ratio:.6}")
                }
                _ => "null".into(),
            };
            let _ = write!(
                &mut json,
                "{{\"size_kib\":{size},\"cpu_ms\":{cpu_val},\"gpu_ms\":{gpu_val},\"speedup\":{speed}}}"
            );
        }
        json.push_str("]}");
        println!("{json}");
    } else {
        println!(
            "Norito compression benchmark ({} payload sizes)",
            SIZES.len()
        );
        println!("Columns: size_kib, cpu_ms, gpu_ms, speedup");
        for (size, cpu_ms, gpu_ms) in rows {
            match (cpu_ms, gpu_ms) {
                (Some(cpu), Some(gpu)) => println!(
                    "{:>8} {:>10.2} {:>10.2} {:>7.2}x",
                    size,
                    cpu,
                    gpu,
                    cpu / gpu
                ),
                (Some(cpu), None) => {
                    println!("{size:>8} {cpu:>10.2}      N/A       N/A (GPU backend unavailable)")
                }
                (None, Some(gpu)) => {
                    println!("{size:>8}      N/A {gpu:>10.2}       N/A (CPU measurement disabled)")
                }
                (None, None) => {
                    println!("{size:>8}      N/A       N/A       N/A (no measurements)")
                }
            }
        }
    }
}
