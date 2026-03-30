// SPDX-License-Identifier: Apache-2.0

//! CUDA bindings for the FASTPQ preview backend.
//!
//! The GPU backend is optional – enable the `fastpq-gpu` feature and provide a CUDA toolchain
//! (SM80+) to compile the kernels. When unavailable, all entry points return
//! [`CudaBackendError::Unavailable`] so the caller can fall back to the scalar implementation.

use core::fmt;
#[cfg(feature = "fastpq-gpu")]
use core::{convert::TryFrom, ffi::c_void, ptr::NonNull};

use crate::bn254::{self, BN254_LIMBS};
#[cfg(feature = "fastpq-gpu")]
use crate::trace::PoseidonColumnSlice;
/// Result alias for CUDA operations.
pub type Result<T> = core::result::Result<T, CudaBackendError>;

/// Errors surfaced by the CUDA backend wrappers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CudaBackendError {
    /// GPU support was not compiled in or deliberately skipped.
    Unavailable,
    /// The invocation failed local input validation before calling CUDA.
    InvalidInput(&'static str),
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
            Self::InvalidInput(message) => f.write_str(message),
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

#[cfg(feature = "fastpq-gpu")]
pub(crate) struct PendingCudaDispatch {
    handle: Option<NonNull<c_void>>,
}

#[cfg(feature = "fastpq-gpu")]
impl PendingCudaDispatch {
    fn new(handle: *mut c_void) -> Result<Self> {
        let handle = NonNull::new(handle).ok_or(CudaBackendError::Unavailable)?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub(crate) fn wait(mut self) -> Result<()> {
        let handle = self
            .handle
            .take()
            .expect("pending CUDA dispatch handle should be live until wait");
        native::pending_wait(handle.as_ptr())
    }
}

#[cfg(feature = "fastpq-gpu")]
impl Drop for PendingCudaDispatch {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = native::pending_wait(handle.as_ptr());
        }
    }
}

#[cfg(all(feature = "fastpq-gpu", not(fastpq_cuda_unavailable)))]
#[allow(unsafe_code)]
mod native {
    use super::{CudaBackendError, Result};
    use crate::bn254::BN254_LIMBS;
    use crate::trace::PoseidonColumnSlice;
    use core::{ffi::c_void, ptr};

