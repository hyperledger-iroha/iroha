//! Differential extension arithmetic for complete P-256 value execution and sorted registrations.

use super::fp4_air_tests::{challenges, opening, rows};
use super::*;

fn terminals() -> P256TerminalRegistrationV1 {
    let mut claims = super::fp4_air_tests::terminals();
    claims.cross_sources.push(P256CrossTraceTerminalClaimV1 {
        role: P256CrossTraceTerminalRoleV1::ValueWriter,
        start: [F(31); 4],
        terminal: [F(37); 4],
    });
    claims
}

#[test]
fn value_fp4_complete_registrations_match_independent_polynomial_lifting() {
    let challenges = challenges();
    let terminals = terminals();
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
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &terminals)
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
            let scalar = p256_opened_residues_v1(
                registration,
                opening(&lifted),
                &lifted[4],
                challenges,
                &terminals,
            )
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
                        .evaluate_residues_v1(
                            opening(&embedded),
                            &embedded[4],
                            challenges,
                            &terminals
                        )
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
fn value_fp4_binds_each_terminal_lane_and_rejects_malformed_inputs() {
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
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("value evaluator")
        };
        let (_, local) = p256_instance_parts_v1(registration.segment.instance).unwrap();
        let terminal_count = if local == 0 { 12 } else { 4 };
        let input = rows(registration);
        let original = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &terminals())
            .unwrap();
        for terminal in 0..terminal_count {
            let mut changed = terminals();
            let lane = terminal % 4;
            let claim = if local == 1 {
                &mut changed.buses.value_sorted[lane]
            } else {
                match terminal / 4 {
                    0 => &mut changed.buses.value_execution[lane],
                    1 => &mut changed.buses.value_arithmetic_copy[lane],
                    _ => &mut changed.cross_sources.last_mut().unwrap().terminal[lane],
                }
            };
            *claim = claim.add(F::ONE);
            let actual = evaluator
                .evaluate_residues_v1(opening(&input), &input[4], challenges, &changed)
                .unwrap();
            assert_eq!(
                &actual[..actual.len() - terminal_count],
                &original[..original.len() - terminal_count]
            );
            assert_ne!(
                actual[actual.len() - terminal_count + terminal],
                original[original.len() - terminal_count + terminal]
            );
        }
        for component in 0..5 {
            let mut malformed = input.clone();
            malformed[component].pop();
            assert!(
                evaluator
                    .evaluate_residues_v1(
                        opening(&malformed),
                        &malformed[4],
                        challenges,
                        &terminals()
                    )
                    .is_err()
            );
        }
        let mut malformed_claims = terminals();
        malformed_claims.buses.value_sorted[0] =
            F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert!(
            evaluator
                .evaluate_residues_v1(opening(&input), &input[4], challenges, &malformed_claims)
                .is_err()
        );
    }
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
                p256_opened_residues_v1(
                    registration,
                    opening(&lifted),
                    &lifted[4],
                    challenges,
                    &terminals(),
                )
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
