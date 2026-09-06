use ivm::Memory;
#[test]
fn partial_commit_consistency() {
    let mut seq = Memory::new();
    let mut batch = Memory::new();
    let a1 = Memory::HEAP_START;
    let a2 = Memory::HEAP_START + 64;
    seq.store_u32(a1, 0xDEAD_BEEF).unwrap();
    seq.commit();
    seq.store_u32(a2, 0xCAFE_BABE).unwrap();
    seq.commit();
    let seq_root = seq.root();
    batch.store_u32(a1, 0xDEAD_BEEF).unwrap();
    batch.store_u32(a2, 0xCAFE_BABE).unwrap();
    batch.commit();
    let batch_root = batch.root();
    assert_eq!(seq_root, batch_root);
}
#[test]
fn dirty_range_tracking() {
    let mut mem = Memory::new();
    let a1 = Memory::HEAP_START;
    let a2 = Memory::HEAP_START + 64;
    mem.store_u32(a1, 1).unwrap();
    mem.store_u32(a2, 2).unwrap();
    let ranges = mem.dirty_ranges();
    assert_eq!(ranges.len(), 2);
    let committed_root = mem.root();
    assert!(mem.dirty_ranges().is_empty());

    let mut reference = Memory::new();
    reference.store_u32(a1, 1).unwrap();
    reference.store_u32(a2, 2).unwrap();
    assert_eq!(committed_root, reference.root());
}
