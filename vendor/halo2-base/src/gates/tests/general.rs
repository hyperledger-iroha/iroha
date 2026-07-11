use crate::ff::Field;
use crate::gates::flex_gate::threads::parallelize_core;
use crate::halo2_proofs::halo2curves::bn256::Fr;
use crate::utils::{BigPrimeField, ScalarField};
use crate::{
    gates::{
        flex_gate::{GateChip, GateInstructions},
        range::{RangeChip, RangeInstructions},
    },
    utils::testing::base_test,
};
use crate::{Context, QuantumCell::Constant};
use rand::rngs::StdRng;
use rand::SeedableRng;
use test_log::test;

fn gate_tests<F: ScalarField>(ctx: &mut Context<F>, inputs: [F; 3]) {
    let [a, b, c]: [_; 3] = ctx.assign_witnesses(inputs).try_into().unwrap();
    let chip = GateChip::default();

    // test add
    chip.add(ctx, a, b);

    // test sub
    chip.sub(ctx, a, b);

    // test multiply
    chip.mul(ctx, c, b);

    // test idx_to_indicator
    chip.idx_to_indicator(ctx, Constant(F::from(3u64)), 4);

    let bits = ctx.assign_witnesses([F::ZERO, F::ONE]);
    chip.bits_to_indicator(ctx, &bits);

    chip.is_equal(ctx, b, a);

    chip.is_zero(ctx, a);
}

#[test]
fn test_multithread_gates() {
    let mut rng = StdRng::seed_from_u64(0);
    base_test().k(6).bench_builder(
        vec![[Fr::ZERO; 3]; 4],
        (0..4usize).map(|_| [(); 3].map(|_| Fr::random(&mut rng))).collect(),
        |pool, _, inputs| {
            parallelize_core(pool, inputs, |ctx, input| {
                gate_tests(ctx, input);
            });
        },
    );
}

#[cfg(feature = "dev-graph")]
#[test]
fn plot_gates() {
    let k = 5;
    use plotters::prelude::*;

    use crate::gates::circuit::builder::BaseCircuitBuilder;

    let root = BitMapBackend::new("layout.png", (1024, 1024)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root.titled("Gates Layout", ("sans-serif", 60)).unwrap();

    let inputs = [Fr::zero(); 3];
    let mut builder = BaseCircuitBuilder::new(false).use_k(k);
    gate_tests(builder.main(0), inputs);

    // auto-tune circuit
    builder.calculate_params(Some(9));
    halo2_proofs::dev::CircuitLayout::default().render(k as u32, &builder, &root).unwrap();
}

fn range_tests<F: BigPrimeField>(
    ctx: &mut Context<F>,
    chip: &RangeChip<F>,
    inputs: [F; 2],
    range_bits: usize,
    lt_bits: usize,
) {
    let [a, b]: [_; 2] = ctx.assign_witnesses(inputs).try_into().unwrap();
    chip.range_check(ctx, a, range_bits);

    chip.check_less_than(ctx, a, b, lt_bits);

    chip.is_less_than(ctx, a, b, lt_bits);

    chip.is_less_than(ctx, b, a, lt_bits);

    chip.div_mod(ctx, a, 7u64, lt_bits);
}

#[test]
fn test_range_single() {
    base_test().k(11).lookup_bits(3).bench_builder(
        [Fr::ZERO; 2],
        [100, 101].map(Fr::from),
        |pool, range, inputs| {
            range_tests(pool.main(), range, inputs, 8, 8);
        },
    );
}

#[test]
fn test_range_multicolumn() {
    let inputs = [100, 101].map(Fr::from);
    base_test().k(5).lookup_bits(3).run(|ctx, range| {
        range_tests(ctx, range, inputs, 8, 8);
    })
}

#[test]
fn test_multithread_range() {
    base_test().k(6).lookup_bits(3).unusable_rows(20).bench_builder(
        vec![[Fr::ZERO; 2]; 3],
        vec![[0, 1].map(Fr::from), [100, 101].map(Fr::from), [254, 255].map(Fr::from)],
        |pool, range, inputs| {
            parallelize_core(pool, inputs, |ctx, input| {
                range_tests(ctx, range, input, 8, 8);
            });
        },
    );
}
