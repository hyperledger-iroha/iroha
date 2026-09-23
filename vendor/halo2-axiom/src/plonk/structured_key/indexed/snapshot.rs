//! Private one-column bridge from an original indexed key to an exact stored receipt.
//!
//! The outer consuming owner must inseparably retain the authenticated original reader/index,
//! admitted key/profile, provider proof context and ordinal schedule. This helper checks local
//! identities and writes one snapshot; it authenticates neither the key source nor the owner.
//! A failed final source/freshness gate must destroy any returned snapshot at that outer owner.
//! TODO: connect that consuming owner before any Core or proof admission; no producer API is
//! established here and caller-selected replacement bytes are not an authorized key source.
//!
//! Charged scratch is one full initialized field column's actual Vec capacity and the complete
//! workspace header, including one 256-scalar encoding chunk. The original index/domain,
//! public FFT twiddles, reader/writer/backend memory, allocator overhead, stack temporaries,
//! compiler copies and arithmetic registers are additional costs. This is not an RSS bound.

use super::{IndexedStructuredProvingKeyV1, reads::IndexedKeyPolynomialV1};
use crate::{
    helpers::{SerdeCurveAffine, SerdePrimeField},
    poly::stored_advice::{
        STORED_SCALARS_PER_CHUNK_V1, StoredKeyMaskV1, StoredPolynomialBasisV1,
        StoredPolynomialErrorV1, StoredPolynomialLayoutV1, StoredPolynomialRoleV1,
        StoredPolynomialSnapshotV1, StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
    },
};
use ff::FromUniformBytes;
use std::{
    io::{self, Read, Seek},
    mem::size_of,
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

#[cfg(test)]
std::thread_local! {
    // Counts only post-clear initialized slots, never field values or surviving pointers.
    static CLEANUP_OBSERVATION: std::cell::Cell<[usize; 6]> = const { std::cell::Cell::new([0; 6]) };
}

#[cfg(test)]
pub(in crate::plonk::structured_key) fn reset_cleanup_observation() {
    CLEANUP_OBSERVATION.with(|record| record.set([0; 6]));
}

#[cfg(test)]
pub(in crate::plonk::structured_key) fn cleanup_observation() -> [usize; 6] {
    CLEANUP_OBSERVATION.with(std::cell::Cell::get)
}

struct FieldColumn<F: StoredAssignmentFieldV1>(Vec<F>);

impl<F: StoredAssignmentFieldV1> FieldColumn<F> {
    fn new(rows: usize) -> Result<Self, StoredPolynomialErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(rows)
            .map_err(|_| StoredPolynomialErrorV1::Allocation)?;
        // Establish the guard before initializing slots. The only initialization is ZERO.
        let mut column = Self(values);
        column.0.resize(rows, F::ZERO);
        Ok(column)
    }

    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: exclusively borrowed initialized Copy Pasta field slots admit ZERO.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

impl<F: StoredAssignmentFieldV1> Drop for FieldColumn<F> {
    fn drop(&mut self) {
        self.clear();
        #[cfg(test)]
        CLEANUP_OBSERVATION.with(|record| {
            let mut counts = record.get();
            counts[0] += 1;
            counts[1] += self.0.len();
            counts[2] += self.0.iter().filter(|value| **value != F::ZERO).count();
            record.set(counts);
        });
    }
}

struct EncodedChunk([[u8; 32]; STORED_SCALARS_PER_CHUNK_V1]);

impl EncodedChunk {
    fn clear(&mut self) {
        for scalar in &mut self.0 {
            for byte in scalar {
                // SAFETY: each byte is exclusively borrowed initialized owned storage.
                unsafe { ptr::write_volatile(byte, 0) };
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}

impl Drop for EncodedChunk {
    fn drop(&mut self) {
        self.clear();
        #[cfg(test)]
        CLEANUP_OBSERVATION.with(|record| {
            let mut counts = record.get();
            counts[3] += 1;
            counts[4] += size_of::<Self>();
            counts[5] += self.0.iter().flatten().filter(|byte| **byte != 0).count();
            record.set(counts);
        });
    }
}

struct Workspace<F: StoredAssignmentFieldV1> {
    column: FieldColumn<F>,
    encoded: EncodedChunk,
}

fn scratch_payload<F: StoredAssignmentFieldV1>(
    capacity: usize,
) -> Result<usize, StoredPolynomialErrorV1> {
    capacity
        .checked_mul(size_of::<F>())
        .and_then(|bytes| bytes.checked_add(size_of::<Workspace<F>>()))
        .ok_or(StoredPolynomialErrorV1::Allocation)
}

#[cfg(test)]
pub(in crate::plonk::structured_key) fn scratch_payload_for_tests<F: StoredAssignmentFieldV1>(
    capacity: usize,
) -> Result<usize, StoredPolynomialErrorV1> {
    scratch_payload::<F>(capacity)
}

fn read_error(error: io::Error) -> StoredPolynomialErrorV1 {
    match error.kind() {
        io::ErrorKind::InvalidData => StoredPolynomialErrorV1::Encoding,
        io::ErrorKind::OutOfMemory => StoredPolynomialErrorV1::Allocation,
        _ => StoredPolynomialErrorV1::Storage,
    }
}

impl<C: SerdeCurveAffine> IndexedStructuredProvingKeyV1<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64> + StoredAssignmentFieldV1,
{
    fn original_stored_role(
        &self,
        polynomial: IndexedKeyPolynomialV1,
    ) -> Result<StoredPolynomialRoleV1, StoredPolynomialErrorV1> {
        match polynomial {
            IndexedKeyPolynomialV1::MaskCoefficient(mask) => {
                let kind = match mask {
                    0 => StoredKeyMaskV1::L0,
                    1 => StoredKeyMaskV1::LLast,
                    2 => StoredKeyMaskV1::LActiveRow,
                    _ => return Err(StoredPolynomialErrorV1::Context),
                };
                Ok(StoredPolynomialRoleV1::KeyMask { kind })
            }
            IndexedKeyPolynomialV1::FixedLagrange(column) => {
                if column >= self.metadata.fixed.len() {
                    return Err(StoredPolynomialErrorV1::Context);
                }
                let column = u32::try_from(column).map_err(|_| StoredPolynomialErrorV1::Context)?;
                Ok(StoredPolynomialRoleV1::KeyFixed { column })
            }
            IndexedKeyPolynomialV1::PermutationLagrange(column) => {
                if column >= self.metadata.permutation_columns {
                    return Err(StoredPolynomialErrorV1::Context);
                }
                let column = u32::try_from(column).map_err(|_| StoredPolynomialErrorV1::Context)?;
                Ok(StoredPolynomialRoleV1::KeyPermutation { column })
            }
        }
    }

    /// Consume one already-admitted destination writer into its exact coefficient snapshot.
    ///
    /// The immutable expected receipt comes from the outer original-key owner, never from a
    /// newly sampled writer descriptor. That owner admits proof context/ordinal/provider and
    /// source identity, revalidates freshness after return and destroys the whole owner on any
    /// failure. This local operation neither returns a proof nor exposes a producer callback.
    ///
    /// Before source I/O, validate native column bounds, exact role/field/k/row/basis, the live
    /// writer descriptor and actual scratch capacity. Decode/transform once into one guarded
    /// column, then write exact sequential logical chunks. The backend supplies zero padding.
    /// Every descriptor mismatch, source/output failure or unwind drops the writer/snapshot
    /// and erases every initialized field/encoding slot. No partial snapshot is returned.
    pub(crate) fn into_coefficient_snapshot<R: Read + Seek, W: StoredPolynomialWriterV1>(
        &self,
        reader: &mut R,
        polynomial: IndexedKeyPolynomialV1,
        mut writer: W,
        expected: StoredPolynomialLayoutV1,
        scratch_limit_bytes: usize,
    ) -> Result<W::Snapshot, StoredPolynomialErrorV1> {
        let role = self.original_stored_role(polynomial)?;
        let k = self.vk.domain.k();
        let rows = 1_usize
            .checked_shl(k)
            .filter(|rows| *rows == self.metadata.rows)
            .ok_or(StoredPolynomialErrorV1::Context)?;
        if expected.field() != C::Scalar::STORED_FIELD
            || expected.k() != k
            || expected.scalar_count() != rows
            || expected.basis() != StoredPolynomialBasisV1::Coefficient
            || expected.role() != role
            || writer.layout() != expected
        {
            return Err(StoredPolynomialErrorV1::Context);
        }
        if scratch_payload::<C::Scalar>(rows)? > scratch_limit_bytes {
            return Err(StoredPolynomialErrorV1::Allocation);
        }
        let mut workspace = Workspace {
            column: FieldColumn::<C::Scalar>::new(rows)?,
            encoded: EncodedChunk([[0; 32]; STORED_SCALARS_PER_CHUNK_V1]),
        };
        if scratch_payload::<C::Scalar>(workspace.column.0.capacity())? > scratch_limit_bytes {
            return Err(StoredPolynomialErrorV1::Allocation);
        }
        if writer.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        self.copy_coefficient_column(reader, polynomial, &mut workspace.column.0)
            .map_err(read_error)?;
        for (chunk, values) in workspace
            .column
            .0
            .chunks(STORED_SCALARS_PER_CHUNK_V1)
            .enumerate()
        {
            if writer.layout() != expected {
                return Err(StoredPolynomialErrorV1::Context);
            }
            workspace.encoded.clear();
            for (encoded, value) in workspace.encoded.0.iter_mut().zip(values) {
                *encoded = <C::Scalar as ff::PrimeField>::to_repr(value);
            }
            writer.write_chunk(chunk as u64, &workspace.encoded.0[..values.len()])?;
            workspace.encoded.clear();
            if writer.layout() != expected {
                return Err(StoredPolynomialErrorV1::Context);
            }
        }
        // These fields have no later use; erase them before the external seal callback.
        workspace.column.clear();
        if writer.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        let snapshot = writer.seal()?;
        if snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::halo2curves::pasta::{Fp, Fq};

    fn scratch_accounting<F: StoredAssignmentFieldV1>() {
        for rows in [0, 1, 16, 256, 512] {
            let mut column = FieldColumn::<F>::new(rows).unwrap();
            let pointer = column.0.as_ptr();
            let capacity = column.0.capacity();
            assert_eq!(
                scratch_payload::<F>(capacity).unwrap(),
                capacity * size_of::<F>() + size_of::<Workspace<F>>()
            );
            assert!(
                scratch_payload::<F>(capacity).unwrap()
                    >= rows * size_of::<F>() + size_of::<Vec<F>>() + 8192
            );
            column.0.fill(F::ONE);
            column.clear();
            assert!(column.0.iter().all(|value| *value == F::ZERO));
            assert_eq!(column.0.as_ptr(), pointer);
            assert_eq!(column.0.capacity(), capacity);
        }
        assert_eq!(
            scratch_payload::<F>(usize::MAX),
            Err(StoredPolynomialErrorV1::Allocation)
        );
        assert_eq!(
            scratch_payload::<F>(usize::MAX / size_of::<F>()),
            Err(StoredPolynomialErrorV1::Allocation)
        );
    }

    #[test]
    fn both_fields_indexed_snapshot_payload_charges_actual_capacity_and_complete_workspace() {
        scratch_accounting::<Fp>();
        scratch_accounting::<Fq>();
    }

    #[test]
    fn indexed_snapshot_encoding_chunk_clears_every_initialized_byte() {
        let mut encoded = EncodedChunk([[255; 32]; STORED_SCALARS_PER_CHUNK_V1]);
        let pointer = encoded.0.as_ptr();
        encoded.clear();
        assert_eq!(encoded.0.as_ptr(), pointer);
        assert!(encoded.0.iter().flatten().all(|value| *value == 0));
    }

    #[test]
    fn indexed_snapshot_source_errors_remain_coarse() {
        assert_eq!(
            read_error(io::Error::from(io::ErrorKind::InvalidData)),
            StoredPolynomialErrorV1::Encoding
        );
        assert_eq!(
            read_error(io::Error::from(io::ErrorKind::OutOfMemory)),
            StoredPolynomialErrorV1::Allocation
        );
        for kind in [
            io::ErrorKind::UnexpectedEof,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            assert_eq!(
                read_error(io::Error::from(kind)),
                StoredPolynomialErrorV1::Storage
            );
        }
    }
}
