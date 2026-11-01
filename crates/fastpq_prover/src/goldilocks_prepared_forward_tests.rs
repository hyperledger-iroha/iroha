//! The wider forward-only admission retains all ordinary and inverse limits.

use super::*;

#[test]
fn prepared_forward_public_budget_and_cpu_admission_preserve_closed_limits() {
    assert_eq!(MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1, 8);
    assert_eq!(MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1, 16);
    for rows in [2usize, 32, 1 << 19] {
        let root = power_v1(7, (FIELD_MODULUS - 1) / rows as u64);
        for width in [1, 4, 8, 9, 16] {
            let payload = rows * width * 8;
            let expected = payload
                + width * (METAL_PAGE_BYTES - 1)
                + METAL_POOL_MAX_CACHED_BYTES
                + crate::gpu_memory::METAL_TWIDDLE_PAYLOAD_ALLOWANCE
                + (1 << 20);
            assert_eq!(
                metal_goldilocks_forward_extra_payload_v1(rows, width).unwrap(),
                expected
            );
            assert_eq!(
                prepared_goldilocks_forward_payload_bound_v1(rows, width).unwrap(),
                payload.div_ceil(METAL_PAGE_BYTES) * METAL_PAGE_BYTES
                    + 256
                    + size_of::<PreparedGoldilocksColumnsV1>()
            );
            if rows > 32 {
                continue;
            }
            let original = vec![vec![FIELD_MODULUS - 1; rows]; width];
            let views = original.iter().map(Vec::as_slice).collect::<Vec<_>>();
            assert!(matches!(
                prepare_goldilocks_forward_columns_v1(&views, root, ExecutionMode::Cpu),
                Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
            ));
            for direction in [
                GoldilocksTransformDirectionV1::Forward,
                GoldilocksTransformDirectionV1::Inverse,
            ] {
                let old =
                    prepare_goldilocks_columns_v1(&views, root, direction, ExecutionMode::Cpu);
                if width <= 8 {
                    assert!(matches!(old, Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)));
                } else {
                    assert_eq!(old.err(), Some(GoldilocksTransformErrorV1::InvalidShape));
                }
            }
            assert!(
                original
                    .iter()
                    .flatten()
                    .all(|&value| value == FIELD_MODULUS - 1)
            );
            if width > 8 {
                assert!(metal_goldilocks_transform_extra_payload_v1(rows, width).is_err());
                assert!(prepared_goldilocks_columns_payload_bound_v1(rows, width).is_err());
            }
        }
    }
    for (rows, width) in [(0, 1), (1, 1), (3, 1), (8, 0), (8, 17), (usize::MAX, 16)] {
        assert!(metal_goldilocks_forward_extra_payload_v1(rows, width).is_err());
        assert!(prepared_goldilocks_forward_payload_bound_v1(rows, width).is_err());
    }
}

#[test]
fn prepared_forward_refusals_check_every_column_before_any_cpu_or_device_route() {
    let root = power_v1(7, (FIELD_MODULUS - 1) / 8);
    for fault in 0..5 {
        let mut columns = vec![vec![7; 8]; 16];
        let mut actual_root = root;
        let expected = match fault {
            0 => {
                columns.push(vec![FIELD_MODULUS; 8]);
                actual_root = 0;
                GoldilocksTransformErrorV1::InvalidShape
            }
            1 => {
                columns[15].pop();
                GoldilocksTransformErrorV1::InvalidShape
            }
            2 => {
                actual_root = 0;
                columns[15][7] = FIELD_MODULUS;
                GoldilocksTransformErrorV1::InvalidRoot
            }
            3 => {
                columns[0][0] = FIELD_MODULUS;
                GoldilocksTransformErrorV1::NonCanonicalInput
            }
            _ => {
                columns[15][7] = u64::MAX;
                GoldilocksTransformErrorV1::NonCanonicalInput
            }
        };
        let before = columns.clone();
        for execution in [ExecutionMode::Cpu, ExecutionMode::Auto, ExecutionMode::Gpu] {
            assert_eq!(
                prepare_goldilocks_forward_columns_v1(
                    &columns.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    actual_root,
                    execution
                )
                .err(),
                Some(expected.clone())
            );
            assert_eq!(columns, before);
        }
    }
    assert_eq!(CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1, 1 << 24);
}
