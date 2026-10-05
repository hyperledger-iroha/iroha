//! MSM microbenchmarks: variable-base `msm_public`/`msm_secret` at 2^10..2^16
//! on 1 and 4 threads, and the fixed-base table MSM.
//!
//! Run with `cargo bench -p iroha_pasta --bench msm`. The pool size is set
//! explicitly per benchmark; no environment variable changes behaviour.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ff::Field;
use group::{Curve, Group};
use iroha_pasta::msm::{FixedBaseTable, MemoryBudget, msm_public, msm_secret};
use iroha_pasta::{Eq, EqAffine, Fp};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

fn inputs(k: u32) -> (Vec<Fp>, Vec<EqAffine>) {
    let mut rng = ChaCha20Rng::seed_from_u64(u64::from(k));
    let n = 1usize << k;
    let scalars = (0..n).map(|_| Fp::random(&mut rng)).collect();
    // Random bases via a cheap linear walk (hash-to-curve would dominate setup).
    let g = Eq::random(&mut rng);
    let step = Eq::random(&mut rng);
    let mut cur = g;
    let projective: Vec<Eq> = (0..n)
        .map(|_| {
            cur += step;
            cur
        })
        .collect();
    let mut bases = vec![EqAffine::default(); n];
    Eq::batch_normalize(&projective, &mut bases);
    (scalars, bases)
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool")
}

fn bench_msm(c: &mut Criterion) {
    let mut group = c.benchmark_group("msm");
    group.sample_size(10);
    for threads in [1usize, 4] {
        let p = pool(threads);
        for k in [10u32, 12, 14, 16] {
            let (scalars, bases) = inputs(k);
            let id = format!("k{k}_t{threads}");
            group.bench_with_input(BenchmarkId::new("public", &id), &k, |b, _| {
                b.iter(|| {
                    p.install(|| {
                        msm_public::<Eq>(
                            black_box(&scalars),
                            black_box(&bases),
                            MemoryBudget::DEFAULT,
                        )
                    })
                })
            });
            group.bench_with_input(BenchmarkId::new("secret", &id), &k, |b, _| {
                b.iter(|| {
                    p.install(|| {
                        msm_secret::<Eq>(
                            black_box(&scalars),
                            black_box(&bases),
                            MemoryBudget::DEFAULT,
                        )
                    })
                })
            });
            if k <= 14 || threads == 1 {
                let table = p
                    .install(|| FixedBaseTable::<Eq>::new(&bases, MemoryBudget::DEFAULT))
                    .expect("table fits");
                group.bench_with_input(BenchmarkId::new("fixed_base", &id), &k, |b, _| {
                    b.iter(|| {
                        p.install(|| table.msm_secret(black_box(&scalars), MemoryBudget::DEFAULT))
                    })
                });
            }
        }
    }
    group.finish();
}

criterion_group!(benches, bench_msm);
criterion_main!(benches);
