//! Polynomial lifting and terminal-binding checks for the complete arithmetic registration.

use super::*;
use crate::privacy_engines::{
    transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as E},
    zk_x509::{
        p256_air::P256_ARITHMETIC_STARK_CONSTRAINT_DEGREE_V1,
        p256_scalar_bit_bus::P256ScalarBitBusLaneChallengesV1,
    },
};

fn challenges() -> (P256ScalarBitBusChallengesV1, P256ArithmeticCopyChallengesV1) {
    (
        P256ScalarBitBusChallengesV1 {
            lanes: core::array::from_fn(|lane| P256ScalarBitBusLaneChallengesV1 {
                terms: core::array::from_fn(|term| F(2 + (lane * 5 + term) as u64)),
            }),
        },
        P256ArithmeticCopyChallengesV1 {
            lanes: core::array::from_fn(|lane| P256ArithmeticCopyLaneChallengesV1 {
                terms: core::array::from_fn(|term| F(31 + (lane * 3 + term) as u64)),
            }),
        },
    )
}

fn rows<A: PolynomialAirFieldV1>(mut cell: impl FnMut(usize) -> A) -> [Vec<A>; 5] {
    let mut index = 0;
    [
        P256_ARITHMETIC_BASE_WIDTH_V1,
        P256_ARITHMETIC_BASE_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
    ]
    .map(|width| {
        (0..width)
            .map(|_| {
                index += 1;
                cell(index)
            })
            .collect()
    })
}

fn evaluate<A: PolynomialAirFieldV1>(input: &[Vec<A>; 5]) -> Vec<A> {
    let aux = input[2].as_slice().try_into().unwrap();
    let fixed = input[4].as_slice().try_into().unwrap();
    let (scalar, arithmetic_copy) = challenges();
    let mut residues = evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
        input[0].as_slice().try_into().unwrap(),
        input[1].as_slice().try_into().unwrap(),
        aux,
        input[3].as_slice().try_into().unwrap(),
        fixed,
        scalar,
        arithmetic_copy,
    )
    .unwrap();
    let selector = p256_arithmetic_last_selector_v1(fixed);
    residues.extend(evaluate_p256_terminal_claim_binding_v1(
        selector,
        p256_arithmetic_scalar_terminal_v1(aux).unwrap(),
        [F(53), F(59), F(61), F(67)],
    ));
    residues.extend(evaluate_p256_terminal_claim_binding_v1(
        selector,
        p256_arithmetic_value_copy_terminal_v1(aux).unwrap(),
        [F(71), F(73), F(79), F(83)],
    ));
    residues
}

#[test]
fn complete_arithmetic_fp4_residues_match_independent_polynomial_lifting() {
    let input = rows(|index| {
        let i = index as u64;
        E::canonical([i + 1, i + 3, i + 5, i + 7]).unwrap()
    });
    let actual = evaluate(&input);
    assert_eq!(actual.len(), P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1);
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; actual.len()];
    // The degree-four AIR on cubic input polynomials has degree at most twelve.
    for sample in 0..13 {
        let t = F(sample);
        let lifted = input.each_ref().map(|row| {
            row.iter()
                .map(|value| {
                    value
                        .coefficients()
                        .iter()
                        .rev()
                        .fold(F::ZERO, |sum, &c| sum.mul(t).add(c))
                })
                .collect::<Vec<_>>()
        });
        let scalar = evaluate(&lifted);
        let (scalar_challenges, copy_challenges) = challenges();
        let native = evaluate_p256_arithmetic_aggregate_residues_v1(
            lifted[0].as_slice().try_into().unwrap(),
            lifted[1].as_slice().try_into().unwrap(),
            lifted[2].as_slice().try_into().unwrap(),
            lifted[3].as_slice().try_into().unwrap(),
            lifted[4].as_slice().try_into().unwrap(),
            scalar_challenges,
            copy_challenges,
        )
        .unwrap();
        assert_eq!(
            native,
            scalar[..P256_ARITHMETIC_AGGREGATE_CONSTRAINT_COUNT_V1]
        );
        let embedded = lifted
            .each_ref()
            .map(|row| row.iter().copied().map(E::from_base).collect());
        assert_eq!(
            evaluate(&embedded),
            scalar.iter().copied().map(E::from_base).collect::<Vec<_>>()
        );
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..13 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (sum, &value) in expected.iter_mut().zip(&scalar) {
            *sum = sum.add(weight.mul_base(value));
        }
    }
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .any(|value| value.coefficients()[1] != F::ZERO)
    );
}

