//! Prepared fixed transitions: independent word parity, owner failures and full cost.

use super::*;
use crate::privacy_engines::zk_x509::{
    p256_aggregate_adapter::{
        P256MainAdapterV1, P256MainRegistrationV1, P256MainVerifierFixedSourceV1,
    },
    private_table::{PrivateTableV1, inspection, zeroize_fields_v1},
    prover_observation::ObservationV1,
};

fn coefficients(log: u8, width: usize) -> Vec<Vec<F>> {
    (0..width)
        .map(|column| {
            (0..1_usize << log)
                .map(|row| F((row * row + 19 * column + 7) as u64))
                .collect()
        })
        .collect()
}

fn prefix(columns: &[Vec<F>], count: usize) -> Vec<PrivateTableV1<F>> {
    columns[..count]
        .iter()
        .map(|column| PrivateTableV1::new(column.clone(), zeroize_fields_v1))
        .collect()
}

fn coordinates(
    native: u8,
    rows: u8,
    ordinal: usize,
) -> main_quotient_stripes::MainQuotientStripeV1 {
    let root = goldilocks_primitive_root_v1(rows).unwrap();
    let full_root = goldilocks_primitive_root_v1(rows + 2).unwrap();
    main_quotient_stripes::MainQuotientStripeV1 {
        rows: 1 << rows,
        count: 4,
        ordinal,
        next_stride: 1 << (rows - native),
        root,
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
    }
}

#[test]
fn prepared_fixed_matches_word_oracle_for_prefixes_padding_tails_and_workers() {
    for workers in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                for width in [1, 8, 17, 33] {
                    for retained in [0, width.min(3), width] {
                        for rows_log in [3, 4] {
                            let original = coefficients(3, width);
                            let cache = prefix(&original, retained);
                            let policy = MainBoundedTransformPolicyV1::for_forward_test_v1(
                                1 << rows_log,
                                16,
                            );
                            assert_eq!(policy.forward_columns_v1(1 << rows_log), 16);
                            let mut exact = MainFixedCosetV1::new_v1(3, original.clone())
                                .unwrap()
                                .with_transform_policy_v1(policy)
                                .unwrap()
                                .with_coefficient_prefix_v1(&cache)
                                .unwrap();
                            let mut word = MainFixedCosetV1::new_v1(3, original.clone())
                                .unwrap()
                                .with_transform_policy_v1(policy)
                                .unwrap()
                                .with_coefficient_prefix_v1(&cache)
                                .unwrap();
                            let mut allocations = None;
                            for ordinal in 0..4 {
                                let stripe = coordinates(3, rows_log, ordinal);
                                let expected = word
                                    .evaluate_with_v1(
                                        stripe,
                                        |words, root, direction| {
                                            transform_goldilocks_columns_v1(
                                                words,
                                                root,
                                                direction,
                                                fastpq_prover::ExecutionMode::Cpu,
                                            )
                                        },
                                        || false,
                                    )
                                    .unwrap();
                                let actual = exact.evaluate_v1(stripe).unwrap();
                                assert_eq!(actual, expected);
                                let current = actual
                                    .iter()
                                    .map(|column| (column.as_ptr(), column.capacity()))
                                    .collect::<Vec<_>>();
                                if let Some(previous) = &allocations {
                                    assert_eq!(&current, previous);
                                }
                                allocations = Some(current);
                                for (column, original) in actual.iter().zip(&original) {
                                    for (row, actual) in column.iter().enumerate() {
                                        let x = stripe.shift.mul(stripe.root.pow(row as u128));
                                        let expected = original
                                            .iter()
                                            .rev()
                                            .fold(F::ZERO, |sum, value| sum.mul(x).add(*value));
                                        assert_eq!(*actual, expected);
                                    }
                                }
                            }
                        }
                    }
                }
            });
    }
}

