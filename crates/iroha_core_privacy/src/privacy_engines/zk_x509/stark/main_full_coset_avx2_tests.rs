//! Real production DIF dispatch, independent scalar parity and clearing controls.

use super::*;
use crate::privacy_engines::transparent_stark::dif_test_counts::Counts;

/// Public expected iteration counts; separate from the actual native loop census.
fn expected_counts(rows: usize, prefix: usize) -> Counts {
    assert!(rows.is_power_of_two() && prefix > 0 && prefix <= rows);
    let mut count = Counts::default();
    let mut nodes = 1;
    let mut half = rows / 2;
    while half != 0 {
        let pairs = prefix.min(half);
        count.vector_blocks += nodes * (pairs / 4);
        count.scalar_pairs += nodes * (pairs % 4);
        half /= 2;
        nodes *= 2;
    }
    count
}

fn cpu_evaluator(common: u8) -> Result<MainTraceCosetEvaluatorV1, AggregateStarkErrorV1> {
    Ok(MainTraceCosetEvaluatorV1 {
        coset_powers: None,
        evaluation_rows: 1usize
            .checked_shl(u32::from(common))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        device_columns: 0,
        receipt: MainTransformReceiptV1 {
            observe_full_cpu_routes: true,
            ..MainTransformReceiptV1::default()
        },
    })
}

/// Exercise the real evaluator, with a private test-only scalar-reference choice.
fn observed_batch(
    columns: &[Column],
    native_log: u8,
    common_log: u8,
    force_scalar: bool,
    selected: Option<&[usize]>,
) -> Result<(Vec<Column>, Counts), AggregateStarkErrorV1> {
    let mut evaluator = cpu_evaluator(common_log)?;
    evaluator.receipt.force_scalar_full_cpu = force_scalar;
    let outputs = evaluator.evaluate_rows_v1(columns, native_log, common_log, selected)?;
    Ok((
        outputs,
        Counts {
            vector_blocks: evaluator.receipt.full_avx2_vector_blocks,
            scalar_pairs: evaluator.receipt.full_avx2_scalar_pairs,
        },
    ))
}

fn coefficients(length: usize, column: usize) -> Column {
    let mut values = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    values.try_reserve_exact(length).unwrap();
    assert_eq!(values.capacity(), length);
    for i in 0..length {
        let value = match (i + column) % 7 {
            0 => F::ZERO,
            1 => F(GOLDILOCKS_MODULUS_V1 - 1),
            _ => F::reduce((i as u128 + 17) * (column as u128 + 31) * 0x9e3779b97f4a7c15),
        };
        values.push(value);
    }
    if let Some(last) = values.last_mut() {
        *last = F(7 + column as u64);
    }
    Column::from_vec_v1(values.into_vec())
}

#[test]
#[ignore = "mandatory native DIF owner/control; real production arithmetic"]
fn required_avx2_dif_owner_matches_scalar_dense_and_refusal_boundaries() {
    let native = FullPrefixBackendV1::detect_v1();
    assert!(native.is_native_v1(), "simd and actual AVX2 required");
    let mut total = Counts::default();
    for workers in [1, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for common in 2..=10 {
                let rows = 1usize << common;
                for length in [
                    1,
                    2,
                    3,
                    rows / 2 - 1,
                    rows / 2,
                    rows / 2 + 1,
                    rows - 1,
                    rows,
                ]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                {
                    if length == 0 || length > rows {
                        continue;
                    }
                    let source = coefficients(length, 3);
                    let expected = evaluate_full_scalar_v1(&source, 1, common).unwrap();
                    let (actual, count) = evaluate_full_counted_with_v1(
                        &source,
                        1,
                        common,
                        native,
                        |_| Ok(()),
                        |_| Ok(()),
                    )
                    .unwrap();
                    assert_eq!(&*actual, &*expected);
                    assert_eq!(count, expected_counts(rows, length));
                    total = total.plus(count);
                    let dense = Column::from_vec_v1(
                        masked_trace_coefficients_on_coset_v1(&source, 1, common).unwrap(),
                    );
                    assert_eq!(&*actual, &*dense);
                    let root = goldilocks_primitive_root_v1(common).unwrap();
                    for row in [0, 1, rows / 2, rows - 1] {
                        assert_eq!(
                            actual[row],
                            horner(
                                &source,
                                F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128))
                            )
                        );
                    }
                }
            }
            // Cross the original Rayon1024-pair and8192-pair thresholds, with
            // non-four-aligned complete masks and a nonzero final coefficient.
            for length in [1023, 1024, 1025, 8191, 8192, 8193, 16385] {
                let source = coefficients(length, 5);
                let expected = evaluate_full_scalar_v1(&source, 5, 16).unwrap();
                let (actual, count) =
                    evaluate_full_counted_with_v1(&source, 5, 16, native, |_| Ok(()), |_| Ok(()))
                        .unwrap();
                assert_eq!(&*actual, &*expected);
                assert_eq!(count, expected_counts(1 << 16, length));
                total = total.plus(count);
            }
        });
    }
    for (source, native_log, common) in [
        (vec![], 4, 6),
        (vec![F::ONE; 65], 4, 6),
        (vec![F::ONE], 6, 6),
        (vec![F::ONE], 7, 6),
        (vec![F::ONE], 255, 6),
        (vec![F::ONE], 4, 255),
        (vec![F::ONE], 4, 33),
        (vec![F(GOLDILOCKS_MODULUS_V1)], 6, 6),
        (vec![F(GOLDILOCKS_MODULUS_V1)], 4, 6),
    ] {
        let source = Column::from_vec_v1(source);
        let expected = evaluate_full_scalar_v1(&source, native_log, common)
            .err()
            .unwrap();
        let (actual, erased) = inspection::observe_v1(|| {
            evaluate_full_counted_with_v1(
                &source,
                native_log,
                common,
                native,
                |_| panic!("invalid input reached private allocation"),
                |_| panic!("invalid input reached transform"),
            )
        });
        assert_eq!(actual.err().unwrap(), expected);
        assert!(erased.is_empty());
    }
    assert!(total.vector_blocks > 0);
    println!(
        "dif_owner_control vector_blocks={} scalar_pairs={} parity=exact",
        total.vector_blocks, total.scalar_pairs
    );
}

