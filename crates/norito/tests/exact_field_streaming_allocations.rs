//! Allocation and wire-parity checks for exact-length nested serialization.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use norito::core::{
    DecodeFlagsGuard, Encoder, Error, NoritoSerialize, header_flags, serialize_to_buffer,
};

struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACKING.with(|tracking| {
            if tracking.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        // SAFETY: this allocator delegates the request unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from the matching System allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        TRACKING.with(|tracking| {
            if tracking.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        // SAFETY: `ptr` and `layout` came from System and `new_size` is forwarded.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn allocations_during(operation: impl FnOnce()) -> usize {
    TRACKING.with(|tracking| tracking.set(false));
    ALLOCATIONS.with(|allocations| allocations.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    operation();
    TRACKING.with(|tracking| tracking.set(false));
    ALLOCATIONS.with(Cell::get)
}

struct ExactBlob(Vec<u8>);

impl NoritoSerialize for ExactBlob {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        NoritoSerialize::encoded_len_exact(&self.0)
    }
}

struct UnknownBlob(Vec<u8>);

impl NoritoSerialize for UnknownBlob {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        NoritoSerialize::serialize(&self.0, writer)
    }
}

#[derive(NoritoSerialize)]
struct ExactInner {
    payload: ExactBlob,
}

#[derive(NoritoSerialize)]
struct UnknownInner {
    payload: UnknownBlob,
}

#[derive(NoritoSerialize)]
struct ExactOuter {
    inner: ExactInner,
}

#[derive(NoritoSerialize)]
struct UnknownOuter {
    inner: UnknownInner,
}

#[derive(NoritoSerialize)]
enum ExactEnum {
    Payload(ExactBlob),
}

#[derive(NoritoSerialize)]
enum UnknownEnum {
    Payload(UnknownBlob),
}

fn bare_bytes(value: &dyn NoritoSerialize, flags: u8) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
    let mut bytes = Vec::new();
    serialize_to_buffer(value, &mut bytes).expect("serialize test value");
    bytes
}

#[test]
fn exact_and_unknown_field_paths_have_identical_wire_bytes() {
    let flags = [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ
            | header_flags::PACKED_STRUCT
            | header_flags::FIELD_BITSET
            | header_flags::COMPACT_LEN,
    ];

    for flags in flags {
        assert_eq!(
            bare_bytes(
                &ExactOuter {
                    inner: ExactInner {
                        payload: ExactBlob(vec![0xA5; 1_025]),
                    },
                },
                flags,
            ),
            bare_bytes(
                &UnknownOuter {
                    inner: UnknownInner {
                        payload: UnknownBlob(vec![0xA5; 1_025]),
                    },
                },
                flags,
            ),
            "struct wire changed for flags {flags:#04x}",
        );
        assert_eq!(
            bare_bytes(&ExactEnum::Payload(ExactBlob(vec![0x5A; 1_025])), flags),
            bare_bytes(
                &UnknownEnum::Payload(UnknownBlob(vec![0x5A; 1_025])),
                flags,
            ),
            "enum wire changed for flags {flags:#04x}",
        );
    }
}

#[test]
fn large_exact_nested_box_streams_without_temporary_allocation() {
    let flags = header_flags::COMPACT_LEN;
    let value = Box::new(ExactOuter {
        inner: ExactInner {
            payload: ExactBlob(vec![0xC3; 1024 * 1024]),
        },
    });
    let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
    let exact_len = NoritoSerialize::encoded_len_exact(&value).expect("exact boxed length");
    let mut output = Vec::with_capacity(exact_len);

    // Initialize thread-local state and the serializer before measuring.
    let mut warm = Vec::with_capacity(exact_len);
    serialize_to_buffer(&value, &mut warm).expect("warm exact serialization");
    drop(warm);
    let allocations = allocations_during(|| {
        assert_eq!(
            NoritoSerialize::encoded_len_exact(&value),
            Some(exact_len)
        );
        serialize_to_buffer(&value, &mut output).expect("stream exact boxed value");
    });

    assert_eq!(output.len(), exact_len);
    assert_eq!(allocations, 0, "exact nested serialization allocated");
}
