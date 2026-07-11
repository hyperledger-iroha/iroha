use std::cmp::max;

use crate::ff::{Field, PrimeField};
use crate::gates::tests::{flex_gate, range, utils::*, Fr};
use crate::utils::{biguint_to_fe, bit_length, fe_to_biguint};
use crate::{QuantumCell, QuantumCell::Witness};

use num_bigint::{BigUint, RandBigInt, RandomBits};
use proptest::{collection::vec, prelude::*};
use rand::rngs::StdRng;
use rand::SeedableRng;

prop_compose! {
    pub fn rand_fr()(seed in any::<u64>()) -> Fr {
        let rng = StdRng::seed_from_u64(seed);
        Fr::random(rng)
    }
}

prop_compose! {
    pub fn rand_witness()(seed in any::<u64>()) -> QuantumCell<Fr> {
        let rng = StdRng::seed_from_u64(seed);
        Witness(Fr::random(rng))
    }
}

prop_compose! {
    pub fn sum_products_with_coeff_and_var_strat(max_length: usize)(val in vec((rand_fr(), rand_witness(), rand_witness()), 1..=max_length), witness in rand_witness()) -> (Vec<(Fr, QuantumCell<Fr>, QuantumCell<Fr>)>, QuantumCell<Fr>) {
        (val, witness)
    }
}

prop_compose! {
    pub fn rand_bin_witness()(val in prop::sample::select(vec![Fr::zero(), Fr::one()])) -> QuantumCell<Fr> {
        Witness(val)
    }
}

prop_compose! {
    pub fn rand_fr_range(bits: u64)(seed in any::<u64>()) -> Fr {
        let mut rng = StdRng::seed_from_u64(seed);
        let n = rng.sample(RandomBits::new(bits));
        biguint_to_fe(&n)
    }
}

prop_compose! {
    pub fn rand_witness_range(bits: u64)(x in rand_fr_range(bits)) -> QuantumCell<Fr> {
        Witness(x)
    }
}

prop_compose! {
    fn lookup_strat((k_lo, k_hi): (usize, usize), min_lookup_bits: usize)
        (k in k_lo..=k_hi)
        (k in Just(k), lookup_bits in min_lookup_bits..k)
    -> (usize, usize) {
        (k, lookup_bits)
    }
}
// k is in [k_lo, k_hi]
// lookup_bits is in [min_lookup_bits, k-1]
prop_compose! {
    fn range_check_strat((k_lo, k_hi): (usize, usize), min_lookup_bits: usize, max_range_bits: u64)
        ((k, lookup_bits) in lookup_strat((k_lo,k_hi), min_lookup_bits), range_bits in 2..=max_range_bits)
        (k in Just(k), lookup_bits in Just(lookup_bits), a in rand_fr_range(range_bits), range_bits in Just(range_bits))
    -> (usize, usize, Fr, usize) {
        (k, lookup_bits, a, range_bits as usize)
    }
}

prop_compose! {
    fn check_less_than_strat((k_lo, k_hi): (usize, usize), min_lookup_bits: usize, max_num_bits: usize)
        (num_bits in 2..max_num_bits, k in k_lo..=k_hi)
        (k in Just(k), num_bits in Just(num_bits), lookup_bits in min_lookup_bits..k, seed in any::<u64>())
    -> (usize, usize, Fr, Fr, usize) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut b = rng.sample(RandomBits::new(num_bits as u64));
        if b == BigUint::from(0u32) {
            b = BigUint::from(1u32)
        }
        let a = rng.gen_biguint_below(&b);
        let [a,b] = [a,b].map(|x| biguint_to_fe(&x));
        (k, lookup_bits, a, b, num_bits)
    }
}

prop_compose! {
    fn check_less_than_safe_strat((k_lo, k_hi): (usize, usize), min_lookup_bits: usize)
    (k in k_lo..=k_hi, b in any::<u64>())
    (lookup_bits in min_lookup_bits..k, k in Just(k), a in 0..b, b in Just(b))
    -> (usize, usize, u64, u64) {
        (k, lookup_bits, a, b)
    }
}

