use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use sm3::{Digest, Sm3};

fn bench_digest(c: &mut Criterion) {
    let message = vec![0u8; 1024];
    if !sm3_neon::is_supported() {
        // Skip gracefully on non-NEON hosts so CI can run the suite everywhere.
        return;
    }

    let mut group = c.benchmark_group("sm3_neon_digest");
    group.throughput(Throughput::Bytes(message.len() as u64));

    group.bench_function("neon_1k", |b| {
        b.iter(|| {
            let digest =
                sm3_neon::digest(black_box(message.as_slice())).expect("NEON backend available");
            black_box(digest);
        });
    });

    group.bench_function("scalar_1k", |b| {
        b.iter(|| {
            let mut hasher = Sm3::new();
            hasher.update(black_box(message.as_slice()));
            black_box(hasher.finalize())
        });
    });

    group.finish();
}

criterion_group!(sm3_neon_benches, bench_digest);
criterion_main!(sm3_neon_benches);
