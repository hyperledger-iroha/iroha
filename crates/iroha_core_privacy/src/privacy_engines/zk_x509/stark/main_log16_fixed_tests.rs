//! Final-owner Log16 interpolation, source order, refusal and complete setup cost.

use super::*;

struct RecordingTransformV1 {
    batches: Vec<usize>,
    failure: Option<usize>,
    unwind: bool,
    completion_after: Option<std::rc::Rc<std::cell::Cell<bool>>>,
}
impl main_native_transform::MainNativeTransformV1 for RecordingTransformV1 {
    fn inverse_v1(
        &mut self,
        _policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let index = self.batches.len();
        self.batches.push(columns.len());
        assert!(!columns.is_empty() && columns.len() <= 8);
        if self.failure == Some(index) {
            columns[0][0] = F(123);
            assert!(!self.unwind, "injected inverse failure");
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        for column in columns {
            goldilocks_ifft_v1(column, root).map_err(map_transparent_error_v1)?;
        }
        if let Some(flag) = &self.completion_after {
            flag.set(true);
        }
        Ok(())
    }
}
fn recording_v1() -> RecordingTransformV1 {
    RecordingTransformV1 {
        batches: Vec::new(),
        failure: None,
        unwind: false,
        completion_after: None,
    }
}
fn fill_v1(columns: &mut [Vec<F>]) {
    for (column, values) in columns.iter_mut().enumerate() {
        for (row, value) in values.iter_mut().enumerate() {
            *value = F((column * 73 + row * 17 + 1) as u64);
        }
    }
}

#[test]
fn log16_fixed_final_owners_match_independent_dft_and_keep_exact_allocations() {
    for width in [1, 7, 8, 9, 36, 47] {
        let rows = 16;
        let root = goldilocks_primitive_root_v1(4).unwrap().pow(3);
        let mut original = vec![vec![F::ZERO; rows]; width];
        fill_v1(&mut original);
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
                    })
                    .map(|value| value.mul(scale))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut allocations = Vec::new();
        let mut transform = recording_v1();
        let mut actual = dense_columns_with_v1(
            rows,
            width,
            root,
            MainBoundedTransformPolicyV1::cpu_v1(),
            |columns| {
                allocations = columns.iter().map(|v| (v.as_ptr(), v.capacity())).collect();
                fill_v1(columns);
                Ok(())
            },
            &mut transform,
            || false,
        )
        .unwrap();
        assert_eq!(actual.0, expected);
        assert_eq!(
            actual
                .0
                .iter()
                .map(|v| (v.as_ptr(), v.capacity()))
                .collect::<Vec<_>>(),
            allocations
        );
        assert!(actual.0.iter().all(|v| v.capacity() == rows));
        assert_eq!(
            transform.batches,
            (0..width)
                .step_by(8)
                .map(|first| (width - first).min(8))
                .collect::<Vec<_>>()
        );
        // This is the unchanged final owner whose Drop runs the same zeroizer
        // on every early return and unwind, including partial interpolation.
        zeroize::Zeroize::zeroize(&mut actual);
        assert!(actual.0.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            actual
                .0
                .iter()
                .map(|v| (v.as_ptr(), v.capacity()))
                .collect::<Vec<_>>(),
            allocations
        );
    }
}

#[test]
fn log16_fixed_refusals_precede_fill_and_dispatch_and_owners_stay_guarded() {
    let root = goldilocks_primitive_root_v1(4).unwrap();
    for (rows, width, root, uncertain, error) in [
        (0, 36, root, false, ZkX509StarkErrorV1::ProfileMismatch),
        (3, 36, root, false, ZkX509StarkErrorV1::ProfileMismatch),
        (16, 0, root, false, ZkX509StarkErrorV1::ProfileMismatch),
        (16, 138, root, false, ZkX509StarkErrorV1::ProfileMismatch),
        (16, 36, F::ONE, false, ZkX509StarkErrorV1::ProfileMismatch),
        (16, 36, F::ZERO, false, ZkX509StarkErrorV1::ProfileMismatch),
        (
            0,
            0,
            F::ZERO,
            true,
            ZkX509StarkErrorV1::AcceleratorCompletionUncertain,
        ),
    ] {
        let mut transform = recording_v1();
        let actual = dense_columns_with_v1(
            rows,
            width,
            root,
            MainBoundedTransformPolicyV1::cpu_v1(),
            |_| panic!("refusal must precede source"),
            &mut transform,
            || uncertain,
        );
        assert!(matches!(actual,Err(observed) if observed==error));
        assert!(transform.batches.is_empty());
    }
    for fault in 0..5 {
        let mut transform = recording_v1();
        if fault >= 2 {
            transform.failure = Some(1);
            transform.unwind = fault == 3;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dense_columns_with_v1(
                16,
                36,
                root,
                MainBoundedTransformPolicyV1::cpu_v1(),
                |columns| {
                    fill_v1(columns);
                    match fault {
                        0 => Err(ZkX509StarkErrorV1::InternalInvariant),
                        1 => {
                            columns[35][15] = F(
                                crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1,
                            );
                            Ok(())
                        }
                        4 => panic!("source unwind"),
                        _ => Ok(()),
                    }
                },
                &mut transform,
                || false,
            )
        }));
        assert_eq!(result.is_err(), matches!(fault, 3 | 4));
        if let Ok(result) = result {
            assert!(result.is_err());
        }
        if fault <= 1 {
            assert!(transform.batches.is_empty());
        }
    }
}

#[test]
fn log16_fixed_completion_refusals_after_fill_and_transform_prevent_publication() {
    let root = goldilocks_primitive_root_v1(4).unwrap();
    for after_transform in [false, true] {
        let uncertain = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut transform = recording_v1();
        if after_transform {
            transform.completion_after = Some(uncertain.clone());
        }
        let result = dense_columns_with_v1(
            16,
            8,
            root,
            MainBoundedTransformPolicyV1::cpu_v1(),
            |columns| {
                fill_v1(columns);
                if !after_transform {
                    // Terminal completion uncertainty also precedes a late
                    // malformed field, just as at the transform boundary.
                    columns[7][15] =
                        F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
                    uncertain.set(true);
                }
                Ok(())
            },
            &mut transform,
            || uncertain.get(),
        );
        assert!(matches!(
            result,
            Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
        ));
        assert_eq!(transform.batches.len(), usize::from(after_transform));
    }
}

#[test]
fn log16_fixed_binding_and_scratch_geometry_keep_original_budget() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let bindings = canonical_p256_main_log16_bindings_v1(&layout).unwrap();
    assert_eq!(bindings.len(), 10);
    for binding in bindings {
        validate_binding_v1(binding).unwrap();
        assert!(binding.main.segment.fixed_width <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1);
        let mut changed = binding;
        changed.main.segment.fixed_width -= 1;
        assert!(validate_binding_v1(changed).is_err());
        let mut changed = binding;
        changed.main.segment.trace_log2 = 15;
        assert!(validate_binding_v1(changed).is_err());
    }
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(
        plan.maximum_live_buffers,
        main_resources::CURRENT_FULL_PROFILE_PEAK_V1
    );
    assert_eq!(main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1, 137);
    assert_eq!(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1, 8);
}

#[path = "main_log16_fixed_cost_tests.rs"]
mod complete_cost;
