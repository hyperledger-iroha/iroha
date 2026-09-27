//! Differential checks for the complete native-log5 comparison AIR subgroup.

use super::*;
use crate::privacy_engines::zk_x509::p256_aggregate_adapter as adapter;

pub(super) fn challenges() -> P256AggregateChallengesV1 {
    let mut transcript = TransparentTranscriptV1::new(
        ZK_X509_DIGEST_CONTEXT_V1,
        b"main-comparison-fp4-test",
        &PrivacyOuterDigestV1::default(),
        &PrivacyOuterDigestV1::default(),
    )
    .unwrap();
    derive_p256_aggregate_challenges_v1(&mut transcript).unwrap()
}

pub(super) fn terminals() -> P256TerminalRegistrationV1 {
    P256TerminalRegistrationV1 {
        buses: P256BusTerminalClaimsV1 {
            value_execution: [F::ONE; 4],
            value_sorted: [F::ONE; 4],
            value_arithmetic_copy: [F::ONE; 4],
            arithmetic_value_copy: [F::ONE; 4],
            arithmetic_scalar: [F::ONE; 4],
            window_scalar: [F::ONE; 4],
            scalar_bus_arithmetic: [F::ONE; 4],
            scalar_bus_window: [F::ONE; 4],
        },
        cross_sources: [
            P256CrossTraceTerminalRoleV1::DigestReduction,
            P256CrossTraceTerminalRoleV1::ResultXReduction,
            P256CrossTraceTerminalRoleV1::WalletLowS,
        ]
        .into_iter()
        .map(|role| P256CrossTraceTerminalClaimV1 {
            role,
            start: [F(23); 4],
            terminal: [F(29); 4],
        })
        .collect(),
        sink: [F::ONE; 4],
    }
}

pub(super) fn rows(registration: RegisteredSegmentLayoutV1) -> [Vec<E>; 5] {
    let mut index = 1;
    [
        registration.segment.base_width,
        registration.segment.base_width,
        registration.segment.aux_width,
        registration.segment.aux_width,
        registration.segment.fixed_width,
    ]
    .map(|width| {
        (0..width)
            .map(|_| {
                index += 1;
                E::canonical([index, index + 3, index + 5, index + 7]).unwrap()
            })
            .collect()
    })
}

pub(super) fn opening<A>(rows: &[Vec<A>; 5]) -> RegisteredOpenedRowsV1<'_, A> {
    RegisteredOpenedRowsV1 {
        base_current: &rows[0],
        base_next: &rows[1],
        aux_current: &rows[2],
        aux_next: &rows[3],
    }
}

#[test]
fn main_fp4_availability_tracks_each_complete_registration() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    assert_eq!(layout.registered_segments.len(), 49);
    let mut counts = [0; 12];
    for &registration in &layout.registered_segments {
        let evaluator = MainFp4AirEvaluatorV1::for_registration_v1(registration)
            .unwrap()
            .expect("every MAIN registration has a complete field-generic AIR");
        assert_eq!(evaluator.registration_v1(), registration);
        let family = match registration.segment.adapter {
            SegmentAdapterIdV1::P256Reduction => 0,
            SegmentAdapterIdV1::P256LowS => 1,
            SegmentAdapterIdV1::P256ScalarBitBus => 2,
            SegmentAdapterIdV1::P256Window => 3,
            SegmentAdapterIdV1::P256Arithmetic => 4,
            SegmentAdapterIdV1::P256ValueBus => {
                5 + usize::from(
                    p256_instance_parts_v1(registration.segment.instance)
                        .unwrap()
                        .1,
                )
            }
            SegmentAdapterIdV1::Sha256CallBus => 8,
            SegmentAdapterIdV1::Projection => 9,
            SegmentAdapterIdV1::ByteMemory => 10,
            SegmentAdapterIdV1::StrictDer | SegmentAdapterIdV1::Rfc5280 => 11,
            _ => panic!("non-MAIN AIR"),
        };
        counts[family] += 1;
        let mut malformed = registration;
        malformed.segment.base_width += 1;
        assert!(MainFp4AirEvaluatorV1::for_registration_v1(malformed).is_err());
    }
    assert_eq!(counts, [10, 1, 5, 5, 5, 5, 5, 5, 4, 1, 1, 2]);
    // Field-generic AIR coverage does not authenticate OODS openings or permit
    // omission of any existing queried relation. The profile remains gated.
    assert_eq!(counts.iter().sum::<usize>(), 49);
}

