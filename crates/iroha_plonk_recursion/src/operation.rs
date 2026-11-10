//! Shared arithmetic/hash lane access for operation constraints between verifiers.

use iroha_pasta::{PastaCurve, poseidon::PoseidonField};
use iroha_plonk::frontend::Error;
use iroha_plonk_gadgets::{
    GlueChip, RunningSumChip, ecc::EccChip, ff::FfChip, pow5_fq::DuplexChip,
};

use crate::verifier::VerifierChip;

/// Mutable lanes retaining the interpreter's exact row cursors.
/// No transcript, key, foreign-field state or accumulator is reset.
#[derive(Debug)]
pub struct OperationLanes<'a, F: PoseidonField> {
    /// Existing glue lane.
    pub glue: &'a mut GlueChip<F>,
    /// Existing integer range-check lane.
    pub range: &'a mut RunningSumChip<F>,
    /// Cleared RP57 lane, using the same columns as recursive transcripts.
    pub hash: &'a mut DuplexChip<F>,
}

/// Mutable lanes a folding verifier drives on this interpreter's columns: the
/// operation lanes plus foreign arithmetic modulo the proof curve's scalar field
/// and the curve chip (the lanes of `iroha_fold::circuit::FoldLanes`). Row
/// cursors are retained; no transcript, key, accumulator or cache is reset.
#[derive(Debug)]
pub struct FoldLaneRefs<'a, C: PastaCurve> {
    /// Existing glue lane.
    pub glue: &'a mut GlueChip<C::Base>,
    /// Existing integer range-check lane.
    pub range: &'a mut RunningSumChip<C::Base>,
    /// Existing foreign-arithmetic chip, modulo the scalar field of `C`.
    pub ff: &'a mut FfChip<C::Base>,
    /// Existing native curve chip.
    pub ecc: &'a mut EccChip<C>,
    /// Cleared RP57 lane, using the same columns as recursive transcripts.
    pub hash: &'a mut DuplexChip<C::Base>,
}

impl<C: PastaCurve> VerifierChip<C> {
    /// Borrow operation lanes only between complete proof transcripts.
    ///
    /// Shared operation gadgets must preserve these chip cursors; the borrow
    /// prevents running a recursive verifier while its lanes are in use.
    ///
    /// # Errors
    /// A proof transcript still owns the duplex, or it has buffered state.
    pub fn operation_lanes(&mut self) -> Result<OperationLanes<'_, C::Base>, Error> {
        let hash = self.duplex.as_mut().ok_or(Error::Synthesis)?;
        if !hash.is_clear() || hash.buffered() != 0 {
            return Err(Error::Synthesis);
        }
        Ok(OperationLanes {
            glue: &mut self.glue,
            range: &mut self.range,
            hash,
        })
    }

    /// Borrow the lanes of a folding verifier only between complete proof
    /// transcripts, so a step that also verifies a proof needs no second chip
    /// set. A folding gadget that drives the FF chip directly does not consult
    /// or fill the interpreter's scalar caches; it keeps every row cursor.
    ///
    /// # Errors
    /// A proof transcript still owns the duplex, or it has buffered state.
    pub fn fold_lanes(&mut self) -> Result<FoldLaneRefs<'_, C>, Error> {
        let hash = self.duplex.as_mut().ok_or(Error::Synthesis)?;
        if !hash.is_clear() || hash.buffered() != 0 {
            return Err(Error::Synthesis);
        }
        Ok(FoldLaneRefs {
            glue: &mut self.glue,
            range: &mut self.range,
            ff: &mut self.arithmetic.ff,
            ecc: &mut self.ecc,
            hash,
        })
    }
}
