use iroha_crypto::HashOf;
use ivm::{IVM, syscalls};

mod common;
use common::assemble_syscalls;

#[test]
fn registers_compact_helper_matches_syscall() {
    use iroha_crypto::MerkleProof;

    let target = 9usize;
    let mut vm = IVM::new(u64::MAX);
    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT as u8]);
    vm.load_program(&prog).unwrap();

    vm.set_register(target, 0xCAFEBABE);
    vm.set_register(5, 0x12345678);

    // Via syscall
    let out_ptr = ivm::Memory::OUTPUT_START;
    let root_out = ivm::Memory::OUTPUT_START + 4096;
    vm.set_register(10, target as u64);
    vm.set_register(11, out_ptr);
    vm.set_register(12, 16);
    vm.set_register(13, root_out);
    let (proof_h, root_h) = vm.registers.merkle_compact(target, Some(16));
    vm.run().unwrap();

    // Decode syscall output
    let mut hdr = [0u8; 1 + 4 + 4];
    vm.memory.load_bytes(out_ptr, &mut hdr).unwrap();
    let depth = hdr[0] as usize;
    let total = 1 + 4 + 4 + depth * 32;
    let mut buf = vec![0u8; total];
    vm.memory.load_bytes(out_ptr, &mut buf).unwrap();
    let (proof_s, _) = ivm::merkle_utils::decode_compact_proof_bytes(&buf).unwrap();
    let mut root_s = [0u8; 32];
    vm.memory.load_bytes(root_out, &mut root_s).unwrap();

    assert_eq!(proof_h.depth(), proof_s.depth());
    assert_eq!(proof_h.dirs(), proof_s.dirs());
    assert_eq!(root_h.as_ref(), &root_s);

    // Verify leaf digest for target register (default tag=false)
    let val = vm.register(target);
    let leaf = register_leaf_digest(val, false);
    let root_hash = root_h;

    assert!(proof_h.clone().verify_sha256(&leaf, &root_hash));
    assert!(proof_s.clone().verify_sha256(&leaf, &root_hash));
    let full_h: MerkleProof<[u8; 32]> = proof_h.into_full_with_index(target as u32);
    let full_s: MerkleProof<[u8; 32]> = proof_s.into_full_with_index(target as u32);
    assert!(full_h.verify_sha256(&leaf, &root_hash, 32));
    assert!(full_s.verify_sha256(&leaf, &root_hash, 32));
}

#[test]
fn registers_compact_depth_cap_adjusts_root() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(3, 0xDEADBEEFCAFEBABE);
    vm.set_register(12, 0xBADC0FFEE0DDFACE);

    let (full_proof, full_root) = vm.registers.merkle_compact(12, None);
    let leaf = register_leaf_digest(vm.register(12), false);
    let full_root_hash = full_root;
    assert!(full_proof.clone().verify_sha256(&leaf, &full_root_hash));

    let (capped_proof, capped_root) = vm.registers.merkle_compact(12, Some(8));
    let capped_root_hash = capped_root;
    assert!(capped_proof.clone().verify_sha256(&leaf, &capped_root_hash));

    if capped_root != full_root {
        assert!(!capped_proof.verify_sha256(&leaf, &full_root_hash));
    }
}

fn register_leaf_digest(value: u64, private: bool) -> HashOf<[u8; 32]> {
    use iroha_crypto::{Hash, HashOf};
    use sha2::Digest as _;

    let mut bytes = [0u8; 9];
    bytes[0] = if private { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&sha2::Sha256::digest(bytes));
    HashOf::from_untyped_unchecked(Hash::prehashed(digest))
}
