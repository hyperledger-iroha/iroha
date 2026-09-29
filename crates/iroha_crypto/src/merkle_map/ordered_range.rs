//! Ordered, bounded range witnesses over raw canonical Norito key bytes.
//!
//! This is a separate commitment from [`super::MerkleMap`], whose keys are
//! already hashed. A trusted table owner supplies canonical Norito key/value
//! frames in strictly increasing **raw key-byte** order. Membership paths bind
//! each row's index, while authenticated immediate neighbors prove that no row
//! inside a requested half-open interval was omitted. The caller must obtain
//! the expected root, schema, and table domain independently; a proof does not
//! establish State finality or complete-State ownership by itself.
//!
//! TODO: Complete the State-owned authoritative row traversal, finalized root
//! custody, and execution-admission funding. The scoped canonical leaf builder
//! already derives its paired lookup index from this tree's retained rows.

use crate::Hash;

const EMPTY_DOMAIN: &[u8] = b"iroha:norito-key-range:empty:v1\0";
const KEY_DOMAIN: &[u8] = b"iroha:norito-key-range:key:v1\0";
const VALUE_DOMAIN: &[u8] = b"iroha:norito-key-range:value:v1\0";
const LEAF_DOMAIN: &[u8] = b"iroha:norito-key-range:leaf:v1\0";
const PAD_DOMAIN: &[u8] = b"iroha:norito-key-range:pad:v1\0";
const BRANCH_DOMAIN: &[u8] = b"iroha:norito-key-range:branch:v1\0";
const ROOT_DOMAIN: &[u8] = b"iroha:norito-key-range:root:v1\0";

/// Maximum number of rows held by one ordered table builder.
pub const MAX_NORITO_TREE_ENTRIES: usize = 65_536;
/// Maximum sum of the table's retained canonical key and value frame lengths.
pub const MAX_NORITO_TREE_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
/// Maximum length of one canonical Norito key frame or interval endpoint.
pub const MAX_NORITO_KEY_BYTES: usize = 4 * 1024;
/// Maximum length of one canonical Norito value frame.
pub const MAX_NORITO_VALUE_BYTES: usize = 1024 * 1024;
/// Maximum length of the canonical table-identity domain.
pub const MAX_NORITO_DOMAIN_BYTES: usize = 128;
/// Maximum number of interior rows in one authenticated range witness.
pub const MAX_NORITO_RANGE_ROWS: usize = 8_192;
/// Maximum logical bytes in one authenticated range witness.
pub const MAX_NORITO_RANGE_PROOF_BYTES: usize = 16 * 1024 * 1024;

const PROOF_HEADER_BYTES: usize = 12;
const ROW_HEADER_BYTES: usize = 12;
const BOUNDARY_HEADER_BYTES: usize = 8 + Hash::LENGTH;

#[path = "ordered_range/error.rs"]
mod error;
pub use error::NoritoKeyRangeError;

#[derive(Clone, Debug)]
struct StoredEntry {
    key: Vec<u8>,
    value: Vec<u8>,
    value_digest: Hash,
}

/// A trusted, bounded commitment to sorted canonical Norito key/value frames.
///
/// The builder checks raw key-byte order and uniqueness. Its caller owns the
/// table-specific canonical Norito encoder and must feed each authoritative
/// key/value frame exactly once; this generic cryptographic layer cannot decode
/// an arbitrary table schema to validate that encoding.
#[derive(Clone, Debug)]
pub struct NoritoKeyRangeTreeV1 {
    entries: Vec<StoredEntry>,
    levels: Vec<Vec<Hash>>,
    root: Hash,
}

/// One fully disclosed interior row and its authenticated Merkle path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoritoKeyRangeRowV1 {
    index: u32,
    key: Vec<u8>,
    value: Vec<u8>,
    siblings: Vec<Hash>,
}

impl NoritoKeyRangeRowV1 {
    /// Raw canonical Norito key bytes, meaningful only after proof verification.
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    /// Raw canonical Norito value bytes, meaningful only after proof verification.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Boundary {
    index: u32,
    key: Vec<u8>,
    value_digest: Hash,
    siblings: Vec<Hash>,
}

/// A complete half-open raw-key interval with its immediate outside neighbors.
///
/// The proof carries no authoritative root. Verify it against a separately
/// obtained root, schema, domain, exact interval, and local resource limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoritoKeyRangeProofV1 {
    entries: u32,
    before: Option<Boundary>,
    rows: Vec<NoritoKeyRangeRowV1>,
    after: Option<Boundary>,
}

