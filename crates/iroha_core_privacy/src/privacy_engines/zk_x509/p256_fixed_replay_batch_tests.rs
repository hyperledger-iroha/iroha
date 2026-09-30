//! Bounded public arithmetic fixed replay with unchanged verifier schedules.
use super::*;

#[test]
fn fixed_arithmetic_batch_rejects_bad_ranges_and_nonarithmetic_owners() {
    let fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let arithmetic = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
    for (count, first, rows) in [
        (0, 0, 8),
        (9, 0, 8),
        (1, usize::MAX, 8),
        (1, 134, 8),
        (1, 0, 8),
    ] {
        let mut columns = vec![vec![F(97); rows]; count];
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        assert_eq!(
            fixed.fill_arithmetic_fixed_columns_v1(arithmetic, first, &mut targets),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.iter().flatten().all(|value| *value == F(97)));
    }
    for adapter in [
        P256MainAdapterV1::ValueBus,
        P256MainAdapterV1::BindingSink,
        P256MainAdapterV1::WindowBatch,
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, 0).unwrap();
        let mut output = [F(97); 8];
        assert_eq!(
            fixed.fill_arithmetic_fixed_columns_v1(registration, 0, &mut [&mut output]),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert_eq!(output, [F(97); 8]);
    }
}

#[test]
#[ignore = "every fixed field at every native row for both verifier-owned role schedules"]
fn fixed_arithmetic_batches_match_full_rows_scalar_columns_and_parallel_coefficients() {
    let fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let root = crate::privacy_engines::transparent_stark::goldilocks_primitive_root_v1(19).unwrap();
    for index in [0, P256_X5S1_SIGNATURES_V1 - 1] {
        let registration =
            P256MainRegistrationV1::new_v1(index, P256MainAdapterV1::Arithmetic, 0).unwrap();
        let shape = registration.shape_v1().unwrap();
        for first in (0..shape.fixed_width).step_by(8) {
            let count = 8.min(shape.fixed_width - first);
            let mut columns = vec![vec![F::ZERO; shape.trace_size]; count];
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            fixed
                .fill_arithmetic_fixed_columns_v1(registration, first, &mut targets)
                .unwrap();
            drop(targets);
            for row in 0..shape.trace_size {
                let expected = fixed
                    .arithmetic_v1(registration.role_v1())
                    .row_v1(row)
                    .unwrap();
                for offset in 0..count {
                    assert_eq!(columns[offset][row], expected[first + offset]);
                }
            }
            for selected in [0, shape.fixed_width / 2, shape.fixed_width - 1] {
                if (first..first + count).contains(&selected) {
                    let mut scalar = vec![F::ZERO; shape.trace_size];
                    fixed
                        .fill_fixed_column_v1(registration, selected, &mut scalar)
                        .unwrap();
                    assert_eq!(columns[selected - first], scalar);
                    crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(
                        &mut scalar,
                        root,
                    )
                    .unwrap();
                    crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(
                        &mut columns[selected - first],
                        root,
                    )
                    .unwrap();
                    assert_eq!(columns[selected - first], scalar);
                }
            }
        }
    }
    assert_eq!(5 * 134 * (1 << 19), 351_272_960);
    assert_eq!(5 * 134_usize.div_ceil(8) * (1 << 19), 44_564_480);
}
