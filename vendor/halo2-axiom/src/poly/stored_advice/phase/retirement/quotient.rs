//! Consuming coefficient reads and exact quotient/cache writer creation.
//!
//! Every operation retains the original phase plan, parameter reference, challenge allocation,
//! coefficient snapshots and sole blind guards. Writer creation moves only the original global
//! cursor. A coset factory validates non-advice source metadata, not a detached witness: the
//! complete quotient owner must authenticate its actual instance/argument snapshot before and
//! after each factory, read and drop. No backend callback or mutable snapshot escapes here.

use super::{CoefficientOnlyStoredAdviceV1, StoredPhaseErrorV1};
use crate::{
    arithmetic::CurveAffine,
    poly::stored_advice::{
        STORED_MAX_K_V1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
        StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
        StoredPolynomialSnapshotV1, StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
    },
};
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

/// A borrow of caller-owned initialized fields; successful copy alone keeps the output.
struct Destination<'a, F: StoredAssignmentFieldV1> {
    values: &'a mut [F],
    keep: bool,
}

impl<F: StoredAssignmentFieldV1> Drop for Destination<'_, F> {
    fn drop(&mut self) {
        if !self.keep {
            for value in self.values.iter_mut() {
                // SAFETY: exclusive initialized slots of the sealed, Copy Pasta fields.
                unsafe { ptr::write_volatile(value, F::ZERO) };
            }
            compiler_fence(Ordering::SeqCst);
        }
    }
}

fn coset_basis(
    k: u32,
    extension_log: u32,
    part: u32,
) -> Result<StoredPolynomialBasisV1, StoredPhaseErrorV1> {
    if k > STORED_MAX_K_V1
        || extension_log == 0
        || extension_log > STORED_MAX_K_V1 - k
        || part >= (1_u32 << extension_log)
    {
        return Err(StoredPhaseErrorV1::Admission);
    }
    Ok(StoredPolynomialBasisV1::CosetPart {
        extension_log,
        part,
    })
}

