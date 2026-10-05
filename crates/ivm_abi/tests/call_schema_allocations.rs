//! Actual allocator refusal and borrowed writer controls for complete CS1 tapes.

use ivm_abi::call::{CallSchemaV1, CallTypeNodeV1};
use norito::core::{DecodeFlagsGuard, DecodeFromSlice, DecodeLimits, Encoder, SerializePayload};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    io::Cursor,
};

struct Observer;
thread_local! {
    static COUNT:Cell<Option<usize>>=const {Cell::new(None)};
    static REFUSE:Cell<Option<Layout>>=const {Cell::new(None)};
}
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        if REFUSE
            .try_with(|slot| {
                if slot.get() == Some(layout) {
                    slot.set(None);
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false)
        {
            return std::ptr::null_mut();
        }
        // SAFETY: forwards the original allocation layout unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: returns the original pointer with its original layout.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = COUNT.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        // SAFETY: preserves the original live allocation and requested new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;
fn observe<T>(body: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNT.with(|n| n.set(None));
            REFUSE.with(|n| n.set(None));
        }
    }
    COUNT.with(|count| assert!(count.replace(Some(0)).is_none()));
    let reset = Reset;
    let value = body();
    let count = COUNT.with(|n| n.replace(None).unwrap());
    drop(reset);
    (value, count)
}
fn payload(schema: &CallSchemaV1) -> Vec<u8> {
    let _flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut bytes = Vec::new();
    schema.serialize(&mut Encoder::new(&mut bytes)).unwrap();
    bytes
}
#[test]
fn complete_nominal_borrowed_write_and_length_observation_allocate_no_wire_graph() {
    let schema = CallSchemaV1 {
        nodes: vec![
            CallTypeNodeV1::Struct {
                name: "Point".into(),
                fields: vec!["left".into(), "right".into()],
            },
            CallTypeNodeV1::Leaf(ivm_abi::entrypoint::EntrypointValueKindV1::String),
            CallTypeNodeV1::Error(ivm_abi::error_types::numeric_error_type()),
        ],
    };
    let expected = payload(&schema);
    let mut backing = vec![0u8; expected.len()];
    let _flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    // Warm the original Norito counting state before observing the borrowed operation.
    assert_eq!(
        norito::core::encoded_payload_len(&schema).unwrap(),
        expected.len()
    );
    let (result, count) = observe(|| {
        let mut cursor = Cursor::new(backing.as_mut_slice());
        schema.serialize(&mut Encoder::new(&mut cursor))?;
        assert_eq!(cursor.position(), expected.len() as u64);
        Ok::<_, norito::Error>((
            norito::core::encoded_payload_len(&schema)?,
            schema.encoded_len_hint(),
            schema.encoded_len_exact(),
        ))
    });
    assert_eq!(result.unwrap().0, expected.len());
    assert_eq!(count, 0);
    assert_eq!(backing, expected);
}
#[test]
fn zero_logical_allocation_budget_refuses_before_the_node_backing_allocation() {
    let bytes = payload(&CallSchemaV1 {
        nodes: vec![CallTypeNodeV1::Unit; 3],
    });
    let _flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let required = 3 + 3 * std::mem::size_of::<CallTypeNodeV1>();
    let (result, count) = norito::core::with_decode_limits_scope(
        DecodeLimits::new(100, 100, 100, required - 1, 256),
        || observe(|| CallSchemaV1::decode_from_slice(&bytes)),
    );
    assert!(
        matches!(result,Err(norito::Error::TotalAllocationExceeded{attempted,limit}) if attempted==required as u64&&limit==(required-1) as u64)
    );
    assert_eq!(
        count, 0,
        "original resource refusal precedes actual Vec backing allocation"
    );
    assert_eq!(
        CallSchemaV1::decode_from_slice(&bytes)
            .unwrap()
            .0
            .nodes
            .len(),
        3
    );
}
#[test]
fn actual_node_allocator_refusal_is_distinct_and_the_original_decode_retries() {
    let schema = CallSchemaV1 {
        nodes: vec![CallTypeNodeV1::Unit; 3],
    };
    let bytes = payload(&schema);
    let _flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let layout = Layout::array::<CallTypeNodeV1>(3).unwrap();
    let (result, count) = norito::core::with_decode_limits_scope(
        DecodeLimits::new(100, 100, 100, 1 << 20, 256),
        || {
            REFUSE.with(|slot| slot.set(Some(layout)));
            observe(|| CallSchemaV1::decode_from_slice(&bytes))
        },
    );
    assert!(
        matches!(result,Err(norito::Error::AllocationFailed{bytes}) if bytes==layout.size() as u64)
    );
    assert_eq!(count, 1, "refuses the genuine admitted node allocation");
    assert_eq!(CallSchemaV1::decode_from_slice(&bytes).unwrap().0, schema);
}
