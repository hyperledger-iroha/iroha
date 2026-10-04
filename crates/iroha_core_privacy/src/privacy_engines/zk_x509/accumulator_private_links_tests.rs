//! Native fixed-provider and adversarial algebra checks for private CA joins.

use super::super::super::sha_call_bus_stark::{
    ZkX509ShaBatchFixedProviderV1, ZkX509ShaCallBusLaneChallengesV1, ZkX509ShaCallPublicShapeV1,
};
use super::super::super::sha_word_stark::{
    SHA_WORD_CAPACITY_CALL_FIRST_V1, SHA_WORD_CAPACITY_CALL_LAST_V1,
    SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1, SHA_WORD_CAPACITY_INPUT_WORD_V1,
    SHA_WORD_CAPACITY_MEMORY_SELECTOR_V1,
};
use super::*;
use crate::privacy_engines::zk_x509::proof_instance::TEST_PROOF_INSTANCE_V1;

fn challenges_v1() -> ZkX509ShaCallBusChallengesV1 {
    ZkX509ShaCallBusChallengesV1 {
        lanes: core::array::from_fn(|lane| ZkX509ShaCallBusLaneChallengesV1 {
            terms: core::array::from_fn(|term| F((2 + lane * 7 + term) as u64)),
        }),
    }
}
fn plan_v1(disclosures: u8) -> (ZkX509ShaCallScheduleV1, CaMainPrivateLinkPlanV1) {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: usize::from(disclosures),
    })
    .unwrap();
    let plan = CaMainPrivateLinkPlanV1::new_v1(&schedule, challenges_v1()).unwrap();
    (schedule, plan)
}
fn transcript_v1() -> TransparentTranscriptV1 {
    TransparentTranscriptV1::new(
        TEST_PROOF_INSTANCE_V1.main_context_v1(),
        b"ca-private-link-test",
        &PrivacyOuterDigestV1::default(),
        &PrivacyOuterDigestV1::default(),
    )
    .unwrap()
}
fn value_v1(column: MainCaColumnV1) -> E {
    let (segment, column) = match column {
        MainCaColumnV1::Sha { segment, column } => (u64::from(segment), column),
        MainCaColumnV1::Rfc { column } => (4, column),
    };
    E::canonical([2 + segment * 1000 + u64::from(column), 3, 5, 7]).unwrap()
}

#[test]
fn all_ca_calls_end_at_native_memory_rows_with_no_source_or_digest_event() {
    let root = goldilocks_primitive_root_v1(19).unwrap();
    let ca_root = goldilocks_primitive_root_v1(12).unwrap();
    for disclosures in 0..=4 {
        let (schedule, plan) = plan_v1(disclosures);
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        for call_index in 16..29 {
            let call = schedule.call(call_index).unwrap();
            let segment = call.first_logical_row / MAIN_ROWS_V1;
            let first = call.first_logical_row % MAIN_ROWS_V1;
            let last = first + call.maximum_logical_rows() - 1;
            assert_eq!(call.maximum_blocks, 3);
            assert_eq!(call.maximum_logical_rows(), 9_632);
            let first_fixed = provider.fixed_row_v1(segment, first).unwrap();
            let last_fixed = provider.fixed_row_v1(segment, last).unwrap();
            let preceding = provider.fixed_row_v1(segment, last - 1).unwrap();
            assert_eq!(first_fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1], F::ONE);
            assert_eq!(first_fixed[SHA_WORD_CAPACITY_CALL_LAST_V1], F::ZERO);
            assert_eq!(last_fixed[SHA_WORD_CAPACITY_CALL_LAST_V1], F::ONE);
            assert_eq!(last_fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1], F::ZERO);
            assert_eq!(last_fixed[SHA_WORD_CAPACITY_MEMORY_SELECTOR_V1], F::ONE);
            assert_eq!(last_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1], F::ZERO);
            assert_eq!(last_fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1], F::ZERO);
            assert_eq!(preceding[SHA_WORD_CAPACITY_CALL_LAST_V1], F::ZERO);
            // The assertion is independent of row-active and compressed private
            // factors: either accumulator's exact after expression is before.
            for active in [F::ZERO, F::ONE, F(29)] {
                for factor in [F::ZERO, F::ONE, F(31)] {
                    let before = F(37);
                    let input_after = before.mul(
                        F::ONE.add(
                            last_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1]
                                .mul(active)
                                .mul(factor.sub(F::ONE)),
                        ),
                    );
                    let digest_after = before.mul(F::ONE.add(
                        last_fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1].mul(factor.sub(F::ONE)),
                    ));
                    assert_eq!(input_after, before);
                    assert_eq!(digest_after, before);
                }
            }
            for family in 0..2 {
                let row = call_index - 16;
                let ca_row = if row == 0 && family == 0 { 103 } else { row };
                for lane in 0..4 {
                    let link = plan.links[row * 8 + family * 4 + lane];
                    assert_eq!(link.endpoint, root.pow(last as u128));
                    let start = plan.main_openings[link.main_start.unwrap()];
                    assert_eq!(start.column, link.main);
                    assert_eq!(start.multiplier, root.pow(514_657));
                    assert_eq!(start.multiplier.mul(link.endpoint), root.pow(first as u128));
                    assert_eq!(
                        plan.ca_openings[link.ca].multiplier.mul(link.endpoint),
                        ca_root.pow(ca_row as u128)
                    );
                }
            }
        }
        for lane in 0..4 {
            let link = plan.links[104 + lane];
            assert_eq!(
                link.main,
                MainCaColumnV1::Rfc {
                    column: (252 + lane) as u16
                }
            );
            assert_eq!(link.main_start, None);
            assert_eq!(plan.ca_openings[link.ca].column, (124 + lane) as u16);
            assert_eq!(plan.ca_openings[link.ca].multiplier, root.pow(13_185));
            assert_eq!(
                plan.ca_openings[link.ca].multiplier.mul(link.endpoint),
                ca_root.pow(103)
            );
        }
        assert_eq!(plan.main_openings_v1().len(), 24);
        assert_eq!(plan.ca_openings_v1().len(), 108);
        assert_eq!(plan.links_v1().len(), 108);
    }
}