    #[link(name = "fastpq_cuda", kind = "static")]
    unsafe extern "C" {
        fn fastpq_pending_wait_cuda(handle: *mut c_void) -> i32;
        fn fastpq_fft_async_submit_cuda(
            elements: *mut u64,
            column_count: usize,
            log_size: u32,
            root: u64,
            out_handle: *mut *mut c_void,
        ) -> i32;
        fn fastpq_ifft_async_submit_cuda(
            elements: *mut u64,
            column_count: usize,
            log_size: u32,
            root: u64,
            out_handle: *mut *mut c_void,
        ) -> i32;
        fn fastpq_lde_async_submit_cuda(
            coeffs: *const u64,
            column_count: usize,
            trace_log: u32,
            blowup_log: u32,
            lde_root: u64,
            coset: u64,
            out: *mut u64,
            out_handle: *mut *mut c_void,
        ) -> i32;
        fn fastpq_fft_cuda(
            elements: *mut u64,
            column_count: usize,
            log_size: u32,
            root: u64,
        ) -> i32;
        fn fastpq_ifft_cuda(
            elements: *mut u64,
            column_count: usize,
            log_size: u32,
            root: u64,
        ) -> i32;
        fn fastpq_lde_cuda(
            coeffs: *const u64,
            column_count: usize,
            trace_log: u32,
            blowup_log: u32,
            lde_root: u64,
            coset: u64,
            out: *mut u64,
        ) -> i32;
        fn fastpq_bn254_fft_cuda(
            elements: *mut u64,
            column_count: usize,
            log_size: u32,
            stage_twiddles: *const u64,
            stage_twiddle_len: usize,
        ) -> i32;
        fn fastpq_bn254_lde_cuda(
            coeffs: *const u64,
            column_count: usize,
            trace_log: u32,
            blowup_log: u32,
            stage_twiddles: *const u64,
            stage_twiddle_len: usize,
            coset: *const u64,
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

    pub(super) fn pending_wait(handle: *mut c_void) -> Result<()> {
        // SAFETY: the caller passes a handle previously returned by the matching submit wrapper.
        let code = unsafe { fastpq_pending_wait_cuda(handle) };
        map_cuda(code)
    }

    pub(super) fn fft_submit(
        elements: &mut [u64],
        column_count: usize,
        log_size: u32,
        root: u64,
    ) -> Result<*mut c_void> {
        let mut handle = ptr::null_mut();
        // SAFETY: the caller validated the dense buffer shape and the out-handle pointer is valid.
        let code = unsafe {
            fastpq_fft_async_submit_cuda(
                elements.as_mut_ptr(),
                column_count,
                log_size,
                root,
                &mut handle,
            )
        };
        map_cuda(code)?;
        Ok(handle)
    }

    pub(super) fn ifft_submit(
        elements: &mut [u64],
        column_count: usize,
        log_size: u32,
        root: u64,
    ) -> Result<*mut c_void> {
        let mut handle = ptr::null_mut();
        // SAFETY: the caller validated the dense buffer shape and the out-handle pointer is valid.
        let code = unsafe {
            fastpq_ifft_async_submit_cuda(
                elements.as_mut_ptr(),
                column_count,
                log_size,
                root,
                &mut handle,
            )
        };
        map_cuda(code)?;
        Ok(handle)
    }

    pub(super) fn lde_submit(
        coeffs: &[u64],
        column_count: usize,
        trace_log: u32,
        blowup_log: u32,
        lde_root: u64,
        coset: u64,
        out: &mut [u64],
    ) -> Result<*mut c_void> {
        let mut handle = ptr::null_mut();
        // SAFETY: the caller validated the dense coefficient and evaluation buffers.
        let code = unsafe {
            fastpq_lde_async_submit_cuda(
                coeffs.as_ptr(),
                column_count,
                trace_log,
                blowup_log,
                lde_root,
                coset,
                out.as_mut_ptr(),
                &mut handle,
            )
        };
        map_cuda(code)?;
        Ok(handle)
    }

    pub(super) fn fft(
        elements: &mut [u64],
        column_count: usize,
        log_size: u32,
        root: u64,
    ) -> Result<()> {
        // SAFETY: the caller validated the buffer shape and passes a live mutable slice.
        let code = unsafe { fastpq_fft_cuda(elements.as_mut_ptr(), column_count, log_size, root) };
        map_cuda(code)
    }

    pub(super) fn ifft(
        elements: &mut [u64],
        column_count: usize,
        log_size: u32,
        root: u64,
    ) -> Result<()> {
        // SAFETY: the caller validated the buffer shape and passes a live mutable slice.
        let code = unsafe { fastpq_ifft_cuda(elements.as_mut_ptr(), column_count, log_size, root) };
        map_cuda(code)
    }

    pub(super) fn lde(
        coeffs: &[u64],
        column_count: usize,
        trace_log: u32,
        blowup_log: u32,
        lde_root: u64,
        coset: u64,
        out: &mut [u64],
    ) -> Result<()> {
        // SAFETY: the caller validated the dense coefficient/input and output buffers.
        let code = unsafe {
            fastpq_lde_cuda(
                coeffs.as_ptr(),
                column_count,
                trace_log,
                blowup_log,
                lde_root,
                coset,
                out.as_mut_ptr(),
            )
        };
        map_cuda(code)
    }

    pub(super) fn bn254_fft(
        elements: &mut [u64],
        column_count: usize,
        log_size: u32,
        stage_twiddles: &[u64],
    ) -> Result<()> {
        // SAFETY: the caller validated the dense BN254 buffer shape and twiddle table length.
        let code = unsafe {
            fastpq_bn254_fft_cuda(
                elements.as_mut_ptr(),
                column_count,
                log_size,
                stage_twiddles.as_ptr(),
                stage_twiddles.len(),
            )
        };
        map_cuda(code)
    }

    pub(super) fn bn254_lde(
        coeffs: &[u64],
        column_count: usize,
        trace_log: u32,
        blowup_log: u32,
        stage_twiddles: &[u64],
        coset: &[u64; BN254_LIMBS],
        out: &mut [u64],
    ) -> Result<()> {
        // SAFETY: the caller validated the dense BN254 buffers, twiddle table, and coset limbs.
        let code = unsafe {
            fastpq_bn254_lde_cuda(
                coeffs.as_ptr(),
                column_count,
                trace_log,
                blowup_log,
                stage_twiddles.as_ptr(),
                stage_twiddles.len(),
                coset.as_ptr(),
                out.as_mut_ptr(),
            )
        };
        map_cuda(code)
    }

    pub(super) fn poseidon_permute(states: &mut [u64], state_count: usize) -> Result<()> {
        // SAFETY: the caller validated the state buffer width and passes a live mutable slice.
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
        // SAFETY: the caller validated payload/slice/output lengths and passes stable slices.
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
        // SAFETY: the caller validated payload/slice/output lengths and passes stable slices.
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
    use crate::bn254::BN254_LIMBS;
    #[cfg(feature = "fastpq-gpu")]
    use crate::trace::PoseidonColumnSlice;
    #[cfg(feature = "fastpq-gpu")]
    use core::ffi::c_void;

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn pending_wait(_handle: *mut c_void) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn fft_submit(
        _elements: &mut [u64],
        _column_count: usize,
        _log_size: u32,
        _root: u64,
    ) -> Result<*mut c_void> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn ifft_submit(
        _elements: &mut [u64],
        _column_count: usize,
        _log_size: u32,
        _root: u64,
    ) -> Result<*mut c_void> {
        Err(CudaBackendError::Unavailable)
    }

    #[cfg(feature = "fastpq-gpu")]
    pub(super) fn lde_submit(
        _coeffs: &[u64],
        _column_count: usize,
        _trace_log: u32,
        _blowup_log: u32,
        _lde_root: u64,
        _coset: u64,
        _out: &mut [u64],
    ) -> Result<*mut c_void> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn fft(
        _elements: &mut [u64],
        _column_count: usize,
        _log_size: u32,
        _root: u64,
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn ifft(
        _elements: &mut [u64],
        _column_count: usize,
        _log_size: u32,
        _root: u64,
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn lde(
        _coeffs: &[u64],
        _column_count: usize,
        _trace_log: u32,
        _blowup_log: u32,
        _lde_root: u64,
        _coset: u64,
        _out: &mut [u64],
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn bn254_fft(
        _elements: &mut [u64],
        _column_count: usize,
        _log_size: u32,
        _stage_twiddles: &[u64],
    ) -> Result<()> {
        Err(CudaBackendError::Unavailable)
    }

    pub(super) fn bn254_lde(
        _coeffs: &[u64],
        _column_count: usize,
        _trace_log: u32,
        _blowup_log: u32,
        _stage_twiddles: &[u64],
        _coset: &[u64; BN254_LIMBS],
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

fn validate_bn254_dense(
    buffer_len: usize,
    column_count: usize,
    log_size: u32,
) -> Result<(usize, usize)> {
    let extent = 1usize
        .checked_shl(log_size)
        .ok_or(CudaBackendError::Unavailable)?;
    let expected = column_count
        .checked_mul(extent)
        .and_then(|value| value.checked_mul(BN254_LIMBS))
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
pub fn fastpq_fft(
    elements: &mut [u64],
    column_count: usize,
    log_size: u32,
    root: u64,
) -> Result<()> {
    validate_dense(elements.len(), column_count, log_size)?;
    native::fft(elements, column_count, log_size, root)
}

#[cfg(feature = "fastpq-gpu")]
pub(crate) fn fastpq_fft_submit(
    elements: &mut [u64],
    column_count: usize,
    log_size: u32,
    root: u64,
) -> Result<PendingCudaDispatch> {
    validate_dense(elements.len(), column_count, log_size)?;
    PendingCudaDispatch::new(native::fft_submit(elements, column_count, log_size, root)?)
}

/// Safe wrapper for the inverse FFT (column-major layout).
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when `elements` does not match the expected
/// dense layout, or propagates [`CudaBackendError::Unavailable`] / [`CudaBackendError::Cuda`]
/// when the CUDA backend is not available or fails.
pub fn fastpq_ifft(
    elements: &mut [u64],
    column_count: usize,
    log_size: u32,
    root: u64,
) -> Result<()> {
    validate_dense(elements.len(), column_count, log_size)?;
    native::ifft(elements, column_count, log_size, root)
}

#[cfg(feature = "fastpq-gpu")]
pub(crate) fn fastpq_ifft_submit(
    elements: &mut [u64],
    column_count: usize,
    log_size: u32,
    root: u64,
) -> Result<PendingCudaDispatch> {
    validate_dense(elements.len(), column_count, log_size)?;
    PendingCudaDispatch::new(native::ifft_submit(elements, column_count, log_size, root)?)
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
    lde_root: u64,
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
    native::lde(
        coeffs,
        column_count,
        trace_log,
        blowup_log,
        lde_root,
        coset,
        out,
    )
}

#[cfg(feature = "fastpq-gpu")]
pub(crate) fn fastpq_lde_submit(
    coeffs: &[u64],
    column_count: usize,
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
    out: &mut [u64],
) -> Result<PendingCudaDispatch> {
    let (trace_len, expected_coeffs) = validate_dense(coeffs.len(), column_count, trace_log)?;
    let eval_extent_log = trace_log
        .checked_add(blowup_log)
        .ok_or(CudaBackendError::Unavailable)?;
    let (eval_len, expected_out) = validate_dense(out.len(), column_count, eval_extent_log)?;
    debug_assert_eq!(trace_len << blowup_log, eval_len);
    let _ = expected_coeffs;
    let _ = expected_out;
    PendingCudaDispatch::new(native::lde_submit(
        coeffs,
        column_count,
        trace_log,
        blowup_log,
        lde_root,
        coset,
        out,
    )?)
}

/// Safe wrapper for the BN254 forward FFT (column-major canonical limbs).
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when `elements` does not match the expected dense
/// canonical-limb layout, [`CudaBackendError::InvalidInput`] when `log_size` is unsupported, or
/// propagates [`CudaBackendError::Unavailable`] / [`CudaBackendError::Cuda`] from the CUDA
/// backend.
pub fn fastpq_bn254_fft(elements: &mut [u64], column_count: usize, log_size: u32) -> Result<()> {
    validate_bn254_dense(elements.len(), column_count, log_size)?;
    let twiddles = bn254::stage_twiddles_limbs(log_size).map_err(CudaBackendError::InvalidInput)?;
    let flat_twiddles = bn254::flatten_twiddles(&twiddles);
    native::bn254_fft(elements, column_count, log_size, &flat_twiddles)
}

/// Safe wrapper for the BN254 coset LDE (column-major canonical limbs).
///
/// # Errors
///
/// Returns [`CudaBackendError::ShapeMismatch`] when the coefficient or evaluation buffers do not
/// match the expected canonical-limb layout, [`CudaBackendError::InvalidInput`] when the trace
/// domain or coset are invalid, or propagates [`CudaBackendError::Unavailable`] /
/// [`CudaBackendError::Cuda`] from the CUDA backend.
pub fn fastpq_bn254_lde(
    coeffs: &[u64],
    column_count: usize,
    trace_log: u32,
    blowup_log: u32,
    coset: [u64; BN254_LIMBS],
    out: &mut [u64],
) -> Result<()> {
    let _ = validate_bn254_dense(coeffs.len(), column_count, trace_log)?;
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(CudaBackendError::Unavailable)?;
    let _ = validate_bn254_dense(out.len(), column_count, eval_log)?;
    let twiddles = bn254::stage_twiddles_limbs(eval_log).map_err(CudaBackendError::InvalidInput)?;
    let flat_twiddles = bn254::flatten_twiddles(&twiddles);
    let coset_scalar =
        bn254::scalar_from_canonical_limbs(&coset).map_err(CudaBackendError::InvalidInput)?;
    let canonical_coset = bn254::scalar_to_canonical_limbs(&coset_scalar);
    native::bn254_lde(
        coeffs,
        column_count,
        trace_log,
        blowup_log,
        &flat_twiddles,
        &canonical_coset,
        out,
    )
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

#[cfg(test)]
mod tests {
    use iroha_zkp_halo2::Bn254Scalar;

    use super::{
        CudaBackendError, fastpq_bn254_fft, fastpq_bn254_lde, usize_to_u32, validate_bn254_dense,
    };
    use crate::bn254::{
        BN254_LIMBS, canonical_to_scalars, cpu_fft, cpu_lde, sample_columns, sample_coset,
        scalar_from_canonical_limbs, scalars_to_canonical, stage_twiddles_scalars,
    };

    #[test]
    fn validate_bn254_dense_requires_full_limb_columns() {
        let err = validate_bn254_dense(3, 1, 0).expect_err("shape mismatch");
        assert_eq!(
            err,
            CudaBackendError::ShapeMismatch {
                expected: usize_to_u32(4),
                got: usize_to_u32(3),
            }
        );
    }

    #[test]
    fn fastpq_bn254_lde_rejects_invalid_coset() {
        let coeffs = vec![0u64; BN254_LIMBS << 1];
        let mut out = vec![0u64; BN254_LIMBS << 2];
        let err = fastpq_bn254_lde(
            &coeffs,
            1,
            1,
            1,
            [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
            &mut out,
        )
        .expect_err("invalid coset rejected");
        assert!(matches!(err, CudaBackendError::InvalidInput(_)));
    }

    #[test]
    fn fastpq_bn254_fft_matches_cpu_when_backend_available() {
        let log_size = 4;
        let column_count = 2;
        let mut gpu_columns = sample_columns(log_size, column_count);
        let mut cpu_columns: Vec<Vec<Bn254Scalar>> = gpu_columns
            .iter()
            .map(|column| canonical_to_scalars(column))
            .collect();
        let twiddles = stage_twiddles_scalars(log_size).expect("twiddles");
        cpu_fft(&mut cpu_columns, log_size, &twiddles);
        let cpu_expected = scalars_to_canonical(&cpu_columns);
        let mut dense = gpu_columns.concat();
        match fastpq_bn254_fft(&mut dense, column_count, log_size) {
            Ok(()) => {}
            Err(err @ (CudaBackendError::Unavailable | CudaBackendError::Cuda { .. })) => {
                eprintln!("skipping BN254 CUDA FFT parity test: {err}");
                return;
            }
            Err(err) => panic!("BN254 CUDA FFT failed: {err}"),
        }
        for (column, chunk) in gpu_columns
            .iter_mut()
            .zip(dense.chunks_exact(1usize << log_size << 2))
        {
            column.copy_from_slice(chunk);
        }
        assert_eq!(gpu_columns, cpu_expected);
    }

    #[test]
    fn fastpq_bn254_lde_matches_cpu_when_backend_available() {
        let trace_log = 3;
        let blowup_log = 2;
        let column_count = 2;
        let coeffs = sample_columns(trace_log, column_count);
        let coeff_scalars: Vec<Vec<Bn254Scalar>> = coeffs
            .iter()
            .map(|column| canonical_to_scalars(column))
            .collect();
        let twiddles = stage_twiddles_scalars(trace_log + blowup_log).expect("lde twiddles");
        let coset_limbs = sample_coset();
        let coset = scalar_from_canonical_limbs(&coset_limbs).expect("valid test coset");
        let cpu_eval = cpu_lde(&coeff_scalars, trace_log, blowup_log, &twiddles, coset);
        let cpu_expected = scalars_to_canonical(&cpu_eval);
        let mut out = vec![0u64; column_count * (1usize << (trace_log + blowup_log)) * BN254_LIMBS];
        match fastpq_bn254_lde(
            &coeffs.concat(),
            column_count,
            trace_log,
            blowup_log,
            coset_limbs,
            &mut out,
        ) {
            Ok(()) => {}
            Err(err @ (CudaBackendError::Unavailable | CudaBackendError::Cuda { .. })) => {
                eprintln!("skipping BN254 CUDA LDE parity test: {err}");
                return;
            }
            Err(err) => panic!("BN254 CUDA LDE failed: {err}"),
        }
        let eval_extent = (1usize << (trace_log + blowup_log)) * BN254_LIMBS;
        let gpu_eval: Vec<Vec<u64>> = out.chunks_exact(eval_extent).map(<[u64]>::to_vec).collect();
        assert_eq!(gpu_eval, cpu_expected);
    }
}
