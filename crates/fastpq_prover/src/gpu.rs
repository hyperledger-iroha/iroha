use std::fmt;

use fastpq_isi::poseidon::STATE_WIDTH;

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::metal;
use crate::{backend::GpuBackend, fastpq_cuda, trace::PoseidonColumnBatch};

/// GPU execution failure.
#[derive(Debug, Clone)]
pub enum GpuError {
    /// Backend is detected but not wired for acceleration yet.
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
            Self::Unsupported(backend) => write!(f, "{backend:?} backend unsupported"),
            Self::Execution { backend, message } => {
                write!(f, "{backend:?} backend failure: {message}")
            }
            Self::InvalidInput(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GpuError {}

/// Pending in-place column operation.
pub(crate) struct ColumnDispatch<'a> {
    inner: ColumnDispatchInner<'a>,
}

enum ColumnDispatchInner<'a> {
    Ready,
    Cuda(PendingCudaColumns<'a>),
    #[cfg(target_os = "macos")]
    Metal(metal::PendingColumns<'a>),
}

struct PendingCudaColumns<'a> {
    columns: &'a mut [Vec<u64>],
    extent: usize,
    buffer: Vec<u64>,
    pending: fastpq_cuda::PendingCudaDispatch,
}

impl PendingCudaColumns<'_> {
    fn wait(self) -> Result<(), GpuError> {
        let PendingCudaColumns {
            columns,
            extent,
            buffer,
            pending,
        } = self;
        pending.wait().map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
        restore(columns, &buffer, extent);
        Ok(())
    }
}

impl<'a> ColumnDispatch<'a> {
    pub(crate) fn ready() -> Self {
        Self {
            inner: ColumnDispatchInner::Ready,
        }
    }

    fn cuda(pending: PendingCudaColumns<'a>) -> Self {
        Self {
            inner: ColumnDispatchInner::Cuda(pending),
        }
    }

    #[cfg(target_os = "macos")]
    fn metal(pending: metal::PendingColumns<'a>) -> Self {
        Self {
            inner: ColumnDispatchInner::Metal(pending),
        }
    }

    pub fn wait(self) -> Result<(), GpuError> {
        match self.inner {
            ColumnDispatchInner::Ready => Ok(()),
            ColumnDispatchInner::Cuda(pending) => pending.wait(),
            #[cfg(target_os = "macos")]
            ColumnDispatchInner::Metal(pending) => pending.wait(),
        }
    }
}

/// Pending LDE evaluation dispatch.
pub(crate) struct LdeDispatch {
    inner: LdeDispatchInner,
}

enum LdeDispatchInner {
    Ready(Option<Vec<Vec<u64>>>),
    Cuda(PendingCudaLde),
    #[cfg(target_os = "macos")]
    Metal(metal::PendingLde),
    #[cfg(test)]
    TestError(GpuError),
}

struct PendingCudaLde {
    eval_len: usize,
    eval_buffer: Vec<u64>,
    pending: fastpq_cuda::PendingCudaDispatch,
}

impl PendingCudaLde {
    fn wait(self) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
        let PendingCudaLde {
            eval_len,
            eval_buffer,
            pending,
        } = self;
        pending.wait().map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
        let mut result = Vec::with_capacity(eval_buffer.len() / eval_len);
        for chunk in eval_buffer.chunks_exact(eval_len) {
            result.push(chunk.to_vec());
        }
        Ok(Some(result))
    }
}

impl LdeDispatch {
    pub(crate) fn ready(result: Option<Vec<Vec<u64>>>) -> Self {
        Self {
            inner: LdeDispatchInner::Ready(result),
        }
    }

    fn cuda(pending: PendingCudaLde) -> Self {
        Self {
            inner: LdeDispatchInner::Cuda(pending),
        }
    }

