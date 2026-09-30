//! Complete raw-key ranges that disclose only canonical value digests.
//!
//! A trusted schema owner must compute each digest from the complete canonical
//! value preimage. Verification authenticates the digest and key-range
//! completeness; it does not reconstruct or validate an undisclosed value.

use super::{
    BOUNDARY_HEADER_BYTES, Boundary, KEY_DOMAIN, MAX_NORITO_KEY_BYTES, MAX_NORITO_TREE_ENTRIES,
    MAX_NORITO_TREE_PAYLOAD_BYTES, NoritoKeyRangeError, NoritoKeyRangeProofV1,
    NoritoKeyRangeVerifyRequestV1, PROOF_HEADER_BYTES, ROW_HEADER_BYTES, add_size, branch_hash,
    copy_bytes, depth, digest_frame, empty_hash, leaf_hash, pad_hash, reserve_exact, root_hash,
    validate_domain, validate_interval, validate_limits,
};
use crate::Hash;
use iroha_allocation::ChargedShared;
use std::io::{self, Write};

struct ExactFrameWriter<'a> {
    inner: &'a mut dyn Write,
    remaining: usize,
    overrun_attempted: bool,
}

impl Write for ExactFrameWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            self.overrun_attempted = true;
            return Err(io::Error::other("value frame exceeds declared length"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Hash one complete canonical value frame without retaining its bytes.
///
/// The producer must provide canonical Norito bytes for its independently
/// authenticated schema. Exact byte count is checked here; this function does
/// not decode or validate arbitrary schema contents.
///
/// # Errors
/// Rejects producer failures or a frame shorter or longer than `length`.
pub fn digest_norito_value_frame_v1(
    length: u32,
    write_frame: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<Hash> {
    Hash::new_from_writer(|writer| {
        writer.write_all(super::VALUE_DOMAIN)?;
        writer.write_all(&length.to_le_bytes())?;
        let mut frame = ExactFrameWriter {
            inner: writer,
            remaining: length as usize,
            overrun_attempted: false,
        };
        write_frame(&mut frame)?;
        if frame.overrun_attempted {
            return Err(io::Error::other(
                "value frame attempted to exceed declared length",
            ));
        }
        if frame.remaining != 0 {
            return Err(io::Error::other(
                "value frame is shorter than declared length",
            ));
        }
        Ok(())
    })
}

#[path = "digest/owned.rs"]
mod owned;
#[path = "digest/verification.rs"]
mod verification;
use owned::DigestTreeBacking;

/// A bounded raw-key commitment retaining original funded key/digest backings.
///
/// Clones share the same immutable allocation owners; eviction or dropping one
/// clone cannot refund bytes still retained by another. No weak reference to
/// the private owner is exposed. Proof buffers and caller staging still require
/// their own allocation custody; these charges are not complete State funding.
#[derive(Clone, Debug)]
pub struct NoritoKeyDigestRangeTreeV1 {
    inner: ChargedShared<DigestTreeBacking>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DigestRow {
    index: u32,
    key: Vec<u8>,
    value_digest: Hash,
    siblings: Vec<Hash>,
}

/// A complete half-open raw-key range with authenticated value digests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoritoKeyDigestRangeProofV1 {
    entries: u32,
    before: Option<Boundary>,
    rows: Vec<DigestRow>,
    after: Option<Boundary>,
}

/// A verified complete interval of keys and committed value digests.
///
/// The digest alone does not authenticate a caller-supplied value preimage;
/// that requires a separate canonical encoder and equality check.
#[derive(Clone, Copy, Debug)]
pub struct VerifiedNoritoKeyDigestRangeV1<'a> {
    rows: &'a [DigestRow],
    root: Hash,
    schema_hash: Hash,
    domain_hash: Hash,
}

impl VerifiedNoritoKeyDigestRangeV1<'_> {
    /// Number of authenticated interior rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the authenticated interval has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Iterate authenticated raw keys and their value digests in exact order.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&[u8], Hash)> + '_ {
        self.rows
            .iter()
            .map(|row| (row.key.as_slice(), row.value_digest))
    }

    /// Independently supplied root against which this range was verified.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Exact V1 table-schema commitment supplied during verification.
    pub fn schema_hash(&self) -> Hash {
        self.schema_hash
    }

    /// Digest of the exact table-identity domain supplied during verification.
    pub fn domain_hash(&self) -> Hash {
        self.domain_hash
    }
}

impl NoritoKeyDigestRangeTreeV1 {
    fn backing(&self) -> &DigestTreeBacking {
        &self.inner
    }
    /// Root committing the exact key order, value digests, schema and domain.
    pub fn root(&self) -> Hash {
        self.backing().root
    }