#[test]
#[ignore = "mandatory native DIF ownership/fallback control; real production arithmetic"]
fn required_avx2_dif_keeps_allocation_erasure_and_original_fallback() {
    let native = FullPrefixBackendV1::detect_v1();
    assert!(native.is_native_v1(), "simd and actual AVX2 required");
    let source = coefficients(21, 0);
    let pointer = std::cell::Cell::new(core::ptr::null());
    let (_, cleared) = aggregate::field_column_erasure::observe_v1(|| {
        let (output, count) = evaluate_full_counted_with_v1(
            &source,
            4,
            6,
            native,
            |values| {
                pointer.set(values.as_ptr());
                Ok(())
            },
            |values| {
                assert_eq!(values.as_ptr(), pointer.get());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(output.as_ptr(), pointer.get());
        assert_eq!(output.len(), 64);
        assert_eq!(count, expected_counts(64, source.len()));
        drop(output);
    });
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].cells, 64);
    assert!(cleared[0].nonzero_before > 0 && cleared[0].nonzero_after == 0);
    for transformed in [false, true] {
        for unwind in [false, true] {
            let inject = |values: &[F]| {
                assert!(values.iter().any(|v| *v != F::ZERO));
                assert!(!unwind, "injected populated DIF owner unwind");
                Err(AggregateStarkErrorV1::AllocationFailure)
            };
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    evaluate_full_counted_with_v1(
                        &source,
                        4,
                        6,
                        native,
                        |values| if transformed { Ok(()) } else { inject(values) },
                        |values| if transformed { inject(values) } else { Ok(()) },
                    )
                }))
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(AggregateStarkErrorV1::AllocationFailure)
                ));
            }
            assert_eq!(erased.iter().map(|r| r.cells).sum::<usize>(), 64);
            assert!(erased.iter().any(|r| r.nonzero_before > 0));
            assert!(erased.iter().all(|r| r.nonzero_after == 0));
        }
    }
    // Native admission disabled is the actual unchanged scalar owner, not an
    // emulated vector result. Smaller widths and all selected queries use it too.
    assert!(!FullPrefixBackendV1::Scalar.is_native_v1());
    for width in 1..=8 {
        let inputs = (0..width)
            .map(|i| coefficients(17 + i, i))
            .collect::<Vec<_>>();
        for selected in [None, Some(&[0usize, 7, 31, 63][..])] {
            let (expected, scalar_count) = observed_batch(&inputs, 4, 6, true, selected).unwrap();
            let (actual, count) = observed_batch(&inputs, 4, 6, false, selected).unwrap();
            assert_eq!(scalar_count, Counts::default());
            assert!(actual.iter().zip(&expected).all(|(a, b)| &**a == &**b));
            assert_eq!(
                count,
                Counts::default(),
                "unmeasured common6 remains scalar"
            );
        }
    }
    for (width, native_log, common, bad_length) in [
        (0, 4, 6, None),
        (9, 4, 6, None),
        (8, 6, 6, None),
        (8, 255, 6, None),
        (8, 4, 255, None),
        (8, 4, 33, None),
        (8, 4, 6, Some(0)),
        (8, 4, 6, Some(65)),
    ] {
        let mut inputs = (0..width).map(|i| coefficients(21, i)).collect::<Vec<_>>();
        if let Some(length) = bad_length {
            inputs[3] = coefficients(length, 3);
        }
        let expected = observed_batch(&inputs, native_log, common, true, None)
            .err()
            .unwrap();
        let (actual, erased) =
            inspection::observe_v1(|| observed_batch(&inputs, native_log, common, false, None));
        assert_eq!(actual.err().unwrap(), expected);
        assert!(erased.is_empty());
    }
    for bad in [0, 4, 7] {
        let mut inputs = (0..8).map(|i| coefficients(21, i)).collect::<Vec<_>>();
        inputs[bad] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1); 21]);
        let expected = observed_batch(&inputs, 4, 6, true, None).err().unwrap();
        let (actual, erased) =
            inspection::observe_v1(|| observed_batch(&inputs, 4, 6, false, None));
        assert_eq!(actual.err().unwrap(), expected);
        assert!(erased.is_empty());
    }
}

