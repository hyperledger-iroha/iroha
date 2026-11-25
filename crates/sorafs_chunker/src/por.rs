//! Canonical SoraFS proof-of-retrievability hashes and incremental chunk commitments.
use blake3::Hasher;
use std::borrow::Borrow;

/// Size of a PoR sampling segment in bytes.
pub const POR_SEGMENT_SIZE: usize = 64 * 1024;
/// Size of a PoR sampling leaf in bytes.
pub const POR_LEAF_SIZE: usize = 4 * 1024;

/// Commit one PoR leaf at its absolute byte offset and global leaf index.
#[must_use]
pub fn hash_leaf(flat_index: u64, offset: u64, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs:por:leaf:v1");
    hasher.update(&flat_index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&(bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn finish_with_children<T: Borrow<[u8; 32]>>(
    mut hasher: Hasher,
    children: impl ExactSizeIterator<Item = T>,
) -> [u8; 32] {
    hasher.update(&(children.len() as u64).to_le_bytes());
    for child in children {
        hasher.update(child.borrow());
    }
    hasher.finalize().into()
}

/// Commit a PoR segment and its ordered leaf digests.
pub fn hash_segment<T: Borrow<[u8; 32]>>(
    offset: u64,
    length: u32,
    leaves: impl ExactSizeIterator<Item = T>,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs:por:segment:v1");
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    finish_with_children(hasher, leaves)
}

/// Commit a source chunk and its ordered PoR segment digests.
pub fn hash_chunk<T: Borrow<[u8; 32]>>(
    index: u64,
    offset: u64,
    length: u32,
    chunk_digest: &[u8; 32],
    segments: impl ExactSizeIterator<Item = T>,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs:por:chunk:v1");
    hasher.update(&index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    hasher.update(chunk_digest);
    finish_with_children(hasher, segments)
}

/// Commit two ordered chunk-tree children at the given level and parent index.
#[must_use]
pub fn hash_chunk_node(
    level: u32,
    parent_index: u64,
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs:por:chunk-node:v1");
    hasher.update(&level.to_le_bytes());
    hasher.update(&parent_index.to_le_bytes());
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// Bind a nonempty ordered chunk tree to its total byte, chunk and leaf counts.
#[must_use]
pub fn hash_root(
    total_len: u64,
    chunk_count: u64,
    leaf_count: u64,
    chunk_tree_root: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs:por:root-merkle:v1");
    hasher.update(&total_len.to_le_bytes());
    hasher.update(&chunk_count.to_le_bytes());
    hasher.update(&leaf_count.to_le_bytes());
    hasher.update(chunk_tree_root);
    hasher.finalize().into()
}

/// Incremental PoR commitment over contiguous chunks in source order.
/// Keeps one pending root per Merkle level and retains no source bytes or proof tree.
pub struct PorRootBuilder {
    frontier: [Option<[u8; 32]>; u64::BITS as usize],
    total_len: u64,
    chunk_count: u64,
    leaf_count: u64,
}

impl Default for PorRootBuilder {
    fn default() -> Self {
        Self {
            frontier: [None; u64::BITS as usize],
            total_len: 0,
            chunk_count: 0,
            leaf_count: 0,
        }
    }
}

impl PorRootBuilder {
    /// Number of leaves committed across all consumed chunks.
    #[must_use]
    pub fn leaf_count(&self) -> u64 {
        self.leaf_count
    }

    /// Absorb one chunk and return its raw BLAKE3 source digest.
    /// Returns `None` without changing the commitment if the chunk is empty or its
    /// length or cumulative extent exceeds the protocol's integer widths.
    #[must_use]
    pub fn push_chunk(&mut self, bytes: &[u8]) -> Option<[u8; 32]> {
        let chunk_len = u32::try_from(bytes.len()).ok().filter(|len| *len != 0)?;
        let end = self.total_len.checked_add(u64::from(chunk_len))?;
        // Each chunk and leaf covers at least one byte, so their cumulative counts fit
        // whenever the total extent does.
        let mut flat_index = self.leaf_count;
        let chunk_offset = self.total_len;
        let segments = bytes
            .chunks(POR_SEGMENT_SIZE)
            .enumerate()
            .map(|(i, segment)| {
                let offset = chunk_offset + (i * POR_SEGMENT_SIZE) as u64;
                let leaves = segment.chunks(POR_LEAF_SIZE).enumerate().map(|(j, leaf)| {
                    let digest = hash_leaf(flat_index, offset + (j * POR_LEAF_SIZE) as u64, leaf);
                    flat_index += 1;
                    digest
                });
                hash_segment(offset, segment.len() as u32, leaves)
            });
        let chunk_digest = *blake3::hash(bytes).as_bytes();
        let mut node = hash_chunk(
            self.chunk_count,
            chunk_offset,
            chunk_len,
            &chunk_digest,
            segments,
        );
        let mut index = self.chunk_count;
        let mut level = 0;
        while let Some(left) = self.frontier[level].take() {
            index /= 2;
            node = hash_chunk_node(level as u32, index, &left, &node);
            level += 1;
        }
        self.frontier[level] = Some(node);
        self.chunk_count += 1;
        self.total_len = end;
        self.leaf_count = flat_index;
        Some(chunk_digest)
    }

    /// Return the complete source commitment, or `None` for an empty source.
    #[must_use]
    pub fn finish(self) -> Option<[u8; 32]> {
        let mut carry: Option<([u8; 32], usize, u64)> = None;
        for (level, left) in self.frontier.into_iter().enumerate() {
            let Some(left) = left else { continue };
            carry = Some(match carry {
                None => (left, level, (self.chunk_count - 1) >> level),
                Some((mut right, mut right_level, mut index)) => {
                    // An unpaired rightmost node is its own sibling at every missing level.
                    while right_level < level {
                        right = hash_chunk_node(right_level as u32, index / 2, &right, &right);
                        right_level += 1;
                        index /= 2;
                    }
                    (
                        hash_chunk_node(level as u32, index / 2, &left, &right),
                        level + 1,
                        index / 2,
                    )
                }
            });
        }
        Some(hash_root(
            self.total_len,
            self.chunk_count,
            self.leaf_count,
            &carry?.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_por_matches_frozen_byte_and_tree_boundaries() {
        // The expected bytes use a full binary tree with explicit odd-node duplication.
        for (len, chunk_size, leaves, root) in [
            (
                1,
                1024,
                1,
                "6e13a34c8e7513c9a35df2c36fbf5af8b0e4007dac453372f2b6411acc3e9304",
            ),
            (
                4097,
                1024,
                5,
                "fbb4d2ce6b40648fe96a6df4395db2ea401c4b59d4e55189dca0241e0b556535",
            ),
            (
                6145,
                1024,
                7,
                "a54012c27fd7d9727f742cbb8b4bceb2b4ffef3351760c79f0a4e923ded7bad8",
            ),
            (
                131073,
                65536,
                33,
                "53d549ebcc9dda0ec9472e17cc638be05dd4b74aecf715fbd2e169c8bb772285",
            ),
            (
                131089,
                65537,
                35,
                "c9e8de6ea5f8e660bfe90d49f36da6119f82dab7830e7ee6a969e4315109ba22",
            ),
            (
                262161,
                131073,
                67,
                "ca322c0936f2f2b743d8417d7c8be463b301d1946e00b82b66ecd84727983771",
            ),
        ] {
            let payload: Vec<_> = (0..len).map(|i| (i % 251) as u8).collect();
            let mut builder = PorRootBuilder::default();
            for chunk in payload.chunks(chunk_size) {
                assert_eq!(
                    builder.push_chunk(chunk),
                    Some(*blake3::hash(chunk).as_bytes())
                );
            }
            assert_eq!(builder.leaf_count(), leaves);
            assert_eq!(
                builder.finish(),
                Some(blake3::Hash::from_hex(root).unwrap().into())
            );
        }
    }

    #[test]
    fn empty_chunks_leave_the_commitment_unchanged() {
        let mut empty = PorRootBuilder::default();
        assert!(empty.push_chunk(&[]).is_none());
        assert_eq!(empty.leaf_count(), 0);
        assert!(empty.finish().is_none());
        let mut with_empty = PorRootBuilder::default();
        with_empty.push_chunk(b"source").unwrap();
        assert!(with_empty.push_chunk(&[]).is_none());
        let mut expected = PorRootBuilder::default();
        expected.push_chunk(b"source").unwrap();
        assert_eq!(with_empty.finish(), expected.finish());
    }
}
