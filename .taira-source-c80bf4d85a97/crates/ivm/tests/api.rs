//! API surface tests for IVM convenience wrappers.

use ivm::Memory;

#[test]
fn load_u64_wrapper_matches_memory() {
    let mut vm = ivm::IVM::new(1_000_000);
    let addr = Memory::HEAP_START;
    let val: u64 = 0xDEAD_BEEF_F00D_CAFE;
    vm.store_u64(addr, val).unwrap();
    let via_vm = vm.load_u64(addr).unwrap();
    let via_mem = vm.memory.load_u64(addr).unwrap();
    assert_eq!(via_vm, val);
    assert_eq!(via_vm, via_mem);
}
