#![allow(clippy::type_complexity)]
use super::*;
use crate::utils::biguint_to_fe;
use crate::utils::testing::base_test;
use crate::QuantumCell::{Constant, Witness};
use crate::{gates::flex_gate::GateInstructions, QuantumCell};
use itertools::Itertools;
use num_bigint::BigUint;
use test_case::test_case;

#[test_case(&[10, 12].map(Fr::from).map(Witness)=> Fr::from(22); "add(): 10 + 12 == 22")]
#[test_case(&[1, 1].map(Fr::from).map(Witness)=> Fr::from(2); "add(): 1 + 1 == 2")]
pub fn test_add(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.add(ctx, inputs[0], inputs[1]).value())
}

#[test_case(Witness(Fr::from(10))=> Fr::from(11); "inc(): 10 -> 11")]
#[test_case(Witness(Fr::from(1))=> Fr::from(2); "inc(): 1 -> 2")]
pub fn test_inc(input: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.inc(ctx, input).value())
}

#[test_case(&[10, 12].map(Fr::from).map(Witness)=> -Fr::from(2) ; "sub(): 10 - 12 == -2")]
#[test_case(&[1, 1].map(Fr::from).map(Witness)=> Fr::from(0) ; "sub(): 1 - 1 == 0")]
pub fn test_sub(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.sub(ctx, inputs[0], inputs[1]).value())
}

#[test_case(Witness(Fr::from(10))=> Fr::from(9); "dec(): 10 -> 9")]
#[test_case(Witness(Fr::from(1))=> Fr::from(0); "dec(): 1 -> 0")]
pub fn test_dec(input: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.dec(ctx, input).value())
}

#[test_case(&[1, 1, 1].map(Fr::from).map(Witness) => Fr::from(0) ; "sub_mul(): 1 - 1 * 1 == 0")]
pub fn test_sub_mul(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.sub_mul(ctx, inputs[0], inputs[1], inputs[2]).value())
}

#[test_case(Witness(Fr::from(1)) => -Fr::from(1) ; "neg(): 1 -> -1")]
pub fn test_neg(a: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.neg(ctx, a).value())
}

#[test_case(&[10, 12].map(Fr::from).map(Witness) => Fr::from(120) ; "mul(): 10 * 12 == 120")]
#[test_case(&[1, 1].map(Fr::from).map(Witness) => Fr::from(1) ; "mul(): 1 * 1 == 1")]
pub fn test_mul(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.mul(ctx, inputs[0], inputs[1]).value())
}

#[test_case(&[1, 1, 1].map(Fr::from).map(Witness) => Fr::from(2) ; "mul_add(): 1 * 1 + 1 == 2")]
pub fn test_mul_add(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.mul_add(ctx, inputs[0], inputs[1], inputs[2]).value())
}

#[test_case(&[0, 10].map(Fr::from).map(Witness) => Fr::from(10); "mul_not(): (1 - 0) * 10 == 10")]
#[test_case(&[1, 10].map(Fr::from).map(Witness) => Fr::from(0); "mul_not(): (1 - 1) * 10 == 0")]
pub fn test_mul_not(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.mul_not(ctx, inputs[0], inputs[1]).value())
}

#[test_case(Fr::from(0), true; "assert_bit(0)")]
#[test_case(Fr::from(1), true; "assert_bit(1)")]
#[test_case(Fr::from(2), false; "assert_bit(2)")]
pub fn test_assert_bit(input: Fr, is_bit: bool) {
    base_test().expect_satisfied(is_bit).run_gate(|ctx, chip| {
        let a = ctx.load_witness(input);
        chip.assert_bit(ctx, a);
    });
}

#[test_case(&[6, 2].map(Fr::from).map(Witness)=> Fr::from(3) ; "div_unsafe(): 6 / 2 == 3")]
#[test_case(&[1, 1].map(Fr::from).map(Witness)=> Fr::from(1) ; "div_unsafe(): 1 / 1 == 1")]
pub fn test_div_unsafe(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.div_unsafe(ctx, inputs[0], inputs[1]).value())
}

#[test_case(&[1, 1].map(Fr::from); "assert_is_const(1,1)")]
#[test_case(&[0, 1].map(Fr::from); "assert_is_const(0,1)")]
pub fn test_assert_is_const(inputs: &[Fr]) {
    base_test().expect_satisfied(inputs[0] == inputs[1]).run_gate(|ctx, chip| {
        let a = ctx.load_witness(inputs[0]);
        chip.assert_is_const(ctx, &a, &inputs[1]);
    });
}