#[test]
fn projection_fp4_capability_evaluates_its_complete_registration_and_rejects_wrong_shape() {
    let registration = AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
        .into_iter()
        .find(|registration| registration.segment.adapter == SegmentAdapterIdV1::Projection)
        .unwrap();
    let Some(MainFp4AirEvaluatorV1::Projection(evaluator)) =
        MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
    else {
        panic!("projection evaluator")
    };
    let mut transcript = TransparentTranscriptV1::new(
        ZK_X509_DIGEST_CONTEXT_V1,
        b"projection-main-fp4-test",
        &PrivacyOuterDigestV1::default(),
        &PrivacyOuterDigestV1::default(),
    )
    .unwrap();
    let challenges = derive_projection_challenges_v1(&mut transcript).unwrap();
    let mut rows = rows(registration);
    assert_eq!(
        evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges)
            .unwrap()
            .len(),
        registration.segment.constraint_count
    );
    rows[0].pop();
    assert!(
        evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges)
            .is_err()
    );
}

#[test]
fn scalar_bus_fp4_capability_binds_all_five_registrations_and_source_claims() {
    let challenges = challenges();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ScalarBitBus {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("scalar evaluator")
        };
        let mut rows = rows(registration);
        let mut claims = terminals();
        let original = evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &claims)
            .unwrap();
        assert_eq!(original.len(), 75);
        claims.buses.arithmetic_scalar[0] = claims.buses.arithmetic_scalar[0].add(F::ONE);
        assert_ne!(
            evaluator
                .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &claims)
                .unwrap(),
            original
        );
        rows[0].pop();
        assert!(
            evaluator
                .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &claims)
                .is_err()
        );
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn byte_memory_fp4_capability_uses_its_registered_shape_and_logical_extent() {
    let registration = AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
        .into_iter()
        .find(|registration| registration.segment.adapter == SegmentAdapterIdV1::ByteMemory)
        .unwrap();
    let Some(MainFp4AirEvaluatorV1::ByteMemory(evaluator)) =
        MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
    else {
        panic!("byte-memory evaluator")
    };
    let input = rows(registration);
    let challenges = ZkX509IoChallengesV1 {
        lanes: core::array::from_fn(
            |lane| super::super::super::io_air::ZkX509IoLaneChallengesV1 {
                beta: F(13 * lane as u64 + 2),
                channel: F(13 * lane as u64 + 3),
                offset: F(13 * lane as u64 + 5),
                value: F(13 * lane as u64 + 7),
                is_write: F(13 * lane as u64 + 11),
            },
        ),
    };
    assert_eq!(
        evaluator
            .evaluate_residues_v1(17, opening(&input), &input[4], challenges)
            .unwrap()
            .len(),
        registration.segment.constraint_count
    );
    assert!(
        evaluator
            .evaluate_residues_v1(usize::MAX, opening(&input), &input[4], challenges)
            .is_err()
    );
    let mut malformed = input;
    malformed[1].pop();
    assert!(
        evaluator
            .evaluate_residues_v1(17, opening(&malformed), &malformed[4], challenges)
            .is_err()
    );
}

