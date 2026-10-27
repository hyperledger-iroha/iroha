//! Public CRL endpoint schedule, polynomial, transcript and actual-assembly regressions.

use super::*;
use crate::privacy_engines::zk_x509::{
    main_assembly::build_zk_x509_main_trace_assembly_v1,
    relation::{
        ZkX509GovernanceV1,
        release_fixture::{
            authoritative_state_v1, build_zk_x509_release_fixture_v1,
            reference_statement_context_v1,
        },
    },
    sha_call_bus_stark::{ZkX509ShaBatchFixedProviderV1, ZkX509ShaBatchSegmentBaseSourceV1},
    sha_word_stark::{
        SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1, SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1,
    },
};
use iroha_data_model::privacy::{
    PrivacyX509CrlDerDigestV1, PrivacyX509CrlIssuerSpkiDigestV1, PrivacyZkX509CrlRecordV1,
};

fn public_v1() -> ZkX509CrlPublicInputsV1 {
    ZkX509CrlPublicInputsV1 {
        crl_der: core::array::from_fn(|i| i as u8 + 7),
        issuer_spki: core::array::from_fn(|i| i as u8 + 107),
        this_update: 1,
        next_update: 2,
    }
}
fn plan_v1(disclosures: usize, public: ZkX509CrlPublicInputsV1) -> MainPublicShaDigestPlanV1 {
    MainPublicShaDigestPlanV1::new_v1(
        &AggregateProofLayoutV1::for_full_profile_v1().unwrap(),
        disclosures,
        public,
    )
    .unwrap()
}
fn multiply_v1(left: &[F], right: &[F]) -> Vec<F> {
    let mut out = vec![F::ZERO; left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            out[i + j] = out[i + j].add(a.mul(*b));
        }
    }
    out
}
fn common_polynomial_v1(plan: &MainPublicShaDigestPlanV1) -> (Vec<F>, [Vec<F>; 2]) {
    let points = (0..2)
        .flat_map(|index| {
            (0..8).map(move |j| plan.blocks[index].start.mul(plan.blocks[index].step.pow(j)))
        })
        .collect::<Vec<_>>();
    assert_eq!(points.len(), 16);
    let values = points
        .iter()
        .enumerate()
        .map(|(j, point)| {
            MainKeyJoinPlanV1::evaluate_coefficients_v1(
                &plan.public_coefficients[j / 8],
                E::from_base(*point),
            )
            .coefficients()[0]
        })
        .collect::<Vec<_>>();
    let mut common = vec![F::ZERO; 16];
    for (i, point) in points.iter().enumerate() {
        let mut basis = vec![F::ONE];
        let mut denominator = F::ONE;
        for (j, other) in points.iter().enumerate() {
            if i != j {
                basis = multiply_v1(&basis, &[F::ZERO.sub(*other), F::ONE]);
                denominator = denominator.mul(point.sub(*other));
            }
        }
        let scale = values[i].mul(denominator.inv().unwrap());
        for (out, value) in common.iter_mut().zip(basis) {
            *out = out.add(value.mul(scale));
        }
    }
    let mut all_roots = vec![F::ONE];
    for point in &points {
        all_roots = multiply_v1(&all_roots, &[F::ZERO.sub(*point), F::ONE]);
    }
    let nonzero = multiply_v1(&all_roots, &[F(11), F(17), F(23), F(31)]);
    common.resize(nonzero.len(), F::ZERO);
    for (out, value) in common.iter_mut().zip(nonzero) {
        *out = out.add(value);
    }
    let quotients = core::array::from_fn(|index| {
        let mut divisor = vec![F::ONE];
        for point in &points[8 * index..8 * index + 8] {
            divisor = multiply_v1(&divisor, &[F::ZERO.sub(*point), F::ONE]);
        }
        let mut numerator = common.clone();
        for (out, value) in numerator.iter_mut().zip(plan.public_coefficients[index]) {
            *out = out.sub(value);
        }
        // Independent monic long division, rather than production's sequence of
        // linear synthetic divisions, supplies the expected quotient polynomial.
        let mut quotient = vec![F::ZERO; numerator.len() - 8];
        for high in (8..numerator.len()).rev() {
            let factor = numerator[high];
            quotient[high - 8] = factor;
            for (j, value) in divisor.iter().enumerate() {
                numerator[high - 8 + j] = numerator[high - 8 + j].sub(factor.mul(*value));
            }
        }
        assert!(numerator.iter().all(|v| *v == F::ZERO));
        quotient
    });
    (common, quotients)
}
fn groups_v1(
    plan: &MainPublicShaDigestPlanV1,
    source: &[F],
    z: E,
) -> Vec<aggregate::AggregateOpenedDeepTraceGroupV1> {
    let max_group = plan.blocks.iter().map(|b| b.source.group).max().unwrap();
    let max_column = plan.blocks.iter().map(|b| b.source.column).max().unwrap();
    let mut groups = (0..=max_group)
        .map(|_| aggregate::AggregateOpenedDeepTraceGroupV1 {
            base_current: vec![E::ZERO; max_column + 1],
            base_next: Vec::new(),
            aux_current: Vec::new(),
            aux_next: Vec::new(),
        })
        .collect::<Vec<_>>();
    for b in &plan.blocks {
        groups[b.source.group].base_current[b.source.column] =
            MainKeyJoinPlanV1::evaluate_coefficients_v1(source, z);
    }
    groups
}
#[test]
fn authoritative_digest_rows_match_actual_fixed_schedule_for_every_shape() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    for disclosures in 0..=4 {
        let plan = plan_v1(disclosures, public_v1());
        assert_eq!(plan.source_v1(0).unwrap(), plan.source_v1(1).unwrap());
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .unwrap();
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .unwrap();
        for (i, call) in CALLS_V1.into_iter().enumerate() {
            let m = schedule.calls()[call];
            let segment = m.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
            let row = m.first_logical_row % ZK_X509_SHA_SEGMENT_ROWS_V1 + m.maximum_local_rows - 8;
            let registration = layout
                .registered_segment(SegmentAdapterIdV1::Sha256CallBus, segment as u16)
                .unwrap();
            assert_eq!(
                plan.source_v1(i).unwrap(),
                ColumnV1 {
                    group: registration.trace_group,
                    column: registration.base_start,
                    log: 19
                }
            );
            let digest = if i == 0 {
                public_v1().crl_der
            } else {
                public_v1().issuer_spki
            };
            for word in 0..8 {
                let actual = fixed.fixed_row_v1(segment, row + word).unwrap();
                assert_eq!(actual[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1], F::ONE);
                assert_eq!(
                    actual[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1],
                    F(word as u64)
                );
                let point = plan.blocks[i]
                    .start
                    .mul(plan.blocks[i].step.pow(word as u128));
                assert_eq!(
                    MainKeyJoinPlanV1::evaluate_coefficients_v1(
                        &plan.public_coefficients[i],
                        E::from_base(point)
                    ),
                    E::from_base(F(u64::from(u32::from_be_bytes(
                        digest[4 * word..4 * word + 4].try_into().unwrap()
                    ))))
                );
            }
        }
    }
    assert!(MainPublicShaDigestPlanV1::new_v1(&layout, 5, public_v1()).is_err());
    let plan = plan_v1(0, public_v1());
    assert!(plan.source_v1(2).is_err());
    assert!(plan.quotient_v1(2, &[F::ZERO]).is_err());
}
#[test]
fn original_polynomial_public_digests_reject_each_word_and_match_fp4_verifier() {
    let plan = plan_v1(4, public_v1());
    let (source, quotients) = common_polynomial_v1(&plan);
    for i in 0..2 {
        let q = plan.quotient_v1(i, &source).unwrap();
        assert_eq!(&q[..quotients[i].len()], &quotients[i]);
        assert!(q[quotients[i].len()..].iter().all(|v| *v == F::ZERO));
        for byte in 0..32 {
            let mut public = public_v1();
            if i == 0 {
                public.crl_der[byte] ^= 1;
            } else {
                public.issuer_spki[byte] ^= 1;
            }
            let wrong = plan_v1(4, public);
            assert!(matches!(
                wrong.quotient_v1(i, &source),
                Err(ZkX509StarkErrorV1::ConstraintOpening)
            ));
            assert!(wrong.quotient_v1(1 - i, &source).is_ok());
        }
        for word in 0..8 {
            let z = E::from_base(plan.blocks[i].start.mul(plan.blocks[i].step.pow(word)));
            assert!(
                plan.evaluate_v1(&groups_v1(&plan, &source, z), z, &[E::ONE; 2])
                    .is_err()
            );
        }
    }
    for z in [E::from_base(F(29)), E::canonical([29, 3, 5, 7]).unwrap()] {
        let groups = groups_v1(&plan, &source, z);
        let alphas = [
            E::canonical([31, 11, 13, 17]).unwrap(),
            E::canonical([37, 19, 23, 29]).unwrap(),
        ];
        let expected = (0..2).fold(E::ZERO, |sum, i| {
            sum.add(alphas[i].mul(MainKeyJoinPlanV1::evaluate_coefficients_v1(
                &quotients[i],
                z,
            )))
        });
        assert_eq!(plan.evaluate_v1(&groups, z, &alphas).unwrap(), expected);
        for i in 0..2 {
            let mut changed = groups.clone();
            let b = plan.blocks[i];
            changed[b.source.group].base_current[b.source.column] =
                changed[b.source.group].base_current[b.source.column].add(E::ONE);
            assert_ne!(plan.evaluate_v1(&changed, z, &alphas).unwrap(), expected);
        }
        assert!(plan.evaluate_v1(&groups, z, &alphas[..1]).is_err());
        assert!(plan.evaluate_v1(&[], z, &alphas).is_err());
    }
}
#[test]
fn authoritative_public_digests_and_plan_coordinates_enter_transcript() {
    let state = |p: &MainPublicShaDigestPlanV1| {
        let mut t = new_main_transcript_after_profile_validation_v1(
            TEST_PROOF_INSTANCE_V1,
            &[7; 32],
            [8; 32],
        )
        .unwrap();
        p.absorb_plan_v1(&mut t).unwrap();
        t.state()
    };
    let plan = plan_v1(0, public_v1());
    let expected = state(&plan);
    for i in 0..2 {
        for byte in 0..32 {
            let mut public = public_v1();
            if i == 0 {
                public.crl_der[byte] ^= 1;
            } else {
                public.issuer_spki[byte] ^= 1;
            }
            assert_ne!(state(&plan_v1(0, public)), expected);
        }
    }
    for variant in 0..5 {
        let mut changed = plan_v1(0, public_v1());
        match variant {
            0 => changed.blocks.swap(0, 1),
            1 => changed.blocks[0].source.group += 1,
            2 => changed.blocks[0].source.column += 1,
            3 => changed.blocks[0].start = changed.blocks[0].start.mul(changed.blocks[0].step),
            _ => changed.blocks[0].step = changed.blocks[0].step.mul(changed.blocks[0].step),
        }
        assert_ne!(state(&changed), expected);
    }
    assert_eq!((1usize << 19) + MASK_DEGREE - 8, 526_095);
    assert_eq!(BLOCKS_V1, 21);
    assert_eq!(OPENINGS_V1, 31);
}