/// Public inputs and resource bounds for complete ordered-range verification.
#[derive(Clone, Copy, Debug)]
pub struct NoritoKeyRangeVerifyRequestV1<'a> {
    /// Root authenticated by the caller's independent table owner.
    pub expected_root: &'a Hash,
    /// Canonical table schema digest.
    pub schema_hash: &'a Hash,
    /// Table domain included in the commitment.
    pub domain: &'a [u8],
    /// Inclusive raw-key lower bound.
    pub start: &'a [u8],
    /// Exclusive raw-key upper bound.
    pub end: &'a [u8],
    /// Maximum disclosed rows accepted by this call.
    pub max_rows: usize,
    /// Maximum proof bytes accepted by this call.
    pub max_bytes: usize,
}

/// Borrowed interior rows after a complete witness has been authenticated.
pub struct VerifiedNoritoKeyRangeV1<'a> {
    rows: &'a [NoritoKeyRangeRowV1],
}

impl VerifiedNoritoKeyRangeV1<'_> {
    /// Number of authenticated interior rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the authenticated interval contains no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Interior key/value frames in exact raw canonical key-byte order.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> + '_ {
        self.rows.iter().map(|row| (row.key(), row.value()))
    }
}

fn validate_domain(domain: &[u8]) -> Result<(), NoritoKeyRangeError> {
    if domain.is_empty() || domain.len() > MAX_NORITO_DOMAIN_BYTES {
        return Err(NoritoKeyRangeError::InvalidDomain);
    }
    Ok(())
}

fn validate_interval(start: &[u8], end: &[u8]) -> Result<(), NoritoKeyRangeError> {
    if start >= end || start.len() > MAX_NORITO_KEY_BYTES || end.len() > MAX_NORITO_KEY_BYTES {
        return Err(NoritoKeyRangeError::InvalidBounds);
    }
    Ok(())
}

fn validate_limits(max_rows: usize, max_bytes: usize) -> Result<(), NoritoKeyRangeError> {
    if max_rows > MAX_NORITO_RANGE_ROWS || max_bytes > MAX_NORITO_RANGE_PROOF_BYTES {
        return Err(NoritoKeyRangeError::InvalidLimit);
    }
    Ok(())
}

fn reserve_exact<T>(values: &mut Vec<T>, additional: usize) -> Result<(), NoritoKeyRangeError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| NoritoKeyRangeError::Allocation)
}

fn reserve_next_entry<T>(values: &mut Vec<T>, maximum: usize) -> Result<(), NoritoKeyRangeError> {
    if values.len() >= maximum {
        return Err(NoritoKeyRangeError::Capacity);
    }
    if values.len() == values.capacity() {
        let target = values.capacity().max(2).saturating_mul(2).min(maximum);
        reserve_exact(values, target - values.len())?;
    }
    Ok(())
}

fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, NoritoKeyRangeError> {
    let mut copied = Vec::new();
    reserve_exact(&mut copied, bytes.len())?;
    copied.extend_from_slice(bytes);
    Ok(copied)
}

fn digest_frame(domain: &[u8], bytes: &[u8]) -> Hash {
    let size = u32::try_from(bytes.len()).expect("admitted frame length fits u32");
    Hash::new_from_chunks(&[domain, &size.to_le_bytes(), bytes])
}

fn leaf_hash(index: u32, key_digest: Hash, value_digest: Hash) -> Hash {
    Hash::new_from_chunks(&[
        LEAF_DOMAIN,
        &index.to_le_bytes(),
        key_digest.as_ref(),
        value_digest.as_ref(),
    ])
}

fn pad_hash(index: u32) -> Hash {
    Hash::new_from_chunks(&[PAD_DOMAIN, &index.to_le_bytes()])
}

fn branch_hash(level: u8, left: Hash, right: Hash) -> Hash {
    Hash::new_from_chunks(&[BRANCH_DOMAIN, &[level], left.as_ref(), right.as_ref()])
}

fn empty_hash() -> Hash {
    Hash::new(EMPTY_DOMAIN)
}

fn root_hash(entries: u32, schema_hash: &Hash, domain: &[u8], top: Hash) -> Hash {
    let domain_len = u16::try_from(domain.len()).expect("admitted table domain fits u16");
    Hash::new_from_chunks(&[
        ROOT_DOMAIN,
        &entries.to_le_bytes(),
        schema_hash.as_ref(),
        &domain_len.to_le_bytes(),
        domain,
        top.as_ref(),
    ])
}

fn depth(entries: u32) -> Result<usize, NoritoKeyRangeError> {
    let count = usize::try_from(entries).map_err(|_| NoritoKeyRangeError::Capacity)?;
    if count > MAX_NORITO_TREE_ENTRIES {
        return Err(NoritoKeyRangeError::Capacity);
    }
    Ok(count.max(1).next_power_of_two().trailing_zeros() as usize)
}