#[test]
fn binding_sink_fp4_capability_binds_each_source_context() {
    let challenges = challenges();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || !matches!(
                p256_instance_parts_v1(registration.segment.instance),
                Some((_, 2))
            )
        {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("binding sink evaluator")
        };
        let input = rows(registration);
        let mut claims = terminals();
        let original = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &claims)
            .unwrap();
        assert_eq!(original.len(), registration.segment.constraint_count);
        claims.sink[0] = claims.sink[0].add(F::ONE);
        let changed = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &claims)
            .unwrap();
        assert_eq!(
            &changed[..changed.len() - 4],
            &original[..original.len() - 4]
        );
        assert_ne!(
            &changed[changed.len() - 4..],
            &original[original.len() - 4..]
        );
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn main_comparison_fp4_residues_match_independent_base_polynomial_lifting() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let challenges = challenges();
    let terminals = terminals();
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    for registration in layout.registered_segments {
        if !matches!(
            registration.segment.adapter,
            SegmentAdapterIdV1::P256Reduction | SegmentAdapterIdV1::P256LowS
        ) {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            continue;
        };
        let rows = rows(registration);
        let actual = evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &terminals)
            .unwrap();
        assert_eq!(actual.len(), registration.segment.constraint_count);
        // Degree <=4 INCLUDING fixed selectors. Each cell is a cubic in w,
        // so thirteen ordinary F evaluations reconstruct every residue before
        // reduction modulo w^4-7, including the final four terminal bindings.
        let mut expected = vec![E::ZERO; actual.len()];
        for sample in 0..13 {
            let t = F(sample);
            let lifted = rows.each_ref().map(|row| {
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
            let base = p256_opened_residues_v1(
                registration,
                opening(&lifted),
                &lifted[4],
                challenges,
                &terminals,
            )
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
            for (sum, &value) in expected.iter_mut().zip(&base) {
                *sum = sum.add(weight.mul_base(value));
            }
            if sample == 0 {
                let identity = p256_main_registration_from_main_layout_v1(registration).unwrap();
                let scalar_adapter = if identity.adapter_v1() == P256MainAdapterV1::WalletLowS {
                    let claim = terminals
                        .cross_claim(P256CrossTraceTerminalRoleV1::WalletLowS)
                        .unwrap();
                    adapter::evaluate_p256_low_s_aggregate_residues_v1(
                        lifted[0].as_slice().try_into().unwrap(),
                        lifted[1].as_slice().try_into().unwrap(),
                        lifted[2].as_slice().try_into().unwrap(),
                        lifted[3].as_slice().try_into().unwrap(),
                        lifted[4].as_slice().try_into().unwrap(),
                        claim.start,
                        challenges.cross,
                    )
                    .unwrap()
                } else {
                    let role = if identity.local_instance_v1() == 0 {
                        P256CrossTraceTerminalRoleV1::DigestReduction
                    } else {
                        P256CrossTraceTerminalRoleV1::ResultXReduction
                    };
                    let claim = terminals.cross_claim(role).unwrap();
                    adapter::evaluate_p256_reduction_aggregate_residues_v1(
                        lifted[0].as_slice().try_into().unwrap(),
                        lifted[1].as_slice().try_into().unwrap(),
                        lifted[2].as_slice().try_into().unwrap(),
                        lifted[3].as_slice().try_into().unwrap(),
                        lifted[4].as_slice().try_into().unwrap(),
                        claim.start,
                        challenges.cross,
                    )
                    .unwrap()
                };
                assert_eq!(scalar_adapter, base[..base.len() - 4]);
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
                    base.into_iter().map(E::from_base).collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(actual, expected);
        assert!(
            actual
                .iter()
                .any(|value| value.coefficients()[1] != F::ZERO)
        );
    }
}

#[test]
fn main_comparison_fp4_binds_terminal_claims_and_rejects_malformed_inputs() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let challenges = challenges();
    for registration in layout.registered_segments {
        if !matches!(
            registration.segment.adapter,
            SegmentAdapterIdV1::P256Reduction | SegmentAdapterIdV1::P256LowS
        ) {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            continue;
        };
        let mut rows = rows(registration);
        let mut terminals = terminals();
        let baseline = evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &terminals)
            .unwrap();
        let identity = p256_main_registration_from_main_layout_v1(registration).unwrap();
        let role = match identity.adapter_v1() {
            P256MainAdapterV1::WalletLowS => P256CrossTraceTerminalRoleV1::WalletLowS,
            _ if identity.local_instance_v1() == 0 => P256CrossTraceTerminalRoleV1::DigestReduction,
            _ => P256CrossTraceTerminalRoleV1::ResultXReduction,
        };
        let claim_index = terminals
            .cross_sources
            .iter()
            .position(|claim| claim.role == role)
            .unwrap();
        terminals.cross_sources[claim_index].terminal[0] = F(31);
        let changed = evaluator
            .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &terminals)
            .unwrap();
        assert_eq!(
            &baseline[..baseline.len() - 4],
            &changed[..changed.len() - 4]
        );
        assert_ne!(baseline[baseline.len() - 4], changed[changed.len() - 4]);
        terminals.cross_sources[claim_index].terminal[0] = F(u64::MAX);
        assert!(
            evaluator
                .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &terminals)
                .is_err()
        );
        terminals.cross_sources[claim_index].terminal[0] = F(29);
        rows[0].pop();
        assert!(
            evaluator
                .evaluate_residues_v1(opening(&rows), &rows[4], challenges, &terminals)
                .is_err()
        );
    }
}

