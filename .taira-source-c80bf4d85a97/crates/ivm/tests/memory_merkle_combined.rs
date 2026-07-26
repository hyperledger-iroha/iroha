use ivm::Memory;

#[test]
fn memory_root_and_path_combined_matches_separate() {
    let mut mem = Memory::new(0);
    // Write a pattern spanning multiple chunks
    let addr0 = Memory::HEAP_START;
    let addr1 = Memory::HEAP_START + 64;
    mem.store_u64(addr0, 0x11_22_33_44_55_66_77_88).unwrap();
    mem.store_u64(addr1, 0xAA_BB_CC_DD_EE_FF_00_99).unwrap();
    mem.commit();

    // Separate calls
    let p0 = mem.merkle_path(addr0);
    let r0 = mem.current_root();

    // Combined call
    let (rc, pc) = mem.merkle_root_and_path(addr0);

    assert_eq!(r0, rc);
    assert_eq!(p0, pc);
}
