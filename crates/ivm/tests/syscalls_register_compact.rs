use ivm::{IVM, syscalls};

mod common;
use common::assemble_syscalls;

#[test]
fn register_compact_proof_decodes_and_verifies() {
    use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
    use sha2::{Digest, Sha256};

    let mut vm = IVM::new(u64::MAX);
    // Modify a few registers (including tag) to create nontrivial tree
    vm.set_register(5, 123456789);
    vm.set_register(6, 42);
    // Simulate tag change via direct method (call VM internal API if accessible)
    // Here we keep only values and rely on default tag=false for simplicity.

    let out_ptr = ivm::Memory::OUTPUT_START;
    let root_out = ivm::Memory::OUTPUT_START + 8192;
    let idx = 6u64;
    vm.set_register(10, idx);
    vm.set_register(11, out_ptr);
    vm.set_register(12, 16);
    vm.set_register(13, root_out);

    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT as u8]);
    vm.load_program(&prog).expect("load");
    vm.run().expect("run");

    // Read and decode compact proof
    let mut hdr = [0u8; 1 + 4 + 4];
    vm.memory.load_bytes(out_ptr, &mut hdr).expect("hdr");
    let depth = hdr[0] as usize;
    let total = 1 + 4 + 4 + depth * 32;
    let mut buf = vec![0u8; total];
    vm.memory.load_bytes(out_ptr, &mut buf).expect("body");
    let (compact, used) = ivm::merkle_utils::decode_compact_proof_bytes(&buf).expect("decode");
    assert_eq!(used, total);

    // Build leaf digest for register idx: SHA256([tag(1)] + value(8))
    let val = vm.register(idx as usize);
    let tag = 0u8; // default tag=false
    let mut bytes = [0u8; 9];
    bytes[0] = tag;
    bytes[1..].copy_from_slice(&val.to_le_bytes());
    let mut leaf_bytes = [0u8; 32];
    leaf_bytes.copy_from_slice(&Sha256::digest(bytes));
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_bytes));

    // Read root
    let mut root_bytes = [0u8; 32];
    vm.memory
        .load_bytes(root_out, &mut root_bytes)
        .expect("root");
    let root = HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(root_bytes));

    assert!(compact.clone().verify_sha256(&leaf, &root));
    let full: MerkleProof<[u8; 32]> = compact.into_full_with_index(idx as u32);
    assert!(full.verify_sha256(&leaf, &root, 32));
}
