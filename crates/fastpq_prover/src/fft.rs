//! Column-aware FFT and LDE planner for FASTPQ.
//!
//! Stage 1 promotes the domain helpers introduced in Stage 0 into a
//! multicolumn planner that drives the prover’s polynomial pipeline.  The
//! planner validates catalogue metadata, exposes parallel FFT/IFFT helpers,
//! and evaluates coefficient columns onto the canonical low-degree extension
//! coset.

use core::convert::TryFrom;
use std::sync::MutexGuard;

use fastpq_isi::StarkParameterSet;
use rayon::{join, prelude::*};
use tracing::{debug, info, warn};

use crate::{
    backend,
    cyclotomic::{self, Domain},
    gpu,
    poseidon::FIELD_MODULUS,
};

/// Column-oriented FFT planner.
#[derive(Clone)]
pub struct Planner {
    params: StarkParameterSet,
    blowup_log: u32,
    trace_domains: Vec<Domain>,
    lde_domains: Vec<Domain>,
}

/// Handle that tracks an in-flight GPU FFT/IFFT dispatch.
pub struct GpuColumnDispatch<'a> {
    dispatch: gpu::ColumnDispatch<'a>,
    lane_guard: Option<MutexGuard<'static, ()>>,
}

impl<'a> GpuColumnDispatch<'a> {
    fn new(dispatch: gpu::ColumnDispatch<'a>, lane_guard: MutexGuard<'static, ()>) -> Self {
        Self {
            dispatch,
            lane_guard: Some(lane_guard),
        }
    }

    /// Wait for the GPU kernel to complete and release the GPU lane.
    pub fn wait(self) -> Result<(), gpu::GpuError> {
        let Self {
            dispatch,
            lane_guard,
        } = self;
        let result = dispatch.wait();
        drop(lane_guard);
        result
    }

    #[cfg(test)]
    pub(crate) fn from_ready(dispatch: gpu::ColumnDispatch<'a>) -> Self {
        Self {
            dispatch,
            lane_guard: None,
        }
    }
}

/// Handle that tracks an in-flight GPU LDE dispatch.
pub struct GpuLdeDispatch {
    dispatch: gpu::LdeDispatch,
    lane_guard: Option<MutexGuard<'static, ()>>,
}

impl GpuLdeDispatch {
    fn new(dispatch: gpu::LdeDispatch, lane_guard: MutexGuard<'static, ()>) -> Self {
        Self {
            dispatch,
            lane_guard: Some(lane_guard),
        }
    }

    /// Wait for the GPU LDE to finish and release the GPU lane.
    pub fn wait(self) -> Result<Option<Vec<Vec<u64>>>, gpu::GpuError> {
        let Self {
            dispatch,
            lane_guard,
        } = self;
        let result = dispatch.wait();
        drop(lane_guard);
        result
    }

    #[cfg(test)]
    pub(crate) fn from_ready(dispatch: gpu::LdeDispatch) -> Self {
        Self {
            dispatch,
            lane_guard: None,
        }
    }
}

impl Planner {
    /// Construct a planner from a canonical parameter set.
    ///
    /// Validates the provided roots of unity and guards against unsupported
    /// two-adicity so misconfigured catalogues surface immediately.
    pub fn new(params: &StarkParameterSet) -> Self {
        assert!(
            params.trace_log_size <= 32,
            "trace domain exceeds supported two-adicity ({} > 32)",
            params.trace_log_size
        );
        assert!(
            params.lde_log_size <= 32,
            "lde domain exceeds supported two-adicity ({} > 32)",
            params.lde_log_size
        );

        verify_root(params.trace_root, params.trace_log_size, "trace_root");
        verify_root(params.lde_root, params.lde_log_size, "lde_root");

        let blowup = params.fri.blowup_factor;
        assert!(
            blowup.is_power_of_two(),
            "FRI blowup factor must be a power of two (got {blowup})"
        );
        let blowup_log = blowup.trailing_zeros();
        assert_eq!(
            params.trace_log_size + blowup_log,
            params.lde_log_size,
            "lde log size must equal trace log size plus log2(blowup)"
        );

        let trace_domains = build_domain_cache(params.trace_root, params.trace_log_size);
        let lde_domains = build_domain_cache(params.lde_root, params.lde_log_size);

        let planner = Self {
            params: *params,
            blowup_log,
            trace_domains,
            lde_domains,
        };

        debug!(
            target: "fastpq::planner",
            parameter = params.name,
            trace_log = params.trace_log_size,
            lde_log = params.lde_log_size,
            blowup,
            "initialised planner"
        );

        planner
    }

    /// Returns the canonical parameters backing the planner.
    pub fn params(&self) -> &StarkParameterSet {
        &self.params
    }

    /// Log₂ of the configured blowup factor.
    pub fn blowup_log(&self) -> u32 {
        self.blowup_log
    }

    /// Perform an in-place FFT over every column.
    ///
    /// The supplied columns must share a power-of-two length not exceeding the
    /// catalogue’s trace domain. Execution is parallelised with Rayon.
    pub fn fft_columns(&self, columns: &mut [Vec<u64>]) {
        let (trace_len, trace_log) = self.validate_columns(columns);
        self.fft_columns_cpu(columns, trace_len, trace_log);
    }

    fn fft_columns_cpu(&self, columns: &mut [Vec<u64>], trace_len: usize, trace_log: u32) {
        if trace_len == 0 {
            return;
        }
        let domain = self.trace_domain(trace_log);

        debug!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            columns = columns.len(),
            trace_log,
            "fft columns"
        );

