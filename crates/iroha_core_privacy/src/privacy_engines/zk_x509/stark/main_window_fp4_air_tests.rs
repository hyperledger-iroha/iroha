//! Differential extension arithmetic for complete P-256 window registrations.

use super::fp4_air_tests::{challenges, opening, rows};
use super::*;

#[test]
fn window_fp4_complete_registrations_match_independent_polynomial_lifting() {
    let challenges = challenges();
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256Window {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("window evaluator")
        };
        let input = rows(registration);
        let actual = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges)
            .unwrap();
        assert_eq!(actual.len(), registration.segment.constraint_count);
        let mut expected = vec![E::ZERO; actual.len()];
        for sample in 0..13 {
            let t = F(sample);
            let lifted = input.each_ref().map(|row| {
                row.iter()
                    .map(|value| {
                        value
                            .coefficients()
                            .iter()
                            .rev()
                            .fold(F::ZERO, |sum, &coefficient| sum.mul(t).add(coefficient))
                    })
                    .collect::<Vec<_>>()
            });
            let scalar =
                p256_opened_residues_v1(registration, opening(&lifted), &lifted[4], challenges)
                    .unwrap();
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
            if sample == 0 {
                let embedded = lifted
                    .each_ref()
                    .map(|row| row.iter().copied().map(E::from_base).collect());
                assert_eq!(
                    evaluator
                        .evaluate_residues_v1(opening(&embedded), &embedded[4], challenges)
                        .unwrap(),
                    scalar.into_iter().map(E::from_base).collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(actual, expected);
        assert!(
            actual
                .iter()
                .any(|value| value.coefficients()[1] != F::ZERO)
        );
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn window_fp4_binds_private_cross_and_scalar_columns_and_preserves_degree_four() {
    let registration = AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
        .into_iter()
        .find(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256Window)
        .unwrap();
    let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
        MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
    else {
        panic!("window evaluator")
    };
    let challenges = challenges();
    use super::super::super::p256_aggregate_adapter::{
        P256PrivateLinkFamilyV1 as Family, p256_private_link_columns_v1,
    };
    let input = rows(registration);
    let original = evaluator
        .evaluate_residues_v1(opening(&input), &input[4], challenges)
        .unwrap();
    let identity = p256_main_registration_from_main_layout_v1(registration).unwrap();
    for family in [Family::ChainTerminal, Family::WindowScalar] {
        for column in p256_private_link_columns_v1(identity, family).unwrap() {
            let mut changed = input.clone();
            changed[2][column] = changed[2][column].add(E::ONE);
            assert_ne!(
                evaluator
                    .evaluate_residues_v1(opening(&changed), &changed[4], challenges)
                    .unwrap(),
                original
            );
            changed[2][column] =
                E::from_raw_coefficients_for_testing([F(u64::MAX), F::ZERO, F::ZERO, F::ZERO]);
            assert!(
                evaluator
                    .evaluate_residues_v1(opening(&changed), &changed[4], challenges)
                    .is_err()
            );
        }
    }
    let mut malformed = input.clone();
    malformed[2].pop();
    assert!(
        evaluator
            .evaluate_residues_v1(opening(&malformed), &malformed[4], challenges)
            .is_err()
    );
    let mut samples = (0..6)
        .map(|sample| {
            let t = F(sample);
            let lifted = input.each_ref().map(|row| {
                row.iter()
                    .map(|value| value.coefficients()[0].add(t.mul(value.coefficients()[1])))
                    .collect::<Vec<_>>()
            });
            p256_opened_residues_v1(registration, opening(&lifted), &lifted[4], challenges).unwrap()
        })
        .collect::<Vec<_>>();
    for _ in 0..5 {
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
    }
    assert!(samples[0].iter().all(|&value| value == F::ZERO));
}