proptest! {
    // Flex Gate Positive Tests
    #[test]
    fn prop_test_add(input in vec(rand_witness(), 2)) {
        let ground_truth = add_ground_truth(input.as_slice());
        let result = flex_gate::test_add(input.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_sub(input in vec(rand_witness(), 2)) {
        let ground_truth = sub_ground_truth(input.as_slice());
        let result = flex_gate::test_sub(input.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_sub_mul(input in vec(rand_witness(), 3)) {
        let ground_truth = sub_mul_ground_truth(input.as_slice());
        let result = flex_gate::test_sub_mul(input.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_neg(input in rand_witness()) {
        let ground_truth = neg_ground_truth(input);
        let result = flex_gate::test_neg(input);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_mul(inputs in vec(rand_witness(), 2)) {
        let ground_truth = mul_ground_truth(inputs.as_slice());
        let result = flex_gate::test_mul(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_mul_add(inputs in vec(rand_witness(), 3)) {
        let ground_truth = mul_add_ground_truth(inputs.as_slice());
        let result = flex_gate::test_mul_add(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_mul_not(inputs in vec(rand_witness(), 2)) {
        let ground_truth = mul_not_ground_truth(inputs.as_slice());
        let result = flex_gate::test_mul_not(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_assert_bit(input in rand_fr()) {
        let ground_truth = input == Fr::one() || input == Fr::zero();
        flex_gate::test_assert_bit(input, ground_truth);
    }

    // Note: due to unwrap after inversion this test will fail if the denominator is zero so we want to test for that. Therefore we do not filter for zero values.
    #[test]
    fn prop_test_div_unsafe(inputs in vec(rand_witness().prop_filter("Input cannot be 0",|x| *x.value() != Fr::zero()), 2)) {
        let ground_truth = div_unsafe_ground_truth(inputs.as_slice());
        let result = flex_gate::test_div_unsafe(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_assert_is_const(input in rand_fr()) {
        flex_gate::test_assert_is_const(&[input; 2]);
    }

    #[test]
    fn prop_test_inner_product(inputs in (vec(rand_witness(), 0..=100), vec(rand_witness(), 0..=100)).prop_filter("Input vectors must have equal length", |(a, b)| a.len() == b.len())) {
        let a = inputs.0.iter().map(|x| *x.value()).collect::<Vec<_>>();
        let b = inputs.1.iter().map(|x| *x.value()).collect::<Vec<_>>();
        let ground_truth = inner_product_ground_truth(&a, &b);
        let result = flex_gate::test_inner_product(inputs);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_inner_product_left_last(inputs in (vec(rand_witness(), 1..=100), vec(rand_witness(), 1..=100)).prop_filter("Input vectors must have equal length", |(a, b)| a.len() == b.len())) {
        let a = inputs.0.iter().map(|x| *x.value()).collect::<Vec<_>>();
        let b = inputs.1.iter().map(|x| *x.value()).collect::<Vec<_>>();
        let ground_truth = inner_product_left_last_ground_truth(&a, &b);
        let result = flex_gate::test_inner_product_left_last(inputs);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_inner_product_with_sums(inputs in (vec(rand_witness(), 0..=10), vec(rand_witness(), 1..=100)).prop_filter("Input vectors must have equal length", |(a, b)| a.len() == b.len())) {
        let ground_truth = inner_product_with_sums_ground_truth(&inputs);
        let result = flex_gate::test_inner_product_with_sums(inputs);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_sum_products_with_coeff_and_var(input in sum_products_with_coeff_and_var_strat(100)) {
        let expected = sum_products_with_coeff_and_var_ground_truth(&input);
        let output = flex_gate::test_sum_products_with_coeff_and_var(input);
        prop_assert_eq!(expected, output);
    }

    #[test]
    fn prop_test_and(inputs in vec(rand_witness(), 2)) {
        let ground_truth = and_ground_truth(inputs.as_slice());
        let result = flex_gate::test_and(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_not(input in rand_witness()) {
        let ground_truth = not_ground_truth(&input);
        let result = flex_gate::test_not(input);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_select(vals in vec(rand_witness(), 2), sel in rand_bin_witness()) {
        let inputs = vec![vals[0], vals[1], sel];
        let ground_truth = select_ground_truth(inputs.as_slice());
        let result = flex_gate::test_select(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_or_and(inputs in vec(rand_witness(), 3)) {
        let ground_truth = or_and_ground_truth(inputs.as_slice());
        let result = flex_gate::test_or_and(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_idx_to_indicator(input in (rand_witness(), 1..=16_usize)) {
        let ground_truth = idx_to_indicator_ground_truth(input);
        let result = flex_gate::test_idx_to_indicator(input.0, input.1);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_select_by_indicator(inputs in (vec(rand_witness(), 1..=10), rand_witness())) {
        let ground_truth = select_by_indicator_ground_truth(&inputs);
        let result = flex_gate::test_select_by_indicator(inputs.0, inputs.1);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_select_from_idx(inputs in (vec(rand_witness(), 1..=10), rand_witness())) {
        let ground_truth = select_from_idx_ground_truth(&inputs);
        let result = flex_gate::test_select_from_idx(inputs.0, inputs.1);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_is_zero(x in rand_fr()) {
        let ground_truth = is_zero_ground_truth(x);
        let result = flex_gate::test_is_zero(x);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_is_equal(inputs in vec(rand_witness(), 2)) {
        let ground_truth = is_equal_ground_truth(inputs.as_slice());
        let result = flex_gate::test_is_equal(inputs.as_slice());
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_num_to_bits(num in any::<u64>()) {
        let mut tmp = num;
        let mut bits = vec![];
        if num == 0 {
            bits.push(0);
        }
        while tmp > 0 {
            bits.push(tmp & 1);
            tmp /= 2;
        }
        let result = flex_gate::test_num_to_bits(num as usize, bits.len());
        prop_assert_eq!(bits.into_iter().map(Fr::from).collect::<Vec<_>>(), result);
    }

    #[test]
    fn prop_test_pow_var(a in rand_fr(), num in any::<u64>()) {
        let native_res = a.pow_vartime([num]);
        let result = flex_gate::test_pow_var(a, BigUint::from(num), Fr::CAPACITY as usize);
        prop_assert_eq!(result, native_res);
    }

    /*
    #[test]
    fn prop_test_lagrange_eval(inputs in vec(rand_fr(), 3)) {
    }
    */

    // Range Check Property Tests

    #[test]
    fn prop_test_is_less_than(
        (k, lookup_bits)in lookup_strat((10,18),4),
        bits in 1..Fr::CAPACITY as usize,
        seed in any::<u64>()
    ) {
        // current is_less_than requires bits to not be too large
        prop_assume!((bits.div_ceil(lookup_bits) + 1) * lookup_bits <= Fr::CAPACITY as usize);
        let mut rng = StdRng::seed_from_u64(seed);
        let a = biguint_to_fe(&rng.sample(RandomBits::new(bits as u64)));
        let b = biguint_to_fe(&rng.sample(RandomBits::new(bits as u64)));
        let ground_truth = is_less_than_ground_truth((a, b));
        let result = range::test_is_less_than(k, lookup_bits, [Witness(a), Witness(b)], bits);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_is_less_than_safe(
        (k, lookup_bits) in lookup_strat((10,18),4),
        a in any::<u64>(),
        b in any::<u64>(),
    ) {
        prop_assume!(bit_length(a) <= bit_length(b));
        let a = Fr::from(a);
        let ground_truth = is_less_than_ground_truth((a, Fr::from(b)));
        let result = range::test_is_less_than_safe(k, lookup_bits, a, b);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_div_mod(
        a in rand_witness(),
        b in any::<u64>().prop_filter("Non-zero divisor", |x| *x != 0u64)
    ) {
        let ground_truth = div_mod_ground_truth((*a.value(), b));
        let num_bits = max(fe_to_biguint(a.value()).bits() as usize, bit_length(b));
        prop_assume!(num_bits <= Fr::CAPACITY as usize);
        let result = range::test_div_mod(a, b, num_bits);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_get_last_bit(bits in 1..Fr::CAPACITY as usize, pad_bits in 0..10usize, seed in any::<u64>()) {
        prop_assume!(bits + pad_bits <= Fr::CAPACITY as usize);
        let mut rng = StdRng::seed_from_u64(seed);
        let a = rng.sample(RandomBits::new(bits as u64));
        let a = biguint_to_fe(&a);
        let ground_truth = get_last_bit_ground_truth(a);
        let bits = bits + pad_bits;
        let result = range::test_get_last_bit(a, bits);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_div_mod_var(a in rand_fr(), b in any::<u64>()) {
        let ground_truth = div_mod_ground_truth((a, b));
        let a_num_bits = fe_to_biguint(&a).bits() as usize;
        let lookup_bits = 9;
        prop_assume!(a_num_bits.div_ceil(lookup_bits) * lookup_bits <= Fr::CAPACITY as usize);
        let b_num_bits= bit_length(b);
        let result = range::test_div_mod_var(Witness(a), Witness(Fr::from(b)), a_num_bits, b_num_bits);
        prop_assert_eq!(result, ground_truth);
    }

    #[test]
    fn prop_test_range_check((k, lookup_bits, a, range_bits) in range_check_strat((14,22),3,253)) {
        // current range check only works when range_bits isn't too big:
        prop_assume!(range_bits.div_ceil(lookup_bits) * lookup_bits <= Fr::CAPACITY as usize);
        range::test_range_check(k, lookup_bits, a, range_bits);
    }

    #[test]
    fn prop_test_check_less_than((k, lookup_bits, a, b, num_bits) in check_less_than_strat((10,18),8,253)) {
        prop_assume!(num_bits.div_ceil(lookup_bits) * lookup_bits <= Fr::CAPACITY as usize);
        range::test_check_less_than(k, lookup_bits, Witness(a), Witness(b), num_bits);
    }

    #[test]
    fn prop_test_check_less_than_safe((k, lookup_bits, a, b) in check_less_than_safe_strat((10,18),3)) {
        range::test_check_less_than_safe(k, lookup_bits, Fr::from(a), b);
    }

    #[test]
    fn prop_test_check_big_less_than_safe((k, lookup_bits, a, b, num_bits) in check_less_than_strat((18,22),8,253)) {
        let b = fe_to_biguint(&b);
        let range_bits = (b.bits() as usize).div_ceil(lookup_bits) * lookup_bits;
        prop_assume!(num_bits.div_ceil(lookup_bits) * lookup_bits <= Fr::CAPACITY as usize);
        prop_assume!(range_bits < Fr::CAPACITY as usize);
        range::test_check_big_less_than_safe(k, lookup_bits, a, b);
    }
}
