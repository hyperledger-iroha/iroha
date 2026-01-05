//! Utilities for working with Merkle proofs emitted by the VM.

use iroha_crypto::{CompactMerkleProof, Hash, HashOf};
use sha2::Digest as _;

use crate::{Memory, Registers};

/// Decode a compact Merkle proof from bytes written by the GET_MERKLE_COMPACT syscall.
///
/// Layout: `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]`
/// - `depth` and `count` must be equal; `count <= 32`.
/// - `dirs` encodes direction bits for each level (0: accumulator is left child; 1: right child).
/// - Siblings are 32-byte digests (all-zero encodes a missing sibling / promotion).
///
/// Returns the decoded `CompactMerkleProof<[u8;32]>` and the total number of bytes consumed.
pub fn decode_compact_proof_bytes(
    data: &[u8],
) -> Result<(CompactMerkleProof<[u8; 32]>, usize), &'static str> {
    if data.len() < 1 + 4 + 4 {
        return Err("buffer too short for compact proof header");
    }
    let depth = data[0] as usize;
    let dirs = u32::from_le_bytes(
        data[1..5]
            .try_into()
            .map_err(|_| "invalid dirs in compact header")?,
    );
    let count = u32::from_le_bytes(
        data[5..9]
            .try_into()
            .map_err(|_| "invalid count in compact header")?,
    ) as usize;
    if depth != count || depth > 32 {
        return Err("invalid compact header depth/count");
    }
    let need = 1 + 4 + 4 + depth * 32;
    if data.len() < need {
        return Err("buffer too short for compact siblings");
    }
    let mut siblings = Vec::with_capacity(depth);
    let mut off = 1 + 4 + 4;
    for _ in 0..depth {
        let mut b = [0u8; 32];
        b.copy_from_slice(&data[off..off + 32]);
        off += 32;
        if b == [0u8; 32] {
            siblings.push(None);
        } else {
            siblings.push(Some(HashOf::from_untyped_unchecked(Hash::prehashed(b))));
        }
    }
    let proof = CompactMerkleProof::from_parts(depth as u8, dirs, siblings);
    Ok((proof, need))
}

/// Build a compact Merkle proof for a given leaf index from a vector of
/// authentication siblings encoded as raw bytes (leaf→root). Zeros encode
/// missing siblings. `depth_cap` limits the number of levels used (<= 32).
pub fn make_compact_from_path_bytes(
    path: &[[u8; 32]],
    leaf_index: u32,
    depth_cap: Option<usize>,
) -> CompactMerkleProof<[u8; 32]> {
    let depth = depth_cap.map_or_else(|| path.len().min(32), |cap| path.len().min(cap).min(32));
    // Direction bits follow leaf-index semantics; depth caps use low bits.
    let dirs = if depth == 32 {
        leaf_index
    } else {
        leaf_index & ((1u32 << depth) - 1)
    };
    let siblings = path
        .iter()
        .take(depth)
        .map(|b| {
            if *b == [0u8; 32] {
                None
            } else {
                Some(HashOf::from_untyped_unchecked(Hash::prehashed(*b)))
            }
        })
        .collect();
    CompactMerkleProof::from_parts(depth as u8, dirs, siblings)
}

/// Compute the SHA-256 digest used as a memory leaf from a 32-byte chunk.
/// The memory leaf is `SHA-256(chunk)` where the last partial chunk is
/// zero-padded to 32 bytes before hashing.
pub fn compute_memory_leaf_digest(chunk: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&sha2::Sha256::digest(chunk));
    out
}

/// Compute the SHA-256 digest used as a register leaf from a value and tag.
/// The register leaf is `SHA-256([tag(1)] || value_le(8))` where `tag=false`
/// is encoded as 0 and `tag=true` as 1.
pub fn compute_register_leaf_digest(value: u64, tag: bool) -> [u8; 32] {
    let mut bytes = [0u8; 9];
    bytes[0] = if tag { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&sha2::Sha256::digest(bytes));
    out
}

