//! FFT microbenchmarks: forward, inverse and coset transforms at k = 11..16 on
//! 1 and 4 threads, and parameter generation (hash-to-curve plus group IFFT).

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ff::{Field, WithSmallOrderMulGroup};
use iroha_pasta::fft::FftDomain;
use iroha_pasta::params::ParamsIpa;
use iroha_pasta::{Eq, Fp};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

fn bench_fft(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft");
    group.sample_size(20);
    for threads in [1usize, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        for k in [11u32, 12, 14, 16] {
            let domain = FftDomain::<Fp>::new(k).expect("domain");
            let mut rng = ChaCha20Rng::seed_from_u64(u64::from(k));
            let a: Vec<Fp> = (0..1usize << k).map(|_| Fp::random(&mut rng)).collect();
            let id = format!("k{k}_t{threads}");
            group.bench_with_input(BenchmarkId::new("fft", &id), &k, |b, _| {
                b.iter_batched(
                    || a.clone(),
                    |mut v| pool.install(|| domain.fft(black_box(&mut v))),
                    criterion::BatchSize::LargeInput,
                )
            });
            group.bench_with_input(BenchmarkId::new("ifft", &id), &k, |b, _| {
                b.iter_batched(
                    || a.clone(),
                    |mut v| pool.install(|| domain.ifft(black_box(&mut v))),
                    criterion::BatchSize::LargeInput,
                )
            });
            group.bench_with_input(BenchmarkId::new("coset_fft", &id), &k, |b, _| {
                b.iter_batched(
                    || a.clone(),
                    |mut v| pool.install(|| domain.coset_fft(black_box(&mut v), Fp::ZETA)),
                    criterion::BatchSize::LargeInput,
                )
            });
        }
    }
    group.finish();

    let mut params = c.benchmark_group("params");
    params.sample_size(10);
    for threads in [1usize, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        for k in [11u32, 14] {
            params.bench_with_input(
                BenchmarkId::new("params_ipa_new", format!("k{k}_t{threads}")),
                &k,
                |b, &k| b.iter(|| pool.install(|| ParamsIpa::<Eq>::new(black_box(k)))),
            );
        }
    }
    params.finish();
}

criterion_group!(benches, bench_fft);
criterion_main!(benches);