        columns.par_iter_mut().for_each(|column| {
            assert_eq!(column.len(), trace_len, "column length mismatch during FFT");
            cyclotomic::fft(column, domain);
        });
    }

    /// Attempt a GPU-accelerated FFT over the provided columns.
    pub fn fft_gpu(&self, columns: &mut [Vec<u64>]) {
        let (trace_len, trace_log) = self.validate_columns(columns);
        if trace_len == 0 {
            return;
        }
        let column_count = columns.len();

        if let Some(backend) = backend::current_gpu_backend() {
            if let Some(permit) = backend::try_acquire_gpu_lane() {
                match self.start_fft_gpu_dispatch(columns, trace_log, backend, permit) {
                    Ok(dispatch) => match dispatch.wait() {
                        Ok(()) => {
                            debug!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                trace_len,
                                columns = column_count,
                                trace_log,
                                backend = ?backend,
                                "fft columns via gpu"
                            );
                            return;
                        }
                        Err(err) => {
                            warn!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                trace_len,
                                columns = column_count,
                                trace_log,
                                backend = ?backend,
                                error = %err,
                                "gpu fft wait failed; falling back to cpu"
                            );
                        }
                    },
                    Err(err) => {
                        warn!(
                            target: "fastpq::planner",
                            parameter = self.params.name,
                            trace_len,
                            columns = column_count,
                            trace_log,
                            backend = ?backend,
                            error = %err,
                            "gpu fft failed; falling back to cpu"
                        );
                    }
                }
            } else if columns.len() >= 2 {
                self.split_fft_gpu_cpu(columns, trace_len, trace_log, backend);
                return;
            } else {
                debug!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = columns.len(),
                    trace_log,
                    "gpu lane busy; running fft on cpu"
                );
            }
        } else {
            debug!(
                target: "fastpq::planner",
                parameter = self.params.name,
                trace_len,
                columns = columns.len(),
                trace_log,
                "no gpu backend detected; falling back to cpu fft"
            );
        }
        self.fft_columns_cpu(columns, trace_len, trace_log);
    }

    /// Stage a GPU FFT and return a pending dispatch if available.
    pub fn fft_gpu_pending<'a>(
        &self,
        columns: &'a mut [Vec<u64>],
    ) -> Option<GpuColumnDispatch<'a>> {
        let (trace_len, trace_log) = self.validate_columns(columns);
        if trace_len == 0 {
            return None;
        }
        let backend = backend::current_gpu_backend()?;
        let permit = backend::try_acquire_gpu_lane()?;
        let column_count = columns.len();
        match self.start_fft_gpu_dispatch(columns, trace_log, backend, permit) {
            Ok(dispatch) => Some(dispatch),
            Err(err) => {
                warn!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = column_count,
                    trace_log,
                    backend = ?backend,
                    error = %err,
                    "gpu fft failed; pending dispatch unavailable"
                );
                None
            }
        }
    }

    fn split_fft_gpu_cpu(
        &self,
        columns: &mut [Vec<u64>],
        trace_len: usize,
        trace_log: u32,
        backend: backend::GpuBackend,
    ) {
        let total_columns = columns.len();
        let split = columns.len() / 2;
        if split == 0 {
            self.fft_columns_cpu(columns, trace_len, trace_log);
            return;
        }
        let (cpu_slice, gpu_slice) = columns.split_at_mut(split);
        info!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            columns = total_columns,
            trace_log,
            cpu_columns = cpu_slice.len(),
            gpu_columns = gpu_slice.len(),
            "gpu lane busy; splitting fft workload across cpu/gpu"
        );
        let ((), gpu_result) = join(
            || self.fft_columns_cpu(cpu_slice, trace_len, trace_log),
            || {
                let guard = backend::acquire_gpu_lane();
                let result = gpu::fft_columns(
                    gpu_slice,
                    trace_log,
                    self.trace_domain(trace_log).generator,
                    backend,
                );
                drop(guard);
                result
            },
        );
        if let Err(err) = gpu_result {
            warn!(
                target: "fastpq::planner",
                parameter = self.params.name,
                trace_len,
                columns = total_columns,
                trace_log,
                backend = ?backend,
                error = %err,
                "gpu fft split failed; cpu handled remaining columns"
            );
            self.fft_columns_cpu(gpu_slice, trace_len, trace_log);
        }
    }

    /// Perform an in-place inverse FFT over every column.
    pub fn ifft_columns(&self, columns: &mut [Vec<u64>]) {
        let (trace_len, trace_log) = self.validate_columns(columns);
        self.ifft_columns_cpu(columns, trace_len, trace_log);
    }

    fn ifft_columns_cpu(&self, columns: &mut [Vec<u64>], trace_len: usize, trace_log: u32) {
        if trace_len == 0 {
            return;
        }
        let domain = self.trace_domain(trace_log);

        debug!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            columns = columns.len(),
            trace_log,
            "ifft columns"
        );

        columns.par_iter_mut().for_each(|column| {
            assert_eq!(
                column.len(),
                trace_len,
                "column length mismatch during IFFT"
            );
            cyclotomic::ifft(column, domain);
        });
    }

    /// Attempt a GPU-accelerated inverse FFT over the provided columns.
    pub fn ifft_gpu(&self, columns: &mut [Vec<u64>]) {
        let (trace_len, trace_log) = self.validate_columns(columns);
        if trace_len == 0 {
            return;
        }
        let column_count = columns.len();

        if let Some(backend) = backend::current_gpu_backend() {
            if let Some(permit) = backend::try_acquire_gpu_lane() {
                match self.start_ifft_gpu_dispatch(columns, trace_log, backend, permit) {
                    Ok(dispatch) => match dispatch.wait() {
                        Ok(()) => {
                            debug!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                trace_len,
                                columns = column_count,
                                trace_log,
                                backend = ?backend,
                                "ifft columns via gpu"
                            );
                            return;
                        }
                        Err(err) => {
                            warn!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                trace_len,
                                columns = column_count,
                                trace_log,
                                backend = ?backend,
                                error = %err,
                                "gpu ifft wait failed; falling back to cpu"
                            );
                        }
                    },
                    Err(err) => {
                        warn!(
                            target: "fastpq::planner",
                            parameter = self.params.name,
                            trace_len,
                            columns = column_count,
                            trace_log,
                            backend = ?backend,
                            error = %err,
                            "gpu ifft failed; falling back to cpu"
                        );
                    }
                }
            } else if columns.len() >= 2 {
                self.split_ifft_gpu_cpu(columns, trace_len, trace_log, backend);
                return;
            } else {
                debug!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = columns.len(),
                    trace_log,
                    "gpu lane busy; running ifft on cpu"
                );
            }
        } else {
            debug!(
                target: "fastpq::planner",
                parameter = self.params.name,
                trace_len,
                columns = columns.len(),
                trace_log,
                "no gpu backend detected; falling back to cpu ifft"
            );
        }
        self.ifft_columns_cpu(columns, trace_len, trace_log);
    }

    /// Stage a GPU inverse FFT and return a pending dispatch if available.
    pub fn ifft_gpu_pending<'a>(
        &self,
        columns: &'a mut [Vec<u64>],
    ) -> Option<GpuColumnDispatch<'a>> {
        let (trace_len, trace_log) = self.validate_columns(columns);
        if trace_len == 0 {
            return None;
        }
        let backend = backend::current_gpu_backend()?;
        let permit = backend::try_acquire_gpu_lane()?;
        let column_count = columns.len();
        match self.start_ifft_gpu_dispatch(columns, trace_log, backend, permit) {
            Ok(dispatch) => Some(dispatch),
            Err(err) => {
                warn!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = column_count,
                    trace_log,
                    backend = ?backend,
                    error = %err,
                    "gpu ifft failed; pending dispatch unavailable"
                );
                None
            }
        }
    }

    fn split_ifft_gpu_cpu(
        &self,
        columns: &mut [Vec<u64>],
        trace_len: usize,
        trace_log: u32,
        backend: backend::GpuBackend,
    ) {
        let total_columns = columns.len();
        let split = columns.len() / 2;
        if split == 0 {
            self.ifft_columns_cpu(columns, trace_len, trace_log);
            return;
        }
        let (cpu_slice, gpu_slice) = columns.split_at_mut(split);
        info!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            columns = total_columns,
            trace_log,
            cpu_columns = cpu_slice.len(),
            gpu_columns = gpu_slice.len(),
            "gpu lane busy; splitting ifft workload across cpu/gpu"
        );
        let ((), gpu_result) = join(
            || self.ifft_columns_cpu(cpu_slice, trace_len, trace_log),
            || {
                let guard = backend::acquire_gpu_lane();
                let result = gpu::ifft_columns(
                    gpu_slice,
                    trace_log,
                    self.trace_domain(trace_log).generator,
                    backend,
                );
                drop(guard);
                result
            },
        );
        if let Err(err) = gpu_result {
            warn!(
                target: "fastpq::planner",
                parameter = self.params.name,
                trace_len,
                columns = total_columns,
                trace_log,
                backend = ?backend,
                error = %err,
                "gpu ifft split failed; cpu handled remaining columns"
            );
            self.ifft_columns_cpu(gpu_slice, trace_len, trace_log);
        }
    }

    fn split_lde_gpu_cpu(
        &self,
        coeffs: &[Vec<u64>],
        trace_len: usize,
        trace_log: u32,
        lde_log: u32,
        backend: backend::GpuBackend,
    ) -> Vec<Vec<u64>> {
        let split = coeffs.len() / 2;
        if split == 0 {
            return self.lde_columns(coeffs);
        }
        let cpu_columns = split;
        let gpu_columns = coeffs.len().saturating_sub(split);
        info!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            columns = coeffs.len(),
            trace_log,
            lde_log,
            cpu_columns,
            gpu_columns,
            "gpu lane busy; splitting lde workload across cpu/gpu"
        );
        let (cpu_part, gpu_result) = join(
            || self.lde_columns(&coeffs[..split]),
            || {
                let guard = backend::acquire_gpu_lane();
                let result = gpu::lde_columns(
                    &coeffs[split..],
                    trace_log,
                    self.blowup_log,
                    self.lde_domain(lde_log).generator,
                    self.params.omega_coset,
                    backend,
                );
                drop(guard);
                result
            },
        );
        let gpu_part = match gpu_result {
            Ok(Some(columns)) => columns,
            Ok(None) => {
                debug!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = gpu_columns,
                    "gpu backend unavailable during split lde; cpu handled gpu partition"
                );
                self.lde_columns(&coeffs[split..])
            }
            Err(err) => {
                warn!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    trace_len,
                    columns = gpu_columns,
                    backend = ?backend,
                    error = %err,
                    "gpu lde split failed; cpu handled remaining columns"
                );
                self.lde_columns(&coeffs[split..])
            }
        };
        let mut combined = cpu_part;
        combined.extend(gpu_part);
        combined
    }

    fn lde_metadata(&self, coeffs: &[Vec<u64>]) -> (usize, u32, u32, usize) {
        let trace_len = coeffs[0].len();
        assert!(
            trace_len.is_power_of_two(),
            "coefficient columns must use power-of-two length"
        );
        let trace_log = log_from_length(trace_len);
        assert!(
            trace_log <= self.params.trace_log_size,
            "trace length exceeds parameter catalogue ({} > {})",
            trace_log,
            self.params.trace_log_size
        );
        let lde_log = trace_log + self.blowup_log;
        assert!(
            lde_log <= self.params.lde_log_size,
            "lde length exceeds parameter catalogue ({} > {})",
            lde_log,
            self.params.lde_log_size
        );
        let lde_len = self.lde_domain(lde_log).size();
        (trace_len, trace_log, lde_log, lde_len)
    }

    /// Evaluate coefficient columns on the canonical low-degree extension coset.
    ///
    /// Coefficient vectors must already be padded to the trace domain size; the
    /// returned evaluations have length `trace_len * params.fri.blowup_factor`.
    pub fn lde_columns(&self, coeffs: &[Vec<u64>]) -> Vec<Vec<u64>> {
        if coeffs.is_empty() {
            return Vec::new();
        }
        let (trace_len, trace_log, lde_log, lde_len) = self.lde_metadata(coeffs);
        let lde_domain = self.lde_domain(lde_log);

        debug!(
            target: "fastpq::planner",
            parameter = self.params.name,
            trace_len,
            lde_len,
            trace_log,
            lde_log,
            coset = self.params.omega_coset,
            "lde columns"
        );

        let coset_table = coset_powers(self.params.omega_coset, trace_len);
        coeffs
            .par_iter()
            .map(|column| {
                assert_eq!(
                    column.len(),
                    trace_len,
                    "coefficient column length mismatch"
                );
                let mut padded = vec![0u64; lde_len];
                for (idx, coeff) in column.iter().enumerate() {
                    padded[idx] = mul_mod(*coeff, coset_table[idx]);
                }
                cyclotomic::fft(&mut padded, lde_domain);
                padded
            })
            .collect()
    }

    /// Attempt a GPU-accelerated low-degree extension evaluation.
    pub fn lde_gpu(&self, coeffs: &[Vec<u64>]) -> Vec<Vec<u64>> {
        if coeffs.is_empty() {
            return Vec::new();
        }
        let (trace_len, trace_log, lde_log, lde_len) = self.lde_metadata(coeffs);
        if let Some(backend) = backend::current_gpu_backend() {
            if let Some(permit) = backend::try_acquire_gpu_lane() {
                match self.start_lde_gpu_dispatch(coeffs, trace_log, backend, permit) {
                    Ok(dispatch) => match dispatch.wait() {
                        Ok(Some(columns)) => {
                            debug!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                columns = coeffs.len(),
                                trace_len,
                                lde_len,
                                backend = ?backend,
                                "lde columns via gpu"
                            );
                            return columns;
                        }
                        Ok(None) => {
                            debug!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                columns = coeffs.len(),
                                trace_len,
                                "gpu backend unavailable for lde; falling back to cpu"
                            );
                        }
                        Err(err) => {
                            warn!(
                                target: "fastpq::planner",
                                parameter = self.params.name,
                                columns = coeffs.len(),
                                trace_len,
                                backend = ?backend,
                                error = %err,
                                "gpu lde wait failed; falling back to cpu"
                            );
                        }
                    },
                    Err(err) => {
                        warn!(
                            target: "fastpq::planner",
                            parameter = self.params.name,
                            columns = coeffs.len(),
                            trace_len,
                            backend = ?backend,
                            error = %err,
                            "gpu lde failed; falling back to cpu"
                        );
                    }
                }
            } else if coeffs.len() >= 2 {
                return self.split_lde_gpu_cpu(coeffs, trace_len, trace_log, lde_log, backend);
            } else {
                debug!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    columns = coeffs.len(),
                    trace_len,
                    "gpu lane busy; running lde on cpu"
                );
            }
        } else {
            debug!(
                target: "fastpq::planner",
                parameter = self.params.name,
                columns = coeffs.len(),
                trace_len,
                lde_len,
                "no gpu backend detected; falling back to cpu lde"
            );
        }

        self.lde_columns(coeffs)
    }

    /// Stage a GPU LDE dispatch and return a pending handle when available.
    pub fn lde_gpu_pending(&self, coeffs: &[Vec<u64>]) -> Option<GpuLdeDispatch> {
        if coeffs.is_empty() {
            return None;
        }
        let (_, trace_log, _, _) = self.lde_metadata(coeffs);
        let backend = backend::current_gpu_backend()?;
        let permit = backend::try_acquire_gpu_lane()?;
        match self.start_lde_gpu_dispatch(coeffs, trace_log, backend, permit) {
            Ok(dispatch) => Some(dispatch),
            Err(err) => {
                warn!(
                    target: "fastpq::planner",
                    parameter = self.params.name,
                    columns = coeffs.len(),
                    trace_log,
                    backend = ?backend,
                    error = %err,
                    "gpu lde failed; pending dispatch unavailable"
                );
                None
            }
        }
    }

    fn validate_columns(&self, columns: &[Vec<u64>]) -> (usize, u32) {
        if columns.is_empty() {
            return (0, 0);
        }
        let len = columns[0].len();
        assert!(
            len.is_power_of_two(),
            "column length must be a power of two"
        );
        let log_len = log_from_length(len);
        assert!(
            log_len <= self.params.trace_log_size,
            "column length exceeds trace domain ({} > {})",
            log_len,
            self.params.trace_log_size
        );
        for (idx, column) in columns.iter().enumerate() {
            assert_eq!(
                column.len(),
                len,
                "column {idx} length ({}) did not match expected ({len})",
                column.len()
            );
        }
        (len, log_len)
    }

    pub(crate) fn trace_domain(&self, log_size: u32) -> Domain {
        assert!(
            log_size <= self.params.trace_log_size,
            "requested trace domain exceeds catalogue ({} > {})",
            log_size,
            self.params.trace_log_size
        );
        self.trace_domains[usize::try_from(log_size).expect("log size fits usize")]
    }

    pub(crate) fn lde_domain(&self, log_size: u32) -> Domain {
        assert!(
            log_size <= self.params.lde_log_size,
            "requested LDE domain exceeds catalogue ({} > {})",
            log_size,
            self.params.lde_log_size
        );
        self.lde_domains[usize::try_from(log_size).expect("log size fits usize")]
    }

    fn start_fft_gpu_dispatch<'a>(
        &self,
        columns: &'a mut [Vec<u64>],
        trace_log: u32,
        backend: backend::GpuBackend,
        permit: MutexGuard<'static, ()>,
    ) -> Result<GpuColumnDispatch<'a>, gpu::GpuError> {
        let domain = self.trace_domain(trace_log);
        match gpu::fft_columns_async(columns, trace_log, domain.generator, backend) {
            Ok(dispatch) => Ok(GpuColumnDispatch::new(dispatch, permit)),
            Err(err) => {
                drop(permit);
                Err(err)
            }
        }
    }

    fn start_ifft_gpu_dispatch<'a>(
        &self,
        columns: &'a mut [Vec<u64>],
        trace_log: u32,
        backend: backend::GpuBackend,
        permit: MutexGuard<'static, ()>,
    ) -> Result<GpuColumnDispatch<'a>, gpu::GpuError> {
        let domain = self.trace_domain(trace_log);
        match gpu::ifft_columns_async(columns, trace_log, domain.generator, backend) {
            Ok(dispatch) => Ok(GpuColumnDispatch::new(dispatch, permit)),
            Err(err) => {
                drop(permit);
                Err(err)
            }
        }
    }

    fn start_lde_gpu_dispatch(
        &self,
        coeffs: &[Vec<u64>],
        trace_log: u32,
        backend: backend::GpuBackend,
        permit: MutexGuard<'static, ()>,
    ) -> Result<GpuLdeDispatch, gpu::GpuError> {
        let lde_root = self.lde_domain(trace_log + self.blowup_log).generator;
        match gpu::lde_columns_async(
            coeffs,
            trace_log,
            self.blowup_log,
            lde_root,
            self.params.omega_coset,
            backend,
        ) {
            Ok(dispatch) => Ok(GpuLdeDispatch::new(dispatch, permit)),
            Err(err) => {
                drop(permit);
                Err(err)
            }
        }
    }
}

