//! Public-fixture diagnostic separating recurring Metal mapping from reuse.
//!
//! These deliberately retained 128MiB test allocations are not a production pool
//! policy. The normal 64MiB idle-pool charge and all production owners are unchanged.

use super::*;
use crate::goldilocks_transform::{
    CANONICAL_SCAN_TILE_WORDS_V1, canonical_word_slice_v1, parallel_canonical_scan_v1,
};
use zeroize::Zeroize as _;

/// A test-only Metal-owned allocation whose host access requires completed work.
struct ReusedPublicMapping {
    buffer: mem::ManuallyDrop<Buffer>,
    words: usize,
    host_access: bool,
}

impl ReusedPublicMapping {
    #[allow(unsafe_code)]
    fn new(device: &Device, words: usize) -> Self {
        let bytes = words.checked_mul(mem::size_of::<u64>()).unwrap() as u64;
        validate_metal_buffer_byte_len(device, bytes).unwrap();
        // SAFETY: the SDK method returns a +1 object, nullable on allocation
        // failure. Check nil before constructing its Rust owner, as production
        // buffer constructors do. Shared storage is host-visible on this target.
        let device_ref: &DeviceRef = device;
        let raw: *mut Object = unsafe {
            msg_send![device_ref,
                newBufferWithLength: bytes
                options: MTLResourceOptions::StorageModeShared
            ]
        };
        assert!(!raw.is_null(), "test Metal allocation failed");
        let buffer = unsafe { Buffer::from_ptr(raw.cast()) };
        assert_eq!(buffer.length(), bytes);
        assert!(!buffer.contents().is_null());
        assert_eq!(buffer.contents() as usize % mem::align_of::<u64>(), 0);
        Self {
            buffer: mem::ManuallyDrop::new(buffer),
            words,
            host_access: true,
        }
    }

    #[allow(unsafe_code)]
    fn words_mut(&mut self) -> &mut [u64] {
        assert!(self.host_access, "an uncompleted command owns these words");
        // SAFETY: this private test owner is unique, the checked allocation
        // contains exactly `words` aligned u64s, and no host borrow spans submit.
        // After submission, only authoritative successful completion re-enables
        // host access. The GPU lane is held by the enclosing diagnostic.
        unsafe { std::slice::from_raw_parts_mut(self.buffer.contents().cast(), self.words) }
    }

    fn transform(
        &mut self,
        context: &MetalPipelines,
        twiddles: &Buffer,
        columns: &mut [Vec<u64>],
        log: u32,
    ) -> PhaseDiagnostic {
        let rows = 1_usize << log;
        assert_eq!(columns.len() * rows, self.words);
        assert!(columns.iter().all(|column| column.len() == rows));
        let scope = PhaseDiagnosticScope::begin();
        let start = diagnostic_start();
        let flatten = diagnostic_start();
        for (destination, source) in self.words_mut().chunks_exact_mut(rows).zip(&*columns) {
            destination.copy_from_slice(source);
        }
        diagnostic_record(flatten, |record| &mut record.flatten);
        let args = ExactRootFftArgs {
            column_len: rows as u64,
            normalization: 1,
            log_len: log,
            column_count: columns.len().try_into().unwrap(),
            stage: 0,
            padding: 0,
        };
        let submission = diagnostic_start();
        // Refuse host access before submit, including any partial-submit unwind.
        // Failure conservatively retains this public allocation without wiping.
        self.host_access = false;
        let ticket = submit(context, &self.buffer, twiddles, args).unwrap();
        diagnostic_record(submission, |record| &mut record.submit);
        let waiting = diagnostic_start();
        wait_for_tickets_with_staging_phase([ticket], Some(ColumnStagingPhase::Fft)).unwrap();
        diagnostic_ticket_wait(waiting);
        self.host_access = true;
        let output = self.words_mut();
        let canonical = if parallel_canonical_scan_v1(output.len()) {
            output
                .par_chunks(CANONICAL_SCAN_TILE_WORDS_V1)
                .all(canonical_word_slice_v1)
        } else {
            output
                .chunks(CANONICAL_SCAN_TILE_WORDS_V1)
                .all(canonical_word_slice_v1)
        };
        assert!(canonical, "completed public output must be canonical");
        assert!(!backend_quarantined());
        let copying = diagnostic_start();
        for (destination, source) in columns.iter_mut().zip(output.chunks_exact(rows)) {
            destination.copy_from_slice(source);
        }
        diagnostic_copy_back(copying);
        let clearing = diagnostic_start();
        self.words_mut().zeroize();
        diagnostic_backing_drop(clearing);
        diagnostic_record(start, |record| &mut record.total);
        scope.finish()
    }
}

