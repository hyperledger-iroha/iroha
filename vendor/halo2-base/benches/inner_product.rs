use halo2_base::gates::circuit::{builder::RangeCircuitBuilder, CircuitBuilderStage};
use halo2_base::gates::flex_gate::{GateChip, GateInstructions};
use halo2_base::halo2_proofs::{
    arithmetic::Field,
    dev::MockProver,
    halo2curves::bn256::{Bn256, Fr},
    plonk::*,
    poly::kzg::commitment::ParamsKZG,
};
use halo2_base::utils::testing::gen_proof;
use halo2_base::utils::ScalarField;
use halo2_base::{Context, QuantumCell::Existing};
use itertools::Itertools;
use rand::rngs::OsRng;

use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion};

use pprof::criterion::{Output, PProfProfiler};
// Thanks to the example provided by @jebbow in his article
// https://www.jibbow.com/posts/criterion-flamegraphs/

const K: u32 = 19;

fn inner_prod_bench<F: ScalarField>(ctx: &mut Context<F>, a: Vec<F>, b: Vec<F>) {
    assert_eq!(a.len(), b.len());
    let a = ctx.assign_witnesses(a);
    let b = ctx.assign_witnesses(b);

    let chip = GateChip::default();
    for _ in 0..(1 << K) / 16 - 10 {
        chip.inner_product(ctx, a.clone(), b.clone().into_iter().map(Existing));
    }
}

fn bench(c: &mut Criterion) {
    let k = 19u32;
    // create circuit for keygen
    let mut builder =
        RangeCircuitBuilder::from_stage(CircuitBuilderStage::Keygen).use_k(k as usize);
    inner_prod_bench(builder.main(0), vec![Fr::zero(); 5], vec![Fr::zero(); 5]);
    let config_params = builder.calculate_params(Some(20));

    // check the circuit is correct just in case
    MockProver::run(k, &builder, vec![]).unwrap().assert_satisfied();

    let params = ParamsKZG::<Bn256>::setup(k, OsRng);
    let vk = keygen_vk(&params, &builder).expect("vk should not fail");
    let pk = keygen_pk(&params, vk, &builder).expect("pk should not fail");

    let break_points = builder.break_points();
    drop(builder);

    let mut group = c.benchmark_group("plonk-prover");
    group.sample_size(10);
    group.bench_with_input(
        BenchmarkId::new("inner_product", k),
        &(&params, &pk),
        |bencher, &(params, pk)| {
            bencher.iter(|| {
                let mut builder =
                    RangeCircuitBuilder::prover(config_params.clone(), break_points.clone());
                let a = (0..5).map(|_| Fr::random(OsRng)).collect_vec();
                let b = (0..5).map(|_| Fr::random(OsRng)).collect_vec();
                inner_prod_bench(builder.main(0), a, b);
                gen_proof(params, pk, builder);
            })
        },
    );
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(10, Output::Flamegraph(None)));
    targets = bench
}
criterion_main!(benches);