fn log_from_length(len: usize) -> u32 {
    len.trailing_zeros()
}

fn subset_generator(root: u64, source_log: u32, target_log: u32) -> u64 {
    if target_log == 0 {
        return 1;
    }
    assert!(
        target_log <= source_log,
        "requested subgroup is larger than the source domain"
    );
    let stride = 1u64 << (source_log - target_log);
    mod_pow(root, stride)
}

fn coset_powers(coset: u64, len: usize) -> Vec<u64> {
    let mut powers = Vec::with_capacity(len);
    let mut current = 1u64;
    for _ in 0..len {
        powers.push(current);
        current = mul_mod(current, coset);
    }
    powers
}

fn verify_root(root: u64, log_size: u32, label: &str) {
    let order = 1u64 << log_size;
    let pow = mod_pow(root, order);
    assert_eq!(pow, 1, "{label}^2^{log_size} != 1");

    if log_size > 0 {
        let half_order = order >> 1;
        let half_pow = mod_pow(root, half_order);
        assert_ne!(
            half_pow, 1,
            "{label} is not a primitive 2^{log_size} root of unity"
        );
    }
}

fn mod_pow(mut base: u64, mut exp: u64) -> u64 {
    let mut result = 1u64;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base);
        }
        base = mul_mod(base, base);
        exp >>= 1;
    }
    result
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    u64::try_from(product % u128::from(FIELD_MODULUS)).expect("modulus reduction fits in u64")
}

