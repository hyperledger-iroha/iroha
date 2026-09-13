//! One-column basis conversions for immutable confidential polynomial snapshots.
//!
//! Authenticated chunk callbacks fill one guarded decoded field column. The backend's shared
//! plaintext window closes after each callback; this owned column remains separate until the
//! transform and sequential destination writes finish. No backend operation is nested within
//! another callback, and no field column or column bank escapes from this adapter.
//!
//! The owned payload is one size-2^k field column plus one 256-scalar encoding chunk. The trusted
//! backend additionally owns its one plaintext chunk. The borrowed domain transform uses the
//! existing in-place baseline FFT with public twiddles, without witness scratch allocations.
//! This does not bound caller copies, public domain tables, arithmetic temporaries, allocator or
//! kernel memory, or whole-process RSS. It establishes no 128 MiB or performance qualification.
//!
//! TODO: Integrate into the consuming prover only after its argument, quotient and opening
//! lifetimes are bounded. The existing prover and its FFT dispatch remain unchanged.

use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

use ff::WithSmallOrderMulGroup;

use super::{
    STORED_SCALARS_PER_CHUNK_V1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
    StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialSnapshotV1,
    StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
};
use crate::poly::EvaluationDomain;

struct FieldColumn<F: StoredAssignmentFieldV1>(Vec<F>);

impl<F: StoredAssignmentFieldV1> FieldColumn<F> {
    fn zeroed(len: usize) -> Result<Self, StoredPolynomialErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| StoredPolynomialErrorV1::Allocation)?;
        values.resize(len, F::ZERO);
        Ok(Self(values))
    }

    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: these are exclusive initialized slots of sealed Copy Pasta fields.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

impl<F: StoredAssignmentFieldV1> Drop for FieldColumn<F> {
    fn drop(&mut self) {
        self.clear();
    }
}

struct EncodedChunk([[u8; 32]; STORED_SCALARS_PER_CHUNK_V1]);

impl EncodedChunk {
    fn clear(&mut self) {
        for scalar in &mut self.0 {
            for byte in scalar {
                // SAFETY: each byte is an exclusive initialized byte in this owned buffer.
                unsafe { ptr::write_volatile(byte, 0) };
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}

impl Drop for EncodedChunk {
    fn drop(&mut self) {
        self.clear();
    }
}

fn coset_factor<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    basis: StoredPolynomialBasisV1,
) -> Result<Option<F>, StoredPolynomialErrorV1> {
    match basis {
        StoredPolynomialBasisV1::Coefficient | StoredPolynomialBasisV1::Lagrange => Ok(None),
        StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part,
        } => {
            if extension_log == 0
                || extension_log != domain.extended_k() - domain.k()
                || extension_log >= u32::BITS
                || part >= 1_u32 << extension_log
            {
                return Err(StoredPolynomialErrorV1::Context);
            }
            Ok(Some(
                domain.get_extended_omega().pow_vartime([u64::from(part)]),
            ))
        }
    }
}

/// Convert one stored column to base evaluations, coefficients, or one exact coset part.
///
/// The source remains available on success. The provider must be its per-proof provider: the
/// destination preserves the proof context, field, k and complete role, and receives a strictly
/// newer ordinal. Equal bases produce a fresh authenticated copy. Different coset parts pass
/// through coefficient form in the same owned allocation; no extended-domain bank is created.
///
/// The callback API is used only for individual authenticated reads. The decoded column retains
/// its zeroizing owner through every borrowed FFT and output operation, including unwind.
/// Zeroization covers these owned buffers, not prior field copies or arithmetic registers.
///
/// # Errors
/// Rejects mismatched identities, scalar fields, domain/coset geometry, noncanonical encodings,
/// allocation and backend failures. Undivided quotient numerator and aliased scratch are always rejected,
/// including equal-basis copies. Metadata preflights occur before reads. Read callback
/// failures poison the source according to the trusted backend contract; output failures drop
/// the incomplete destination. No partially converted snapshot is returned.
pub fn convert_stored_advice_v1<F, P, S>(
    domain: &EvaluationDomain<F>,
    provider: &mut P,
    source: &mut S,
    expected: StoredPolynomialLayoutV1,
    destination_basis: StoredPolynomialBasisV1,
) -> Result<<P::Writer as StoredPolynomialWriterV1>::Snapshot, StoredPolynomialErrorV1>
where
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    S: StoredPolynomialSnapshotV1,
{
    // Preserve geometry/source preflight before provider side effects in the public entry.
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context);
    }
    conversion_geometry::<F>(domain, expected, destination_basis)?;
    let writer = provider.create(
        expected.field(),
        destination_basis,
        expected.k(),
        expected.role(),
    )?;
    let destination = writer.layout();
    convert_stored_advice_with_writer_v1(
        domain,
        source,
        expected,
        destination_basis,
        writer,
        destination,
    )
}

