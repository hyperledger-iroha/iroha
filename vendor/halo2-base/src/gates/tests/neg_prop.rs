use crate::{
    ff::Field,
    gates::{
        range::RangeInstructions,
        tests::{pos_prop::rand_fr, utils},
        GateInstructions,
    },
    halo2_proofs::halo2curves::bn256::Fr,
    utils::{biguint_to_fe, bit_length, fe_to_biguint, testing::base_test, ScalarField},
    QuantumCell::Witness,
};

use num_bigint::BigUint;
use proptest::{collection::vec, prelude::*};
use rand::rngs::OsRng;

// Strategies for generating random witnesses
prop_compose! {
    // length == 1 is just selecting [0] which should be covered in unit test
    fn idx_to_indicator_strat(k_bounds: (usize, usize), max_size: usize)
        (k in k_bounds.0..=k_bounds.1, idx_val in prop::sample::select(vec![Fr::zero(), Fr::one(), Fr::random(OsRng)]), len in 2usize..=max_size)
        (k in Just(k), idx in 0..len, idx_val in Just(idx_val), len in Just(len), mut witness_vals in arb_indicator::<Fr>(len))
        -> (usize, usize, usize, Vec<Fr>) {
        witness_vals[idx] = idx_val;
        (k, len, idx, witness_vals)
    }
}

prop_compose! {
    fn select_strat(k_bounds: (usize, usize))
    (k in k_bounds.0..=k_bounds.1,  a in rand_fr(), b in rand_fr(), sel in any::<bool>(), rand_output in rand_fr())
    -> (usize, Fr, Fr, bool, Fr) {
        (k, a, b, sel, rand_output)
    }
}

prop_compose! {
    fn select_by_indicator_strat(k_bounds: (usize, usize), max_size: usize)
    (k in k_bounds.0..=k_bounds.1, len in 2usize..=max_size)
    (k in Just(k), a in vec(rand_fr(), len), idx in 0..len, rand_output in rand_fr())
    -> (usize, Vec<Fr>, usize, Fr) {
        (k, a, idx, rand_output)
    }
}

prop_compose! {
    fn select_from_idx_strat(k_bounds: (usize, usize), max_size: usize)
    (k in k_bounds.0..=k_bounds.1, len in 2usize..=max_size)
    (k in Just(k), cells in vec(rand_fr(), len), idx in 0..len, rand_output in rand_fr())
    -> (usize, Vec<Fr>, usize, Fr) {
        (k, cells, idx, rand_output)
    }
}

prop_compose! {
    fn inner_product_strat(k_bounds: (usize, usize), max_size: usize)
    (k in k_bounds.0..=k_bounds.1, len in 2usize..=max_size)
    (k in Just(k), a in vec(rand_fr(), len), b in vec(rand_fr(), len), rand_output in rand_fr())
    -> (usize, Vec<Fr>, Vec<Fr>, Fr) {
        (k, a, b, rand_output)
    }
}

prop_compose! {
    fn inner_product_left_last_strat(k_bounds: (usize, usize), max_size: usize)
    (k in k_bounds.0..=k_bounds.1, len in 2usize..=max_size)
    (k in Just(k), a in vec(rand_fr(), len), b in vec(rand_fr(), len), rand_output in (rand_fr(), rand_fr()))
    -> (usize, Vec<Fr>, Vec<Fr>, (Fr, Fr)) {
        (k, a, b, rand_output)
    }
}

prop_compose! {
    pub fn range_check_strat(k_bounds: (usize, usize), max_range_bits: usize)
    (k in k_bounds.0..=k_bounds.1, range_bits in 1usize..=max_range_bits) // lookup_bits must be less than k
    (k in Just(k), range_bits in Just(range_bits), lookup_bits in 8..k,
    rand_a in prop::sample::select(vec![
        biguint_to_fe(&(BigUint::from(2u64).pow(range_bits as u32) - 1usize)),
        biguint_to_fe(&BigUint::from(2u64).pow(range_bits as u32)),
        biguint_to_fe(&(BigUint::from(2u64).pow(range_bits as u32) + 1usize)),
        Fr::random(OsRng)
    ]))
    -> (usize, usize, usize, Fr) {
        (k, range_bits, lookup_bits, rand_a)
    }
}

