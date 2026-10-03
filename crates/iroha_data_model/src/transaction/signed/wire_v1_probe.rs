//! Standalone UNLINKED writer parity and allocation census using exact daemon Cargo artifacts.
//!
//! Construct/sign fixtures on another thread so the measured thread has not entered a model
//! serializer before its first observation. This is cold encoding-thread evidence; fixture
//! construction can warm process-wide crypto state and is outside admission claims. All profiles
//! are public synthetic input. No original State, signed custody or release qualification exists.

use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_data_model::transaction::TransactionEntrypoint;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    io::{self, Write},
};

#[path = "wire_v1_test_support.rs"]
mod support;
#[path = "wire_v1.rs"]
mod wire;

#[derive(Clone, Copy)]
struct Census {
    count: usize,
    total_requested: usize,
    sizes: [usize; 512],
}
impl Census {
    const EMPTY: Self = Self {
        count: 0,
        total_requested: 0,
        sizes: [0; 512],
    };
}
thread_local! { static ACTIVE: Cell<Option<Census>> = const { Cell::new(None) }; }
struct ObservedAllocator;
fn record(size: usize) {
    let _ = ACTIVE.try_with(|active| {
        if let Some(mut census) = active.get() {
            if census.count < census.sizes.len() {
                census.sizes[census.count] = size;
            }
            census.count += 1;
            census.total_requested += size;
            active.set(Some(census));
        }
    });
}
// SAFETY: only fixed thread-local integers are recorded; all allocator inputs and pointers are
// forwarded unchanged. This observer claims requested layouts, not allocator RSS or native I/O.
#[allow(unsafe_code, reason = "isolated component allocator census")]
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: forward the original allocation contract unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: forward the original allocation contract unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        // SAFETY: forward original live allocation and requested replacement size unchanged.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: return the original live allocation to its allocator unchanged.
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
fn observe<T>(phase: &str, run: impl FnOnce() -> T) -> T {
    assert!(
        ACTIVE
            .with(|active| active.replace(Some(Census::EMPTY)))
            .is_none()
    );
    let guard = Observation;
    let result = run();
    let census = ACTIVE.with(|active| active.get().unwrap());
    drop(guard);
    println!(
        "{phase}: requests={} total-requested={} layouts={:?}",
        census.count,
        census.total_requested,
        &census.sizes[..census.count.min(census.sizes.len())]
    );
    assert!(
        census.count <= census.sizes.len(),
        "increase fixed census slots"
    );
    result
}

struct FundedWriter(ChargedBuffer<u8>);
impl Write for FundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: probe advance|check|check-present|check-metadata|large|multisig-a|multisig-b signed|entry count|write|funded|ordinary"
    );
    let profile = args[1].clone();
    let (signed, original, entry, original_entry) = std::thread::spawn(move || {
        let signed = support::signed(&profile);
        let original = signed.encode_wire_v1().unwrap();
        let entry = TransactionEntrypoint::External(signed.clone());
        let original_entry = entry.encode_wire_v1().unwrap();
        (signed, original, entry, original_entry)
    })
    .join()
    .unwrap();
    let (value, canonical): (&dyn norito::core::SerializePayload, &[u8]) = match args[2].as_str() {
        "signed" => (&signed, &original),
        "entry" => (&entry, &original_entry),
        _ => panic!("unknown wire shape"),
    };
    let budget = AllocationBudget::new(canonical.len()); // Fixture, not original State admission.
    println!(
        "profile={} shape={} phase={} output-bytes={}",
        args[1],
        args[2],
        args[3],
        canonical.len()
    );
    for phase in ["cold-encoding-thread", "warm-1", "warm-2"] {
        match args[3].as_str() {
            "count" => {
                let plan = observe(phase, || wire::WireV1Plan::new(value)).unwrap();
                assert_eq!(plan.wire_length(), canonical.len());
            }
            "write" => {
                // Count outside observation; write-only census excludes that pass explicitly.
                let plan = wire::WireV1Plan::new(value).unwrap();
                let mut destination =
                    FundedWriter(ChargedBuffer::new(plan.wire_length(), &budget).unwrap());
                observe(phase, || plan.write_to(&mut destination)).unwrap();
                assert_eq!(destination.0.as_slice(), canonical);
                drop(destination);
            }
            "funded" => {
                let destination = observe(phase, || {
                    let plan = wire::WireV1Plan::new(value).unwrap();
                    let mut destination =
                        FundedWriter(ChargedBuffer::new(plan.wire_length(), &budget).unwrap());
                    plan.write_to(&mut destination).unwrap();
                    destination
                });
                assert_eq!(destination.0.as_slice(), canonical);
                assert_eq!(budget.reserved_bytes(), canonical.len());
                assert!(destination.0.belongs_to(&budget));
                drop(destination);
            }
            "ordinary" => {
                let bytes =
                    observe(phase, || wire::WireV1Plan::new(value).unwrap().into_vec()).unwrap();
                assert_eq!(bytes, canonical);
            }
            _ => panic!("unknown operation"),
        }
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
