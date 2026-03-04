//! Quick comparison between Metal-accelerated and CPU Merkle hashing paths.
//! Run with `cargo run --release -p ivm --example merkle_threshold`.

use std::{
    env,
    fmt::Write as _,
    hint::black_box,
    time::{Duration, Instant},
};

use ivm::{AccelerationConfig, ByteMerkleTree};

const CHUNK: usize = 32;
const LEAF_COUNTS: &[usize] = &[1_024, 4_096, 8_192, 16_384, 32_768, 65_536];
const SAMPLES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    Auto,
    CpuOnly,
    AccelOnly,
}

fn build_bytes(leaves: usize) -> Vec<u8> {
    let total = leaves * CHUNK;
    let mut data = vec![0u8; total];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(13).wrapping_add(5);
    }
    data
}

fn measure<F: FnMut()>(mut f: F) -> Duration {
    // Warm-up once to include shader compilation / pipeline setup.
    f();
    let mut best = Duration::MAX;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        f();
        best = best.min(start.elapsed());
    }
    best
}

fn bench(leaves: usize, iterations: usize, policy: Policy) -> (Option<Duration>, Option<Duration>) {
    let payload = build_bytes(leaves);

    // CPU baseline (Metal disabled) unless acceleration-only policy.
    let cpu = if policy != Policy::AccelOnly {
        ivm::set_acceleration_config(AccelerationConfig {
            enable_simd: true,
            enable_metal: false,
            enable_cuda: false,
            max_gpus: None,
            merkle_min_leaves_gpu: None,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        });
        Some(measure(|| {
            let mut checksum = 0u8;
            for _ in 0..iterations {
                let tree = ByteMerkleTree::from_bytes_accel(&payload, CHUNK);
                let root = tree.root();
                checksum ^= root.as_ref()[0];
            }
            black_box(checksum);
        }))
    } else {
        None
    };

    // Metal acceleration measurement unless CPU-only policy.
    let metal = if policy != Policy::CpuOnly {
        ivm::reset_metal_backend_for_tests();
        ivm::set_acceleration_config(AccelerationConfig {
            enable_simd: true,
            enable_metal: true,
            enable_cuda: false,
            max_gpus: None,
            merkle_min_leaves_gpu: None,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        });
        if ivm::metal_available() {
            Some(measure(|| {
                let mut checksum = 0u8;
                for _ in 0..iterations {
                    let tree = ByteMerkleTree::from_bytes_accel(&payload, CHUNK);
                    let root = tree.root();
                    checksum ^= root.as_ref()[0];
                }
                black_box(checksum);
            }))
        } else {
            None
        }
    } else {
        None
    };

    (cpu, metal)
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
                "gpu" | "metal" | "accel" => Policy::AccelOnly,
                _ => Policy::Auto,
            };
        }
    }

    let mut rows = Vec::new();
    let mut metal_seen = false;

    for &leaves in LEAF_COUNTS {
        let iterations = if leaves <= 8_192 { 10 } else { 4 };
        let (cpu_opt, metal_opt) = bench(leaves, iterations, policy);
        if metal_opt.is_some() {
            metal_seen = true;
        }
        rows.push((
            leaves,
            cpu_opt.map(|d| d.as_secs_f64() * 1_000.0),
            metal_opt.map(|d| d.as_secs_f64() * 1_000.0),
        ));
    }

    if output_json {
        let mut json = String::new();
        json.push_str(&format!("{{\"metal_available\":{metal_seen},\"rows\":["));
        for (idx, (leaves, cpu_ms, metal_ms)) in rows.iter().enumerate() {
            if idx > 0 {
                json.push(',');
            }
            let cpu_val = cpu_ms
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "null".into());
            let metal_val = metal_ms
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "null".into());
            let speed = match (cpu_ms, metal_ms) {
                (Some(cpu), Some(metal)) if *metal > 0.0 => {
                    let ratio = cpu / metal;
                    format!("{ratio:.6}")
                }
                _ => "null".into(),
            };
            let _ = write!(
                &mut json,
                "{{\"leaves\":{leaves},\"cpu_ms\":{cpu_val},\"metal_ms\":{metal_val},\"speedup\":{speed}}}"
            );
        }
        json.push_str("]}");
        println!("{json}");
    } else {
        println!("IVM ByteMerkle GPU benchmark (chunk size {CHUNK} bytes)");
        println!("Columns: leaves, cpu_ms, metal_ms, speedup");
        for (leaves, cpu_ms, metal_ms) in rows {
            match (cpu_ms, metal_ms) {
                (Some(cpu), Some(metal)) => println!(
                    "{leaves:>8} {cpu:>10.2} {metal:>10.2} {ratio:>7.2}x",
                    ratio = cpu / metal
                ),
                (Some(cpu), None) => println!(
                    "{leaves:>8} {cpu:>10.2}      N/A      N/A (Metal backend not available)"
                ),
                (None, Some(metal)) => println!(
                    "{leaves:>8}      N/A {metal:>10.2}       N/A (CPU measurement disabled)"
                ),
                (None, None) => {
                    println!("{leaves:>8}      N/A      N/A       N/A (no measurements)")
                }
            }
        }
    }

    // Restore defaults (Metal enabled, CUDA state unchanged).
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: ivm::cuda_available(),
        max_gpus: None,
        merkle_min_leaves_gpu: None,
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
    });
}