    #[cfg(target_os = "macos")]
    fn metal(pending: metal::PendingLde) -> Self {
        Self {
            inner: LdeDispatchInner::Metal(pending),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_error(error: GpuError) -> Self {
        Self {
            inner: LdeDispatchInner::TestError(error),
        }
    }

    pub fn wait(self) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
        match self.inner {
            LdeDispatchInner::Ready(result) => Ok(result),
            LdeDispatchInner::Cuda(pending) => pending.wait(),
            #[cfg(target_os = "macos")]
            LdeDispatchInner::Metal(pending) => pending.wait(),
            #[cfg(test)]
            LdeDispatchInner::TestError(err) => Err(err),
        }
    }
}

/// Execute an in-place FFT across the provided columns.
pub fn fft_columns(
    columns: &mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<(), GpuError> {
    if columns.is_empty() {
        return Ok(());
    }
    let len = columns[0].len();
    if columns.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }

    fft_columns_async(columns, log_size, root, backend)?.wait()
}

/// Initiate an FFT dispatch and return a guard that completes on [`ColumnDispatch::wait`].
pub fn fft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'a>, GpuError> {
    if columns.is_empty() {
        return Ok(ColumnDispatch::ready());
    }
    let len = columns[0].len();
    if columns.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }

    match backend {
        GpuBackend::Cuda => fft_cuda_async(columns, log_size, root),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => metal::fft_columns_async(columns, log_size).map(ColumnDispatch::metal),
        other => Err(GpuError::Unsupported(other)),
    }
}

/// Execute an in-place inverse FFT across the provided columns.
pub fn ifft_columns(
    columns: &mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<(), GpuError> {
    if columns.is_empty() {
        return Ok(());
    }
    let len = columns[0].len();
    if columns.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }

    ifft_columns_async(columns, log_size, root, backend)?.wait()
}

/// Initiate an IFFT dispatch, returning a pending guard.
pub fn ifft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'a>, GpuError> {
    if columns.is_empty() {
        return Ok(ColumnDispatch::ready());
    }
    let len = columns[0].len();
    if columns.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }

    match backend {
        GpuBackend::Cuda => ifft_cuda_async(columns, log_size, root),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            metal::ifft_columns_async(columns, log_size).map(ColumnDispatch::metal)
        }
        other => Err(GpuError::Unsupported(other)),
    }
}

/// Evaluate the low-degree extension columns on the GPU backend.
pub fn lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
    backend: GpuBackend,
) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
    if coeffs.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let len = coeffs[0].len();
    if coeffs.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput(
            "coefficient columns must share length",
        ));
    }

    lde_columns_async(coeffs, trace_log, blowup_log, lde_root, coset, backend)?.wait()
}

/// Initiate an LDE evaluation and return a pending guard.
pub fn lde_columns_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
    backend: GpuBackend,
) -> Result<LdeDispatch, GpuError> {
    if coeffs.is_empty() {
        return Ok(LdeDispatch::ready(Some(Vec::new())));
    }
    let len = coeffs[0].len();
    if coeffs.iter().any(|column| column.len() != len) {
        return Err(GpuError::InvalidInput(
            "coefficient columns must share length",
        ));
    }

    match backend {
        GpuBackend::Cuda => lde_cuda_async(coeffs, trace_log, blowup_log, lde_root, coset),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            metal::lde_columns_async(coeffs, trace_log, blowup_log, coset).map(LdeDispatch::metal)
        }
        other => Err(GpuError::Unsupported(other)),
    }
}

fn fft_cuda_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let extent = 1usize << log_size;
    let column_count = columns.len();
    let mut buffer = flatten(columns);
    let pending = fastpq_cuda::fastpq_fft_submit(&mut buffer, column_count, log_size, root)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
    Ok(ColumnDispatch::cuda(PendingCudaColumns {
        columns,
        extent,
        buffer,
        pending,
    }))
}

fn ifft_cuda_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let extent = 1usize << log_size;
    let column_count = columns.len();
    let mut buffer = flatten(columns);
    let pending = fastpq_cuda::fastpq_ifft_submit(&mut buffer, column_count, log_size, root)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
    Ok(ColumnDispatch::cuda(PendingCudaColumns {
        columns,
        extent,
        buffer,
        pending,
    }))
}

