//! Checked indexing of one original structured proving-key frame without dense public banks.

use super::*;

/// A checked VK and immutable ranges into the exact structured frame supplied to the reader.
///
/// This owns no source bytes and provides no artifact authentication or release authority. The
/// caller must retain and authenticate the original frame, bind its role/profile, and compare the
/// embedded VK with the authenticated standalone VK before a proof consumer may use it. There is
/// no constructor from caller-supplied offsets and no clone or mutable VK accessor.
///
/// Indexing avoids mask, fixed and permutation field banks. Original VK/domain/selector metadata,
/// the complete permutation validation bitmap, and caller-owned bytes still require accounting.
pub struct IndexedStructuredProvingKeyV1<C: SerdeCurveAffine> {
    vk: VerifyingKey<C>,
    metadata: StructuredMetadata,
}

impl<C: SerdeCurveAffine> std::fmt::Debug for IndexedStructuredProvingKeyV1<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedStructuredProvingKeyV1")
            .field("rows", &self.metadata.rows)
            .field("frame_bytes", &self.metadata.frame_bytes)
            .field("fixed_columns", &self.metadata.fixed.len())
            .field("permutation_columns", &self.metadata.permutation_columns)
            .finish_non_exhaustive()
    }
}

impl<C: SerdeCurveAffine> IndexedStructuredProvingKeyV1<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Check one exact frame and emit its canonical encoding from validated values.
    ///
    /// Uses the same parser as the ordinary dense structured reader, with trusted k, byte length
    /// and original circuit parameters. The canonical writer is additionally bounded to the frame
    /// length. It may contain partial output after failure; the caller owns flushing, digest
    /// comparison and atomic publication. Bytes after the exact frame remain unread. No index is
    /// returned on parse, read or canonical-write failure or unwind.
    pub fn read_checked<R: Read, W: Write, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        expected_k: u32,
        expected_bytes: u64,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
        canonical_writer: &mut W,
    ) -> io::Result<Self> {
        let ScannedStructuredKey { vk, metadata, .. } =
            scan_structured::<C, _, _, ConcreteCircuit, NoValues>(
                reader,
                expected_k,
                expected_bytes,
                #[cfg(feature = "circuit-params")]
                params,
                canonical_writer,
                NoValues,
            )?;
        Ok(Self { vk, metadata })
    }

    /// Borrow the checked embedded verification key.
    pub fn get_vk(&self) -> &VerifyingKey<C> {
        &self.vk
    }

    /// Return the original checked number of rows in each polynomial.
    pub fn rows(&self) -> usize {
        self.metadata.rows
    }

    /// Return the exact original structured frame length in bytes.
    pub fn frame_bytes(&self) -> u64 {
        self.metadata.frame_bytes
    }

    pub(super) fn metadata(&self) -> &StructuredMetadata {
        &self.metadata
    }
}

// TODO: connect the original authenticated key owner before stored proof admission.
#[allow(dead_code)]
pub(crate) mod reads;

// TODO: connect the original authenticated consuming key owner before proof admission.
#[allow(dead_code)]
pub(crate) mod snapshot;