impl Drop for ReusedPublicMapping {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.host_access {
            self.words_mut().zeroize();
            // SAFETY: this is the sole explicit destruction, only when there
            // is no pending command. An uncertain/unwinding command retains
            // its public allocation instead of permitting host clearing.
            unsafe { mem::ManuallyDrop::drop(&mut self.buffer) };
        }
    }
}

/// Copy of the ordinary no-copy mapping plumbing, exposing its retention handle
/// only to this closed public-fixture owner. Production mapping is unchanged.
#[allow(unsafe_code)]
fn retained_public_pooled_buffer(
    device: &Device,
    data: &mut PooledBuffer,
) -> MetalResult<(Buffer, Arc<MetalBufferBackingRetention>)> {
    let (data_ptr, byte_len) = data.metal_region();
    validate_metal_buffer_byte_len(device, byte_len)?;
    let retention = Arc::new(MetalBufferBackingRetention::new(data.backing()));
    let completion_retention = Arc::clone(&retention);
    let deallocator = ConcreteBlock::new(move |_: *const c_void, _: NSUInteger| {
        completion_retention.release();
    })
    .copy();
    let deallocator_block: &Block<(*const c_void, NSUInteger), ()> = &deallocator;
    let device_ref: &DeviceRef = device;
    // SAFETY: identical to shared_pooled_buffer: page-aligned initialized pages
    // stay retained until the copied deallocator releases its backing handle.
    // The nullable SDK result is checked before accepting +1 object ownership.
    let raw: *mut Object = unsafe {
        msg_send![device_ref,
            newBufferWithBytesNoCopy: data_ptr
            length: byte_len
            options: MTLResourceOptions::StorageModeShared
            deallocator: Some(deallocator_block)
        ]
    };
    if raw.is_null() {
        return Err(metal_nil_error(
            "-[MTLDevice newBufferWithBytesNoCopy:length:options:deallocator:]",
        ));
    }
    Ok((unsafe { Buffer::from_ptr(raw.cast()) }, retention))
}

fn rearm_public_pooled_backing(retention: &MetalBufferBackingRetention, data: &PooledBuffer) {
    let mut retained = retention.backing.lock().expect("unpoisoned test retention");
    assert!(retained.is_none(), "previous backing was not discharged");
    *retained = Some(data.backing());
}

/// Same clearing PooledBuffer as the original route, with a retained Metal view.
/// Host writes require successful completion plus unique backing Arc ownership.
struct ReusedPublicPooledMapping {
    buffer: mem::ManuallyDrop<Buffer>,
    pooled: mem::ManuallyDrop<PooledBuffer>,
    retention: mem::ManuallyDrop<Arc<MetalBufferBackingRetention>>,
    host_access: bool,
}

impl ReusedPublicPooledMapping {
    fn new(device: &Device, words: usize) -> Self {
        let mut pooled = PooledBuffer::sensitive_zeroed(words).unwrap();
        let (buffer, retention) = retained_public_pooled_buffer(device, &mut pooled).unwrap();
        assert_eq!(buffer.length() as usize, words * mem::size_of::<u64>());
        assert_eq!(pooled.backing.pages.len(), pooled.backing.pages.capacity());
        assert_eq!(
            buffer.contents() as usize,
            pooled.backing.pages.as_ptr() as usize
        );
        // No command has ever seen this mapping. The outer PooledBuffer owns
        // every page while the idle view's callback is temporarily detached.
        retention.release();
        assert!(Arc::get_mut(&mut pooled.backing).is_some());
        Self {
            buffer: mem::ManuallyDrop::new(buffer),
            pooled: mem::ManuallyDrop::new(pooled),
            retention: mem::ManuallyDrop::new(retention),
            host_access: true,
        }
    }

