//! Bounded exact-root FFT with independent tiles and globally ordered stages.
//!
//! Caller columns are published only after the entire command completes. The
//! sole private copy uses the existing clearing, Arc-backed shared allocation;
//! the existing dispatch ticket drains or quarantines it on error and unwind.

use super::*;
use rayon::prelude::*;

const LANES: u64 = 256;
const LOCAL_LOG: u32 = 8;
// A fused group owns 1024 four-word cells; an unpaired tail owns 2048 pairs.
// Both mappings cover exactly 4096 words with the same dispatch geometry.
const WORDS_PER_GLOBAL_GROUP: u64 = 4096;
pub(super) const BIT_REVERSE_KERNEL: &str = "exact_root_bit_reverse_v1";
pub(super) const LOCAL_TILES_KERNEL: &str = "exact_root_local_tiles_v1";
pub(super) const GLOBAL_STAGE_KERNEL: &str = "exact_root_global_stage_v1";

#[derive(Clone, Copy)]
#[repr(C)]
struct ExactRootFftArgs {
    column_len: u64,
    normalization: u64,
    log_len: u32,
    column_count: u32,
    stage: u32,
    padding: u32,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
struct PhaseDiagnostic {
    context_twiddles: Duration,
    flatten: Duration,
    mapping: Duration,
    submit: Duration,
    wait_and_cleanup: Duration,
    ticket_wait: Duration,
    copy_back: Duration,
    backing_drop: Duration,
    total: Duration,
    gpu_seconds: f64,
    kernel_seconds: f64,
}

#[cfg(test)]
thread_local! {
    static PHASE_DIAGNOSTIC: std::cell::RefCell<Option<PhaseDiagnostic>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
struct PhaseDiagnosticScope;

#[cfg(test)]
impl PhaseDiagnosticScope {
    fn begin() -> Self {
        PHASE_DIAGNOSTIC.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(PhaseDiagnostic::default());
        });
        Self
    }

    fn finish(self) -> PhaseDiagnostic {
        PHASE_DIAGNOSTIC.with(|slot| slot.borrow_mut().take().unwrap())
    }
}

#[cfg(test)]
impl Drop for PhaseDiagnosticScope {
    fn drop(&mut self) {
        PHASE_DIAGNOSTIC.with(|slot| *slot.borrow_mut() = None);
    }
}

#[cfg(test)]
pub(super) fn diagnostic_start() -> Option<Instant> {
    PHASE_DIAGNOSTIC.with(|slot| slot.borrow().as_ref().map(|_| Instant::now()))
}

#[cfg(test)]
fn diagnostic_record(
    start: Option<Instant>,
    field: impl FnOnce(&mut PhaseDiagnostic) -> &mut Duration,
) {
    if let Some(start) = start {
        let elapsed = start.elapsed();
        PHASE_DIAGNOSTIC.with(|slot| {
            if let Some(record) = slot.borrow_mut().as_mut() {
                *field(record) += elapsed;
            }
        });
    }
}

#[cfg(test)]
pub(super) fn diagnostic_ticket_wait(start: Option<Instant>) {
    diagnostic_record(start, |record| &mut record.ticket_wait);
}

#[cfg(test)]
pub(super) fn diagnostic_copy_back(start: Option<Instant>) {
    diagnostic_record(start, |record| &mut record.copy_back);
}

#[cfg(test)]
pub(super) fn diagnostic_backing_drop(start: Option<Instant>) {
    diagnostic_record(start, |record| &mut record.backing_drop);
}

// These read-only properties are valid after successful completion. Do not
// clone/retain the command: its existing owner still releases it at the same
// point in wait_for_ticket, including the original no-copy backing lifetimes.
#[cfg(test)]
#[allow(unsafe_code)]
pub(super) fn diagnostic_completed_command(command: &CommandBufferRef) {
    PHASE_DIAGNOSTIC.with(|slot| {
        if let Some(record) = slot.borrow_mut().as_mut() {
            // SAFETY: macOS Metal exposes these CFTimeInterval properties on a
            // successfully completed command buffer; no mutation occurs.
            let (gpu_start, gpu_end, kernel_start, kernel_end): (f64, f64, f64, f64) = unsafe {
                (
                    msg_send![command, GPUStartTime],
                    msg_send![command, GPUEndTime],
                    msg_send![command, kernelStartTime],
                    msg_send![command, kernelEndTime],
                )
            };
            record.gpu_seconds += (gpu_end - gpu_start).max(0.0);
            record.kernel_seconds += (kernel_end - kernel_start).max(0.0);
        }
    });
}

