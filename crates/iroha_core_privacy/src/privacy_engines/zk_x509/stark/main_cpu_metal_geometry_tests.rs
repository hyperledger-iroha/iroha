//! Test-only complete CPU/Metal geometry comparison.
//!
//! No production selection, clock threshold, or artificial delay is introduced.
//! Run the same frozen test under observed host load and compare its command
//! timestamps; a label such as "stalled" must come from those measurements.

use super::super::main_native_transform::ExactNativeTransformV1;
use super::*;
use crate::privacy_engines::zk_x509::prover_observation::ObservationV1;
use fastpq_prover::goldilocks_transform::{
    available_goldilocks_transform_backend_v1, goldilocks_transform_completion_uncertain_v1,
    transform_goldilocks_columns_v1,
};
use rayon::prelude::*;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum GeometryMode {
    CpuOriginal,
    CpuSharedPowers,
    MetalCurrent,
}

impl GeometryMode {
    const ALL: [Self; 3] = [Self::CpuOriginal, Self::CpuSharedPowers, Self::MetalCurrent];

    fn policy(self, rows: usize, width: usize) -> MainBoundedTransformPolicyV1 {
        let admitted = MainBoundedTransformPolicyV1::for_test_v1(rows, width);
        match self {
            Self::CpuOriginal => MainBoundedTransformPolicyV1::cpu_v1(),
            Self::CpuSharedPowers => MainBoundedTransformPolicyV1 {
                available: admitted.available,
                backend: None,
                // Same separate fixed public-table reservation as the real
                // assembly policy; no device slack pays for these tables.
                public_powers_reserved: true,
            },
            Self::MetalCurrent => admitted,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum GeometryOperation {
    NativeInverse,
    QuotientForward,
    FixedDiagonalForward,
}

impl GeometryOperation {
    const ALL: [Self; 3] = [
        Self::NativeInverse,
        Self::QuotientForward,
        Self::FixedDiagonalForward,
    ];

    fn apply(
        self,
        mode: GeometryMode,
        columns: &mut [Vec<F>],
        root: F,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let policy = mode.policy(columns[0].len(), columns.len());
        let mut device = |words: &mut [Vec<u64>], root, direction| {
            assert!(
                matches!(mode, GeometryMode::MetalCurrent),
                "CPU may not dispatch"
            );
            let backend = transform_goldilocks_columns_v1(
                words,
                root,
                direction,
                fastpq_prover::ExecutionMode::Gpu,
            )?;
            assert_eq!(backend, Backend::Metal, "required actual Metal control");
            Ok(backend)
        };
        match self {
            Self::NativeInverse => policy.inverse_native_with_v1(
                columns,
                root,
                &mut ExactNativeTransformV1,
                goldilocks_transform_completion_uncertain_v1,
            ),
            Self::QuotientForward => policy.forward_with_v1(
                columns,
                root,
                &mut device,
                goldilocks_transform_completion_uncertain_v1,
            ),
            Self::FixedDiagonalForward if !matches!(mode, GeometryMode::MetalCurrent) => {
                // The fixed device adapter does not implement CPU fallback.
                // Reproduce MainFixedCosetV1's first-stripe CPU branch: its
                // separately admitted power pair, bounded column parallelism,
                // exact serial diagonal, and original coarse FFT arithmetic.
                check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
                let rows = validate_private_columns_v1(columns, root)?;
                let (forward_powers, inverse_powers) =
                    policy.cpu_power_pair_v1(rows, root, false)?;
                assert!(inverse_powers.is_none());
                columns.par_iter_mut().try_for_each(|column| {
                    let mut power = F::ONE;
                    for value in &mut *column {
                        *value = value.mul(power);
                        power = power.mul(F(7));
                    }
                    if let Some(powers) = &forward_powers {
                        crate::privacy_engines::transparent_stark::goldilocks_fft_coarse_with_powers_v1(
                            column, root, powers,
                        )
                    } else {
                        crate::privacy_engines::transparent_stark::goldilocks_fft_coarse_v1(
                            column, root,
                        )
                    }
                    .map_err(map_transparent_error_v1)
                })?;
                check_completion_v1(goldilocks_transform_completion_uncertain_v1())
            }
            Self::FixedDiagonalForward => policy.apply_with_v1(
                columns,
                root,
                F(7),
                false,
                &mut device,
                goldilocks_transform_completion_uncertain_v1,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct GeometrySample {
    allocation: Duration,
    transform: Duration,
    clear: Duration,
    staging: fastpq_prover::ColumnStagingPhaseStats,
}

impl GeometrySample {
    fn total(self) -> Duration {
        self.allocation + self.transform + self.clear
    }
}

#[test]
#[ignore = "isolated complete native/fixed/quotient CPU versus Metal geometry A/B"]
fn public_geometry_cpu_and_metal_cost_preserves_full_outputs_and_original_admission() {
    const ROUNDS: usize = 6;
    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert_eq!(
        rayon::current_thread_index(),
        None,
        "use external prover caller"
    );
    assert_eq!(
        rayon::current_num_threads(),
        20,
        "run with RAYON_NUM_THREADS=20"
    );
    assert_eq!(
        available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    assert!(!goldilocks_transform_completion_uncertain_v1());
    rayon::broadcast(|_| ());
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            fastpq_prover::enable_queue_depth_stats(false);
        }
    }
    fastpq_prover::enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    for log in [16_u8, 18, 19] {
        let rows = 1_usize << log;
        let root = goldilocks_primitive_root_v1(log).unwrap();
        for width in [1, 4, 8] {
            let original = ZeroizingBaseColumnsV1(
                (0..width)
                    .map(|column| {
                        (0..rows)
                            .map(|row| F::reduce((31 * row + 17 * column + 1) as u128))
                            .collect()
                    })
                    .collect(),
            );
            let shared = GeometryMode::CpuSharedPowers.policy(rows, width);
            let pair = shared.cpu_power_pair_v1(rows, root, true).unwrap();
            let charged = pair
                .0
                .as_ref()
                .unwrap()
                .allocated_payload_bytes_v1()
                .unwrap()
                + pair
                    .1
                    .as_ref()
                    .unwrap()
                    .allocated_payload_bytes_v1()
                    .unwrap();
            assert!(charged <= SHARED_POWERS_ALLOWANCE_V1);
            drop(pair);
            assert_eq!(
                GeometryMode::MetalCurrent
                    .policy(rows, width)
                    .columns_v1(rows),
                width
            );
            for operation in GeometryOperation::ALL {
                let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
                operation
                    .apply(GeometryMode::CpuOriginal, &mut expected.0, root)
                    .unwrap();
                let mut samples = [[GeometrySample::default(); ROUNDS]; GeometryMode::ALL.len()];
                // One warmup per mode precedes six rounds in which each mode
                // occupies each position twice. Table construction/destruction
                // remains inside every timed CPU-shared transform, as in use.
                for round in 0..=ROUNDS {
                    for position in 0..GeometryMode::ALL.len() {
                        let measured = round.saturating_sub(1);
                        let index = if measured % 2 == 0 {
                            (measured / 2 + position) % GeometryMode::ALL.len()
                        } else {
                            (measured / 2 + GeometryMode::ALL.len() - 1 - position)
                                % GeometryMode::ALL.len()
                        };
                        let mode = GeometryMode::ALL[index];
                        fastpq_prover::take_column_staging_stats().unwrap();
                        let observation = ObservationV1::begin_v1();
                        let started = Instant::now();
                        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
                        let allocation = started.elapsed();
                        let pointers = actual
                            .iter()
                            .map(|v| (v.as_ptr(), v.capacity()))
                            .collect::<Vec<_>>();
                        let started = Instant::now();
                        operation.apply(mode, &mut actual.0, root).unwrap();
                        let transform = started.elapsed();
                        let staging = fastpq_prover::take_column_staging_stats().unwrap().fft();
                        let receipt = observation.finish_v1().public_text_v1();
                        if matches!(mode, GeometryMode::MetalCurrent) {
                            assert_eq!(staging.batches, 1, "actual device dispatch is required");
                            if matches!(operation, GeometryOperation::NativeInverse) {
                                assert!(receipt.contains(&format!(
                                    "native_replay_metal_inverse_columns={width}"
                                )));
                                assert!(receipt.contains("native_replay_cpu_inverse_columns=0"));
                            }
                        } else {
                            assert_eq!(staging.batches, 0, "CPU control dispatched unexpectedly");
                        }
                        assert_eq!(&*actual, &*expected);
                        assert_eq!(
                            actual
                                .iter()
                                .map(|v| (v.as_ptr(), v.capacity()))
                                .collect::<Vec<_>>(),
                            pointers
                        );
                        assert!(!goldilocks_transform_completion_uncertain_v1());
                        let started = Instant::now();
                        drop(actual);
                        let clear = started.elapsed();
                        if round != 0 {
                            samples[index][measured] = GeometrySample {
                                allocation,
                                transform,
                                clear,
                                staging,
                            };
                        }
                    }
                }
                // Emit after all measured rounds to avoid stdout in the cycle.
                for (index, mode) in GeometryMode::ALL.into_iter().enumerate() {
                    let mut totals = samples[index].map(|sample| sample.total());
                    totals.sort_unstable();
                    let mean = samples[index]
                        .iter()
                        .map(|sample| sample.total().as_secs_f64())
                        .sum::<f64>()
                        / ROUNDS as f64;
                    let median = (totals[ROUNDS / 2 - 1].as_secs_f64()
                        + totals[ROUNDS / 2].as_secs_f64())
                        / 2.0;
                    println!(
                        "x509_geometry_summary operation={operation:?} mode={mode:?} log={log} width={width} workers=20 caller_context=external rounds={ROUNDS} mean_ms={:.6} median_ms={:.6} public_power_allowance_bytes={SHARED_POWERS_ALLOWANCE_V1} parity=all_fields_exact pointers=unchanged",
                        mean * 1000.0,
                        median * 1000.0
                    );
                    for (round, sample) in samples[index].iter().enumerate() {
                        println!(
                            "x509_geometry_sample operation={operation:?} mode={mode:?} log={log} width={width} round={round} allocation_ms={:.6} transform_ms={:.6} clear_ms={:.6} wait_ms={:.6} flatten_ms={:.6} timestamped_commands={} gpu_execution_ms={:.6} cpu_driver_scheduling_ms={:.6} pre_gpu_queue_ms={:.6} timing_scope=successful-command-intervals-not-additive",
                            sample.allocation.as_secs_f64() * 1000.0,
                            sample.transform.as_secs_f64() * 1000.0,
                            sample.clear.as_secs_f64() * 1000.0,
                            sample.staging.wait_ms,
                            sample.staging.flatten_ms,
                            sample.staging.timestamped_commands,
                            sample.staging.gpu_execution_ms,
                            sample.staging.cpu_driver_scheduling_ms,
                            sample.staging.pre_gpu_queue_ms
                        );
                    }
                }
            }
        }
    }
}