    fn assert_host_access(&self) {
        assert!(self.host_access, "an uncompleted command owns these pages");
        assert_eq!(Arc::strong_count(&self.pooled.backing), 1);
        assert!(self.retention.backing.lock().unwrap().is_none());
    }

    fn identity(&self) -> (usize, usize, usize) {
        self.assert_host_access();
        (
            self.buffer.contents() as usize,
            self.pooled.backing.pages.as_ptr() as usize,
            self.pooled.backing.pages.capacity(),
        )
    }

    fn clear(&mut self) {
        self.assert_host_access();
        // Same erasure as ordinary PooledBufferBacking::drop, including all
        // allocated page padding; Arc::get_mut forbids hidden shared mutation.
        Arc::get_mut(&mut self.pooled.backing)
            .expect("completed backing is uniquely owned")
            .wipe_sensitive_pages();
    }

    fn is_clear(&self) -> bool {
        self.assert_host_access();
        self.pooled
            .backing
            .pages
            .iter()
            .all(|page| page.words.iter().all(|&word| word == 0))
    }

    fn transform(
        &mut self,
        context: &MetalPipelines,
        twiddles: &Buffer,
        columns: &mut [Vec<u64>],
        log: u32,
    ) -> PhaseDiagnostic {
        let rows = 1_usize << log;
        assert_eq!(columns.len() * rows, self.pooled.len());
        assert!(columns.iter().all(|column| column.len() == rows));
        self.assert_host_access();
        assert!(!backend_quarantined());
        let scope = PhaseDiagnosticScope::begin();
        let start = diagnostic_start();
        let flatten = diagnostic_start();
        for (column, source) in columns.iter().enumerate() {
            self.pooled.copy_from_slice_at(column * rows, source);
        }
        diagnostic_record(flatten, |record| &mut record.flatten);
        let args = ExactRootFftArgs {
            column_len: rows as u64,
            normalization: 1,
            log_len: log,
            column_count: columns.len().try_into().unwrap(),
            stage: 0,
            padding: 0,
        };
        let submission = diagnostic_start();
        // End all mutable host borrows, then forbid host access before re-arming
        // callback custody. Any panic from here retains all three owners.
        self.host_access = false;
        rearm_public_pooled_backing(&self.retention, &self.pooled);
        let ticket = submit(context, &self.buffer, twiddles, args).unwrap();
        diagnostic_record(submission, |record| &mut record.submit);
        let waiting = diagnostic_start();
        wait_for_tickets_with_staging_phase([ticket], Some(ColumnStagingPhase::Fft)).unwrap();
        diagnostic_ticket_wait(waiting);
        // Only authoritative Completed reaches here. The retained Metal view
        // continues to exist; its backing handle can now be detached so the
        // outer PooledBuffer alone owns the pages throughout host access.
        self.retention.release();
        assert!(Arc::get_mut(&mut self.pooled.backing).is_some());
        self.host_access = true;
        assert!(!backend_quarantined());
        assert!(completed_output_is_canonical_v1(
            &self.pooled.backing.pages,
            self.pooled.len(),
        ));
        let copying = diagnostic_start();
        for (column, destination) in columns.iter_mut().enumerate() {
            self.pooled.copy_range_to_slice(column * rows, destination);
        }
        diagnostic_copy_back(copying);
        let clearing = diagnostic_start();
        self.clear();
        diagnostic_backing_drop(clearing);
        diagnostic_record(start, |record| &mut record.total);
        scope.finish()
    }
}

impl Drop for ReusedPublicPooledMapping {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.host_access {
            self.clear();
            // SAFETY: sole explicit destruction after completed/never-submitted
            // work. Destroy the view before releasing the final backing owner.
            unsafe {
                mem::ManuallyDrop::drop(&mut self.buffer);
                mem::ManuallyDrop::drop(&mut self.retention);
                mem::ManuallyDrop::drop(&mut self.pooled);
            }
        }
        // A pending/uncertain/unwinding command keeps the Metal view, callback
        // handle and original pages alive without host reads, writes or recycle.
    }
}