#[test_case((vec![Witness(Fr::one()); 5], vec![Witness(Fr::one()); 5]) => Fr::from(5) ; "inner_product(): 1 * 1 + ... + 1 * 1 == 5")]
pub fn test_inner_product(input: (Vec<QuantumCell<Fr>>, Vec<QuantumCell<Fr>>)) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.inner_product(ctx, input.0, input.1).value())
}

#[test_case((vec![Witness(Fr::one()); 5], vec![Witness(Fr::one()); 5]) => (Fr::from(5), Fr::from(1)); "inner_product_left_last(): 1 * 1 + ... + 1 * 1 == (5, 1)")]
pub fn test_inner_product_left_last(
    input: (Vec<QuantumCell<Fr>>, Vec<QuantumCell<Fr>>),
) -> (Fr, Fr) {
    base_test().run_gate(|ctx, chip| {
        let a = chip.inner_product_left_last(ctx, input.0, input.1);
        (*a.0.value(), *a.1.value())
    })
}

#[test_case([4,5,6].map(Fr::from).to_vec(), [1,2,3].map(|x| Constant(Fr::from(x))).to_vec() => (Fr::from(32), [4,5,6].map(Fr::from).to_vec());
"inner_product_left(): <[4,5,6],[1,2,3]> Constant b starts with 1")]
#[test_case([1,2,3].map(Fr::from).to_vec(), [4,5,6].map(|x| Witness(Fr::from(x))).to_vec() => (Fr::from(32), [1,2,3].map(Fr::from).to_vec());
"inner_product_left(): <[1,2,3],[4,5,6]> Witness")]
pub fn test_inner_product_left(a: Vec<Fr>, b: Vec<QuantumCell<Fr>>) -> (Fr, Vec<Fr>) {
    base_test().run_gate(|ctx, chip| {
        let (prod, a) = chip.inner_product_left(ctx, a.into_iter().map(Witness), b);
        (*prod.value(), a.iter().map(|v| *v.value()).collect())
    })
}

#[test_case((vec![Witness(Fr::one()); 5], vec![Witness(Fr::one()); 5]) => (1..=5).map(Fr::from).collect::<Vec<_>>(); "inner_product_with_sums(): 1 * 1 + ... + 1 * 1 == [1, 2, 3, 4, 5]")]
pub fn test_inner_product_with_sums(
    input: (Vec<QuantumCell<Fr>>, Vec<QuantumCell<Fr>>),
) -> Vec<Fr> {
    base_test().run_gate(|ctx, chip| {
        chip.inner_product_with_sums(ctx, input.0, input.1).map(|a| *a.value()).collect()
    })
}

#[test_case((vec![(Fr::from(1), Witness(Fr::from(1)), Witness(Fr::from(1)))], Witness(Fr::from(1))) => Fr::from(2) ; "sum_product_with_coeff_and_var(): 1 * 1 + 1 == 2")]
pub fn test_sum_products_with_coeff_and_var(
    input: (Vec<(Fr, QuantumCell<Fr>, QuantumCell<Fr>)>, QuantumCell<Fr>),
) -> Fr {
    base_test()
        .run_gate(|ctx, chip| *chip.sum_products_with_coeff_and_var(ctx, input.0, input.1).value())
}

#[test_case(&[1, 0].map(Fr::from).map(Witness) => Fr::from(0) ; "and(): 1 && 0 == 0")]
#[test_case(&[1, 1].map(Fr::from).map(Witness) => Fr::from(1) ; "and(): 1 && 1 == 1")]
pub fn test_and(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.and(ctx, inputs[0], inputs[1]).value())
}

#[test_case(Witness(Fr::from(1)) => Fr::zero(); "not(): !1 == 0")]
#[test_case(Witness(Fr::from(0)) => Fr::one(); "not(): !0 == 1")]
pub fn test_not(a: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.not(ctx, a).value())
}

#[test_case(&[2, 3, 1].map(Fr::from).map(Witness) => Fr::from(2); "select(): 2 ? 3 : 1 == 2")]
pub fn test_select(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.select(ctx, inputs[0], inputs[1], inputs[2]).value())
}

#[test_case(&[0, 1, 0].map(Fr::from).map(Witness) => Fr::from(0); "or_and(): 0 || (1 && 0) == 0")]
#[test_case(&[1, 0, 1].map(Fr::from).map(Witness) => Fr::from(1); "or_and(): 1 || (0 && 1) == 1")]
#[test_case(&[1, 1, 1].map(Fr::from).map(Witness) => Fr::from(1); "or_and(): 1 || (1 && 1) == 1")]
pub fn test_or_and(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.or_and(ctx, inputs[0], inputs[1], inputs[2]).value())
}

