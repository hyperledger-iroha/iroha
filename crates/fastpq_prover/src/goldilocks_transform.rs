//! Exact-root Goldilocks transforms shared by bounded prover adapters.
//!
//! This is arithmetic dispatch, not a proof-profile selector. Public dimensions,
//! canonical inputs and the full order of the supplied root are checked before
//! any mutation. GPU staging has clearing owners; input and completed output
//! columns remain the caller's responsibility.

use core::fmt;
use rayon::prelude::*;

use crate::{backend::ExecutionMode, cyclotomic, poseidon::FIELD_MODULUS};

/// Maximum resident columns accepted by one exact-root transform call.
pub const MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1: usize = 8;

/// Direction of an exact-root transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldilocksTransformDirectionV1 {
    /// Evaluate coefficients at successive powers of the supplied root.
    Forward,
    /// Interpolate evaluations, including multiplication by the inverse length.
    Inverse,
}

/// Backend that actually completed this transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldilocksTransformBackendV1 {
    /// Deterministic scalar arithmetic, parallel across independent columns.
    Cpu,
    /// The existing Metal Goldilocks kernel completed successfully.
    Metal,
    /// The existing CUDA Goldilocks kernel completed successfully.
    Cuda,
}

/// Whether device completion is uncertain and private staging may remain live.
///
/// This state is sticky for the process. Memory-limited proof producers must
/// reject before allocating sources, even when selecting CPU execution; switching
/// backends does not release storage still owned by an unfinished command.
#[must_use]
pub fn goldilocks_transform_completion_uncertain_v1() -> bool {
    #[cfg(feature = "fastpq-gpu")]
    {
        crate::gpu::transform_completion_uncertain_v1()
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    false
}

/// Return the hardware backend eligible for exact-root transform dispatch.
///
/// Detection is shared with the existing process-wide backend owner. CPU-only
/// builds return `None`; a quarantined CUDA backend is unavailable. This does
/// not execute a transform or qualify a complete proof pipeline.
#[must_use]
pub fn available_goldilocks_transform_backend_v1() -> Option<GoldilocksTransformBackendV1> {
    if goldilocks_transform_completion_uncertain_v1() {
        return None;
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
            return None;
        }
        match crate::backend::current_gpu_backend() {
            Some(crate::backend::GpuBackend::Metal) => Some(GoldilocksTransformBackendV1::Metal),
            Some(crate::backend::GpuBackend::Cuda) => Some(GoldilocksTransformBackendV1::Cuda),
            _ => None,
        }
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    None
}

// Shared with Metal's actual pool policy when this module is wired. Charging
// the complete pool independently of this request also covers oversized cache
// entries reused for a smaller request. Stage twiddles are only one u64/stage.
use crate::gpu_memory::{METAL_PAGE_BYTES, METAL_POOL_MAX_CACHED_BYTES};
pub(crate) const EXACT_ROOT_METAL_TWIDDLE_ENTRIES_V1: usize = 64;

/// Conservative additional host/shared payload for one bounded Metal FFT.
///
/// The caller's columns are excluded. The returned allowance includes original
/// rollback-sized reserve, rounded shared staging, the complete 64MiB
/// pool cache, all cached stage twiddles, and 1MiB for dispatch metadata. It does
/// not replace a process reserve for allocator/driver/thread overhead or imply
/// a measured RSS bound. CUDA has a different pool and is deliberately outside
/// this Metal-specific accounting contract.
///
/// # Errors
/// Returns [`GoldilocksTransformErrorV1::InvalidShape`] for an unsupported row
/// or column count, or when the checked byte allowance overflows.
pub fn metal_goldilocks_transform_extra_payload_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    if rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || columns == 0
        || columns > MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
    {
        return Err(GoldilocksTransformErrorV1::InvalidShape);
    }
    let column_bytes = rows
        .checked_mul(8)
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    let private = column_bytes
        .checked_mul(columns)
        .and_then(|n| n.checked_mul(2))
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    let padding = columns
        .checked_mul(METAL_PAGE_BYTES - 1)
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    [
        private,
        padding,
        METAL_POOL_MAX_CACHED_BYTES,
        (EXACT_ROOT_METAL_TWIDDLE_ENTRIES_V1 + 2) * 32 * 8,
        1 << 20,
    ]
    .into_iter()
    .try_fold(0_usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or(GoldilocksTransformErrorV1::InvalidShape)
    })
}