#[test]
fn retained_public_pooled_backing_rearms_original_pages_and_clears_padding() {
    let mut pooled = PooledBuffer::sensitive_zeroed(METAL_BUFFER_PAGE_WORDS + 1).unwrap();
    let pointer = pooled.backing.pages.as_ptr() as usize;
    let retention = MetalBufferBackingRetention::new(pooled.backing());
    assert!(Arc::get_mut(&mut pooled.backing).is_none());
    for value in [7, 19, 31] {
        retention.release();
        let backing = Arc::get_mut(&mut pooled.backing).unwrap();
        assert_eq!(backing.pages.as_ptr() as usize, pointer);
        for page in &mut backing.pages {
            page.words.fill(value);
        }
        // Include the initialized padding beyond the logical last word.
        backing.wipe_sensitive_pages();
        assert!(
            backing
                .pages
                .iter()
                .all(|page| page.words.iter().all(|&word| word == 0))
        );
        rearm_public_pooled_backing(&retention, &pooled);
        assert!(Arc::get_mut(&mut pooled.backing).is_none());
    }
    retention.release();
    assert!(Arc::get_mut(&mut pooled.backing).is_some());
}

#[test]
#[ignore = "isolated actual Metal fresh-map versus retained Metal-owned/no-copy mapping; optional 8GiB public resident fixture"]
fn required_metal_common22_mapping_reuse_preserves_complete_public_outputs() {
    const LOG: u32 = 22;
    const ROWS: usize = 1 << LOG;
    const WIDTH: usize = 4;
    const ROUNDS: usize = 12;
    assert!(!cfg!(debug_assertions), "run optimized diagnostic");
    assert_eq!(rayon::current_num_threads(), 20, "use RAYON_NUM_THREADS=20");
    assert!(rayon::current_thread_index().is_none());
    let resident_gib: usize = std::env::var("X509_MAPPING_RESIDENT_GIB").map_or(0, |value| {
        value
            .parse()
            .expect("public fixture GiB must be an integer")
    });
    assert!(
        [0, 8].contains(&resident_gib),
        "only bounded 0/8GiB fixture is supported"
    );
    let mut resident = Vec::<u8>::new();
    resident.try_reserve_exact(resident_gib << 30).unwrap();
    resident.resize(resident_gib << 30, 0);
    // Touch each 4KiB interval so the zero-filled allocation is actually resident,
    // even on hosts whose zero pages would otherwise be mapped lazily.
    for page in resident.chunks_mut(4096) {
        page[0] = 1;
    }
    let _lane = crate::backend::acquire_gpu_lane();
    let context = metal_context().expect("actual Metal is required");
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> LOG);
    let twiddles = context
        .factorized_root_twiddle_buffer(LOG, root, false)
        .unwrap();
    let original: Vec<Vec<u64>> = (0..WIDTH)
        .map(|column| {
            (0..ROWS)
                .map(|row| (31 * row + 17 * column + 1) as u64)
                .collect()
        })
        .collect();
    let mut expected = original.clone();
    expected.par_iter_mut().for_each(|column| {
        crate::cyclotomic::fft(
            column,
            crate::cyclotomic::Domain {
                log_size: LOG,
                generator: root,
            },
        );
    });
    let mut actual = original.clone();
    let pointers: [(usize, usize); WIDTH] =
        core::array::from_fn(|index| (actual[index].as_ptr() as usize, actual[index].capacity()));
    let allocation_start = Instant::now();
    let mut mapping = ReusedPublicMapping::new(&context.device, ROWS * WIDTH);
    mapping.words_mut().zeroize();
    let retained_allocation_ms = allocation_start.elapsed().as_secs_f64() * 1000.0;
    let allocation_start = Instant::now();
    let mut pooled_mapping = ReusedPublicPooledMapping::new(&context.device, ROWS * WIDTH);
    pooled_mapping.clear();
    let retained_pooled_allocation_ms = allocation_start.elapsed().as_secs_f64() * 1000.0;
    let pooled_identity = pooled_mapping.identity();
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            enable_queue_depth_stats(false);
        }
    }
    enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    let mut samples = [[(
        0.0,
        ColumnStagingPhaseStats::default(),
        PhaseDiagnostic::default(),
    ); ROUNDS]; 3];
    for round in 0..=ROUNDS {
        // Every arm occupies every order position equally over the12 samples;
        // all six permutations repeat twice after one unmeasured warmup round.
        let order = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for mode in order[round.saturating_sub(1) % order.len()] {
            for (destination, source) in actual.iter_mut().zip(&original) {
                destination.copy_from_slice(source);
            }
            take_column_staging_stats().unwrap();
            let started = Instant::now();
            let phase = if mode == 0 {
                let scope = PhaseDiagnosticScope::begin();
                transform(&mut actual, LOG, root, false).unwrap();
                scope.finish()
            } else if mode == 1 {
                mapping.transform(context, &twiddles, &mut actual, LOG)
            } else {
                pooled_mapping.transform(context, &twiddles, &mut actual, LOG)
            };
            let total_ms = started.elapsed().as_secs_f64() * 1000.0;
            let staging = take_column_staging_stats().unwrap().fft();
            assert_eq!(staging.batches, u32::from(mode == 0));
            assert_eq!(staging.timestamped_commands, 1);
            assert_eq!(actual, expected, "complete CPU parity, mode={mode}");
            assert_eq!(
                core::array::from_fn::<_, WIDTH, _>(|index| (
                    actual[index].as_ptr() as usize,
                    actual[index].capacity()
                )),
                pointers,
            );
            assert!(mapping.words_mut().iter().all(|&word| word == 0));
            assert!(pooled_mapping.is_clear());
            assert_eq!(pooled_mapping.identity(), pooled_identity);
            assert!(!backend_quarantined());
            if round > 0 {
                samples[mode][round - 1] = (total_ms, staging, phase);
            }
        }
    }
    // Retain touched pages for the entire three-arm run; prevent early release.
    assert!(resident.chunks(4096).all(|page| page[0] == 1));
    let cleanup_start = Instant::now();
    drop(mapping);
    let retained_cleanup_ms = cleanup_start.elapsed().as_secs_f64() * 1000.0;
    let cleanup_start = Instant::now();
    drop(pooled_mapping);
    let retained_pooled_cleanup_ms = cleanup_start.elapsed().as_secs_f64() * 1000.0;
    for (mode, samples) in samples.into_iter().enumerate() {
        for (round, (total_ms, staging, phase)) in samples.into_iter().enumerate() {
            let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
            eprintln!(
                "exact_root_mapping_reuse mode={mode} log={LOG} width={WIDTH} round={round} resident_gib={resident_gib} retained_allocation_ms={retained_allocation_ms:.6} retained_pooled_allocation_ms={retained_pooled_allocation_ms:.6} retained_cleanup_ms={retained_cleanup_ms:.6} retained_pooled_cleanup_ms={retained_pooled_cleanup_ms:.6} total_ms={total_ms:.6} flatten_ms={:.6} mapping_ms={:.6} submit_ms={:.6} ticket_wait_ms={:.6} copyback_ms={:.6} clearing_ms={:.6} gpu_ms={:.6} driver_ms={:.6} pre_gpu_queue_ms={:.6} parity=all_words_exact pointers=unchanged; mode0=original_allocate_map_publish_clear mode1=retained_metal_owned_fill_submit_publish_clear mode2=retained_original_pooled_nocopy_rearm_fill_submit_publish_clear; per_arm_mapped_bytes=134217728 total_retained_test_bytes=268435456; one_time_retained_allocation_and_terminal_cleanup_reported_separately; caller_reset_allocation_clear_parity_and_resident_touch_excluded; retained_test_owners_not_a_production_pool_policy; intervals_not_additive",
                ms(phase.flatten),
                ms(phase.mapping),
                ms(phase.submit),
                ms(phase.ticket_wait),
                ms(phase.copy_back),
                ms(phase.backing_drop),
                staging.gpu_execution_ms,
                staging.cpu_driver_scheduling_ms,
                staging.pre_gpu_queue_ms,
            );
        }
    }
}