#[test]
#[ignore = "actual signed assembly, one shared native SHA column and two masked original polynomial quotients"]
#[allow(clippy::too_many_lines)]
fn production_assembly_rejects_retained_crl_digest_and_issuer_substitutions() {
    use crate::privacy_engines::transparent_stark::goldilocks_ifft_v1;
    let fixture =
        build_zk_x509_release_fixture_v1(reference_statement_context_v1(), false).unwrap();
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    let public = ZkX509CrlPublicInputsV1::from_authoritative_state_v1(&fixture.authoritative_state);
    assert_eq!(assembly.crl_public_inputs, public);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let make_plan = |statement: &IrohaZkX509StarkP256StatementV1, public| {
        MainKeyJoinPlanV1::new_v1(&layout, statement, assembly.rfc_base.schedule.shape, public)
            .unwrap()
    };
    let plan = make_plan(&fixture.statement, public);
    let manifest = assembly.sha_schedule.calls()[CALLS_V1[0]];
    let segment = manifest.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
    let source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
        &assembly.sha_schedule,
        &assembly.sha_witnesses,
        segment,
    )
    .unwrap();
    let n = 1usize << 19;
    let mut coefficients = ZeroizingMainTraceColumnV1(vec![F::ZERO; n + MASK_DEGREE + 1]);
    source
        .fill_base_column_v1(segment, 0, &mut coefficients.0[..n])
        .unwrap();
    goldilocks_ifft_v1(
        &mut coefficients.0[..n],
        goldilocks_primitive_root_v1(19).unwrap(),
    )
    .unwrap();
    // A deterministic nonzero mask changes every original coefficient owner
    // but vanishes at every native row. This tests the masked polynomial itself.
    for i in 0..=MASK_DEGREE {
        let mask = F(i as u64 + 41);
        coefficients.0[i] = coefficients.0[i].sub(mask);
        coefficients.0[n + i] = coefficients.0[n + i].add(mask);
    }
    for (index, call) in CALLS_V1.into_iter().enumerate() {
        assert_eq!(
            assembly.sha_schedule.calls()[call].first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1,
            segment
        );
        let canonical = plan
            .public_digest
            .quotient_v1(index, &coefficients)
            .unwrap();
        assert!(canonical.len() <= n + MASK_DEGREE + 1);
        for word in 0..8 {
            let mut crl_der = *crl.crl_der_digest.as_bytes();
            let mut issuer_spki = *crl.issuer_spki_digest.as_bytes();
            if index == 0 {
                crl_der[word * 4] ^= 1;
            } else {
                issuer_spki[word * 4] ^= 1;
            }
            let changed_record = PrivacyZkX509CrlRecordV1::new(
                crl.trust_anchor_id,
                crl.certificate_policy_id,
                crl.record_epoch,
                crl.crl_number,
                PrivacyX509CrlDerDigestV1::new(crl_der),
                PrivacyX509CrlIssuerSpkiDigestV1::new(issuer_spki),
                crl.this_update_unix_seconds,
                crl.next_update_unix_seconds,
                crl.previous_record_digest,
                crl.lifecycle,
            )
            .unwrap();
            let changed_state = authoritative_state_v1(
                trust_anchor,
                fixture.authoritative_state.certificate_policy().clone(),
                changed_record,
            )
            .unwrap();
            assert_eq!(changed_state.crl_record().crl_number, crl.crl_number);
            assert_eq!(
                changed_state.crl_record().this_update_unix_seconds,
                crl.this_update_unix_seconds
            );
            assert_eq!(
                changed_state.crl_record().next_update_unix_seconds,
                crl.next_update_unix_seconds
            );
            assert_eq!(
                changed_state.trust_anchor(),
                fixture.authoritative_state.trust_anchor()
            );
            let mut changed_statement = fixture.statement.clone();
            changed_statement.crl_record_digest = changed_record.record_digest;
            crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
                &changed_statement,
                &changed_state,
                changed_statement.presentation_not_before_unix_seconds * 1_000,
                &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
            )
            .expect("changed canonical retained digest remains ordinary admitted public state");
            let changed = make_plan(
                &changed_statement,
                ZkX509CrlPublicInputsV1::from_authoritative_state_v1(&changed_state),
            );
            assert!(matches!(
                changed.public_digest.quotient_v1(index, &coefficients),
                Err(ZkX509StarkErrorV1::ConstraintOpening)
            ));
            // The rejected source is the unchanged real SHA column, not a
            // reference-validator failure or a fabricated digest-only witness.
            assert_eq!(
                assembly.sha_witnesses[call].digest,
                if index == 0 {
                    public.crl_der
                } else {
                    public.issuer_spki
                }
            );
        }
    }
}
