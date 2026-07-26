use halo2_base::gates::circuit::{builder::RangeCircuitBuilder, CircuitBuilderStage};
use halo2_base::gates::flex_gate::{GateChip, GateInstructions};
use halo2_base::halo2_proofs::{
    halo2curves::bn256::{Bn256, Fr},
    halo2curves::ff::Field,
    plonk::*,
    poly::kzg::commitment::ParamsKZG,
};
use halo2_base::utils::testing::gen_proof;
use halo2_base::utils::ScalarField;
use halo2_base::Context;
use rand::rngs::OsRng;

use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion};

use pprof::criterion::{Output, PProfProfiler};
// Thanks to the example provided by @jebbow in his article
// https://www.jibbow.com/posts/criterion-flamegraphs/

const K: u32 = 9;

fn mul_bench<F: ScalarField>(ctx: &mut Context<F>, inputs: [F; 2]) {
    let [a, b]: [_; 2] = ctx.assign_witnesses(inputs).try_into().unwrap();
    let chip = GateChip::default();

    for _ in 0..120 {
        chip.mul(ctx, a, b);
    }
}

fn bench(c: &mut Criterion) {
    // create circuit for keygen
    let mut builder =
        RangeCircuitBuilder::from_stage(CircuitBuilderStage::Keygen).use_k(K as usize);
    mul_bench(builder.main(0), [Fr::zero(); 2]);
    let config_params = builder.calculate_params(Some(9));

    let params = ParamsKZG::<Bn256>::setup(K, OsRng);
    let vk = keygen_vk(&params, &builder).expect("vk should not fail");
    let pk = keygen_pk(&params, vk, &builder).expect("pk should not fail");

    let break_points = builder.break_points();

    let a = Fr::random(OsRng);
    let b = Fr::random(OsRng);
    // native multiplication 120 times
    c.bench_with_input(
        BenchmarkId::new("native mul", K),
        &(&params, &pk, [a, b]),
        |bencher, &(params, pk, inputs)| {
            bencher.iter(|| {
                let mut builder =
                    RangeCircuitBuilder::prover(config_params.clone(), break_points.clone());
                // do the computation
                mul_bench(builder.main(0), inputs);

                gen_proof(params, pk, builder);
            })
        },
    );
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(10, Output::Flamegraph(None)));
    targets = bench
}
criterion_main!(benches);