#[test_case(&[0,1] => [0,0,1,0].map(Fr::from).to_vec(); "bits_to_indicator(): bin\"10 -> [0, 0, 1, 0]")]
#[test_case(&[0] => [1,0].map(Fr::from).to_vec(); "bits_to_indicator(): 0 -> [1, 0]")]
pub fn test_bits_to_indicator(bits: &[u8]) -> Vec<Fr> {
    base_test().run_gate(|ctx, chip| {
        let a = ctx.assign_witnesses(bits.iter().map(|x| Fr::from(*x as u64)));
        chip.bits_to_indicator(ctx, &a).iter().map(|a| *a.value()).collect()
    })
}

#[test_case(Witness(Fr::from(0)),3 => [1,0,0].map(Fr::from).to_vec(); "idx_to_indicator(): 0 -> [1, 0, 0]")]
pub fn test_idx_to_indicator(idx: QuantumCell<Fr>, len: usize) -> Vec<Fr> {
    base_test().run_gate(|ctx, chip| {
        chip.idx_to_indicator(ctx, idx, len).iter().map(|a| *a.value()).collect()
    })
}

#[test_case((0..3).map(Fr::from).map(Witness).collect(), Witness(Fr::one()) => Fr::from(1); "select_by_indicator(1): [0, 1, 2] -> 1")]
pub fn test_select_by_indicator(array: Vec<QuantumCell<Fr>>, idx: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| {
        let a = chip.idx_to_indicator(ctx, idx, array.len());
        *chip.select_by_indicator(ctx, array, a).value()
    })
}

#[test_case((0..3).map(Fr::from).map(Witness).collect(), Witness(Fr::from(1)) => Fr::from(1); "select_from_idx(): [0, 1, 2] -> 1")]
pub fn test_select_from_idx(array: Vec<QuantumCell<Fr>>, idx: QuantumCell<Fr>) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.select_from_idx(ctx, array, idx).value())
}

#[test_case(vec![vec![1,2,3], vec![4,5,6], vec![7,8,9]].into_iter().map(|a| a.into_iter().map(Fr::from).collect_vec()).collect_vec(),
Fr::from(1) =>
[4,5,6].map(Fr::from).to_vec();
"select_array_by_indicator(1): [[1,2,3], [4,5,6], [7,8,9]] -> [4,5,6]")]
pub fn test_select_array_by_indicator(array2d: Vec<Vec<Fr>>, idx: Fr) -> Vec<Fr> {
    base_test().run_gate(|ctx, chip| {
        let array2d = array2d.into_iter().map(|a| ctx.assign_witnesses(a)).collect_vec();
        let idx = ctx.load_witness(idx);
        let ind = chip.idx_to_indicator(ctx, idx, array2d.len());
        chip.select_array_by_indicator(ctx, &array2d, &ind).iter().map(|a| *a.value()).collect()
    })
}

#[test_case(Fr::zero() => Fr::from(1); "is_zero(): 0 -> 1")]
pub fn test_is_zero(input: Fr) -> Fr {
    base_test().run_gate(|ctx, chip| {
        let input = ctx.load_witness(input);
        *chip.is_zero(ctx, input).value()
    })
}

#[test_case(&[1, 1].map(Fr::from).map(Witness) => Fr::one(); "is_equal(): 1 == 1")]
pub fn test_is_equal(inputs: &[QuantumCell<Fr>]) -> Fr {
    base_test().run_gate(|ctx, chip| *chip.is_equal(ctx, inputs[0], inputs[1]).value())
}

#[test_case(6, 3 => [0,1,1].map(Fr::from).to_vec(); "num_to_bits(): 6")]
pub fn test_num_to_bits(num: usize, bits: usize) -> Vec<Fr> {
    base_test().run_gate(|ctx, chip| {
        let num = ctx.load_witness(Fr::from(num as u64));
        chip.num_to_bits(ctx, num, bits).iter().map(|a| *a.value()).collect()
    })
}

#[test_case(Fr::from(3), BigUint::from(3u32), 4 => Fr::from(27); "pow_var(): 3^3 = 27")]
pub fn test_pow_var(a: Fr, exp: BigUint, max_bits: usize) -> Fr {
    assert!(exp.bits() <= max_bits as u64);
    base_test().run_gate(|ctx, chip| {
        let a = ctx.load_witness(a);
        let exp = ctx.load_witness(biguint_to_fe(&exp));
        *chip.pow_var(ctx, a, exp, max_bits).value()
    })
}
