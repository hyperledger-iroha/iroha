use super::*;
use crate::utils::biguint_to_fe;
use crate::utils::testing::base_test;
use crate::QuantumCell::Witness;
use crate::{gates::range::RangeInstructions, QuantumCell};
use num_bigint::BigUint;
use test_case::test_case;

#[test_case(16, 10, Fr::zero(), 0; "range_check() 0 bits")]
#[test_case(16, 10, Fr::from(100), 8; "range_check() pos")]
pub fn test_range_check(k: usize, lookup_bits: usize, a_val: Fr, range_bits: usize) {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        let a = ctx.load_witness(a_val);
        chip.range_check(ctx, a, range_bits);
    })
}

#[test_case(12, 10, Witness(Fr::zero()), Witness(Fr::one()), 64; "check_less_than() pos")]
pub fn test_check_less_than(
    k: usize,
    lookup_bits: usize,
    a: QuantumCell<Fr>,
    b: QuantumCell<Fr>,
    num_bits: usize,
) {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        chip.check_less_than(ctx, a, b, num_bits);
    })
}

#[test_case(10, 8, Fr::zero(), 1; "check_less_than_safe() pos")]
pub fn test_check_less_than_safe(k: usize, lookup_bits: usize, a: Fr, b: u64) {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        let a = ctx.load_witness(a);
        chip.check_less_than_safe(ctx, a, b);
    })
}

#[test_case(10, 8, biguint_to_fe(&BigUint::from(2u64).pow(239)), BigUint::from(2u64).pow(240) - 1usize; "check_big_less_than_safe() pos")]
pub fn test_check_big_less_than_safe(k: usize, lookup_bits: usize, a: Fr, b: BigUint) {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        let a = ctx.load_witness(a);
        chip.check_big_less_than_safe(ctx, a, b)
    })
}

#[test_case(10, 8, [6, 7].map(Fr::from).map(Witness), 3 => Fr::from(1); "is_less_than() pos")]
pub fn test_is_less_than(
    k: usize,
    lookup_bits: usize,
    inputs: [QuantumCell<Fr>; 2],
    bits: usize,
) -> Fr {
    base_test()
        .k(k as u32)
        .lookup_bits(lookup_bits)
        .run(|ctx, chip| *chip.is_less_than(ctx, inputs[0], inputs[1], bits).value())
}

#[test_case(10, 8, Fr::from(2), 3 => Fr::from(1); "is_less_than_safe() pos")]
pub fn test_is_less_than_safe(k: usize, lookup_bits: usize, a: Fr, b: u64) -> Fr {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        let a = ctx.load_witness(a);
        let lt = chip.is_less_than_safe(ctx, a, b);
        *lt.value()
    })
}

#[test_case(10, 8, biguint_to_fe(&BigUint::from(2u64).pow(239)), BigUint::from(2u64).pow(240) - 1usize => Fr::from(1); "is_big_less_than_safe() pos")]
pub fn test_is_big_less_than_safe(k: usize, lookup_bits: usize, a: Fr, b: BigUint) -> Fr {
    base_test().k(k as u32).lookup_bits(lookup_bits).run(|ctx, chip| {
        let a = ctx.load_witness(a);
        *chip.is_big_less_than_safe(ctx, a, b).value()
    })
}

#[test_case(Witness(Fr::from(3)), 2, 2 => (Fr::from(1), Fr::from(1)) ; "div_mod(3, 2)")]
pub fn test_div_mod(a: QuantumCell<Fr>, b: u64, num_bits: usize) -> (Fr, Fr) {
    base_test().run(|ctx, chip| {
        let a = chip.div_mod(ctx, a, b, num_bits);
        (*a.0.value(), *a.1.value())
    })
}

#[test_case(Fr::from(3), 8 => Fr::one() ; "get_last_bit(): 3, 8 bits")]
#[test_case(Fr::from(3), 2 => Fr::one() ; "get_last_bit(): 3, 2 bits")]
#[test_case(Fr::from(0), 2 => Fr::zero() ; "get_last_bit(): 0")]
#[test_case(Fr::from(1), 2 => Fr::one() ; "get_last_bit(): 1")]
#[test_case(Fr::from(2), 2 => Fr::zero() ; "get_last_bit(): 2")]
pub fn test_get_last_bit(a: Fr, bits: usize) -> Fr {
    base_test().run(|ctx, chip| {
        let a = ctx.load_witness(a);
        *chip.get_last_bit(ctx, a, bits).value()
    })
}

#[test_case(Witness(Fr::from(3)), Witness(Fr::from(2)), 3, 3 => (Fr::one(), Fr::one()); "div_mod_var(3 ,2)")]
pub fn test_div_mod_var(
    a: QuantumCell<Fr>,
    b: QuantumCell<Fr>,
    a_num_bits: usize,
    b_num_bits: usize,
) -> (Fr, Fr) {
    base_test().run(|ctx, chip| {
        let a = chip.div_mod_var(ctx, a, b, a_num_bits, b_num_bits);
        (*a.0.value(), *a.1.value())
    })
}
