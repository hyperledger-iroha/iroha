//! Stage-1 (structural index) micro-benchmarks: scalar vs NEON.
#[cfg(all(feature = "json", feature = "simd-accel"))]
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(all(feature = "json", feature = "simd-accel"))]
fn make_large_json(doc_kib: usize) -> String {
    // Deterministic large JSON: array of small objects with tricky strings.
    // Total size ~ doc_kib KiB.
    let mut s = String::with_capacity(doc_kib * 1024 + 2);
    s.push('[');
    let mut first = true;
    let base = r#"{"k":"a\"b","u":"\u0041","bs":"\\\\"}"#; // includes escaped quote, unicode, backslashes
    let unit_len = base.len() + 1; // + comma
    let mut written = 1usize; // leading '['
    while written + unit_len + 1 < doc_kib * 1024 {
        // + trailing ']'
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(base);
        written += unit_len;
    }
    s.push(']');
    s
}

#[cfg(all(feature = "json", feature = "simd-accel"))]
fn bench_stage1(c: &mut Criterion) {
    let mut group = c.benchmark_group("stage1_struct_index");
    for kib in [64usize, 256, 1024] {
        let doc = make_large_json(kib);
        group.throughput(Throughput::Bytes(doc.len() as u64));

        // Scalar reference builder (exposed via bench-internal feature)
        group.bench_function(BenchmarkId::new("scalar", kib), |b| {
            b.iter_batched(
                || doc.clone(),
                |d| {
                    #[cfg(feature = "bench-internal")]
                    {
                        let tape = norito::json::build_struct_index_scalar_bench(&d);
                        std::hint::black_box(tape.offsets.len());
                    }
                    #[cfg(not(feature = "bench-internal"))]
                    {
                        let _ = d; // no-op; feature not enabled
                    }
                },
                BatchSize::SmallInput,
            )
        });

        // NEON (or fallback if not available at runtime)
        group.bench_function(BenchmarkId::new("neon_or_fallback", kib), |b| {
            b.iter_batched(
                || doc.clone(),
                |d| {
                    let tape = norito::json::build_struct_index(&d);
                    std::hint::black_box(tape.offsets.len());
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

#[cfg(all(feature = "json", feature = "simd-accel"))]
criterion_group!(benches, bench_stage1);

#[cfg(all(feature = "json", feature = "simd-accel"))]
criterion_main!(benches);

#[cfg(not(all(feature = "json", feature = "simd-accel")))]
fn main() {
    eprintln!("Enable `json` and `simd-accel` features to run this benchmark.");
}