/// The public exact-root facade validates canonicality and root order first.
#[cfg(test)]
pub fn transform(
    columns: &mut [Vec<u64>],
    log_size: u32,
    root: u64,
    inverse: bool,
) -> MetalResult<()> {
    #[cfg(test)]
    let diagnostic_total = diagnostic_start();
    if columns.len() > crate::goldilocks_transform::MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1 {
        return Err(GpuError::InvalidInput(
            "invalid bounded exact-root FFT shape",
        ));
    }
    let count = columns.len();
    let prepared = {
        let views: [&[u64]; 8] = core::array::from_fn(|index| -> &[u64] {
            columns.get(index).map_or(&[], Vec::as_slice)
        });
        prepare(&views[..count], log_size, root, inverse)?
    };
    let mut destinations: [&mut [u64]; 8] = core::array::from_fn(|_| -> &mut [u64] { &mut [] });
    for (destination, column) in destinations.iter_mut().zip(columns) {
        *destination = column;
    }
    #[cfg(test)]
    let diagnostic_publish = diagnostic_start();
    let result = prepared
        .publish_into(&mut destinations[..count])
        .map_err(|error| {
            if error == crate::goldilocks_transform::GoldilocksTransformErrorV1::CompletionUncertain
            {
                GpuError::CompletionUncertain {
                    backend: GpuBackend::Metal,
                }
            } else {
                GpuError::Execution {
                    backend: GpuBackend::Metal,
                    message: error.to_string(),
                }
            }
        });
    #[cfg(test)]
    diagnostic_record(diagnostic_publish, |record| &mut record.wait_and_cleanup);
    #[cfg(test)]
    diagnostic_record(diagnostic_total, |record| &mut record.total);
    result
}

/// Complete the original single shared backing without publishing caller data.
pub(crate) fn prepare(
    columns: &[&[u64]],
    log_size: u32,
    root: u64,
    inverse: bool,
) -> MetalResult<PreparedColumns> {
    prepare_with_limit(
        columns,
        log_size,
        root,
        inverse,
        crate::goldilocks_transform::MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1,
    )
}

/// The separate forward facade has admitted the complete sixteen-column backing.
pub(crate) fn prepare_forward(
    columns: &[&[u64]],
    log_size: u32,
    root: u64,
) -> MetalResult<PreparedColumns> {
    prepare_with_limit(
        columns,
        log_size,
        root,
        false,
        crate::goldilocks_transform::MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1,
    )
}

fn prepare_with_limit(
    columns: &[&[u64]],
    log_size: u32,
    root: u64,
    inverse: bool,
    column_limit: usize,
) -> MetalResult<PreparedColumns> {
    let _drain_scope = DrainScope::enter(METAL_COMMAND_TIMEOUT);
    ensure_backend_available()?;
    let extent = goldilocks_domain_len(log_size)?;
    if log_size == 0
        || columns.is_empty()
        || columns.len() > column_limit
        || columns.iter().any(|column| column.len() != extent)
    {
        return Err(GpuError::InvalidInput(
            "invalid bounded exact-root FFT shape",
        ));
    }
    prepare_staged_v1(columns.len(), log_size, root, inverse, || {
        flatten_with_stats(columns, ColumnStagingPhase::Fft)
    })
}

