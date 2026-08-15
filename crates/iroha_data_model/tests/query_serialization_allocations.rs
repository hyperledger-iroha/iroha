//! Allocation regressions for borrowed query predicates and erased-query framing.
// This isolated integration test is the narrow exception that needs `GlobalAlloc`
// to observe steady-state heap traffic in the production serialization paths.
#![allow(unsafe_code)]
use iroha_data_model::{
    domain::Domain,
    query::{
        CommittedTransaction, ErasedIterQuery, QueryBox, QueryOutputBatchBox,
        dsl::{CommittedTxPredicate, CompoundPredicate, SelectorTuple},
    },
    role::RoleId,
};
use iroha_primitives::json::Json;
use norito::core::NoritoSerialize;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};
struct CountingAllocator;
thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout are forwarded unchanged to the allocator that created it.
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn serialize_into(value: &dyn NoritoSerialize, output: &mut Vec<u8>) {
    output.clear();
    let mut encoder = norito::core::Encoder::for_buffer(output);
    value.serialize(&mut encoder).expect("serialize value");
}
#[test]
fn predicate_and_query_box_sizing_and_streaming_do_not_clone_large_payloads() {
    let filtered = CompoundPredicate::<RoleId>::build(|prototype| {
        prototype.exists("predicate-path-".repeat(64 * 1024))
    });
    let filtered_clone = filtered.clone();
    let filtered_len = filtered
        .encoded_len_exact()
        .expect("JSON predicate has an exact wire length");
    let mut filtered_output = Vec::with_capacity(filtered_len);
    let mut tree_nodes: Vec<_> = (0_u64..1_000).map(CommittedTxPredicate::TsGte).collect();
    tree_nodes.push(CommittedTxPredicate::MetadataEq {
        key: "large_metadata".parse().expect("metadata key"),
        value: Json::new("M".repeat(512 * 1024)),
    });
    let tree = CompoundPredicate::<CommittedTransaction>::from_committed_tx_predicate(
        CommittedTxPredicate::And(tree_nodes),
    );
    let tree_clone = tree.clone();
    let tree_len = tree
        .encoded_len_exact()
        .expect("typed predicate has an exact streamed wire length");
    let mut tree_output = Vec::with_capacity(tree_len);
    let query: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Domain>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        vec![0xA5; 1024 * 1024],
    ));
    let query_len = query
        .encoded_len_exact()
        .expect("erased iterable query has an exact streamed wire length");
    let mut query_output = Vec::with_capacity(query_len);
    // Warm all registry and thread-local codec state before measuring the steady-state paths.
    serialize_into(&filtered, &mut filtered_output);
    serialize_into(&tree, &mut tree_output);
    serialize_into(&query, &mut query_output);
    filtered_output.clear();
    tree_output.clear();
    query_output.clear();
    ALLOCATIONS.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let filtered_is_pass = black_box(filtered.is_pass());
    let filtered_equal = black_box(filtered == filtered_clone);
    let filtered_exact = black_box(filtered.encoded_len_exact());
    serialize_into(&filtered, &mut filtered_output);
    let tree_equal = black_box(tree == tree_clone);
    let tree_exact = black_box(tree.encoded_len_exact());
    serialize_into(&tree, &mut tree_output);
    let query_exact = black_box(query.encoded_len_exact());
    serialize_into(&query, &mut query_output);
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
    assert!(!filtered_is_pass);
    assert!(filtered_equal);
    assert_eq!(filtered_exact, Some(filtered_len));
    assert_eq!(filtered_output.len(), filtered_len);
    assert!(tree_equal);
    assert_eq!(tree_exact, Some(tree_len));
    assert_eq!(tree_output.len(), tree_len);
    assert_eq!(query_exact, Some(query_len));
    assert_eq!(query_output.len(), query_len);
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
