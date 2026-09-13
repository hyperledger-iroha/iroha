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
    STORED_SCALARS_PER_CHUNK_V1, StoredAdviceErrorV1, StoredAdviceLayoutV1, StoredAdviceProviderV1,
    StoredAdviceSnapshotV1, StoredAdviceWriterV1, StoredPolynomialBasisV1,
    assignment::StoredAssignmentFieldV1,
};
use crate::poly::EvaluationDomain;

struct FieldColumn<F: StoredAssignmentFieldV1>(Vec<F>);

impl<F: StoredAssignmentFieldV1> FieldColumn<F> {
    fn zeroed(len: usize) -> Result<Self, StoredAdviceErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| StoredAdviceErrorV1::Allocation)?;
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
) -> Result<Option<F>, StoredAdviceErrorV1> {
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
                return Err(StoredAdviceErrorV1::Context);
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
/// destination preserves the proof context, field, k, column and phase, and receives a strictly
/// newer ordinal. Equal bases produce a fresh authenticated copy. Different coset parts pass
/// through coefficient form in the same owned allocation; no extended-domain bank is created.
///
/// The callback API is used only for individual authenticated reads. The decoded column retains
/// its zeroizing owner through every borrowed FFT and output operation, including unwind.
/// Zeroization covers these owned buffers, not prior field copies or arithmetic registers.
///
/// # Errors
/// Rejects mismatched identities, scalar fields, domain/coset geometry, noncanonical encodings,
/// allocation and backend failures. Metadata preflights occur before reads. Read callback
/// failures poison the source according to the trusted backend contract; output failures drop
/// the incomplete destination. No partially converted snapshot is returned.
pub fn convert_stored_advice_v1<F, P, S>(
    domain: &EvaluationDomain<F>,
    provider: &mut P,
    source: &mut S,
    expected: StoredAdviceLayoutV1,
    destination_basis: StoredPolynomialBasisV1,
) -> Result<<P::Writer as StoredAdviceWriterV1>::Snapshot, StoredAdviceErrorV1>
where
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredAdviceProviderV1,
    S: StoredAdviceSnapshotV1,
{
    if source.layout() != expected
        || expected.field() != F::STORED_FIELD
        || expected.k() != domain.k()
    {
        return Err(StoredAdviceErrorV1::Context);
    }
    // Validate destination geometry before provider side effects, including the storage k cap.
    StoredAdviceLayoutV1::new(
        expected.proof_context,
        expected.ordinal(),
        expected.field(),
        destination_basis,
        expected.k(),
        expected.column(),
        expected.phase(),
    )?;
    let source_coset = coset_factor(domain, expected.basis())?;
    let destination_coset = coset_factor(domain, destination_basis)?;
    let mut writer = provider.create(
        expected.field(),
        destination_basis,
        expected.k(),
        expected.column(),
        expected.phase(),
    )?;
    let destination = writer.layout();
    if destination.proof_context != expected.proof_context
        || destination.ordinal() <= expected.ordinal()
        || destination.field() != expected.field()
        || destination.basis() != destination_basis
        || destination.k() != expected.k()
        || destination.column() != expected.column()
        || destination.phase() != expected.phase()
    {
        return Err(StoredAdviceErrorV1::Context);
    }

    let mut values = FieldColumn::<F>::zeroed(expected.scalar_count())?;
    for chunk in 0..expected.chunk_count() as u64 {
        if source.layout() != expected {
            return Err(StoredAdviceErrorV1::Context);
        }
        let count = expected.chunk_scalar_count(chunk)?;
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        source.with_chunk(expected, chunk, |encoded| {
            if encoded.len() != count {
                return Err(StoredAdviceErrorV1::Encoding);
            }
            for (index, scalar) in encoded.iter().enumerate() {
                values.0[start + index] = Option::<F>::from(F::from_repr(*scalar))
                    .ok_or(StoredAdviceErrorV1::Encoding)?;
            }
            Ok(())
        })?;
    }
    if source.layout() != expected {
        return Err(StoredAdviceErrorV1::Context);
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
            return Err(StoredAdviceErrorV1::Context);
        }
        for (output, scalar) in encoded.0.iter_mut().zip(chunk) {
            *output = scalar.to_repr();
        }
        writer.write_chunk(index as u64, &encoded.0[..chunk.len()])?;
    }
    if writer.layout() != destination {
        return Err(StoredAdviceErrorV1::Context);
    }
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredAdviceErrorV1::Context);
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests;