impl<'params, C, S> CoefficientOnlyStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Copy one exact original coefficient chunk without exposing a source or callback.
    ///
    /// The supplied destination is cleared on every error or unwind, including preflight.
    /// Its complete allocation and successful output remain owned by the outer guarded stage.
    /// Canonical decoding and all retained advice receipts must succeed before returning the
    /// original owner; an error destroys every coefficient snapshot and sole advice blind.
    pub(crate) fn copy_coefficient_chunk_into(
        mut self,
        column: u32,
        chunk: u64,
        output: &mut [C::Scalar],
    ) -> Result<Self, StoredPhaseErrorV1> {
        let mut destination = Destination {
            values: output,
            keep: false,
        };
        self.validate_live_receipts()?;
        let index = usize::try_from(column).map_err(|_| StoredPhaseErrorV1::Admission)?;
        let source = self
            .columns
            .get_mut(index)
            .ok_or(StoredPhaseErrorV1::Admission)?;
        let expected = source.coefficient.layout;
        if expected.advice_coordinates()?.0 != column
            || expected.basis() != StoredPolynomialBasisV1::Coefficient
            || source.coefficient.snapshot.layout() != expected
            || destination.values.len() != expected.chunk_scalar_count(chunk)?
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut decoded = false;
        source
            .coefficient
            .snapshot
            .with_chunk(expected, chunk, |encoded| {
                if encoded.len() != destination.values.len()
                    || encoded
                        .iter()
                        .any(|value| !expected.field().is_canonical(value))
                {
                    return Err(StoredPolynomialErrorV1::Encoding);
                }
                for (output, value) in destination.values.iter_mut().zip(encoded) {
                    *output =
                        Option::<C::Scalar>::from(<C::Scalar as ff::PrimeField>::from_repr(*value))
                            .ok_or(StoredPolynomialErrorV1::Encoding)?;
                }
                decoded = true;
                Ok(())
            })?;
        if !decoded || source.coefficient.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate_live_receipts()?;
        destination.keep = true;
        Ok(self)
    }

    /// Preflight output opportunities against the original high-water mark and next cursor.
    ///
    /// Zero outputs returns the original mark without demanding another provider ordinal.
    /// The first actual provider ordinal can jump; the complete owner must then check its
    /// remaining opportunities again from that actual ordinal before private reads/writes.
    pub(crate) fn quotient_ordinal_boundary(
        &self,
        outputs: usize,
    ) -> Result<Option<u64>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        let outputs = u64::try_from(outputs).map_err(|_| StoredPhaseErrorV1::Admission)?;
        if outputs != 0 {
            if let Some(last) = self.greatest_ordinal {
                last.checked_add(outputs)
                    .and_then(|last| last.checked_add(1))
                    .ok_or(StoredPolynomialErrorV1::Capacity)?;
            }
        }
        Ok(self.greatest_ordinal)
    }

    /// Create the exact numerator part; only this factory can establish an empty owner's context.
    ///
    /// Returned identity comes from the actual provider writer and is checked twice before
    /// updating the retained cursor. The outer owner retains and checks this writer thereafter.
    pub(crate) fn create_quotient_writer<P>(
        self,
        provider: &mut P,
        extension_log: u32,
        part: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        let basis = coset_basis(self.plan.k, extension_log, part)?;
        self.create_quotient_output(provider, basis, StoredPolynomialRoleV1::QuotientNumerator)
    }

    /// Create one role-preserving coset cache entry under the original established context.
    ///
    /// Advice metadata must be the exact retained physical coefficient receipt. For instance
    /// and argument roles this validates metadata only: the enclosing quotient owner owns and
    /// authenticates the actual source and retained-key index bounds before and after creation.
    /// A matching metadata value is never proof of a non-advice snapshot's existence or content.
    pub(crate) fn create_coset_writer<P>(
        self,
        provider: &mut P,
        source: StoredPolynomialLayoutV1,
        extension_log: u32,
        part: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        let basis = coset_basis(self.plan.k, extension_log, part)?;
        if source.field() != self.plan.field
            || source.k() != self.plan.k
            || source.basis() != StoredPolynomialBasisV1::Coefficient
            || self.proof_context != Some(source.proof_context)
            || self
                .greatest_ordinal
                .is_none_or(|last| source.ordinal() > last)
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        match source.role() {
            StoredPolynomialRoleV1::Advice { column, .. } => {
                let index = usize::try_from(column).map_err(|_| StoredPhaseErrorV1::Admission)?;
                if self
                    .columns
                    .get(index)
                    .is_none_or(|original| original.coefficient.layout != source)
                {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
            }
            StoredPolynomialRoleV1::Instance { .. }
            | StoredPolynomialRoleV1::CopyPermutationProduct { .. }
            | StoredPolynomialRoleV1::LookupPermuted { .. }
            | StoredPolynomialRoleV1::LookupProduct { .. } => (),
            _ => return Err(StoredPolynomialErrorV1::Context.into()),
        }
        self.create_quotient_output(provider, basis, source.role())
    }

    /// Create an inverse-only alias under the already-established original proof context.
    ///
    /// The outer consuming owner validates the original numerator receipt and actual key
    /// extension. This factory owns the original cursor, not detached numerator authority.
    pub(crate) fn create_quotient_alias_writer<P>(
        self,
        provider: &mut P,
        extension_log: u32,
        part: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        coset_basis(self.plan.k, extension_log, part)?;
        if self.proof_context.is_none() {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.create_quotient_output(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            },
        )
    }

    /// Create one final coefficient piece using the original cursor and established context.
    /// The outer owner additionally requires increasing piece order and piece < original q.
    pub(crate) fn create_quotient_piece_writer<P>(
        self,
        provider: &mut P,
        piece: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        if self.plan.k > STORED_MAX_K_V1
            || piece >= (1_u32 << (STORED_MAX_K_V1 - self.plan.k))
            || self.proof_context.is_none()
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.create_quotient_output(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::QuotientPiece { piece },
        )
    }

    fn create_quotient_output<P>(
        mut self,
        provider: &mut P,
        basis: StoredPolynomialBasisV1,
        role: StoredPolynomialRoleV1,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.quotient_ordinal_boundary(1)?;
        let field = self.plan.field;
        let k = self.plan.k;
        let writer = provider.create(field, basis, k, role)?;
        let expected = writer.layout();
        if expected.field() != field
            || expected.k() != k
            || expected.basis() != basis
            || expected.role() != role
            || expected.proof_context == [0; 32]
            || self
                .proof_context
                .is_some_and(|old| expected.proof_context != old)
            || self
                .greatest_ordinal
                .is_some_and(|old| expected.ordinal() <= old)
            || writer.layout() != expected
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        // Even the final valid output must leave a representable provider next-ordinal cursor.
        expected
            .ordinal()
            .checked_add(1)
            .ok_or(StoredPolynomialErrorV1::Capacity)?;
        self.proof_context = Some(expected.proof_context);
        self.greatest_ordinal = Some(expected.ordinal());
        self.validate_live_receipts()?;
        // A retained snapshot's metadata callback can mutate the new writer during this sweep.
        if writer.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok((self, writer, expected))
    }
}

#[cfg(test)]
#[path = "quotient_tests.rs"]
mod tests;
