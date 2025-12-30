// SPDX-License-Identifier: Apache-2.0

//! CUDA bindings for the FASTPQ preview backend.
//!
//! The GPU backend is optional – enable the `fastpq-gpu` feature and provide a CUDA toolchain
//! (SM80+) to compile the kernels. When unavailable, all entry points return
//! [`CudaBackendError::Unavailable`] so the caller can fall back to the scalar implementation.

#[cfg(feature = "fastpq-gpu")]
use core::convert::TryFrom;
use core::fmt;

#[cfg(feature = "fastpq-gpu")]
use crate::trace::PoseidonColumnSlice;
/// Result alias for CUDA operations.
pub type Result<T> = core::result::Result<T, CudaBackendError>;

/// Errors surfaced by the CUDA backend wrappers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CudaBackendError {
    /// GPU support was not compiled in or deliberately skipped.
    Unavailable,
    /// The invocation was passed mismatched or out-of-bounds buffers.
    ShapeMismatch {
        /// Expected element count for the CUDA kernel invocation (saturates at `u32::MAX`).
        expected: u32,
        /// Actual number of elements supplied by the caller (saturates at `u32::MAX`).
        got: u32,
    },
    /// Underlying CUDA call returned an error code.
    Cuda {
        /// Raw CUDA error code forwarded from the runtime.
        code: u32,
    },
}

impl fmt::Display for CudaBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "GPU backend unavailable"),
            Self::ShapeMismatch { expected, got } => {
                write!(f, "buffer length mismatch (expected {expected}, got {got})")
            }
            Self::Cuda { code } => write!(f, "cudaError_t({code})"),
        }
    }
}

impl std::error::Error for CudaBackendError {}

#[cfg(feature = "fastpq-gpu")]
const POSEIDON_STATE_WIDTH: usize = 3;
#[cfg(feature = "fastpq-gpu")]
const POSEIDON_RATE: usize = 2;

#[cfg(all(feature = "fastpq-gpu", not(fastpq_cuda_unavailable)))]
mod native {
    use super::{CudaBackendError, Result};

    #[link(name = "fastpq_cuda", kind = "static")]
    extern "C" {
        fn fastpq_fft_cuda(elements: *mut u64, column_count: usize, log_size: u32) -> i32;
        fn fastpq_ifft_cuda(elements: *mut u64, column_count: usize, log_size: u32) -> i32;
        fn fastpq_lde_cuda(
            coeffs: *const u64,
            column_count: usize,
            trace_log: u32,
            blowup_log: u32,
            coset: u64,
            out: *mut u64,
        ) -> i32;
        fn fastpq_poseidon_permute_cuda(states: *mut u64, state_count: usize) -> i32;
        fn fastpq_poseidon_hash_columns_cuda(
            payloads: *const u64,
            slices: *const PoseidonColumnSlice,
            column_count: usize,
            block_count: usize,
            out_states: *mut u64,
        ) -> i32;
        fn fastpq_poseidon_hash_columns_fused_cuda(
            payloads: *const u64,
            slices: *const PoseidonColumnSlice,
            column_count: usize,
            block_count: usize,
            out_hashes: *mut u64,
        ) -> i32;
    }

    #[inline]
    fn map_cuda(code: i32) -> Result<()> {
        match code {
            0 => Ok(()),
            other => Err(CudaBackendError::Cuda { code: other as u32 }),
        }
    }

    pub(super) fn fft(elements: &mut [u64], column_count: usize, log_size: u32) -> Result<()> {
        let code = unsafe { fastpq_fft_cuda(elements.as_mut_ptr(), column_count, log_size) };
        map_cuda(code)
    }

    pub(super) fn ifft(elements: &mut [u64], column_count: usize, log_size: u32) -> Result<()> {
        let code = unsafe { fastpq_ifft_cuda(elements.as_mut_ptr(), column_count, log_size) };
        map_cuda(code)
    }

    pub(super) fn lde(
        coeffs: &[u64],
        column_count: usize,
        trace_log: u32,
        blowup_log: u32,
        coset: u64,
        out: &mut [u64],
    ) -> Result<()> {
        let code = unsafe {
            fastpq_lde_cuda(
                coeffs.as_ptr(),
                column_count,
                trace_log,
                blowup_log,
                coset,
                out.as_mut_ptr(),
            )
        };
        map_cuda(code)
    }

    pub(super) fn poseidon_permute(states: &mut [u64], state_count: usize) -> Result<()> {
        let code = unsafe { fastpq_poseidon_permute_cuda(states.as_mut_ptr(), state_count) };
        map_cuda(code)
    }

