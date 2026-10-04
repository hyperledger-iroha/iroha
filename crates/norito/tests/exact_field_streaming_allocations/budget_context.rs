//! Actual lazy context-clone allocation census after payload scratch is prepared.

use super::allocations_during;
use norito::{DecodeLimits, StreamMapIter, StreamSeqIter};
use std::{collections::BTreeMap, io::Cursor};

fn limits() -> DecodeLimits {
    DecodeLimits::new(64, 1024, 256, 64 * 1024, 16)
}

#[test]
fn bounded_sequence_next_clones_its_original_context_without_allocation() {
    let bytes = norito::to_bytes(&vec![7_u64, 11, 13]).unwrap();
    let mut iterator = StreamSeqIter::<u64>::new_with_limits(Cursor::new(bytes), limits()).unwrap();
    assert_eq!(iterator.next().unwrap().unwrap(), 7);
    // Prepare actual scalar decode scratch and TLS capacity above; this tests
    // per-next context cloning, not scope construction or payload funding.
    let mut result = None;
    let allocations = allocations_during(|| result = Some(iterator.next()));
    assert_eq!(allocations, 0);
    assert_eq!(result.unwrap().unwrap().unwrap(), 11);
    assert_eq!(iterator.next().unwrap().unwrap(), 13);
    assert!(iterator.next().is_none());
}

#[test]
fn bounded_map_next_clones_its_original_context_without_allocation() {
    let bytes = norito::to_bytes(&BTreeMap::from([(7_u64, 11_u64), (13, 17), (19, 23)])).unwrap();
    let mut iterator =
        StreamMapIter::<u64, u64>::new_btree_with_limits(Cursor::new(bytes), limits()).unwrap();
    assert_eq!(iterator.next().unwrap().unwrap(), (7, 11));
    let mut result = None;
    let allocations = allocations_during(|| result = Some(iterator.next()));
    assert_eq!(allocations, 0);
    assert_eq!(result.unwrap().unwrap().unwrap(), (13, 17));
    assert_eq!(iterator.next().unwrap().unwrap(), (19, 23));
    assert!(iterator.next().is_none());
}
