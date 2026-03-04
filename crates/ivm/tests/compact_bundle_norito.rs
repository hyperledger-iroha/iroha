use ivm::IVM;

#[test]
fn compact_bundle_norito_roundtrip_memory_and_registers() {
    // Memory bundle roundtrip
    let mut vm = IVM::new(u64::MAX);
    let addr = ivm::Memory::HEAP_START + 64;
    vm.memory.store_u32(addr, 0xAABBCCDD).unwrap();
    vm.memory.commit();
    let b1 = ivm::merkle_utils::memory_compact_bundle(&mut vm.memory, addr, Some(8));
    let bytes = norito::to_bytes(&b1).expect("encode bundle");
    let d1: ivm::merkle_utils::CompactProofBundle =
        norito::decode_from_bytes(&bytes).expect("decode bundle");
    assert_eq!(b1, d1);

    // Registers bundle roundtrip
    vm.set_register(7, 0x0102_0304_0506_0708);
    let b2 = ivm::merkle_utils::registers_compact_bundle(&vm.registers, 7, Some(8));
    let bytes2 = norito::to_bytes(&b2).expect("encode bundle");
    let d2: ivm::merkle_utils::CompactProofBundle =
        norito::decode_from_bytes(&bytes2).expect("decode bundle");
    assert_eq!(b2, d2);
}