fn lde_cuda_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
) -> Result<LdeDispatch, GpuError> {
    let coeff_buffer = flatten(coeffs);
    let eval_len = 1usize << (trace_log + blowup_log);
    let mut eval_buffer = vec![0u64; coeffs.len() * eval_len];
    let pending = fastpq_cuda::fastpq_lde_submit(
        &coeff_buffer,
        coeffs.len(),
        trace_log,
        blowup_log,
        lde_root,
        coset,
        &mut eval_buffer,
    )
    .map_err(|err| GpuError::Execution {
        backend: GpuBackend::Cuda,
        message: err.to_string(),
    })?;
    Ok(LdeDispatch::cuda(PendingCudaLde {
        eval_len,
        eval_buffer,
        pending,
    }))
}

fn flatten(columns: &[Vec<u64>]) -> Vec<u64> {
    let len = columns.first().map_or(0, Vec::len);
    let mut buffer = Vec::with_capacity(columns.len() * len);
    for column in columns {
        buffer.extend_from_slice(column);
    }
    buffer
}

fn restore(columns: &mut [Vec<u64>], buffer: &[u64], extent: usize) {
    for (column, chunk) in columns.iter_mut().zip(buffer.chunks_exact(extent)) {
        column.copy_from_slice(chunk);
    }
}

pub fn poseidon_hash_columns(
    batch: &PoseidonColumnBatch,
    backend: GpuBackend,
) -> Result<Vec<u64>, GpuError> {
    match backend {
        GpuBackend::Cuda => poseidon_hash_columns_cuda(batch),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => metal::poseidon_hash_columns(batch),
        other => Err(GpuError::Unsupported(other)),
    }
}

fn poseidon_hash_columns_cuda(batch: &PoseidonColumnBatch) -> Result<Vec<u64>, GpuError> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    if batch.padded_len() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let _lane = crate::backend::acquire_gpu_lane();
    let mut states = vec![0u64; batch.columns() * STATE_WIDTH];
    fastpq_cuda::fastpq_poseidon_hash_columns(
        batch.payloads(),
        batch.offsets(),
        batch.columns(),
        batch.block_count(),
        &mut states,
    )
    .map_err(|err| GpuError::Execution {
        backend: GpuBackend::Cuda,
        message: err.to_string(),
    })?;
    Ok(states
        .chunks_exact(STATE_WIDTH)
        .map(|state| state[0])
        .collect())
}

pub fn poseidon_hash_columns_fused(
    batch: &PoseidonColumnBatch,
    backend: GpuBackend,
) -> Result<Vec<u64>, GpuError> {
    match backend {
        GpuBackend::Cuda => poseidon_hash_columns_fused_cuda(batch),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => metal::poseidon_hash_columns_fused(batch),
        other => Err(GpuError::Unsupported(other)),
    }
}

fn poseidon_hash_columns_fused_cuda(batch: &PoseidonColumnBatch) -> Result<Vec<u64>, GpuError> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    if batch.padded_len() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let parent_count = (batch.columns() + 1) / 2;
    let total = batch.columns() + parent_count;
    let _lane = crate::backend::acquire_gpu_lane();
    let mut hashes = vec![0u64; total];
    fastpq_cuda::fastpq_poseidon_hash_columns_fused(
        batch.payloads(),
        batch.offsets(),
        batch.columns(),
        batch.block_count(),
        &mut hashes,
    )
    .map_err(|err| GpuError::Execution {
        backend: GpuBackend::Cuda,
        message: err.to_string(),
    })?;
    Ok(hashes)
}

#[cfg(test)]
mod tests {
    use super::{ColumnDispatch, GpuBackend, GpuError, LdeDispatch};

    #[test]
    fn column_dispatch_ready_waits() {
        assert!(ColumnDispatch::ready().wait().is_ok());
    }

    #[test]
    fn lde_dispatch_ready_waits() {
        let ready = LdeDispatch::ready(Some(vec![vec![1, 2, 3]]));
        let result = ready.wait().expect("wait succeeds");
        assert_eq!(result.unwrap()[0], vec![1, 2, 3]);
    }

    #[test]
    fn lde_dispatch_from_error_returns_payload_error() {
        let dispatch = LdeDispatch::from_error(GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: "boom".into(),
        });
        let err = dispatch.wait().expect_err("error should surface");
        assert!(matches!(
            err,
            GpuError::Execution {
                backend: GpuBackend::Cuda,
                ..
            }
        ));
    }
}
