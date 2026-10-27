//! Exact authoritative-time joins and unchanged-private-CRL counterexample controls.
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
    rfc5280_stark::zk_x509_rfc_crl_time_join_rows_v1,
};
use iroha_data_model::privacy::{PrivacyConsensusLimitsV1, PrivacyZkX509CrlRecordV1};

fn plan_v1(this_update: u64, next_update: u64) -> MainPublicCrlTimePlanV1 {
    let statement =
        crate::privacy_engines::zk_x509::stark::tests::main_log19_statement_fixture_v1();
    MainPublicCrlTimePlanV1::new_v1(
        &AggregateProofLayoutV1::for_full_profile_v1().unwrap(),
        ZkX509Rfc5280StarkShapeV1::from_statement(&statement).unwrap(),
        ZkX509CrlPublicInputsV1 {
            crl_der: [7; 32],
            issuer_spki: [8; 32],
            this_update,
            next_update,
        },
    )
    .unwrap()
}
#[test]
fn public_time_quotients_use_one_original_column_and_reject_each_mismatch() {
    let plan = plan_v1(1_672_531_200, 1_672_531_500);
    assert_eq!(plan.blocks[0].source, plan.blocks[1].source);
    let [x, y] = plan.blocks.map(|b| b.start);
    assert_ne!(x, y);
    let slope = plan.values[1]
        .sub(plan.values[0])
        .mul(y.sub(x).inv().unwrap());
    // S interpolates both timestamps and includes a nonzero multiple of both
    // endpoint factors. Both quotients consume exactly this same polynomial.
    let source = vec![
        plan.values[0].sub(slope.mul(x)).add(x.mul(y).mul(F(11))),
        slope.sub(x.add(y).mul(F(11))),
        F(11),
    ];
    let expected = [
        vec![slope.sub(y.mul(F(11))), F(11)],
        vec![slope.sub(x.mul(F(11))), F(11)],
    ];
    for i in 0..2 {
        let q = plan.quotient_v1(i, &source).unwrap();
        assert_eq!(&q[..2], &expected[i]);
        assert_eq!(q[2], F::ZERO);
        let changed = if i == 0 {
            plan_v1(plan.values[0].0 + 1, plan.values[1].0)
        } else {
            plan_v1(plan.values[0].0, plan.values[1].0 + 1)
        };
        assert!(matches!(
            changed.quotient_v1(i, &source),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));
        assert!(changed.quotient_v1(1 - i, &source).is_ok());
    }
    for z in [E::from_base(F(29)), E::canonical([29, 3, 5, 7]).unwrap()] {
        let owner = plan.source_v1();
        let mut groups = (0..=owner.group)
            .map(|_| aggregate::AggregateOpenedDeepTraceGroupV1 {
                base_current: vec![E::ZERO; owner.column + 1],
                base_next: Vec::new(),
                aux_current: Vec::new(),
                aux_next: Vec::new(),
            })
            .collect::<Vec<_>>();
        groups[owner.group].base_current[owner.column] =
            MainKeyJoinPlanV1::evaluate_coefficients_v1(&source, z);
        let alphas = [
            E::canonical([31, 11, 13, 17]).unwrap(),
            E::canonical([37, 19, 23, 29]).unwrap(),
        ];
        let direct = (0..2).fold(E::ZERO, |sum, i| {
            sum.add(alphas[i].mul(MainKeyJoinPlanV1::evaluate_coefficients_v1(&expected[i], z)))
        });
        assert_eq!(plan.evaluate_v1(&groups, z, &alphas).unwrap(), direct);
        for i in 0..2 {
            assert!(
                plan.evaluate_v1(&groups, E::from_base(plan.blocks[i].start), &alphas)
                    .is_err()
            );
        }
        assert!(plan.evaluate_v1(&[], z, &alphas).is_err());
        assert!(plan.evaluate_v1(&groups, z, &alphas[..1]).is_err());
    }
    assert!(plan.quotient_v1(2, &source).is_err());
    assert_eq!((1usize << 19) + MASK_DEGREE - 1, 526_102);
    assert_eq!(BLOCKS_V1, 21);
    assert_eq!(OPENINGS_V1, 31);
}
#[test]
fn public_time_values_and_native_locations_are_transcript_bound() {
    let state = |plan: &MainPublicCrlTimePlanV1| {
        let mut t = new_main_transcript_after_profile_validation_v1(
            TEST_PROOF_INSTANCE_V1,
            &[7; 32],
            [8; 32],
        )
        .unwrap();
        plan.absorb_plan_v1(&mut t).unwrap();
        t.state()
    };
    let original = plan_v1(1000, 1300);
    let expected = state(&original);
    assert_ne!(state(&plan_v1(1001, 1300)), expected);
    assert_ne!(state(&plan_v1(1000, 1301)), expected);
    for variant in 0..4 {
        let mut changed = plan_v1(1000, 1300);
        match variant {
            0 => changed.blocks.swap(0, 1),
            1 => changed.blocks[0].source.column += 1,
            2 => changed.blocks[0].start = changed.blocks[0].start.mul(changed.blocks[0].step),
            _ => changed.values.swap(0, 1),
        }
        assert_ne!(state(&changed), expected);
    }
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let shape = ZkX509Rfc5280StarkShapeV1::from_statement(
        &crate::privacy_engines::zk_x509::stark::tests::main_log19_statement_fixture_v1(),
    )
    .unwrap();
    for (this_update, next_update) in [
        (1, 1),
        (2, 1),
        (
            1,
            iroha_data_model::privacy::ZK_X509_MAX_UNIX_SECONDS_V1 + 1,
        ),
    ] {
        assert!(
            MainPublicCrlTimePlanV1::new_v1(
                &layout,
                shape,
                ZkX509CrlPublicInputsV1 {
                    crl_der: [7; 32],
                    issuer_spki: [8; 32],
                    this_update,
                    next_update
                }
            )
            .is_err()
        );
    }
}
#[test]
#[ignore = "actual signed assembly and full original RFC timestamp column with nonzero mask"]
#[allow(clippy::too_many_lines)]
fn production_assembly_rejects_retained_crl_time_substitutions_after_public_admission() {
    use crate::privacy_engines::transparent_stark::goldilocks_ifft_v1;
    let fixture =
        build_zk_x509_release_fixture_v1(reference_statement_context_v1(), false).unwrap();
    let state = &fixture.authoritative_state;
    let crl = state.crl_record();
    let trust = state.trust_anchor();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust,
            certificate_policy: state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let shape = assembly.rfc_base.schedule.shape;
    let original = MainKeyJoinPlanV1::new_v1(
        &layout,
        &fixture.statement,
        shape,
        assembly.crl_public_inputs,
    )
    .unwrap();
    let (rows, column) = zk_x509_rfc_crl_time_join_rows_v1(shape).unwrap();
    assert_eq!(
        assembly.rfc_base.base_row(rows[0]).unwrap()[column],
        F(crl.this_update_unix_seconds)
    );
    assert_eq!(
        assembly.rfc_base.base_row(rows[1]).unwrap()[column],
        F(crl.next_update_unix_seconds)
    );
    let n = 1usize << 19;
    let mut coefficients = ZeroizingMainTraceColumnV1(vec![F::ZERO; n + MASK_DEGREE + 1]);
    assembly
        .rfc_base
        .fill_base_columns_v1(column, &mut [&mut coefficients.0[..n]])
        .unwrap();
    goldilocks_ifft_v1(
        &mut coefficients.0[..n],
        goldilocks_primitive_root_v1(19).unwrap(),
    )
    .unwrap();
    for i in 0..=MASK_DEGREE {
        let mask = F(i as u64 + 43);
        coefficients.0[i] = coefficients.0[i].sub(mask);
        coefficients.0[n + i] = coefficients.0[n + i].add(mask);
    }
    for i in 0..2 {
        assert!(original.public_times.quotient_v1(i, &coefficients).is_ok());
    }
    for index in 0..2 {
        let changed = PrivacyZkX509CrlRecordV1::new(
            crl.trust_anchor_id,
            crl.certificate_policy_id,
            crl.record_epoch,
            crl.crl_number,
            crl.crl_der_digest,
            crl.issuer_spki_digest,
            crl.this_update_unix_seconds + u64::from(index == 0),
            crl.next_update_unix_seconds + u64::from(index == 1),
            crl.previous_record_digest,
            crl.lifecycle,
        )
        .unwrap();
        let changed_state =
            authoritative_state_v1(trust, state.certificate_policy().clone(), changed).unwrap();
        let mut statement = fixture.statement.clone();
        statement.crl_record_digest = changed.record_digest;
        crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
            &statement,
            &changed_state,
            statement.presentation_not_before_unix_seconds * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
        )
        .expect("independent retained and signed intervals both cover this window");
        assert_eq!(changed.crl_der_digest, crl.crl_der_digest);
        assert_eq!(changed.issuer_spki_digest, crl.issuer_spki_digest);
        assert_eq!(changed.crl_number, crl.crl_number);
        let public = ZkX509CrlPublicInputsV1::from_authoritative_state_v1(&changed_state);
        let plan = MainKeyJoinPlanV1::new_v1(&layout, &statement, shape, public).unwrap();
        assert!(matches!(
            plan.public_times.quotient_v1(index, &coefficients),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));
        assert!(
            plan.public_times
                .quotient_v1(1 - index, &coefficients)
                .is_ok()
        );
    }
}
