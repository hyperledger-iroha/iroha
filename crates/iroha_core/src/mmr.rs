//! Minimal Merkle Mountain Range (MMR) accumulator for block hashes.

use std::collections::VecDeque;

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;

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
    /// Peaking nodes stored from left to right (in insertion order).
    pub peaks: VecDeque<MmrNode>,
    /// Current number of leaves.
    pub leaves: u64,
}

impl BlockMmr {
    /// Append a new block hash leaf and update peaks.
    pub fn push(&mut self, leaf: HashOf<BlockHeader>) {
        let mut carry = MmrNode {
            hash: *leaf.as_ref(),
            height: 0,
        };
        self.leaves = self.leaves.saturating_add(1);

        while let Some(last) = self.peaks.back().copied() {
            if last.height != carry.height {
                break;
            }
            let last = self.peaks.pop_back().expect("peak exists");
            carry = MmrNode {
                hash: hash_pair(last.hash, carry.hash),
                height: carry.height.saturating_add(1),
            };
        }

        self.peaks.push_back(carry);
    }

    /// Return the current MMR root (bagged peaks hash) if any leaves exist.
    ///
    /// Peaks are bagged from right to left using their left-to-right ordering:
    /// `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.
    #[must_use]
    pub fn root(&self) -> Option<[u8; 32]> {
        let mut acc: Option<[u8; 32]> = None;
        for peak in &self.peaks {
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
    use iroha_crypto::{Hash, HashOf};

    use super::*;

    #[test]
    fn computes_root_for_two_leaves() {
        let mut mmr = BlockMmr::default();
        mmr.push(HashOf::from_untyped_unchecked(Hash::prehashed([0x01; 32])));
        mmr.push(HashOf::from_untyped_unchecked(Hash::prehashed([0x02; 32])));
        let root = mmr.root().expect("root");
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn merges_peaks_in_stack_order() {
        let mut mmr = BlockMmr::default();
        let leaves = [0x01, 0x02, 0x03, 0x04]
            .map(|byte| HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])));
        for leaf in leaves {
            mmr.push(leaf);
        }

        let peak = mmr.peaks.front().expect("single peak");
        assert_eq!(mmr.peaks.len(), 1);
        assert_eq!(peak.height, 2);

        let h12 = hash_pair(*leaves[0].as_ref(), *leaves[1].as_ref());
        let h34 = hash_pair(*leaves[2].as_ref(), *leaves[3].as_ref());
        let expected = hash_pair(h12, h34);
        assert_eq!(peak.hash, expected);
        assert_eq!(mmr.root(), Some(expected));
    }

    #[test]
    fn bags_peaks_right_to_left() {
        let mut mmr = BlockMmr::default();
        let leaves = [0x01, 0x02, 0x03]
            .map(|byte| HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])));
        for leaf in leaves {
            mmr.push(leaf);
        }

        assert_eq!(mmr.peaks.len(), 2);
        assert_eq!(mmr.peaks[0].height, 1);
        assert_eq!(mmr.peaks[1].height, 0);

        let h12 = hash_pair(*leaves[0].as_ref(), *leaves[1].as_ref());
        let expected = hash_pair(*leaves[2].as_ref(), h12);
        assert_eq!(mmr.root(), Some(expected));
    }
}
