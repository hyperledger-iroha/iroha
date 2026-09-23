//! Consuming Horner folding of the original phase-owned advice blind.
//!
//! The enclosing opening prover owns and guards the derived accumulator. No original blind,
//! snapshot, or mutable phase reference is returned separately from its coefficient owner.

use super::{CoefficientOnlyStoredAdviceV1, StoredPhaseErrorV1};
use crate::{
    arithmetic::CurveAffine,
    poly::{
        commitment::Blind,
        stored_advice::{StoredPolynomialSnapshotV1, assignment::StoredAssignmentFieldV1},
    },
};
use ff::Field;
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

/// Caller-owned derived secret, cleared unless the complete original owner returns successfully.
struct Accumulator<'a, F: Field> {
    value: &'a mut Blind<F>,
    keep: bool,
}

impl<F: Field> Drop for Accumulator<'_, F> {
    fn drop(&mut self) {
        if !self.keep {
            // SAFETY: this is an exclusive borrow of an initialized Copy field element.
            unsafe { ptr::write_volatile(&mut self.value.0, F::ZERO) };
            compiler_fence(Ordering::SeqCst);
        }
    }
}

impl<'params, C, S> CoefficientOnlyStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Fold one sole original advice blind into the enclosing prover's guarded accumulator.
    ///
    /// The column is the original global advice index, never an ordinal or a replacement
    /// receipt. The exact operation is `accumulator * challenge + original_blind`. All live
    /// advice identities are checked on both sides. Errors and unwinds clear the supplied
    /// accumulator and destroy every original receipt and blind. Success returns the same
    /// complete owner, retaining its allocations, phase plan, challenges and global cursor.
    /// The enclosing prover must also validate its other sources before and after this call.
    /// The derived scalar contains the original blind's numerical contribution; this is not an
    /// extraction barrier. Its caller must keep the accumulator guarded throughout opening.
    /// TODO: connect this bridge to the complete stored IPA opening continuation.
    pub(crate) fn fold_opening_blind(
        self,
        column: u32,
        challenge: C::Scalar,
        accumulator: &mut Blind<C::Scalar>,
    ) -> Result<Self, StoredPhaseErrorV1> {
        let mut output = Accumulator {
            value: accumulator,
            keep: false,
        };
        self.validate_live_receipts()?;
        let index = usize::try_from(column).map_err(|_| StoredPhaseErrorV1::Admission)?;
        let original = self
            .columns
            .get(index)
            .ok_or(StoredPhaseErrorV1::Admission)?;
        if original.coefficient.layout.advice_coordinates()?.0 != column {
            return Err(StoredPhaseErrorV1::Admission);
        }
        output.value.0 *= challenge;
        output.value.0 += (original.blind.0).0;
        self.validate_live_receipts()?;
        output.keep = true;
        Ok(self)
    }
}