#[test]
fn each_of_108_quotients_binds_original_main_start_and_ca_fields() {
    let (_, plan) = plan_v1(4);
    let point = E::canonical([13, 17, 19, 23]).unwrap();
    let main_extra: [E; 24] =
        core::array::from_fn(|index| E::canonical([index as u64 + 41, 43, 47, 53]).unwrap());
    let ca_extra: [E; 108] =
        core::array::from_fn(|index| E::canonical([index as u64 + 59, 61, 67, 71]).unwrap());
    for index in 0..108 {
        let mut alphas = [E::ZERO; 108];
        alphas[index] = E::ONE;
        let link = plan.links[index];
        let start = link.main_start.map_or(E::ONE, |i| main_extra[i]);
        let inverse = point.sub(E::from_base(link.endpoint)).inv().unwrap();
        let expected = value_v1(link.main)
            .sub(start.mul(ca_extra[index]).mul_base(link.public_factor))
            .mul(inverse);
        assert_eq!(
            plan.evaluate_v1(point, &alphas, &main_extra, &ca_extra, |c| Ok(value_v1(c)))
                .unwrap(),
            expected
        );
        let mut bad_ca = ca_extra;
        bad_ca[index] = bad_ca[index].add(E::ONE);
        assert_ne!(
            plan.evaluate_v1(point, &alphas, &main_extra, &bad_ca, |c| Ok(value_v1(c)))
                .unwrap(),
            expected
        );
        if let Some(start_index) = link.main_start {
            let mut bad_start = main_extra;
            bad_start[start_index] = bad_start[start_index].add(E::ONE);
            assert_ne!(
                plan.evaluate_v1(point, &alphas, &bad_start, &ca_extra, |c| Ok(value_v1(c)))
                    .unwrap(),
                expected
            );
        }
        assert_ne!(
            plan.evaluate_v1(point, &alphas, &main_extra, &ca_extra, |c| Ok(
                value_v1(c).add(if c == link.main { E::ONE } else { E::ZERO })
            ))
            .unwrap(),
            expected
        );
        let mut changed_alpha = alphas;
        changed_alpha[index] = E::ZERO;
        assert_ne!(
            plan.evaluate_v1(point, &changed_alpha, &main_extra, &ca_extra, |c| Ok(
                value_v1(c)
            ))
            .unwrap(),
            expected
        );
    }
    assert_eq!(
        plan.evaluate_v1(
            point,
            &[E::ONE; 108],
            &[E::ZERO; 24],
            &[E::ZERO; 108],
            |_| Ok(E::ZERO)
        )
        .unwrap(),
        E::ZERO
    );
    // A zero cumulative start intentionally annihilates its call product. This
    // is the grand-product bad-challenge event, not an exact product decoder.
    // Security accounting must include its probability across four lanes.
    let mut alphas = [E::ZERO; 108];
    alphas[0] = E::ONE;
    assert_eq!(
        plan.evaluate_v1(point, &alphas, &[E::ZERO; 24], &ca_extra, |_| Ok(E::ZERO))
            .unwrap(),
        E::ZERO
    );
    assert_ne!(
        plan.evaluate_v1(point, &alphas, &[E::ZERO; 24], &ca_extra, |_| Ok(E::ONE))
            .unwrap(),
        E::ZERO
    );
}

