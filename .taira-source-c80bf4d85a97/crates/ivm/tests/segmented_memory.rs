use ivm::{VMError, segmented_memory::Memory};

#[test]
fn load_store_segments() {
    let mut mem = Memory::new();
    // store to stack
    let addr_stack = Memory::STACK_START;
    mem.store_u64(addr_stack, 0x1122_3344_5566_7788).unwrap();
    assert_eq!(mem.load_u64(addr_stack).unwrap(), 0x1122_3344_5566_7788);

    // store to heap
    let addr_heap = Memory::HEAP_START + 8;
    mem.store_u64(addr_heap, 0x99AA_BBCC_DDEE_FF00).unwrap();
    assert_eq!(mem.load_u64(addr_heap).unwrap(), 0x99AA_BBCC_DDEE_FF00);
}

#[test]
fn unaligned_access() {
    let mut mem = Memory::new();
    let res = mem.store_u64(Memory::STACK_START + 4, 1); // not 8-byte aligned
    assert!(matches!(res, Err(VMError::UnalignedAccess)));
}

#[test]
fn out_of_bounds() {
    let mem = Memory::new();
    let res = mem.load_u64(Memory::HEAP_START + Memory::SEGMENT_SIZE as u64);
    assert!(matches!(res, Err(VMError::MemoryOutOfBounds)));
}

#[test]
fn code_write_permission() {
    let mut mem = Memory::new();
    let res = mem.store_u64(Memory::CODE_START, 0xAA);
    assert!(matches!(res, Err(VMError::MemoryPermissionDenied)));
}

#[test]
fn endian_roundtrip() {
    let mut mem = Memory::new();
    let val = 0xDEADBEEF_AA55CCDDu64;
    mem.store_u64(Memory::STACK_START, val).unwrap();
    assert_eq!(mem.load_u64(Memory::STACK_START).unwrap(), val);
}