    pub(super) fn poseidon_hash_columns(
        payloads: &[u64],
        slices: &[PoseidonColumnSlice],
        column_count: usize,
        block_count: usize,
        out: &mut [u64],
    ) -> Result<()> {
        let code = unsafe {
            fastpq_poseidon_hash_columns_cuda(
                payloads.as_ptr(),
                slices.as_ptr(),
                column_count,
                block_count,
                out.as_mut_ptr(),
            )
        };
        map_cuda(code)
    }

    pub(super) fn poseidon_hash_columns_fused(
        payloads: &[u64],
        slices: &[PoseidonColumnSlice],
        column_count: usize,
        block_count: usize,
        out: &mut [u64],
    ) -> Result<()> {
        let code = unsafe {
            fastpq_poseidon_hash_columns_fused_cuda(
                payloads.as_ptr(),
                slices.as_ptr(),
                column_count,
                block_count,
                out.as_mut_ptr(),
            )
        };
        map_cuda(code)
    }
}

#[cfg(any(not(feature = "fastpq-gpu"), fastpq_cuda_unavailable))]
mod native {
    use super::{CudaBackendError, Result};
    #[cfg(feature = "fastpq-gpu")]
    use crate::trace::PoseidonColumnSlice;

    pub(super) fn fft(_elements: &mut [u64], _column_count: usize, _log_size: u32) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn ifft(_elements: &mut [u64], _column_count: usize, _log_size: u32) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn lde(
        _coeffs: &[u64],
        _column_count: usize,
        _trace_log: u32,
        _blowup_log: u32,
        _coset: u64,
        _out: &mut [u64],
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn poseidon_permute(_states: &mut [u64], _state_count: usize) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn poseidon_hash_columns(
        _payloads: &[u64],
        _slices: &[PoseidonColumnSlice],
        _column_count: usize,
        _block_count: usize,
        _out: &mut [u64],
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn poseidon_hash_columns_fused(
        _payloads: &[u64],
        _slices: &[PoseidonColumnSlice],
        _column_count: usize,
        _block_count: usize,
        _out: &mut [u64],
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }
}

fn validate_dense(buffer_len: usize, column_count: usize, log_size: u32) -> Result<(usize, usize)> {
    let extent = 1usize
        .checked_shl(log_size)
        .ok_or(CudaBackendError::Unavailable)?;
    let expected = column_count
        .checked_mul(extent)
        .ok_or(CudaBackendError::Unavailable)?;
    if buffer_len != expected {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(expected),
            got: usize_to_u32(buffer_len),
        });
    }
    Ok((extent, expected))
}

fn usize_to_u32(value: usize) -> u32 {
    let capped = value.min(u32::MAX as usize);
    u32::try_from(capped).unwrap_or(u32::MAX)
}

/// Safe wrapper for the forward FFT (column-major layout).
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when `elements` does not match the expected
/// dense column-major layout, or [`CudaBackendError::Unavailable`] / [`CudaBackendError::Cuda`]
/// if the GPU backend cannot service the request.
pub fn fastpq_fft(elements: &mut [u64], column_count: usize, log_size: u32) -> Result<()> {
    validate_dense(elements.len(), column_count, log_size)?;
    native::fft(elements, column_count, log_size)
}

/// Safe wrapper for the inverse FFT (column-major layout).
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when `elements` does not match the expected
/// dense layout, or propagates [`CudaBackendError::Unavailable`] / [`CudaBackendError::Cuda`]
/// when the CUDA backend is not available or fails.
pub fn fastpq_ifft(elements: &mut [u64], column_count: usize, log_size: u32) -> Result<()> {
    validate_dense(elements.len(), column_count, log_size)?;
    native::ifft(elements, column_count, log_size)
}

/// Safe wrapper for the coset low-degree extension.
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when either the coefficient or evaluation buffers
/// do not match the expected lengths, or forwards [`CudaBackendError::Unavailable`] /
/// [`CudaBackendError::Cuda`] when GPU acceleration is not compiled in or reports an error.
pub fn fastpq_lde(
    coeffs: &[u64],
    column_count: usize,
    trace_log: u32,
    blowup_log: u32,
    coset: u64,
    out: &mut [u64],
) -> Result<()> {
    let (trace_len, expected_coeffs) = validate_dense(coeffs.len(), column_count, trace_log)?;
    let eval_extent_log = trace_log
        .checked_add(blowup_log)
        .ok_or(CudaBackendError::Unavailable)?;
    let (eval_len, expected_out) = validate_dense(out.len(), column_count, eval_extent_log)?;
    debug_assert_eq!(trace_len << blowup_log, eval_len);
    let _ = expected_coeffs;
    let _ = expected_out;
    native::lde(coeffs, column_count, trace_log, blowup_log, coset, out)
}

