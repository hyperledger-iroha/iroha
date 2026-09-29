//! Scalable ordered table commitments with caller-owned external node custody.
//!
//! This is a crypto substrate, not a State root. The caller must enumerate one
//! complete, immutable canonical table generation, reserve durable node storage,
//! and publish its root only after the staging store is committed. A failed
//! build may leave staging nodes and never returns a publishable root.

use super::{
    BOUNDARY_HEADER_BYTES, KEY_DOMAIN, MAX_NORITO_KEY_BYTES, MAX_NORITO_VALUE_BYTES,
    NoritoKeyRangeError, NoritoKeyRangeVerifyRequestV1, ROW_HEADER_BYTES, add_size, copy_bytes,
    digest_frame, empty_hash, reserve_exact, validate_domain, validate_limits,
};
use crate::Hash;

const EXTERNAL_LEAF_DOMAIN: &[u8] = b"iroha:norito-key-range:external-leaf:v1\0";
const EXTERNAL_BRANCH_DOMAIN: &[u8] = b"iroha:norito-key-range:external-branch:v1\0";
const EXTERNAL_ROOT_DOMAIN: &[u8] = b"iroha:norito-key-range:external-root:v1\0";
const EXTERNAL_PROOF_HEADER_BYTES: usize = 16;
const EXTERNAL_ROW_HEADER_BYTES: usize = ROW_HEADER_BYTES + 4;
const EXTERNAL_BOUNDARY_HEADER_BYTES: usize = BOUNDARY_HEADER_BYTES + 4;
const EXTERNAL_SIBLING_BYTES: usize = 1 + Hash::LENGTH;

/// Caller-owned, fallible staging storage for immutable Merkle node hashes.
///
/// `put` must write each `(level, index)` exactly once. `get` must return the
/// previously written hash or fail. The owner must reserve before growth,
/// discard staging on any error, retain nodes for the root's lifetime, and
/// authenticate durable recovery before serving witnesses. This trait itself
/// neither owns persistence nor grants publication authority.
pub trait NoritoKeyRangeNodeStoreV1 {
    /// Stage one node at its canonical level and zero-based position.
    ///
    /// # Errors
    /// Returns the original storage or resource-admission failure.
    fn put(&mut self, level: u8, index: u64, hash: Hash) -> Result<(), NoritoKeyRangeError>;
    /// Read one already staged or committed node.
    ///
    /// # Errors
    /// Returns the original storage or resource-admission failure.
    fn get(&self, level: u8, index: u64) -> Result<Hash, NoritoKeyRangeError>;
}

/// A u64-sized ordered table root backed by externally retained node hashes.
///
/// The source's raw canonical key/value frames are not retained here. A later
/// witness request supplies an immutable view of the same source generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoritoKeyRangeExternalV1 {
    root: Hash,
    entries: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExternalBoundary {
    index: u64,
    key: Vec<u8>,
    value_digest: Hash,
    siblings: Vec<Option<Hash>>,
}

/// One bounded disclosed row of an external-table range witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRangeRowV1 {
    index: u64,
    key: Vec<u8>,
    value: Vec<u8>,
    siblings: Vec<Option<Hash>>,
}

impl ExternalRangeRowV1 {
    /// Raw canonical key bytes after successful proof verification.
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    /// Raw canonical value bytes after successful proof verification.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

/// A complete half-open interval with authenticated immediate neighbors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoritoKeyRangeExternalProofV1 {
    entries: u64,
    before: Option<ExternalBoundary>,
    rows: Vec<ExternalRangeRowV1>,
    after: Option<ExternalBoundary>,
}

/// Borrowed, authenticated rows of a complete interval.
pub struct VerifiedNoritoKeyRangeExternalV1<'a> {
    rows: &'a [ExternalRangeRowV1],
}

impl VerifiedNoritoKeyRangeExternalV1<'_> {
    /// Number of authenticated interior rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the authenticated interval contains no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Complete interior key/value frames in raw canonical key-byte order.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> + '_ {
        self.rows.iter().map(|row| (row.key(), row.value()))
    }
}

