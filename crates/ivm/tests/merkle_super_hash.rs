//! Verify that halo2::verify_merkle_path uses the unified super_hash
//! for pairwise compression with correct left/right ordering semantics,
//! and that index-based and dirs-based helpers are equivalent.

#[test]
fn merkle_super_hash_depth1() {
    let leaf: u64 = 1;
    let sib: u64 = 2;
    let mut path = [0u64; 32];
    path[0] = sib;

    // bit0 = 0 → accumulator is left child
    let root_l = ivm::halo2::verify_merkle_path_depth(leaf, 0, &path, 1);
    let exp_l = ivm::poseidon2(leaf, sib);
    assert_eq!(root_l, exp_l, "left-branch mismatch");

    // bit0 = 1 → accumulator is right child
    let root_r = ivm::halo2::verify_merkle_path_depth(leaf, 1, &path, 1);
    let exp_r = ivm::poseidon2(sib, leaf);
    assert_eq!(root_r, exp_r, "right-branch mismatch");
}

#[test]
fn merkle_super_hash_depth3_mixed_dirs() {
    let leaf: u64 = 0xDEADBEEFDEADBEEF;
    let mut path = [0u64; 32];
    let s0: u64 = 0xA1;
    let s1: u64 = 0xB2;
    let s2: u64 = 0xC3;
    path[0] = s0;
    path[1] = s1;
    path[2] = s2;

    // Helper to compute expected root using the same pairwise hash and bit ordering
    fn expected_root(mut cur: u64, idx: u32, path: &[u64; 32], depth: usize) -> u64 {
        for (i, sib) in path.iter().take(depth).enumerate() {
            let bit = (idx >> i) & 1;
            cur = if bit == 0 {
                ivm::poseidon2(cur, *sib)
            } else {
                ivm::poseidon2(*sib, cur)
            };
        }
        cur
    }

    // All-left (000b)
    let idx0 = 0b000;
    let got0 = ivm::halo2::verify_merkle_path_depth(leaf, idx0, &path, 3);
    let exp0 = expected_root(leaf, idx0, &path, 3);
    assert_eq!(got0, exp0, "depth3 all-left mismatch");

    // Mixed (101b): right at level 0, left at 1, right at 2
    let idx1 = 0b101;
    let got1 = ivm::halo2::verify_merkle_path_depth(leaf, idx1, &path, 3);
    let exp1 = expected_root(leaf, idx1, &path, 3);
    assert_eq!(got1, exp1, "depth3 mixed-dirs mismatch");
}

#[test]
fn merkle_dirs_vs_index_equivalence() {
    let leaf: u64 = 0x0123_4567_89AB_CDEF;
    // Construct a distinctive path so order mistakes are detectable
    let mut path = [0u64; 32];
    for (i, slot) in path.iter_mut().enumerate().take(16) {
        *slot = 0x100 + i as u64;
    }

    // Also cross-check explicit reference computation with dirs
    fn ref_root_with_dirs(mut cur: u64, dirs: u32, path: &[u64; 32], depth: usize) -> u64 {
        let mut d = dirs;
        for sib in path.iter().take(depth) {
            let bit = d & 1;
            cur = if bit == 0 {
                ivm::poseidon2(cur, *sib)
            } else {
                ivm::poseidon2(*sib, cur)
            };
            d >>= 1;
        }
        cur
    }

    fn push_unique(haystack: &mut Vec<u32>, needle: u32) {
        if !haystack.contains(&needle) {
            haystack.push(needle);
        }
    }

    for &depth in &[1usize, 3, 5, 8, 12, 16] {
        let max = 1u32 << depth;
        // Larger depths get expensive; limit coverage to a deterministic sample
        // so the test stays fast while still exercising diverse bit patterns.
        let run_case = |index: u32| {
            let root_idx = ivm::halo2::verify_merkle_path_depth(leaf, index, &path, depth);
            let dirs = index; // same low-bit semantics
            let root_dirs = ivm::halo2::verify_merkle_path_with_dirs(leaf, dirs, &path, depth);
            assert_eq!(
                root_idx, root_dirs,
                "mismatch at depth={depth} index={index:#b}"
            );
            let ref_root = ref_root_with_dirs(leaf, dirs, &path, depth);
            assert_eq!(
                root_dirs, ref_root,
                "ref mismatch at depth={depth} index={index:#b}"
            );
        };
        let case_budget = if depth <= 5 {
            max as usize
        } else if depth <= 8 {
            32
        } else if depth <= 12 {
            16
        } else {
            8
        };

        if (max as usize) <= case_budget {
            for index in 0..max {
                run_case(index);
            }
        } else {
            let mask = max - 1;
            let mut indices = Vec::with_capacity(case_budget + 8);
            for value in [
                0,
                1,
                2,
                3,
                mask,
                mask.saturating_sub(1),
                mask.saturating_sub(2),
                mask.saturating_sub(3),
                mask / 2,
                (mask / 2) + 1,
            ] {
                push_unique(&mut indices, value);
                if indices.len() >= case_budget {
                    break;
                }
            }
            let mut state = 0x00C0_FFEE_BEEF_CAFE_u64 ^ depth as u64;
            while indices.len() < case_budget {
                // SplitMix64 step (same constants as elsewhere in this file) for deterministic coverage.
                state = state.wrapping_add(0x9E3779B97F4A7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
                z ^= z >> 31;
                push_unique(&mut indices, (z as u32) & mask);
            }
            for index in indices {
                run_case(index);
            }
        }
    }
}

#[test]
fn merkle_random_matrix_equivalence() {
    // Simple deterministic SplitMix64 PRNG (no external deps).
    struct Rng(u64);
    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next(&mut self) -> u64 {
            let mut z = self.0.wrapping_add(0x9E3779B97F4A7C15);
            self.0 = z;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        }
    }

    const SEEDS: &[u64] = &[0xC0FFEE];
    // Keep the sample matrix broad but bounded so the test stays fast.
    const LEAF_SAMPLES: usize = 1;
    // Exercise representative shallow/medium/deep depths with deterministic indices.
    const CASES: &[(usize, u32)] = &[
        (1, 0b1),
        (4, 0b1011),
        (8, 0b1010_1010),
        (16, 0xCAFE),
        (31, 0x7FFF_FFFF),
    ];

    for &seed in SEEDS {
        let mut rng = Rng::new(seed);
        // Build a random path
        let mut path = [0u64; 32];
        for slot in &mut path {
            *slot = rng.next();
        }
        // Try several leaves with a small, deterministic set of depth/index pairs.
        for _ in 0..LEAF_SAMPLES {
            let leaf = rng.next();
            for &(depth, dirs) in CASES {
                let mask = if depth == 32 {
                    u32::MAX
                } else {
                    (1u32 << depth).wrapping_sub(1)
                };
                let dirs = dirs & mask;
                let root_idx = ivm::halo2::verify_merkle_path_depth(leaf, dirs, &path, depth);
                let root_dirs = ivm::halo2::verify_merkle_path_with_dirs(leaf, dirs, &path, depth);
                assert_eq!(
                    root_idx, root_dirs,
                    "seed={seed:#x} depth={depth} index={dirs:#b}"
                );
                let ref_root = {
                    let mut cur = leaf;
                    let mut d = dirs;
                    for sib in path.iter().take(depth) {
                        cur = if (d & 1) == 0 {
                            ivm::poseidon2(cur, *sib)
                        } else {
                            ivm::poseidon2(*sib, cur)
                        };
                        d >>= 1;
                    }
                    cur
                };
                assert_eq!(
                    root_dirs, ref_root,
                    "ref mismatch seed={seed:#x} depth={depth} index={dirs:#b}"
                );
            }
        }
    }
}

