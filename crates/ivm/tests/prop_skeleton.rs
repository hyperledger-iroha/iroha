#![cfg(feature = "ivm_prop")]
//! Property-based testing skeleton for IVM (dev-only).
//!
//! This file is compiled only when the `ivm_prop` feature is enabled.
//! The feature gates the optional `proptest` dependency so the workspace stays
//! lean in default builds. Run `cargo test -p ivm --features ivm_prop` locally to
//! exercise the property suite.

#[cfg(feature = "ivm_prop")]
mod prop {
    use iroha_crypto::{Hash, HashOf};
    use ivm::Memory;
    use proptest::prelude::*;
    use sha2::Digest as _;

    const CHUNK_BYTES: usize = 32;
    const MAX_INPUT_CHUNKS: usize = (Memory::INPUT_SIZE as usize) / CHUNK_BYTES;

    fn leaf_hash(bytes: &[u8; CHUNK_BYTES]) -> HashOf<[u8; CHUNK_BYTES]> {
        let digest = sha2::Sha256::digest(bytes);
        let mut arr = [0u8; CHUNK_BYTES];
        arr.copy_from_slice(&digest);
        HashOf::from_untyped_unchecked(Hash::prehashed(arr))
    }

    proptest! {
        #[test]
        fn memory_merkle_compact_proofs_verify(
            chunk_idx in 0usize..MAX_INPUT_CHUNKS,
            data in proptest::array::uniform32(any::<u8>()),
            depth_cap in proptest::option::of(0usize..=10),
        ) {
            let mut memory = Memory::new(0);
            let offset = (chunk_idx * CHUNK_BYTES) as u64;
            memory.preload_input(offset, &data).expect("preload input chunk");
            memory.commit();

            let addr = Memory::INPUT_START + offset;
            let (proof, root) = memory.merkle_compact(addr, depth_cap);
            let leaf = leaf_hash(&data);

            prop_assert!(proof.clone().verify_sha256(&leaf, &root));

            let mut tampered = data;
            tampered[0] ^= 0x01;
            let tampered_leaf = leaf_hash(&tampered);
            prop_assert!(!proof.verify_sha256(&tampered_leaf, &root));
        }
    }
}
