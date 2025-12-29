//! Bench: hardware_crc64 vs crc64_fallback on various buffer sizes.
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

// Simple deterministic PRNG (xorshift64*) to avoid external deps
fn fill_bytes(buf: &mut [u8]) {
    let mut x: u64 = 0xDEAD_BEEF_D15C_AFE5;
    for b in buf.iter_mut() {
        // xorshift64*
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
}

fn bench_crc64(c: &mut Criterion) {
    let mut group = c.benchmark_group("crc64");
    let sizes = [256usize, 1024, 8 * 1024, 64 * 1024, 512 * 1024];
    for &sz in &sizes {
        group.bench_with_input(BenchmarkId::new("fallback", sz), &sz, |b, &sz| {
            b.iter_batched(
                || {
                    let mut buf = vec![0u8; sz];
                    fill_bytes(&mut buf);
                    buf
                },
                |buf| {
                    let crc = norito::core::crc64_fallback(std::hint::black_box(&buf));
                    std::hint::black_box(crc)
                },
                BatchSize::SmallInput,
            )
        });

        group.bench_with_input(BenchmarkId::new("hardware", sz), &sz, |b, &sz| {
            b.iter_batched(
                || {
                    let mut buf = vec![0u8; sz];
                    fill_bytes(&mut buf);
                    buf
                },
                |buf| {
                    let crc = norito::core::hardware_crc64(std::hint::black_box(&buf));
                    std::hint::black_box(crc)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_crc64);
criterion_main!(benches);