#[cfg(feature = "fastpq-gpu")]
fn validate_poseidon_states(len: usize) -> Result<usize> {
    const STATE_WIDTH: usize = 3;
    if !len.is_multiple_of(STATE_WIDTH) {
        let expected = (len.div_ceil(STATE_WIDTH))
            .saturating_mul(STATE_WIDTH)
            .min(u32::MAX as usize);
        let expected_u32 =
            u32::try_from(expected).expect("saturated Poseidon state count must fit in u32");
        let got_u32 = u32::try_from(len.min(u32::MAX as usize))
            .expect("saturated Poseidon state count must fit in u32");
        return Err(CudaBackendError::ShapeMismatch {
            expected: expected_u32,
            got: got_u32,
        });
    }
    Ok(len / STATE_WIDTH)
}

/// Safe wrapper for the Poseidon permutation over a batch of states.
#[cfg(feature = "fastpq-gpu")]
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when the input slice does not consist of whole
/// Poseidon states, or propagates [`CudaBackendError::Unavailable`] / [`CudaBackendError::Cuda`]
/// from the underlying CUDA kernels.
pub fn fastpq_poseidon_permute(states: &mut [u64]) -> Result<()> {
    if states.is_empty() {
        return Ok(());
    }
    let state_count = validate_poseidon_states(states.len())?;
    native::poseidon_permute(states, state_count)
}

#[cfg(feature = "fastpq-gpu")]
pub fn fastpq_poseidon_hash_columns(
    payloads: &[u64],
    slices: &[PoseidonColumnSlice],
    column_count: usize,
    block_count: usize,
    out: &mut [u64],
) -> Result<()> {
    if payloads.is_empty() || out.is_empty() || column_count == 0 || block_count == 0 {
        return Err(CudaBackendError::Unavailable);
    }
    if slices.len() != column_count {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(column_count),
            got: usize_to_u32(slices.len()),
        });
    }
    let per_column_elements = block_count
        .checked_mul(POSEIDON_RATE)
        .ok_or(CudaBackendError::Unavailable)?;
    if let Some(mismatch) = slices
        .iter()
        .find(|slice| slice.len() != per_column_elements)
    {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(per_column_elements),
            got: usize_to_u32(mismatch.len()),
        });
    }
    let expected_payload = slices
        .last()
        .map(|slice| slice.offset().saturating_add(slice.len()))
        .unwrap_or(0);
    if payloads.len() != expected_payload {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(expected_payload),
            got: usize_to_u32(payloads.len()),
        });
    }
    let expected_states = column_count
        .checked_mul(POSEIDON_STATE_WIDTH)
        .ok_or(CudaBackendError::Unavailable)?;
    if out.len() != expected_states {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(expected_states),
            got: usize_to_u32(out.len()),
        });
    }
    native::poseidon_hash_columns(payloads, slices, column_count, block_count, out)
}

#[cfg(feature = "fastpq-gpu")]
pub fn fastpq_poseidon_hash_columns_fused(
    payloads: &[u64],
    slices: &[PoseidonColumnSlice],
    column_count: usize,
    block_count: usize,
    out: &mut [u64],
) -> Result<()> {
    if payloads.is_empty() || out.is_empty() || column_count == 0 || block_count == 0 {
        return Err(CudaBackendError::Unavailable);
    }
    if slices.len() != column_count {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(column_count),
            got: usize_to_u32(slices.len()),
        });
    }
    let per_column_elements = block_count
        .checked_mul(POSEIDON_RATE)
        .ok_or(CudaBackendError::Unavailable)?;
    if let Some(mismatch) = slices
        .iter()
        .find(|slice| slice.len() != per_column_elements)
    {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(per_column_elements),
            got: usize_to_u32(mismatch.len()),
        });
    }
    let expected_payload = slices
        .last()
        .map(|slice| slice.offset().saturating_add(slice.len()))
        .unwrap_or(0);
    if payloads.len() != expected_payload {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(expected_payload),
            got: usize_to_u32(payloads.len()),
        });
    }
    let parents = column_count
        .checked_add(1)
        .ok_or(CudaBackendError::Unavailable)?
        / 2;
    let expected_hashes = column_count
        .checked_add(parents)
        .ok_or(CudaBackendError::Unavailable)?;
    if out.len() != expected_hashes {
        return Err(CudaBackendError::ShapeMismatch {
            expected: usize_to_u32(expected_hashes),
            got: usize_to_u32(out.len()),
        });
    }
    native::poseidon_hash_columns_fused(payloads, slices, column_count, block_count, out)
}
