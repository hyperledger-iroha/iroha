// Full original-material matrix parity against the independent scalar recurrences.

#[test]
#[ignore = "maximum RFC all565-column base/aux parity and one-pass costs; optimized build required"]
fn rfc_full_retained_native_matrices_match_independent_scalar_and_report_costs() {
    use super::super::relation::release_fixture::{
        build_zk_x509_release_fixture_v1, reference_statement_context_v1,
    };
    use super::super::verifier_profile::compile_zk_x509_rfc_statement_from_authoritative_state_v1;
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    let trace = build_zk_x509_rfc5280_trace_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
        compile_zk_x509_rfc_statement_from_authoritative_state_v1(
            &fixture.statement,
            &fixture.authoritative_state,
        ),
    )
    .unwrap();
    let material = build_zk_x509_rfc5280_stark_base_material_v1(&trace).unwrap();
    let rows = ZK_X509_RFC5280_STARK_TRACE_SIZE_V1;
    let mut base = PrivateTableV1::new(
        vec![vec![F::ZERO; rows]; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1],
        zeroize_field_rows_v1,
    );
    let started = std::time::Instant::now();
    material
        .fill_base_matrix_v1(&mut base.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>())
        .unwrap();
    let base_seconds = started.elapsed().as_secs_f64();
    for index in 0..rows {
        let original = zeroize::Zeroizing::new(material.base_row(index).unwrap());
        for (column, value) in base.iter().zip(original.iter()) {
            assert_eq!(column[index], *value);
        }
    }
    drop(base);
    let der = der_challenges_v1();
    let challenges = challenges_v1();
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    let provider =
        ZkX509Rfc5280StarkColumnProviderV1::fixture_v1(&material, &der, &challenges).unwrap();
    let mut actual = PrivateTableV1::new(
        vec![vec![F::ZERO; rows]; ZK_X509_RFC5280_STARK_AUX_WIDTH_V1],
        zeroize_field_rows_v1,
    );
    let started = std::time::Instant::now();
    provider
        .fill_aux_matrix_v1(&mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>())
        .unwrap();
    let matrix_seconds = started.elapsed().as_secs_f64();
    let mut bounded_seconds = 0.0;
    for first in (0..ZK_X509_RFC5280_STARK_AUX_WIDTH_V1).step_by(8) {
        let mut bounded = PrivateTableV1::new(vec![vec![F::ZERO; rows]; 8], zeroize_field_rows_v1);
        let started = std::time::Instant::now();
        provider
            .fill_aux_columns_v1(
                first,
                &mut bounded
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        bounded_seconds += started.elapsed().as_secs_f64();
        for (offset, column) in bounded.iter().enumerate() {
            assert_eq!(column, &actual[first + offset]);
        }
    }
    let started = std::time::Instant::now();
    for column in 0..ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 {
        let reference = PrivateTableV1::new(
            scalar_reference_v1(&material, &der, &challenges, column, &centers).unwrap(),
            zeroize_fields_v1,
        );
        assert_eq!(&*reference, &actual[column]);
    }
    eprintln!(
        "rfc_full_native_matrix base_seconds={base_seconds:.6} aux_matrix_seconds={matrix_seconds:.6} aux_bounded_seconds={bounded_seconds:.6} aux_speedup={:.3} scalar_reference_seconds={:.6} source_scratch_bytes={} all565columns_identical=true",
        bounded_seconds / matrix_seconds,
        started.elapsed().as_secs_f64(),
        zk_x509_rfc_aux_replay_scratch_bytes_v1()
    );
}
