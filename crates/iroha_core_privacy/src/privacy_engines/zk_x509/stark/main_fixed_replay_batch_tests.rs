//! Exact-source arithmetic fixed-polynomial parity after bounded public replay.
use super::*;
use crate::privacy_engines::zk_x509::{
    main_assembly::build_zk_x509_main_trace_assembly_v1,
    relation::{
        ZkX509GovernanceV1,
        release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
    },
};
#[test]
#[ignore = "all five maximum-credential arithmetic fixed polynomial sets against independent scalar interpolation"]
fn maximum_arithmetic_fixed_sets_preserve_every_coefficient_and_registration() {
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum signed credential");
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
    .expect("actual maximum MAIN assembly");
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    assert_eq!(layout.registered_segments.len(), 49);
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
        [0x81; 32],
        assembly.verifier_profile.compiled_profile_digest,
        core::array::from_fn(|index| digest(index as u8 + 1)),
    );
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        digest(0x91),
        digest(0xA1),
        digest(0xB1),
    )
    .unwrap();
    let sha = core::array::from_fn(|segment| {
        ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
            &assembly.sha_schedule,
            &assembly.sha_witnesses,
            segment,
        )
        .unwrap()
    });
    let p256 = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let source = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
        &layout, &assembly, sha, p256, binding,
    )
    .unwrap();

    let prover = MainLog19ProverConstraintSourceV1::for_main_v1(&layout, &source).unwrap();
    let observation =
        crate::privacy_engines::zk_x509::prover_observation::ObservationV1::begin_v1();
    let mut checked = 0;
    for registration in source.registrations.iter().copied() {
        if registration.segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
            continue;
        }
        let root = goldilocks_primitive_root_v1(registration.segment.trace_log2).unwrap();
        let actual = main_log19_fixed_polynomial_set_v1(&prover, registration).unwrap();
        assert_eq!(actual.registration, registration);
        assert_eq!(actual.columns.len(), registration.segment.fixed_width);
        for (column, coefficients) in actual.columns.iter().enumerate() {
            let mut expected = source.native_fixed_column_v1(registration, column).unwrap();
            goldilocks_ifft_v1(&mut expected, root).unwrap();
            assert_eq!(coefficients.as_slice(), &*expected);
        }
        let mut invalid = registration;
        invalid.segment.fixed_width -= 1;
        assert!(main_log19_fixed_polynomial_set_v1(&prover, invalid).is_err());
        checked += 1;
    }
    assert_eq!(checked, 5);
    let receipt = observation.finish_v1().public_text_v1();
    for phase in [
        "CompositionArithmeticFixedRows",
        "CompositionArithmeticFixedInverseTransform",
    ] {
        assert!(receipt.contains(&format!(
            "phase={phase} calls=5 completed=5 interrupted=0 unwound=0"
        )));
    }
    eprintln!("{receipt}");
}
