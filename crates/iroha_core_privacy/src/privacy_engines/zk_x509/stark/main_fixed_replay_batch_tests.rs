//! Exact-source SHA and arithmetic fixed-polynomial parity after bounded public replay.
use super::*;
use crate::privacy_engines::zk_x509::{
    main_assembly::build_zk_x509_main_trace_assembly_v1,
    relation::{
        ZkX509GovernanceV1,
        release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
    },
};
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError,
};

fn inverse_fixture_v1(
    words: &mut [Vec<u64>],
    root: u64,
    direction: Direction,
) -> Result<Backend, TransformError> {
    assert_eq!(direction, Direction::Inverse);
    for column in words {
        let mut values = column.iter().map(|&word| F(word)).collect::<Vec<_>>();
        goldilocks_ifft_v1(&mut values, F(root)).unwrap();
        for (word, value) in column.iter_mut().zip(values) {
            *word = value.0;
        }
    }
    Ok(Backend::Metal)
}

#[test]
fn initial_fixed_inverse_keeps_137_column_order_and_matches_direct_inverse_dft() {
    let rows = 16;
    let root = goldilocks_primitive_root_v1(4).unwrap().pow(3);
    let original = (0..137)
        .map(|column| {
            (0..rows)
                .map(|row| F((column * 79 + row * 13 + 3) as u64))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let inverse = root.inv().unwrap();
    let scale = F(rows as u64).inv().unwrap();
    let expected = original
        .iter()
        .map(|column| {
            (0..rows)
                .map(|degree| {
                    column
                        .iter()
                        .enumerate()
                        .fold(F::ZERO, |sum, (row, value)| {
                            sum.add(value.mul(inverse.pow((row * degree) as u128)))
                        })
                        .mul(scale)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for device in [false, true] {
        let mut actual = original.clone();
        let storage = actual
            .iter()
            .map(|column| (column.as_ptr(), column.capacity()))
            .collect::<Vec<_>>();
        let policy = if device {
            main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(rows, 8)
        } else {
            main_bounded_transform::MainBoundedTransformPolicyV1::cpu_v1()
        };
        let mut batches = Vec::new();
        main_initial_fixed_coefficients_with_v1(
            &mut actual,
            root,
            policy,
            |words, root, direction| {
                batches.push(words.len());
                inverse_fixture_v1(words, root, direction)
            },
            || false,
        )
        .unwrap();
        assert_eq!(actual, expected);
        for (column, original) in actual.iter().zip(storage) {
            assert_eq!((column.as_ptr(), column.capacity()), original);
        }
        if device {
            assert_eq!(batches, [vec![8; 17], vec![1]].concat());
        } else {
            assert!(batches.is_empty());
        }
    }
}

#[test]
fn initial_fixed_inverse_validates_every_column_and_exact_root_before_mutation() {
    let root = goldilocks_primitive_root_v1(4).unwrap();
    for failure in 0..8 {
        let mut columns = vec![vec![F(19); 16]; 137];
        let mut supplied_root = root;
        match failure {
            0 => columns.clear(),
            1 => {
                columns[136].pop();
            }
            2 => {
                columns[136][15] =
                    F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1)
            }
            3 => supplied_root = F::ZERO,
            4 => supplied_root = F::ONE,
            5 => supplied_root = root.mul(root),
            6 => supplied_root = goldilocks_primitive_root_v1(5).unwrap(),
            _ => {
                supplied_root = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1)
            }
        }
        let before = columns.clone();
        assert!(
            main_initial_fixed_coefficients_with_v1(
                &mut columns,
                supplied_root,
                main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(16, 8),
                |_, _, _| panic!("invalid full matrix dispatched a prefix"),
                || false,
            )
            .is_err()
        );
        assert_eq!(columns, before);
    }
    let mut columns = vec![vec![F(19); 16]; 137];
    let before = columns.clone();
    assert!(
        main_initial_fixed_coefficients_with_v1(
            &mut columns,
            root,
            main_bounded_transform::MainBoundedTransformPolicyV1::cpu_v1(),
            |_, _, _| panic!("uncertain completion dispatched"),
            || true,
        )
        .is_err()
    );
    assert_eq!(columns, before);
}

fn with_maximum_fixed_source_v1(
    consume: impl FnOnce(
        &AggregateProofLayoutV1,
        &MainLog19BoundTraceGroupSourceV1<'_>,
        main_bounded_transform::MainBoundedTransformPolicyV1,
    ),
) {
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
        core::array::from_fn(|index| {
            digest(u8::try_from(index).expect("fixture value fits u8") + 1)
        }),
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
        &layout, &assembly, sha, p256, &binding,
    )
    .unwrap();

    let policy = main_bounded_transform::MainBoundedTransformPolicyV1::for_assembly_v1(
        &layout,
        assembly.allocated_payload_bytes_v1(),
    )
    .unwrap();
    consume(&layout, &source, policy);
}

#[test]
#[ignore = "all five maximum-credential arithmetic fixed polynomial sets against independent scalar interpolation"]
fn maximum_arithmetic_fixed_sets_preserve_every_coefficient_and_registration() {
    with_maximum_fixed_source_v1(|layout, source, policy| {
        let prover = MainLog19ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
        let observation =
            crate::privacy_engines::zk_x509::prover_observation::ObservationV1::begin_v1();
        let mut checked = 0;
        for registration in source.registrations.iter().copied() {
            if registration.segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
                continue;
            }
            let root = goldilocks_primitive_root_v1(registration.segment.trace_log2).unwrap();
            let actual = main_log19_fixed_polynomial_set_v1(&prover, registration, policy).unwrap();
            assert_eq!(actual.registration, registration);
            assert_eq!(actual.columns.len(), registration.segment.fixed_width);
            for (column, coefficients) in actual.columns.iter().enumerate() {
                let mut expected = source.native_fixed_column_v1(registration, column).unwrap();
                goldilocks_ifft_v1(&mut expected, root).unwrap();
                assert_eq!(coefficients.as_slice(), &*expected);
            }
            let mut invalid = registration;
            invalid.segment.fixed_width -= 1;
            assert!(main_log19_fixed_polynomial_set_v1(&prover, invalid, policy).is_err());
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
    });
}

#[test]
#[ignore = "four complete maximum-credential SHA fixed polynomial sets against the prior per-column reconstruction and independent scalar interpolation"]
fn maximum_sha_fixed_sets_preserve_every_coefficient_and_registration() {
    with_maximum_fixed_source_v1(|layout, source, policy| {
        let prover = MainLog19ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
        let observation =
            crate::privacy_engines::zk_x509::prover_observation::ObservationV1::begin_v1();
        let mut checked = 0;
        for registration in source.registrations.iter().copied() {
            if registration.segment.adapter != SegmentAdapterIdV1::Sha256CallBus {
                continue;
            }
            let root = goldilocks_primitive_root_v1(registration.segment.trace_log2).unwrap();
            let actual = main_log19_fixed_polynomial_set_v1(&prover, registration, policy).unwrap();
            assert_eq!(actual.registration, registration);
            assert_eq!(actual.columns.len(), registration.segment.fixed_width);
            assert_eq!(actual.columns.len(), 137);
            for (column, coefficients) in actual.columns.iter().enumerate() {
                // Exact previous production path; retain every native coefficient.
                let mut expected = source.native_fixed_column_v1(registration, column).unwrap();
                goldilocks_ifft_v1(&mut expected, root).unwrap();
                assert_eq!(coefficients.as_slice(), &*expected);
            }
            let mut invalid = registration;
            invalid.segment.fixed_width -= 1;
            assert!(main_log19_fixed_polynomial_set_v1(&prover, invalid, policy).is_err());
            checked += 1;
        }
        assert_eq!(checked, 4);
        let receipt = observation.finish_v1().public_text_v1();
        for phase in [
            "CompositionShaFixedRows",
            "CompositionShaFixedInverseTransform",
        ] {
            assert!(receipt.contains(&format!(
                "phase={phase} calls=4 completed=4 interrupted=0 unwound=0"
            )));
        }
        eprintln!("{receipt}");
    });
}

#[test]
#[ignore = "full137-column public SHA fixed setup cost with independent scalar interpolation"]
fn initial_sha_fixed_inverse_cost_preserves_every_native_coefficient() {
    use std::time::Instant;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let rows = 1usize << 19;
    let root = goldilocks_primitive_root_v1(19).unwrap();
    let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(
        crate::privacy_engines::zk_x509::sha_call_bus_stark::ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        },
    )
    .unwrap();
    let mut native =
        ZeroizingBaseColumnsV1(vec![vec![F::ZERO; rows]; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]);
    fixed.fill_fixed_matrix_v1(0, &mut native.0).unwrap();
    let policy =
        main_bounded_transform::MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 215_244_730)
            .unwrap();
    for round in 0..3 {
        let mut scalar = ZeroizingBaseColumnsV1(native.0.clone());
        let mut bounded = ZeroizingBaseColumnsV1(native.0.clone());
        let mut elapsed = [0; 2];
        let mut metal_columns = 0;
        for kind in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let start = Instant::now();
            if kind == 0 {
                for column in &mut scalar.0 {
                    goldilocks_ifft_v1(column, root).unwrap();
                }
            } else {
                main_initial_fixed_coefficients_with_v1(&mut bounded.0, root, policy, |words, root, direction| {
                    let backend = fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1(words, root, direction, fastpq_prover::ExecutionMode::Auto)?;
                    if backend == Backend::Metal { metal_columns += words.len(); }
                    Ok(backend)
                }, fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1).unwrap();
            }
            elapsed[kind] = start.elapsed().as_nanos();
        }
        assert_eq!(&*scalar, &*bounded);
        if policy.columns_v1(rows) > 0 {
            assert_eq!(metal_columns, 137);
        }
        eprintln!(
            "x509_initial_sha_fixed_inverse rows={rows} width=137 workers={} round={round} scalar_ns={} bounded_ns={} admitted_device_columns={} actual_metal_columns={metal_columns} parity=exact",
            rayon::current_num_threads(),
            elapsed[0],
            elapsed[1],
            policy.columns_v1(rows)
        );
    }
}
