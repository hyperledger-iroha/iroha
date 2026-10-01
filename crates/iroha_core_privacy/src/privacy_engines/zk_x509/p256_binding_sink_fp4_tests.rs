//! Independent polynomial and adversarial checks for the complete binding-sink relation.

use super::*;
use crate::privacy_engines::{
    transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as E},
    zk_x509::p256_cross_trace_bus::P256CrossTraceLaneChallengesV1,
};

fn challenges() -> P256CrossTraceChallengesV1 {
    P256CrossTraceChallengesV1 {
        lanes: core::array::from_fn(|lane| P256CrossTraceLaneChallengesV1 {
            terms: core::array::from_fn(|term| F(2 + (lane * 4 + term) as u64)),
        }),
    }
}

fn rows<A: PolynomialAirFieldV1>(mut cell: impl FnMut(usize) -> A) -> [Vec<A>; 5] {
    let mut index = 0;
    [
        P256_BINDING_SINK_BASE_WIDTH_V1,
        P256_BINDING_SINK_BASE_WIDTH_V1,
        P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
        P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
        P256_BINDING_SINK_FIXED_WIDTH_V1,
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

fn evaluate_registered<A: PolynomialAirFieldV1>(input: &[Vec<A>; 5]) -> Vec<A> {
    let aux = input[2].as_slice().try_into().unwrap();
    let fixed = input[4].as_slice().try_into().unwrap();
    evaluate_p256_binding_sink_aggregate_residues_over_field_v1(
        input[0].as_slice().try_into().unwrap(),
        input[1].as_slice().try_into().unwrap(),
        aux,
        input[3].as_slice().try_into().unwrap(),
        fixed,
        challenges(),
    )
    .unwrap()
}

fn evaluate<A: PolynomialAirFieldV1>(input: &[Vec<A>; 5]) -> Vec<A> {
    let aux = input[2].as_slice().try_into().unwrap();
    let fixed = input[4].as_slice().try_into().unwrap();
    let mut residues = evaluate_registered(input);
    residues.extend(evaluate_p256_terminal_claim_binding_v1(
        p256_binding_sink_last_selector_v1(fixed),
        p256_binding_sink_terminal_v1(aux).unwrap(),
        [F(31), F(37), F(41), F(43)],
    ));
    residues
}

#[test]
fn complete_binding_sink_fp4_residues_match_independent_polynomial_lifting() {
    let input = rows(|index| {
        let i = index as u64;
        E::canonical([i + 1, i + 3, i + 5, i + 7]).unwrap()
    });
    let actual = evaluate(&input);
    // MAIN registers only the local AIR; its private cross-registration links
    // are joined separately. Keep testing the independent diagnostic terminal
    // checks below without counting them as registered/public claim constraints.
    assert_eq!(
        evaluate_registered(&input).len(),
        P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1
    );
    assert_eq!(
        actual.len(),
        P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1 + P256_CROSS_TRACE_LANES_V1
    );
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; actual.len()];
    // Degree-three constraints on cubic inputs require ten independent samples.
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
        let native = evaluate_p256_binding_sink_aggregate_residues_v1(
            lifted[0].as_slice().try_into().unwrap(),
            lifted[1].as_slice().try_into().unwrap(),
            lifted[2].as_slice().try_into().unwrap(),
            lifted[3].as_slice().try_into().unwrap(),
            lifted[4].as_slice().try_into().unwrap(),
            challenges(),
        )
        .unwrap();
        assert_eq!(native, scalar[..P256_BINDING_SINK_CONSTRAINT_COUNT_V1]);
        let embedded = lifted
            .each_ref()
            .map(|row| row.iter().copied().map(E::from_base).collect());
        assert_eq!(
            evaluate(&embedded),
            scalar.iter().copied().map(E::from_base).collect::<Vec<_>>()
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
    }
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .any(|value| value.coefficients()[1] != F::ZERO)
    );
}

#[test]
fn binding_sink_degree_three_includes_optional_certificate_selection() {
    assert_eq!(P256_BINDING_SINK_CONSTRAINT_DEGREE_V1, 3);
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
fn binding_sink_extension_selection_and_each_terminal_remain_constrained() {
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut input = rows(|_| E::ZERO);
    input[0][SINK_SELECTION_ACTIVE_BASE] = w;
    input[0][SINK_SELECTION_REAL_BASE] = E::from_base(F(17));
    input[4][SINK_SELECTION_BYTE_FIXED] = w.mul(w);
    input[4][SINK_SELECTION_DUMMY_FIXED] = E::from_base(F(23));
    let offset = P256_BINDING_SINK_CONSTRAINT_COUNT_V1 - 41;
    let expected = E::ZERO.sub(
        w.mul(w)
            .mul(w.mul_base(F(17)).add(E::ONE.sub(w).mul_base(F(23)))),
    );
    assert_eq!(evaluate(&input)[offset + 3], expected);
    assert_ne!(expected, E::ZERO);
    input[4][SINK_BOUNDARY_FIXED + 1] = w;
    let honest = evaluate(&input);
    let terminal_offset = compact_terminal_start_v1(6);
    for lane in 0..4 {
        let mut changed = input.clone();
        changed[2][terminal_offset + lane] = w;
        let actual = evaluate(&changed);
        assert_eq!(
            actual[P256_BINDING_SINK_CONSTRAINT_COUNT_V1 + lane]
                .sub(honest[P256_BINDING_SINK_CONSTRAINT_COUNT_V1 + lane]),
            w.mul(w)
        );
    }
}

#[test]
fn binding_sink_rejects_noncanonical_selection_cells_and_challenges() {
    let input = rows(|_| F::ZERO);
    for family in 0..5 {
        let mut invalid = input.clone();
        let last = invalid[family].len() - 1;
        invalid[family][last] = F(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            evaluate_p256_binding_sink_aggregate_residues_over_field_v1(
                invalid[0].as_slice().try_into().unwrap(),
                invalid[1].as_slice().try_into().unwrap(),
                invalid[2].as_slice().try_into().unwrap(),
                invalid[3].as_slice().try_into().unwrap(),
                invalid[4].as_slice().try_into().unwrap(),
                challenges(),
            ),
            Err(P256AggregateAdapterErrorV1::Constraint)
        );
    }
    let extension = rows(|_| E::ZERO);
    let mut invalid = challenges();
    invalid.lanes[0].terms[0] = invalid.lanes[1].terms[0];
    assert_eq!(
        evaluate_p256_binding_sink_aggregate_residues_over_field_v1(
            extension[0].as_slice().try_into().unwrap(),
            extension[1].as_slice().try_into().unwrap(),
            extension[2].as_slice().try_into().unwrap(),
            extension[3].as_slice().try_into().unwrap(),
            extension[4].as_slice().try_into().unwrap(),
            invalid,
        ),
        Err(P256AggregateAdapterErrorV1::Challenge)
    );
}