fn prepare_staged_v1(
    columns: usize,
    log_size: u32,
    root: u64,
    inverse: bool,
    stage: impl FnOnce() -> MetalResult<PooledBuffer>,
) -> MetalResult<PreparedColumns> {
    let extent = goldilocks_domain_len(log_size)?;
    let column_count = u32::try_from(columns)
        .map_err(|_| GpuError::InvalidInput("exact-root FFT column count exceeds u32"))?;
    let words = extent.checked_mul(columns).ok_or(GpuError::InvalidInput(
        "exact-root FFT staging extent exceeds platform limits",
    ))?;
    #[cfg(test)]
    let diagnostic_context = diagnostic_start();
    let context = metal_context()?;
    for pipeline in [
        &context.exact_root_bit_reverse,
        &context.exact_root_local_tiles,
        &context.exact_root_global_stage,
    ] {
        if pipeline.max_total_threads_per_threadgroup() < LANES {
            return Err(GpuError::Unsupported(GpuBackend::Metal));
        }
    }
    validate_metal_pooled_word_len(&context.device, words)?;
    let twiddles = context.factorized_root_twiddle_buffer(log_size, root, inverse)?;
    #[cfg(test)]
    diagnostic_record(diagnostic_context, |record| &mut record.context_twiddles);
    let args = ExactRootFftArgs {
        column_len: extent as u64,
        normalization: if inverse {
            goldilocks_inv(extent as u64)
        } else {
            1
        },
        log_len: log_size,
        column_count,
        stage: 0,
        padding: 0,
    };
    #[cfg(test)]
    let diagnostic_flatten = diagnostic_start();
    let mut buffer = stage()?;
    #[cfg(test)]
    diagnostic_record(diagnostic_flatten, |record| &mut record.flatten);
    #[cfg(test)]
    let diagnostic_mapping = diagnostic_start();
    let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer)?;
    #[cfg(test)]
    diagnostic_record(diagnostic_mapping, |record| &mut record.mapping);
    #[cfg(test)]
    let diagnostic_submit = diagnostic_start();
    let ticket = submit(context, &metal_buffer, &twiddles, args)?;
    #[cfg(test)]
    diagnostic_record(diagnostic_submit, |record| &mut record.submit);
    // Reuse the original wait owner and its failure injection. A completed
    // token keeps the same backing and Metal view until checked publication.
    #[cfg(test)]
    let diagnostic_wait = diagnostic_start();
    let completed = ColumnBatchTicket {
        range: 0..columns,
        buffer,
        metal_buffer,
        tickets: smallvec::smallvec![ticket],
    }
    .complete(true)?;
    #[cfg(test)]
    diagnostic_record(diagnostic_wait, |record| &mut record.wait_and_cleanup);
    let CompletedColumnBatch {
        range,
        buffer,
        _metal_buffer,
    } = completed;
    debug_assert_eq!(range, 0..columns);
    Ok(PreparedColumns {
        output: PreparedOutput {
            buffer,
            rows: extent,
            columns,
        },
        _metal_buffer,
    })
}

/// Stage canonical masked prefixes into the same bounded exact-root owner.
/// The public facade validates every coefficient, power and exact root first.
pub(crate) fn prepare_prefixes_v1(
    prefixes: &[&[u64]],
    log_size: u32,
    root: u64,
    powers: &[u64],
) -> MetalResult<PreparedColumns> {
    let _drain_scope = DrainScope::enter(METAL_COMMAND_TIMEOUT);
    ensure_backend_available()?;
    let rows = goldilocks_domain_len(log_size)?;
    if log_size == 0
        || prefixes.is_empty()
        || prefixes.len() > crate::goldilocks_transform::MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
        || prefixes
            .iter()
            .any(|prefix| prefix.is_empty() || prefix.len() > rows || prefix.len() > powers.len())
    {
        return Err(GpuError::InvalidInput("invalid bounded prefix FFT shape"));
    }
    prepare_staged_v1(prefixes.len(), log_size, root, false, || {
        stage_validated_prefixes_v1(prefixes, rows, powers)
    })
}