fn external_leaf(index: u64, key: &[u8], value: &[u8]) -> Hash {
    Hash::new_from_chunks(&[
        EXTERNAL_LEAF_DOMAIN,
        &index.to_le_bytes(),
        digest_frame(KEY_DOMAIN, key).as_ref(),
        digest_frame(super::VALUE_DOMAIN, value).as_ref(),
    ])
}

fn external_leaf_from_digests(index: u64, key: &[u8], value_digest: Hash) -> Hash {
    Hash::new_from_chunks(&[
        EXTERNAL_LEAF_DOMAIN,
        &index.to_le_bytes(),
        digest_frame(KEY_DOMAIN, key).as_ref(),
        value_digest.as_ref(),
    ])
}

fn external_branch(level: u8, left: Hash, right: Hash) -> Hash {
    Hash::new_from_chunks(&[
        EXTERNAL_BRANCH_DOMAIN,
        &[level],
        left.as_ref(),
        right.as_ref(),
    ])
}

fn external_root(entries: u64, schema: &Hash, domain: &[u8], top: Hash) -> Hash {
    let domain_len = u16::try_from(domain.len()).expect("admitted domain fits u16");
    Hash::new_from_chunks(&[
        EXTERNAL_ROOT_DOMAIN,
        &entries.to_le_bytes(),
        schema.as_ref(),
        &domain_len.to_le_bytes(),
        domain,
        top.as_ref(),
    ])
}

fn next_width(width: u64) -> u64 {
    width / 2 + width % 2
}

fn validate_external_interval(start: &[u8], end: &[u8]) -> Result<(), NoritoKeyRangeError> {
    // A lookup uses the immediate lexicographic successor `key || 0`; that
    // sentinel may be one byte longer than a maximum-size canonical key.
    if start >= end || start.len() > MAX_NORITO_KEY_BYTES || end.len() > MAX_NORITO_KEY_BYTES + 1 {
        return Err(NoritoKeyRangeError::InvalidBounds);
    }
    Ok(())
}

fn lookup_end(key: &[u8]) -> Result<Vec<u8>, NoritoKeyRangeError> {
    if key.len() > MAX_NORITO_KEY_BYTES {
        return Err(NoritoKeyRangeError::InvalidBounds);
    }
    let mut end = Vec::new();
    reserve_exact(&mut end, key.len() + 1)?;
    end.extend_from_slice(key);
    end.push(0);
    Ok(end)
}

fn tree_depth(mut width: u64) -> usize {
    let mut depth = 0;
    while width > 1 {
        width = next_width(width);
        depth += 1;
    }
    depth
}

fn proof_path(
    store: &impl NoritoKeyRangeNodeStoreV1,
    entries: u64,
    index: u64,
) -> Result<Vec<Option<Hash>>, NoritoKeyRangeError> {
    let mut path = Vec::new();
    reserve_exact(&mut path, tree_depth(entries))?;
    let mut width = entries;
    let mut position = index;
    let mut level = 0_u8;
    while width > 1 {
        let sibling = position ^ 1;
        path.push(
            (sibling < width)
                .then(|| store.get(level, sibling))
                .transpose()?,
        );
        width = next_width(width);
        position /= 2;
        level += 1;
    }
    Ok(path)
}

fn proof_path_size(entries: u64) -> Result<usize, NoritoKeyRangeError> {
    tree_depth(entries)
        .checked_mul(EXTERNAL_SIBLING_BYTES)
        .ok_or(NoritoKeyRangeError::Capacity)
}

fn checked_row_size(
    size: &mut usize,
    key: &[u8],
    value: &[u8],
    path_bytes: usize,
    limit: usize,
) -> Result<(), NoritoKeyRangeError> {
    add_size(size, EXTERNAL_ROW_HEADER_BYTES + path_bytes, limit)?;
    add_size(size, key.len(), limit)?;
    add_size(size, value.len(), limit)
}

fn checked_boundary_size(
    size: &mut usize,
    key: &[u8],
    path_bytes: usize,
    limit: usize,
) -> Result<(), NoritoKeyRangeError> {
    add_size(size, EXTERNAL_BOUNDARY_HEADER_BYTES + path_bytes, limit)?;
    add_size(size, key.len(), limit)
}