fn conversion_geometry<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    expected: StoredPolynomialLayoutV1,
    destination_basis: StoredPolynomialBasisV1,
) -> Result<(Option<F>, Option<F>), StoredPolynomialErrorV1> {
    if expected.field() != F::STORED_FIELD
        || expected.k() != domain.k()
        || matches!(
            expected.role(),
            super::StoredPolynomialRoleV1::QuotientNumerator
                | super::StoredPolynomialRoleV1::QuotientAliasedPart { .. }
        )
    {
        return Err(StoredPolynomialErrorV1::Context);
    }
    StoredPolynomialLayoutV1::new(
        expected.proof_context,
        expected.ordinal(),
        expected.field(),
        destination_basis,
        expected.k(),
        expected.role(),
    )?;
    Ok((
        coset_factor(domain, expected.basis())?,
        coset_factor(domain, destination_basis)?,
    ))
}

/// Convert with the exact already-created destination receipt retained by a consuming owner.
///
/// Unlike recapturing a writer's metadata as authoritative, this checks the supplied immutable
/// receipt against its live writer before any source witness read and every write/seal. The
/// caller still owns key/domain provenance, global ordinal admission and whole-owner failure;
/// this helper exposes neither a new snapshot callback nor a reusable partial output.
pub(crate) fn convert_stored_advice_with_writer_v1<F, W, S>(
    domain: &EvaluationDomain<F>,
    source: &mut S,
    expected: StoredPolynomialLayoutV1,
    destination_basis: StoredPolynomialBasisV1,
    mut writer: W,
    destination: StoredPolynomialLayoutV1,
) -> Result<W::Snapshot, StoredPolynomialErrorV1>
where
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    W: StoredPolynomialWriterV1,
    S: StoredPolynomialSnapshotV1,
{
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context);
    }
    let (source_coset, destination_coset) =
        conversion_geometry::<F>(domain, expected, destination_basis)?;
    if destination.proof_context != expected.proof_context
        || destination.ordinal() <= expected.ordinal()
        || destination.field() != expected.field()
        || destination.basis() != destination_basis
        || destination.k() != expected.k()
        || destination.role() != expected.role()
        || writer.layout() != destination
    {
        return Err(StoredPolynomialErrorV1::Context);
    }

    let mut values = FieldColumn::<F>::zeroed(expected.scalar_count())?;
    for chunk in 0..expected.chunk_count() as u64 {
        if source.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        let count = expected.chunk_scalar_count(chunk)?;
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        source.with_chunk(expected, chunk, |encoded| {
            if encoded.len() != count {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            for (index, scalar) in encoded.iter().enumerate() {
                values.0[start + index] = Option::<F>::from(F::from_repr(*scalar))
                    .ok_or(StoredPolynomialErrorV1::Encoding)?;
            }
            Ok(())
        })?;
    }
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context);
    }
    if expected.basis() != destination_basis {
        if expected.basis() != StoredPolynomialBasisV1::Coefficient {
            domain.stored_column_transform_in_place(&mut values.0, true, source_coset);
        }
        if destination_basis != StoredPolynomialBasisV1::Coefficient {
            domain.stored_column_transform_in_place(&mut values.0, false, destination_coset);
        }
    }

    let mut encoded = EncodedChunk([[0; 32]; STORED_SCALARS_PER_CHUNK_V1]);
    for (index, chunk) in values.0.chunks(STORED_SCALARS_PER_CHUNK_V1).enumerate() {
        if writer.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context);
        }
        for (output, scalar) in encoded.0.iter_mut().zip(chunk) {
            *output = scalar.to_repr();
        }
        writer.write_chunk(index as u64, &encoded.0[..chunk.len()])?;
    }
    if writer.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context);
    }
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context);
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests;