fn stage_validated_prefixes_v1(
    prefixes: &[&[u64]],
    rows: usize,
    powers: &[u64],
) -> MetalResult<PooledBuffer> {
    let words = rows
        .checked_mul(prefixes.len())
        .ok_or(GpuError::InvalidInput("prefix extent overflow"))?;
    let start = Instant::now();
    let mut buffer = PooledBuffer::sensitive_zeroed(words)?;
    let backing = Arc::get_mut(&mut buffer.backing).expect("new prefix backing is unique");
    if rows >= METAL_BUFFER_PAGE_WORDS {
        // rows is a power of two, so independent full columns have disjoint pages.
        backing
            .pages
            .par_chunks_mut(rows / METAL_BUFFER_PAGE_WORDS)
            .zip(prefixes.par_iter())
            .for_each(|(pages, prefix)| {
                for (index, page) in pages
                    .iter_mut()
                    .take(prefix.len().div_ceil(METAL_BUFFER_PAGE_WORDS))
                    .enumerate()
                {
                    let first = index * METAL_BUFFER_PAGE_WORDS;
                    let count = (prefix.len() - first).min(METAL_BUFFER_PAGE_WORDS);
                    for ((destination, coefficient), power) in page.words[..count]
                        .iter_mut()
                        .zip(&prefix[first..first + count])
                        .zip(&powers[first..first + count])
                    {
                        *destination = prefix_product_v1(*coefficient, *power);
                    }
                }
            });
    } else {
        // Tiny test domains may share pages; each word still has exactly one writer.
        for (column, prefix) in prefixes.iter().enumerate() {
            for (row, (&coefficient, &power)) in prefix.iter().zip(powers).enumerate() {
                let index = column * rows + row;
                backing.pages[index / METAL_BUFFER_PAGE_WORDS].words
                    [index % METAL_BUFFER_PAGE_WORDS] = prefix_product_v1(coefficient, power);
            }
        }
    }
    record_staging_flatten(ColumnStagingPhase::Fft, start.elapsed());
    Ok(buffer)
}

/// Same unsigned pseudo-Mersenne fold as the privacy field arithmetic.
#[inline]
fn prefix_product_v1(left: u64, right: u64) -> u64 {
    const EPSILON: u64 = 0xffff_ffff;
    let product = u128::from(left) * u128::from(right);
    let low = product as u64;
    let high = (product >> 64) as u64;
    let (low, borrowed) = low.overflowing_sub(high >> 32);
    let low = if borrowed {
        low.wrapping_sub(EPSILON)
    } else {
        low
    };
    let (value, carried) = low.overflowing_add((high & EPSILON) * EPSILON);
    let value = if carried {
        value.wrapping_add(EPSILON)
    } else {
        value
    };
    if value >= FIELD_MODULUS {
        value - FIELD_MODULUS
    } else {
        value
    }
}

/// A completed device allocation. Only the original successful wait constructs it.
pub(crate) struct PreparedColumns {
    output: PreparedOutput,
    _metal_buffer: Buffer,
}

/// Validated completed backing, with no device view or GPU-lane guard.
/// Only immutable slices escape, and its final backing owner erases all pages.
pub(crate) struct CompletedReadColumns {
    output: PreparedOutput,
}

impl CompletedReadColumns {
    pub(crate) fn erase_completed_v1(self) {
        self.output.buffer.erase_completed_v1();
    }

    pub(crate) fn rows(&self) -> usize {
        self.output.rows
    }
    pub(crate) fn columns(&self) -> usize {
        self.output.columns
    }
    pub(crate) fn backing_payload_bytes(
        &self,
    ) -> Result<usize, crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        self.output.backing_payload_bytes()
    }
    #[allow(unsafe_code)]
    pub(crate) fn column(&self, index: usize) -> Option<&[u64]> {
        if index >= self.output.columns {
            return None;
        }
        const {
            assert!(
                mem::size_of::<MetalBufferPage>()
                    == METAL_BUFFER_PAGE_WORDS * mem::size_of::<u64>()
            );
        }
        // SAFETY: validate_output established exact logical/page extents and
        // canonicality before constructing this owner. repr(C) pages contain
        // only an exactly page-sized u64 array, with no inter-page padding.
        // The index bounds put the entire slice inside initialized allocation.
        // Authoritative GPU completion precedes this owner; no mutable access,
        // dispatch or view survives it. Shared borrows keep backing alive.
        Some(unsafe {
            core::slice::from_raw_parts(
                self.output
                    .buffer
                    .backing
                    .pages
                    .as_ptr()
                    .cast::<u64>()
                    .add(index * self.output.rows),
                self.output.rows,
            )
        })
    }
}

/// The shared publication contract can also be exercised without GPU execution.
/// Tests construct this private owner from the real clearing backing pool.
struct PreparedOutput {
    buffer: PooledBuffer,
    rows: usize,
    columns: usize,
}