impl NoritoKeyRangeExternalV1 {
    /// Stream a sorted table into a staged external node store using bounded RAM.
    ///
    /// This performs one row pass plus level folds over caller-owned nodes. Its
    /// transient row workspace is one key (at most 4 KiB); it retains no table
    /// payloads or in-memory node vector. The row count is u64, while key/value
    /// frame and witness limits stay fixed. Caller-provided storage must be
    /// isolated from any published root until this returns successfully.
    ///
    /// # Errors
    /// Rejects invalid domains, frames, key order, count overflow, or store
    /// failures. A failed staging store is not safe to publish.
    pub fn from_sorted<'a>(
        schema: Hash,
        domain: &[u8],
        rows: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
        store: &mut impl NoritoKeyRangeNodeStoreV1,
    ) -> Result<Self, NoritoKeyRangeError> {
        validate_domain(domain)?;
        let mut previous = Vec::new();
        let mut entries = 0_u64;
        for (key, value) in rows {
            if key.len() > MAX_NORITO_KEY_BYTES || value.len() > MAX_NORITO_VALUE_BYTES {
                return Err(NoritoKeyRangeError::Capacity);
            }
            if entries > 0 && previous.as_slice() >= key {
                return Err(NoritoKeyRangeError::UnsortedKeys);
            }
            if entries == u64::MAX {
                return Err(NoritoKeyRangeError::Capacity);
            }
            store.put(0, entries, external_leaf(entries, key, value))?;
            previous.clear();
            reserve_exact(&mut previous, key.len())?;
            previous.extend_from_slice(key);
            entries += 1;
        }
        let mut width = entries;
        let mut level = 0_u8;
        while width > 1 {
            let parent_width = next_width(width);
            for index in 0..parent_width {
                let left = store.get(level, index * 2)?;
                let right = if index * 2 + 1 < width {
                    Some(store.get(level, index * 2 + 1)?)
                } else {
                    None
                };
                let parent = right.map_or(left, |right| external_branch(level, left, right));
                store.put(level + 1, index, parent)?;
            }
            width = parent_width;
            level += 1;
        }
        let top = if entries == 0 {
            empty_hash()
        } else {
            store.get(level, 0)?
        };
        Ok(Self {
            root: external_root(entries, &schema, domain, top),
            entries,
        })
    }

    /// Root binding exact u64 row count, declared schema, and table identity.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Number of sorted canonical rows committed by the builder.
    pub fn len(&self) -> u64 {
        self.entries
    }

    /// Whether the table committed no rows.
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    /// Regenerate one bounded complete interval from the same immutable row cut.
    ///
    /// The source is scanned without retaining unrelated rows. The returned
    /// witness authenticates its disclosed rows and immediate neighbors against
    /// `root()`. The caller must hold the committed source generation; this API
    /// scans it completely to validate its exact count and raw-key order.
    ///
    /// # Errors
    /// Rejects malformed source order/count, invalid interval, excess witness
    /// resources, missing external nodes, or allocation refusal.
    pub fn prove_range<'a>(
        &self,
        store: &impl NoritoKeyRangeNodeStoreV1,
        rows: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
        start: &[u8],
        end: &[u8],
        max_rows: usize,
        max_bytes: usize,
    ) -> Result<NoritoKeyRangeExternalProofV1, NoritoKeyRangeError> {
        validate_external_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        let path_bytes = proof_path_size(self.entries)?;
        let mut size = EXTERNAL_PROOF_HEADER_BYTES;
        if size > max_bytes {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let mut before = None;
        let mut rows_out = Vec::new();
        let mut after = None;
        let mut previous = Vec::new();
        let mut previous_digest = None;
        let mut found_start = false;
        let mut count = 0_u64;
        for (key, value) in rows {
            if key.len() > MAX_NORITO_KEY_BYTES || value.len() > MAX_NORITO_VALUE_BYTES {
                return Err(NoritoKeyRangeError::Capacity);
            }
            if count > 0 && previous.as_slice() >= key {
                return Err(NoritoKeyRangeError::UnsortedKeys);
            }
            if count >= self.entries {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            if !found_start && key >= start {
                found_start = true;
                if let Some(digest) = previous_digest {
                    checked_boundary_size(&mut size, &previous, path_bytes, max_bytes)?;
                    before = Some(ExternalBoundary {
                        index: count - 1,
                        key: copy_bytes(&previous)?,
                        value_digest: digest,
                        siblings: Vec::new(),
                    });
                }
            }
            if key >= start && key < end {
                if rows_out.len() >= max_rows {
                    return Err(NoritoKeyRangeError::Capacity);
                }
                checked_row_size(&mut size, key, value, path_bytes, max_bytes)?;
                reserve_exact(&mut rows_out, 1)?;
                rows_out.push(ExternalRangeRowV1 {
                    index: count,
                    key: copy_bytes(key)?,
                    value: copy_bytes(value)?,
                    siblings: Vec::new(),
                });
            } else if key >= end && after.is_none() {
                checked_boundary_size(&mut size, key, path_bytes, max_bytes)?;
                after = Some(ExternalBoundary {
                    index: count,
                    key: copy_bytes(key)?,
                    value_digest: digest_frame(super::VALUE_DOMAIN, value),
                    siblings: Vec::new(),
                });
            }
            previous.clear();
            reserve_exact(&mut previous, key.len())?;
            previous.extend_from_slice(key);
            previous_digest = Some(digest_frame(super::VALUE_DOMAIN, value));
            count += 1;
        }
        if count != self.entries {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        if !found_start && let Some(digest) = previous_digest {
            checked_boundary_size(&mut size, &previous, path_bytes, max_bytes)?;
            before = Some(ExternalBoundary {
                index: count - 1,
                key: previous,
                value_digest: digest,
                siblings: Vec::new(),
            });
        }
        if let Some(boundary) = &mut before {
            boundary.siblings = proof_path(store, self.entries, boundary.index)?;
        }
        for row in &mut rows_out {
            row.siblings = proof_path(store, self.entries, row.index)?;
        }
        if let Some(boundary) = &mut after {
            boundary.siblings = proof_path(store, self.entries, boundary.index)?;
        }
        Ok(NoritoKeyRangeExternalProofV1 {
            entries: self.entries,
            before,
            rows: rows_out,
            after,
        })
    }

    /// Prove exact-key inclusion or absence using the immediate byte successor.
    ///
    /// This works even for a 4 KiB canonical key: the one-byte-longer endpoint
    /// is a query sentinel and is never admitted as a stored key.
    ///
    /// # Errors
    /// Returns the same bounded source, store, and allocation errors as a
    /// one-row complete-range witness.
    pub fn prove_lookup<'a>(
        &self,
        store: &impl NoritoKeyRangeNodeStoreV1,
        rows: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
        key: &[u8],
        max_bytes: usize,
    ) -> Result<NoritoKeyRangeExternalProofV1, NoritoKeyRangeError> {
        let end = lookup_end(key)?;
        self.prove_range(store, rows, key, &end, 1, max_bytes)
    }
}

impl NoritoKeyRangeExternalProofV1 {
    /// Entry count claimed by this untrusted witness.
    pub fn entry_count(&self) -> u64 {
        self.entries
    }

    fn verify_member(
        &self,
        index: u64,
        key: &[u8],
        value_digest: Hash,
        siblings: &[Option<Hash>],
        context: (&Hash, &Hash, &[u8]),
    ) -> Result<(), NoritoKeyRangeError> {
        let (expected_root, schema, domain) = context;
        if index >= self.entries || siblings.len() != tree_depth(self.entries) {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        let mut node = external_leaf_from_digests(index, key, value_digest);
        let mut width = self.entries;
        let mut position = index;
        for (level, sibling) in siblings.iter().enumerate() {
            let expected_sibling = (position ^ 1) < width;
            if sibling.is_some() != expected_sibling {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            if let Some(sibling) = sibling {
                let level = u8::try_from(level).expect("u64 tree depth fits u8");
                node = if position & 1 == 0 {
                    external_branch(level, node, *sibling)
                } else {
                    external_branch(level, *sibling, node)
                };
            }
            width = next_width(width);
            position /= 2;
        }
        if external_root(self.entries, schema, domain, node) != *expected_root {
            return Err(NoritoKeyRangeError::RootMismatch);
        }
        Ok(())
    }

    /// Authenticate every disclosed row and prove no row in the interval is omitted.
    ///
    /// Immediate outside neighbors and contiguous u64 indices establish a
    /// complete interval, including empty intervals and table edges. No heap
    /// allocation is performed during verification.
    ///
    /// # Errors
    /// Rejects forged roots, malformed paths or adjacency, wrong bounds, and
    /// witnesses above the fixed row/byte ceilings.
    pub fn verify(
        &self,
        request: NoritoKeyRangeVerifyRequestV1<'_>,
    ) -> Result<VerifiedNoritoKeyRangeExternalV1<'_>, NoritoKeyRangeError> {
        let NoritoKeyRangeVerifyRequestV1 {
            expected_root,
            schema_hash: schema,
            domain,
            start,
            end,
            max_rows,
            max_bytes,
        } = request;
        validate_domain(domain)?;
        validate_external_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        if self.rows.len() > max_rows {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let path_depth = tree_depth(self.entries);
        let path_bytes = proof_path_size(self.entries)?;
        let mut size = EXTERNAL_PROOF_HEADER_BYTES;
        if size > max_bytes {
            return Err(NoritoKeyRangeError::Capacity);
        }
        for row in &self.rows {
            if row.key.len() > MAX_NORITO_KEY_BYTES
                || row.value.len() > MAX_NORITO_VALUE_BYTES
                || row.siblings.len() != path_depth
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            checked_row_size(&mut size, &row.key, &row.value, path_bytes, max_bytes)?;
        }
        for boundary in [&self.before, &self.after].into_iter().flatten() {
            if boundary.key.len() > MAX_NORITO_KEY_BYTES || boundary.siblings.len() != path_depth {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            checked_boundary_size(&mut size, &boundary.key, path_bytes, max_bytes)?;
        }
        if self.entries == 0 {
            if self.before.is_some() || !self.rows.is_empty() || self.after.is_some() {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            if external_root(0, schema, domain, empty_hash()) != *expected_root {
                return Err(NoritoKeyRangeError::RootMismatch);
            }
            return Ok(VerifiedNoritoKeyRangeExternalV1 { rows: &self.rows });
        }
        let mut next = 0_u64;
        let mut previous_key: Option<&[u8]> = if let Some(before) = &self.before {
            if before.key.as_slice() >= start {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            self.verify_member(
                before.index,
                &before.key,
                before.value_digest,
                &before.siblings,
                (expected_root, schema, domain),
            )?;
            next = before.index + 1;
            Some(&before.key)
        } else {
            None
        };
        for row in &self.rows {
            if row.index != next
                || row.key.as_slice() < start
                || row.key.as_slice() >= end
                || previous_key.is_some_and(|key| key >= row.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            self.verify_member(
                row.index,
                &row.key,
                digest_frame(super::VALUE_DOMAIN, &row.value),
                &row.siblings,
                (expected_root, schema, domain),
            )?;
            next += 1;
            previous_key = Some(&row.key);
        }
        if let Some(after) = &self.after {
            if after.index != next
                || after.key.as_slice() < end
                || previous_key.is_some_and(|key| key >= after.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            self.verify_member(
                after.index,
                &after.key,
                after.value_digest,
                &after.siblings,
                (expected_root, schema, domain),
            )?;
        } else if next != self.entries {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        Ok(VerifiedNoritoKeyRangeExternalV1 { rows: &self.rows })
    }

    /// Verify exact-key inclusion or absence and return the authenticated value.
    ///
    /// # Errors
    /// Rejects the same malformed or excessive proof conditions as `verify`.
    pub fn verify_lookup<'a>(
        &'a self,
        expected_root: &Hash,
        schema: &Hash,
        domain: &[u8],
        key: &[u8],
        max_bytes: usize,
    ) -> Result<Option<&'a [u8]>, NoritoKeyRangeError> {
        let end = lookup_end(key)?;
        let verified = self.verify(NoritoKeyRangeVerifyRequestV1 {
            expected_root,
            schema_hash: schema,
            domain,
            start: key,
            end: &end,
            max_rows: 1,
            max_bytes,
        })?;
        Ok(verified.rows.first().map(ExternalRangeRowV1::value))
    }
}

#[cfg(test)]
#[path = "external/tests.rs"]
mod tests;