#[test]
fn prepared_fixed_partial_group_error_unwind_and_uncertainty_poison_and_erase_owner() {
    struct FailAfter {
        fail: usize,
        calls: usize,
        unwind: bool,
    }
    impl MainFixedTransformV1 for FailAfter {
        fn columns_v1(&self, _: MainBoundedTransformPolicyV1, _: usize) -> usize {
            16
        }
        fn apply_v1(
            &mut self,
            policy: MainBoundedTransformPolicyV1,
            columns: &mut [Vec<F>],
            root: F,
            diagonal: F,
            recovery: bool,
            uncertain: &mut dyn FnMut() -> bool,
        ) -> Result<(), ZkX509StarkErrorV1> {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail {
                // A prepared inverse can publish before a later stage fails.
                // The enclosing fixed matrix must remain unpublished/poisoned.
                columns[0][0] = F(987);
                assert!(!self.unwind, "injected fixed transition unwind");
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            ExactFixedTransformV1.apply_v1(policy, columns, root, diagonal, recovery, uncertain)
        }
    }
    for fail in 0..3 {
        for unwind in [false, true] {
            let ((), erased) = inspection::observe_v1(|| {
                let mut owner = MainFixedCosetV1::new_v1(3, coefficients(3, 33)).unwrap();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    owner
                        .evaluate_transform_v1(
                            coordinates(3, 3, 0),
                            &mut FailAfter {
                                fail,
                                calls: 0,
                                unwind,
                            },
                            || false,
                        )
                        .map(|_| ())
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert!(owner.poisoned);
                assert!(owner.evaluate_v1(coordinates(3, 3, 0)).is_err());
                drop(owner);
            });
            assert_eq!(
                erased.iter().map(|entry| entry.cells).sum::<usize>(),
                33 * 8
            );
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
    let original = coefficients(3, 3);
    let mut owner = MainFixedCosetV1::new_v1(3, original.clone()).unwrap();
    let result =
        owner.evaluate_transform_v1(coordinates(3, 4, 0), &mut ExactFixedTransformV1, || true);
    assert!(matches!(
        result,
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    ));
    assert_eq!(owner.columns.0, original);
    assert!(owner.poisoned);
}

#[test]
fn prepared_fixed_exact_admission_preserves_inputs_and_uncertainty_priority() {
    for malformed in 0..7 {
        for uncertain in [false, true] {
            let mut columns = coefficients(3, 2);
            let mut root = goldilocks_primitive_root_v1(3).unwrap();
            let mut diagonal = F(7);
            match malformed {
                0 => columns.clear(),
                1 => {
                    columns[1].pop();
                }
                2 => columns[0][0] = F(u64::MAX),
                3 => root = F::ONE,
                4 => diagonal = F::ZERO,
                5 => diagonal = F(u64::MAX),
                6 => columns = coefficients(3, 17),
                _ => unreachable!(),
            }
            let original = columns.clone();
            let result = MainBoundedTransformPolicyV1::cpu_v1().apply_exact_v1(
                &mut columns,
                root,
                diagonal,
                true,
                || uncertain,
            );
            if uncertain {
                assert_eq!(
                    result,
                    Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
                );
            } else {
                assert_eq!(result, Err(ZkX509StarkErrorV1::ProfileMismatch));
            }
            assert_eq!(columns, original);
        }
    }
}

#[test]
#[ignore = "full native19 Metal word/prepared fixed transition cost with four exact cosets"]
fn prepared_fixed_complete_four_stripe_cost_matches_all_public_fields() {
    use std::time::{Duration, Instant};
    const WIDTH: usize = 134;
    const LOG: u8 = 19;
    const ROWS: usize = 1 << LOG;
    const RETAINED: usize = 27;
    const ROUNDS: usize = 4;
    assert!(!cfg!(debug_assertions));
    assert_eq!(rayon::current_num_threads(), 20);
    assert!(rayon::current_thread_index().is_none());
    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 215_244_730)
        .unwrap()
        .for_quotient_layout_v1(211, 72)
        .unwrap();
    assert_eq!(policy.columns_v1(ROWS), 8);
    assert_eq!(policy.forward_columns_v1(ROWS), 16);
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let mut original = ZeroizingBaseColumnsV1(vec![vec![F::ZERO; ROWS]; WIDTH]);
    provider
        .fill_arithmetic_fixed_matrix_v1(
            P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap(),
            &mut original.0,
        )
        .unwrap();
    original.0.par_iter_mut().for_each(|column| {
        goldilocks_ifft_coarse_v1(column, goldilocks_primitive_root_v1(LOG).unwrap()).unwrap();
    });
    // Reference allocation and all CPU FFTs finish before warmup/sampling.
    // These ~2 GiB are test-only, outside the production owner budget.
    let expected: [ZeroizingBaseColumnsV1; 4] = core::array::from_fn(|ordinal| {
        let stripe = coordinates(LOG, LOG, ordinal);
        let mut matrix = ZeroizingBaseColumnsV1(Vec::with_capacity(WIDTH));
        for column in &original.0 {
            matrix
                .0
                .push(stripe.evaluate_v1(column).unwrap().into_vec_v1());
        }
        matrix
    });
    // Warm both exact paths and all four coset kernels before either sample.
    for prepared in [false, true] {
        let mut owner = MainFixedCosetV1::new_v1(LOG, original.0[..16].to_vec())
            .unwrap()
            .with_transform_policy_v1(policy)
            .unwrap();
        for ordinal in 0..4 {
            let stripe = coordinates(LOG, LOG, ordinal);
            if prepared {
                owner.evaluate_v1(stripe).unwrap();
            } else {
                owner
                    .evaluate_with_v1(
                        stripe,
                        required_metal,
                        goldilocks_transform_completion_uncertain_v1,
                    )
                    .unwrap();
            }
        }
    }
    let mut samples = [[Duration::ZERO; ROUNDS]; 2];
    for round in 0..ROUNDS {
        for prepared in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let observation = ObservationV1::begin_v1();
            fastpq_prover::enable_queue_depth_stats(true);
            fastpq_prover::take_column_staging_stats().unwrap();
            let started = Instant::now();
            let mut columns = ZeroizingBaseColumnsV1(original.0.clone());
            let cache = prefix(&columns.0, RETAINED);
            let mut owner = MainFixedCosetV1::new_v1(LOG, core::mem::take(&mut columns.0))
                .unwrap()
                .with_transform_policy_v1(policy)
                .unwrap()
                .with_coefficient_prefix_v1(&cache)
                .unwrap();
            for ordinal in 0..4 {
                let stripe = coordinates(LOG, LOG, ordinal);
                let actual = if prepared {
                    owner.evaluate_v1(stripe).unwrap()
                } else {
                    owner
                        .evaluate_with_v1(
                            stripe,
                            required_metal,
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap()
                };
                assert_eq!(actual.len(), WIDTH);
                assert!(
                    actual
                        .iter()
                        .flatten()
                        .all(|value| F::canonical(value.0).is_some())
                );
                for (column, (actual, expected)) in
                    actual.iter().zip(&expected[ordinal].0).enumerate()
                {
                    assert!(
                        actual == expected,
                        "prepared={prepared}, stripe={ordinal}, column={column}"
                    );
                }
            }
            drop(owner);
            drop(cache);
            drop(columns);
            let elapsed = started.elapsed();
            let staging = fastpq_prover::take_column_staging_stats().unwrap().fft();
            fastpq_prover::enable_queue_depth_stats(false);
            let receipt = observation.finish_v1().public_text_v1();
            assert!(receipt.contains("fixed_coset_metal_forward_columns=536"));
            assert!(receipt.contains("fixed_coset_metal_inverse_columns=321"));
            assert!(receipt.contains("fixed_coset_cpu_forward_columns=0"));
            assert!(receipt.contains("fixed_coset_cpu_inverse_columns=0"));
            samples[usize::from(prepared)][round] = elapsed;
            println!(
                "x509_fixed_prepared_cost prepared={prepared} round={round} rows={ROWS} width={WIDTH} cached={RETAINED} stripes=4 complete_seconds={:.9} metal_batches={} wait_ms={:.6} gpu_ms={:.6} pre_gpu_queue_ms={:.6}; exact_all_fields; contiguous_wall_time_includes_matrix_clone_cachecopy_admission_four_transitions_full_canonicality_equal_reference_comparison_and_owner_erasure; excludes_fixture_initial_IFFT_CPU_reference_allocation_setup_and_erasure; reference_memory_test_only; counters={receipt}",
                elapsed.as_secs_f64(),
                staging.batches,
                staging.wait_ms,
                staging.gpu_execution_ms,
                staging.pre_gpu_queue_ms,
            );
        }
    }
    for samples in &mut samples {
        samples.sort_unstable();
    }
    let medians =
        samples.map(|samples| (samples[1].as_secs_f64() + samples[2].as_secs_f64()) / 2.0);
    println!(
        "x509_fixed_prepared_median word_seconds={:.9} prepared_seconds={:.9} speedup={:.6}; component_only_not_full_proof",
        medians[0],
        medians[1],
        medians[0] / medians[1]
    );
}

fn required_metal(
    words: &mut [Vec<u64>],
    root: u64,
    direction: Direction,
) -> Result<Backend, TransformError> {
    let backend =
        transform_goldilocks_columns_v1(words, root, direction, fastpq_prover::ExecutionMode::Gpu)?;
    assert_eq!(backend, Backend::Metal);
    Ok(backend)
}