/// Scan exactly the validated logical extent, excluding the final page padding.
fn completed_output_is_canonical_v1(pages: &[MetalBufferPage], words: usize) -> bool {
    use crate::goldilocks_transform::{
        CANONICAL_SCAN_TILE_WORDS_V1, canonical_word_slice_v1, parallel_canonical_scan_v1,
    };
    use rayon::prelude::*;

    let full = words / METAL_BUFFER_PAGE_WORDS;
    let tail = words % METAL_BUFFER_PAGE_WORDS;
    const TILE_PAGES: usize = CANONICAL_SCAN_TILE_WORDS_V1 / METAL_BUFFER_PAGE_WORDS;
    const { assert!(TILE_PAGES > 0) };
    let complete = if parallel_canonical_scan_v1(words) {
        pages[..full]
            .par_chunks(TILE_PAGES)
            .all(|tile| tile.iter().all(|page| canonical_word_slice_v1(&page.words)))
    } else {
        pages[..full]
            .iter()
            .all(|page| canonical_word_slice_v1(&page.words))
    };
    complete && (tail == 0 || canonical_word_slice_v1(&pages[full].words[..tail]))
}

impl PreparedColumns {
    pub(crate) fn into_readonly(
        self,
    ) -> Result<CompletedReadColumns, crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        let Self {
            output,
            _metal_buffer,
        } = self;
        let completed = output.into_readonly()?;
        // No pending command exists. Drop the view before publishing host-only
        // custody; its callback can release its separate backing retention.
        drop(_metal_buffer);
        Ok(completed)
    }

    pub(crate) fn rows(&self) -> usize {
        self.output.rows
    }
    pub(crate) fn columns(&self) -> usize {
        self.output.columns
    }

    pub(crate) fn backing_payload_bytes(
        &self,
    ) -> Result<usize, crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        self.output.backing_payload_bytes()
    }

    pub(crate) fn publish_into(
        self,
        destinations: &mut [&mut [u64]],
    ) -> Result<(), crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        let Self {
            output,
            _metal_buffer,
        } = self;
        output.publish_into(destinations)?;
        // Device callbacks cannot invoke worker cleanup: release this view,
        // then attempt exclusive completed backing custody on this caller.
        drop(_metal_buffer);
        output.buffer.erase_completed_v1();
        Ok(())
    }
}

impl PreparedOutput {
    fn into_readonly(
        self,
    ) -> Result<CompletedReadColumns, crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        self.validate_output()?;
        Ok(CompletedReadColumns { output: self })
    }

    fn backing_payload_bytes(
        &self,
    ) -> Result<usize, crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        const {
            assert!(mem::size_of::<PooledBufferBacking>() + 2 * mem::size_of::<usize>() <= 256);
        }
        self.buffer
            .backing
            .pages
            .capacity()
            .checked_mul(METAL_BUFFER_PAGE_BYTES)
            .and_then(|bytes| bytes.checked_add(mem::size_of::<PooledBufferBacking>()))
            // Arc's allocation owns both atomic reference counts before its data.
            .and_then(|bytes| bytes.checked_add(2 * mem::size_of::<usize>()))
            .ok_or(crate::goldilocks_transform::GoldilocksTransformErrorV1::InvalidShape)
    }

    fn validate_output(
        &self,
    ) -> Result<(), crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        use crate::goldilocks_transform::{
            GoldilocksTransformErrorV1 as Error, goldilocks_transform_completion_uncertain_v1,
        };
        if goldilocks_transform_completion_uncertain_v1() {
            return Err(Error::CompletionUncertain);
        }
        let words = self
            .rows
            .checked_mul(self.columns)
            .ok_or(Error::InvalidShape)?;
        if self.rows < 2
            || !self.rows.is_power_of_two()
            || self.rows.trailing_zeros() > 32
            || !(1..=crate::goldilocks_transform::MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1)
                .contains(&self.columns)
            || self.buffer.len() != words
            || self.buffer.backing.pages.len() != metal_buffer_page_count(words)
            || self.buffer.backing.pages.capacity() != self.buffer.backing.pages.len()
        {
            return Err(Error::InvalidShape);
        }
        if !completed_output_is_canonical_v1(&self.buffer.backing.pages, words) {
            return Err(Error::NonCanonicalOutput);
        }
        // A different submitted command may become uncertain during the scan.
        // Every refusal still precedes the first destination write.
        if goldilocks_transform_completion_uncertain_v1() {
            return Err(Error::CompletionUncertain);
        }
        Ok(())
    }

    fn publish_into(
        &self,
        destinations: &mut [&mut [u64]],
    ) -> Result<(), crate::goldilocks_transform::GoldilocksTransformErrorV1> {
        use crate::goldilocks_transform::{
            GoldilocksTransformErrorV1 as Error, goldilocks_transform_completion_uncertain_v1,
        };
        if goldilocks_transform_completion_uncertain_v1() {
            return Err(Error::CompletionUncertain);
        }
        if destinations.len() != self.columns
            || destinations.iter().any(|column| column.len() != self.rows)
        {
            return Err(Error::InvalidShape);
        }
        self.validate_output()?;
        #[cfg(test)]
        let diagnostic_copy = diagnostic_start();
        for (index, column) in destinations.iter_mut().enumerate() {
            self.buffer.copy_range_to_slice(index * self.rows, column);
        }
        #[cfg(test)]
        diagnostic_copy_back(diagnostic_copy);
        Ok(())
    }
}

