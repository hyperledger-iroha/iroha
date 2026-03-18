#[macro_use]
extern crate criterion;

use crate::arithmetic::best_fft;
use group::ff::Field;
use halo2_axiom::{poly::EvaluationDomain, *};
use halo2curves::bn256::Fr as Scalar;

use criterion::{BenchmarkId, Criterion};
use rand_core::OsRng;

fn criterion_benchmark(c: &mut Criterion) {
    let j = 5;
    let mut group = c.benchmark_group("fft");
    for k in 3..19 {
        let domain = EvaluationDomain::new(j, k);
        let omega = domain.get_omega();
        let l = 1 << k;
        let data = domain.get_fft_data(l);

        group.bench_function(BenchmarkId::new("k", k), |b| {
            let mut a = (0..(1 << k))
                .map(|_| Scalar::random(OsRng))
                .collect::<Vec<_>>();

            b.iter(|| {
                best_fft(&mut a, omega, k, data, false);
            });
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