prop_compose! {
    fn is_less_than_safe_strat(k_bounds: (usize, usize))
    // compose strat to generate random rand fr in range
    (b in any::<u64>().prop_filter("not zero", |&i| i != 0), k in k_bounds.0..=k_bounds.1)
    (k in Just(k), b in Just(b), lookup_bits in k_bounds.0 - 1..k, rand_a in rand_fr(), out in any::<bool>())
    -> (usize, u64, usize, Fr, bool) {
        (k, b, lookup_bits, rand_a, out)
    }
}

fn arb_indicator<F: ScalarField>(max_size: usize) -> impl Strategy<Value = Vec<F>> {
    vec(Just(0), max_size).prop_map(|val| val.iter().map(|&x| F::from(x)).collect::<Vec<_>>())
}

fn check_idx_to_indicator(idx: Fr, len: usize, ind_witnesses: &[Fr]) -> bool {
    // check that:
    // the length of the witnes array is correct
    // the sum of the witnesses is 1, indicting that there is only one index that is 1
    if ind_witnesses.len() != len
        || ind_witnesses.iter().fold(Fr::zero(), |acc, val| acc + *val) != Fr::one()
    {
        return false;
    }

    let idx_val = idx.get_lower_64() as usize;

    // Check that all indexes are zero except for the one at idx
    for (i, v) in ind_witnesses.iter().enumerate() {
        if i != idx_val && *v != Fr::zero() {
            return false;
        }
    }
    true
}

// verify rand_output == a if sel == 1, rand_output == b if sel == 0
fn check_select(a: Fr, b: Fr, sel: bool, rand_output: Fr) -> bool {
    if (!sel && rand_output != b) || (sel && rand_output != a) {
        return false;
    }
    true
}

fn neg_test_idx_to_indicator(k: usize, len: usize, idx: usize, ind_witnesses: &[Fr]) {
    // Check soundness of witness values
    let is_valid_witness = check_idx_to_indicator(Fr::from(idx as u64), len, ind_witnesses);
    base_test().k(k as u32).expect_satisfied(is_valid_witness).run_gate(|ctx, gate| {
        // assign value to advice column before by assigning `idx` via ctx.load() -> use same method as ind_offsets to get offset
        let dummy_idx = Witness(Fr::from(idx as u64));
        let mut indicator = gate.idx_to_indicator(ctx, dummy_idx, len);
        for (advice, prank_val) in indicator.iter_mut().zip(ind_witnesses) {
            advice.debug_prank(ctx, *prank_val);
        }
    });
}

fn neg_test_select(k: usize, a: Fr, b: Fr, sel: bool, prank_output: Fr) {
    // Check soundness of output
    let is_valid_instance = check_select(a, b, sel, prank_output);
    base_test().k(k as u32).expect_satisfied(is_valid_instance).run_gate(|ctx, gate| {
        let [a, b, sel] = [a, b, Fr::from(sel)].map(|x| ctx.load_witness(x));
        let select = gate.select(ctx, a, b, sel);
        select.debug_prank(ctx, prank_output);
    })
}

fn neg_test_select_by_indicator(k: usize, a: Vec<Fr>, idx: usize, prank_output: Fr) {
    // retrieve the value of a[idx] and check that it is equal to rand_output
    let is_valid_witness = prank_output == a[idx];
    base_test().k(k as u32).expect_satisfied(is_valid_witness).run_gate(|ctx, gate| {
        let indicator = gate.idx_to_indicator(ctx, Witness(Fr::from(idx as u64)), a.len());
        let a = ctx.assign_witnesses(a);
        let a_idx = gate.select_by_indicator(ctx, a, indicator);
        a_idx.debug_prank(ctx, prank_output);
    });
}

fn neg_test_select_from_idx(k: usize, cells: Vec<Fr>, idx: usize, prank_output: Fr) {
    // Check soundness of witness values
    let is_valid_witness = prank_output == cells[idx];
    base_test().k(k as u32).expect_satisfied(is_valid_witness).run_gate(|ctx, gate| {
        let cells = ctx.assign_witnesses(cells);
        let idx_val = gate.select_from_idx(ctx, cells, Witness(Fr::from(idx as u64)));
        idx_val.debug_prank(ctx, prank_output);
    });
}

