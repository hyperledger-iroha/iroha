//! Allocation and wire regressions for streamed metadata serialization.
// This isolated integration test needs `GlobalAlloc` to observe steady-state heap traffic in the
// production serializer without affecting the existing allocation-test binaries.
#![allow(unsafe_code)]

use iroha_data_model::{metadata::Metadata, prelude::Name};
use iroha_primitives::json::Json;
use norito::core::{DecodeFlagsGuard, Encoder, SerializePayload, header_flags};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

const ENTRY_COUNT: usize = 2;
const SMALL_JSON_STRING_BYTES: usize = 16;
const LARGE_JSON_STRING_BYTES: usize = 256 * 1024;

struct TrackingAllocator;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AllocationStats {
    count: usize,
    requested_bytes: usize,
    largest_request: usize,
}

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    static REQUESTED_BYTES: Cell<usize> = const { Cell::new(0) };
    static LARGEST_REQUEST: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn record_allocation(requested_bytes: usize) {
    TRACKING.with(|tracking| {
        if !tracking.get() {
            return;
        }
        ALLOCATION_COUNT.with(|count| count.set(count.get().saturating_add(1)));
        REQUESTED_BYTES.with(|bytes| {
            bytes.set(bytes.get().saturating_add(requested_bytes));
        });
        LARGEST_REQUEST.with(|largest| largest.set(largest.get().max(requested_bytes)));
    });
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: the allocation request is delegated unchanged to `System`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: the allocation request is delegated unchanged to `System`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from the matching `System` allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation(new_size);
        // SAFETY: the original allocation and resized request are forwarded to `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

struct StopTracking;

impl Drop for StopTracking {
    fn drop(&mut self) {
        TRACKING.with(|tracking| tracking.set(false));
    }
}

fn allocations_during<T>(operation: impl FnOnce() -> T) -> (T, AllocationStats) {
    TRACKING.with(|tracking| tracking.set(false));
    ALLOCATION_COUNT.with(|count| count.set(0));
    REQUESTED_BYTES.with(|bytes| bytes.set(0));
    LARGEST_REQUEST.with(|largest| largest.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    let stop = StopTracking;
    let result = operation();
    drop(stop);
    let stats = AllocationStats {
        count: ALLOCATION_COUNT.with(Cell::get),
        requested_bytes: REQUESTED_BYTES.with(Cell::get),
        largest_request: LARGEST_REQUEST.with(Cell::get),
    };
    (result, stats)
}

fn canonical_json_string(payload_bytes: usize) -> Json {
    Json::from_raw_json(format!("\"{}\"", "x".repeat(payload_bytes)))
        .expect("canonical bounded JSON string")
}

fn metadata_fixture(payload_bytes: usize) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "payload".parse::<Name>().expect("metadata name"),
        canonical_json_string(payload_bytes),
    );
    metadata.insert(
        "sentinel".parse::<Name>().expect("metadata name"),
        Json::from_raw_json("{\"fixed\":true}".to_owned()).expect("canonical sentinel JSON"),
    );
    assert_eq!(metadata.iter().len(), ENTRY_COUNT);
    metadata
}

fn serialize_preallocated(value: &Metadata, flags: u8) -> (Vec<u8>, AllocationStats) {
    let _flags = DecodeFlagsGuard::enter(flags);
    let exact = value
        .encoded_len_exact()
        .expect("metadata fixture has an exact encoded length");
    let mut output = Vec::with_capacity(exact);

    // Initialize encoder and thread-local state before measuring the steady-state path.
    {
        let mut encoder = Encoder::for_buffer(&mut output);
        value.serialize(&mut encoder).expect("warm serialization");
    }
    assert_eq!(output.len(), exact);
    output.clear();
    let original_capacity = output.capacity();

    let (result, stats) = allocations_during(|| {
        let mut encoder = Encoder::for_buffer(&mut output);
        value.serialize(&mut encoder)
    });
    result.expect("measured metadata serialization");
    assert_eq!(output.len(), exact);
    assert_eq!(output.capacity(), original_capacity);
    (output, stats)
}

#[test]
fn metadata_streaming_allocations_do_not_scale_with_json_payload() {
    let small = metadata_fixture(SMALL_JSON_STRING_BYTES);
    let large = metadata_fixture(LARGE_JSON_STRING_BYTES);

    let (small_fixed_bytes, small_fixed) = serialize_preallocated(&small, 0);
    let (large_fixed_bytes, large_fixed) = serialize_preallocated(&large, 0);
    assert!(large_fixed_bytes.len() > small_fixed_bytes.len());
    assert_eq!(small_fixed, AllocationStats::default());
    assert_eq!(large_fixed, small_fixed);

    let (_, small_packed) = serialize_preallocated(&small, header_flags::PACKED_SEQ);
    let (_, large_packed) = serialize_preallocated(&large, header_flags::PACKED_SEQ);
    let offset_lengths_bytes = ENTRY_COUNT * core::mem::size_of::<usize>();
    assert_eq!(
        small_packed,
        AllocationStats {
            count: 1,
            requested_bytes: offset_lengths_bytes,
            largest_request: offset_lengths_bytes,
        }
    );
    assert_eq!(large_packed, small_packed);
}

#[test]
fn empty_metadata_serialization_needs_no_heap_scratch() {
    let empty = Metadata::default();
    for flags in [0, header_flags::PACKED_SEQ] {
        let (_, stats) = serialize_preallocated(&empty, flags);
        assert_eq!(stats, AllocationStats::default());
    }
}

#[test]
fn allocation_fixtures_preserve_sequence_wire_and_roundtrip() {
    for metadata in [
        Metadata::default(),
        metadata_fixture(SMALL_JSON_STRING_BYTES),
        metadata_fixture(LARGE_JSON_STRING_BYTES),
    ] {
        let reference: Vec<(Name, Json)> = metadata
            .iter()
            .map(|(name, json)| (name.clone(), json.clone()))
            .collect();
        for requested in [
            0,
            header_flags::COMPACT_LEN,
            header_flags::PACKED_SEQ,
            header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        ] {
            let _flags = DecodeFlagsGuard::enter(requested);
            let (payload, flags) = norito::codec::encode_with_header_flags(&metadata);
            assert_eq!(
                (payload.clone(), flags),
                norito::codec::encode_with_header_flags(&reference),
                "metadata must retain the canonical sequence-of-tuples wire"
            );
            let frame = norito::core::frame_bare_with_header_flags::<Metadata>(&payload, flags)
                .expect("frame metadata allocation fixture");
            let decoded: Metadata =
                norito::decode_from_bytes(&frame).expect("roundtrip metadata allocation fixture");
            assert_eq!(decoded, metadata);
            assert_eq!(
                norito::to_bytes(&decoded).expect("re-encode metadata"),
                frame
            );
        }
    }
}
