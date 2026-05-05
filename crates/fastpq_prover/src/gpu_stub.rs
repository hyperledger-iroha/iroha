//! GPU stubs for CPU-only builds.
//!
//! When the `fastpq-gpu` feature is disabled the runtime still attempts to
//! negotiate GPU execution. These helpers provide deterministic fallbacks that
//! surface a consistent unsupported error so the planner can return to the CPU
//! path without linking GPU backends.

use core::marker::PhantomData;
use std::fmt;

use crate::backend::GpuBackend;

/// GPU execution failure used when GPU support is not compiled in.
#[derive(Debug, Clone)]
pub enum GpuError {
    /// The requested backend is not available in this build.
    Unsupported(GpuBackend),
    /// Kernel launch or runtime failure.
    Execution {
        backend: GpuBackend,
        message: String,
    },
    /// Inputs were malformed before dispatching work.
    InvalidInput(&'static str),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(backend) => {
                write!(f, "{backend:?} backend unavailable in CPU-only build")
            }
            Self::Execution { backend, message } => {
                write!(f, "{backend:?} backend failure: {message}")
            }
            Self::InvalidInput(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GpuError {}

/// Placeholder column dispatch returned when GPU support is disabled.
#[derive(Debug)]
pub struct ColumnDispatch<'a> {
    outcome: Result<(), GpuError>,
    _lifetime: PhantomData<&'a mut ()>,
}

impl<'a> ColumnDispatch<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ready() -> ColumnDispatch<'a> {
        Self {
            outcome: Ok(()),
            _lifetime: PhantomData,
        }
    }

    pub fn wait(self: ColumnDispatch<'a>) -> Result<(), GpuError> {
        self.outcome
    }
}

/// Placeholder LDE dispatch for CPU-only builds.
#[derive(Debug)]
pub struct LdeDispatch {
    outcome: Result<Option<Vec<Vec<u64>>>, GpuError>,
}

impl LdeDispatch {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ready(result: Option<Vec<Vec<u64>>>) -> Self {
        Self {
            outcome: Ok(result),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_error(error: GpuError) -> Self {
        Self {
            outcome: Err(error),
        }
    }

    pub fn wait(self) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
        self.outcome
    }
}

/// FFT stub that reports the backend as unsupported.
pub fn fft_columns(
    _columns: &mut [Vec<u64>],
    _log_size: u32,
    _root: u64,
    backend: GpuBackend,
) -> Result<(), GpuError> {
    Err(GpuError::Unsupported(backend))
}

/// Async FFT stub that reports unsupported backends.
pub fn fft_columns_async(
    _columns: &mut [Vec<u64>],
    _log_size: u32,
    _root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'_>, GpuError> {
    Err(GpuError::Unsupported(backend))
}

/// IFFT stub that reports the backend as unsupported.
pub fn ifft_columns(
    _columns: &mut [Vec<u64>],
    _log_size: u32,
    _root: u64,
    backend: GpuBackend,
) -> Result<(), GpuError> {
    Err(GpuError::Unsupported(backend))
}

/// Async IFFT stub that reports unsupported backends.
pub fn ifft_columns_async(
    _columns: &mut [Vec<u64>],
    _log_size: u32,
    _root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'_>, GpuError> {
    Err(GpuError::Unsupported(backend))
}

/// LDE stub that reports the backend as unsupported and leaves evaluation on the CPU.
pub fn lde_columns(
    _coeffs: &[Vec<u64>],
    _trace_log: u32,
    _blowup_log: u32,
    _lde_root: u64,
    _coset: u64,
    backend: GpuBackend,
) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
    Err(GpuError::Unsupported(backend))
}

/// Async LDE stub that reports unsupported backends.
pub fn lde_columns_async(
    _coeffs: &[Vec<u64>],
    _trace_log: u32,
    _blowup_log: u32,
    _lde_root: u64,
    _coset: u64,
    backend: GpuBackend,
) -> Result<LdeDispatch, GpuError> {
    Err(GpuError::Unsupported(backend))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_report_unsupported() {
        let backend = GpuBackend::Cuda;

        let mut fft_columns_buf = vec![vec![0u64]];
        assert!(matches!(
            fft_columns(&mut fft_columns_buf, 0, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));

        let mut ifft_columns_buf = vec![vec![0u64]];
        assert!(matches!(
            ifft_columns(&mut ifft_columns_buf, 0, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));

        let coeffs = vec![vec![0u64]];
        assert!(matches!(
            lde_columns(&coeffs, 0, 0, 1, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));

        assert!(matches!(
            fft_columns_async(&mut fft_columns_buf, 0, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));
        assert!(matches!(
            ifft_columns_async(&mut ifft_columns_buf, 0, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));
        assert!(matches!(
            lde_columns_async(&coeffs, 0, 0, 1, 1, backend).unwrap_err(),
            GpuError::Unsupported(GpuBackend::Cuda)
        ));
    }

    #[test]
    fn column_dispatch_ready_waits() {
        let dispatch = ColumnDispatch::ready();
        assert!(dispatch.wait().is_ok());
    }

    #[test]
    fn lde_dispatch_ready_yields_payload() {
        let payload = vec![vec![1u64, 2u64]];
        let dispatch = LdeDispatch::ready(Some(payload.clone()));
        let result = dispatch.wait().expect("wait succeeds").expect("payload");
        assert_eq!(result, payload);
    }
}
