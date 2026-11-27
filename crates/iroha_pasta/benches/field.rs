//! Field arithmetic microbenchmarks for `Fp` and `Fq`, with `pasta_curves`
//! 0.6.1 measured in the same binary as the baseline.
//!
//! Latency benchmarks chain 1000 dependent operations; throughput benchmarks
//! run 1024 independent operations. Divide the reported time by 1000 or 1024
//! for the per-operation cost.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ff::{Field, PrimeField};
use iroha_pasta::PastaField;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

macro_rules! field_benches {
    ($c:expr, $name:literal, $f:ty) => {{
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        let a = <$f>::random(&mut rng);
        let b = <$f>::random(&mut rng);
        let xs: Vec<$f> = (0..1024).map(|_| <$f>::random(&mut rng)).collect();
        let ys: Vec<$f> = (0..1024).map(|_| <$f>::random(&mut rng)).collect();
        let mut g = $c.benchmark_group($name);
        g.bench_function("mul_latency_x1000", |bench| {
            bench.iter(|| {
                let mut x = black_box(a);
                for _ in 0..1000 {
                    x *= b;
                }
                x
            })
        });
        g.bench_function("square_latency_x1000", |bench| {
            bench.iter(|| {
                let mut x = black_box(a);
                for _ in 0..1000 {
                    x = x.square();
                }
                x
            })
        });
        g.bench_function("mul_throughput_x1024", |bench| {
            bench.iter(|| {
                let mut acc = <$f>::ZERO;
                for (x, y) in xs.iter().zip(ys.iter()) {
                    acc += *x * y;
                }
                acc
            })
        });
        g.bench_function("add_latency_x1000", |bench| {
            bench.iter(|| {
                let mut x = black_box(a);
                for _ in 0..1000 {
                    x += b;
                }
                x
            })
        });
        g.bench_function("invert", |bench| bench.iter(|| black_box(a).invert()));
        g.bench_function("sqrt", |bench| bench.iter(|| black_box(a).sqrt()));
        g.bench_function("to_repr_from_repr", |bench| {
            bench.iter(|| <$f>::from_repr(black_box(a).to_repr()))
        });
        g.finish();
    }};
}

/// Same coefficient and dependent operands for both paths. The full-width
/// multiplication receives its coefficient already in Montgomery form.
fn word_product<F: PastaField>(c: &mut Criterion, name: &str) {
    let mut g = c.benchmark_group(name);
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let input = F::random(&mut rng);
    let word = black_box(0xff25_2b8f_1c51_7663_u64);
    let field = F::from(word);
    g.bench_function("full_x1000", |bench| {
        bench.iter(|| {
            let mut x = black_box(input);
            for _ in 0..1000 {
                x *= field;
            }
            x
        })
    });
    g.bench_function("word_x1000", |bench| {
        bench.iter(|| {
            let mut x = black_box(input);
            for _ in 0..1000 {
                x = x.mul_u64(word);
            }
            x
        })
    });
}

fn bench_fields(c: &mut Criterion) {
    word_product::<iroha_pasta::Fp>(c, "fp_word_product");
    word_product::<iroha_pasta::Fq>(c, "fq_word_product");
    field_benches!(c, "iroha_pasta_fp", iroha_pasta::Fp);
    field_benches!(c, "iroha_pasta_fq", iroha_pasta::Fq);
    field_benches!(c, "pasta_curves_fp", pasta_curves::Fp);
    field_benches!(c, "pasta_curves_fq", pasta_curves::Fq);

    let mut rng = ChaCha20Rng::seed_from_u64(2);
    let a = iroha_pasta::Fq::random(&mut rng);
    let mut g = c.benchmark_group("iroha_pasta_fq_extra");
    g.bench_function("invert_vartime", |bench| {
        bench.iter(|| black_box(a).invert_vartime())
    });
    let values: Vec<iroha_pasta::Fq> = (0..1024)
        .map(|_| iroha_pasta::Fq::random(&mut rng))
        .collect();
    g.bench_function("batch_invert_1024", |bench| {
        bench.iter(|| {
            let mut v = values.clone();
            iroha_pasta::field::batch_invert(&mut v);
            v
        })
    });
    g.bench_function("batch_invert_vartime_1024", |bench| {
        bench.iter(|| {
            let mut v = values.clone();
            iroha_pasta::field::batch_invert_vartime(&mut v);
            v
        })
    });
    g.finish();
}

criterion_group!(benches, bench_fields);
criterion_main!(benches);
