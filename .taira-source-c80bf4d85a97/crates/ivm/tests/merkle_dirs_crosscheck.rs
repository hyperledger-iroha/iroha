//! Cross-check that Merkle compact proofs encode direction bits (`dirs`)
//! consistent with the full audit path + leaf index across Memory and
//! Registers trees. This validates independent derivations of `dirs`.

use ivm::IVM;

#[test]
fn memory_merkle_dirs_match_path() {
    let mut vm = IVM::new(u64::MAX);
    // Touch multiple, different chunks to exercise various leaf indices.
    let addrs = [
        ivm::Memory::HEAP_START,
        ivm::Memory::HEAP_START + 32,
        ivm::Memory::HEAP_START + 128,
        ivm::Memory::HEAP_START + 1024,
    ];

    // Write different values to force distinct leaf digests and commit once
    for (i, &addr) in addrs.iter().enumerate() {
        vm.memory
            .store_u64(addr, 0xA5A5_0000_0000_0000u64.wrapping_add(i as u64))
            .unwrap();
    }
    vm.memory.commit();

    for &addr in &addrs {
        let path = vm.memory.merkle_path(addr);
        let leaf_index = (addr / 32) as u32;
        let depth_cap = Some(16);

        // Derive compact proof two independent ways
        let (cp_mem, _root_mem) = vm.memory.merkle_compact(addr, depth_cap);
        let cp_from_path =
            ivm::merkle_utils::make_compact_from_path_bytes(&path, leaf_index, depth_cap);

        // Cross-check fields
        assert_eq!(cp_mem.depth(), cp_from_path.depth());
        assert_eq!(cp_mem.dirs(), cp_from_path.dirs());
        assert_eq!(cp_mem.siblings().len(), cp_from_path.siblings().len());
        let depth = cp_mem.depth() as usize;
        let mask = (1u64 << depth) - 1;
        assert_eq!(cp_mem.dirs() as u64, (leaf_index as u64) & mask);

        // Also verify the proof using SHA-256 semantics against the returned root
        // Already cross-checked fields; proof verification exercised elsewhere.
    }
}

#[test]
fn registers_merkle_dirs_match_path() {
    let mut vm = IVM::new(u64::MAX);
    // Set a few registers to nontrivial values
    vm.set_register(0, 0xDEAD_BEEF);
    vm.set_register(3, 0x0123_4567_89AB_CDEF);
    vm.set_register(7, 0xA5A5_A5A5_A5A5_A5A5);

    for &idx in &[0usize, 3, 7] {
        let path = vm.registers.merkle_path(idx);
        let depth_cap = Some(16);

        // Two independent derivations of compact proof
        let (cp_regs, _root_regs) = vm.registers.merkle_compact(idx, depth_cap);
        let cp_from_path =
            ivm::merkle_utils::make_compact_from_path_bytes(&path, idx as u32, depth_cap);

        eprintln!(
            "idx={idx} depth={} dirs=0x{:08x}",
            cp_from_path.depth(),
            cp_from_path.dirs()
        );

        assert_eq!(cp_regs.depth(), cp_from_path.depth());
        assert_eq!(cp_regs.dirs(), cp_from_path.dirs());
        assert_eq!(cp_regs.siblings().len(), cp_from_path.siblings().len());
        let depth = cp_regs.depth() as usize;
        let mask = (1u64 << depth) - 1;
        assert_eq!(cp_regs.dirs() as u64, (idx as u64) & mask);

        // Proof verification is covered by dedicated Merkle tests; here we only cross-check dirs.
    }
}
