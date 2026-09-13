//! Consuming lookup bridge retaining original advice, coefficients and sole blind guards.
//!
//! Only authenticated output-writer creation and the concrete expression evaluator cross this
//! boundary. No mutable source receipt, arbitrary session closure, RNG or transcript escapes.
//! An error or unwind drops the entire input owner; an outer argument owner must likewise drop
//! its partial outputs. This bridge alone does not complete an argument or produce a proof.

use ff::WithSmallOrderMulGroup;

use super::CoefficientStoredAdviceV1;
use crate::{
    arithmetic::CurveAffine,
    plonk::stored::{
        StoredAuxiliarySourceV1, StoredExpressionErrorV1, StoredExpressionPlanV1, StoredRowTileV1,
    },
    poly::{
        EvaluationDomain,
        commitment::Params,
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
            StoredPolynomialSnapshotV1, StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1, phase::StoredPhaseErrorV1,
        },
    },
};

fn expression_error(error: StoredPhaseErrorV1) -> StoredExpressionErrorV1 {
    match error {
        StoredPhaseErrorV1::Store(error) => error.into(),
        StoredPhaseErrorV1::Poisoned => StoredPolynomialErrorV1::Poisoned.into(),
        _ => StoredExpressionErrorV1::Context,
    }
}

impl<'params, C, S> CoefficientStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Return the original parameter reference retained through phase admission and staging.
    pub(crate) fn params(&self) -> Result<&'params ParamsIPA<C>, StoredPhaseErrorV1> {
        self.lagrange.params()
    }

    /// Return immutable original advice identities in global column order.
    pub(crate) fn layouts(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = StoredPolynomialLayoutV1> + '_, StoredPhaseErrorV1>
    {
        self.lagrange.layouts()
    }

    /// Return public original phase challenges in global challenge-index order.
    pub(crate) fn challenges(
        &self,
    ) -> Result<impl Iterator<Item = C::Scalar> + '_, StoredPhaseErrorV1> {
        self.lagrange.challenges()
    }

    /// Return the actual storage context, including the first zero-advice output if created.
    pub(crate) fn proof_context(&self) -> Result<Option<[u8; 32]>, StoredPhaseErrorV1> {
        self.lagrange.params()?;
        Ok(self.proof_context)
    }

    /// Recheck phase geometry and every captured source/coefficient identity before lookup use.
    ///
    /// The outer concrete prover supplies its retained key domain. Matching geometry is not
    /// authentication of a detached key or parameter artifact; their ownership remains above.
    pub(crate) fn validate_for_lookup(
        &self,
        domain: &EvaluationDomain<C::Scalar>,
    ) -> Result<(), StoredPhaseErrorV1>
    where
        C::Scalar: WithSmallOrderMulGroup<3>,
    {
        self.validate_live_receipts()?;
        if domain.k() != self.params()?.k() {
            return Err(StoredPhaseErrorV1::Admission);
        }
        Ok(())
    }

    /// Validate all original and coefficient receipts, including unused advice columns.
    ///
    /// Lookup outputs belong to the outer owner and require its own final all-output sweep.
    /// The continuation high-water mark can exceed the final coefficient ordinal once such
    /// outputs exist, but can never fall below any original or coefficient receipt.
    pub(crate) fn validate_live_receipts(&self) -> Result<(), StoredPhaseErrorV1> {
        let session = self
            .lagrange
            .session
            .as_ref()
            .ok_or(StoredPhaseErrorV1::Poisoned)?;
        let plan = &session.plan;
        if plan.k > STORED_MAX_K_V1
            || plan.params.k() != plan.k
            || plan.params.n() != 1_u64 << plan.k
            || plan.params.get_g_lagrange().len() != 1_usize << plan.k
            || plan.field != C::Scalar::STORED_FIELD
            || plan.usable_rows > 1_usize << plan.k
            || session.columns.len() != plan.columns
            || self.coefficients.len() != plan.columns
            || session.next_phase != plan.phases.len()
            || plan.phases.is_empty()
            || plan.phases.len() > 3
            || session.challenges.len() != plan.challenges
            || session.challenges.iter().any(Option::is_none)
            || (plan.columns == 0 && (plan.phases.len() != 1 || plan.challenges != 0))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut scheduled_columns = 0_usize;
        let mut scheduled_challenges = 0_usize;
        let mut greatest_source = None;
        for (phase_index, phase) in plan.phases.iter().enumerate() {
            if (phase.columns.is_empty() && (plan.columns != 0 || !phase.challenges.is_empty()))
                || phase.columns.windows(2).any(|pair| pair[0] >= pair[1])
                || phase.challenges.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            scheduled_columns = scheduled_columns
                .checked_add(phase.columns.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            scheduled_challenges = scheduled_challenges
                .checked_add(phase.challenges.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            for index in &phase.challenges {
                if *index >= plan.challenges
                    || plan.phases[..phase_index]
                        .iter()
                        .any(|earlier| earlier.challenges.binary_search(index).is_ok())
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
            }
            for index in &phase.columns {
                let column = session
                    .columns
                    .get(*index)
                    .ok_or(StoredPhaseErrorV1::Admission)?;
                if column.layout.role()
                    != (StoredPolynomialRoleV1::Advice {
                        column: u32::try_from(*index).map_err(|_| StoredPhaseErrorV1::Admission)?,
                        phase: phase_index as u8,
                    })
                    || greatest_source.is_some_and(|old| column.layout.ordinal() <= old)
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
                greatest_source = Some(column.layout.ordinal());
            }
        }
        if scheduled_columns != plan.columns
            || scheduled_challenges != plan.challenges
            || greatest_source != session.greatest_ordinal
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut greatest = greatest_source;
        for (index, (source, coefficient)) in
            session.columns.iter().zip(&self.coefficients).enumerate()
        {
            let original = source.layout;
            let copied = coefficient.layout;
            if original
                .advice_coordinates()
                .map_err(|_| StoredPhaseErrorV1::Admission)?
                .0 as usize
                != index
                || original.field() != plan.field
                || original.k() != plan.k
                || original.basis() != StoredPolynomialBasisV1::Lagrange
                || original.proof_context == [0; 32]
                || session.proof_context != Some(original.proof_context)
                || self.proof_context != session.proof_context
                || copied.field() != original.field()
                || copied.k() != original.k()
                || copied.role() != original.role()
                || copied.basis() != StoredPolynomialBasisV1::Coefficient
                || copied.proof_context != original.proof_context
                || greatest.is_some_and(|old| copied.ordinal() <= old)
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            if source.snapshot.layout() != original || coefficient.snapshot.layout() != copied {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            greatest = Some(copied.ordinal());
        }
        if (session.columns.is_empty()
            && (session.proof_context.is_some() || session.greatest_ordinal.is_some()))
            || self.proof_context == Some([0; 32])
            || self.proof_context.is_some() != self.greatest_ordinal.is_some()
            || greatest.is_some_and(|old| self.greatest_ordinal.is_none_or(|last| last < old))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        Ok(())
    }

    /// Evaluate a concrete expression tile, consuming both bases on any error or unwind.
    ///
    /// All original and coefficient identities are checked before and after the real evaluator.
    /// In particular, success from a consumer cannot restore a changed unused coefficient.
    pub(crate) fn with_expression_sources<V, X>(
        mut self,
        plan: &StoredExpressionPlanV1<'_, C::Scalar>,
        tile: StoredRowTileV1,
        auxiliary: &mut X,
        challenges: &[C::Scalar],
        consume: impl FnOnce(&[C::Scalar]) -> Result<V, StoredExpressionErrorV1>,
    ) -> Result<(Self, V), StoredExpressionErrorV1>
    where
        X: StoredAuxiliarySourceV1<C::Scalar>,
    {
        self.validate_live_receipts().map_err(expression_error)?;
        let result = self
            .lagrange
            .with_expression_sources(plan, tile, auxiliary, challenges, consume)?;
        self.validate_live_receipts().map_err(expression_error)?;
        Ok((self, result))
    }

    /// Create one authenticated lookup-output writer under the single continuation ordinal.
    ///
    /// Capture the full first writer identity before any witness read, check it against the
    /// request, then compare a second live observation against that immutable expectation.
    /// Zero-advice proofs establish their context only from this actual provider writer.
    /// The returned writer stays owned by the outer consuming argument stage, which must check
    /// this exact layout before/after every write and seal and drop partial outputs on failure.
    pub(crate) fn create_output_writer<P>(
        self,
        provider: &mut P,
        role: StoredPolynomialRoleV1,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        if !matches!(
            role,
            StoredPolynomialRoleV1::LookupCompressed { .. }
                | StoredPolynomialRoleV1::LookupSorted { .. }
                | StoredPolynomialRoleV1::LookupLeftoverTable { .. }
        ) {
            return Err(StoredPhaseErrorV1::Admission);
        }
        self.create_argument_writer(provider, StoredPolynomialBasisV1::Lagrange, role)
    }

    /// Allocate a permutation output under the original proof's single ordinal high-water mark.
    /// Only the concrete consuming prover supplies these retained-key coordinates; no receipt
    /// callback, detached parameter/domain or basis outside this stage is admitted here.
    pub(crate) fn create_permuted_writer<P>(
        self,
        provider: &mut P,
        lookup: u32,
        side: crate::poly::stored_advice::StoredLookupSideV1,
        basis: StoredPolynomialBasisV1,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        if !matches!(
            basis,
            StoredPolynomialBasisV1::Lagrange | StoredPolynomialBasisV1::Coefficient
        ) {
            return Err(StoredPhaseErrorV1::Admission);
        }
        self.create_argument_writer(
            provider,
            basis,
            StoredPolynomialRoleV1::LookupPermuted { lookup, side },
        )
    }

    fn create_argument_writer<P>(
        mut self,
        provider: &mut P,
        basis: StoredPolynomialBasisV1,
        role: StoredPolynomialRoleV1,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.validate_live_receipts()?;
        let session = self
            .lagrange
            .session
            .as_ref()
            .ok_or(StoredPhaseErrorV1::Poisoned)?;
        let field = session.plan.field;
        let k = session.plan.k;
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
        self.proof_context = Some(expected.proof_context);
        self.greatest_ordinal = Some(expected.ordinal());
        // A shared backend can mutate an existing receipt even during writer creation.
        self.validate_live_receipts()?;
        Ok((self, writer, expected))
    }
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;

impl<'params, C, S> CoefficientStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Preflight a retained-key product output count against the original ordinal cursor.
    /// The first returned writer must additionally leave room from its actual provider ordinal.
    pub(crate) fn product_ordinal_boundary(
        &self,
        outputs: usize,
    ) -> Result<Option<u64>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        let count = u64::try_from(outputs).map_err(|_| StoredPhaseErrorV1::Admission)?;
        if count != 0 {
            if let Some(last) = self.greatest_ordinal {
                last.checked_add(count)
                    .and_then(|last| last.checked_add(1))
                    .ok_or(StoredPolynomialErrorV1::Capacity)?;
            }
        }
        Ok(self.greatest_ordinal)
    }

    /// Create one copy-product coefficient writer under the original shared ordinal cursor.
    pub(crate) fn create_copy_product_writer<P>(
        self,
        provider: &mut P,
        set: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.create_argument_writer(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::CopyPermutationProduct { set },
        )
    }

    /// Create one lookup-product coefficient writer under the original shared ordinal cursor.
    pub(crate) fn create_lookup_product_writer<P>(
        self,
        provider: &mut P,
        lookup: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.create_argument_writer(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::LookupProduct { lookup },
        )
    }

    /// Create one original instance coefficient under the single original cursor.
    pub(crate) fn create_instance_writer<P>(
        self,
        provider: &mut P,
        column: u32,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.create_argument_writer(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::Instance { column },
        )
    }
    /// Create the sole vanishing random coefficient under the single original cursor.
    pub(crate) fn create_vanishing_writer<P>(
        self,
        provider: &mut P,
    ) -> Result<(Self, P::Writer, StoredPolynomialLayoutV1), StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        self.create_argument_writer(
            provider,
            StoredPolynomialBasisV1::Coefficient,
            StoredPolynomialRoleV1::VanishingRandom,
        )
    }

    /// Copy an exact original advice chunk while retaining both bases and all sole blinds.
    ///
    /// No backend callback or receipt escapes. The outer concrete product owner must supply a
    /// guarded preallocated destination and destroy it with its partial outputs on failure or
    /// unwind. Actual decoder completion, every original/coefficient receipt and exact returned
    /// scalar encodings are checked before restoring the consumed advice owner.
    pub(crate) fn copy_lagrange_chunk_into(
        mut self,
        column: u32,
        chunk: u64,
        output: &mut [C::Scalar],
    ) -> Result<Self, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        let session = self
            .lagrange
            .session
            .as_mut()
            .ok_or(StoredPhaseErrorV1::Poisoned)?;
        let source = session
            .columns
            .get_mut(column as usize)
            .ok_or(StoredPhaseErrorV1::Admission)?;
        let expected = source.layout;
        if expected.advice_coordinates()?.0 != column
            || expected.basis() != StoredPolynomialBasisV1::Lagrange
            || source.snapshot.layout() != expected
            || output.len() != expected.chunk_scalar_count(chunk)?
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut decoded = false;
        source.snapshot.with_chunk(expected, chunk, |encoded| {
            if encoded.len() != output.len()
                || encoded
                    .iter()
                    .any(|value| !expected.field().is_canonical(value))
            {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            for (output, value) in output.iter_mut().zip(encoded) {
                *output =
                    Option::<C::Scalar>::from(<C::Scalar as ff::PrimeField>::from_repr(*value))
                        .ok_or(StoredPolynomialErrorV1::Encoding)?;
            }
            decoded = true;
            Ok(())
        })?;
        if !decoded || source.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate_live_receipts()?;
        Ok(self)
    }
}