    /// Number of rows supplied to the trusted builder.
    pub fn len(&self) -> usize {
        self.backing().entries.as_slice().len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.backing().entries.as_slice().is_empty()
    }

    /// Borrow retained keys and digests without claiming value preimages.
    pub fn digest_rows(&self) -> impl ExactSizeIterator<Item = (&[u8], Hash)> + '_ {
        self.backing()
            .entries
            .as_slice()
            .iter()
            .map(|entry| (entry.key.as_slice(), entry.value_digest))
    }

    fn siblings(&self, index: usize) -> Result<Vec<Hash>, NoritoKeyRangeError> {
        let levels = self.backing().levels.as_slice();
        let depth = levels.len().saturating_sub(1);
        let mut siblings = Vec::new();
        reserve_exact(&mut siblings, depth)?;
        let mut position = index;
        for nodes in levels.iter().take(depth) {
            siblings.push(nodes.as_slice()[position ^ 1]);
            position /= 2;
        }
        Ok(siblings)
    }

    fn boundary(&self, index: usize) -> Result<Boundary, NoritoKeyRangeError> {
        let entry = &self.backing().entries.as_slice()[index];
        Ok(Boundary {
            index: u32::try_from(index).expect("bounded entry index fits u32"),
            key: copy_bytes(entry.key.as_slice())?,
            value_digest: entry.value_digest,
            siblings: self.siblings(index)?,
        })
    }

    /// Prove every key and value digest in `[start, end)`, with immediate neighbors.
    ///
    /// # Errors
    /// Rejects malformed intervals, resource bounds and allocation failures.
    pub fn prove_range(
        &self,
        start: &[u8],
        end: &[u8],
        max_rows: usize,
        max_bytes: usize,
    ) -> Result<NoritoKeyDigestRangeProofV1, NoritoKeyRangeError> {
        validate_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        let first = self
            .backing()
            .entries
            .as_slice()
            .partition_point(|entry| entry.key.as_slice() < start);
        let after = self
            .backing()
            .entries
            .as_slice()
            .partition_point(|entry| entry.key.as_slice() < end);
        let count = after - first;
        if count > max_rows {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let path_bytes = depth(
            u32::try_from(self.backing().entries.as_slice().len()).expect("bounded count fits u32"),
        )?
        .checked_mul(Hash::LENGTH)
        .ok_or(NoritoKeyRangeError::Capacity)?;
        let mut size = 0_usize;
        add_size(&mut size, PROOF_HEADER_BYTES, max_bytes)?;
        for entry in &self.backing().entries.as_slice()[first..after] {
            add_size(
                &mut size,
                ROW_HEADER_BYTES + Hash::LENGTH + path_bytes,
                max_bytes,
            )?;
            add_size(&mut size, entry.key.as_slice().len(), max_bytes)?;
        }
        for index in [
            first.checked_sub(1),
            (after < self.backing().entries.as_slice().len()).then_some(after),
        ]
        .into_iter()
        .flatten()
        {
            add_size(&mut size, BOUNDARY_HEADER_BYTES + path_bytes, max_bytes)?;
            add_size(
                &mut size,
                self.backing().entries.as_slice()[index]
                    .key
                    .as_slice()
                    .len(),
                max_bytes,
            )?;
        }
        let before = first
            .checked_sub(1)
            .map(|index| self.boundary(index))
            .transpose()?;
        let mut rows = Vec::new();
        reserve_exact(&mut rows, count)?;
        for index in first..after {
            let entry = &self.backing().entries.as_slice()[index];
            rows.push(DigestRow {
                index: u32::try_from(index).expect("bounded entry index fits u32"),
                key: copy_bytes(entry.key.as_slice())?,
                value_digest: entry.value_digest,
                siblings: self.siblings(index)?,
            });
        }
        let after = (after < self.backing().entries.as_slice().len())
            .then(|| self.boundary(after))
            .transpose()?;
        Ok(NoritoKeyDigestRangeProofV1 {
            entries: u32::try_from(self.backing().entries.as_slice().len())
                .expect("bounded count fits u32"),
            before,
            rows,
            after,
        })
    }
}

