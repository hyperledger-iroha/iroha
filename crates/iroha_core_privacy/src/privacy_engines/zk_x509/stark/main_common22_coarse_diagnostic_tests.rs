//! CPU scheduling diagnostic; synthetic polynomials confer no credential authority.

use super::*;
use crate::privacy_engines::transparent_stark::{
    TransparentStarkErrorV1, masked_trace_coefficients_on_coset_coarse_for_test_v1,
};

fn coefficients(native: u8, count: usize) -> Vec<Column> {
    let length = (1_usize << native) + MASK_DEGREE + 1;
    (0..count)
        .map(|column| {
            Column::from_vec_v1(
                (0..length)
                    .map(|index| F(u64::try_from(1 + index * 37 + column * 101).unwrap()))
                    .collect(),
            )
        })
        .collect()
}

fn coarse(columns: &[Column], native: u8, common: u8) -> Vec<Column> {
    columns
        .par_iter()
        .map(|column| {
            Column::from_vec_v1(
                masked_trace_coefficients_on_coset_coarse_for_test_v1(column, native, common)
                    .unwrap(),
            )
        })
        .collect()
}

fn original(columns: &[Column], native: u8, common: u8) -> Vec<Column> {
    let mut evaluator = MainTraceCosetEvaluatorV1 {
        evaluation_rows: 1 << common,
        device_columns: 0,
        receipt: MainTransformReceiptV1::default(),
    };
    let result = evaluator.evaluate_v1(columns, native, common).unwrap();
    assert_eq!(evaluator.receipt_v1().cpu_columns, columns.len());
    assert_eq!(evaluator.receipt_v1().metal_columns, 0);
    result
}

#[test]
fn coarse_coset_retains_original_domain_and_field_refusals() {
    let invalid = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
    for (values, native, common, expected) in [
        (vec![], 2, 4, TransparentStarkErrorV1::InvalidDomain),
        (
            vec![F::ONE; 17],
            2,
            4,
            TransparentStarkErrorV1::InvalidDomain,
        ),
        (vec![F::ONE], 4, 4, TransparentStarkErrorV1::InvalidDomain),
        (vec![F::ONE], 5, 4, TransparentStarkErrorV1::InvalidDomain),
        (
            vec![F::ONE],
            u8::MAX,
            4,
            TransparentStarkErrorV1::InvalidDomain,
        ),
        (
            vec![F::ONE],
            2,
            u8::MAX,
            TransparentStarkErrorV1::InvalidDomain,
        ),
        (
            vec![invalid],
            2,
            4,
            TransparentStarkErrorV1::NonCanonicalField,
        ),
    ] {
        assert_eq!(
            masked_trace_coefficients_on_coset_v1(&values, native, common),
            Err(expected)
        );
        assert_eq!(
            masked_trace_coefficients_on_coset_coarse_for_test_v1(&values, native, common),
            Err(expected)
        );
    }
}

#[test]
fn coarse_coset_matches_full_original_outputs_across_threads_and_batch_tails() {
    for threads in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            for native in [5, 11] {
                for width in [1, 3, 8] {
                    let input = coefficients(native, width);
                    let expected = original(&input, native, 14);
                    let actual = coarse(&input, native, 14);
                    for (actual, expected) in actual.iter().zip(expected.iter()) {
                        assert_eq!(actual.as_ref(), expected.as_ref());
                    }
                }
            }
        });
    }
}

#[test]
#[ignore = "full common22 CPU scheduling parity/timing; coordinate an isolated 20-worker native run"]
fn common22_original_and_coarse_batches_report_exact_output_parity_and_timings() {
    use std::collections::BTreeSet;
    use std::time::Instant;
    assert_eq!(
        rayon::current_num_threads(),
        20,
        "same worker count as maximum4"
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    assert_eq!(layout.common_lde_log2, 22);
    let native_logs = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<BTreeSet<_>>();
    assert_eq!(native_logs, BTreeSet::from([5, 8, 15, 16, 18, 19]));
    for (group_index, native) in native_logs.into_iter().enumerate() {
        for (batch_index, width) in [1, 4, 8].into_iter().enumerate() {
            let input = coefficients(native, width);
            // Alternate fixed public order. Each measurement uses the same
            // pool and source values; compare every output, not a digest sample.
            let coarse_first = (group_index + batch_index) % 2 == 1;
            let run = |use_coarse| {
                let start = Instant::now();
                let result = if use_coarse {
                    coarse(&input, native, 22)
                } else {
                    original(&input, native, 22)
                };
                (result, start.elapsed().as_nanos())
            };
            let (first, first_ns) = run(coarse_first);
            let (second, second_ns) = run(!coarse_first);
            assert_eq!(first.len(), width);
            assert_eq!(second.len(), width);
            for (left, right) in first.iter().zip(second.iter()) {
                assert_eq!(left.len(), 1 << 22);
                assert_eq!(left.as_ref(), right.as_ref());
            }
            let (original_ns, coarse_ns) = if coarse_first {
                (second_ns, first_ns)
            } else {
                (first_ns, second_ns)
            };
            let initialized_field_bytes = input
                .iter()
                .chain(first.iter())
                .chain(second.iter())
                .map(|column| column.len() * core::mem::size_of::<F>())
                .sum::<usize>();
            assert!(
                initialized_field_bytes < 1 << 30,
                "initialized field extent of the two output batches plus input"
            );
            eprintln!(
                "COMMON22_COARSE_DIAGNOSTIC_V1 native_log={native} common_log=22 width={width} workers=20 coarse_first={coarse_first} original_ns={original_ns} coarse_ns={coarse_ns} initialized_field_bytes={initialized_field_bytes} parity=exact"
            );
        }
    }
}
