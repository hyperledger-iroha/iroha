//! Minimal Merkle Mountain Range (MMR) accumulator for block hashes.

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use std::collections::VecDeque;

/// Stored node in the MMR (either leaf or peak).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MmrNode {
    /// Hash of the node.
    pub hash: [u8; 32],
    /// Height of the node in the binary tree (leaf = 0).
    pub height: u8,
}

/// Simple MMR accumulator keeping recent peaks in memory.
#[derive(Clone, Debug, Default)]
pub struct BlockMmr {
    /// Peaking nodes (sorted by insertion).
    pub peaks: VecDeque<MmrNode>,
    /// Current number of leaves.
    pub leaves: u64,
}

impl BlockMmr {
    /// Append a new block hash leaf and update peaks.
    pub fn push(&mut self, leaf: HashOf<BlockHeader>) {
        let node = MmrNode {
            hash: *leaf.as_ref(),
            height: 0,
        };
        self.leaves = self.leaves.saturating_add(1);

        let mut carry = node;
        let mut next_peaks = VecDeque::new();
        let iter = self.peaks.drain(..).collect::<Vec<_>>().into_iter();

        for peak in iter {
            if peak.height == carry.height {
                carry = MmrNode {
                    hash: hash_pair(peak.hash, carry.hash),
                    height: carry.height.saturating_add(1),
                };
            } else {
                next_peaks.push_back(peak);
            }
        }
        next_peaks.push_back(carry);
        self.peaks = next_peaks;
    }

    /// Return the current MMR root (bagged peaks hash) if any leaves exist.
    #[must_use]
    pub fn root(&self) -> Option<[u8; 32]> {
        let mut peaks: Vec<_> = self.peaks.iter().copied().collect();
        if peaks.is_empty() {
            return None;
        }
        peaks.sort_by_key(|n| n.height);
        let mut acc: Option<[u8; 32]> = None;
        for peak in peaks {
            acc = Some(acc.map_or(peak.hash, |curr| hash_pair(peak.hash, curr)));
        }
        acc
    }

    /// Number of leaves inserted.
    #[must_use]
    pub const fn leaves(&self) -> u64 {
        self.leaves
    }
}

fn hash_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    use iroha_crypto::Hash;
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(&left);
    buf[32..].copy_from_slice(&right);
    Hash::new(buf).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::HashOf;

    #[test]
    fn computes_root_for_two_leaves() {
        let mut mmr = BlockMmr::default();
        mmr.push(HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x01; 32]),
        ));
        mmr.push(HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x02; 32]),
        ));
        let root = mmr.root().expect("root");
        assert_ne!(root, [0u8; 32]);
    }
}
