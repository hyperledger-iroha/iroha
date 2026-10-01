//! Differential extension arithmetic for complete P-256 value execution and sorted registrations.

use super::fp4_air_tests::{challenges, opening, rows};
use super::*;

#[test]
fn value_fp4_complete_registrations_match_independent_polynomial_lifting() {
    let challenges = challenges();
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || !matches!(
                p256_instance_parts_v1(registration.segment.instance),
                Some((_, 0 | 1))
            )
        {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("value evaluator")
        };
        let input = rows(registration);
        let actual = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges)
            .unwrap();
        assert_eq!(actual.len(), registration.segment.constraint_count);
        let mut expected = vec![E::ZERO; actual.len()];
        for sample in 0..10 {
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
    assert_eq!(count, 10);
}

#[test]
fn value_fp4_binds_each_private_terminal_column_and_rejects_malformed_inputs() {
    use super::super::super::p256_aggregate_adapter::{
        P256PrivateLinkFamilyV1 as Family, p256_private_link_columns_v1,
    };
    let challenges = challenges();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || !matches!(
                p256_instance_parts_v1(registration.segment.instance),
                Some((_, 0 | 1))
            )
        {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("value evaluator");
        };
        let identity = p256_main_registration_from_main_layout_v1(registration).unwrap();
        let families = if identity.local_instance_v1() == 0 {
            [Family::Value, Family::Copy, Family::ChainTerminal].as_slice()
        } else {
            [Family::Value].as_slice()
        };
        let input = rows(registration);
        let original = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges)
            .unwrap();
        for &family in families {
            for column in p256_private_link_columns_v1(identity, family).unwrap() {
                let mut changed = input.clone();
                changed[2][column] = changed[2][column].add(E::ONE);
                assert_ne!(
                    evaluator
                        .evaluate_residues_v1(opening(&changed), &changed[4], challenges)
                        .unwrap(),
                    original
                );
                changed[2][column] = E::noncanonical_fixture_v1();
                assert!(
                    evaluator
                        .evaluate_residues_v1(opening(&changed), &changed[4], challenges)
                        .is_err()
                );
            }
        }
        for component in 0..5 {
            let mut malformed = input.clone();
            malformed[component].pop();
            assert!(
                evaluator
                    .evaluate_residues_v1(opening(&malformed), &malformed[4], challenges)
                    .is_err()
            );
        }
        count += 1;
    }
    assert_eq!(count, 10);
}

#[test]
fn value_execution_and_sorted_total_degree_remains_three_including_fixed_columns() {
    let challenges = challenges();
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || !matches!(
                p256_instance_parts_v1(registration.segment.instance),
                Some((_, 0 | 1))
            )
        {
            continue;
        }
        let input = rows(registration);
        let mut samples = (0..5)
            .map(|sample| {
                let t = F(sample);
                let lifted = input.each_ref().map(|row| {
                    row.iter()
                        .map(|value| value.coefficients()[0].add(t.mul(value.coefficients()[1])))
                        .collect::<Vec<_>>()
                });
                p256_opened_residues_v1(registration, opening(&lifted), &lifted[4], challenges)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for _ in 0..3 {
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
        assert!(samples[0].iter().any(|&value| value != F::ZERO));
        assert_eq!(samples[0], samples[1]);
    }
}