/// A compact proof bundle suitable for IPC: includes the compact proof header
/// (depth, dirs), the sibling list encoded as raw bytes (32‑zero indicates a
/// missing sibling), and the Merkle root bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactProofBundle {
    /// Number of levels in the proof (≤ 32).
    pub depth: u8,
    /// Direction bitset (bit i is the direction at level i).
    pub dirs: u32,
    /// Sibling list (leaf→root), with 32‑byte zero indicating a missing node.
    pub siblings: Vec<[u8; 32]>,
    /// Merkle root bytes associated with this proof.
    pub root: [u8; 32],
}

impl CompactProofBundle {
    /// Convert this bundle back into a typed `CompactMerkleProof` using
    /// `None` for zero-siblings.
    pub fn to_compact_proof(&self) -> CompactMerkleProof<[u8; 32]> {
        let siblings = self
            .siblings
            .iter()
            .map(|b| {
                if *b == [0u8; 32] {
                    None
                } else {
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(*b)))
                }
            })
            .collect();
        CompactMerkleProof::from_parts(self.depth, self.dirs, siblings)
    }

    /// Convert this bundle into a full Merkle proof by first converting to a
    /// typed compact proof and then expanding with the given `leaf_index`.
    pub fn into_full_proof(self, leaf_index: u32) -> iroha_crypto::MerkleProof<[u8; 32]> {
        if crate::dev_env::debug_compact_enabled() {
            eprintln!(
                "[bundle→full] depth={} dirs=0x{:08x} leaf_index={}",
                self.depth, self.dirs, leaf_index
            );
        }
        // Build directly from raw sibling bytes to avoid any ambiguity in
        // direction-bit interpretation across crates.
        iroha_crypto::MerkleProof::<[u8; 32]>::from_audit_path_bytes(leaf_index, self.siblings)
    }
}

// Norito encoding/decoding so bundles can be passed via INPUT/OUTPUT.
impl norito::core::NoritoSerialize for CompactProofBundle {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        // Serialize as a tuple (depth, dirs, siblings, root)
        let tuple = (self.depth, self.dirs, self.siblings.clone(), self.root);
        norito::core::NoritoSerialize::serialize(&tuple, &mut writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for CompactProofBundle {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        // SAFETY: The archived layout matches the tuple (u8, u32, Vec<[u8;32]>, [u8;32])
        #[allow(unsafe_code)]
        let (depth, dirs, siblings, root): (u8, u32, Vec<[u8; 32]>, [u8; 32]) =
            norito::core::NoritoDeserialize::deserialize(unsafe {
                &*core::ptr::from_ref(archived).cast::<norito::core::Archived<(
                    u8,
                    u32,
                    Vec<[u8; 32]>,
                    [u8; 32],
                )>>()
            });
        CompactProofBundle {
            depth,
            dirs,
            siblings,
            root,
        }
    }
}

/// Build a `CompactProofBundle` for a memory address using the in‑process
/// compact builder.
pub fn memory_compact_bundle(
    mem: &mut Memory,
    addr: u64,
    depth_cap: Option<usize>,
) -> CompactProofBundle {
    let (cp, root) = mem.merkle_compact(addr, depth_cap);
    if crate::dev_env::debug_compact_enabled() {
        eprintln!(
            "[mem-compact] addr=0x{addr:08x} depth={} dirs=0x{:08x}",
            cp.depth(),
            cp.dirs(),
            addr = addr,
        );
    }
    let siblings: Vec<[u8; 32]> = cp
        .siblings()
        .iter()
        .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
        .collect();
    let root_bytes = *root.as_ref();
    CompactProofBundle {
        depth: cp.depth(),
        dirs: cp.dirs(),
        siblings,
        root: root_bytes,
    }
}

/// Build a `CompactProofBundle` for a register index using the in‑process
/// compact builder.
pub fn registers_compact_bundle(
    regs: &Registers,
    idx: usize,
    depth_cap: Option<usize>,
) -> CompactProofBundle {
    let (cp, root) = regs.merkle_compact(idx, depth_cap);
    let siblings: Vec<[u8; 32]> = cp
        .siblings()
        .iter()
        .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
        .collect();
    let root_bytes = *root.as_ref();
    CompactProofBundle {
        depth: cp.depth(),
        dirs: cp.dirs(),
        siblings,
        root: root_bytes,
    }
}