/// Failure before validation or while explicitly requiring a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldilocksTransformErrorV1 {
    /// Columns do not have one supported public power-of-two extent.
    InvalidShape,
    /// The root is noncanonical or does not have exactly the requested order.
    InvalidRoot,
    /// An input word is not a canonical Goldilocks residue.
    NonCanonicalInput,
    /// Required GPU execution is unavailable in this build or on this host.
    DeviceUnavailable,
    /// The required device rejected or failed the transform.
    DeviceFailure(String),
    /// An unfinished device command may still own private staging. No automatic
    /// fallback or subsequent memory-limited proof admission is safe.
    CompletionUncertain,
}

impl fmt::Display for GoldilocksTransformErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidShape => formatter.write_str("invalid bounded Goldilocks transform shape"),
            Self::InvalidRoot => formatter.write_str("Goldilocks root has another exact order"),
            Self::NonCanonicalInput => {
                formatter.write_str("noncanonical Goldilocks transform input")
            }
            Self::DeviceUnavailable => {
                formatter.write_str("required Goldilocks transform device unavailable")
            }
            Self::DeviceFailure(message) => {
                write!(formatter, "Goldilocks transform device failure: {message}")
            }
            Self::CompletionUncertain => formatter.write_str(
                "Goldilocks device completion is uncertain; private staging remains reserved",
            ),
        }
    }
}
impl std::error::Error for GoldilocksTransformErrorV1 {}

