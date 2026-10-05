//! Generator-fold microbenchmarks: the lockstep GLV fold at k = 10..16 on 1
//! and 4 threads, and the constant-time reference collapse (the vendored
//! algorithm, `lo + u * hi` per point) at k = 10 and 12.
//!
//! Per-point cost is the reported time divided by `2^(k-1)`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ff::Field;
use group::prime::PrimeCurveAffine;
use group::{Curve, Group};
use iroha_pasta::fold::fold_generators_vartime;
use iroha_pasta::{Eq, EqAffine, Fp};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

fn generators(k: u32) -> Vec<EqAffine> {
    let mut rng = ChaCha20Rng::seed_from_u64(u64::from(k));
    let step = Eq::random(&mut rng);
    let mut cur = Eq::random(&mut rng);
    let projective: Vec<Eq> = (0..1usize << k)
        .map(|_| {
            cur += step;
            cur
        })
        .collect();
    let mut out = vec![EqAffine::default(); projective.len()];
    Eq::batch_normalize(&projective, &mut out);
    out
}

fn reference_collapse(g: &mut [EqAffine], u: Fp) {
    let h = g.len() / 2;
    let tmp: Vec<Eq> = (0..h).map(|i| g[i].to_curve() + g[i + h] * u).collect();
    Eq::batch_normalize(&tmp, &mut g[..h]);
}

fn bench_fold(c: &mut Criterion) {
    let mut group = c.benchmark_group("fold");
    group.sample_size(10);
    let u = Fp::random(&mut ChaCha20Rng::seed_from_u64(99));
    for threads in [1usize, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        for k in [10u32, 12, 14, 16] {
            let g = generators(k);
            group.bench_with_input(
                BenchmarkId::new("lockstep_glv", format!("k{k}_t{threads}")),
                &k,
                |b, _| {
                    b.iter_batched(
                        || g.clone(),
                        |mut v| {
                            pool.install(|| fold_generators_vartime::<Eq>(black_box(&mut v), &u))
                        },
                        criterion::BatchSize::LargeInput,
                    )
                },
            );
        }
    }
    for k in [10u32, 12] {
        let g = generators(k);
        group.bench_with_input(
            BenchmarkId::new("reference_ct", format!("k{k}_t1")),
            &k,
            |b, _| {
                b.iter_batched(
                    || g.clone(),
                    |mut v| reference_collapse(black_box(&mut v), u),
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_fold);
criterion_main!(benches);
