//! Independent coefficient-oracle, canonicality and scheduling checks for FRI.

use super::*;
use fastpq_isi::FASTPQ_FINAL_V1;

fn root(length: usize) -> u64 {
    assert!(length.is_power_of_two());
    field_pow(
        FASTPQ_FINAL_V1.lde_root,
        1_u64 << (FASTPQ_FINAL_V1.lde_log_size - length.ilog2()),
    )
}

fn coefficients(length: usize) -> Vec<GoldilocksFp4V1> {
    (0..length)
        .map(|index| {
            let index = index as u64;
            GoldilocksFp4V1::new([
                7 + index * 13,
                11 + index * 17,
                19 + index * 23,
                29 + index * 31,
            ])
            .unwrap()
        })
        .collect()
}

fn evaluate(coefficients: &[GoldilocksFp4V1], point: u64) -> GoldilocksFp4V1 {
    coefficients
        .iter()
        .rev()
        .fold(GoldilocksFp4V1::ZERO, |value, &coefficient| {
            value.mul_base(point).add(coefficient)
        })
}

// This oracle starts from polynomial coefficients and never interpolates the
// fiber or uses a domain inverse, twiddle table, bit reversal or butterfly.
fn folded_coefficients(
    coefficients: &[GoldilocksFp4V1],
    arity: usize,
    challenge: GoldilocksFp4V1,
) -> Vec<GoldilocksFp4V1> {
    coefficients
        .chunks(arity)
        .map(|chunk| {
            let mut result = GoldilocksFp4V1::ZERO;
            let mut power = GoldilocksFp4V1::ONE;
            for &coefficient in chunk {
                result = result.add(coefficient.mul(power));
                power = power.mul(challenge);
            }
            result
        })
        .collect()
}

#[test]
fn every_arity_matches_independent_coefficients_with_all_challenge_coordinates() {
    for arity in [2, 4, 8, 16] {
        let generator = root(arity);
        let plan = FriFoldPlan::new(arity, generator).unwrap();
        for length in [1, arity - 1, arity, arity + 1, 4 * arity + 3] {
            let coefficients = coefficients(length);
            for x in [1, 7, 1_u64 << 32, GOLDILOCKS_MODULUS - 1] {
                let values: Vec<_> = (0..arity)
                    .map(|index| {
                        evaluate(
                            &coefficients,
                            mul_mod(x, field_pow(generator, index as u64)),
                        )
                    })
                    .collect();
                for challenge in [
                    GoldilocksFp4V1::ZERO,
                    GoldilocksFp4V1::ONE,
                    GoldilocksFp4V1::new([0, 1, 0, 0]).unwrap(),
                    GoldilocksFp4V1::new([0, 0, 1, 0]).unwrap(),
                    GoldilocksFp4V1::new([0, 0, 0, 1]).unwrap(),
                    GoldilocksFp4V1::new([GOLDILOCKS_MODULUS - 1, 37, 41, 43]).unwrap(),
                ] {
                    let expected = evaluate(
                        &folded_coefficients(&coefficients, arity, challenge),
                        field_pow(x, arity as u64),
                    );
                    assert_eq!(plan.fold_coset(&values, challenge, x).unwrap(), expected);
                }
            }
        }
    }
}

#[test]
fn binary_folding_preserves_the_existing_owner() {
    let plan = FriFoldPlan::new(2, GOLDILOCKS_MODULUS - 1).unwrap();
    let values = coefficients(2);
    for challenge in coefficients(8) {
        for x in [1, 7, 101, GOLDILOCKS_MODULUS - 1] {
            assert_eq!(
                plan.fold_coset(&values, challenge, x).unwrap(),
                crate::backend::fold_fri_coset(&values, challenge, x, GOLDILOCKS_MODULUS - 1,)
                    .unwrap()
            );
        }
    }
}

#[test]
fn root_order_and_fiber_shape_are_checked_before_arithmetic() {
    for arity in [0, 1, 3, 5, 32, usize::MAX] {
        assert!(FriFoldPlan::new(arity, 1).is_err());
    }
    for arity in [2, 4, 8, 16] {
        for generator in [0, 1, GOLDILOCKS_MODULUS, u64::MAX] {
            assert!(FriFoldPlan::new(arity, generator).is_err());
        }
        if arity > 2 {
            assert!(FriFoldPlan::new(arity, root(arity / 2)).is_err());
        }
        let plan = FriFoldPlan::new(arity, root(arity)).unwrap();
        for length in [0, arity - 1, arity + 1] {
            assert!(
                plan.fold_coset(&coefficients(length), GoldilocksFp4V1::ONE, 7)
                    .is_err()
            );
        }
        for x in [0, GOLDILOCKS_MODULUS, u64::MAX] {
            assert!(
                plan.fold_coset(&coefficients(arity), GoldilocksFp4V1::ONE, x)
                    .is_err()
            );
        }
    }
}

#[test]
fn each_malformed_field_coordinate_is_rejected() {
    let plan = FriFoldPlan::new(16, root(16)).unwrap();
    for lane in 0..4 {
        let mut words = [1; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(words);
        assert!(matches!(
            plan.fold_coset(&coefficients(16), bad, 7),
            Err(Error::NonCanonicalGoldilocksElement { context: "fri_fold_challenge", indices }) if indices == [lane]
        ));
        for position in 0..16 {
            let mut values = coefficients(16);
            values[position] = bad;
            assert!(matches!(
                plan.fold_coset(&values, GoldilocksFp4V1::ONE, 7),
                Err(Error::NonCanonicalGoldilocksElement { context: "fri_fold_value", indices }) if indices == [position, lane]
            ));
        }
    }
}