/// Transform at most eight columns using the exact caller-supplied root.
///
/// Each column must have the same length, from `2` through `2^32`, and contain
/// only canonical residues. CPU mode does not discover or touch a GPU. Auto
/// mode uses the configured backend when its lane is free and falls back to the
/// CPU if dispatch is unavailable or fails. Required GPU mode waits for that
/// lane and reports an error instead of substituting CPU work.
///
/// Backend failures preserve the original columns: both Metal and CUDA publish
/// their guarded staging only after a successful wait. The Metal path orders
/// independent tiles and butterflies with device resource barriers. Automatic
/// fallback therefore transforms the original inputs after a
/// fully drained failure. Uncertain completion is terminal, including in Auto
/// mode, because switching arithmetic does not release device-owned storage.
///
/// # Errors
/// Returns a shape, root or noncanonical-input error before any transform, a
/// terminal uncertain-completion error, or, in required GPU mode, a device
/// unavailability or failure error.
pub fn transform_goldilocks_columns_v1(
    columns: &mut [Vec<u64>],
    root: u64,
    direction: GoldilocksTransformDirectionV1,
    execution: ExecutionMode,
) -> Result<GoldilocksTransformBackendV1, GoldilocksTransformErrorV1> {
    let domain = validate_v1(columns, root)?;
    if matches!(execution, ExecutionMode::Cpu) {
        transform_cpu_v1(columns, domain, direction);
        return Ok(GoldilocksTransformBackendV1::Cpu);
    }
    if goldilocks_transform_completion_uncertain_v1() {
        return Err(GoldilocksTransformErrorV1::CompletionUncertain);
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        use crate::backend::{self, GpuBackend};
        if matches!(execution.resolve(), ExecutionMode::Gpu) {
            let backend = backend::current_gpu_backend();
            let actual = match backend {
                Some(GpuBackend::Metal) => Some(GoldilocksTransformBackendV1::Metal),
                Some(GpuBackend::Cuda) => Some(GoldilocksTransformBackendV1::Cuda),
                _ => None,
            };
            if let (Some(backend), Some(actual)) = (backend, actual) {
                let lane = if matches!(execution, ExecutionMode::Gpu) {
                    Some(backend::acquire_gpu_lane())
                } else {
                    backend::try_acquire_gpu_lane()
                };
                if let Some(_lane) = lane {
                    let result = match backend {
                        #[cfg(target_os = "macos")]
                        GpuBackend::Metal => crate::metal::exact_root::transform(
                            columns,
                            domain.log_size,
                            root,
                            matches!(direction, GoldilocksTransformDirectionV1::Inverse),
                        ),
                        _ => match direction {
                            GoldilocksTransformDirectionV1::Forward => {
                                crate::gpu::fft_columns_async(
                                    columns,
                                    domain.log_size,
                                    root,
                                    backend,
                                )
                            }
                            GoldilocksTransformDirectionV1::Inverse => {
                                crate::gpu::ifft_columns_async(
                                    columns,
                                    domain.log_size,
                                    root,
                                    backend,
                                )
                            }
                        }
                        .and_then(crate::gpu::ColumnDispatch::wait),
                    };
                    // Partial-batch cleanup can discover uncertain completion
                    // while an earlier ordinary error is being returned. Inspect
                    // the sticky owner state after every pending guard drops.
                    check_dispatch_completion_v1(
                        &result,
                        goldilocks_transform_completion_uncertain_v1(),
                    )?;
                    match result {
                        Ok(()) => return Ok(actual),
                        Err(error) if matches!(execution, ExecutionMode::Gpu) => {
                            return Err(GoldilocksTransformErrorV1::DeviceFailure(
                                error.to_string(),
                            ));
                        }
                        Err(error) => tracing::warn!(
                            target: "fastpq::transform",
                            %error,
                            "exact-root device transform failed; using original columns on CPU"
                        ),
                    }
                }
            }
        }
    }
    if matches!(execution, ExecutionMode::Gpu) {
        return Err(GoldilocksTransformErrorV1::DeviceUnavailable);
    }
    transform_cpu_v1(columns, domain, direction);
    Ok(GoldilocksTransformBackendV1::Cpu)
}

#[cfg(feature = "fastpq-gpu")]
fn check_dispatch_completion_v1(
    result: &Result<(), crate::gpu::GpuError>,
    uncertain_owner: bool,
) -> Result<(), GoldilocksTransformErrorV1> {
    if uncertain_owner
        || matches!(
            result,
            Err(crate::gpu::GpuError::CompletionUncertain { .. })
        )
    {
        Err(GoldilocksTransformErrorV1::CompletionUncertain)
    } else {
        Ok(())
    }
}

fn validate_v1(
    columns: &[Vec<u64>],
    root: u64,
) -> Result<cyclotomic::Domain, GoldilocksTransformErrorV1> {
    let rows = columns.first().map_or(0, Vec::len);
    if columns.is_empty()
        || columns.len() > MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
        || rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || columns.iter().any(|column| column.len() != rows)
        || rows
            .checked_mul(columns.len())
            .and_then(|n| n.checked_mul(8))
            .is_none()
    {
        return Err(GoldilocksTransformErrorV1::InvalidShape);
    }
    let order = rows as u64;
    if root == 0
        || root >= FIELD_MODULUS
        || power_v1(root, order) != 1
        || power_v1(root, order / 2) == 1
    {
        return Err(GoldilocksTransformErrorV1::InvalidRoot);
    }
    if columns.iter().flatten().any(|word| *word >= FIELD_MODULUS) {
        return Err(GoldilocksTransformErrorV1::NonCanonicalInput);
    }
    Ok(cyclotomic::Domain {
        log_size: rows.trailing_zeros(),
        generator: root,
    })
}