fn add_size(total: &mut usize, additional: usize, limit: usize) -> Result<(), NoritoKeyRangeError> {
    *total = total
        .checked_add(additional)
        .filter(|size| *size <= limit)
        .ok_or(NoritoKeyRangeError::Capacity)?;
    Ok(())
}

impl NoritoKeyRangeTreeV1 {
    /// Build one history-independent tree from canonical frames in raw key order.
    ///
    /// The schema hash and nonempty table-identity domain are committed in the
    /// root along with entry count and all key/value digests. Duplicate or
    /// descending raw keys, oversized frames, and failed reservations reject
    /// construction. No partial tree is returned.
    ///
    /// # Errors
    /// Returns an invalid-domain, ordering, capacity, or allocation error.
    pub fn from_sorted<'a>(
        schema_hash: Hash,
        domain: &[u8],
        entries: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
    ) -> Result<Self, NoritoKeyRangeError> {
        validate_domain(domain)?;
        let mut retained: Vec<StoredEntry> = Vec::new();
        let mut payload_bytes = 0_usize;
        for (key, value) in entries {
            if retained.len() >= MAX_NORITO_TREE_ENTRIES
                || key.len() > MAX_NORITO_KEY_BYTES
                || value.len() > MAX_NORITO_VALUE_BYTES
            {
                return Err(NoritoKeyRangeError::Capacity);
            }
            if retained
                .last()
                .is_some_and(|previous| previous.key.as_slice() >= key)
            {
                return Err(NoritoKeyRangeError::UnsortedKeys);
            }
            add_size(&mut payload_bytes, key.len(), MAX_NORITO_TREE_PAYLOAD_BYTES)?;
            add_size(
                &mut payload_bytes,
                value.len(),
                MAX_NORITO_TREE_PAYLOAD_BYTES,
            )?;
            reserve_next_entry(&mut retained, MAX_NORITO_TREE_ENTRIES)?;
            retained.push(StoredEntry {
                key: copy_bytes(key)?,
                value: copy_bytes(value)?,
                value_digest: digest_frame(VALUE_DOMAIN, value),
            });
        }

        let mut levels: Vec<Vec<Hash>> = Vec::new();
        if !retained.is_empty() {
            let leaf_count = retained.len().next_power_of_two();
            let mut leaves = Vec::new();
            reserve_exact(&mut leaves, leaf_count)?;
            for (index, entry) in retained.iter().enumerate() {
                let index = u32::try_from(index).expect("admitted entry count fits u32");
                leaves.push(leaf_hash(
                    index,
                    digest_frame(KEY_DOMAIN, &entry.key),
                    entry.value_digest,
                ));
            }
            for index in retained.len()..leaf_count {
                leaves.push(pad_hash(
                    u32::try_from(index).expect("admitted padded count fits u32"),
                ));
            }
            reserve_exact(&mut levels, 1)?;
            levels.push(leaves);
            let mut level = 0_u8;
            while levels.last().is_some_and(|nodes| nodes.len() > 1) {
                let current = levels.last().expect("a nonempty tree has a level");
                let mut parents = Vec::new();
                reserve_exact(&mut parents, current.len() / 2)?;
                for pair in current.chunks_exact(2) {
                    parents.push(branch_hash(level, pair[0], pair[1]));
                }
                reserve_exact(&mut levels, 1)?;
                levels.push(parents);
                level += 1;
            }
        }
        let entries = u32::try_from(retained.len()).expect("admitted entry count fits u32");
        let top = levels
            .last()
            .and_then(|nodes| nodes.first())
            .copied()
            .unwrap_or_else(empty_hash);
        Ok(Self {
            root: root_hash(entries, &schema_hash, domain, top),
            entries: retained,
            levels,
        })
    }

    /// Root for the exact sorted key/value set, count, schema, and domain.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Number of authoritative rows supplied to the trusted builder.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the authenticated table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Borrow the exact trusted-builder rows in canonical raw-key order.
    ///
    /// A higher-level State owner can derive a second index from these same
    /// retained bytes without re-encoding mutable application values. This
    /// iterator does not attest that the caller supplied a complete table.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> + '_ {
        self.entries
            .iter()
            .map(|entry| (entry.key.as_slice(), entry.value.as_slice()))
    }

    fn siblings(&self, index: usize) -> Result<Vec<Hash>, NoritoKeyRangeError> {
        let mut siblings = Vec::new();
        reserve_exact(&mut siblings, self.levels.len().saturating_sub(1))?;
        let mut position = index;
        for nodes in self.levels.iter().take(self.levels.len().saturating_sub(1)) {
            siblings.push(nodes[position ^ 1]);
            position /= 2;
        }
        Ok(siblings)
    }

    fn row(&self, index: usize) -> Result<NoritoKeyRangeRowV1, NoritoKeyRangeError> {
        let entry = &self.entries[index];
        Ok(NoritoKeyRangeRowV1 {
            index: u32::try_from(index).expect("admitted entry index fits u32"),
            key: copy_bytes(&entry.key)?,
            value: copy_bytes(&entry.value)?,
            siblings: self.siblings(index)?,
        })
    }

    fn boundary(&self, index: usize) -> Result<Boundary, NoritoKeyRangeError> {
        let entry = &self.entries[index];
        Ok(Boundary {
            index: u32::try_from(index).expect("admitted entry index fits u32"),
            key: copy_bytes(&entry.key)?,
            value_digest: entry.value_digest,
            siblings: self.siblings(index)?,
        })
    }

    /// Export every row in `[start, end)` plus authenticated immediate neighbors.
    ///
    /// The exact requested endpoints are supplied again during verification;
    /// the proof cannot substitute a wider or narrower interval. The logical
    /// proof-byte budget includes disclosed frames, indices, digests, and every
    /// sibling. All output growth is fallibly reserved.
    ///
    /// # Errors
    /// Rejects invalid bounds or limits, excess rows/bytes, or allocation
    /// refusal without altering the tree.
    pub fn prove_range(
        &self,
        start: &[u8],
        end: &[u8],
        max_rows: usize,
        max_bytes: usize,
    ) -> Result<NoritoKeyRangeProofV1, NoritoKeyRangeError> {
        validate_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        let first = self
            .entries
            .partition_point(|entry| entry.key.as_slice() < start);
        let after = self
            .entries
            .partition_point(|entry| entry.key.as_slice() < end);
        let count = after - first;
        if count > max_rows {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let path_bytes =
            depth(u32::try_from(self.entries.len()).expect("admitted count fits u32"))?
                .checked_mul(Hash::LENGTH)
                .ok_or(NoritoKeyRangeError::Capacity)?;
        let mut size = 0_usize;
        add_size(&mut size, PROOF_HEADER_BYTES, max_bytes)?;
        for entry in &self.entries[first..after] {
            add_size(&mut size, ROW_HEADER_BYTES + path_bytes, max_bytes)?;
            add_size(&mut size, entry.key.len(), max_bytes)?;
            add_size(&mut size, entry.value.len(), max_bytes)?;
        }
        for index in [
            first.checked_sub(1),
            (after < self.entries.len()).then_some(after),
        ]
        .into_iter()
        .flatten()
        {
            add_size(&mut size, BOUNDARY_HEADER_BYTES + path_bytes, max_bytes)?;
            add_size(&mut size, self.entries[index].key.len(), max_bytes)?;
        }

        let before = first
            .checked_sub(1)
            .map(|index| self.boundary(index))
            .transpose()?;
        let mut rows = Vec::new();
        reserve_exact(&mut rows, count)?;
        for index in first..after {
            rows.push(self.row(index)?);
        }
        let after = (after < self.entries.len())
            .then(|| self.boundary(after))
            .transpose()?;
        Ok(NoritoKeyRangeProofV1 {
            entries: u32::try_from(self.entries.len()).expect("admitted count fits u32"),
            before,
            rows,
            after,
        })
    }
}

