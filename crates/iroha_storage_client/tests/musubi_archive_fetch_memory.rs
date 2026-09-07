//! Isolated logical-heap regression for the Musubi production CAR stream bridge.
//!
//! The child measurement covers the exact bounded worker/channel implementation, canonical CAR
//! layout/writer, the caller-retained plan, its worker clone, and one maximum provider chunk. It
//! deliberately excludes HTTP/TLS client state, response JSON DOM construction, and cache
//! filesystem ingestion; those require a deployment-equivalent process-RSS or cgroup gate before
//! the complete 64 MiB fetch requirement can be claimed.
// This integration test is the narrow exception that needs `GlobalAlloc` in order to
// measure the production stream bridge in an isolated child process.
#![allow(unsafe_code)]
use iroha_data_model::musubi::{MUSUBI_MAX_FILES_V1, MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1};
use sorafs_car::{CarBuildPlan, CarChunk, CarStreamingWriter, FilePlan};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    io::{self, Read},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "../src/musubi_archive_fetch/bounded_stream.rs"]
mod bounded_stream;
const CHILD_MODE_ENV: &str = "IROHA_MUSUBI_FETCH_MEMORY_CHILD_V1";
const FETCH_HEAP_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const BUNDLE_METADATA_FILE_COUNT: usize = 3;
struct PeakAllocator;
static CURRENT_ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
fn record_allocation(bytes: usize) {
    let current = CURRENT_ALLOCATED_BYTES
        .fetch_add(bytes, Ordering::SeqCst)
        .saturating_add(bytes);
    PEAK_ALLOCATED_BYTES.fetch_max(current, Ordering::SeqCst);
}
fn record_deallocation(bytes: usize) {
    CURRENT_ALLOCATED_BYTES.fetch_sub(bytes, Ordering::SeqCst);
}
// SAFETY: every operation delegates to `System` with the exact pointer/layout
// contract it received, then updates only independent atomic byte counters.
unsafe impl GlobalAlloc for PeakAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        // SAFETY: the pointer and layout are forwarded unchanged to their allocator.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the pointer, old layout, and requested size are forwarded unchanged.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            record_deallocation(layout.size());
            record_allocation(new_size);
        }
        replacement
    }
}
#[global_allocator]
static GLOBAL_ALLOCATOR: PeakAllocator = PeakAllocator;
struct ZeroReader {
    remaining: u64,
}
impl ZeroReader {
    const fn new(remaining: u64) -> Self {
        Self { remaining }
    }
}
impl Read for ZeroReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let output_len = u64::try_from(output.len()).expect("output length fits u64");
        let count = usize::try_from(self.remaining.min(output_len))
            .expect("bounded zero-reader length fits usize");
        output[..count].fill(0);
        self.remaining -= u64::try_from(count).expect("bounded read length fits u64");
        Ok(count)
    }
}
fn zero_digest(length: usize) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    let zeroes = [0_u8; 8 * 1024];
    let mut remaining = length;
    while remaining != 0 {
        let count = remaining.min(zeroes.len());
        hasher.update(&zeroes[..count]);
        remaining -= count;
    }
    hasher.finalize()
}
fn worst_case_v1_geometry_plan() -> CarBuildPlan {
    let profile = sorafs_car::chunker_registry::default_descriptor().profile;
    let content_length = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1;
    let file_count = usize::try_from(MUSUBI_MAX_FILES_V1)
        .expect("V1 file count fits usize")
        .saturating_add(BUNDLE_METADATA_FILE_COUNT);
    let mut extra = usize::try_from(content_length)
        .expect("V1 payload length fits usize")
        .checked_sub(file_count)
        .expect("one byte per file fits the V1 payload");
    let mut chunks = Vec::new();
    let mut files = Vec::new();
    let mut offset = 0_u64;
    for file_index in 0..file_count {
        let first_chunk = chunks.len();
        let mut size = 1_u64;
        if file_index == 0 {
            let maximum = profile.max_size.min(extra);
            if maximum >= profile.min_size {
                push_zero_chunk(&mut chunks, &mut offset, maximum);
                size += u64::try_from(maximum).expect("registered chunk length fits u64");
                extra -= maximum;
            }
            while extra >= profile.min_size {
                push_zero_chunk(&mut chunks, &mut offset, profile.min_size);
                size += u64::try_from(profile.min_size).expect("registered chunk length fits u64");
                extra -= profile.min_size;
            }
            size += u64::try_from(extra).expect("registered chunk length fits u64");
            push_zero_chunk(&mut chunks, &mut offset, extra.saturating_add(1));
            extra = 0;
        } else {
            push_zero_chunk(&mut chunks, &mut offset, 1);
        }
        files.push(FilePlan {
            path: vec![format!("file-{file_index:04}")],
            first_chunk,
            chunk_count: chunks.len() - first_chunk,
            size,
        });
    }
    assert_eq!(offset, content_length);
    let plan = CarBuildPlan {
        chunk_profile: profile,
        payload_digest: zero_digest(
            usize::try_from(content_length).expect("V1 payload length fits usize"),
        ),
        content_length,
        chunks,
        files,
    };
    plan.validate().expect("worst-case V1 geometry is valid");
    plan
}
fn push_zero_chunk(chunks: &mut Vec<CarChunk>, offset: &mut u64, length: usize) {
    let length_u32 = u32::try_from(length).expect("registered chunk length fits u32");
    chunks.push(CarChunk {
        offset: *offset,
        length: length_u32,
        digest: *zero_digest(length).as_bytes(),
    });
    *offset += u64::try_from(length).expect("registered chunk length fits u64");
}
fn retained_plan_heap_bytes(plan: &CarBuildPlan) -> usize {
    let mut retained = plan.chunks.capacity() * std::mem::size_of::<CarChunk>()
        + plan.files.capacity() * std::mem::size_of::<FilePlan>();
    for file in &plan.files {
        retained += file.path.capacity() * std::mem::size_of::<String>();
        retained += file.path.iter().map(String::capacity).sum::<usize>();
    }
    retained
}
fn measured_peak_delta(operation: impl FnOnce()) -> usize {
    let baseline = CURRENT_ALLOCATED_BYTES.load(Ordering::SeqCst);
    PEAK_ALLOCATED_BYTES.store(baseline, Ordering::SeqCst);
    operation();
    PEAK_ALLOCATED_BYTES
        .load(Ordering::SeqCst)
        .saturating_sub(baseline)
}
#[test]
fn production_stream_bridge_peak_heap_is_bounded() {
    let output = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--ignored",
            "--exact",
            "production_stream_bridge_peak_heap_child",
            "--nocapture",
        ])
        .env(CHILD_MODE_ENV, "1")
        .output()
        .expect("run isolated stream-memory child");
    assert!(
        output.status.success(),
        "isolated stream-memory child failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "executed in an isolated child by production_stream_bridge_peak_heap_is_bounded"]
fn production_stream_bridge_peak_heap_child() {
    if std::env::var_os(CHILD_MODE_ENV).is_none() {
        return;
    }
    let plan = worst_case_v1_geometry_plan();
    let mut source = ZeroReader::new(plan.content_length);
    let expected = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut source, io::sink())
        .expect("precompute canonical CAR statistics");
    let caller_plan_bytes = retained_plan_heap_bytes(&plan);
    let maximum_provider_chunk = plan.chunk_profile.max_size;
    let stream_peak_delta = measured_peak_delta(|| {
        let worker_plan = plan.clone();
        let expected_roots = expected.root_cids.clone();
        let expected_car_size = expected.car_size;
        let expected_car_digest = expected.car_archive_digest;
        let mut reader = bounded_stream::bounded_car_reader(expected_car_size, move |output| {
            let mut source = ZeroReader::new(worker_plan.content_length);
            let actual = CarStreamingWriter::with_expected_roots(&worker_plan, expected_roots)
                .write_from_reader(&mut source, output)
                .map_err(|_| "MUSUBI_MEMORY_PROBE_CAR_WRITE_FAILED")?;
            if actual.car_size != expected_car_size
                || actual.car_archive_digest != expected_car_digest
            {
                return Err("MUSUBI_MEMORY_PROBE_CAR_MISMATCH");
            }
            Ok(())
        })
        .expect("spawn production bounded CAR bridge");
        io::copy(&mut reader, &mut io::sink()).expect("consume exact production CAR bridge");
        drop(reader);
    });
    let accounted_bridge_peak = caller_plan_bytes
        .checked_add(stream_peak_delta)
        .and_then(|bytes| bytes.checked_add(maximum_provider_chunk))
        // The observed allocator delta is scheduling-dependent. Add the full
        // channel ownership reserve so a fast consumer cannot make this probe
        // understate four queued frames plus the producer/consumer frames.
        .and_then(|bytes| bytes.checked_add(bounded_stream::STREAM_MAX_OWNED_FRAME_BYTES))
        .expect("stream-memory accounting fits usize");
    eprintln!(
        "musubi stream bridge heap: caller_plan={caller_plan_bytes} measured_delta={stream_peak_delta} provider_chunk_reserve={maximum_provider_chunk} channel_reserve={} accounted={accounted_bridge_peak} limit={FETCH_HEAP_LIMIT_BYTES}",
        bounded_stream::STREAM_MAX_OWNED_FRAME_BYTES
    );
    assert!(
        accounted_bridge_peak <= FETCH_HEAP_LIMIT_BYTES,
        "production CAR stream bridge exceeded the 64 MiB logical-heap envelope"
    );
}