fn neg_test_inner_product(k: usize, a: Vec<Fr>, b: Vec<Fr>, prank_output: Fr) {
    let is_valid_witness = prank_output == utils::inner_product_ground_truth(&a, &b);
    base_test().k(k as u32).expect_satisfied(is_valid_witness).run_gate(|ctx, gate| {
        let a = ctx.assign_witnesses(a);
        let inner_product = gate.inner_product(ctx, a, b.into_iter().map(Witness));
        inner_product.debug_prank(ctx, prank_output);
    });
}

fn neg_test_inner_product_left_last(
    k: usize,
    a: Vec<Fr>,
    b: Vec<Fr>,
    (prank_output, prank_a_last): (Fr, Fr),
) {
    let is_valid_witness = prank_output == utils::inner_product_ground_truth(&a, &b)
        && prank_a_last == *a.last().unwrap();
    base_test().k(k as u32).expect_satisfied(is_valid_witness).run_gate(|ctx, gate| {
        let a = ctx.assign_witnesses(a);
        let (inner_product, a_last) =
            gate.inner_product_left_last(ctx, a, b.into_iter().map(Witness));
        inner_product.debug_prank(ctx, prank_output);
        a_last.debug_prank(ctx, prank_a_last);
    });
}

// Range Check

fn neg_test_range_check(k: usize, range_bits: usize, lookup_bits: usize, rand_a: Fr) {
    let correct = fe_to_biguint(&rand_a).bits() <= range_bits as u64;
    base_test().k(k as u32).lookup_bits(lookup_bits).expect_satisfied(correct).run(|ctx, range| {
        let a_witness = ctx.load_witness(rand_a);
        range.range_check(ctx, a_witness, range_bits);
    })
}

// TODO: expand to prank output of is_less_than_safe()
fn neg_test_is_less_than_safe(k: usize, b: u64, lookup_bits: usize, rand_a: Fr, prank_out: bool) {
    let a_big = fe_to_biguint(&rand_a);
    let is_lt = a_big < BigUint::from(b);
    let correct = (is_lt == prank_out)
        && (a_big.bits() as usize <= bit_length(b).div_ceil(lookup_bits) * lookup_bits); // circuit should always fail if `a` doesn't pass range check

    base_test().k(k as u32).lookup_bits(lookup_bits).expect_satisfied(correct).run(|ctx, range| {
        let a_witness = ctx.load_witness(rand_a);
        let out = range.is_less_than_safe(ctx, a_witness, b);
        out.debug_prank(ctx, Fr::from(prank_out));
    });
}

proptest! {
    // Note setting the minimum value of k to 8 is intentional as it is the smallest value that will not cause an `out of columns` error. Should be noted that filtering by len * (number cells per iteration) < 2^k leads to the filtering of to many cases and the failure of the tests w/o any runs.
    #[test]
    fn prop_test_neg_idx_to_indicator((k, len, idx, witness_vals) in idx_to_indicator_strat((10,20),100)) {
        neg_test_idx_to_indicator(k, len, idx, witness_vals.as_slice());
    }

    #[test]
    fn prop_test_neg_select((k, a, b, sel, rand_output) in select_strat((10,20))) {
        neg_test_select(k, a, b, sel, rand_output);
    }

    #[test]
    fn prop_test_neg_select_by_indicator((k, a, idx, rand_output) in select_by_indicator_strat((12,20),100)) {
        neg_test_select_by_indicator(k, a, idx, rand_output);
    }

    #[test]
    fn prop_test_neg_select_from_idx((k, cells, idx, rand_output) in select_from_idx_strat((10,20),100)) {
        neg_test_select_from_idx(k, cells, idx, rand_output);
    }

    #[test]
    fn prop_test_neg_inner_product((k, a, b, rand_output) in inner_product_strat((10,20),100)) {
        neg_test_inner_product(k, a, b, rand_output);
    }

    #[test]
    fn prop_test_neg_inner_product_left_last((k, a, b, rand_output) in inner_product_left_last_strat((10,20),100)) {
        neg_test_inner_product_left_last(k, a, b, rand_output);
    }

    #[test]
    fn prop_test_neg_range_check((k, range_bits, lookup_bits, rand_a) in range_check_strat((10,23),90)) {
        neg_test_range_check(k, range_bits, lookup_bits, rand_a);
    }

    #[test]
    fn prop_test_neg_is_less_than_safe((k, b, lookup_bits, rand_a, out) in is_less_than_safe_strat((10,20))) {
        neg_test_is_less_than_safe(k, b, lookup_bits, rand_a, out);
    }
}