impl NoritoKeyDigestRangeProofV1 {
    /// Entry count claimed by this untrusted witness.
    pub fn entry_count(&self) -> u32 {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::AllocationBudget;

    fn test_budget() -> AllocationBudget {
        AllocationBudget::new(64 * 1024 * 1024)
    }

    #[test]
    fn empty_digest_range_requires_the_complete_proof_header_budget() {
        let schema = Hash::new(b"schema");
        let tree = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
            schema,
            b"table",
            std::iter::empty::<(&[u8], Hash)>(),
            0,
            &test_budget(),
        )
        .unwrap();
        let proof = tree.prove_range(b"a", b"z", 0, 12).unwrap();
        for maximum in [0, 11, 12] {
            let built = tree.prove_range(b"a", b"z", 0, maximum);
            let verified = proof.verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema,
                domain: b"table",
                start: b"a",
                end: b"z",
                max_rows: 0,
                max_bytes: maximum,
            });
            if maximum < 12 {
                assert!(matches!(built, Err(NoritoKeyRangeError::Capacity)));
                assert!(matches!(verified, Err(NoritoKeyRangeError::Capacity)));
            } else {
                assert!(built.is_ok());
                assert!(verified.unwrap().is_empty());
            }
        }
    }

    #[test]
    fn digest_range_rejects_omission_and_digest_forgery() {
        let schema = Hash::new(b"schema");
        let value_a = super::super::digest_frame(super::super::VALUE_DOMAIN, b"a");
        let value_b = super::super::digest_frame(super::super::VALUE_DOMAIN, b"b");
        let tree = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
            schema,
            b"table",
            [(b"a".as_slice(), value_a), (b"b".as_slice(), value_b)],
            256,
            &test_budget(),
        )
        .unwrap();
        let disclosed = super::super::NoritoKeyRangeTreeV1::from_sorted(
            schema,
            b"table",
            [
                (b"a".as_slice(), b"a".as_slice()),
                (b"b".as_slice(), b"b".as_slice()),
            ],
        )
        .unwrap();
        assert_eq!(tree.root(), disclosed.root());
        let root = tree.root();
        let request = || NoritoKeyRangeVerifyRequestV1 {
            expected_root: &root,
            schema_hash: &schema,
            domain: b"table",
            start: b"a",
            end: b"c",
            max_rows: 2,
            max_bytes: 4096,
        };
        let proof = tree.prove_range(b"a", b"c", 2, 4096).unwrap();
        let verified = proof.verify(request()).unwrap();
        assert_eq!(verified.rows().count(), 2);
        assert_eq!(verified.root(), tree.root());
        assert_eq!(verified.schema_hash(), schema);
        assert_eq!(verified.domain_hash(), Hash::new(b"table"));
        assert!(matches!(
            proof.verify(NoritoKeyRangeVerifyRequestV1 {
                domain: b"other-table",
                ..request()
            }),
            Err(NoritoKeyRangeError::RootMismatch)
        ));
        let mut omitted = proof.clone();
        omitted.rows.remove(0);
        assert_eq!(
            omitted.verify(request()).err(),
            Some(NoritoKeyRangeError::InvalidProof)
        );
        let mut forged = proof;
        forged.rows[0].value_digest = value_b;
        assert_eq!(
            forged.verify(request()).err(),
            Some(NoritoKeyRangeError::RootMismatch)
        );
    }

    #[test]
    fn digest_tree_retention_bound_includes_every_padded_merkle_level() {
        let schema = Hash::new(b"schema");
        let digest = Hash::new(b"value");
        let rows = [
            (b"a".as_slice(), digest),
            (b"b".as_slice(), digest),
            (b"c".as_slice(), digest),
        ];
        let bytes = 3 * (1 + Hash::LENGTH) + 7 * Hash::LENGTH;
        assert!(
            NoritoKeyDigestRangeTreeV1::from_sorted_digests(
                schema,
                b"table",
                rows,
                bytes,
                &test_budget()
            )
            .is_ok()
        );
        assert!(matches!(
            NoritoKeyDigestRangeTreeV1::from_sorted_digests(
                schema,
                b"table",
                rows,
                bytes - 1,
                &test_budget()
            ),
            Err(NoritoKeyRangeError::Capacity)
        ));
    }

    #[test]
    fn streamed_full_frame_matches_disclosed_merkle_digest() {
        let bytes = vec![7_u8; 1024 * 1024 + 17];
        let frame_len = u32::try_from(bytes.len()).expect("bounded fixture frame");
        let streamed = digest_norito_value_frame_v1(frame_len, |writer| {
            for chunk in bytes.chunks(31 * 1024) {
                writer.write_all(chunk)?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            streamed,
            super::super::digest_frame(super::super::VALUE_DOMAIN, &bytes)
        );
        assert!(
            digest_norito_value_frame_v1(frame_len - 1, |writer| { writer.write_all(&bytes) })
                .is_err()
        );
        assert!(
            digest_norito_value_frame_v1(frame_len + 1, |writer| { writer.write_all(&bytes) })
                .is_err()
        );
        assert!(
            digest_norito_value_frame_v1(1, |writer| {
                let _ignored = writer.write_all(b"oversized");
                writer.write_all(b"x")
            })
            .is_err()
        );
    }
}

#[cfg(test)]
#[path = "digest/funding_tests.rs"]
mod funding_tests;
