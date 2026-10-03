//! Shared parallel CPU leaf traversal with explicit caller policy and receipts.

use super::{ByteMerkleTree, CanonicalNodes, Mutex, VMError, sha256_oneblock32_in_context};
use crate::vector::{Sha256Context, Sha256Observed};
use rayon::prelude::*;

/// A complete fixed leaf, including zero padding and the canonical zero shortcut.
pub(super) fn digest_leaf(
    data: &[u8],
    index: usize,
    chunk: usize,
    zero_hash: [u8; 32],
    context: Sha256Context,
) -> ([u8; 32], Sha256Observed) {
    let start = index.saturating_mul(chunk);
    let end = start.saturating_add(chunk).min(data.len());
    let mut bytes = [0; 32];
    if start < end {
        bytes[..end - start].copy_from_slice(&data[start..end]);
    }
    if bytes[..chunk].iter().all(|&byte| byte == 0) {
        (zero_hash, Sha256Observed::default())
    } else {
        sha256_oneblock32_in_context(&bytes[..chunk], context)
    }
}

impl ByteMerkleTree {
    pub(super) fn from_bytes_hashed(
        data: &[u8],
        chunk: usize,
        parallel: bool,
    ) -> Result<Self, VMError> {
        Self::from_bytes_in_context(data, chunk, parallel, Sha256Context::production())
            .map(|(tree, _)| tree)
    }

    /// The ordinary parallel builder and its original explicit CPU context.
    /// Calibration changes only receipt routing and retains its returned tree.
    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    pub(crate) fn from_bytes_parallel_in_context(
        data: &[u8],
        chunk: usize,
        context: Sha256Context,
    ) -> Result<(Self, Sha256Observed), VMError> {
        Self::from_bytes_in_context(data, chunk, true, context)
    }

    fn from_bytes_in_context(
        data: &[u8],
        chunk: usize,
        parallel: bool,
        context: Sha256Context,
    ) -> Result<(Self, Sha256Observed), VMError> {
        Self::validate_chunk_size(chunk)?;
        let zero_hash = Self::compute_zero_hash(chunk);
        let mut leaves = crate::cache_memory::OwnedAllocation::try_filled_copy(
            data.len().div_ceil(chunk).max(1),
            zero_hash,
        )?;
        let hash = |(index, leaf): (usize, &mut [u8; 32])| {
            let (digest, observed) = digest_leaf(data, index, chunk, zero_hash, context);
            *leaf = digest;
            observed
        };
        let observed = if parallel {
            leaves
                .par_iter_mut()
                .enumerate()
                .map(hash)
                .reduce(Sha256Observed::default, Sha256Observed::merge)
        } else {
            leaves
                .iter_mut()
                .enumerate()
                .map(hash)
                .fold(Sha256Observed::default(), Sha256Observed::merge)
        };
        // Ordinary output owners do not expose this observation, but their
        // traversal must pay the same reduction as measured construction.
        let observed = std::hint::black_box(observed);
        let nodes = CanonicalNodes::from_leaves(&leaves, None)?;
        Ok((
            Self {
                chunk,
                zero_hash,
                leaves: Mutex::new(leaves.into()),
                nodes: Mutex::new(nodes),
            },
            observed,
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Canonical empty/tail/zero leaves and the actual ordinary parallel producer.

    use super::*;
    use crate::vector::{Sha256Backend, SimdChoice};
    use sha2::{Digest, Sha256};

    fn scalar_context() -> Sha256Context {
        let previous = crate::vector::set_thread_forced_simd(Some(SimdChoice::Scalar));
        let context = Sha256Context::production();
        crate::vector::set_thread_forced_simd(previous);
        context
    }

    #[test]
    fn leaf_context_preserves_zero_tail_and_missing_fixed_leaf_semantics() {
        let context = scalar_context();
        for chunk in [1, 7, 17, 32] {
            let zero_hash = ByteMerkleTree::compute_zero_hash(chunk);
            for length in [0, 1, chunk - 1, chunk, chunk + 1, chunk * 3 + 1] {
                let input = vec![0x53; length];
                for index in 0..5 {
                    let start = index * chunk;
                    let end = (start + chunk).min(input.len());
                    let mut padded = [0; 32];
                    if start < end {
                        padded[..end - start].copy_from_slice(&input[start..end]);
                    }
                    let expected: [u8; 32] = Sha256::digest(&padded[..chunk]).into();
                    let (digest, observed) = digest_leaf(&input, index, chunk, zero_hash, context);
                    assert_eq!(digest, expected);
                    assert!(observed.matches(Sha256Backend::Scalar, start < end));
                    if start >= end {
                        assert_eq!(observed, Sha256Observed::default());
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_context_matches_ordinary_and_canonical_trees_for_ragged_geometry() {
        let context = scalar_context();
        for chunk in [1, 17, 32] {
            for length in [0, chunk, chunk * 65 - 1] {
                let input = vec![0x63; length];
                let canonical =
                    iroha_crypto::MerkleTree::<[u8; 32]>::from_byte_chunks(&input, chunk).unwrap();
                let (tree, observed) =
                    ByteMerkleTree::from_bytes_parallel_in_context(&input, chunk, context).unwrap();
                assert_eq!(tree.root(), *canonical.root().unwrap().as_ref());
                assert_eq!(
                    tree.root(),
                    ByteMerkleTree::from_bytes(&input, chunk).unwrap().root()
                );
                assert!(observed.matches(Sha256Backend::Scalar, length > 0));
            }
        }
        for chunk in [0, 33] {
            assert!(matches!(
                ByteMerkleTree::from_bytes_parallel_in_context(&[1], chunk, context),
                Err(VMError::MemoryOutOfBounds)
            ));
        }
    }
}
