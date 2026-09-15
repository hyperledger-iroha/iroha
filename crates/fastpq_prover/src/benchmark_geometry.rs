//! Shared input preflight for the maintained native developer benchmarks.
//!
//! The bounds come from the sole FASTPQ V1 parameters and Rust allocation/count
//! representations. This owner neither chooses performance limits nor changes
//! proof geometry, runtime configuration, or hardware dispatch.

use std::alloc::Layout;

use fastpq_isi::{FASTPQ_FINAL_V1_ID, find_by_name};
use iroha_zkp_halo2::Bn254Scalar;

/// Preflighted row extents shared by both native benchmark producers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BenchmarkGeometryV1 {
    padded_rows: usize,
    evaluation_rows: usize,
}

impl BenchmarkGeometryV1 {
    /// Canonical power-of-two trace extent, bounded by the V1 trace domain.
    #[must_use]
    pub const fn padded_rows(self) -> usize {
        self.padded_rows
    }

    /// LDE extent under the unchanged canonical blowup factor.
    #[must_use]
    pub const fn evaluation_rows(self) -> usize {
        self.evaluation_rows
    }
}

/// Validate native benchmark counts before tracing, allocation, or device work.
///
/// Zero warmups are valid. Sample-buffer layouts and the producers' column and
/// byte counts must be representable; successful preflight does not promise
/// available physical memory or bound wall-clock duration.
///
/// # Errors
/// Rejects zero or oversized rows, zero columns or iterations, invocation-sum
/// overflow, and unrepresentable sample/vector/column extents. All arithmetic
/// and layout checks complete without allocating benchmark input buffers.
pub fn preflight_benchmark_geometry_v1(
    rows: usize,
    columns: usize,
    warmups: usize,
    iterations: usize,
) -> Result<BenchmarkGeometryV1, String> {
    let params = find_by_name(FASTPQ_FINAL_V1_ID)
        .ok_or_else(|| "canonical FASTPQ V1 parameters are missing".to_owned())?;
    let maximum_rows = 1_usize
        .checked_shl(params.trace_log_size)
        .ok_or_else(|| "canonical benchmark row bound exceeds usize".to_owned())?;
    if rows == 0 || rows > maximum_rows {
        return Err(format!("--rows must be between 1 and {maximum_rows}"));
    }
    if columns == 0 {
        return Err("--column-count must be greater than zero".to_owned());
    }
    if iterations == 0 {
        return Err("--iterations must be greater than zero".to_owned());
    }
    let invocations = warmups
        .checked_add(iterations)
        .ok_or_else(|| "benchmark warmups + iterations overflow usize".to_owned())?;
    for count in [rows, columns, warmups, iterations, invocations] {
        u64::try_from(count)
            .map_err(|_| "benchmark count exceeds the V1 u64 report representation".to_owned())?;
    }
    let padded_rows = rows
        .checked_next_power_of_two()
        .filter(|padded| *padded <= maximum_rows)
        .ok_or_else(|| "benchmark padded rows exceed the canonical trace domain".to_owned())?;
    let blowup = usize::try_from(params.fri.blowup_factor)
        .map_err(|_| "canonical benchmark blowup exceeds usize".to_owned())?;
    let evaluation_rows = padded_rows
        .checked_mul(blowup)
        .ok_or_else(|| "benchmark evaluation extent overflows usize".to_owned())?;
    Layout::array::<f64>(iterations)
        .map_err(|_| "benchmark sample allocation extent is not representable".to_owned())?;
    Layout::array::<Vec<u64>>(columns)
        .map_err(|_| "benchmark column-vector extent is not representable".to_owned())?;
    Layout::array::<crate::TraceColumn>(columns)
        .map_err(|_| "benchmark named-column extent is not representable".to_owned())?;
    Layout::array::<Bn254Scalar>(evaluation_rows)
        .map_err(|_| "benchmark scalar column allocation extent is not representable".to_owned())?;
    // CUDA owns one contiguous four-word BN254 trace buffer and, for LDE,
    // one contiguous expanded output. The latter also covers the smaller
    // Goldilocks staging/zero-fill buffer and aggregate report byte sums.
    let flattened_words = columns
        .checked_mul(evaluation_rows)
        .and_then(|elements| elements.checked_mul(4))
        .ok_or_else(|| "benchmark flattened word count overflows usize".to_owned())?;
    Layout::array::<u64>(flattened_words)
        .map_err(|_| "benchmark flattened allocation extent is not representable".to_owned())?;
    let evaluation_log = usize::try_from(evaluation_rows.ilog2())
        .map_err(|_| "benchmark evaluation log exceeds usize".to_owned())?;
    let twiddle_count = (evaluation_rows / 2)
        .checked_mul(evaluation_log)
        .ok_or_else(|| "benchmark twiddle count overflows usize".to_owned())?;
    Layout::array::<Bn254Scalar>(twiddle_count)
        .map_err(|_| "benchmark twiddle allocation extent is not representable".to_owned())?;
    Ok(BenchmarkGeometryV1 {
        padded_rows,
        evaluation_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_defaults_and_final_row_boundary_have_exact_extents() {
        let defaults = preflight_benchmark_geometry_v1(20_000, 16, 1, 5).unwrap();
        assert_eq!(defaults.padded_rows(), 32_768);
        assert_eq!(defaults.evaluation_rows(), 262_144);
        let maximum = preflight_benchmark_geometry_v1(65_536, 16, 0, 1).unwrap();
        assert_eq!(maximum.padded_rows(), 65_536);
        assert_eq!(maximum.evaluation_rows(), 524_288);
        assert_eq!(
            preflight_benchmark_geometry_v1(1, 1, 0, 1)
                .unwrap()
                .padded_rows(),
            1
        );
    }

    #[test]
    fn invalid_rows_reject_before_power_of_two_or_allocation() {
        for rows in [0, 65_537, usize::MAX] {
            assert!(
                preflight_benchmark_geometry_v1(rows, 16, 0, 1).is_err(),
                "{rows}"
            );
        }
    }

    #[test]
    fn flattened_bn254_extent_must_fit_one_allocation_not_only_usize() {
        let bytes_per_column = 524_288 * std::mem::size_of::<[u64; 4]>();
        let largest = usize::try_from(isize::MAX).unwrap() / bytes_per_column;
        assert!(preflight_benchmark_geometry_v1(65_536, largest, 0, 1).is_ok());
        let overflowing = largest + 1;
        assert!(overflowing.checked_mul(bytes_per_column).is_some());
        assert!(Layout::array::<Vec<u64>>(overflowing).is_ok());
        assert!(Layout::array::<crate::TraceColumn>(overflowing).is_ok());
        let error = preflight_benchmark_geometry_v1(65_536, overflowing, 0, 1).unwrap_err();
        assert!(error.contains("flattened allocation extent"));
    }

    #[test]
    fn invalid_counts_reject_without_running_or_allocating_samples() {
        for (columns, warmups, iterations) in [
            (16, 0, 0),
            (16, usize::MAX, 1),
            (16, 1, usize::MAX),
            (16, 0, usize::MAX),
            (0, 0, 1),
            (usize::MAX, 0, 1),
        ] {
            assert!(preflight_benchmark_geometry_v1(8, columns, warmups, iterations).is_err());
        }
        let layout_overflow = usize::try_from(isize::MAX).unwrap() / std::mem::size_of::<f64>() + 1;
        assert!(preflight_benchmark_geometry_v1(8, 1, 0, layout_overflow).is_err());
        let byte_overflow = usize::MAX / (524_288 * std::mem::size_of::<[u64; 4]>()) + 1;
        assert!(preflight_benchmark_geometry_v1(65_536, byte_overflow, 0, 1).is_err());
    }
}
