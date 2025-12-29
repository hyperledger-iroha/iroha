//! Benchmarks for CRC64 implementations (hardware, SIMD, and fallback).
use criterion::{BatchSize, Criterion};

fn random_bytes(n: usize) -> Vec<u8> {
    // Deterministic pseudo-random data for benchmarking
    let mut v = Vec::with_capacity(n);
    let mut x: u64 = 0x1234_5678_9abc_def0;
    for _ in 0..n {
        x ^= x << 7;
        x ^= x >> 9;
        v.push((x as u8) ^ 0x5a);
    }
    v
}

fn bench_crc64(c: &mut Criterion) {
    let sizes = [1024usize, 16 * 1024, 256 * 1024, 1024 * 1024];
    for &n in &sizes {
        c.bench_function(&format!("crc64_hardware_{n}B"), |b| {
            b.iter_batched(
                || random_bytes(n),
                |buf| {
                    let h = norito::hardware_crc64(std::hint::black_box(&buf));
                    std::hint::black_box(h)
                },
                BatchSize::SmallInput,
            )
        });

        c.bench_function(&format!("crc64_fallback_{n}B"), |b| {
            b.iter_batched(
                || random_bytes(n),
                |buf| {
                    let h = norito::crc64_fallback(std::hint::black_box(&buf));
                    std::hint::black_box(h)
                },
                BatchSize::SmallInput,
            )
        });

        #[cfg(all(
            feature = "simd-accel",
            target_arch = "x86_64",
            target_feature = "pclmulqdq"
        ))]
        c.bench_function(&format!("crc64_pclmul_{}B", n), |b| {
            b.iter_batched(
                || random_bytes(n),
                |buf| {
                    let h = norito::core::simd_crc64::crc64_pclmul(std::hint::black_box(&buf));
                    std::hint::black_box(h)
                },
                BatchSize::SmallInput,
            )
        });

        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        c.bench_function(&format!("crc64_pmull_{}B", n), |b| {
            b.iter_batched(
                || random_bytes(n),
                |buf| {
                    // SAFETY: compiled only when simd-accel is enabled; runtime feature check is inside impl
                    let h = unsafe {
                        norito::core::simd_crc64::crc64_pmull(std::hint::black_box(&buf))
                    };
                    std::hint::black_box(h)
                },
                BatchSize::SmallInput,
            )
        });
    }
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_crc64(&mut c);
    c.final_summary();
}