fn build_domain_cache(root: u64, max_log: u32) -> Vec<Domain> {
    let size = usize::try_from(max_log).expect("log size fits usize") + 1;
    let mut cache = Vec::with_capacity(size);
    for log in 0..=max_log {
        cache.push(Domain {
            log_size: log,
            generator: subset_generator(root, max_log, log),
        });
    }
    cache
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;
    #[cfg(feature = "fastpq-gpu")]
    use std::{
        sync::{Arc, Barrier},
        thread,
    };

    use fastpq_isi::CANONICAL_PARAMETER_SETS;
    use proptest::{collection::vec as pvec, prelude::*};

    use super::*;
    use crate::backend;
    #[cfg(feature = "fastpq-gpu")]
    use crate::gpu::{self, GpuError};

    const MAX_TRACE_LOG: u32 = 4;

    fn random_column_set() -> impl Strategy<Value = (u32, Vec<Vec<u64>>)> {
        (0u32..=MAX_TRACE_LOG, 1usize..=3).prop_flat_map(|(trace_log, column_count)| {
            let len = 1usize << trace_log;
            pvec(pvec(0u64..FIELD_MODULUS, len), column_count)
                .prop_map(move |columns| (trace_log, columns))
        })
    }

    #[cfg(feature = "fastpq-gpu")]
    fn try_cuda_fft(columns: &mut [Vec<u64>], trace_log: u32, root: u64) -> Result<(), String> {
        match gpu::fft_columns(columns, trace_log, root, backend::GpuBackend::Cuda) {
            Ok(()) => Ok(()),
            Err(GpuError::Unsupported(_)) => Err("CUDA backend unsupported in this build".into()),
            Err(GpuError::Execution {
                backend: backend::GpuBackend::Cuda,
                message,
            }) if message.contains("unavailable") => Err(message),
            Err(err) => panic!("CUDA FFT failed: {err}"),
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    fn try_cuda_ifft(columns: &mut [Vec<u64>], trace_log: u32, root: u64) -> Result<(), String> {
        match gpu::ifft_columns(columns, trace_log, root, backend::GpuBackend::Cuda) {
            Ok(()) => Ok(()),
            Err(GpuError::Unsupported(_)) => Err("CUDA backend unsupported in this build".into()),
            Err(GpuError::Execution {
                backend: backend::GpuBackend::Cuda,
                message,
            }) if message.contains("unavailable") => Err(message),
            Err(err) => panic!("CUDA IFFT failed: {err}"),
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    fn try_cuda_lde(
        coeffs: &[Vec<u64>],
        trace_log: u32,
        blowup_log: u32,
        lde_root: u64,
        coset: u64,
    ) -> Result<Vec<Vec<u64>>, String> {
        match gpu::lde_columns(
            coeffs,
            trace_log,
            blowup_log,
            lde_root,
            coset,
            backend::GpuBackend::Cuda,
        ) {
            Ok(Some(columns)) => Ok(columns),
            Ok(None) => Err("CUDA backend declined the workload".into()),
            Err(GpuError::Unsupported(_)) => Err("CUDA backend unsupported in this build".into()),
            Err(GpuError::Execution {
                backend: backend::GpuBackend::Cuda,
                message,
            }) if message.contains("unavailable") => Err(message),
            Err(err) => panic!("CUDA LDE failed: {err}"),
        }
    }

    #[test]
    fn gpu_column_dispatch_waits_ready_variant() {
        let dispatch = gpu::ColumnDispatch::ready();
        let guard = GpuColumnDispatch::from_ready(dispatch);
        assert!(guard.wait().is_ok());
    }

    #[test]
    fn gpu_lde_dispatch_waits_ready_variant() {
        let dispatch = gpu::LdeDispatch::ready(Some(vec![vec![5u64, 9u64]]));
        let guard = GpuLdeDispatch::from_ready(dispatch);
        let result = guard.wait().expect("wait succeeds").expect("result");
        assert_eq!(result[0], vec![5u64, 9u64]);
    }

    #[test]
    fn planner_domains_match_catalogue() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);

        let full_trace = planner.trace_domain(params.trace_log_size);
        assert_eq!(full_trace.generator, params.trace_root);

        let trivial_trace = planner.trace_domain(0);
        assert_eq!(trivial_trace.generator, 1);
        assert_eq!(trivial_trace.log_size, 0);

        let reduced_trace = planner.trace_domain(params.trace_log_size - 3);
        let expected = super::mod_pow(params.trace_root, 1 << 3);
        assert_eq!(reduced_trace.generator, expected);

        let lde_domain = planner.lde_domain(params.lde_log_size);
        assert_eq!(lde_domain.generator, params.lde_root);
        let trivial_lde = planner.lde_domain(0);
        assert_eq!(trivial_lde.generator, 1);
        assert_eq!(trivial_lde.log_size, 0);
    }

    #[test]
    fn planner_rejects_non_primitive_trace_root() {
        let mut params = CANONICAL_PARAMETER_SETS[0];
        params.trace_root = 1;
        let result = catch_unwind(|| Planner::new(&params));
        assert!(
            result.is_err(),
            "planner accepted trace root that is not a primitive 2-adic generator"
        );
    }

    #[test]
    fn planner_rejects_non_primitive_lde_root() {
        let mut params = CANONICAL_PARAMETER_SETS[0];
        params.lde_root = 1;
        let result = catch_unwind(|| Planner::new(&params));
        assert!(
            result.is_err(),
            "planner accepted LDE root that is not a primitive 2-adic generator"
        );
    }

    #[test]
    fn fft_round_trip_matches_input() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = 1;
        let trace_len = 1usize << trace_log;
        let values: Vec<u64> = (0..trace_len)
            .map(|idx| (idx as u64).wrapping_mul(17).wrapping_add(3) % FIELD_MODULUS)
            .collect();
        let mut columns = vec![values.clone()];

        let domain = planner.trace_domain(trace_log);
        let mut direct = values.clone();
        cyclotomic::fft(&mut direct, domain);
        cyclotomic::ifft(&mut direct, domain);
        assert_eq!(direct, values);

        planner.fft_columns(&mut columns);
        planner.ifft_columns(&mut columns);
        assert_eq!(columns[0], values);
    }

    #[test]
    fn gpu_fft_matches_cpu_output() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size - 2;
        let trace_len = 1usize << trace_log;
        let mut cpu_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(11).wrapping_add(5) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(19).wrapping_add(3) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut gpu_columns = cpu_columns.clone();

        planner.fft_columns(&mut cpu_columns);
        #[cfg(feature = "fastpq-gpu")]
        {
            match try_cuda_fft(
                &mut gpu_columns,
                trace_log,
                planner.trace_domain(trace_log).generator,
            ) {
                Ok(()) => {}
                Err(reason) => {
                    eprintln!("skipping CUDA FFT comparison: {reason}");
                    return;
                }
            }
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        planner.fft_columns(&mut gpu_columns);

        assert_eq!(cpu_columns, gpu_columns);
    }

    #[test]
    fn gpu_fft_matches_cpu_output_for_latency_parameters() {
        let params = CANONICAL_PARAMETER_SETS[1];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size - 3;
        let trace_len = 1usize << trace_log;
        let mut cpu_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(37).wrapping_add(11) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut gpu_columns = cpu_columns.clone();

        planner.fft_columns(&mut cpu_columns);
        #[cfg(feature = "fastpq-gpu")]
        {
            match try_cuda_fft(
                &mut gpu_columns,
                trace_log,
                planner.trace_domain(trace_log).generator,
            ) {
                Ok(()) => {}
                Err(reason) => {
                    eprintln!("skipping CUDA FFT latency-parameter comparison: {reason}");
                    return;
                }
            }
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        planner.fft_columns(&mut gpu_columns);

        assert_eq!(cpu_columns, gpu_columns);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn concurrent_cuda_ffts_match_cpu_output_across_parameter_sets() {
        let params_balanced = CANONICAL_PARAMETER_SETS[0];
        let params_latency = CANONICAL_PARAMETER_SETS[1];
        let planner_balanced = Planner::new(&params_balanced);
        let planner_latency = Planner::new(&params_latency);

        let trace_log_balanced = params_balanced.trace_log_size - 2;
        let trace_len_balanced = 1usize << trace_log_balanced;
        let balanced_input = vec![
            (0..trace_len_balanced)
                .map(|idx| (idx as u64).wrapping_mul(11).wrapping_add(5) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len_balanced)
                .map(|idx| (idx as u64).wrapping_mul(19).wrapping_add(3) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut cpu_balanced = balanced_input.clone();
        planner_balanced.fft_columns(&mut cpu_balanced);
        let gpu_balanced = balanced_input;

        let trace_log_latency = params_latency.trace_log_size - 3;
        let trace_len_latency = 1usize << trace_log_latency;
        let mut cpu_latency = vec![
            (0..trace_len_latency)
                .map(|idx| (idx as u64).wrapping_mul(37).wrapping_add(11) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        planner_latency.fft_columns(&mut cpu_latency);
        let gpu_latency = vec![
            (0..trace_len_latency)
                .map(|idx| (idx as u64).wrapping_mul(37).wrapping_add(11) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];

        let barrier = Arc::new(Barrier::new(2));
        let balanced_root = planner_balanced.trace_domain(trace_log_balanced).generator;
        let latency_root = planner_latency.trace_domain(trace_log_latency).generator;
        let (balanced_gpu, latency_gpu) = thread::scope(|scope| {
            let balanced_barrier = Arc::clone(&barrier);
            let balanced = scope.spawn(move || {
                let mut columns = gpu_balanced;
                balanced_barrier.wait();
                try_cuda_fft(&mut columns, trace_log_balanced, balanced_root).map(|()| columns)
            });
            let latency_barrier = Arc::clone(&barrier);
            let latency = scope.spawn(move || {
                let mut columns = gpu_latency;
                latency_barrier.wait();
                try_cuda_fft(&mut columns, trace_log_latency, latency_root).map(|()| columns)
            });
            (
                balanced
                    .join()
                    .expect("balanced CUDA FFT thread should not panic"),
                latency
                    .join()
                    .expect("latency CUDA FFT thread should not panic"),
            )
        });

        let balanced_gpu = match balanced_gpu {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping concurrent CUDA FFT comparison: {reason}");
                return;
            }
        };
        let latency_gpu = match latency_gpu {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping concurrent CUDA FFT comparison: {reason}");
                return;
            }
        };

        assert_eq!(cpu_balanced, balanced_gpu);
        assert_eq!(cpu_latency, latency_gpu);
    }

    #[test]
    fn gpu_fft_split_matches_cpu_output_without_gpu() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size.saturating_sub(1);
        let trace_len = 1usize << trace_log;
        let mut baseline = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(5).wrapping_add(1) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(7).wrapping_add(3) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(11).wrapping_add(9) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(13).wrapping_add(17) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut split = baseline.clone();

        planner.fft_columns(&mut baseline);
        planner.split_fft_gpu_cpu(&mut split, trace_len, trace_log, backend::GpuBackend::Cuda);

        assert_eq!(baseline, split);
    }

    #[test]
    fn gpu_ifft_matches_cpu_output() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size - 2;
        let trace_len = 1usize << trace_log;
        let mut cpu_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(13).wrapping_add(17) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(29).wrapping_add(31) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        planner.fft_columns(&mut cpu_columns);
        let mut gpu_columns = cpu_columns.clone();

        planner.ifft_columns(&mut cpu_columns);
        #[cfg(feature = "fastpq-gpu")]
        {
            match try_cuda_ifft(
                &mut gpu_columns,
                trace_log,
                planner.trace_domain(trace_log).generator,
            ) {
                Ok(()) => {}
                Err(reason) => {
                    eprintln!("skipping CUDA IFFT comparison: {reason}");
                    return;
                }
            }
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        planner.ifft_columns(&mut gpu_columns);

        assert_eq!(cpu_columns, gpu_columns);
    }

    #[test]
    fn gpu_ifft_split_matches_cpu_output_without_gpu() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size.saturating_sub(1);
        let trace_len = 1usize << trace_log;
        let mut spectral = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(17).wrapping_add(23) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(19).wrapping_add(29) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(31).wrapping_add(37) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(41).wrapping_add(43) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        planner.fft_columns(&mut spectral);
        let mut baseline = spectral.clone();
        planner.ifft_columns(&mut baseline);
        let mut split = spectral;

        planner.split_ifft_gpu_cpu(&mut split, trace_len, trace_log, backend::GpuBackend::Cuda);

        assert_eq!(baseline, split);
    }

    #[test]
    fn lde_coset_matches_manual_linear_polynomial() {
        let params = CANONICAL_PARAMETER_SETS[1];
        let planner = Planner::new(&params);
        let trace_len = 1usize << params.trace_log_size;
        let mut coeff_column = vec![0u64; trace_len];
        coeff_column[1] = 1; // polynomial f(x) = x

        let evaluations = planner.lde_columns(&[coeff_column]).pop().unwrap();
        let lde_domain = planner.lde_domain(params.trace_log_size + planner.blowup_log());
        let mut expected = Vec::with_capacity(evaluations.len());
        let mut point = params.omega_coset;
        for _ in 0..evaluations.len() {
            expected.push(point);
            point = mul_mod(point, lde_domain.generator);
        }

        assert_eq!(evaluations, expected);
    }

    #[test]
    fn gpu_lde_matches_cpu_output() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size - 1;
        let trace_len = 1usize << trace_log;
        let value_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(7).wrapping_add(13) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(23).wrapping_add(29) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut coeff_columns = value_columns.clone();
        planner.ifft_columns(&mut coeff_columns);

        let cpu = planner.lde_columns(&coeff_columns);
        #[cfg(feature = "fastpq-gpu")]
        let gpu = match try_cuda_lde(
            &coeff_columns,
            trace_log,
            planner.blowup_log(),
            planner
                .lde_domain(trace_log + planner.blowup_log())
                .generator,
            params.omega_coset,
        ) {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping CUDA LDE comparison: {reason}");
                return;
            }
        };
        #[cfg(not(feature = "fastpq-gpu"))]
        let gpu = planner.lde_columns(&coeff_columns);

        assert_eq!(cpu, gpu);
    }

    #[test]
    fn gpu_lde_matches_cpu_output_for_latency_parameters() {
        let params = CANONICAL_PARAMETER_SETS[1];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size - 3;
        let trace_len = 1usize << trace_log;
        let value_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(41).wrapping_add(7) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut coeff_columns = value_columns.clone();
        planner.ifft_columns(&mut coeff_columns);

        let cpu = planner.lde_columns(&coeff_columns);
        #[cfg(feature = "fastpq-gpu")]
        let gpu = match try_cuda_lde(
            &coeff_columns,
            trace_log,
            planner.blowup_log(),
            planner
                .lde_domain(trace_log + planner.blowup_log())
                .generator,
            params.omega_coset,
        ) {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping CUDA LDE latency-parameter comparison: {reason}");
                return;
            }
        };
        #[cfg(not(feature = "fastpq-gpu"))]
        let gpu = planner.lde_columns(&coeff_columns);

        assert_eq!(cpu, gpu);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn concurrent_cuda_ldes_match_cpu_output_across_parameter_sets() {
        let params_balanced = CANONICAL_PARAMETER_SETS[0];
        let params_latency = CANONICAL_PARAMETER_SETS[1];
        let planner_balanced = Planner::new(&params_balanced);
        let planner_latency = Planner::new(&params_latency);

        let trace_log_balanced = params_balanced.trace_log_size - 1;
        let trace_len_balanced = 1usize << trace_log_balanced;
        let value_columns_balanced = vec![
            (0..trace_len_balanced)
                .map(|idx| (idx as u64).wrapping_mul(7).wrapping_add(13) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len_balanced)
                .map(|idx| (idx as u64).wrapping_mul(23).wrapping_add(29) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut coeff_columns_balanced = value_columns_balanced.clone();
        planner_balanced.ifft_columns(&mut coeff_columns_balanced);
        let cpu_balanced = planner_balanced.lde_columns(&coeff_columns_balanced);

        let trace_log_latency = params_latency.trace_log_size - 3;
        let trace_len_latency = 1usize << trace_log_latency;
        let value_columns_latency = vec![
            (0..trace_len_latency)
                .map(|idx| (idx as u64).wrapping_mul(41).wrapping_add(7) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        let mut coeff_columns_latency = value_columns_latency.clone();
        planner_latency.ifft_columns(&mut coeff_columns_latency);
        let cpu_latency = planner_latency.lde_columns(&coeff_columns_latency);

        let barrier = Arc::new(Barrier::new(2));
        let balanced_lde_root = planner_balanced
            .lde_domain(trace_log_balanced + planner_balanced.blowup_log())
            .generator;
        let latency_lde_root = planner_latency
            .lde_domain(trace_log_latency + planner_latency.blowup_log())
            .generator;
        let (balanced_gpu, latency_gpu) = thread::scope(|scope| {
            let balanced_barrier = Arc::clone(&barrier);
            let balanced = scope.spawn(move || {
                balanced_barrier.wait();
                try_cuda_lde(
                    &coeff_columns_balanced,
                    trace_log_balanced,
                    planner_balanced.blowup_log(),
                    balanced_lde_root,
                    params_balanced.omega_coset,
                )
            });
            let latency_barrier = Arc::clone(&barrier);
            let latency = scope.spawn(move || {
                latency_barrier.wait();
                try_cuda_lde(
                    &coeff_columns_latency,
                    trace_log_latency,
                    planner_latency.blowup_log(),
                    latency_lde_root,
                    params_latency.omega_coset,
                )
            });
            (
                balanced
                    .join()
                    .expect("balanced CUDA LDE thread should not panic"),
                latency
                    .join()
                    .expect("latency CUDA LDE thread should not panic"),
            )
        });

        let balanced_gpu = match balanced_gpu {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping concurrent CUDA LDE comparison: {reason}");
                return;
            }
        };
        let latency_gpu = match latency_gpu {
            Ok(columns) => columns,
            Err(reason) => {
                eprintln!("skipping concurrent CUDA LDE comparison: {reason}");
                return;
            }
        };

        assert_eq!(cpu_balanced, balanced_gpu);
        assert_eq!(cpu_latency, latency_gpu);
    }

    #[test]
    fn gpu_lde_split_matches_cpu_output_without_gpu() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = params.trace_log_size.saturating_sub(1);
        let trace_len = 1usize << trace_log;
        let mut coeff_columns = vec![
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(47).wrapping_add(59) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(53).wrapping_add(61) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(67).wrapping_add(71) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| (idx as u64).wrapping_mul(73).wrapping_add(79) % FIELD_MODULUS)
                .collect::<Vec<u64>>(),
        ];
        planner.ifft_columns(&mut coeff_columns);

        let cpu = planner.lde_columns(&coeff_columns);
        let split = planner.split_lde_gpu_cpu(
            &coeff_columns,
            trace_len,
            trace_log,
            trace_log + planner.blowup_log(),
            backend::GpuBackend::Cuda,
        );

        assert_eq!(cpu, split);
    }

    #[test]
    fn lde_is_deterministic() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_len = 1usize << params.trace_log_size;
        let coeff_column: Vec<u64> = (0..trace_len)
            .map(|idx| (idx as u64).wrapping_mul(31).wrapping_add(7))
            .collect();

        let first = planner.lde_columns(std::slice::from_ref(&coeff_column));
        let second = planner.lde_columns(std::slice::from_ref(&coeff_column));
        assert_eq!(first, second);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn fft_roundtrip_over_random_columns_is_deterministic((_trace_log, columns) in random_column_set()) {
            let params = CANONICAL_PARAMETER_SETS[0];
            let planner = Planner::new(&params);

            let original = columns.clone();

            let mut fft_first = original.clone();
            planner.fft_columns(&mut fft_first);

            let mut fft_second = original.clone();
            planner.fft_columns(&mut fft_second);
            prop_assert_eq!(&fft_first, &fft_second);

            let mut fft_roundtrip = fft_second.clone();
            planner.ifft_columns(&mut fft_roundtrip);
            prop_assert_eq!(&fft_roundtrip, &original);

            let mut fft_gpu = original.clone();
            planner.fft_gpu(&mut fft_gpu);
            prop_assert_eq!(&fft_gpu, &fft_second);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn lde_columns_over_random_polynomials_are_deterministic((trace_log, columns) in random_column_set()) {
            let params = CANONICAL_PARAMETER_SETS[0];
            let planner = Planner::new(&params);
            let evaluations_a = planner.lde_columns(&columns);
            let evaluations_b = planner.lde_columns(&columns);
            prop_assert_eq!(&evaluations_a, &evaluations_b);

            let trace_len = 1usize << trace_log;
            let expected_len = trace_len << planner.blowup_log();
            prop_assert!(evaluations_a.iter().all(|column| column.len() == expected_len));
        }
    }
}