#[test]
fn all_plan_coordinates_endpoints_and_public_leaf_factors_precede_alphas() {
    let (_, plan) = plan_v1(0);
    let expected = plan.derive_alphas_v1(&mut transcript_v1()).unwrap();
    for index in 0..108 {
        for kind in 0..5 {
            let mut mutant = plan.clone();
            let link = &mut mutant.links[index];
            match kind {
                0 => link.main = MainCaColumnV1::Rfc { column: 1 },
                1 => link.main_start = Some((link.main_start.unwrap_or(0) + 1) % 24),
                2 => link.ca = (link.ca + 1) % 108,
                3 => link.endpoint = link.endpoint.add(F::ONE),
                _ => link.public_factor = link.public_factor.add(F::ONE),
            }
            assert_ne!(
                mutant.derive_alphas_v1(&mut transcript_v1()).unwrap(),
                expected
            );
        }
    }
    for index in 0..24 {
        for kind in 0..2 {
            let mut mutant = plan.clone();
            if kind == 0 {
                mutant.main_openings[index].column = MainCaColumnV1::Rfc { column: 0 };
            } else {
                mutant.main_openings[index].multiplier =
                    mutant.main_openings[index].multiplier.add(F::ONE);
            }
            assert_ne!(
                mutant.derive_alphas_v1(&mut transcript_v1()).unwrap(),
                expected
            );
        }
    }
    for index in 0..108 {
        for kind in 0..2 {
            let mut mutant = plan.clone();
            if kind == 0 {
                mutant.ca_openings[index].column += 1;
            } else {
                mutant.ca_openings[index].multiplier =
                    mutant.ca_openings[index].multiplier.add(F::ONE);
            }
            assert_ne!(
                mutant.derive_alphas_v1(&mut transcript_v1()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn malformed_extra_counts_fields_and_singular_points_are_rejected() {
    let (_, plan) = plan_v1(0);
    let point = E::canonical([2, 3, 5, 7]).unwrap();
    let alphas = [E::ONE; 108];
    let main = [E::ONE; 24];
    let ca = [E::ONE; 108];
    assert!(
        plan.evaluate_v1(point, &alphas[..107], &main, &ca, |_| Ok(E::ONE))
            .is_err()
    );
    assert!(
        plan.evaluate_v1(point, &alphas, &main[..23], &ca, |_| Ok(E::ONE))
            .is_err()
    );
    assert!(
        plan.evaluate_v1(point, &alphas, &main, &ca[..107], |_| Ok(E::ONE))
            .is_err()
    );
    for link in plan.links {
        assert!(
            plan.evaluate_v1(E::from_base(link.endpoint), &alphas, &main, &ca, |_| Ok(
                E::ONE
            ))
            .is_err()
        );
    }
    assert!(
        plan.evaluate_v1(E::ZERO, &alphas, &main, &ca, |_| Ok(E::ONE))
            .is_err()
    );
    assert!(
        plan.evaluate_v1(point, &alphas, &main, &ca, |_| Err(
            ZkX509CaAccumulatorProofErrorV1::ConstraintOpening
        ))
        .is_err()
    );
}

#[test]
fn translated_point_census_bounds_actual_per_column_mask_coordinates() {
    let (_, plan) = plan_v1(4);
    let mut per_column = std::collections::BTreeMap::<u16, std::collections::BTreeSet<u64>>::new();
    let root = goldilocks_primitive_root_v1(19).unwrap();
    for opening in plan.ca_openings {
        assert_eq!(opening.multiplier.pow(1 << 19), F::ONE);
        assert_ne!(opening.multiplier, F::ONE);
        assert_ne!(opening.multiplier, root.pow(128));
        assert!(
            per_column
                .entry(opening.column)
                .or_default()
                .insert(opening.multiplier.0)
        );
    }
    for column in 96..100 {
        assert_eq!(per_column[&column].len(), 12);
    }
    for column in 116..120 {
        assert_eq!(per_column[&column].len(), 13);
    }
    for column in 120..128 {
        assert_eq!(per_column[&column].len(), 1);
    }
    assert_eq!(per_column.len(), 16);
    assert_eq!(2 * 136 + 4 * (2 + 13), 332);
    assert!(332 < ZK_X509_CA_TRACE_MASK_DEGREE_V1 as usize + 1);
    assert_eq!(2 * 136 + 4 * 3, 284);
    assert!(284 < super::super::super::profile::ZK_X509_TRACE_MASK_DEGREE_V1 as usize + 1);
    assert_eq!((24 + 108) * 32, 4_224);
    let main_degree =
        (1_usize << 19) + usize::from(super::super::super::profile::ZK_X509_TRACE_MASK_DEGREE_V1);
    let ca_degree =
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 + usize::from(ZK_X509_CA_TRACE_MASK_DEGREE_V1);
    assert_eq!(ca_degree, 4_096 + 2_099);
    assert_eq!(main_degree + ca_degree - 1, 532_297);
    assert!(main_degree + ca_degree - 1 < 1 << 20);
    for opening in plan.main_openings {
        assert_eq!(opening.multiplier.pow(1 << 19), F::ONE);
        assert_ne!(opening.multiplier, F::ONE);
        assert_ne!(opening.multiplier, root);
    }
}

#[test]
fn complete_ca_translation_admission_and_original_auxiliary_openings_are_exact() {
    let (_, plan) = plan_v1(4);
    let z = E::canonical([31, 5, 17, 2]).unwrap();
    assert!(plan.admissible_v1(z).unwrap());
    for bad in [
        E::ZERO,
        E::ONE,
        E::from_base(F(7)),
        E::from_raw_coefficients_for_testing([F(u64::MAX), F::ZERO, F::ZERO, F::ZERO]),
    ] {
        assert!(!plan.admissible_v1(bad).unwrap());
    }
    let ca_root = goldilocks_primitive_root_v1(12).unwrap();
    let main_root = goldilocks_primitive_root_v1(19).unwrap();
    for opening in plan.ca_openings {
        let inverse = opening.multiplier.inv().unwrap();
        for target in [F::ONE, ca_root, F(7)] {
            assert!(
                !plan
                    .admissible_v1(E::from_base(target.mul(inverse)))
                    .unwrap()
            );
        }
    }
    for opening in plan.main_openings {
        let inverse = opening.multiplier.inv().unwrap();
        for target in [F::ONE, main_root, F(7)] {
            assert!(
                !plan
                    .admissible_v1(E::from_base(target.mul(inverse)))
                    .unwrap()
            );
        }
    }
    let values: [E; 108] = core::array::from_fn(|index| E::from_base(F(index as u64 + 3)));
    let mixes: [E; 108] = core::array::from_fn(|index| E::from_base(F(index as u64 + 107)));
    let extras = plan.ca_supplemental_v1(z, &values, &mixes).unwrap();
    assert_eq!(extras.len(), 108);
    assert_eq!(extras.capacity(), 108);
    for (index, actual) in extras.iter().enumerate() {
        assert_eq!(actual.group, 0);
        assert_eq!(
            actual.column,
            aggregate::AggregateSupplementalColumnV1::Auxiliary(usize::from(
                plan.ca_openings[index].column
            ))
        );
        assert_eq!(actual.point, z.mul_base(plan.ca_openings[index].multiplier));
        assert_eq!(actual.value, values[index]);
        assert_eq!(actual.mix, mixes[index]);
    }
    assert!(plan.ca_supplemental_v1(z, &values[..107], &mixes).is_err());
    assert!(plan.ca_supplemental_v1(z, &values, &mixes[..107]).is_err());
    assert!(plan.ca_supplemental_v1(E::ONE, &values, &mixes).is_err());
    for index in 0..108 {
        let mut bad_values = values;
        bad_values[index] =
            E::from_raw_coefficients_for_testing([F(u64::MAX), F::ZERO, F::ZERO, F::ZERO]);
        assert!(plan.ca_supplemental_v1(z, &bad_values, &mixes).is_err());
        let mut bad_mixes = mixes;
        bad_mixes[index] =
            E::from_raw_coefficients_for_testing([F(u64::MAX), F::ZERO, F::ZERO, F::ZERO]);
        assert!(plan.ca_supplemental_v1(z, &values, &bad_mixes).is_err());
    }
}