#[test]
fn dif_scalar_fallback_uses_original_owner_without_native_admission() {
    let disabled = FullPrefixBackendV1::Scalar;
    assert!(!disabled.is_native_v1());
    if !cfg!(feature = "simd") || !std::is_x86_feature_detected!("avx2") {
        assert!(!FullPrefixBackendV1::detect_v1().is_native_v1());
    }
    let source = coefficients(21, 0);
    let expected = evaluate_full_scalar_v1(&source, 4, 6).unwrap();
    let (actual, counts) =
        evaluate_full_counted_with_v1(&source, 4, 6, disabled, |_| Ok(()), |_| Ok(())).unwrap();
    assert_eq!(&*actual, &*expected);
    assert_eq!(counts, Counts::default());
    assert_eq!(
        evaluate_full_counted_with_v1(
            &[],
            4,
            6,
            disabled,
            |_| panic!("invalid fallback allocated"),
            |_| unreachable!()
        )
        .err(),
        evaluate_full_scalar_v1(&[], 4, 6).err(),
    );
}

#[test]
#[ignore = "mandatory common22 DIF exact outputs and eight-column budget; no timing"]
fn required_avx2_dif_registered_common22_outputs_fit_original_funding() {
    let native = FullPrefixBackendV1::detect_v1();
    assert!(native.is_native_v1(), "simd and actual AVX2 required");
    let (groups, rows, _, replay, runtime) =
        ZkX509MainRegistrationShapeV1::row_hash_diagnostic_geometry_v1().unwrap();
    assert_eq!(rows, 1 << 22);
    assert_eq!(runtime, 1 << 30);
    assert_eq!(
        groups.iter().map(|g| g.0).collect::<Vec<_>>(),
        [5, 8, 15, 16, 18, 19]
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    let mut total = Counts::default();
    pool.install(|| {
        for (log,_,_) in groups {
            let source=coefficients((1usize<<log)+MASK_DEGREE+1,0);
            // At most twoN-cell outputs plus one coefficient column. This exact
            // common22 oracle never lives beside a full eight-column candidate.
            assert!(2*rows*core::mem::size_of::<F>()+source.len()*core::mem::size_of::<F>()<8*rows*core::mem::size_of::<F>());
            assert!(8*rows*core::mem::size_of::<F>()<=replay);
            let expected=evaluate_full_scalar_v1(&source,log,22).unwrap();
            let (actual,count)=evaluate_full_counted_with_v1(&source,log,22,native,|_|Ok(()),|_|Ok(())).unwrap();
            assert_eq!(&*actual,&*expected);
            assert_eq!(count,expected_counts(rows,source.len()));
            total=total.plus(count);
            drop(actual);
            let dense=Column::from_vec_v1(masked_trace_coefficients_on_coset_v1(&source,log,22).unwrap());
            assert_eq!(&*expected,&*dense);
            println!("dif_registered_control native_log={log} common_log=22 vector_blocks={} scalar_pairs={} parity=exact",count.vector_blocks,count.scalar_pairs);
        }
    });
    assert!(total.vector_blocks > 0);
}

/// Compare public outputs sequentially without retaining a second8N matrix.
fn output_digest(columns: &[Column]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    for column in columns {
        for value in column.iter() {
            hasher.update(&value.0.to_le_bytes());
        }
    }
    hasher.finalize()
}

#[test]
#[ignore = "mandatory actual full8 production dispatch, scalar parity and original budget"]
fn required_avx2_dif_actual_full_batch_dispatch_keeps_original_outputs() {
    assert!(
        FullPrefixBackendV1::detect_v1().is_native_v1(),
        "simd and actual AVX2 required"
    );
    let (groups, rows, _, replay, runtime) =
        ZkX509MainRegistrationShapeV1::row_hash_diagnostic_geometry_v1().unwrap();
    assert_eq!(rows, 1 << 22);
    assert_eq!(runtime, 1 << 30);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    pool.install(|| {
        for (log, _, _) in groups {
            let length = (1usize << log) + MASK_DEGREE + 1;
            let inputs = (0..8).map(|i| coefficients(length, i)).collect::<Vec<_>>();
            assert!(8 * (rows + length) * core::mem::size_of::<F>() <= replay);
            let (reference, scalar_counts) = observed_batch(&inputs, log, 22, true, None).unwrap();
            assert_eq!(scalar_counts, Counts::default());
            let expected = output_digest(&reference);
            drop(reference);
            let (actual, counts) = observed_batch(&inputs, log, 22, false, None).unwrap();
            assert_eq!(actual.len(), 8);
            assert!(actual.iter().all(|column| column.len() == rows));
            assert_eq!(output_digest(&actual), expected);
            let per_column = expected_counts(rows, length);
            assert_eq!(counts.vector_blocks, 8 * per_column.vector_blocks);
            assert_eq!(counts.scalar_pairs, 8 * per_column.scalar_pairs);
            let ((), erased) = aggregate::field_column_erasure::observe_v1(|| drop(actual));
            assert_eq!(erased.len(), 8);
            assert!(erased.iter().all(|entry| entry.cells == rows
                && entry.nonzero_before > 0 && entry.nonzero_after == 0));
            // The real unobserved entry keeps the ordinary for_each traversal.
            // Compare it too, after releasing both prior8N outputs.
            let mut unobserved = cpu_evaluator(22).unwrap();
            unobserved.receipt.observe_full_cpu_routes = false;
            let actual = unobserved.evaluate_v1(&inputs, log, 22).unwrap();
            assert_eq!(output_digest(&actual), expected);
            assert_eq!(unobserved.receipt.full_avx2_vector_blocks, 0);
            assert_eq!(unobserved.receipt.full_avx2_scalar_pairs, 0);
            drop(actual);
            println!("dif_production_batch_control native_log={log} common_log=22 width=8 vector_blocks={} scalar_pairs={} counted_and_unobserved_parity=exact", counts.vector_blocks, counts.scalar_pairs);
        }
        // A single different masked length leaves the entire valid batch scalar.
        let length = (1 << 5) + MASK_DEGREE + 1;
        let mut mixed = (0..8).map(|i| coefficients(length, i)).collect::<Vec<_>>();
        mixed[7] = coefficients(length + 1, 7);
        let (reference, _) = observed_batch(&mixed, 5, 22, true, None).unwrap();
        let expected = output_digest(&reference);
        drop(reference);
        let (actual, counts) = observed_batch(&mixed, 5, 22, false, None).unwrap();
        assert_eq!(counts, Counts::default());
        assert_eq!(output_digest(&actual), expected);
        drop(actual);
        // Selected rows remain outside full-prefix native admission even when
        // all complete source lengths match the measured geometry.
        mixed[7] = coefficients(length, 7);
        let selected = [0, rows - 1];
        let (actual, counts) = observed_batch(&mixed, 5, 22, false, Some(&selected)).unwrap();
        assert_eq!(counts, Counts::default());
        let root = goldilocks_primitive_root_v1(22).unwrap();
        for (source, actual) in mixed.iter().zip(&actual) {
            for (&row, &value) in selected.iter().zip(actual.iter()) {
                assert_eq!(value, horner(source, F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128))));
            }
        }
        drop(actual);
        // Complete all-column admission must reject even the final malformed
        // cell before native routing or a private evaluation allocation.
        mixed[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1); length]);
        let mut evaluator = cpu_evaluator(22).unwrap();
        let before = evaluator.receipt;
        let (result, erased) = inspection::observe_v1(|| evaluator.evaluate_v1(&mixed, 5, 22));
        assert!(matches!(result, Err(AggregateStarkErrorV1::NonCanonicalField)));
        assert!(erased.is_empty());
        assert_eq!(evaluator.receipt, before);
    });
}