fn submit(
    context: &MetalPipelines,
    columns: &Buffer,
    twiddles: &Buffer,
    mut args: ExactRootFftArgs,
) -> MetalResult<DispatchTicket> {
    let (queue, queue_index) = context.queues.select(args.column_count, 0);
    let mut permit = CommandPermit::try_new(queue_index)?;
    autoreleasepool(|| {
        let command = try_command_buffer(queue)?;
        let encoder = try_compute_encoder(&command)?;
        encoder.set_buffer(0, Some(columns), 0);
        encoder.set_buffer(1, Some(twiddles), 0);
        let encode = |pipeline: &ComputePipelineState, groups: u64, args: &ExactRootFftArgs| {
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_bytes(
                2,
                mem::size_of::<ExactRootFftArgs>() as u64,
                ptr::from_ref(args).cast(),
            );
            encoder.dispatch_thread_groups(
                MTLSize::new(groups, u64::from(args.column_count), 1),
                MTLSize::new(LANES, 1, 1),
            );
            // A resource barrier orders writes across *all* groups. It also
            // separates bit reversal from tiles and tiles from global stages.
            // A shader threadgroup barrier cannot establish those dependencies.
            encoder.memory_barrier_with_resources(&[columns]);
        };
        encode(
            &context.exact_root_bit_reverse,
            args.column_len.div_ceil(LANES),
            &args,
        );
        encode(
            &context.exact_root_local_tiles,
            args.column_len >> args.log_len.min(LOCAL_LOG),
            &args,
        );
        for stage in (LOCAL_LOG..args.log_len).step_by(2) {
            args.stage = stage;
            encode(
                &context.exact_root_global_stage,
                args.column_len.div_ceil(WORDS_PER_GLOBAL_GROUP),
                &args,
            );
        }
        encoder.end_encoding();
        let completion = permit.completion();
        let completion_handler = ConcreteBlock::new(move |_| completion.complete()).copy();
        command.add_completed_handler(&completion_handler);
        permit.mark_launched();
        command.commit();
        Ok(DispatchTicket {
            command,
            trace_label: None,
            timing_start: None,
            kernel_context: None,
            permit,
            adaptive_sample: None,
            drain_budget: ticket_lifetime::ticket_budget(METAL_COMMAND_TIMEOUT),
            completion_checked: false,
        })
    })
}

#[cfg(test)]
#[path = "metal_exact_root_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "metal_exact_root_transaction_tests.rs"]
mod transaction_tests;

#[cfg(test)]
#[path = "metal_exact_root_canonical_scan_tests.rs"]
mod canonical_scan_tests;

#[cfg(test)]
#[path = "metal_exact_root_mapping_tests.rs"]
mod mapping_tests;

#[cfg(test)]
#[path = "metal_exact_root_prefix_tests.rs"]
pub(crate) mod prefix_tests;

#[cfg(test)]
#[path = "metal_exact_root_readonly_tests.rs"]
mod readonly_tests;
