use ivm::IVM;

#[test]
fn memory_compact_bundle_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    let addr = ivm::Memory::HEAP_START + 128;
    vm.memory.store_u64(addr, 0xAA55_AA55_AA55_AA55).unwrap();
    vm.memory.commit();

    let bundle = ivm::merkle_utils::memory_compact_bundle(&mut vm.memory, addr, Some(16));
    let (cp, root) = vm.memory.merkle_compact(addr, Some(16));
    let cp2 = bundle.to_compact_proof();
    assert_eq!(bundle.depth, cp.depth());
    assert_eq!(bundle.dirs, cp.dirs());
    assert_eq!(bundle.root, *root.as_ref());
    assert_eq!(cp2.depth(), cp.depth());
    assert_eq!(cp2.dirs(), cp.dirs());

    // Also expand to full proof via bundle and verify
    use iroha_crypto::{Hash, HashOf};
    use sha2::Digest as _;
    let base = (addr / 32) * 32;
    let mut chunk = [0u8; 32];
    vm.memory.load_bytes(base, &mut chunk).unwrap();
    let mut leaf = [0u8; 32];
    leaf.copy_from_slice(&sha2::Sha256::digest(chunk));
    let leaf_t = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf));
    let root_t = root;
    let full = bundle.into_full_proof((addr / 32) as u32);
    let computed = full
        .compute_root_sha256(&leaf_t, 32)
        .expect("memory proof should produce a root");
    assert_eq!(computed, root_t, "memory compact bundle root mismatch");
}

#[test]
fn registers_compact_bundle_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(3, 0xDEADBEEF);

    let bundle = ivm::merkle_utils::registers_compact_bundle(&vm.registers, 3, Some(16));
    let (cp, root) = vm.registers.merkle_compact(3, Some(16));
    let cp2 = bundle.to_compact_proof();
    assert_eq!(bundle.depth, cp.depth());
    assert_eq!(bundle.dirs, cp.dirs());
    assert_eq!(bundle.root, *root.as_ref());
    assert_eq!(cp2.depth(), cp.depth());
    assert_eq!(cp2.dirs(), cp.dirs());

    // Also expand to full proof via bundle and verify (tag=false default)
    use iroha_crypto::{Hash, HashOf};
    use sha2::Digest as _;
    let val = vm.register(3);
    let mut bytes = [0u8; 9];
    bytes[0] = 0;
    bytes[1..].copy_from_slice(&val.to_le_bytes());
    let mut leaf = [0u8; 32];
    leaf.copy_from_slice(&sha2::Sha256::digest(bytes));
    let leaf_t = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf));
    let root_t = root;
    let full = bundle.into_full_proof(3);
    let computed = full
        .compute_root_sha256(&leaf_t, 32)
        .expect("register proof should produce a root");
    assert_eq!(computed, root_t, "register compact bundle root mismatch");
}
