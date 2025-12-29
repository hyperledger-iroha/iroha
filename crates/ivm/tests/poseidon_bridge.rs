//! Ensure the Poseidon compression used by Merkle gadgets equals the internal
//! `poseidon2` permutation for a single level (both child orderings).

#[test]
fn poseidon_merkle_level_matches_internal() {
    let samples: &[(u64, u64)] = &[
        (0, 0),
        (1, 2),
        (5, 7),
        (u64::MAX, 123456789),
        (0xDEAD_BEEF_DEAD_BEEF, 0x0123_4567_89AB_CDEF),
    ];
    for &(leaf, sib) in samples {
        // Depth=1, index LSB=0 => acc is left child
        let r0 = ivm::halo2::verify_merkle_path_depth(leaf, 0, &[sib; 32], 1);
        let e0 = ivm::poseidon2(leaf, sib);
        assert_eq!(r0, e0, "poseidon(left, right) mismatch");

        // Depth=1, index LSB=1 => acc is right child
        let r1 = ivm::halo2::verify_merkle_path_depth(leaf, 1, &[sib; 32], 1);
        let e1 = ivm::poseidon2(sib, leaf);
        assert_eq!(r1, e1, "poseidon(right, left) mismatch");
    }
}

#[test]
fn bridge_matches_internal_poseidon2() {
    let mut vectors = Vec::new();
    vectors.extend([
        (0u64, 0u64),
        (1, 2),
        (u64::MAX, 42),
        (0xDEAD_BEEF_DEAD_BEEF, 0x0123_4567_89AB_CDEF),
    ]);
    for seed in 1u64..8 {
        vectors.push((seed * 17, seed * 33 + 5));
    }
    for (a, b) in vectors {
        let bridge64 = ivm::pair_hash_u64(a, b);
        let internal64 = ivm::poseidon2(a, b);
        assert_eq!(bridge64, internal64, "low64 mismatch for ({a}, {b})");

        let bridge_bytes = ivm::pair_hash_bytes(a, b);
        assert_eq!(
            &bridge_bytes[..8],
            &bridge64.to_le_bytes(),
            "byte prefix mismatch for ({a}, {b})"
        );
    }
}

#[test]
fn bridge_matches_internal_poseidon6() {
    let mut cases = vec![
        [0u64, 0, 0, 0, 0, 0],
        [1, 2, 3, 4, 5, 6],
        [u64::MAX, 42, 17, 9, 5, 1],
    ];
    for seed in 0u64..8 {
        cases.push([
            seed,
            seed.wrapping_mul(5).wrapping_add(3),
            seed.wrapping_mul(7).wrapping_add(11),
            seed ^ 0xDEAD_BEEF,
            seed.rotate_left(17),
            seed.rotate_right(9),
        ]);
    }
    for inputs in cases {
        let bridge64 = iroha_zkp_halo2::poseidon::hash6_u64(inputs);
        let internal64 = ivm::poseidon6(inputs);
        assert_eq!(
            bridge64, internal64,
            "poseidon6 low64 mismatch for inputs={inputs:?}"
        );
        let bridge_bytes = iroha_zkp_halo2::poseidon::hash6_bytes(inputs);
        assert_eq!(
            &bridge_bytes[..8],
            &bridge64.to_le_bytes(),
            "poseidon6 byte prefix mismatch for inputs={inputs:?}"
        );
    }
}
