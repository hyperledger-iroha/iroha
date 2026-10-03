//! Standalone, unlinked component probe for the exact draft claim codec.
//!
//! Compile with rustc against the Norito, iroha_allocation and blake3 rlibs emitted by the
//! successful daemon Cargo JSON. This reuses format.rs without linking the journal into irohad.
//! Its allocation pool is a probe fixture, not a State owner or production allocation grant.
//! Native I/O, transaction wire production, State/finality and inventory admission are excluded.

use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    io,
};

const MAX_CLAIM_BYTES_V1: usize = 4 * 1024;
const MAX_WIRE_BYTES_V1: usize = 64 * 1024;

#[derive(Debug)]
enum ControlJournalErrorV1 {
    Invalid,
    Allocation(ChargedBufferError),
    Codec(norito::Error),
}

impl From<ChargedBufferError> for ControlJournalErrorV1 {
    fn from(error: ChargedBufferError) -> Self {
        Self::Allocation(error)
    }
}

impl std::fmt::Display for ControlJournalErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid => formatter.write_str("invalid claim"),
            Self::Allocation(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
        }
    }
}

#[path = "format.rs"]
mod format;

#[derive(Clone, Copy, Debug)]
struct Request {
    size: usize,
    align: usize,
    kind: u8,
}

#[derive(Clone, Copy)]
struct Census {
    requests: [Request; 256],
    count: usize,
}

impl Census {
    const EMPTY: Self = Self {
        requests: [Request {
            size: 0,
            align: 0,
            kind: 0,
        }; 256],
        count: 0,
    };
}

thread_local! { static ACTIVE: Cell<Option<Census>> = const { Cell::new(None) }; }

struct ObservedAllocator;

fn record(size: usize, align: usize, kind: u8) {
    let _ = ACTIVE.try_with(|active| {
        if let Some(mut census) = active.get() {
            if census.count < census.requests.len() {
                census.requests[census.count] = Request { size, align, kind };
            }
            census.count += 1;
            active.set(Some(census));
        }
    });
}

// SAFETY: the observer only records scalars in preallocated thread-local storage. Every
// original allocator argument and returned pointer is forwarded unchanged to System.
#[allow(unsafe_code, reason = "isolated component allocation census")]
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), layout.align(), 1);
        // SAFETY: forwards the original caller allocation contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), layout.align(), 2);
        // SAFETY: forwards the original caller allocation contract.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size, layout.align(), 3);
        // SAFETY: forwards the original live allocation and requested size.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: returns the original live allocation to its original allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(None));
    }
}

fn observe<T>(name: &str, operation: impl FnOnce() -> T) -> (T, Census) {
    let prior = ACTIVE.with(|active| active.replace(Some(Census::EMPTY)));
    assert!(prior.is_none());
    let scope = Observation;
    let value = operation();
    let census = ACTIVE.with(|active| active.get().unwrap());
    drop(scope);
    print!("{name}: requests={}", census.count);
    for request in &census.requests[..census.count.min(census.requests.len())] {
        print!(
            " [kind={} size={} align={}]",
            request.kind, request.size, request.align
        );
    }
    println!();
    assert!(
        census.count <= census.requests.len(),
        "census record overflow"
    );
    (value, census)
}

fn main() {
    use format::{
        ClaimBindingV1, ClaimDescriptorV1, ClaimFrameV1, ControlSlotV1, SlotNamesV1, decode_claim,
    };
    // A separate process per shape/operation makes the first observation codec-cold. Reading a
    // prior exact frame here uses a fixture Vec outside observation; it is not production funding.
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: probe advance|check fixture|measure|encode|view|scope|decode|verify|full-decode fixture-path"
    );
    let slot = match args[1].as_str() {
        "advance" => ControlSlotV1::Advance {
            predecessor_revision: 0,
        },
        "check" => ControlSlotV1::Check { challenge: [3; 32] },
        _ => panic!("unknown shape"),
    };
    let budget = AllocationBudget::new(4 * MAX_CLAIM_BYTES_V1);
    let binding = ClaimBindingV1 {
        network_id: [1; 32],
        owner_marker_digest: [0; 32],
        session_id: [2; 32],
    };
    let wire = [1; 64]; // Opaque inert bytes; this probe makes no signed-transaction claim.
    let descriptor = ClaimDescriptorV1::new(binding, slot, &wire).unwrap();
    descriptor.validate().unwrap();
    assert!(descriptor.matches_wire(&wire));
    let names = SlotNamesV1::new(slot).unwrap();
    assert_ne!(names.claim(), names.wire());
    if args[2] == "fixture" {
        let frame = ClaimFrameV1::encode(&descriptor, &budget).unwrap();
        std::fs::write(&args[3], frame.0.as_slice()).unwrap();
        return;
    }
    let fixture = std::fs::read(&args[3]).unwrap();
    println!(
        "shape={} operation={} fixture-bytes={}",
        args[1],
        args[2],
        fixture.len()
    );
    for phase in ["cold", "warm-1", "warm-2"] {
        match args[2].as_str() {
            "measure" => {
                let (length, census) = observe(phase, || norito::canonical_frame_len(&descriptor));
                assert_eq!(length.unwrap(), fixture.len());
                assert_eq!(census.count, 0, "unexpected measurement scratch");
            }
            "encode" => {
                let (frame, census) = observe(phase, || ClaimFrameV1::encode(&descriptor, &budget));
                let frame = frame.unwrap();
                assert_eq!(frame.0.as_slice(), fixture);
                assert_eq!(budget.reserved_bytes(), fixture.len());
                assert!(frame.0.belongs_to(&budget));
                assert_eq!(census.count, 1, "unexpected encoding scratch");
                let request = census.requests[0];
                assert_eq!(request.kind, 1);
                assert_eq!(request.size, fixture.len());
                assert_eq!(request.align, 1);
                println!(
                    "owned-output: size={} align=1; total requests={}",
                    fixture.len(),
                    census.count
                );
                drop(frame);
            }
            "decode" => {
                let (decoded, _) = observe(phase, || {
                    let view = norito::core::from_bytes_view(&fixture)?;
                    view.decode_exact::<ClaimDescriptorV1>()
                });
                assert_eq!(decoded.unwrap(), descriptor);
            }
            "view" => {
                let (view, census) = observe(phase, || norito::core::from_bytes_view(&fixture));
                view.unwrap();
                assert_eq!(census.count, 0, "unexpected borrowed frame scratch");
            }
            "scope" => {
                observe(phase, || {
                    norito::core::with_decode_limits(
                        norito::canonical_decode_limits(fixture.len()),
                        || Ok(()),
                    )
                })
                .0
                .unwrap();
            }
            "verify" => {
                let (verified, census) = observe(phase, || {
                    norito::verify_exact_canonical_frame(&descriptor, &fixture)
                });
                verified.unwrap();
                assert_eq!(census.count, 0, "unexpected exact verification scratch");
            }
            "full-decode" => {
                assert_eq!(
                    observe(phase, || decode_claim(&fixture)).0.unwrap(),
                    descriptor
                );
            }
            _ => panic!("unknown operation"),
        }
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