#[test]
fn arithmetic_fp4_capability_binds_five_registrations_and_source_claims() {
    let challenges = challenges();
    let mut count = 0;
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        if registration.segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
            continue;
        }
        let Some(MainFp4AirEvaluatorV1::P256(evaluator)) =
            MainFp4AirEvaluatorV1::for_registration_v1(registration).unwrap()
        else {
            panic!("arithmetic evaluator")
        };
        let input = rows(registration);
        let mut claims = terminals();
        let original = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &claims)
            .unwrap();
        assert_eq!(original.len(), 463);
        claims.buses.arithmetic_value_copy[0] = F(7);
        claims.buses.arithmetic_scalar[1] = F(11);
        let changed = evaluator
            .evaluate_residues_v1(opening(&input), &input[4], challenges, &claims)
            .unwrap();
        assert_eq!(&changed[..455], &original[..455]);
        assert_ne!(&changed[455..], &original[455..]);
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn der_and_rfc_fp4_capabilities_require_complete_registered_openings() {
    let der = ZkX509DerStarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|slot| F((1_000 + lane * 100 + slot) as u64))
        }),
        byte_lookup: [F(9_001), F(9_002), F(9_003), F(9_004)],
    };
    let rfc = ZkX509Rfc5280StarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|slot| F((10_000 + lane * 100 + slot) as u64))
        }),
    };
    let der_context = || DerMainFp4AirContextV1 {
        challenges: der,
        public: ZkX509DerStarkPublicTerminalsV1,
        terminals: ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(13); 4],
            node: [F(17); 4],
        },
    };
    let rfc_context = || RfcMainFp4AirContextV1 {
        der,
        rfc,
        terminals: ZkX509Rfc5280StarkTerminalClaimsV1::canonical_for_der_test_v1(
            der_context().terminals,
        )
        .unwrap(),
    };
    let mut counts = [0; 2];
    for registration in AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .registered_segments
    {
        let evaluator = MainFp4AirEvaluatorV1::for_registration_v1(registration)
            .unwrap()
            .unwrap();
        let input = rows(registration);
        match evaluator {
            MainFp4AirEvaluatorV1::StrictDer(evaluator) => {
                let next_fixed = input[4]
                    .iter()
                    .map(|&value| value.add(E::ONE))
                    .collect::<Vec<_>>();
                let actual = evaluator
                    .evaluate_residues_v1(opening(&input), &input[4], &next_fixed, der_context())
                    .unwrap();
                assert_eq!(actual.len(), registration.segment.constraint_count);
                assert_ne!(
                    actual,
                    evaluator
                        .evaluate_residues_v1(opening(&input), &input[4], &input[4], der_context())
                        .unwrap()
                );
                assert!(
                    evaluator
                        .evaluate_residues_v1(
                            opening(&input),
                            &input[4],
                            &next_fixed[1..],
                            der_context()
                        )
                        .is_err()
                );
                counts[0] += 1;
            }
            MainFp4AirEvaluatorV1::Rfc5280(evaluator) => {
                assert_eq!(
                    evaluator
                        .evaluate_residues_v1(opening(&input), &input[4], rfc_context())
                        .unwrap()
                        .len(),
                    registration.segment.constraint_count
                );
                let mut malformed = input;
                malformed[2].pop();
                assert!(
                    evaluator
                        .evaluate_residues_v1(opening(&malformed), &malformed[4], rfc_context())
                        .is_err()
                );
                counts[1] += 1;
            }
            _ => {}
        }
    }
    assert_eq!(counts, [1, 1]);
}