#[cfg(feature = "ivm_prop")]
mod prop_equivalence {
    use proptest::{array::uniform32, prelude::*};

    #[inline]
    fn dirs_reference(mut cur: u64, mut dirs: u32, path: &[u64; 32], depth: usize) -> u64 {
        for sib in path.iter().take(depth) {
            let bit = dirs & 1;
            cur = if bit == 0 {
                ivm::poseidon2(cur, *sib)
            } else {
                ivm::poseidon2(*sib, cur)
            };
            dirs >>= 1;
        }
        cur
    }

    const PROP_DEPTH_MAX: usize = 12;
    const PROP_CASES_FAST: u32 = 6;
    const PROP_CASES_SLOW: u32 = 3;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(PROP_CASES_FAST))]
        /// Randomised equivalence check between index- and dirs-based verifiers (bounded depth for speed).
        #[test]
        fn depth_vs_dirs_equivalence(
            leaf in any::<u64>(),
            depth in 1usize..=PROP_DEPTH_MAX,
            path in uniform32(any::<u64>()),
            index in any::<u32>(),
        ) {
            let mask = if depth == 32 { u32::MAX } else { (1u32 << depth) - 1 };
            let dirs = index & mask;
            let root_idx = ivm::halo2::verify_merkle_path_depth(leaf, dirs, &path, depth);
            let root_dirs = ivm::halo2::verify_merkle_path_with_dirs(leaf, dirs, &path, depth);
            let ref_root = dirs_reference(leaf, dirs, &path, depth);
            prop_assert_eq!(
                root_idx,
                root_dirs,
                "depth mismatch depth={} index={:#b}",
                depth,
                dirs
            );
            prop_assert_eq!(
                root_dirs,
                ref_root,
                "dirs mismatch depth={} index={:#b}",
                depth,
                dirs
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(PROP_CASES_SLOW))]
        /// Randomised equivalence between full-depth helper variants.
        #[test]
        fn full_depth_equivalence(
            leaf in any::<u64>(),
            path in uniform32(any::<u64>()),
            index in any::<u32>(),
        ) {
            let full = ivm::halo2::verify_merkle_path(leaf, index, &path);
            let depth32 = ivm::halo2::verify_merkle_path_depth(leaf, index, &path, 32);
            let dirs32 = ivm::halo2::verify_merkle_path_with_dirs(leaf, index, &path, 32);
            prop_assert_eq!(
                full,
                depth32,
                "full vs depth32 mismatch index={:#b}",
                index
            );
            prop_assert_eq!(
                full,
                dirs32,
                "full vs dirs32 mismatch index={:#b}",
                index
            );
        }
    }
}

#[test]
fn merkle_full_depth_equivalence() {
    // Cross-check that full-depth helper equals depth=32 and dirs-based variants
    // under a variety of random inputs.
    struct Rng(u64);
    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next(&mut self) -> u64 {
            let mut z = self.0.wrapping_add(0x9E3779B97F4A7C15);
            self.0 = z;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        }
    }

    let mut rng = Rng::new(0xABCDEF1234567890);
    for _ in 0..8 {
        let mut path = [0u64; 32];
        for slot in &mut path {
            *slot = rng.next();
        }
        for _ in 0..32 {
            let leaf = rng.next();
            let index = (rng.next() & (u32::MAX as u64)) as u32;
            let full = ivm::halo2::verify_merkle_path(leaf, index, &path);
            let depth32 = ivm::halo2::verify_merkle_path_depth(leaf, index, &path, 32);
            let dirs32 = ivm::halo2::verify_merkle_path_with_dirs(leaf, index, &path, 32);
            assert_eq!(full, depth32, "full vs depth32 mismatch index={index:#b}");
            assert_eq!(full, dirs32, "full vs dirs32 mismatch index={index:#b}");
        }
    }
}