impl NoritoKeyRangeProofV1 {
    /// Entry count claimed by this untrusted witness.
    pub fn entry_count(&self) -> u32 {
        self.entries
    }

    fn logical_size(&self, path_depth: usize, limit: usize) -> Result<(), NoritoKeyRangeError> {
        let path_bytes = path_depth
            .checked_mul(Hash::LENGTH)
            .ok_or(NoritoKeyRangeError::Capacity)?;
        let mut size = 0_usize;
        add_size(&mut size, PROOF_HEADER_BYTES, limit)?;
        for row in &self.rows {
            if row.key.len() > MAX_NORITO_KEY_BYTES
                || row.value.len() > MAX_NORITO_VALUE_BYTES
                || row.siblings.len() != path_depth
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            add_size(&mut size, ROW_HEADER_BYTES + path_bytes, limit)?;
            add_size(&mut size, row.key.len(), limit)?;
            add_size(&mut size, row.value.len(), limit)?;
        }
        for boundary in [&self.before, &self.after].into_iter().flatten() {
            if boundary.key.len() > MAX_NORITO_KEY_BYTES || boundary.siblings.len() != path_depth {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            add_size(&mut size, BOUNDARY_HEADER_BYTES + path_bytes, limit)?;
            add_size(&mut size, boundary.key.len(), limit)?;
        }
        Ok(())
    }

    fn verify_member(
        entries: u32,
        index: u32,
        key: &[u8],
        value_digest: Hash,
        siblings: &[Hash],
        context: (&Hash, &Hash, &[u8]),
    ) -> Result<(), NoritoKeyRangeError> {
        let (expected_root, schema_hash, domain) = context;
        if index >= entries {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        let mut position = index;
        let mut node = leaf_hash(index, digest_frame(KEY_DOMAIN, key), value_digest);
        for (level, sibling) in siblings.iter().enumerate() {
            let level = u8::try_from(level).expect("admitted tree depth fits u8");
            node = if position & 1 == 0 {
                branch_hash(level, node, *sibling)
            } else {
                branch_hash(level, *sibling, node)
            };
            position >>= 1;
        }
        if root_hash(entries, schema_hash, domain, node) != *expected_root {
            return Err(NoritoKeyRangeError::RootMismatch);
        }
        Ok(())
    }

    /// Verify a complete interval against a root supplied by its separate owner.
    ///
    /// Every interior row and immediate neighbor must authenticate at its
    /// claimed index. Consecutive indices and raw-byte ordering rule out omitted,
    /// reordered, or duplicate rows. Empty intervals require adjacent neighbors
    /// or a proved table edge. Verification borrows the already bounded proof
    /// rows and performs no allocation.
    ///
    /// # Errors
    /// Rejects wrong roots/schema/domains, invalid bounds, excessive proof
    /// resources, forged paths, and incomplete or malformed intervals.
    pub fn verify(
        &self,
        request: NoritoKeyRangeVerifyRequestV1<'_>,
    ) -> Result<VerifiedNoritoKeyRangeV1<'_>, NoritoKeyRangeError> {
        let NoritoKeyRangeVerifyRequestV1 {
            expected_root,
            schema_hash,
            domain,
            start,
            end,
            max_rows,
            max_bytes,
        } = request;
        validate_domain(domain)?;
        validate_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        if self.rows.len() > max_rows {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let path_depth = depth(self.entries)?;
        self.logical_size(path_depth, max_bytes)?;
        if self.entries == 0 {
            if self.before.is_some() || !self.rows.is_empty() || self.after.is_some() {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            if root_hash(0, schema_hash, domain, empty_hash()) != *expected_root {
                return Err(NoritoKeyRangeError::RootMismatch);
            }
            return Ok(VerifiedNoritoKeyRangeV1 { rows: &self.rows });
        }

        let mut next_index = 0_u32;
        let mut previous_key: Option<&[u8]> = if let Some(before) = &self.before {
            if before.key.as_slice() >= start {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            Self::verify_member(
                self.entries,
                before.index,
                &before.key,
                before.value_digest,
                &before.siblings,
                (expected_root, schema_hash, domain),
            )?;
            next_index = before.index + 1;
            Some(&before.key)
        } else {
            None
        };
        for row in &self.rows {
            if row.index != next_index
                || row.key.as_slice() < start
                || row.key.as_slice() >= end
                || previous_key.is_some_and(|key| key >= row.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            Self::verify_member(
                self.entries,
                row.index,
                &row.key,
                digest_frame(VALUE_DOMAIN, &row.value),
                &row.siblings,
                (expected_root, schema_hash, domain),
            )?;
            next_index += 1;
            previous_key = Some(&row.key);
        }
        if let Some(after) = &self.after {
            if after.index != next_index
                || after.key.as_slice() < end
                || previous_key.is_some_and(|key| key >= after.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            Self::verify_member(
                self.entries,
                after.index,
                &after.key,
                after.value_digest,
                &after.siblings,
                (expected_root, schema_hash, domain),
            )?;
        } else if next_index != self.entries {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        Ok(VerifiedNoritoKeyRangeV1 { rows: &self.rows })
    }
}

#[cfg(test)]
#[path = "ordered_range/tests.rs"]
mod tests;

#[path = "ordered_range/external.rs"]
mod external;
pub use external::{
    NoritoKeyRangeExternalProofV1, NoritoKeyRangeExternalV1, NoritoKeyRangeNodeStoreV1,
    VerifiedNoritoKeyRangeExternalV1,
};

#[path = "ordered_range/digest.rs"]
mod digest;
pub use digest::{
    NoritoKeyDigestRangeProofV1, NoritoKeyDigestRangeTreeV1, VerifiedNoritoKeyDigestRangeV1,
    digest_norito_value_frame_v1,
};