#[test]
fn arithmetic_degree_four_includes_coefficient_modulus_and_boundary_selectors() {
    assert_eq!(P256_ARITHMETIC_STARK_CONSTRAINT_DEGREE_V1, 4);
    let mut samples = (0..7)
        .map(|sample| {
            let t = F(sample);
            evaluate(&rows(|index| {
                F(index as u64 + 1).add(t.mul(F(index as u64 + 3)))
            }))
        })
        .collect::<Vec<_>>();
    for order in 1..=5 {
        samples = samples
            .windows(2)
            .map(|pair| {
                pair[1]
                    .iter()
                    .zip(&pair[0])
                    .map(|(&right, &left)| right.sub(left))
                    .collect()
            })
            .collect();
        if order == 4 {
            assert!(samples.iter().flatten().any(|&value| value != F::ZERO));
        }
    }
    assert!(samples.iter().flatten().all(|&value| value == F::ZERO));
}

#[test]
fn arithmetic_fp4_binds_each_scalar_and_operand_copy_terminal() {
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut input = rows(|_| E::ZERO);
    input[4][ARITHMETIC_VALUE_COPY_BOUNDARY_FIXED + 1] = w;
    let honest = evaluate(&input);
    let offsets = [
        ARITHMETIC_SCALAR_AUX + compact_terminal_start_v1(8),
        ARITHMETIC_VALUE_COPY_AUX + compact_terminal_start_v1(3),
    ];
    for (family, start) in offsets.into_iter().enumerate() {
        for lane in 0..4 {
            let mut changed = input.clone();
            changed[2][start + lane] = w;
            let actual = evaluate(&changed);
            for terminal in 0..8 {
                let index = P256_ARITHMETIC_AGGREGATE_CONSTRAINT_COUNT_V1 + terminal;
                assert_eq!(
                    actual[index].sub(honest[index]),
                    if terminal == family * 4 + lane {
                        w.mul(w)
                    } else {
                        E::ZERO
                    }
                );
            }
        }
    }
}

#[test]
fn arithmetic_rejects_noncanonical_native_and_copy_columns_before_arithmetic() {
    let input = rows(|_| F::ZERO);
    let (scalar_challenges, copy_challenges) = challenges();
    for family in 0..5 {
        for index in [0, input[family].len() - 1] {
            let mut invalid = input.clone();
            invalid[family][index] = F(GOLDILOCKS_MODULUS_V1);
            assert!(
                evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
                    invalid[0].as_slice().try_into().unwrap(),
                    invalid[1].as_slice().try_into().unwrap(),
                    invalid[2].as_slice().try_into().unwrap(),
                    invalid[3].as_slice().try_into().unwrap(),
                    invalid[4].as_slice().try_into().unwrap(),
                    scalar_challenges,
                    copy_challenges,
                )
                .is_err()
            );
        }
    }
    let extension = rows(|_| E::ZERO);
    let mut invalid = copy_challenges;
    invalid.lanes[0].terms[0] = F::ZERO;
    assert!(
        evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
            extension[0].as_slice().try_into().unwrap(),
            extension[1].as_slice().try_into().unwrap(),
            extension[2].as_slice().try_into().unwrap(),
            extension[3].as_slice().try_into().unwrap(),
            extension[4].as_slice().try_into().unwrap(),
            scalar_challenges,
            invalid,
        )
        .is_err()
    );
}
