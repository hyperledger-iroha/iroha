//! Polynomial lifting, degree and terminal-binding regressions for the scalar-bit bus.

use super::*;
use crate::privacy_engines::{
    transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as E},
    zk_x509::p256_aggregate_adapter::{
        evaluate_p256_scalar_bit_bus_aggregate_residues_v1,
        evaluate_p256_scalar_source_terminal_openings_v1,
    },
};

fn challenges() -> P256ScalarBitBusChallengesV1 {
    P256ScalarBitBusChallengesV1 {
        lanes: core::array::from_fn(|lane| P256ScalarBitBusLaneChallengesV1 {
            terms: core::array::from_fn(|term| F(2 + (lane * 5 + term) as u64)),
        }),
    }
}

fn rows<A: PolynomialAirFieldV1>(mut cell: impl FnMut(usize) -> A) -> [Vec<A>; 5] {
    let mut index = 0;
    [
        P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1,
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
    let mut residues = evaluate_p256_scalar_bit_bus_stark_residues_over_field_v1(
        input[0].as_slice().try_into().unwrap(),
        input[1].as_slice().try_into().unwrap(),
        aux,
        input[3].as_slice().try_into().unwrap(),
        fixed,
        challenges(),
    )
    .unwrap();
    residues.extend(evaluate_p256_scalar_source_terminal_openings_v1(
        p256_scalar_bit_bus_stark_last_active_selector_v1(fixed),
        [F(31), F(37), F(41), F(43)],
        [F(47), F(53), F(59), F(61)],
        p256_scalar_bit_bus_opened_terminals_v1(aux),
    ));
    residues
}

#[test]
fn complete_scalar_bus_fp4_residues_match_independent_polynomial_lifting() {
    let input = rows(|index| {
        let i = index as u64;
        E::canonical([i + 1, i + 3, i + 5, i + 7]).unwrap()
    });
    let actual = evaluate(&input);
    assert_eq!(actual.len(), 67 + 8);
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; actual.len()];
    // Cubic inputs and degree-three residues give degree at most nine.
    // Ten ordinary F samples determine each polynomial independently of Fp4 arithmetic.
    for sample in 0..10 {
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
        let native = evaluate_p256_scalar_bit_bus_stark_residues_v1(
            lifted[0].as_slice().try_into().unwrap(),
            lifted[1].as_slice().try_into().unwrap(),
            lifted[2].as_slice().try_into().unwrap(),
            lifted[3].as_slice().try_into().unwrap(),
            lifted[4].as_slice().try_into().unwrap(),
            challenges(),
        )
        .unwrap();
        assert_eq!(native, scalar[..67]);
        assert_eq!(
            evaluate_p256_scalar_bit_bus_aggregate_residues_v1(
                lifted[0].as_slice().try_into().unwrap(),
                lifted[1].as_slice().try_into().unwrap(),
                lifted[2].as_slice().try_into().unwrap(),
                lifted[3].as_slice().try_into().unwrap(),
                lifted[4].as_slice().try_into().unwrap(),
                challenges(),
            )
            .unwrap(),
            native
        );
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..10 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (sum, &value) in expected.iter_mut().zip(&scalar) {
            *sum = sum.add(weight.mul_base(value));
        }
        let embedded = lifted
            .each_ref()
            .map(|row| row.iter().copied().map(E::from_base).collect());
        assert_eq!(
            evaluate(&embedded),
            scalar.into_iter().map(E::from_base).collect::<Vec<_>>()
        );
    }
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .any(|value| value.coefficients()[1] != F::ZERO)
    );
}

#[test]
fn scalar_bus_degree_three_includes_all_fixed_selectors() {
    assert_eq!(P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_DEGREE_V1, 3);
    let mut samples = (0..6)
        .map(|sample| {
            let t = F(sample);
            evaluate(&rows(|index| {
                F(index as u64 + 1).add(t.mul(F(index as u64 + 3)))
            }))
        })
        .collect::<Vec<_>>();
    for order in 1..=4 {
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
        if order == 3 {
            assert!(samples.iter().flatten().any(|&value| value != F::ZERO));
        }
    }
    assert!(samples.iter().flatten().all(|&value| value == F::ZERO));
}

#[test]
fn scalar_bus_extension_terminal_mutations_bind_every_source_lane() {
    let selector = E::canonical([0, 1, 2, 3]).unwrap();
    let arithmetic = [F(31), F(37), F(41), F(43)];
    let window = [F(47), F(53), F(59), F(61)];
    let honest = [arithmetic.map(E::from_base), window.map(E::from_base)];
    assert_eq!(
        evaluate_p256_scalar_source_terminal_openings_v1(selector, arithmetic, window, honest),
        [E::ZERO; 8]
    );
    let delta = E::canonical([0, 0, 1, 0]).unwrap();
    for family in 0..2 {
        for lane in 0..4 {
            let mut bus = honest;
            bus[family][lane] = bus[family][lane].add(delta);
            let residues =
                evaluate_p256_scalar_source_terminal_openings_v1(selector, arithmetic, window, bus);
            for (index, &residue) in residues.iter().enumerate() {
                assert_eq!(
                    residue,
                    if index == family * 4 + lane {
                        E::ZERO.sub(selector.mul(delta))
                    } else {
                        E::ZERO
                    }
                );
            }
        }
    }
}

#[test]
fn scalar_bus_rejects_noncanonical_rows_and_degenerate_challenges() {
    let input = rows(|_| F::ZERO);
    for family in 0..5 {
        let mut invalid = input.clone();
        invalid[family][0] = F(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            evaluate_p256_scalar_bit_bus_stark_residues_over_field_v1(
                invalid[0].as_slice().try_into().unwrap(),
                invalid[1].as_slice().try_into().unwrap(),
                invalid[2].as_slice().try_into().unwrap(),
                invalid[3].as_slice().try_into().unwrap(),
                invalid[4].as_slice().try_into().unwrap(),
                challenges(),
            ),
            Err(P256ScalarBitBusErrorV1::Range)
        );
    }
    let extension = rows(|_| E::ZERO);
    for invalid_term in [
        F::ZERO,
        F(GOLDILOCKS_MODULUS_V1),
        challenges().lanes[1].terms[0],
    ] {
        let mut invalid = challenges();
        invalid.lanes[0].terms[0] = invalid_term;
        assert!(
            evaluate_p256_scalar_bit_bus_stark_residues_over_field_v1(
                extension[0].as_slice().try_into().unwrap(),
                extension[1].as_slice().try_into().unwrap(),
                extension[2].as_slice().try_into().unwrap(),
                extension[3].as_slice().try_into().unwrap(),
                extension[4].as_slice().try_into().unwrap(),
                invalid,
            )
            .is_err()
        );
    }
}