fn transform_cpu_v1(
    columns: &mut [Vec<u64>],
    domain: cyclotomic::Domain,
    direction: GoldilocksTransformDirectionV1,
) {
    columns.par_iter_mut().for_each(|column| match direction {
        GoldilocksTransformDirectionV1::Forward => cyclotomic::fft(column, domain),
        GoldilocksTransformDirectionV1::Inverse => cyclotomic::ifft(column, domain),
    });
}

fn power_v1(mut value: u64, mut exponent: u64) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul_mod_v1(result, value);
        }
        value = mul_mod_v1(value, value);
        exponent >>= 1;
    }
    result
}

/// Multiply two words modulo the Goldilocks prime with exact 128-bit arithmetic.
fn mul_mod_v1(left: u64, right: u64) -> u64 {
    let product = (u128::from(left) * u128::from(right)) % u128::from(FIELD_MODULUS);
    u64::try_from(product).expect("a residue modulo the Goldilocks prime fits u64")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_v1(log: u32, odd_power: u64) -> u64 {
        power_v1(power_v1(7, (FIELD_MODULUS - 1) >> log), odd_power)
    }

    fn horner_v1(coefficients: &[u64], point: u64) -> u64 {
        coefficients.iter().rev().fold(0, |value, coefficient| {
            u64::try_from(
                (u128::from(value) * u128::from(point) + u128::from(*coefficient))
                    % u128::from(FIELD_MODULUS),
            )
            .unwrap()
        })
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn uncertain_completion_is_terminal_even_after_an_ordinary_error_or_success() {
        use crate::{backend::GpuBackend, gpu::GpuError};
        let ordinary = Err(GpuError::InvalidInput("injected submission failure"));
        assert!(check_dispatch_completion_v1(&ordinary, false).is_ok());
        for result in [
            Ok(()),
            ordinary,
            Err(GpuError::CompletionUncertain {
                backend: GpuBackend::Metal,
            }),
        ] {
            assert_eq!(
                check_dispatch_completion_v1(&result, true),
                Err(GoldilocksTransformErrorV1::CompletionUncertain)
            );
        }
        assert_eq!(
            check_dispatch_completion_v1(
                &Err(GpuError::CompletionUncertain {
                    backend: GpuBackend::Cuda
                }),
                false
            ),
            Err(GoldilocksTransformErrorV1::CompletionUncertain)
        );
    }

    #[test]
    fn metal_payload_forecast_charges_cache_snapshots_staging_and_rejects_shapes() {
        for columns in [1, 2, 4, 8] {
            let bytes = metal_goldilocks_transform_extra_payload_v1(1 << 22, columns).unwrap();
            assert_eq!(
                bytes,
                2 * columns * (1 << 22) * 8
                    + columns * 16383
                    + (64 << 20)
                    + 66 * 32 * 8
                    + (1 << 20)
            );
        }
        for (rows, columns) in [(0, 1), (1, 1), (3, 1), (8, 0), (8, 9), (usize::MAX, 1)] {
            assert!(metal_goldilocks_transform_extra_payload_v1(rows, columns).is_err());
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        assert_eq!(available_goldilocks_transform_backend_v1(), None);
    }

    #[test]
    fn explicit_roots_match_independent_evaluation_and_inverse() {
        for log in 1..=5 {
            for odd in [1, 3, 5] {
                let root = root_v1(log, odd);
                let original = (0_u64..3)
                    .map(|column| {
                        (0..(1_u64 << log))
                            .map(|row| row * 31 + column * 17 + 1)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let expected = original
                    .iter()
                    .map(|column| {
                        (0..column.len())
                            .map(|row| horner_v1(column, power_v1(root, row as u64)))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let mut actual = original.clone();
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        &mut actual,
                        root,
                        GoldilocksTransformDirectionV1::Forward,
                        ExecutionMode::Cpu
                    )
                    .unwrap(),
                    GoldilocksTransformBackendV1::Cpu
                );
                assert_eq!(actual, expected);
                transform_goldilocks_columns_v1(
                    &mut actual,
                    root,
                    GoldilocksTransformDirectionV1::Inverse,
                    ExecutionMode::Cpu,
                )
                .unwrap();
                assert_eq!(actual, original);
            }
        }
    }

    #[test]
    fn all_preflight_failures_preserve_every_input_word() {
        let root = root_v1(3, 1);
        let good = vec![vec![11; 8]; 2];
        let mut cases = vec![
            (Vec::new(), root),
            (vec![vec![11]], 1),
            (vec![vec![11; 3]], root),
            (vec![vec![11; 8]; 9], root),
            (vec![vec![11; 8], vec![11; 4]], root),
            (good.clone(), 0),
            (good.clone(), 1),
            (good.clone(), FIELD_MODULUS),
            (good.clone(), root_v1(2, 1)),
        ];
        let mut malformed = good;
        malformed[1][7] = FIELD_MODULUS;
        cases.push((malformed, root));
        for (mut columns, root) in cases {
            let before = columns.clone();
            assert!(
                transform_goldilocks_columns_v1(
                    &mut columns,
                    root,
                    GoldilocksTransformDirectionV1::Forward,
                    ExecutionMode::Gpu
                )
                .is_err()
            );
            assert_eq!(columns, before);
        }
    }

    #[cfg(not(feature = "fastpq-gpu"))]
    #[test]
    fn build_without_device_uses_explicit_cpu_fallback_and_rejects_required_gpu() {
        let root = root_v1(3, 1);
        let source = vec![vec![3, 5, 7, 11, 13, 17, 19, 23]];
        let mut automatic = source.clone();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut automatic,
                root,
                GoldilocksTransformDirectionV1::Forward,
                ExecutionMode::Auto
            )
            .unwrap(),
            GoldilocksTransformBackendV1::Cpu
        );
        let mut required = source.clone();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut required,
                root,
                GoldilocksTransformDirectionV1::Forward,
                ExecutionMode::Gpu
            ),
            Err(GoldilocksTransformErrorV1::DeviceUnavailable)
        );
        assert_eq!(required, source);
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    #[ignore = "actual required-Metal eight-column native19/common22 parity and cost diagnostic"]
    fn required_metal_native19_common22_exact_root_parity_and_timing() {
        use std::time::Instant;

        const COLUMNS: usize = 8;
        const NATIVE_LOG: u32 = 19;
        const COMMON_LOG: u32 = 22;
        const MASK_COEFFICIENTS: usize = 1816;
        assert_eq!(
            crate::backend::current_gpu_backend(),
            Some(crate::backend::GpuBackend::Metal),
            "this receipt requires the real Metal backend"
        );
        let native_root = root_v1(NATIVE_LOG, 1);
        let common_root = root_v1(COMMON_LOG, 1);
        let native_rows = 1 << NATIVE_LOG;
        let common_rows = 1 << COMMON_LOG;
        assert_metal_uses_supplied_roots();

        let source = (0..COLUMNS)
            .map(|column| {
                (0..native_rows)
                    .map(|row| (row * 31 + column * 17 + 1) as u64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut coefficients = source.clone();
        let started = Instant::now();
        transform_goldilocks_columns_v1(
            &mut coefficients,
            native_root,
            GoldilocksTransformDirectionV1::Inverse,
            ExecutionMode::Cpu,
        )
        .unwrap();
        let cpu_inverse = started.elapsed();
        let mut actual = source;
        let started = Instant::now();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut actual,
                native_root,
                GoldilocksTransformDirectionV1::Inverse,
                ExecutionMode::Gpu
            )
            .unwrap(),
            GoldilocksTransformBackendV1::Metal,
        );
        let metal_inverse = started.elapsed();
        assert_eq!(actual, coefficients);
        drop(actual);

        // Use every mask coefficient, including the highest X^(N+1815) term.
        apply_vanishing_masks(&mut coefficients, native_rows, MASK_COEFFICIENTS);
        let shifted = coset_shifted(&coefficients, common_rows);
        let mut expected = shifted.clone();
        let started = Instant::now();
        transform_goldilocks_columns_v1(
            &mut expected,
            common_root,
            GoldilocksTransformDirectionV1::Forward,
            ExecutionMode::Cpu,
        )
        .unwrap();
        let cpu_forward = started.elapsed();
        for (coefficients, values) in coefficients.iter().zip(&expected) {
            for index in [0, 1, 17, common_rows / 2 + 3, common_rows - 1] {
                let point = mul_mod_v1(7, power_v1(common_root, index as u64));
                assert_eq!(values[index], horner_v1(coefficients, point));
            }
        }
        for device_batch in [8, 4, 2] {
            let mut actual = shifted.clone();
            let started = Instant::now();
            for batch in actual.chunks_mut(device_batch) {
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        batch,
                        common_root,
                        GoldilocksTransformDirectionV1::Forward,
                        ExecutionMode::Gpu
                    )
                    .unwrap(),
                    GoldilocksTransformBackendV1::Metal,
                );
            }
            let elapsed = started.elapsed();
            assert_eq!(actual, expected);
            eprintln!(
                "exact-root Metal transform: native_rows={native_rows}, common_rows={common_rows}, resident_columns={COLUMNS}, dispatch_columns={device_batch}, mask_coefficients={MASK_COEFFICIENTS}, cpu_inverse={cpu_inverse:?}, metal_inverse={metal_inverse:?}, cpu_forward={cpu_forward:?}, metal_forward={elapsed:?}; includes dispatch/staging/wait/clearing, excludes source construction, coset packing, row hashing, constraints and full proof; concurrent host load must be recorded"
            );
        }
    }

    /// Non-default primitive roots establish that dispatch uses the supplied
    /// root, not a catalog root hidden in a kernel or planner.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn assert_metal_uses_supplied_roots() {
        for odd in [1, 3, 5] {
            let root = root_v1(4, odd);
            let mut values = vec![(0_u64..16).map(|i| i * 31 + 17).collect::<Vec<_>>()];
            let expected = (0..16)
                .map(|i| horner_v1(&values[0], power_v1(root, i)))
                .collect::<Vec<_>>();
            assert_eq!(
                transform_goldilocks_columns_v1(
                    &mut values,
                    root,
                    GoldilocksTransformDirectionV1::Forward,
                    ExecutionMode::Gpu
                )
                .unwrap(),
                GoldilocksTransformBackendV1::Metal,
            );
            assert_eq!(values[0], expected);
        }
    }

    /// Extend each coefficient column by `masks` terms: subtract each mask from
    /// coefficient `i` and place it at coefficient `native_rows + i`.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn apply_vanishing_masks(coefficients: &mut [Vec<u64>], native_rows: usize, masks: usize) {
        for (column, coefficients) in coefficients.iter_mut().enumerate() {
            coefficients.resize(native_rows + masks, 0);
            for index in 0..masks {
                let mask = (index * 13 + column * 19 + 1) as u64;
                coefficients[index] = u64::try_from(
                    (u128::from(coefficients[index]) + u128::from(FIELD_MODULUS)
                        - u128::from(mask))
                        % u128::from(FIELD_MODULUS),
                )
                .expect("a residue modulo the Goldilocks prime fits u64");
                coefficients[native_rows + index] = mask;
            }
        }
    }

    /// Scale coefficient `i` of every column by `7^i` into a zero-padded
    /// `common_rows` vector, so a forward transform evaluates on the 7-coset.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn coset_shifted(coefficients: &[Vec<u64>], common_rows: usize) -> Vec<Vec<u64>> {
        coefficients
            .iter()
            .map(|coefficients| {
                let mut values = vec![0; common_rows];
                let mut shift = 1;
                for (output, coefficient) in values.iter_mut().zip(coefficients) {
                    *output = mul_mod_v1(*coefficient, shift);
                    shift = mul_mod_v1(shift, 7);
                }
                values
            })
            .collect()
    }
}
