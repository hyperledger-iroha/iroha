#![allow(clippy::elidable_lifetime_names, clippy::redundant_pub_crate)]
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::metal;
use crate::{backend::GpuBackend, fastpq_cuda, trace::PoseidonColumnBatch};
use fastpq_isi::poseidon::STATE_WIDTH;
use std::fmt;
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
    buffer: Option<Vec<u64>>,
    pending: Option<fastpq_cuda::PendingCudaDispatch>,
}
impl PendingCudaColumns<'_> {
    fn finish(&mut self) -> Result<(), GpuError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let buffer = self
            .buffer
            .as_mut()
            .expect("pending CUDA column output should live until completion");
        if let Err(err) = pending.wait(buffer) {
            self.buffer.take();
            return Err(GpuError::Execution {
                backend: GpuBackend::Cuda,
                message: err.to_string(),
            });
        }
        let buffer = self
            .buffer
            .take()
            .expect("completed CUDA column output should remain available");
        restore(self.columns, &buffer, self.extent);
        Ok(())
    }

    fn wait(mut self) -> Result<(), GpuError> {
        self.finish()
    }
}
impl Drop for PendingCudaColumns<'_> {
    fn drop(&mut self) {
        let _ = self.finish();
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
    eval_buffer: Option<Vec<u64>>,
    pending: Option<fastpq_cuda::PendingCudaDispatch>,
}
impl PendingCudaLde {
    fn complete_output(&mut self) -> Result<(), GpuError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let eval_buffer = self
            .eval_buffer
            .as_mut()
            .expect("pending CUDA LDE output should live until completion");
        if let Err(err) = pending.wait(eval_buffer) {
            self.eval_buffer.take();
            return Err(GpuError::Execution {
                backend: GpuBackend::Cuda,
                message: err.to_string(),
            });
        }
        Ok(())
    }

    fn wait(mut self) -> Result<Option<Vec<Vec<u64>>>, GpuError> {
        self.complete_output()?;
        let eval_buffer = self
            .eval_buffer
            .take()
            .expect("completed CUDA LDE output should remain available");
        let column_count = eval_buffer.len() / self.eval_len;
        let mut result = Vec::new();
        result.try_reserve_exact(column_count).map_err(|_| {
            GpuError::InvalidInput("CUDA LDE result list exceeds available host memory")
        })?;
        for chunk in eval_buffer.chunks_exact(self.eval_len) {
            let mut column = Vec::new();
            column.try_reserve_exact(chunk.len()).map_err(|_| {
                GpuError::InvalidInput("CUDA LDE result column exceeds available host memory")
            })?;
            column.extend_from_slice(chunk);
            result.push(column);
        }
        Ok(Some(result))
    }
}
impl Drop for PendingCudaLde {
    fn drop(&mut self) {
        let _ = self.complete_output();
        self.eval_buffer.take();
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
    fft_columns_async(columns, log_size, root, backend)?.wait()
}
/// Initiate an FFT dispatch and return a guard that completes on [`ColumnDispatch::wait`].
pub fn fft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let shape = preflight_fft_columns(columns, log_size)?;
    if columns.is_empty() {
        return Ok(ColumnDispatch::ready());
    }
    match backend {
        GpuBackend::Cuda => fft_cuda_async(columns, log_size, root, shape),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            metal::fft_columns_async(columns, log_size, root).map(ColumnDispatch::metal)
        }
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
    ifft_columns_async(columns, log_size, root, backend)?.wait()
}
/// Initiate an IFFT dispatch, returning a pending guard.
pub fn ifft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    backend: GpuBackend,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let shape = preflight_fft_columns(columns, log_size)?;
    if columns.is_empty() {
        return Ok(ColumnDispatch::ready());
    }
    match backend {
        GpuBackend::Cuda => ifft_cuda_async(columns, log_size, root, shape),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            metal::ifft_columns_async(columns, log_size, root).map(ColumnDispatch::metal)
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
    let shape = preflight_lde_columns(coeffs, trace_log, blowup_log)?;
    if coeffs.is_empty() {
        return Ok(LdeDispatch::ready(Some(Vec::new())));
    }
    match backend {
        GpuBackend::Cuda => lde_cuda_async(coeffs, trace_log, blowup_log, lde_root, coset, shape),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            metal::lde_columns_async(coeffs, trace_log, blowup_log, lde_root, coset)
                .map(LdeDispatch::metal)
        }
        other => Err(GpuError::Unsupported(other)),
    }
}
#[derive(Clone, Copy)]
struct DenseColumnShape {
    extent: usize,
    total_len: usize,
}
#[derive(Clone, Copy)]
struct LdeColumnShape {
    coefficients: DenseColumnShape,
    eval_len: usize,
    eval_total_len: usize,
}
fn checked_extent(log_size: u32, error: &'static str) -> Result<usize, GpuError> {
    1usize
        .checked_shl(log_size)
        .ok_or(GpuError::InvalidInput(error))
}
fn checked_cardinality(
    column_count: usize,
    extent: usize,
    error: &'static str,
) -> Result<usize, GpuError> {
    column_count
        .checked_mul(extent)
        .ok_or(GpuError::InvalidInput(error))
}
fn validate_dense_columns(
    columns: &[Vec<u64>],
    extent: usize,
    shared_length_error: &'static str,
    requested_extent_error: &'static str,
    cardinality_error: &'static str,
) -> Result<DenseColumnShape, GpuError> {
    if let Some(first) = columns.first() {
        if columns.iter().any(|column| column.len() != first.len()) {
            return Err(GpuError::InvalidInput(shared_length_error));
        }
        if first.len() != extent {
            return Err(GpuError::InvalidInput(requested_extent_error));
        }
    }
    let total_len = checked_cardinality(columns.len(), extent, cardinality_error)?;
    Ok(DenseColumnShape { extent, total_len })
}
fn preflight_fft_columns(
    columns: &[Vec<u64>],
    log_size: u32,
) -> Result<DenseColumnShape, GpuError> {
    let extent = checked_extent(log_size, "FFT log size exceeds host address width")?;
    validate_dense_columns(
        columns,
        extent,
        "columns must share length",
        "column length does not match requested FFT extent",
        "FFT column cardinality exceeds host address width",
    )
}
fn preflight_lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
) -> Result<LdeColumnShape, GpuError> {
    if blowup_log == 0 {
        return Err(GpuError::InvalidInput(
            "LDE requires a positive blowup factor",
        ));
    }
    let trace_len = checked_extent(trace_log, "LDE trace log size exceeds host address width")?;
    let coefficients = validate_dense_columns(
        coeffs,
        trace_len,
        "coefficient columns must share length",
        "coefficient column length does not match requested trace extent",
        "LDE coefficient cardinality exceeds host address width",
    )?;
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput("LDE trace and blowup logs overflow"))?;
    let eval_len = checked_extent(
        eval_log,
        "LDE evaluation log size exceeds host address width",
    )?;
    let eval_total_len = checked_cardinality(
        coeffs.len(),
        eval_len,
        "LDE evaluation cardinality exceeds host address width",
    )?;
    Ok(LdeColumnShape {
        coefficients,
        eval_len,
        eval_total_len,
    })
}
fn fft_cuda_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    shape: DenseColumnShape,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let column_count = columns.len();
    let mut buffer = flatten(columns, shape.total_len)?;
    let pending = fastpq_cuda::fastpq_fft_submit(&mut buffer, column_count, log_size, root)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
    Ok(ColumnDispatch::cuda(PendingCudaColumns {
        columns,
        extent: shape.extent,
        buffer: Some(buffer),
        pending: Some(pending),
    }))
}
fn ifft_cuda_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    shape: DenseColumnShape,
) -> Result<ColumnDispatch<'a>, GpuError> {
    let column_count = columns.len();
    let mut buffer = flatten(columns, shape.total_len)?;
    let pending = fastpq_cuda::fastpq_ifft_submit(&mut buffer, column_count, log_size, root)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Cuda,
            message: err.to_string(),
        })?;
    Ok(ColumnDispatch::cuda(PendingCudaColumns {
        columns,
        extent: shape.extent,
        buffer: Some(buffer),
        pending: Some(pending),
    }))
}
fn lde_cuda_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
    shape: LdeColumnShape,
) -> Result<LdeDispatch, GpuError> {
    let coeff_buffer = flatten(coeffs, shape.coefficients.total_len)?;
    let mut eval_buffer = Vec::new();
    eval_buffer
        .try_reserve_exact(shape.eval_total_len)
        .map_err(|_| GpuError::InvalidInput("CUDA LDE output exceeds available host memory"))?;
    eval_buffer.resize(shape.eval_total_len, 0);
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
        eval_len: shape.eval_len,
        eval_buffer: Some(eval_buffer),
        pending: Some(pending),
    }))
}
fn flatten(columns: &[Vec<u64>], total_len: usize) -> Result<Vec<u64>, GpuError> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(total_len)
        .map_err(|_| GpuError::InvalidInput("CUDA staging buffer exceeds available host memory"))?;
    for column in columns {
        buffer.extend_from_slice(column);
    }
    debug_assert_eq!(buffer.len(), total_len);
    Ok(buffer)
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
        GpuBackend::Metal => {
            let _lane = crate::backend::acquire_gpu_lane();
            metal::poseidon_hash_columns(batch)
        }
        other => Err(GpuError::Unsupported(other)),
    }
}
pub fn poseidon_hash_rows(columns: &[Vec<u64>], backend: GpuBackend) -> Result<Vec<u64>, GpuError> {
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_count = columns[0].len();
    if columns.iter().any(|column| column.len() != row_count) {
        return Err(GpuError::InvalidInput("columns must share row length"));
    }
    if row_count == 0 {
        return Ok(Vec::new());
    }
    match backend {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            let _lane = crate::backend::acquire_gpu_lane();
            metal::poseidon_hash_rows(columns)
        }
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
/// Dispatch the low-level fused leaf-plus-parent Poseidon kernel.
///
/// The production trace commitment path uses `trace::hash_columns_gpu_fused`, which composes the
/// parity-proven column batch and Merkle-pair helpers. This lower-level hook remains available for
/// backend parity tests and throughput experiments.
#[allow(dead_code)]
pub fn poseidon_hash_columns_fused(
    batch: &PoseidonColumnBatch,
    backend: GpuBackend,
) -> Result<Vec<u64>, GpuError> {
    match backend {
        GpuBackend::Cuda => poseidon_hash_columns_fused_cuda(batch),
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        GpuBackend::Metal => {
            let _lane = crate::backend::acquire_gpu_lane();
            metal::poseidon_hash_columns_fused(batch)
        }
        other => Err(GpuError::Unsupported(other)),
    }
}
/// CUDA implementation for the low-level fused leaf-plus-parent Poseidon hook.
#[allow(dead_code)]
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
    let parent_count = batch.columns().div_ceil(2);
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
    use super::{
        ColumnDispatch, GpuBackend, GpuError, LdeDispatch, PendingCudaColumns, PendingCudaLde,
        checked_cardinality, fft_columns_async, ifft_columns_async, lde_columns_async,
    };
    use crate::fastpq_cuda::PendingCudaDispatch;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
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
    fn dropping_cuda_column_dispatch_waits_before_releasing_output() {
        let waited = Arc::new(AtomicBool::new(false));
        let waited_by_hook = Arc::clone(&waited);
        let mut columns = vec![vec![1, 2]];
        {
            let pending = PendingCudaDispatch::from_wait_hook(2, move |output| {
                output.copy_from_slice(&[9, 10]);
                waited_by_hook.store(true, Ordering::Release);
            });
            let dispatch = ColumnDispatch::cuda(PendingCudaColumns {
                columns: &mut columns,
                extent: 2,
                buffer: Some(vec![0; 2]),
                pending: Some(pending),
            });
            drop(dispatch);
        }
        assert!(waited.load(Ordering::Acquire));
        assert_eq!(columns, vec![vec![9, 10]]);
    }
    #[test]
    fn dropping_cuda_lde_dispatch_waits_without_materializing_results() {
        let waited = Arc::new(AtomicBool::new(false));
        let waited_by_hook = Arc::clone(&waited);
        let pending = PendingCudaDispatch::from_wait_hook(4, move |output| {
            output.copy_from_slice(&[1, 2, 3, 4]);
            waited_by_hook.store(true, Ordering::Release);
        });
        let dispatch = LdeDispatch::cuda(PendingCudaLde {
            eval_len: 4,
            eval_buffer: Some(vec![0; 4]),
            pending: Some(pending),
        });
        drop(dispatch);
        assert!(waited.load(Ordering::Acquire));
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
    #[test]
    fn fft_preflight_rejects_oversized_logs_before_backend_selection() {
        let mut fft = vec![vec![0u64]];
        assert!(matches!(
            fft_columns_async(&mut fft, usize::BITS, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "FFT log size exceeds host address width"
            ))
        ));

        let mut ifft = vec![vec![0u64]];
        assert!(matches!(
            ifft_columns_async(&mut ifft, usize::BITS, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "FFT log size exceeds host address width"
            ))
        ));
    }
    #[test]
    fn lde_preflight_rejects_log_addition_overflow_before_backend_selection() {
        let coeffs = vec![vec![0u64; 2]];
        assert!(matches!(
            lde_columns_async(&coeffs, 1, u32::MAX, 1, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput("LDE trace and blowup logs overflow"))
        ));
    }
    #[test]
    fn lde_preflight_rejects_zero_blowup_for_empty_and_nonempty_inputs() {
        let empty = Vec::<Vec<u64>>::new();
        assert!(matches!(
            lde_columns_async(&empty, 1, 0, 1, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "LDE requires a positive blowup factor"
            ))
        ));

        let coeffs = vec![vec![0u64; 2]];
        assert!(matches!(
            lde_columns_async(&coeffs, 1, 0, 1, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "LDE requires a positive blowup factor"
            ))
        ));
    }
    #[test]
    fn transform_preflight_rejects_mismatched_requested_extents() {
        let mut fft = vec![vec![0u64; 2]];
        assert!(matches!(
            fft_columns_async(&mut fft, 2, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "column length does not match requested FFT extent"
            ))
        ));

        let mut ifft = vec![vec![0u64; 2]];
        assert!(matches!(
            ifft_columns_async(&mut ifft, 2, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "column length does not match requested FFT extent"
            ))
        ));

        let coeffs = vec![vec![0u64; 2]];
        assert!(matches!(
            lde_columns_async(&coeffs, 2, 1, 1, 1, GpuBackend::OpenCl),
            Err(GpuError::InvalidInput(
                "coefficient column length does not match requested trace extent"
            ))
        ));
    }
    #[test]
    fn transform_preflight_rejects_overflowing_cardinality() {
        assert!(matches!(
            checked_cardinality(usize::MAX, 2, "cardinality overflow"),
            Err(GpuError::InvalidInput("cardinality overflow"))
        ));
    }
}
